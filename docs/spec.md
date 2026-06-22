---
last_verified: "2026-06-22"
version: "unified-v9"
---

# 框架统一规约 (Unified Framework Specification)

> 本文件是框架**总览规约**，覆盖架构总览、设计原则与七层模型（宿主层/路由层/Skill层/工具层/运行层/Hook层/Feature层）。
> 各子系统详细规约见下方 `extends` 延伸文档（各自在其领域内为真源）。
> 实施路线图见 `artifacts/current/roadmap-v9.md`（全栈治理）。
> v7 路线图已归档：`artifacts/current/roadmap-v7.md`（v7.0-final, 2026-06-18）。

---

## 目录

1. [架构总览](#1-架构总览)
2. [七层模型](#2-七层模型)
3. [Core Crates](spec/core-crates.md)
4. [多 Agent 编排契约](spec/multi-agent.md)
5. [跨宿主统一矩阵](spec/host-matrix.md) + [宿主接入契约](spec/host-matrix.md#7-宿主接入契约)
6. [路由与插件契约](spec/routing-plugin.md)
7. [运行时子系统](spec/runtime-subsystems.md) + [Hook 系统](spec/runtime-subsystems.md#10-hook-系统) + [传输与持久化](spec/runtime-subsystems.md#13-传输与持久化)
8. [安全守卫](spec/security-lifecycle.md) + [Closeout 与生命周期](spec/security-lifecycle.md#12-closeout-与生命周期)
9. [辅助模块](spec/auxiliary.md)
10. [可观测性](spec/observability-testing.md) + [存储压缩](spec/observability-testing.md#16-存储压缩) + [测试契约](spec/observability-testing.md#17-测试契约) + [Schema 索引](spec/observability-testing.md#18-schema-索引)

---

## 1. 架构总览

### 1.1 Crate 拓扑 (七层)

```
┌─────────────────────────────────────────────┐
│ Feature Layer    research-harness, paper    │
├─────────────────────────────────────────────┤
│ Hook Layer       hook-layer (hook registry) │
├─────────────────────────────────────────────┤
│ Runtime Layer    runtime-core               │
│  ├─ Behavior     loop-engine, goal, context │
│  ├─ Orchestrate  session, multi-agent       │
│  ├─ Infra        transport, config, telemetry│
│  └─ Exit Gate    quality-gate, closeout     │
├─────────────────────────────────────────────┤
│ Tool Layer       tool-layer (ToolRegistry)  │
├─────────────────────────────────────────────┤
│ Skill Layer      skill-layer (SKILL.md mgmt)│
├─────────────────────────────────────────────┤
│ Routing Layer    routing-engine, router-rs  │
├─────────────────────────────────────────────┤
│ Host Layer       host-projection (thin)     │
└─────────────────────────────────────────────┘
```

> 各 crate 的详细模块拆解、pub API 和技术债见 [`docs/modules/`](modules/) 下对应文档。

| 原则 | 含义 |
|------|------|
| **单一权威真源** | `RUNTIME_REGISTRY.json` 为宿主闭集唯一权威 |
| **L0/L4 解耦** | 宿主差异仅存于 L0 适配壳（host-projection） |
| **二元编排** | 仅 `subagent` + `workflow`；team 已废弃 |
| **纯 Rust 隔离** | PID + SQLite |
| **配置驱动接入** | 新宿主 ≤ 1 天（5 文件：provider + AGENTS + docs + feature + registry） |
| **Fail-closed** | 未知均默认拒绝 |
| **函数指针注册表** | hooks 通过 OnceLock 函数指针注册（非 trait），49 个 slots |
| **MCP 统一** | 4 个 MCP server 四宿主统一注册 |
| **用户级配置** | MCP/hooks/settings 配置只放用户级（~/.config/），不在项目目录 |

### 1.3 依赖关系约束 (v7 DAG)

```
router-rs → runtime-core → host-projection → core-state
                         → core-policy
                         → routing-engine
                         → framework-kernel (含 framework_profile)
                         → framework-runtime (extracted)
                         → session-supervisor (extracted)
                         → runtime-storage (extracted)
                         → trace-runtime (extracted)
                         → tools/codegraph-rs (optional feature)
                         → browser-mcp (optional)
```

- host-projection 包含所有宿主 hooks 实现（从 runtime-core 迁出）
- runtime-core 通过 re-export shim 向后兼容 `framework_runtime::*`、`session_supervisor::*` 等
- `framework-runtime`、`session-supervisor`、`runtime-storage`、`trace-runtime` 是 v7 从 runtime-core 提取的自洽 crate
- `codegraph-rs`、`evolution-rs` 位于 `tools/` 目录下；`research-harness` 位于 `core/` 目录下
- B0 core crates 不依赖 `router-rs`
- host 特有逻辑禁止出现在 B0 core crates 中

---

## 2. 七层模型

| 层 | 职责 | 允许 | 禁止 |
|----|------|------|------|
| L0 宿主层 | argv/stdin/stdout 协议转换 | 轻量适配，事件映射 | 业务逻辑、路由决策 |
| L1 路由层 | 意图匹配、skill/tool 路由 | 策略矩阵、评分 | 直接执行工具 |
| L2 Skill层 | SKILL.md 注入、技能生命周期 | verify_commands、契约 | 第二套连续性目录 |
| L3 工具层 | ToolRegistry、统一注册/发现 | MCP/原生/插件工具 | 宿主特定逻辑 |
| L4 运行层 | 层间编排、session 管理 | 行为/编排/基建/退出门 | Feature 逻辑侵入 |
| L5 Hook层 | 函数指针注册、事件分发 | 49 slots、review gate | 具体业务逻辑 |
| L6 Feature层 | 领域特化插件 | research-harness/paper | 核心运行时修改 |

---

## 2.1 工具路由 vs Skill 路由隔离边界

两条独立管线，共享 hook 事件流但职责分明：

| 维度 | Skill 路由 | 工具路由（Hook 门控） |
|------|-----------|---------------------|
| 入口 | UserPromptSubmit → `route_task()` | PreToolUse/PostToolUse → hook handler |
| 数据源 | `SKILL_ROUTING_RUNTIME.json` | `.claude/settings.json` matcher + Rust 层分类 |
| 分类 | `kind`: skill / framework_command | `ToolOrigin`: NativeHost / McpServer / Unknown |
| 联动 | `allowedTools` → `active-skill-context.json` → PreToolUse advisory |

### 隔离保证

- `allowedTools` 不参与 NL 评分（`RawSkillRecord` 不含此字段，记录加载时跳过）
- `keyword_tokens` 不含工具名（仅从 summary/trigger_hints/tags 构建）
- MCP 工具 FQN（`mcp__*__*`）格式与自然语言 query 差异大，不会误匹配
- NL 路由调整规则 `has_mcp_tool_invocation_intent` 在查询含 `mcp__` 或工具使用意图时 suppress 所有 skill

### 四宿主 Matcher 策略

| 宿主 | PreToolUse matcher | PostToolUse matcher | MCP 覆盖 |
|------|-------------------|---------------------|---------|
| Claude | `""` (全局) | `""` (全局) | ✅ 正则 `^mcp__` 或空 matcher |
| Cursor | 无 PreToolUse 事件 | 全局触发 | ✅ 自动 |
| Codex | `""` (全局) | `""` (全局) | ✅ 空 matcher |
| OpenCode | 全局 (TS 插件) | 全局 (TS 插件) | ✅ 自动 |

### 关键 Rust 类型

- `ToolOrigin`（`core_policy::hook_common`）：工具来源分类枚举
- `classify_tool_origin()`：分类工具为 NativeHost/McpServer/Unknown
- `parse_mcp_tool_fqn()`：解析 `mcp__{server}__{tool}` FQN
- `dangerous_mcp_tool_reason()`：MCP 工具安全审查（高风险名 + arg 模式 + shell 注入）

---

## extends 延伸文档

| 文档 | 覆盖章节 |
|------|---------|
| [spec/core-crates.md](spec/core-crates.md) | §3 Core Crates |
| [spec/multi-agent.md](spec/multi-agent.md) | §5 多 Agent 编排契约 |
| [spec/host-matrix.md](spec/host-matrix.md) | §6 跨宿主统一矩阵 + §7 宿主接入契约 |
| [spec/routing-plugin.md](spec/routing-plugin.md) | §8 路由与插件契约 |
| [spec/runtime-subsystems.md](spec/runtime-subsystems.md) | §9 运行时子系统 + §10 Hook 系统 + §13 传输与持久化 |
| [spec/security-lifecycle.md](spec/security-lifecycle.md) | §11 安全守卫 + §12 Closeout 与生命周期 |
| [spec/auxiliary.md](spec/auxiliary.md) | §14 辅助模块 |
| [spec/observability-testing.md](spec/observability-testing.md) | §15 可观测性 + §16 存储压缩 + §17 测试契约 + §18 Schema 索引 |
| [spec/research-harness.md](spec/research-harness.md) | §19 科研 Harness 系统 |
| [spec/loop-architecture.md](spec/loop-architecture.md) | Loop Architecture（v8，loop-auto 调度引擎） |
| [spec/performance-guide.md](spec/performance-guide.md) | §20 性能指南 |

---

## 契约漂移规则

本规约中的机器可读 Schema、状态流转图、指标定义是开发和测试的第一断言断点。

涉及上述配置规则的代码变更，**必须以"文档先行"形式首先修改本文件**，然后进行 Rust 实现与测试回归。

禁止：
- 从一个宿主复制 adapter 模板到另一个宿主而不改 key 名
- 测试夹具用"预期 bug 形态"反向锁死 bug
- 新增宿主/模块时不更新本文件
