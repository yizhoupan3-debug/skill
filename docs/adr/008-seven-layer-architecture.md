---
last_verified: "2026-06-22"
depends_on:
  - ../spec.md
---

# ADR-008: 七层架构分离

## Status

Accepted (2026-06-22).

## Context

v7/v8 架构中 `runtime-core` 膨胀至 ~7,000+ 行，承担了宿主协调、路由分发、工具注册、Skill 加载、Feature hook 等多重职责。具体问题包括：

1. **Layer 归属模糊**：`runtime-core` 同时包含运行时代码和 Feature 层逻辑，paper/test 等 hooks 直接嵌入核心运行时。
2. **工具注册表未抽象**：工具注册逻辑散布在 `runtime-core` 插件加载路径和 MCP 适配器中，无独立抽象层。
3. **跨宿主差异耦合**：不同宿主（Claude/Cursor/Codex/OpenCode）的路由策略、hook 事件集、权限策略混在同一个模块中，修改一个宿主逻辑可能影响其他宿主。
4. **Feature 扩展困难**：新增 Feature（如 research-harness）需侵入 `runtime-core`，没有稳定的扩展点。

## Decision

将框架拆分为 7 个层次，每层职责清晰且单向依赖：

1. **Host Layer**（宿主适配层）：每宿主一个轻薄适配器，负责 stdin/stdout 协议转换、生命周期事件映射、宿主特有 hook 事件集。不包含业务逻辑。
2. **Routing Layer**（路由层）：路由策略实现（意图匹配、skill 路由、tool 路由）。从 `runtime-core` 剥离为独立 crate，支持按宿主加载不同策略矩阵。
3. **Skill Layer**（技能层）：Skill 加载/执行引擎，负责 SKILL.md 注入、技能路由命中、技能生命周期。提供 `SkillProvider` trait 供宿主扩展。
4. **Tool Layer**（工具层）：工具注册表抽象（`ToolRegistry` trait），统一 MCP 工具、Native 工具、插件工具的注册/发现/调用。从 `runtime-core` 剥离。
5. **Runtime Layer**（运行时层）：`runtime-core` 瘦身为 ~3,000 行胶水层，负责层间编排、session 生命周期管理、配置加载、插件发现。不直接引用任何 Feature 或 Skill。内部包含四子系统：Behavior（loop-engine/goal/context）、Orchestration（session/multi-agent）、Infrastructure（transport/config/telemetry）、Exit Gate（quality-gate/closeout）。
6. **Hook Layer**（钩子层）：函数指针注册表（49+ OnceLock slots）、事件分发、review gate 调度。
7. **Feature Layer**（功能层）：领域特化插件。运用前面各层的能力做特化，例如 research-harness、paper hooks、closeout gates、review loop 等。

关键分离动作：
- Paper hooks 从 `runtime-core` 迁至 `research-harness`（Feature 层）
- 工具注册表抽象为 `ToolRegistry` trait，宿主/插件各自实现
- 路由策略按宿主独立 crate（`router-rs-claude`、`router-rs-cursor` 等）
- Hook 层从运行时解耦为独立函数指针注册表（OnceLock slots）
- Feature Plugin API 通过 trait + 事件订阅实现，不依赖具体 Feature 类型

## Consequences

- **优势**：
  - `runtime-core` 从 ~7,000 行瘦身至 ~3,000 行胶水层，职责清晰
  - Feature 扩展不再需改核心运行时，新增 Feature 只需实现 Plugin trait
  - 跨宿主差异隔离到 Router Layer 和 Host Layer，单宿主修改不影响其他宿主
  - 工具注册表统一后，MCP/原生/插件工具调用路径一致
- **代价**：
  - 7 层架构增加 crate 数量和编译时间
  - 首次拆分需重写现有 Feature 调用点为 Plugin trait 接口
  - 跨层调用增加间接性，影响热路径性能（可通过 trait 静态分发优化）
- **迁移策略**：在 `runtime-core` 中先定义接口 trait，逐层剥离；过渡期保留向后兼容的 re-export。

## Related

- `docs/spec.md` — 7 层架构规约
- `artifacts/current/roadmap-v8.md` §6 — 模块解耦 Wave
