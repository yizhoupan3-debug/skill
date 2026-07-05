# 框架五层架构映射（Layer Map）

> 真源文档；将 5 层语义模型与 L0-L7 构建层编号统一对齐。
> 所有 `core/*/Cargo.toml` 的 `description` 字段以此为准。
>
> 更新者每次添加/移动/重命名 crate 时须更新本文。

---

## 1. 总览

```
用户视角（5 层语义模型）         构建视角（L0-L7 编号）
─────────────────────          ─────────────────────
宿主层 (5L:host)               L5-L7（顶层入口）
路由层 (5L:route)              L0 + L4（路由原语 + 引擎）
Skill层 (5L:skill)             L2（元数据/发现/健康）
工具层 (5L:tool)               L4 + 独立二进制（注册 + 路由 + 执行）
运行层 (5L:runtime)            L0-L7（基础设施 + 引擎 + 聚合器）
```

**依赖方向**：宿主层 → 路由层 → Skill 层 → 工具层 → 运行层（单向向下）。

<!-- 注意：工具层依赖路由层（tool-routing-engine 依赖 routing-core），但上层的工具配置/注册不反向依赖下层的运行时引擎。调用链通过 fn 指针注册表实现依赖反转。 -->

---

## 2. 宿主层（5L:host）

宿主抽象、HostProvider 注册表、session/worker 生命周期管理、CLI 入口。

| Crate | 路径 | 声明的 L 编号 | 说明 |
|-------|------|-------------|------|
| `host-projection` | `core/host-projection` | L5 | **宿主层核心接口**：`HostProvider` trait 族 + 4 宿主 build.rs 代码生成 + 投影安装 + MCP stdio 桥 |
| `session-supervisor` | `core/session-supervisor` | L6 | **Worker 生命周期管理**：启动/检查/终止 worker 进程，多 agent 团队协调 |
| `router-rs` | `core/router-rs` | L7 | **CLI 入口**：hook 分发入口 + agent 分发入口 + 框架自检命令 |

**配置文件归属**：
- `configs/framework/RUNTIME_REGISTRY.json` → 宿主元数据和驱动 build.rs 代码生成
- `.mcp.json` → 宿主侧 MCP server 注册表

**宿主层 public API**：
- `HostProvider` trait（`host-projection/src/hosts/host_provider.rs`）
- `host_provider_for_id()` / `host_provider_registry()`（同上）
- `RuntimeHooks` struct（`host-projection/src/hooks.rs`）— 跨层回调注册表
- `session-supervisor` 的启动/检查 API

### 2.1 RuntimeHooks vs RuntimeCoreHooks 对照

> 两处 fn 指针注册表共存于宿主层和运行层之间；以下为职责分割。

| 注册表 | 位置（crate） | 字段数 | 注册方 | 消费方 |
|--------|-------------|-------|--------|--------|
| `RuntimeHooks` | `host-projection::hooks` | ~30 | `runtime-core::init_hooks()` | `host-projection` 各模块 |
| `RuntimeCoreHooks` | `framework-kernel::runtime_hooks` | ~10 | `runtime-core::init_hooks()` | `framework-extra`, `core-state` |

`RuntimeHooks` → 宿主侧代理函数（closeout/evidence/paper/research/skill-route/fetch-guard/tool-dispatch/浏览器/追踪）。
`RuntimeCoreHooks` → 运行层内核回调（host-provider-lookup/goal-drive/orchestrator/background-state/concurrency-defaults/eval-route）。

---

## 3. 路由层（5L:route）

意图→技能匹配、搜索、评分、路由决策、路由评估。

| Crate | 路径 | 声明的 L 编号 | 说明 |
|-------|------|-------------|------|
| `routing-engine` | `core/routing-engine` | L4 | **Skill 路由引擎**：16 步评分管道、`route_task()`、`search_skills()` |
| `routing-core` | `core/routing-core` | L0 | **路由共享原语**：trigram Jaccard、n-gram 余弦相似度、配置 hooks |
| `eval-route` | `core/eval-route` | L4 | **路由评估**：`eval_route_contract()`、`run_eval_route()` |

**配置文件归属**：
- `skills/SKILL_ROUTING_RUNTIME.json` → 技能路由热表（唯一真源）

**路由层依赖关系**：
```
routing-engine ──→ routing-core ──→ （无其他内部依赖）
eval-route ──→ framework-extra ──→ routing-engine
```

**路由层不依赖**：skill-layer（元数据解析在路由层外，通过路由层 hooks 注入宿主信息）。

---

## 4. Skill 层（5L:skill）

技能元数据定义、SKILL.md 解析、技能发现、健康清单。**不含技能执行**——执行在运行层。

| Crate | 路径 | 声明的 L 编号 | 说明 |
|-------|------|-------------|------|
| `skill-layer` | `core/skill-layer` | L2 | **Skill 层核心**：frontmatter 类型解析、skills/ 目录发现、验证、刷新 |

**目录归属**：
- `skills/*/SKILL.md` → 技能元数据（47 个技能）

> **设计约束**：Skill 层只做元数据管理。技能的执行（包括 agent 提示、工具调用、退出条件验证）由运行层通过 runtime-core hooks 实现。路由决策在路由层通过 `routing-engine` 实现。

---

## 5. 工具层（5L:tool）

MCP 工具注册表、工具路由、框架内建工具实现、独立 MCP 服务器二进制。

**区分**：**工具注册/路由**在 `core/` 内，**工具二进制实现**在 `rust_tools/` 和 `tools/`。

| Crate | 路径 | 声明的 L 编号 | 说明 |
|-------|------|-------------|------|
| `tool-routing-engine` | `core/tool-routing-engine` | L4 | **工具路由引擎**：8 步评分管道、`route_tool()` / `search_tools()` |
| `mcp-tool-registry` | `core/mcp-tool-registry` | L4 | **统一 MCP 工具注册表**：`McpToolRecord` 类型、JSON 加载、缓存 |

**工具二进制实现**（独立的 MCP 服务器）：

| 目录 | 工具 |
|------|------|
| `rust_tools/batch-common` | 批量处理共享库 |
| `rust_tools/citation_tool_rs` | 学术引用审核 |
| `rust_tools/financial_data_rs` | 金融市场数据 |
| `rust_tools/gh_source_gate_rs` | GitHub 源门控 |
| `rust_tools/mcp-stdio-common` | MCP stdio 共享通信库 |
| `rust_tools/ooxml_parser_rs` | Office Open XML 解析 |
| `rust_tools/pdf_tool_rs` | PDF 读取/提取 |
| `rust_tools/pptx_tool_rs` | PowerPoint 生成/分析 |
| `tools/browser-mcp` | 浏览器自动化 MCP 服务器 |

**框架内建工具**（在运行层实现，通过工具层注册暴露）：

| 工具 | 实现位置 | 分发域 |
|------|---------|--------|
| `goal_state_manage` | `runtime-core::framework_runtime::tool_handlers` | `domain:goal` |
| `closeout_record_write` | 同上 | `domain:closeout` |
| `closeout_gate` | 同上 | `domain:closeout` |
| `quality_gate_evaluate` | `runtime-core::qg_route` | `domain:quality-gate` |

**配置文件归属**：
- `configs/framework/MCP_TOOL_REGISTRY.json` → 工具注册表真源
- `configs/tool_scoring_weights.json` → 工具路由评分权重

---

## 6. 运行层（5L:runtime）

任务脚手架 + Loop Goal + QG Route + 基础设施。

按 **v10 3 概念**（Task、Goal、QG Route）分组：

### 6.1 Task 脚手架

| Crate | 路径 | 说明 |
|-------|------|------|
| `core-state` | `core/core-state` | 任务状态机：goal_drive、step_ledger、task_ledger、closeout_validation |
| `core-state-utils` | `core/core-state-utils` | IO/路径/JSONL 辅助函数 |

### 6.2 Loop Goal（含退出门）

| Crate | 路径 | 说明 |
|-------|------|------|
| `goal-engine` | `core/goal-engine` | Loop Goal 状态机：6 态（Dormant→Active→ReviewPending→Completed/Superseded/Aborted） |
| `quality-gate` | `core/quality-gate` | QG Route：GateChecker trait、CheckerRegistry、evaluate() |

### 6.3 BootManager

| Crate | 路径 | 说明 |
|-------|------|------|
| `runtime-core` | `core/runtime-core` | **平台聚合器**：`init_hooks()`、framework_runtime（stdio dispatch + tool handlers）、QG route 初始化 |

### 6.4 运行时基础设施

| Crate | 路径 | 说明 |
|-------|------|------|
| `framework-kernel` | `core/framework-kernel` | **框架内核**：RUNTIME_REGISTRY.json 加载、RuntimeCoreHooks、时间/JSON/CLI 工具、路由缓存 |
| `core-errors` | `core/core-errors` | 通用 FrameworkError 类型（零依赖） |
| `core-policy` | `core/core-policy` | Hook 策略、安全规则、环境标志、审查门 |
| `runtime-storage` | `core/runtime-storage` | 存储后端（文件系统 + SQLite + 内存） |
| `runtime-infra` | `core/runtime-infra` | 运行时基础设施：kernel_bootstrap、stdio_transport |
| `trace-runtime` | `core/trace-runtime` | 追踪记录：event record、replay |
| ~~`fr-utils`~~ | ~~`core/fr-utils`~~ | ~~IO 工具、常量、类型（无止义逻辑）~~ *(已合并到 runtime-core, 2026-07)* |
| ~~`fr-contracts`~~ | ~~`core/fr-contracts`~~ | ~~执行合约：PreExecutionGuard~~ *(已合并到 runtime-core, 2026-07)* |
| ~~`fr-exec`~~ | ~~`core/fr-exec`~~ | ~~执行引擎：实时执行、运行时视图、trace_attach~~ *(已合并到 runtime-core, 2026-07)* |
| `http-util` | `core/http-util` | HTTP 工具函数 |
| `codegraph-rs` | `core/codegraph-rs` | 代码知识图谱 |

### 6.5 运行时扩展

| Crate | 路径 | 说明 |
|-------|------|------|
| `framework-extra` | `core/framework-extra` | 编排控制：closeout/evidence/manifest_fallback/session_artifacts |
| `framework-maint` | `core/framework-maint` | 框架维护：内联快照、maint 命令 |
| `research-harness` | `core/research-harness` | 研究工具：review_loop、aigc_check、verification、claim_drift |

---

## 7. 完整归属总表

| 序号 | Crate | 5L 归属 | v9 L 编号 | 核心职责 |
|------|-------|---------|----------|---------|
| 1 | `host-projection` | 宿主层 | L5 | HostProvider trait + build.rs 代码生成 + 投影安装 |
| 2 | `session-supervisor` | 宿主层 | L6 | Worker 进程生命周期管理 |
| 3 | `router-rs` | 宿主层 | L7 | CLI 入口、hook/agent 分发 |
| 4 | `routing-engine` | 路由层 | L4 | Skill 路由引擎（评分/搜索/决策） |
| 5 | `routing-core` | 路由层 | L0 | trigram Jaccard、模糊匹配等路由原语 |
| 6 | `eval-route` | 路由层 | L4 | 路由合约评估 + 用例运行 |
| 7 | `skill-layer` | Skill 层 | L2 | SKILL.md 解析、技能发现、健康清单 |
| 8 | `tool-routing-engine` | 工具层 | L4 | Tool 路由引擎（评分/路由/搜索） |
| 9 | `mcp-tool-registry` | 工具层 | L4 | 统一 MCP 工具注册表（记录/JSON/缓存） |
| 10 | `core-state` | 运行层 | L4 | 任务状态机（GoalState/StepLedger/Closeout） |
| 11 | `core-state-utils` | 运行层 | L0 | 状态 I/O 辅助函数 |
| 12 | `goal-engine` | 运行层 | L6 | Loop Goal 状态机 |
| 13 | `quality-gate` | 运行层 | L4 | QG Route（Checker trait + 注册表） |
| 14 | `runtime-core` | 运行层 | L7 | 平台聚合器（init_hooks / framework_runtime） |
| 15 | `framework-kernel` | 运行层 | L0 | 框架内核（注册表加载 / RuntimeCoreHooks / CLI） |
| 16 | `core-errors` | 运行层 | L0 | FrameworkError 类型 |
| 17 | `core-policy` | 运行层 | L0 | Hook 策略 / 安全规则 / env_flags |
| 18 | `runtime-storage` | 运行层 | L1 | 存储后端：FS + SQLite + 内存 |
| 19 | `runtime-infra` | 运行层 | B0 | kernel_bootstrap / stdio_transport |
| 20 | `trace-runtime` | 运行层 | L1 | 追踪记录 + 压缩 |
| 21 | ~~`fr-utils`~~ | ~~运行层~~ | ~~L1~~ | ~~IO 工具 / 常量~~ *(已合并到 runtime-core, 2026-07)* |
| 22 | ~~`fr-contracts`~~ | ~~运行层~~ | ~~L2~~ | ~~执行合约（PreExecutionGuard）~~ *(已合并到 runtime-core, 2026-07)* |
| 23 | ~~`fr-exec`~~ | ~~运行层~~ | ~~L3~~ | ~~执行引擎 / trace_attach~~ *(已合并到 runtime-core, 2026-07)* |
| 24 | `framework-extra` | 运行层 | L6 | 编排控制（closeout / evidence） |
| 25 | `framework-maint` | 运行层 | L6 | 框架维护（snapshot / maint） |
| 26 | `research-harness` | 运行层 | L5 | 研究工具（review_loop / aigc_check） |
| 27 | `http-util` | 运行层 | — | HTTP 工具函数 |
| 31 | `codegraph-rs` | 运行层 | — | 代码知识图谱 |

---

## 8. 依赖方向验证

```
宿主层（5L:host）
  host-projection ──→ runtime-core（通过 RuntimeHooks fn 指针）──→ 运行层
  session-supervisor ──→ 启动 worker 进程
  router-rs ──→ runtime-core::init_hooks() ──→ 运行层

路由层（5L:route）
  routing-engine ──→ routing-core + skill-layer
  eval-route ──→ framework-extra ──→ routing-engine

Skill 层（5L:skill）
  skill-layer ──→ （无 core crate 依赖，纯类型解析）

工具层（5L:tool）
  tool-routing-engine ──→ routing-core + mcp-tool-registry
  mcp-tool-registry ──→ （无内部 core 依赖，JSON 加载）

运行层（5L:runtime）——被所有上层依赖，不反向依赖任何上层
```

**关键规则**：
1. 运行层不 reverse-depend 宿主层/路由层/Skill 层/工具层的**业务类型**
2. 所有回调由运行层通过 fn 指针（`RuntimeHooks` / `RuntimeCoreHooks`）注册
3. Cargo.toml 中的 feature flags 不可引入上层的编译时依赖
4. Rust trait（如 `HostProvider`）的定义在所属 crate，实现由运行层通过代码生成/手动注册注入

---

## 9. 更新记录

| 日期 | 修改者 | 变更 |
|------|--------|------|
| 2026-06-28 | — | 初始版本：5 层归属表 + L0-L7 对照 |
