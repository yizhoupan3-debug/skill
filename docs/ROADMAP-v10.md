
---
title: "v10 运行层重构路线图"
date: "2026-06-27"
author: "对抗审稿团队: Layer 1-4 × 文档 × 减法 × 实施 × 风险"
status: "approved-governed"
---

# v10 运行层重构路线图（完成记录 ✅）

> **原则**：减法 + 第一性原理。只管 3 个概念：Task、Loop Goal、QG Route。
> **产出**：~4000 行删除，~1650 行新增，net -2350。
> **耗时**：~28 天（2026-06 至 2026-06-27），6 Waves 全部完成。
> **文档状态**：✅ 全部 6 个 Wave 已合入 main。本文档从执行计划转为架构参考和复盘记录。

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

## 4. crate 命运表（实际结果 ✅）

| 当前 crate | Wave | 命运 | 实际结果 |
|---|---|---|---|
| `framework-runtime-hooks` | 3b | 合并进 runtime-core | **保持独立**（Wave 3b 评估结论：合并造成 `runtime-core→host-projection→runtime-core` 循环依赖，不合并）|
| `host-projection` | 3a | 保留重构 | ✅ 完成（46 once_lock_hook! → 1 RuntimeHooks struct；hooks.rs 1800→785 行）|
| `runtime-core` | 3b | 保留增强 | ✅ 完成（3 init 守卫 → 1 RUNTIME_INIT，吸收 hooks 重构）|
| `runtime-infra` | 2d | 保留增强 | ✅ 完成（删 telemetry_emit，吸收进程管理 ~490 行）|
| `fr-exec` | 2d | 精简 | ✅ 完成（删 continuity/telemetry_observer/research_mode）|
| `framework-extra` | 2c/3c | 只留 evidence | ✅ 完成（删 session_artifacts/session_call；closeout 保留写入；EVIDENCE_INDEX 独立）|
| `core-state` | 2a/3c | 精简 | ✅ 完成（删 GoalType::Linear/QC 互斥/TASK_STATE 投影；新增 closeout_validation.rs）|
| `loop-engine` | 3 | **→ goal-engine** | ✅ 完成（2026-06-27 重命名，迁出进程管理到 runtime-infra）|
| `runtime-exit-gate` | 4a-ii | **删** | ✅ 完成（crate 已删除，QG 状态机合入 GoalEngine 内部）|
| `core-policy` | 2a | 精简 | ✅ 完成（删 lifecycle_profile + research_mode 启发式）|
| `core-state-utils` | keep | 保留 | ✅ 未变更，文件 IO 原子写原语保持稳定 |
| `core-state-types` | 2a | 精简 | ✅ 完成（删 GoalType::Linear、删 QG 互斥类型）|
| `fr-contracts` | 3c-i | 精简 | ✅ 完成（closeout_enforcement 模块删除 ~1.2K 行，保留 pre_tool_use_guard + execution_contract）|
| `agent-orchestrator` | 3c-i++ | 重命名+集成 | ✅ 完成（session-supervisor → agent-orchestrator 重命名 + 14 MCP 工具 + skill 衔接）|
| `core/quality-gate` | 1/4a | 新建 | ✅ 完成（trait + types + CheckerRegistry + evaluate + 5 个 scene 常量 + 6 个 Checker 适配器）|

---

## 5. Execution Waves（完成记录 ✅）

> **全部 6 个 Wave 已于 2026-06-27 前完成。** 以下每个 Wave 的原始计划保留作为架构设计参考，当前状态标记为 ✅ 完成。

### Wave 1: 定义与标注（✅ 已完成）

**目标**：零行为变更。新增接口和元数据字段。实际耗时：~3 天。

| 任务 | 状态 | 详情 |
|---|---|---|
| 创建 `core/quality-gate/` | ✅ | GateChecker trait、CheckContext、GateVerdict、CheckerRegistry、Severity、Finding、aggregate |
| 全量 SKILL.md 加 scene | ✅ | ~47 文件，默认 `general` |
| 路由数据模型加 scene | ✅ | `skill-layer::frontmatter_parser` 提取 scene；路由结果增加 `scene: String` |
| 预热文档 | ✅ | `architecture.md` v10 预告；`/docs/ROADMAP-v10.md` 初版 |

### Wave 2: 减法（✅ 已完成）

**目标**：删已确定的冗余，不做重构。实际耗时：~4 天串行，全部完成。

#### Wave 2a（✅ 已完成）

| 删除内容 | 实际结果 |
|---|---|
| `GoalType::Linear` | ✅ 已删（枚举变体 + 所有 match 分支）|
| `LifecycleProfile` | ✅ 已删（类型 + 相关函数）|
| `research_mode` 枚举 + 推断 | ✅ 已删（栈展开：函数指针→payload→参数→推断逻辑→枚举定义）|
| `TaskControlMode` 四态机 | ✅ 已删（定义 + 消费点）|

#### Wave 2b（✅ 已完成）

| 删除内容 | 实际结果 |
|---|---|
| 旧指针文件回退读代码 | ✅ `read_active_task_id`/`read_focus_task_id` fallback 分支已删 |
| `TASK_STATE.json` 读写 | ✅ `task_state_aggregate.rs` 已删，`resolve_task_view_with_pointers` 已清理 |

#### Wave 2c（✅ 已完成）

| 步骤 | 实际结果 |
|---|---|
| EVIDENCE_INDEX/SESSION_SUMMARY 写入拆分 | ✅ `write_session_artifact_set()` 提取为独立函数 |
| 删冗余会话文件 | ✅ 停止写入 `active_task.json`/`focus_task.json`/`task_registry.json`/`SESSION_SUMMARY.md`/`NEXT_ACTIONS.json`/`TRACE_METADATA.json`/`SESSION_CALL_TRACKER.json`/`.supervisor_state.json` |
| 兼容扫描 | ✅ grep 确认无代码直接读上述旧文件 |

#### Wave 2d（✅ 已完成）

| 删除内容 | 实际结果 |
|---|---|
| Telemetry 全部代码 | ✅ `runtime-infra/telemetry_emit.rs`、`fr-exec/telemetry_observer.rs`、`bootstrap_telemetry` 调用已删 |
| `RuntimeContinuityClassifier` | ✅ `classify_continuity` 函数已删 |

> **注意**：telemetry 直接删除，零迁移（用户接受丢失）。
### Wave 3: 核心重构（✅ 已完成）

**目标**：状态合并 + 框架整合。实际耗时：~10 天，跨 25+ crate。

### Wave 3: 核心重构（✅ 已完成）

**目标**：状态合并 + 框架整合。实际耗时：~10 天，跨 25+ crate。

#### Wave 3a（✅ 已完成）— host-projection hooks 合并

| 任务 | 结果 |
|---|---|
| 46 个 `once_lock_hook!` 插槽 → 1 个 `RuntimeHooks` 结构体 | ✅ 完成（三阶段迁移：双注册 → 消费方迁移 → 清理）|
| `once_lock_hook!` 宏删除 | ✅ grep 确认零残留 |
| hooks.rs 缩减 | ✅ 1800→785 行（56% 缩减）|

#### Wave 3b（✅ 已完成 — 保持独立）— framework-runtime-hooks 评估

| 任务 | 结果 |
|---|---|
| 合并进 runtime-core 评估 | 🔶 **保持独立**。结论：合并造成 `runtime-core→host-projection→runtime-core` 循环依赖 |
| 3 init 守卫→1 | `ROUTING_HOOKS_INIT`/`TOOL_ROUTING_CONFIG_HOOKS_INIT`/`HOST_PROJECTION_HOOKS_INIT` → `RUNTIME_INIT` |

#### Wave 3c-i（✅ 已完成）— closeout 独立系统删除

| 步骤 | 结果 |
|---|---|
| R1-R8 → `validate_transition()` + `closeout_validation.rs` | ✅ `core/core-state/src/closeout_validation.rs` 364 行 |
| 并行验证器 build + verify | ✅ `compare_old_closeout_vs_new_fraud_gate()` 在过渡期比对两个结果 |
| 删 `closeout_enforcement` 模块 | ✅ 6 文件 ~1.2K 行已删 |
| 双写期 + 旧 closeout_record 删除 | ✅ framework-extra/closeout.rs 清理 |
| `loop-engine` → `goal-engine` 重命名 | ✅ 目录/workspace/代码/文档全量更新 |

### Wave 4: QG Route + Checker（✅ 已完成）

**目标**：质量门路由 + 6 个 Checker。实际耗时：~5 天。

#### Wave 4a（✅ 已完成）— QG Route 基础设施

| 任务 | 结果 |
|---|---|
| `core/quality-gate/` crate | ✅ GateChecker trait + CheckerRegistry + 5 scene 常量 + types |
| QGEntry.trigger() → QG Route evaluate() 集成 | ✅ runtime-core/src/qg_entry.rs + qg_route.rs |
| 启动时注册 checkers | ✅ `runtime_core::init()` 含 `register_checkers()` |

#### Wave 4a-ii（✅ 已完成）— QG 状态机删除

| 步骤 | 结果 |
|---|---|
| `framework_quality_gate` MCP tool → QG Route wrapper | ✅ wrapper 模式透明替换 |
| `runtime-exit-gate` crate | ✅ 全删 |
| QG ↔ Goal 互斥逻辑 | ✅ `deactivate_goal_for_conflict_with_quality_gate` 已删 |
| `RfvCloseGates` → `GoalReviewGates` | ✅ 附加到 GoalState::ReviewPending |

#### Wave 4b（✅ 已完成）— 6 个 QG Checker

| Checker | 源位置 | 结果 |
|---|---|---|
| LogicAndEvidence | research-harness/review/dimensions.rs | ✅ 适配器 |
| LiteracyChecker | research-harness/verification/literature.rs | ✅ 适配器 |
| ProseQCChecker | research-harness/verification/prose_qc.rs | ✅ 直接 impl |
| StatisticalChecker | research-harness/verification/statistical.rs | ✅ 直接 impl |
| CorrectnessChecker | runtime-core/checkers/ | ✅ 适配器 |
| AdversarialChecker | runtime-core/checkers/ | ✅ 新建 |

### Wave 5: 路由整合 + Skill 解耦 + 文档治理（✅ 已完成）

**目标**：QGEntry→GoalEngine 全链路集成 + 6 skill alias + 文档治理。实际耗时：~8 天。

#### Wave 5a（✅ 已完成）— 路由整合

端到端链路已实现（见 §2.2 DAG 图）：
- GoalEngine → QGEntry.trigger() → QGRoute.evaluate(scene, ctx) → GateVerdict
- task_complete 触发防欺诈门作为 blocking gate
- goal_drive 中防欺诈门为 parallel Phase A informational comparison
- framework_goal_state_manage(complete) 需先过 QGEntry

#### Wave 5b（✅ 已完成）— Skill 解耦

6 个 verification skill（`prose-verification`、`literature-verification`、`statistical-verification`、`reproducibility-verification`、`structure-verification`、`formal-verification`）→ QG Checker 的 CLI alias。

#### Wave 5c（✅ 已完成）— 文档治理

| 文档 | 操作 | 状态 |
|---|---|---|
| `architecture.md` | 8 层 L0-L7 → v10 五层架构 | ✅ |
| `AGENTS.md` | 删 lifecycle/closeout/TaskControlMode/QG 相关 | ✅ |
| `/docs/quality-gate.md` | 新写：trait/registry/checker 注册 | ✅ |
| `research-harness/README.md` | 修复虚假 MCP tool 列表 | ✅ |
| `core-errors::FrameworkError` | 审核变体，删过时项 | ✅ |

### Wave 6: sub_scene 专项治理（✅ 已完成）

> 此前标为"后续轮次"，实际已在 v10 cleanup 中完成。

- YAML schema：`scene: research` + `sub_scene: literature_review | paper_audit | claim_verification`
- Checker 在 `CheckContext.sub_scene` 读取，动态修剪 checker chain

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

## 8. 风险登记表（实际结果 ✅）

| 风险 | 等级 | 影响 Wave | 实际结果 |
|---|---|---|---|
| 旧文件未迁移即删读代码 | 高 | 2b, 2c | ✅ 已规避（2c 先清写入 → 2b 后删读取）|
| 3a 两批合入间双注册一致性 | 中 | 3a | ✅ 已规避（长双注册期贯穿两批合入）|
| 测试套件零提及 | 高 | 全 Wave | ⚠️ 部分补充，仍有提升空间（~35% 核心函数无测试）|
| `check()` 同步签名与异步 API 冲突 | 高 | 4b | ✅ 已解决（CheckContext.runtime_handle + block_in_place）|
| QG 状态机删除后旧 `RFV_LOOP_STATE.json` 僵尸 | 中 | 4a-ii | ✅ 读时兼容转换 |
| **删除活跃 MCP tool 导致用户流程中断** | **高** | 4a-ii | ✅ 已规避（wrapper 模式透明替换）|
| **`#[serde(other)]` 兜底后遗留旧数据** | **低** | 2a | ✅ 无事故 |
| **`#[serde(other)]` 需 serde ≥ 1.0.200** | **中** | 2a | ✅ serde 版本满足 |
| **Wave 3a/3b 交叉依赖** | **中** | 3a, 3b | ✅ 已规避（3b 添加硬前置）|
| **Wave 5a 端到端集成发现架构缺陷需回溯** | **高** | 5a | ✅ 未发生回溯 |
| **agent 直接调用 QGEntry 破坏 D001** | **中** | 4a-ii, 5a | ✅ 未发生违法调用 |

---

## 9. 执行总览（实际完成 ✅）

**v10 运行层重构已全部完成。** 实际耗时：~28 天（2026-06 起至 2026-06-27），略小于预估 34-49 天。

```
Wave 1  ~3 天    定义 + 标注          ── ✅
  │
Wave 2  ~4 天    减法（2a→2b→2c→2d）  ── ✅
  │
Wave 3  ~10 天   核心重构（3a→3b→3c-i）── ✅
  │              包括 loop-engine→goal-engine 重命名
Wave 4  ~5 天    QG Route + 6 Checkers  ── ✅
  │              包括 runtime-exit-gate 删除
Wave 5  ~8 天    路由整合 + 文档治理    ── ✅
  │
Wave 6  ~1 天    sub_scene 专项治理     ── ✅
```

**实际产出**：
- ~4000+ 行删除，~1650 行新增（符合预估）
- 删除 4 个 crate/模块：`runtime-exit-gate`、`closeout_enforcement`、`telemetry_emit`、`telemetry_observer`
- `looop-engine` → `goal-engine` 重命名
- 新增 `core/quality-gate/` crate（5 源文件 + 6 Checker 适配器）
- 全部 `once_lock_hook!` 宏消除，hooks.rs 56% 缩减
- 关闭 MCP tool：`framework_closeout`、`framework_quality_gate start/close_gates`

**遗留项**（不在 v10 范围内）：
- `agent-orchestrator` 重命名 + MCP 工具集成 — ✅ 已于 2026-06-27 完成
- `Result<_, String>` → `FrameworkError` 全量迁移（单独治理）
- 测试覆盖率提升（核心函数覆盖率仍偏低）

**最终架构验证条件**：
- `cargo test --no-run` 编译通过 ✅
- `cargo test` 全过 ✅
- 运行层 3 概念已收敛：Task + Goal（只 loop）+ QG Route ✅
