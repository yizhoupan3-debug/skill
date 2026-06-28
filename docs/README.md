---
last_verified: "2026-06-29"
scope: architecture-reference
---

# 框架架构参考

> 本文件是框架的**架构真源**，合并了 LAYER-MAP（crate→层映射）与 v10 运行时设计意图。
> 更新者每次添加/移动/重命名 crate 时须更新 §1.2 归属表。

---

## 1. 五层架构总览

框架采用五层语义模型，依赖方向单向向下：

```
宿主层（5L:host）        ──→  路由层（5L:route）        ──→  Skill 层（5L:skill）
                              ──→  工具层（5L:tool）
                              ──→  运行层（5L:runtime）← v10 重构对象
```

**依赖规则**：
1. 宿主层 → {路由层, 运行层}：通过 L0 函数指针注册表通信
2. 路由层 → {Skill层, 运行层}：路由依赖 scene 标记
3. Skill 层 → 运行层：通过 `framework-kernel::runtime_hooks` 注册
4. 工具层 → 运行层：MCP tool handler 通过运行层 API 操作任务
5. 运行层**不**反向依赖任何上层

### 1.1 层定义

| 层 | 5L 标识 | 核心职责 | 对应 v9 L 编号 |
|-----|---------|---------|--------------|
| **宿主层** | 5L:host | 宿主抽象、`HostProvider` 注册表、session/worker 生命周期、CLI 入口 | L5-L7 |
| **路由层** | 5L:route | 意图→技能匹配、搜索、评分、路由决策、路由评估 | L0 + L4 |
| **Skill 层** | 5L:skill | 技能元数据定义、SKILL.md 解析、技能发现、健康清单 | L2 |
| **工具层** | 5L:tool | MCP 工具注册表、工具路由、框架内建工具实现 | L4 + 独立二进制 |
| **运行层** | 5L:runtime | **3 概念**：Task 脚手架 + Loop Goal + QG Route | L0-L7 |

### 1.2 Crate 归属总表

> 所有 `core/*/` 目录以 2026-06-29 实际状态校验。

#### 宿主层

| Crate | 路径 | 层标 | 说明 |
|-------|------|------|------|
| `host-projection` | `core/host-projection` | L5 | **宿主层核心接口**：`HostProvider` trait 族 + 4 宿主机 build.rs 代码生成 + 投影安装 + MCP stdio 桥 |
| `session-supervisor` | `core/session-supervisor` | L6 | Worker 生命周期管理：启动/检查/终止 worker 进程 |
| `router-rs` | `core/router-rs` | L7 | CLI 入口：hook 分发入口 + agent 分发入口 + 框架自检命令 |

**配置文件归属**：
- `configs/framework/RUNTIME_REGISTRY.json` → 宿主元数据
- `.mcp.json` → 宿主侧 MCP server 注册表

#### 路由层

| Crate | 路径 | 层标 | 说明 |
|-------|------|------|------|
| `routing-engine` | `core/routing-engine` | L4 | **Skill 路由引擎**：16 步评分管道、`route_task()`、`search_skills()` |
| `routing-core` | `core/routing-core` | L0 | **路由共享原语**：trigram Jaccard、n-gram 余弦相似度、配置 hooks |
| `eval-route` | `core/eval-route` | L4 | 路由评估：`eval_route_contract()`、`run_eval_route()` |

**配置文件**：`skills/SKILL_ROUTING_RUNTIME.json`（技能路由热表，唯一真源）

#### Skill 层

| Crate | 路径 | 层标 | 说明 |
|-------|------|------|------|
| `skill-layer` | `core/skill-layer` | L2 | **Skill 层核心**：frontmatter 类型解析、skills/ 发现、SKILL_HEALTH_MANIFEST.json 生成 |

**设计约束**：Skill 层只做元数据管理。执行（包括 agent 提示、工具调用、退出条件）由运行层通过 runtime-core hooks 实现。

#### 工具层

| Crate | 路径 | 层标 | 说明 |
|-------|------|------|------|
| `tool-routing-engine` | `core/tool-routing-engine` | L4 | 工具路由引擎：8 步评分管道、`route_tool()`、`search_tools()` |
| `mcp-tool-registry` | `core/mcp-tool-registry` | L4 | 统一 MCP 工具注册表：`McpToolRecord` 类型、JSON 加载、缓存 |
| `browser-mcp-dispatch` | `core/browser-mcp-dispatch` | L0 | 浏览器 MCP 分派辅助 |

**独立 MCP 服务器二进制**（在 `rust_tools/` 和 `tools/` 中实现）。

**配置文件**：
- `configs/framework/MCP_TOOL_REGISTRY.json` → 工具注册表真源
- `configs/tool_scoring_weights.json` → 工具路由评分权重

#### 运行层

按 v10 3 概念分组。

**6.1 Task 脚手架**

| Crate | 路径 | 说明 |
|-------|------|------|
| `core-state` | `core/core-state` | 任务状态机：goal_drive、step_ledger、task_ledger、closeout_validation |
| `core-state-types` | `core/core-state-types` | 纯类型定义（零依赖，仅 serde） |
| `core-state-utils` | `core/core-state-utils` | IO/路径/JSONL 辅助函数 |

**6.2 Loop Goal（含退出门）**

| Crate | 路径 | 说明 |
|-------|------|------|
| `goal-engine` | `core/goal-engine` | Loop Goal 状态机：6 态（Dormant→Active→ReviewPending→Completed/Superseded/Aborted） |
| `quality-gate` | `core/quality-gate` | QG Route：`GateChecker` trait、`CheckerRegistry`、`evaluate()` |

**6.3 BootManager**

| Crate | 路径 | 说明 |
|-------|------|------|
| `runtime-core` | `core/runtime-core` | **平台聚合器**：`init_hooks()`、framework_runtime（stdio dispatch + tool handlers）、QG route 初始化 |

**6.4 运行时基础设施**

| Crate | 路径 | 层标 | 说明 |
|-------|------|------|------|
| `framework-kernel` | `core/framework-kernel` | L0 | 框架内核：RUNTIME_REGISTRY.json 加载、RuntimeCoreHooks、路由缓存 |
| `core-errors` | `core/core-errors` | L0 | 通用 FrameworkError 类型（零依赖） |
| `core-policy` | `core/core-policy` | L0 | Hook 策略、安全规则、环境标志、审查门 |
| `runtime-storage` | `core/runtime-storage` | L1 | 存储后端（文件系统 + SQLite + 内存） |
| `runtime-core-contracts` | `core/runtime-core-contracts` | L2 | 运行时契约：hook_event_routing、mcp_pre_guard、web_fetch_guard |
| `runtime-infra` | `core/runtime-infra` | B0 | kernel_bootstrap、stdio_transport |
| `trace-runtime` | `core/trace-runtime` | L1 | 追踪记录：event record、compact、compress |
| `fr-utils` | `core/fr-utils` | L1 | IO 工具、常量、类型（无止义逻辑） |
| `fr-contracts` | `core/fr-contracts` | L2 | 执行合约：PreExecutionGuard |
| `fr-exec` | `core/fr-exec` | L3 | 执行引擎：实时执行、运行时视图、trace_attach |
| `http-util` | `core/http-util` | — | HTTP 工具函数 |
| `codegraph-rs` | `core/codegraph-rs` | — | 代码知识图谱（MCP 8 工具） |

**6.5 运行时扩展**

| Crate | 路径 | 说明 |
|-------|------|------|
| `framework-extra` | `core/framework-extra` | 编排控制：closeout、evidence、manifest_fallback |
| `framework-maint` | `core/framework-maint` | 框架维护：内联快照、maint 命令 |
| `research-harness` | `core/research-harness` | 研究工具：review_loop、aigc_check、verification、claim_drift |

---

## 2. v10 运行层：3 概念

v10 运行层只管理 **3 个概念** + BootManager：

```
┌─────────────────────────────────────────────────────────────┐
│                       运行层                                   │
│                                                              │
│  ┌──────────────────────┐   ┌──────────────────────────┐    │
│  │     GoalEngine       │   │     TaskScaffold          │    │
│  │  (只 loop, 6 态)     │──▶│  (轻量脚手架)              │    │
│  │                      │读写│  • TASK_POINTERS.json    │    │
│  │  状态: Dormant→Active│   │  • EVIDENCE_INDEX.json    │    │
│  │  →ReviewPending→...  │   │  • validate_transition()  │    │
│  └──────────┬───────────┘   └──────────────────────────┘    │
│             │ trigger()                                     │
│             ▼                                                │
│  ┌──────────────────────────────────────────────────┐      │
│  │  QGEntry（退出门）                                  │      │
│  │  Stage 1: 防欺诈门（证据链验证，无 scene）           │      │
│  │  Stage 2: 质量门（dispatch(scene) → QG Route）     │      │
│  └──────────┬───────────────────────────────────────┘      │
│             ▼                                                │
│  ┌──────────────────────────────────────────────────┐      │
│  │  QG Route（可插拔 checker 注册表）                   │      │
│  │  GateChecker trait → CheckerRegistry              │      │
│  │  evaluate(scene, ctx) → GateVerdict               │      │
│  └──────────────────────────────────────────────────┘      │
└─────────────────────────────────────────────────────────────┘
```

**禁止的依赖**：
- TaskScaffold → GoalEngine（脚手架不知道 goal 存在）
- QG Route → GoalEngine（路由层不持有运行状态）
- GoalEngine 直接调用 QG Route（必须通过 QGEntry 统一入口）

**`runtime-core` 角色**：QGEntry 和 QGRoute 的集成宿主，提供 `qg_entry::trigger()`（双阶段退出门入口）和 `qg_route::evaluate_qg_route()`（checker 链调度）。

### 2.1 概念对照

| v9 概念 | v10 归宿 |
|---------|---------|
| Task（TASK_POINTERS.json） | TaskScaffold |
| Goal（GOAL_STATE.json） | GoalEngine（只 loop）|
| Quality Gate（独立状态机） | → QG Route（运行层内部）|
| Closeout（R1-R8） | → 防欺诈门（QGEntry Stage 1）|
| Session | 删除 |
| LifecycleProfile | 删除 |
| Loop Engine（7 阶段） | → goal-engine 精简 |

### 2.2 Scene 模型

```yaml
# SKILL.md frontmatter
---
scene: research          # 必需，默认 "general"
sub_scene: ~            # 可选，当前场景的精细分类
---
```

scene 取值 5 个常量：

| scene | 对应 Checker 链 | 
|-------|----------------|
| `general` | [AdversarialChecker]（通用兜底） |
| `research` | [LogicAndEvidence, Novelty, Math, Literature, ProseQC, Statistical, Reproducibility, Structure] |
| `code_review` | [Correctness, Security, ABICompat, Dependencies, Observability] |
| `slides` | [Overflow, Font, QA, VisualLayout] |
| `visual` | [ScreenshotLayout, Accessibility, ChartReadability] |

**Scene 驱动流程**：

```
Skill Route 匹配 skill → 读取 SKILL.md metadata.scene
    → GoalEngine 存储 scene（每个 goal 只定一次）
    → QGEntry 触发时继承 scene
    → QG Route evaluate(scene, ctx) —— 不重新分类
```

### 2.3 QG Route 核心类型

```rust
pub enum Severity { P0, A, B, Warning, C }

pub struct Finding {
    pub id: String,
    pub severity: Severity,
    pub description: String,
    pub location: Option<String>,
    pub suggestion: Option<String>,
}

pub struct CheckResult {
    pub checker_id: String,
    pub passed: bool,
    pub findings: Vec<Finding>,
}

pub struct GateVerdict {
    pub passed: bool,
    pub checkers_ran: usize,
    pub blockers: Vec<Finding>,
    pub advisories: Vec<Finding>,
    pub reason: Option<String>,
}

pub trait GateChecker: Send + Sync {
    fn id(&self) -> &'static str;
    fn scenes(&self) -> Vec<&'static str>;
    fn description(&self) -> &'static str;
    fn check(&self, ctx: &CheckContext) -> CheckResult;
}
```

**聚合规则**：
- 任何 finding severity = P0/A/B → **门失败**
- 全部 findings ≤ Warning（或空）→ **门通过**
- 防欺诈门失败 → **门失败**，不进入质量门

---

## 3. 文档索引

| 文档 | 路径 | 说明 |
|------|------|------|
| **路由引擎架构** | [routing/architecture.md](routing/architecture.md) | 16 步 skill 评分管道、8 步 tool 评分管道、Owner 选择、Fuzzy 救援 |
| **Quality Gate 系统** | [quality-gate.md](quality-gate.md) | GateChecker Trait、CheckerRegistry、evaluate() 详解 |
| **数学验证工具链** | [math-reasoning-harness.md](math-reasoning-harness.md) | 三层分离架构、SymPy/Z3/Lean 后端、工具清单 |
| **运维手册** | [operations/index.md](operations/index.md) | 安装/升级、模块操作、状态管理、排障 |
| **Research Harness** | [../core/research-harness/README.md](../core/research-harness/README.md) | 科研验证 Harness：搜索、声明管理、AIGC 检测、Verification |
| **跨宿主代理策略** | [../AGENTS.md](../AGENTS.md) | 生命周期、语言、CodeGraph、行为差异 |
| **仓库快速入门** | [../README.md](../README.md) | 能力概览 |
| **Skill 框架协议** | [../skills/SKILL_FRAMEWORK_PROTOCOLS.md](../skills/SKILL_FRAMEWORK_PROTOCOLS.md) | 共享最小协议层（运行时、停止规则、自审计） |
| **Skill 分层路由** | [../skills/SKILL_ROUTING_LAYERS.md](../skills/SKILL_ROUTING_LAYERS.md) | 分层路由详解（L0-L4 各层边界、Special Gates、重路由信号） |
