
---
title: "v10 运行层重构路线图"
date: "2026-06-27"
author: "对抗审稿团队: Layer 1-4 × 文档 × 减法 × 实施 × 风险"
status: "approved-governed"
---

# v10 运行层重构路线图

> **原则**：减法 + 第一性原理。只管 3 个概念：Task、Loop Goal、QG Route。
> **产出**：~4000 行删除，~1650 行新增，net -2350。
> **耗时**：6 Waves × 34-49 天，每批独立合入 main。
> **文档状态**：每 Wave 完成后立即更新对应文档，最终 Wave 全量校验。

---

## 1. 设计意图（真源）

来自用户的原始设计陈述，以下为不可违反的约束。

### 1.1 核心信条

| # | 约束 | 说明 |
|---|------|------|
| D1 | 运行层 = 轻量 Task 脚手架 + Loop 式 Goal（内部含 QGEntry）+ QG Route 注册表 | 3 概念：Task、Goal、QG Route。QGEntry 是 GoalEngine 内部方法，非独立概念 |
| D2 | 退出门有两阶段 | GoalEngine.continue() 内部分两阶段：Stage 1 防欺诈门（每 task 证据核查）→ Stage 2 质量门（每轮 checker chain）|
| D3 | Task 每轮由主 agent 生成 | 运行层只做脚手架，不分解逻辑 |
| D4 | Goal 只有 loop 类型 | 没有 linear，只有退出条件 |
| D5 | 每轮都维护 task list（由 agent 生成） | 运行层只验证 transition 合法性，不生成 task。空 task list 时防欺诈门自动通过（无 task = 无欺诈可能）|
| D6 | 质量门由场景驱动 | Checker 由 scene 决定，运行层只定义接口 |
| D7 | 质量门不接受人工审批 | 全自动 checker chain |
| D8 | QG Route 可被独立调用 | Skill Route 涉及 review 时可直接 `evaluate(scene, ctx)`，不必须经过 Goal。但 Goal 内触发（QGEntry）是主路径 |
| D9 | Agent 决定是否下一轮 | agent 调用 GoalEngine.continue() 时内部自动触发 QGEntry，非运行层自动推进 |
| D10 | 不需要 session 概念 | 不保留 lifecycle 管理 |
| D11 | 减法优先 | 该删全删，不给未来留抽象 |
| D12 | 分批合入 main | 不搞一锅端 |

### 1.2 概念精简

```
v9 概念（7 个）           v10 概念（3 个）
─────────────────         ─────────────────
Task                     Task（轻量脚手架）
Session                  │
Lifecycle Profile        │
Goal（Linear + Loop）    Goal（只 loop，内部含两阶段退出门：防欺诈+质量）
Quality Gate (独立状态机)  │  └─ QG Route（插拔式 checker 注册表，可独立调用）
Closeout（R1-R8 独立）    │     └─ 防欺诈门（per-task 证据核查）
Loop Engine（7 阶段）      │     └─ 质量门（per-round checker）
```

v9 虚拟概念不参与运行层管理：Plan、Skill Route、设计活动等直接交付不经过 Goal 系统。

### 1.3 迁移兼容性原则（静默无感升级）

**硬约束**：每批合入 main 后，用户运行层不受影响。非功能性变更（如删除只写不读的代码）一次到位；功能性变更（活跃 MCP 工具、
数据持久格式、初始化路径）必须分阶段迁移。

| # | 原则 | 说明 |
|---|------|------|
| D13 | **删前留后路**：删除活跃读写路径前，先建立同等覆盖的新路径。确认新路径生效后方可删旧路径 | 不接受"先删再修" |
| D14 | **双写期**：持久格式变更至少有一轮完整的双写期（新旧格式同时写入），期间回退旧版可读 | 每批合入状态：旧格式仍被写入 + 新格式已写入 |
| D15 | **反序列化兼容**：枚举删除前先 deprecate + `#[serde(other)]`；字段删除前加 `#[serde(default)]` | 不能因 GOAL_STATE.json 有旧字段/变体而启动失败 |
| D16 | **MCP 工具 wrapper**：旧工具在其功能被替换前保持可用。替换后包装为新入口的内部调用 | 用户不能因 QG Route 合入而找不到 quality_gate 工具 |

**三阶段通用模式**（适用于每项功能性变更）：

```
Phase A（并行期）         Phase B（切换期）             Phase C（清理期）
────────────────────────  ───────────────────────────  ──────────────────
新旧共存 / 双写         新路径成为主路径             删旧路径
消费方任选路径         旧路径只读不写               更新文档断言
#[serde(default)] 保障  旧路径写入停止               ──

典型节奏：Phase A 和 Phase B 各跨越至少一批合入（不同 PR）。不安排在同一天。
```

**旧/OLD 标记规范**：

所有待删项（枚举变体、函数、类型、文件）在迁移期间必须加明确标记。**不要用 `#[allow(dead_code)]`**——`dead_code` 意味着"已无引用但忘了删"；
`OLD` 意味着"还有引用，但已安排退场"。

| 项类型 | 标记方式 | 示例 |
|--------|---------|------|
| 枚举变体 | `#[deprecated]` | `#[deprecated(note = "OLD: v10 将删, use Loop")] Linear` |
| 结构体/函数 | `#[deprecated]` | `#[deprecated(note = "OLD: v10 将删")]` |
| 宏/OnceLock | `// OLD:` 行首注释 | `// OLD: v10 删, use RuntimeHooks` |
| 结构体字段 | `#[deprecated]` 或 `// OLD:` | `// OLD: v10 删, scene 字段已替代` |
| 整个文件 | 文件第一行 `// OLD:` | `// OLD: v10 Wave X 将删除此文件` |

**每个 Wave 的"旧层退场"步骤**（见 §5）：

```
┌─────────────────────────────────────────┐
│ 1. 标记（标记 → deprecate 编译告警）       │
│ 2. 并行（新旧共存，双写/双注册/双路径）     │
│ 3. 退场（删除标记项 → 移除兼容代码）       │
└─────────────────────────────────────────┘
```

标记在 Wave 开始时立即做（触达开发者），退场在 Wave 最后一批合入时完成。

---

## 2. v10 架构

### 2.1 整体五层

```
┌──────────────────────────────────────────────────────────────────────┐
│  宿主层（Host Adapter）                                               │
│                                                                      │
│  host-projection：MCP stdio 桥、投影安装、文件状态锁、主机适配          │
│  依赖方向：宿主层 → {路由层, 运行层}（通过 L0 函数指针注册表通信）     │
│  对应 v9 层：L5 host-projection                                       │
├──────────────────────────────────────────────────────────────────────┤
│  路由层（Routing）                                                   │
│                                                                      │
│  routing-engine：16 步 skill 路由评分管道                             │
│  tool-routing-engine：8 步 tool 路由评分管道                          │
│  routing-core：共享原语（n-gram、trigram Jaccard）                    │
│  依赖方向：路由层 → {Skill层, 运行层}（路由依赖 scene 标记）          │
│  对应 v9 层：L4 routing-engine + tool-routing-engine                  │
├──────────────────────────────────────────────────────────────────────┤
│  Skill 层                                                             │
│                                                                      │
│  skill-layer：schema 校验、依赖管理、SKILL.md 元数据解析（含 scene）  │
│  依赖方向：Skill 层 → 运行层（通过 framework-kernel::runtime_hooks 注册，Wave 3b 替代旧 framework-runtime-hooks）     │
│  对应 v9 层：L4 skill-layer                                           │
├──────────────────────────────────────────────────────────────────────┤
│  工具层（MCP 接入层）                                                  │
│                                                                      │
│  mcp-tool-registry：统一 MCP 工具注册表（发现/路由/注册）              │
│  router-rs MCP tools：框架提供给 agent 的 MCP 工具实现                │
│    （framework_goal_xxx、framework_quality_gate_xxx 等）               │
│  依赖方向：工具层 → 运行层（MCP tool handler 通过运行层 API 操作任务） │
│  对应 v9 层：L5 mcp-tool-registry + L7 router-rs                     │
├──────────────────────────────────────────────────────────────────────┤
│  运行层（Runtime Layer）← v10 重构对象                                │
│                                                                      │
│  Task 脚手架 + Loop Goal（含退出门）+ QG Route（插拔式 checker 链）   │
│  BootManager：init/shutdown、1 个 RuntimeHooks struct                 │
│  runtime-core / core-state / loop-engine→goal-engine                  │
│  framework-extra（删后只留 evidence 写入）                            │
│  对应 v9 层：跨 L0-L7（v10 目标：收敛到 3 概念）                     │
└──────────────────────────────────────────────────────────────────────┘

层间通信规则：
  - 宿主层 → 路由层/Skill层/工具层 → 运行层：依赖方向单向向下（符合 v9 P3）
  - 运行层通过 L0 framework-kernel::runtime_hooks 函数指针注册表接收上层回调（替代已删除的 framework-runtime-hooks crate）
  - 禁止运行层反向依赖宿主层/路由层/Skill层/工具层

### 2.2 v10 运行层内部

v10 运行层只管理 **3 个概念**（Task、Loop Goal、QG Route）+ BootManager：

```
┌─────────────────────────────────────────────────────────────────────┐
│                       运行层（Runtime Layer）                         │
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │ BootManager（初始化 / 关闭）                                  │   │
│  │  • init()：注册 hooks（46→1 个 RuntimeHooks struct）          │   │
│  │  • shutdown()：优雅关闭                                      │   │
│  │  • 职责：初始化所有下级组件，不持有业务状态                    │   │
│  └────────────────────┬─────────────────────────────────────────┘   │
│                       │                                             │
│          ┌────────────┴────────────┐                                │
│          ▼                         ▼                                │
│  ┌──────────────────┐   ┌──────────────────────────┐               │
│  │  GoalEngine      │   │     TaskScaffold         │               │
│  │  (只 loop)       │   │  (轻量脚手架)             │               │
│  │                  │──▶│                          │               │
│  │  GoalState:      │读写│  • TASK_POINTERS.json   │               │
│  │  ┌──────────┐    │   │  • EVIDENCE_INDEX.json   │               │
│  │  │ Dormant  │    │   │  • validate_transition() │               │
│  │  └────┬─────┘    │   │  • write/read_evidence() │               │
│  │       ▼          │   └──────────────────────────┘               │
│  │  ┌──────────┐    │                                             │
│  │  │ Active   │    │                                             │
│  │  └────┬─────┘    │                                             │
│  │       ▼          │                                             │
│  │  ┌──────────┐    │                                             │
│  │  │ReviewPend│    │                                             │
│  │  └────┬─────┘    │                                             │
│  │       ▼          │                                             │
│  │  ┌──────────┐    │                                             │
│  │  │Completed │    │                                             │
│  │  ├──────────┤    │                                             │
│  │  │Superseded│    │                                             │
│  │  ├──────────┤    │                                             │
│  │  │ Aborted  │    │                                             │
│  │  └──────────┘    │                                             │
│  └────────┬─────────┘                                             │
│           │ trigger()（GoalEngine 内部自动调用，agent 不直接触发）    │
│           ▼                                                       │
│  ┌────────────────────────────────────────────────────────────┐   │
│  │               QGEntry（退出门）                              │   │
│  │                                                             │   │
│  │  Stage 1: 防欺诈门                                           │   │
│  │  ├─ 读 EVIDENCE_INDEX 检查完整性                             │   │
│  │  ├─ 跨链接口 evidence 行                                     │   │
│  │  └─ 不区分 scene，永远触发                                    │   │
│  │                                                             │   │
│  │  Stage 2: 质量门                                            │   │
│  │  ├─ dispatch(scene) → QG Route                              │   │
│  │  └─ 聚合 GateVerdict                                        │   │
│  └─────────────────┬───────────────────────────────────────────┘   │
│                    │ return GateVerdict（返回 GoalEngine 决定 Active/ReviewPending）│
│                    ▼                                               │
│  ┌────────────────────────────────────────────────────────────┐   │
│  │               QG Route                                      │   │
│  │  ├─ GateChecker trait                                       │   │
│  │  ├─ CheckerRegistry (scene→[checker])                       │   │
│  │  ├─ evaluate(scene, ctx) → GateVerdict                      │   │
│  │  └─ registry 在 init() 时由 register_*_checkers() 填充      │   │
│  └────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

**运行层内部 DAG：**

```
GoalEngine ──read/write──→ TaskScaffold（读写 task 和 evidence）
GoalEngine ──trigger()──→ QGEntry（退出门触发）
QGEntry ──read_evidence()──→ TaskScaffold（防欺诈门读证据）
QGEntry ──evaluate()──→ QG Route（质量门 dispatch）
QGEntry ──return GateVerdict──→ GoalEngine（状态更新：Active 继续 / ReviewPending 等待）
BootManager ──init()──→ {GoalEngine, TaskScaffold, QG Route}

禁止的依赖：
  TaskScaffold → GoalEngine（违反，脚手架不知道 goal 存在）
  QG Route → GoalEngine（违反，路由层不持有运行状态）
  GoalEngine 直接调用 QG Route（违反，必须通过 QGEntry 统一入口）
```

**v9 → v10 运行层映射：**

| v9 概念 | v9 位置 | v10 归宿 |
|---------|---------|---------|
| Task（TASK_POINTERS.json） | core-state L4 | TaskScaffold |
| Goal（GOAL_STATE.json） | core-state L4 | GoalEngine（只 loop）|
| Quality Gate（独立状态机） | runtime-exit-gate L5 | → QG Route（运行层内部）|
| Closeout（R1-R8） | fr-contracts L2 | → 防欺诈门（QGEntry Stage 1）|
| Session | framework-extra L6 | 删除（D10）|
| LifecycleProfile | core-policy L0 | 删除 |
| Loop Engine（7 阶段） | loop-engine L6 | → goal-engine（精简）|

### 2.3 Scene 模型

#### 字段规范

```yaml
# SKILL.md frontmatter
---
scene: research          # 必需（Wave 1 起），默认 "general"
sub_scene: ~            # 可选（Wave 6 启用），当前场景的精细分类
---
```

scene 取值只有 5 个常量值（见 scene 常量表）。sub_scene 的取值规范在 Wave 6 中定义。

#### Scene 驱动流程

```
Skill Route 匹配 skill ──→ 读取 SKILL.md metadata.scene
                              │
                              ▼
                        GoalEngine 存储 scene（每个 goal 只定一次）
                              │
                              ▼
                        QGEntry 触发时继承 scene
                              │
                              ▼
                        QG Route 的 evaluate(scene, ctx) —— 不重新分类
```

#### Scene → Checker 链

```
general       → [AdversarialChecker]（通用兜底）
research      → [LogicAndEvidence, Novelty, Math, Figures, Language, Length,
               FullRegression, Literature, ProseQC, Statistical, Reproducibility,
               Structure]（~12 个，sub_scene 可修剪）
code_review   → [Correctness, Security, ABICompat, Dependencies, Observability,
               FirstPrinciples]（~6 个）
slides        → [Overflow, Font, QA, VisualLayout]（4 个）
visual        → [ScreenshotLayout, Accessibility, ChartReadability]（3 个）
```

### 2.4 关键类型定义

```rust
// === QG Route 核心类型 ===

/// 严重度沿用 research-harness 规范
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

pub struct CheckContext {
    pub scene: String,
    pub sub_scene: Option<String>,       // Wave 6 启用
    pub goal: String,
    pub round: u64,
    pub repo_root: PathBuf,
    pub task_id: String,
    pub evidence_path: Option<PathBuf>,
    pub runtime_handle: Option<tokio::runtime::Handle>, // async checker 用 block_in_place
}

pub trait GateChecker: Send + Sync {
    fn id(&self) -> &'static str;
    fn scenes(&self) -> Vec<&'static str>;
    fn description(&self) -> &'static str;
    fn check(&self, ctx: &CheckContext) -> CheckResult;
}

pub struct CheckerRegistry {
    checkers: HashMap<&'static str, Vec<Box<dyn GateChecker>>>,
}

impl CheckerRegistry {
    pub fn register(&mut self, scene: &'static str, checker: Box<dyn GateChecker>);
    pub fn evaluate(&self, scene: &str, ctx: &CheckContext) -> GateVerdict;
}

// === scene 常量表 ===
pub mod scene {
    pub const GENERAL: &str = "general";
    pub const RESEARCH: &str = "research";
    pub const CODE_REVIEW: &str = "code_review";
    pub const SLIDES: &str = "slides";
    pub const VISUAL: &str = "visual";
}

// === GoalEngine 状态机 ===

pub enum GoalState {
    Dormant,
    Active,
    ReviewPending,
    Completed,
    Superseded,
    Paused,
    Aborted,
}

// === Task 状态机（轻量脚手架） ===
pub enum TaskState {
    Created,
    Assigned,
    InProgress,
    Completed,
    Failed,
    Retry,
}

// === EVIDENCE_INDEX.json 结构（TaskScaffold 管理） ===
// 位置: {repo_root}/EVIDENCE_INDEX.json
// 写入时机: agent 通过 MCP tool 记录每条 evidence
struct EvidenceIndex {
    task_id: String,
    evidence: Vec<EvidenceEntry>,
}
struct EvidenceEntry {
    id: String,              // ev-001
    type_: String,           // "file" | "link" | "diff" | "inline"
    path: Option<String>,    // 文件路径
    content_hash: Option<String>,  // SHA-256
    cross_refs: Vec<String>,        // 跨链引用: ["TASK_POINTERS.json#task_001"]
    created_at: String,      // ISO 8601
}

// === QGEntry 退出门 ===
impl QGEntry {
    pub fn trigger(
        goal: &GoalEngine,
        scaffold: &TaskScaffold,
        route: &QG_Route,
    ) -> GateVerdict {
        // Stage 1: 防欺诈门 — 无 scene，始终执行
        let evidence_ok = verify_evidence_chain(scaffold);
        if !evidence_ok {
            return GateVerdict { passed: false, reason: "防欺诈: evidence 不完整" };
        }
        // Stage 2: 质量门 — dispatch 到 QG Route
        route.evaluate(goal.scene(), &CheckContext { ... })
    }
}

// === 防欺诈门核心函数 ===
// 位置：runtime-core 或 core-state
fn verify_evidence_chain(scaffold: &TaskScaffold) -> bool {
    // 遍历 EVIDENCE_INDEX 中当前 goal 的所有 task
    // 每个 task 必须至少有一条 evidence 条目且有有效 cross_ref
    // 若存在 task 无 evidence → 返回 false（证据链断裂）
    // 若 task list 为空 → 返回 true（无 task = 无欺诈可能）
}
```

### 2.5 聚合规则

| 条件 | 结果 |
|------|------|
| 任何 finding severity = P0 | **门失败**（无条件）|
| 任何 finding severity = A 或 B | **门失败** |
| 全部 findings ≤ Warning（或空）| **门通过** |
| 防欺诈门失败 | **门失败**，不进入质量门 |

---

## 4. crate 命运表（最终）

| 当前 crate | Wave | 命运 | 说明 |
|---|---|---|---|
| `framework-runtime-hooks` | 3b | 合并进 runtime-core | 13 个 fn 指针字段归入 1 struct |
| `host-projection` | 3a | 保留重构 | 46 once_lock_hook! → 1 RuntimeHooks struct |
| `runtime-core` | 3b | 保留增强 | 主入口，吸收 hooks 合并 |
| `runtime-infra` | 2d | 保留增强 | 删 telemetry_emit，吸收进程管理 ~490 行 |
| `fr-exec` | 2d | 精简 | 删 continuity/telemetry_observer/research_mode |
| `framework-extra` | 2c/3c | 只留 evidence | 删 session_artifacts(5文件)/closeout/session_call；EVIDENCE_INDEX 拆为独立函数 |
| `core-state` | 2a/3c | 精简 | 删 GoalType::Linear、删 QG_ops 互斥、删 TASK_STATE 投影 |
| `loop-engine` | 3 | **→ goal-engine** | 迁出 ~490 行进程管理到 runtime-infra 后重命名 |
| `runtime-exit-gate` | 4a-ii | **删** | QG 状态机（flow/close_gates/evidence）→ GoalState::ReviewPending + QG Route |
| `core-policy` | 2a | 精简 | 删 lifecycle_profile + 删研究模式启发式 |
| `core-state-utils` | keep | — | 文件 IO 原子写原语底层实现，被运行层 FileIo 工具函数依赖 |
| `core-state-types` | 2a | 精简 | 删 GoalType::Linear、删 QG 互斥类型 |
| `fr-contracts` | 3c-i | 精简 | closeout_enforcement 合入 validate_transition (per-task) 前置；保留 pre_tool_use_guard + execution_contract |
| `session-supervisor` | 待定 | 未来决策 | — |
| `core/quality-gate` | 1/4a | 新建 | trait + types + CheckerRegistry + evaluate() — 单一 crate，不拆 quality-gate-routing |

---

## 5. Execution Waves

### Wave 1: 定义与标注（3-5 天）

**目标**：零行为变更。新增接口和元数据字段。

| 任务 | 详情 |
|---|---|
| 创建 `core/quality-gate/` | GateChecker trait、CheckContext、GateVerdict、CheckerRegistry、Severity、Finding、aggregate |
| 全量 SKILL.md 加 scene | ~47 文件，默认 `general`。不做 sub_scene。 |
| 路由数据模型加 scene | `skill-layer::frontmatter_parser` 提取 scene；路由结果增加 `scene: String` |
| 预热文档 | `architecture.md` 页首 v10 预告；`/docs/ROADMAP-v10.md` 初版（本文档）|

**验证**：`cargo test` 全过；scene 缺失或无效时降级 `"general"`；旧路由结果无 scene 字段时 $scene = None → generalize。

### Wave 2: 减法（4 天，完全串行）

**目标**：删已确定的冗余，不做重构。

#### Wave 2a（1 天）

| 删除内容 | 涉及文件 |
|---|---|
| `GoalType::Linear` | `core-state-types/task_state_types.rs`, 枚举变体 + 所有 match 分支 |
| `LifecycleProfile` | `core-policy/hook_common/goal_signals.rs:, 相关函数 `lifecycle_profile_is_loop_capable`|
| `research_mode` 枚举 + 推断 | `research-harness/research_mode.rs`（全删）；`host-projection/hooks.rs` 的 `INFER_RESEARCH_MODE` 指针；`stdio_payload_types.rs` 的字段；`live_execute.rs` 参数；`architecture.md §5` |
| `TaskControlMode` 四态机 | `core-state/types.rs` 定义；所有消费点如果只有 GoalDrive/QG 互斥则直接删 |

**静默兼容策略（Wave 2a）：**

| 删除项 | Phase A（并行） | Phase B（切换） | Phase C（清理） |
|--------|----------------|----------------|-----------------|
| `GoalType::Linear` | deprecate 变体 + `#[serde(other)]` 兜底为 Loop | 读入 Linear→Loop；确认无 GOAL_STATE.json 含 Linear | 删变体 |
| `LifecycleProfile` | deprecate 类型 + `#[serde(default)]` 兜底 | stop-loop-check 存活确定 | 删类型 |
| `research_mode` | 字段加 `#[serde(default)]`；删推断逻辑 | 删 payload 字段；删函数指针 | 删枚举定义 |
| `TaskControlMode` | deprecate 变体 | 删消费点 | 删定义 |

**删除次序验证**：每步删除前运行 `cargo test` + 手动确认 GOAL_STATE.json 无旧格式残留。
`research_mode` 删除需按栈展开：函数指针→payload→参数→推断逻辑→枚举定义。每步可编译验证。

**前置检查**：`cargo tree -p serde` 确认 serde ≥ 1.0.200（`#[serde(other)]` 需要）。若版本不足则改用 `#[serde(untagged)]` + match default 方案。

**旧层退场（Wave 2a）：**

| 标记项 | 标记方式 | 退场时机 |
|--------|---------|---------|
| `GoalType::Linear` 变体 | `#[deprecated(note = "OLD: v10 删, use Loop")]` | Phase C：确认无 GOAL_STATE.json 含 Linear → 删变体 |
| `LifecycleProfile` 结构体 | `#[deprecated(note = "OLD: v10 删")]` | Phase C：stop-loop-check 存活确认 → 删类型 |
| `research_mode` 枚举 | `#[deprecated]` + 函数指针处 `// OLD:` | Phase A 标记 → Phase B 删字段 → Phase C 删枚举定义 |
| `TaskControlMode` 枚举 | `#[deprecated(note = "OLD: v10 删")]` | Phase C：确认无消费者 → 删定义 |

#### Wave 2b（1 天）

| 删除内容 | 涉及文件 |
|---|---|
| 旧指针文件回退读代码 | `core-state/pointer_ops.rs` 的 `read_active_task_id`/`read_focus_task_id` |
| `TASK_STATE.json` 读写 | `task_state_aggregate.rs`（全删），`task_state.rs` 的 `resolve_task_view_with_pointers` |

**前提**：Wave 2c 必须先清理写入路径（`write_focused_repo_mirrors` 写 `active_task.json` 等）。

**静默兼容策略：**

| 步 | 操作 | 兼容性验证 |
|----|------|-----------|
| 2b-i | 删 `read_active_task_id`/`read_focus_task_id` 中 fallback 到 `active_task.json` 的分支 | 确认 TASK_POINTERS.json 已覆盖所有活跃会话（2c 保障）|
| 2b-ii | 删 `TASK_STATE.json` 读写 | 确认 TASK_POINTERS.json + EVIDENCE_INDEX.json 已覆盖所有字段 |
| 2b-iii | 删 `task_state_aggregate.rs` | `cargo test` + 手动运行验证无 task 视图丢失 |

**不提供旧格式回退写**：TASK_POINTERS.json 是主格式，`active_task.json` 等为冗余镜像。2c 清理写入后，2b 清理读取——之间无用户可见差异。

**旧层退场（Wave 2b）：**

| 标记项 | 标记方式 | 退场时机 |
|--------|---------|---------|
| `read_active_task_id` fallback 分支 | `// OLD: v10 删, TASK_POINTERS 为主` | Phase B：确认 TASK_POINTERS 全覆盖 → 删 |
| `read_focus_task_id` fallback 分支 | `// OLD: v10 删, TASK_POINTERS 为主` | 同上，一并删 |
| `resolve_task_view_with_pointers` 旧路径 | `// OLD: v10 删, TASK_POINTERS 替代 TASK_STATE` | Phase B：确认字段覆盖 → 删 |
| `task_state_aggregate.rs` 全文件 | 文件首行 `// OLD: v10 删` | Phase B：最后删文件 |

#### Wave 2c（1 天）

**最重要的一步**：`session_artifacts.rs` 中 EVIDENCE_INDEX 和 SESSION_SUMMARY 的写入在同一个函数中。必须先拆分。

| 步骤 | 操作 |
|---|---|
| 2c-i | 将 `write_session_artifact_set()` 和 `build_evidence_index_payload()` 从 `session_artifacts.rs` 提取到 `core-state` 或 `runtime-core` 的独立函数 |
| 2c-ii | 删 `write_framework_session_artifacts()` → 只保留 EVIDENCE_INDEX + TASK_POINTERS 的写入口 |
| 2c-iii | 删 `write_focused_repo_mirrors()`（不再写 `active_task.json`/`focus_task.json`/`task_registry.json`）|
| 2c-iv | 删 `write_optional_session_mirror()`、`write_repo_session_focus()`、`write_supervisor_state_*` |
| 2c-v | 删 `SESSION_SUMMARY.md`、`NEXT_ACTIONS.json`、`TRACE_METADATA.json`、`SESSION_CALL_TRACKER.json`、`.supervisor_state.json` 的写入 |
| 2c-vi | 确认 `TASK_POINTERS.json` + `EVIDENCE_INDEX.json` 正常保留 |
| 2c-vii | **兼容扫描**：grep 确认无代码直接读 `active_task.json`/`focus_task.json`/`task_registry.json`（应全走 `pointer_ops::read_*` 中转）|

**旧层退场（Wave 2c）：**

| 标记项 | 标记方式 | 退场时机 |
|--------|---------|---------|
| `write_focused_repo_mirrors()` | `// OLD: v10 删, TASK_POINTERS 替代` | 2c-iii 直接删 |
| `write_optional_session_mirror()` | `// OLD: v10 删` | 2c-iv 直接删 |
| `write_repo_session_focus()` | `// OLD: v10 删` | 2c-iv 直接删 |
| `write_supervisor_state_*()` | `// OLD: v10 删` | 2c-iv 直接删 |
| `SESSION_SUMMARY.md` 写入 | `// OLD: v10 删` | 2c-v 直接删 |
| `NEXT_ACTIONS.json` 写入 | `// OLD: v10 删` | 2c-v 直接删 |
| `TRACE_METADATA.json` / `SESSION_CALL_TRACKER.json` / `.supervisor_state.json` | `// OLD: v10 删` | 2c-v 直接删 |

#### Wave 2d（1 天）

| 删除内容 | 涉及文件 |
|---|---|
| Telemetry 全部代码 | `runtime-infra/telemetry_emit.rs`（删）；`fr-exec/telemetry_observer.rs`（删）；`kernel_bootstrap.rs` 的 `bootstrap_telemetry` 调用 |
| `RuntimeContinuityClassifier` | `fr-exec/runtime_view.rs` 的 `classify_continuity` 函数（删）|

**用户明确：接受 telemetry/log 丢失**。不需要迁移协议、不需要 fallback。

**静默删除次序**（确保编译不中断）：
1. 先删 `kernel_bootstrap.rs` 中 `bootstrap_telemetry()` 调用点（函数变为 dead code → 编译告警）
2. 再删 `bootstrap_telemetry()` 函数体
3. 最后删 `telemetry_emit.rs` / `telemetry_observer.rs` / `classify_continuity()` 文件

**旧层退场（Wave 2d）：**

| 标记项 | 标记方式 | 退场时机 |
|--------|---------|---------|
| `telemetry_emit.rs` | 文件首行 `// OLD: v10 删` | 直接删除 |
| `telemetry_observer.rs` | 文件首行 `// OLD: v10 删` | 直接删除 |
| `bootstrap_telemetry()` | `// OLD: v10 删, telemetry 整体移除` | 直接删除调用 |
| `RuntimeContinuityClassifier` | `#[deprecated(note = "OLD: v10 删")]` | 直接删除 |
| `classify_continuity()` | `#[deprecated(note = "OLD: v10 删")]` | 直接删除 |

### Wave 3: 核心重构（10-14 天）

**目标**：状态合并 + 框架整合。高风险，跨 25+ crate。

#### Wave 3a（7-10 天）

`host-projection` 的 46 个 `once_lock_hook!` 插槽合并为 1 个 `RuntimeHooks` 结构体。

**静默迁移策略（三阶段）：**

```
Phase A（并行期 ─ 首批合入前准备）
  1. 定义 RuntimeHooks struct（字段类型 = 原 fn 指针签名）
  2. init 路径做双注册：既设 struct 字段，also 设旧 OnceLock
     → 旧消费方仍走 OnceLock，新消费方走 struct
  3. 编译验证：cargo test 全过，无行为变化

Phase B（切换期 ─ 两批合入）
  Batch 1（~20 个 tool 类）：
    - tool_xxx 消费方从 OnceLock 改为读 RuntimeHooks 字段
    - 旧 OnceLock 注册仍然保留（双注册保障，未迁移的 26 个不中断）
    - 验证：所有 tool 类 MCP 工具行为不变
  Batch 2（~26 个生命周期类）：
    - init_xxx/shutdown_xxx + 函数指针消费方迁移至 RuntimeHooks
    - 旧 OnceLock 注册仍然保留（read side 已全迁）
    - 验证：启动/关闭/函数指针调用行为不变

Phase C（清理期 ─ 两批合入后）
  - 删 once_lock_hook! 宏定义
  - 删 init 路径中的 OnceLock 双注册代码
  - 删 runtime-hooks crate 的 OnceLock 相关函数
```

**关键约束**：Phase B 期间始终有双注册（新旧并存），不出现"消费方已迁但注册路径断裂"的窗口。
每批合入 CI 全绿前，手动跑一次端到端 MCP 工具调用验证。

**旧层退场（Wave 3a）：**

| 标记项 | 标记方式 | 退场时机 |
|--------|---------|---------|
| `once_lock_hook!` 宏定义 | `// OLD: v10 删, use RuntimeHooks struct` | Phase C：删宏定义 |
| 各 `once_lock_hook!` 调用点（46 处） | `// OLD: v10 删, use RuntimeHooks.{field}` | Phase B：消费方迁移后删注册代码 |
| 双注册 shim 代码 | `// OLD: v10 删, 过渡期兼容` | Phase C：删所有双注册逻辑 |

#### Wave 3b（2-3 天）

**前提：Wave 3a Phase C 已完成（`once_lock_hook!` 已清理，RuntimeHooks struct 已稳定）。**

**影响范围**：3 个 init 守卫被以下 crate 的初始化代码调用：
- `ROUTING_HOOKS_INIT` → `router-rs` 和 `routing-engine`
- `TOOL_ROUTING_CONFIG_HOOKS_INIT` → `tool-routing-engine`
- `HOST_PROJECTION_HOOKS_INIT` → `host-projection`

合并为 `RUNTIME_INIT` 后需更新 ~10+ 个文件的 `use` 和调用点。

| 任务 | 详情 |
|---|---|
| `framework-runtime-hooks` 合并进 `runtime-core` | 将 struct 定义 + regist 函数搬到 runtime-core。原 crate 打 `// OLD:` 后删除 |
| 3 init 守卫→1 | `ROUTING_HOOKS_INIT` / `TOOL_ROUTING_CONFIG_HOOKS_INIT` / `HOST_PROJECTION_HOOKS_INIT` 合并为 `RUNTIME_INIT`。更新所有调用点引用 |

**旧层退场（Wave 3b）：**

| 标记项 | 标记方式 | 退场时机 |
|--------|---------|---------|
| `framework-runtime-hooks` crate | 文件首行 `// OLD: v10 删, merged into runtime-core` | 合并后删 crate |
| `ROUTING_HOOKS_INIT` | `#[deprecated(note = "OLD: v10 删, use RUNTIME_INIT")]` | 合并为 RUNTIME_INIT 后删 |
| `TOOL_ROUTING_CONFIG_HOOKS_INIT` | 同上 | 同上 |
| `HOST_PROJECTION_HOOKS_INIT` | 同上 | 同上 |

#### Wave 3c-i（1 天）— closeout 独立系统删除

| 步骤 | 操作 |
|---|---|
| 3c-i-a | `fr-contracts/closeout_enforcement/` 的 R1-R8 规则提取为 `validate_transition()`（TaskState → Completed 时触发，per-task）的防欺诈前置检查。**并行期**：新防欺诈门 + 旧 closeout 同时运行，验证结果一致后切走 |
| 3c-i-a-match | **并行验证器**：增加 `compare_old_closeout_vs_new_fraud_gate(task_id)` 函数，逐 task 比对两者结果。不一致时 emit warning（不阻断）。此函数仅在迁移期存在，3c-i-c 中随旧 closeout 一起删除 |
| 3c-i-b | `framework-extra/closeout.rs` 的 `closeout_record_path_for_task` 写入逻辑迁入 `validate_transition()`。**双写期**：旧 closeout_record + 新 evidence 同时写 |
| 3c-i-c | 删 `closeout_enforcement` 模块（代码迁入 validate_transition 后原 crate 只保留 pre_tool_use_guard + execution_contract）|
| 3c-i-d | 更新 `fr-contracts/Cargo.toml` 移除 closeout 相关 feature |

**旧层退场（Wave 3c-i）：**

| 标记项 | 标记方式 | 退场时机 |
|--------|---------|---------|
| `closeout_enforcement` 模块 | `// OLD: v10 删, 防欺诈门替代` | 3c-i-c：并行验证一致后删模块 |
| `closeout_record_path_for_task()` | `#[deprecated(note = "OLD: v10 删, validate_transition 内置")]` | 3c-i-b：双写期后删旧写路径 |
| 并行期验证断言 | `// OLD: v10 删, 过渡期双路径验证` | 3c-i-c：删并行校验代码 |

### Wave 4: QG Route + Checker（5-7 天）

#### Wave 4a（2-3 天）

| 任务 | 详情 |
|---|---|
| 创建 `core/quality-gate-routing/` | evaluate() 入口 + CheckerRegistry 消费端 |
| 集成到 runtime-core | QGEntry.trigger() 调用 QG Route evaluate() |
| 启动时注册 | `runtime_core::init()` 增加 `register_checkers(&mut registry, ...)` |

#### Wave 4a-ii（1 天）— QG 状态机删除

> **必须在 Wave 4a 之后执行**。QG Route 就绪后才能删旧的 MCP tool。

| 步骤 | 操作 |
|---|---|
| (准备) | QG Route（Wave 4a）提供 `evaluate()` 入口后，旧 `framework_quality_gate` MCP tool handler 内部改为调用 QG Route——用户无感知 |
| `flow.rs` | `framework_quality_gate` MCP tool → 替换为 QG Route 调用（wrapper 模式） |
| `close_gates.rs` | `RfvCloseGates` → `GoalReviewGates`（附加到 GoalState::ReviewPending 状态）|
| `evidence.rs` | `cross_link_evidence` → 防欺诈门内部实现 |
| `runtime-exit-gate` crate | 全删（不再有独立 crate）|
| QG ↔ Goal 互斥逻辑 | `quality_gate_ops.rs` 的 `deactivate_goal_for_conflict_with_quality_gate` 消失 → QG 只是 Goal 的内部模式，不需要互斥 |

**旧层退场（Wave 4a-ii）：**

| 标记项 | 标记方式 | 退场时机 |
|--------|---------|---------|
| `flow.rs`（旧 QG MCP tool handler） | 文件首行 `// OLD: v10 删, QG Route wrapper` | wrapper 稳定后删文件 |
| `close_gates.rs` | 文件首行 `// OLD: v10 删, GoalState::ReviewPending` | 替换为 GoalReviewGates 后删 |
| `evidence.rs`（旧 `cross_link_evidence`）| `// OLD: v10 删, 防欺诈门内部实现` | 迁入后删 |
| `runtime-exit-gate` crate | `// OLD: v10 删, 合入 runtime-core` | 最后删整个 crate |
| `deactivate_goal_for_conflict_with_quality_gate()` | `#[deprecated(note = "OLD: v10 删, QG 非独立状态机")]` | 删函数 |

#### Wave 4b（3-4 天）

**in-place 原则**：Checker 不搬迁物理文件。在每个现有的 review/verification 模块上加 `impl GateChecker`。

| Checker | 源位置 | 模式 |
|---|---|---|
| LogicAndEvidence | research-harness/review/dimensions.rs | 适配器 |
| LiteracyChecker | research-harness/verification/literature.rs | 适配器（异步需 runtime_handle）|
| ProseQCChecker | research-harness/verification/prose_qc.rs | 直接 impl（纯函数）|
| StatisticalChecker | research-harness/verification/statistical.rs | 直接 impl |
| CorrectnessChecker | `core/quality-gate/checkers/` 或 code-review-deep skill 的 checker adapter（新建） | 适配器（包装 code-review-deep 的审查逻辑）|
| AdversarialChecker | runtime-core/checkers/ | 新编写 |
| ... | ... | ... |

**异步 checker 的处理**：`CheckContext` 增加 `runtime_handle: Option<tokio::runtime::Handle>`，checker 内部用 `block_in_place` 处理异步调用。

### Wave 5: 路由整合 + Skill 解耦 + 文档治理（9-13 天）

#### Wave 5a（3-5 天）— 路由整合

**端到端链路设计**：

```
agent 调用 MCP tool
    │
    ├─ framework_goal_drive(scene=research)
    │      → GoalEngine.start_or_continue()  → Dormant→Active
    │
    ├─ framework_task_create() → TaskScaffold.write(TASK_POINTERS.json)
    ├─ framework_task_complete()
    │      → TaskScaffold.validate_transition(InProgress→Completed)
    │           └─ 内部触发：verify_evidence_chain(scaffold)  # Stage 1 防欺诈
    │      → GoalEngine.continue()
    │           └─ 内部触发：QGEntry.trigger()
    │                └─ Stage 2: QGRoute.evaluate(scene, ctx)
    │                     └─ return GateVerdict
    │                          ├─ passed=true  → GoalEngine state=Active (continue loop)
    │                          └─ passed=false → GoalEngine state=ReviewPending (wait agent)
    │                                               agent 读取 blockers[] → 修复 → continue
    │
    ├─ framework_goal_state_manage(complete) 
    │      → GoalEngine Active→Completed (需先过 QGEntry)
    │
    └─ Skill Route (独立路径，不经过 Goal)
           → QGRoute.evaluate(scene, ctx)  // 直接调用，用于 review skill 二次路由
```

**MCP tool 变更矩阵**：

| MCP Tool | Wave | 变更 | 说明 |
|----------|------|------|------|
| `framework_goal_drive` | 2a, 5a | goal_type 只接受 Loop；内部集成 QGEntry 触发 | Linear 类型废弃 |
| `framework_goal_state_manage` | 5a | 新增 ReviewPending 状态处理 | agent 可以看到 blockers 后决策 |
| `framework_quality_gate start` | 4a-ii | **删除** | wrapper → QGRoute.evaluate() |
| `framework_quality_gate close_gates` | 4a-ii | **删除** | → GoalReviewGates |
| `framework_closeout` | 3c-i | **删除** | → validate_transition() 内部触发 |
| `framework_task_create` | — | 不变 | 仍由 agent 直接调用 |
| `framework_task_complete` | 3c-i | 内部增加防欺诈检查 | 透明，agent 无感知 |
| `framework_task_focus` | — | 不变 | — |

**Smoke test 检查清单**（Wave 4 完成后、Wave 5a 正式整合前执行）：
1. GoalEngine Active→ReviewPending→Active 完整循环
2. QGEntry 返回 passed=false 时 agent 正确接收到 blockers
3. validate_transition() 防欺诈门正确拦截无 evidence 的 task
4. Skill Route 独立调用 QGRoute.evaluate() 返回正确 scene→checker 匹配
5. 空 task list 边界：防欺诈门返回 passed=true

#### Wave 5b（3-5 天）— Skill 解耦

6 个 verification skill（`prose-verification`、`literature-verification`、`statistical-verification`、`reproducibility-verification`、`structure-verification`、`formal-verification`）从独立路由条目改为 QG Checker 的 CLI alias。

#### Wave 5c（3-5 天）— 文档治理

| 文档 | 操作 | 触发时机 |
|---|---|---|
| `architecture.md` | 8 层 L0-L7 → v10 五层架构（宿主/路由/Skill/工具/运行层）+ 运行层内部 | Wave 5c |
| `AGENTS.md` | 删 lifecycle/closeout/TaskControlMode/QG 相关 | Wave 2 完成后 |
| `/docs/quality-gate.md` | 新写：trait/registry/checker 注册 | Wave 4 完成后 |
| `Cargo.toml` description | 全量 crate description 审核 | Wave 3 完成后 |
| `research-harness/README.md` | 修复虚假 MCP tool 列表 | Wave 4b 完成后 |
| `core-errors::FrameworkError` | 审核变体，删过时项 | Wave 3 完成后 |

### Wave 6: sub_scene 专项治理（后续轮次）

> 此 Wave 不在 34-49 天路线图内，可独立推进。

- YAML schema：`scene: research` + `sub_scene: literature_review | paper_audit | claim_verification`
- Checker 在 `CheckContext.sub_scene` 中读取，动态修剪 checker chain
- sub_scene 默认值：`scene` 等于 `"research"` 时默认 `"full"`（跑全部 12 个 checker）

---

## 6. 关键设计决定日志

| # | 决定 | 时间 | 理由 |
|---|------|------|------|
| D001 | QGEntry 是 GoalEngine 内部方法（非独立状态机），QG Route 是独立注册表 | 2026-06 | QG 不需要自己的生命周期；Goal Loop 内部两阶段退出门；QG Route 可被 Skill Route 独立调用 |
| D002 | 防欺诈门是 QGEntry 内置通用预检查，非 QG Checker | 2026-06 | 证据完整性是运行层基本契约，不区分 scene |
| D003 | closeout R1-R8 规则作为防欺诈门（per-task evidence 检查）与 closeout 记录写入合并进 per-task validate_transition，不在 Goal.complete 触发 | 2026-06 | 用户设计意图：每 task 核查证据，不只在退出时 |
| D004 | `GateChecker::check()` 同步签名，异步通过 `runtime_handle` + `block_in_place` 支持 | 2026-06 | 保持 QG Route 同步性，避免异步传播到 GoalEngine |
| D005 | `CheckResult` 无 severity 字段，severity 只属于 `Finding` | 2026-06 | 单个 checker 可能产出 P0 + C 混搭结果 |
| D006 | `CheckContext` 无 `previous_results` | 2026-06 | 保持纯函数式无状态承诺 |
| D007 | Checker 实现 in-place，不搬迁文件 | 2026-06 | 避免引入不必要的构建依赖（reqwest/sha2/rusqlite 等）|
| D008 | `CheckResult.passed` 由 checker 自判，`GateVerdict.passed` 由 aggregate 判断 | 2026-06 | 分离判断职责：checker 说自己有没有发现问题，gate 说是否阻塞退出 |
| D009 | Loop Engine → goal-engine 重命名 | 2026-06 | 迁出进程管理后只剩纯 Goal Engine 领域逻辑 |
| D010 | telemetry 直接删除，零迁移 | 2026-06 | 用户接受丢失 |
| D011 | research_mode → scene 字段替代 | 2026-06 | scene 字段覆盖了原先的 feature 层+研究模式双重概念 |
| D012 | Batch 2a/2b/2c/2d 完全串行（原文 2b/2c 可并行错误）| 2026-06 | 文件写入路径缠绕，必须 2c 先拆写入→2b 再删读代码 |
| D013 | Batch 3c 必须拆为两阶段 | 2026-06 | QG 状态机是活跃 MCP tool，QG Route 就绪前不能删 |
| D014 | Batch 3a 时间修正 4-5 天 → 7-10 天 | 2026-06 | 35 个宏插槽 × 10 个调用文件 |
| D015 | sub_scene 延迟到 Wave 6 | 2026-06 | 不影响核心减法，可在架构稳定后独立推进 |
| D016 | 文档治理每 Wave 触发，非一次性 | 2026-06 | 减少最终阶段负担，保证中间过程可读 |
| D017 | **Wave 2a 删除枚举/字段前先 `#[serde(other)]`/`#[serde(default)]` 兜底** | 2026-06 | 静默无感：旧 GOAL_STATE.json 含 Linear/LifecycleProfile 时启动不失败 |
| D018 | **Wave 3a 保持双注册期贯穿两批合入** | 2026-06 | 静默无感：旧 OnceLock + 新 RuntimeHooks 同时注册，消费方分批迁移 |
| D019 | **Wave 3c-i 防欺诈门 + 旧 closeout 并行运行验证结果一致后才切走** | 2026-06 | 静默无感：closeout 是活跃执行路径，不能无覆盖替换 |
| D020 | **Wave 4a-ii 旧 QG MCP tool 内部 wrapper 调用 QG Route** | 2026-06 | 静默无感：MCP tool 接口不变，用户不察觉后端替换 |
| D021 | **`RFV_LOOP_STATE.json` 等旧格式通过读时兼容转换** | 2026-06 | 静默无感：不要求用户手动迁移旧状态文件 |
| D022 | **Wave 2b/2c 之间不安排用户可见变更合入** | 2026-06 | 静默无感：写路径移除（2c）与读路径移除（2b）之间 window 为 0 行为变化 |

---

## 7. 被否决的方案

| 方案 | 被否决理由 |
|---|---|
| QG 作为 Goal 的并行状态机 | 用户设计意图明确不需要，QG 只是 Goal 的内部 review_mode |
| 防欺诈门作为 QG Checker（注册到 general scene）| 审计矛盾：防欺诈门验证自身写入。必须是 QGEntry 预处理 |
| Checker 实现搬迁到 `core/quality-gate/checkers/` | 引入不必要的构建依赖，且 Wave 3 跨 25+ crate 的重构风险已高 |
| Closeout 作为 Goal.complete 前置 | 用户明确说防欺诈门是每 task 证据核查，不是退出时一次性检查 |
| Telemetry 保留为可选 feature | 用户接受丢失，直接删除更简单 |
| lifecycle 管理 | 用户明确不需要，BootManager 只做 init/shutdown |
| 额外多层抽象（facade/中介者） | 减法原则：不解耦、不抽象、不多层 |
| sub_scene 在 Wave 1 加入 | 增加当轮复杂度；可等架构稳定后独立治理 |

---

## 8. 风险登记表

| 风险 | 等级 | 影响 Wave | 缓解措施 |
|---|---|---|---|
| 旧文件未迁移即删读代码 | 高 | 2b, 2c | 先 Wave 2c 清写入 → 再 Wave 2b 删读取。磁盘扫描迁移补丁。 |
| 3a 两批合入间双注册一致性 | 中 | 3a | Phase B 保持长双注册（旧 OnceLock + 新 struct），不出现消费方已迁但注册路径断裂的窗口 |
| 测试套件零提及 | 高 | 全 Wave | 每个 Wave 追加测试清理子任务。 |
| `check()` 同步签名与异步 API 冲突 | 高 | 4b | CheckContext 加 `runtime_handle`；async checker 使用 `block_in_place`。 |
| QG 状态机删除后旧 `RFV_LOOP_STATE.json` 僵尸 | 中 | 4a-ii | 读时兼容：读取旧文件转为 `GOAL_STATE.json` 的 `ReviewPending` 状态写回。 |
| **删除活跃 MCP tool 导致用户流程中断** | **高** | 4a-ii | Wave 4a 先提供 QG Route，4a-ii 用 wrapper 模式透明替换。禁止同一天合入 4a + 4a-ii。 |
| **`#[serde(other)]` 兜底后遗留旧数据** | **低** | 2a | Phase C 清理后旧格式不再被读取。补充文档：清理后可手动删旧 GOAL_STATE.json 中 `linear` 条目。 |
| **`#[serde(other)]` 需 serde ≥ 1.0.200** | **中** | 2a | 编译前检查 Cargo.lock 中 serde 版本。若低于 1.0.200，改用 `#[serde(untagged)]` + match default 分支方案。 |
| **Wave 3a/3b 交叉依赖：RuntimeHooks struct 未稳定时 3b 就搬 crate** | **中** | 3a, 3b | 3b 添加硬前置：3a Phase C 完成后 + `once_lock_hook!` 宏已删除后，才能开始 3b。 |
| **Wave 5a 端到端集成时发现架构缺陷需要回溯 Wave 3/4 修改** | **高** | 5a | Wave 4 完成后立即做 smoke test（manual 端到端 flow）。在 Wave 5a 正式整合前确认 GoalEngine→QGEntry→QG Route 链路可工作。 |
| **agent 直接调用 QGEntry 破坏了 D001（QGEntry 是 GoalEngine 内部方法）** | **中** | 4a-ii, 5a | 明确 QGEntry.trigger() 仅在 GoalEngine 内部调用。如 agent 需触发 quality gate，走 MCP tool → GoalEngine.continue() 间接触发。 |

---

## 9. 执行总览

```
Wave 1  3-5 天   定义 + 标注
  │
Wave 2  4 天     减法（2a→2b→2c→2d 串行）
  │
Wave 3  10-14 天  核心重构（3a→3b→3c-i→等待 4a→4a-ii）
  │           ↘
Wave 4  5-7 天    QG Route + Checker（4a→4a-ii→4b）
  │
Wave 5  9-13 天   路由整合 + 文档治理（5a→5b→5c）
  │
Wave 6  后续轮次   sub_scene 专项治理
```

**总预估**: 34-49 天（已从原 30-40 天修正）

**每 Wave 合入验证条件**:
- `cargo test --no-run` 编译通过
- `cargo test` 全过
- 受影响 crate 的单元测试更新
- 对应文档部分更新

**文档触发规则**:
- Wave 1 完成 → 写 `quality-gate.md`（trait 部分）
- Wave 2 完成 → 更新 `AGENTS.md`（删旧概念引用）
- Wave 3 完成 → 更新 crate `Cargo.toml` description
- Wave 4 完成 → 更 `quality-gate.md`（checker 注册部分）
- Wave 5 完成 → 全量校验 `architecture.md` / `AGENTS.md` / README
