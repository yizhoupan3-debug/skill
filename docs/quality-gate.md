# Quality Gate System

> 独立、可组合的场景分发质量门评估系统，替代已删除的 `runtime-exit-gate` 旧系统。

---

## 1. Overview

Quality Gate (QG) 是 v10 架构引入的统一质量门评估系统。它替代了旧有的 `runtime-exit-gate` crate，提供了一个基于 `GateChecker` trait 的可插拔、场景分发的质量门框架。

### 1.1 设计目标

- **场景分发**：不同场景（代码审查、研究验证、视觉输出等）运行不同 set 的 checker
- **可组合**：checker 通过 trait 接口注册到 `CheckerRegistry`，新增 checker 无需修改评估逻辑
- **两阶段退出**：Stage 1 防欺诈（证据链验证）+ Stage 2 QG Route（场景分发 checker 评估）
- **就地适配**：checker 逻辑留在其自然模块位置，通过 `impl GateChecker` 适配注册（roadmap D007）
- **纯同步合约**：所有 checker 的 `check()` 方法为同步；异步操作通过 `CheckContext::runtime_handle` 桥接
- **结构化输出数据**：`CheckContext::output_data` 允许 MCP 工具 payload 直接传递任务输出给 checker

### 1.2 系统角色

QG 系统是 GoalEngine 流程的关键组成部分。每次 goal "complete" 操作的最后一步触发两阶段退出门：

```
task_complete → Stage 1: 防欺诈 → Stage 2: QG Route → 返回 GateVerdict
                                                                │
                         ┌──────────────────────────────────────┘
                         ▼
              passed=true → goal 正常完成
              passed=false → goal 进入 review_pending 状态，等待修复后重试
```

---

## 2. Core Types

QG 系统的核心类型定义在 `core/quality-gate/src/` 中（L4 crate）。

### 2.1 `GateChecker` Trait

`core/quality-gate/src/checker.rs`

所有 checker 必须实现此 trait。已是 `Send + Sync` 以支持跨线程共享。

```rust
pub trait GateChecker: Send + Sync {
    fn id(&self) -> &'static str;                                    // 唯一稳定标识，如 "adversarial"
    fn scenes(&self) -> Vec<&'static str>;                            // 适用的场景列表
    fn description(&self) -> &'static str;                           // 人类可读的描述
    fn check(&self, ctx: &CheckContext) -> CheckResult;              // 执行检查（必须同步）

    // 可选：子场景亲和性（Wave 6），返回 Some("sub_scene_name") 限制仅在该子场景运行
    fn sub_scene_affinity(&self) -> Option<&'static str> { None }
}
```

### 2.2 `Severity`

`core/quality-gate/src/types.rs`

| 枚举值 | 含义 | 门结果 |
|--------|------|--------|
| `P0` | 无条件阻断 | Gate 失败 |
| `A` | 阻断 | Gate 失败 |
| `B` | 阻断 | Gate 失败 |
| `Warning` | 非阻断建议 | 仅 Advisory |
| `C` | 仅供参考 | 仅 Advisory |

### 2.3 `Finding`

```rust
pub struct Finding {
    pub id: String,             // checker 范围内的唯一标识
    pub severity: Severity,     // 严重度
    pub description: String,    // 人类可读描述
    pub location: Option<String>,   // 可选源位置 (file:line)
    pub suggestion: Option<String>, // 可选修复建议
}
```

### 2.4 `CheckResult`

单个 checker 的检查结果。`passed` 是 checker 的自判结果，但聚合规则以 `Finding` 的 `Severity` 为准：

```rust
pub struct CheckResult {
    pub checker_id: String, // 对应 GateChecker::id()
    pub passed: bool,       // 自判是否通过（聚合以 findings severity 为准）
    pub findings: Vec<Finding>,
}
```

### 2.5 `GateVerdict`

聚合后的最终裁决：

```rust
pub struct GateVerdict {
    pub passed: bool,               // 无 P0/A/B 则 true
    pub checkers_ran: usize,       // 运行的 checker 数量
    pub blockers: Vec<Finding>,    // P0/A/B 级别 finding
    pub advisories: Vec<Finding>,  // Warning/C 级别 finding
    pub reason: Option<String>,    // 可读原因
}
```

**聚合规则**（§2.5）：
- 任何 `P0` → 无条件 gate 失败
- 任何 `A`/`B` → gate 失败
- 全部 `≤ Warning`（或空）→ gate 通过

**自证证据警告**（v7.2）：Stage 1 检测到所有成功证据均为 MCP 自证（`mcp_record_evidence`，无 host-bound `tool_call_id`）时，添加 `Severity::Warning` 级别 advisory 到 `advisories` 列表。调用方可通过 `advisories` 感知自证风险。（对应 P2-007）

### 2.6 `CheckContext`

每次 checker 评估传入的上下文：

```rust
pub struct CheckContext {
    pub scene: String,                                 // 场景标识
    pub sub_scene: Option<String>,                     // 可选子场景（Wave 6）
    pub goal: String,                                  // goal 描述
    pub round: u64,                                    // 当前验证轮次（1-based）
    pub repo_root: PathBuf,                            // 仓库根路径
    pub task_id: String,                               // 任务 ID
    pub evidence_path: Option<PathBuf>,                // EVIDENCE_INDEX.json 路径
    pub runtime_handle: Option<tokio::runtime::Handle>, // 异步运行时句柄
    pub output_data: Option<serde_json::Value>,        // 结构化任务输出数据（来自 MCP payload）
}
```

**`output_data` 设计说明**：该字段允许 MCP 工具 payload 中的结构化数据直接传递给 checker，无需扫描 repo 文件。checker 从 `ctx.output_data` 中提取自己关心的键，缺失时输出 C 级 info finding（graceful skip）。

### 2.7 `Scene`

`core/quality-gate/src/scene.rs`

五种有效场景常量：

| 常量 | 值 | 用途 |
|------|-----|------|
| `GENERAL` | `"general"` | 通用默认场景（fallback） |
| `RESEARCH` | `"research"` | 研究验证（论文、声明、证据） |
| `CODE_REVIEW` | `"code_review"` | 代码审查（正确性、安全、ABI） |
| `SLIDES` | `"slides"` | 幻灯片审查（溢出、字体、QA） |
| `VISUAL` | `"visual"` | 视觉输出审查（截图布局、可访问性） |

未知场景默认归一化为 `GENERAL`。

### 2.8 Sub-scene Filtering

`sub_scene` 字段用于在 RESEARCH 场景内进一步过滤 checker。当前有 2 个 checker 声明了 `sub_scene_affinity`：

| Checker | `sub_scene_affinity` | 含义 |
|---------|---------------------|------|
| `Reproducibility` | `"reproducibility"` | 仅在可重现性子场景运行 |
| `Structure` | `"structure"` | 仅在结构验证子场景运行 |

过滤逻辑：`CheckerRegistry::get_checkers_for_scene()` 返回某场景的全部 checker；`evaluate_qg_route()` 在分发前根据 `ctx.sub_scene` 和 `checker.sub_scene_affinity()` 过滤——affinity 为 `Some` 但与 `ctx.sub_scene` 不匹配的 checker 被跳过。

---

## 3. QG Route

`core/runtime-core/src/qg_route.rs`

QG Route 是持有 `CheckerRegistry` 单例的桥梁层，提供场景分发评估入口。

### 3.1 单例结构

```rust
static QG_ROUTE: OnceLock<CheckerRegistry> = OnceLock::new();
static EXTERN_CHECKERS: OnceLock<ExternCheckersFn> = OnceLock::new();
```

- `QG_ROUTE`：持有 `CheckerRegistry` 的 OnceLock 单例
- `EXTERN_CHECKERS`：可选的外部 checker 注册函数指针（避免循环依赖）

### 3.2 初始化

```rust
pub fn init_qg_route() {
    QG_ROUTE.get_or_init(|| {
        let mut registry = CheckerRegistry::new();
        // In-place checkers from runtime-core
        crate::checkers::register_checkers_from_registry(&mut registry);
        // External checkers from research-harness (if registered via set_extern_checkers)
        if let Some(f) = EXTERN_CHECKERS.get() {
            f(&mut registry);
        }
        registry
    });
}
```

### 3.3 评估入口

```rust
pub fn evaluate_qg_route(scene: &str, ctx: &CheckContext) -> GateVerdict
```

- 归一化 scene（未知 → `GENERAL`）
- 根据 scene 从 `CheckerRegistry` 查找对应的 checker 列表
- 支持可选的 `sub_scene` 过滤（Wave 6）
- 未初始化时返回 passed（no-op fallback，符合 P10）

### 3.4 外部 Checker 注册

```rust
// 由 router-rs-cli 在 bootstrap 阶段调用
set_extern_checkers(research_harness::register_checkers_from_registry);
```

这桥接 `research-harness` crate 的 checker 注入到 QG Route，避免 runtime-core (L7) 与 research-harness (L5) 间的直接依赖。

---

## 4. QG Entry

`core/runtime-core/src/qg_entry.rs`

两阶段退出门，被 GoalEngine 在 goal "complete" 操作的最后一步调用。

### 4.1 两阶段结构

```
trigger(repo_root, task_id, scene, goal, sub_scene, round, runtime_handle, output_data) → GateVerdict
    │
    ├── Stage 1: Anti-fraud gate
    │   ├── 调用 core_state::state_manager::task_evidence_artifacts_summary_for_task()
    │   ├── 存在证据但全失败 → P0 阻断，不执行 Stage 2
    │   └── 无证据 / 证据通过 → 进入 Stage 2
    │
    └── Stage 2: Quality Gate
        ├── 构造 CheckContext（含 evidence_path, output_data）
        ├── 调用 evaluate_qg_route(scene, &ctx)
        └── 返回聚合 GateVerdict
```

**Stage 1 逻辑**（D5 规则）：
- `has_evidence && !evidence_ok`：证据已存在但全部失败 → 欺诈嫌疑，P0 阻断
- `!has_evidence`：无证据 → 空 task 列表 = 无欺诈可能，通过
- `has_evidence && evidence_ok`：有通过证据 → 通过

### 4.2 调用入口

QG Entry 有两个主要调用路径：

1. **MCP 工具 `TaskLedgerCommand::QualityGate`**（`task_command.rs`）：
   - 从 payload 提取 `repo_root`/`task_id`/`goal`/`round`/`scene`/`sub_scene`/`output_data`
   - 先执行 `validate_transition()`（Stage 1 前置检查）
   - 再调用 `qg_entry::trigger()`
   - 这是 GoalEngine 的主要集成路径

2. **向后兼容 stdio dispatch**（`stdio_dispatch.rs`）：
   - 处理旧有的 `framework_quality_gate` MCP 操作
   - 固定使用 `scene::GENERAL`，`output_data = None`

---

## 5. Checkers

共 **16 个** checker，分为两组：6 个 in-place（runtime-core）+ 10 个外部（research-harness，通过 `EXTERN_CHECKERS` 函数指针注入）。

### 5.1 就地 Checker（runtime-core，场景分发如下）

| Checker | 文件 | 场景 | 状态 | 描述 |
|---------|------|------|------|------|
| `EvidenceChecker` | `evidence_checker.rs` | GENERAL, CODE_REVIEW, RESEARCH | ✅ Real | 验证 task 存在且成功（扫描 EVIDENCE_INDEX.json） |
| `AdversarialChecker` | `adversarial_checker.rs` | GENERAL | ✅ Real | 通用对抗检查：证据文件存在性、单轮完成警告 |
| `CorrectnessChecker` | `correctness_checker.rs` | CODE_REVIEW | ✅ Real | 扫描 unwrap() / todo!() / unimplemented!() 计数 |
| `SecurityChecker` | `security_checker.rs` | CODE_REVIEW | ✅ Real | 扫描 unsafe / transmute / Command::new(var) / shell 执行 |
| `ScreenshotLayoutChecker` | `screenshot_layout_checker.rs` | VISUAL | C-level stub | 截图布局一致性（需 image crate） |
| `OverflowChecker` | `overflow_checker.rs` | SLIDES | C-level stub | 幻灯片溢出检测（需 token counter） |

### 5.2 外部 Checker（research-harness，场景——RESEARCH）

这 10 个 checker 通过 `RUNTIME_REGISTRY.json` → `quality_gate_checkers.registrations` 注册（`crate: "research-harness"`），均由 `build.rs` 生成 `register_checkers_from_registry()`。

| Checker | 文件 | 状态 | 描述 |
|---------|------|------|------|
| `Asymptotic` | `asymptotic_gate.rs` | ✅ Wired | 渐近分析：链组合、量级估计、声明验证 |
| `DimensionalConsistency` | `formal_gate.rs` | ✅ Wired | 量纲一致性：SI 单位匹配、方程链验证 |
| `Inequality` | `inequality_gate.rs` | ✅ Wired | 不等式验证：LaTeX 解析、LP 可行性求解 |
| `Literature` | `literature_gate.rs` | ✅ Wired | 文献验证：DOI 可达性、引用-声明对齐 |
| `ProseQCChecker` | `prose_qc_gate.rs` | ✅ Wired | 文本质量：术语一致性、风格、声明漂移 |
| `Reproducibility` | `reproducibility_gate.rs` | ✅ Wired | 可重现性：种子锁定、确定性重跑、环境锁定 |
| `StatisticalChecker` | `statistical_gate.rs` | ✅ Wired | 统计验证：GRIM、p 值、多重比较、效应量 |
| `Structure` | `structure_gate.rs` | ✅ Wired | 结构验证：LaTeX 编译、交叉引用、格式合规 |
| `Symbolic` | `symbolic_gate.rs` | ✅ Wired | 符号验证：恒等式证明、等价性、增长分类 |
| `SympyBridge` | `sympy_bridge_gate.rs` | ✅ Wired | SymPy 验证：恒等式检查、表达式化简 |

### 5.3 注册矩阵

**In-place checkers**（由 `runtime-core/build.rs` 从 `RUNTIME_REGISTRY.json` → `quality_gate_checkers.registrations` (`crate: "runtime-core"`) 生成）：

```rust
// @generated by build.rs — from RUNTIME_REGISTRY.json → quality_gate_checkers.registrations
pub(crate) fn register_checkers_from_registry(registry: &mut quality_gate::CheckerRegistry) {
    registry.register(quality_gate::scene::GENERAL,
        Box::new(crate::checkers::evidence_checker::EvidenceChecker));
    registry.register(quality_gate::scene::GENERAL,
        Box::new(crate::checkers::adversarial_checker::AdversarialChecker));
    registry.register(quality_gate::scene::RESEARCH,
        Box::new(crate::checkers::adversarial_checker::AdversarialChecker));
    registry.register(quality_gate::scene::CODE_REVIEW,
        Box::new(crate::checkers::correctness_checker::CorrectnessChecker));
    registry.register(quality_gate::scene::CODE_REVIEW,
        Box::new(crate::checkers::security_checker::SecurityChecker));
    registry.register(quality_gate::scene::VISUAL,
        Box::new(crate::checkers::screenshot_layout_checker::ScreenshotLayoutChecker));
    registry.register(quality_gate::scene::SLIDES,
        Box::new(crate::checkers::overflow_checker::OverflowChecker));
}
```

**外部 checkers**（`research-harness`，由 `build.rs` 从 `RUNTIME_REGISTRY.json` 生成）：

```rust
// @generated by build.rs — from RUNTIME_REGISTRY.json → quality_gate_checkers.registrations
pub fn register_checkers_from_registry(registry: &mut quality_gate::CheckerRegistry) {
    registry.register(quality_gate::scene::RESEARCH,
        Box::new(crate::verification::asymptotic_gate::Asymptotic));
    registry.register(quality_gate::scene::RESEARCH,
        Box::new(crate::verification::formal_gate::DimensionalConsistency));
    registry.register(quality_gate::scene::RESEARCH,
        Box::new(crate::verification::inequality_gate::Inequality));
    registry.register(quality_gate::scene::RESEARCH,
        Box::new(crate::verification::literature_gate::Literature));
    registry.register(quality_gate::scene::RESEARCH,
        Box::new(crate::verification::prose_qc_gate::ProseQCChecker));
    registry.register(quality_gate::scene::RESEARCH,
        Box::new(crate::verification::reproducibility_gate::Reproducibility));
    registry.register(quality_gate::scene::RESEARCH,
        Box::new(crate::verification::statistical_gate::StatisticalChecker));
    registry.register(quality_gate::scene::RESEARCH,
        Box::new(crate::verification::structure_gate::Structure));
    registry.register(quality_gate::scene::RESEARCH,
        Box::new(crate::verification::symbolic_gate::Symbolic));
    registry.register(quality_gate::scene::RESEARCH,
        Box::new(crate::verification::sympy_bridge_gate::SympyBridge));
}
```

### 5.4 output_data JSON Schema

checker 从 `ctx.output_data` 提取数据，各 checker 期望的键如下：

| Checker | output_data key | 结构 |
|---------|----------------|------|
| `StatisticalChecker` | `grim` | `{ "mean": f64, "n": usize, "decimals": usize }` |
| | `p_value` | `{ "observed": f64, "expected": f64, "tolerance": f64 }` |
| | `multiple_comparison` | `{ "num_tests": usize, "correction_applied": bool }` |
| | `effect_size` | `{ "effect_size": Option<f64>, "test_type": String }` |
| `Asymptotic` | `magnitude_estimate` | `{ "expr": String, "var": String, "regime": String }` |
| | `chain` | `{ "steps": Vec<AsymptoticStep>, "var": String, "regime": String, "sympy_check": bool }` |
| | `claim` | `{ "f": String, "g": String, "relation": String, "var": String, "regime": String }` |
| `DimensionalConsistency` | `equations` | `Vec<String>` |
| `Symbolic` | `identity` | `{ "lhs": String, "rhs": String }` |
| | `equivalent` | `{ "lhs": String, "rhs": String }` |
| | `growth` | `{ "expr": String, "var": String }` |
| | `compare_growth` | `{ "f": String, "g": String, "var": String }` |
| `Inequality` | `inequalities` | `Vec<String>` (LaTeX) |
| | `timeout_ms` | `u64` |
| `SympyBridge` | `identity` | `{ "lhs": String, "rhs": String }` |
| | `simplify` | `String` |

所有键均为可选；缺失时 checker 输出 C 级 info finding，不阻断 gate。

---

## 6. GoalEngine Integration

QG 系统与 GoalEngine 通过 MCP 工具命令模式集成。集成点位于 `core/runtime-core/src/task_command.rs`。

### 6.1 MCP 工具调用

`TaskLedgerCommand::QualityGate(payload)` 是主要入口：

```
payload = {
    "repo_root": "/path/to/repo",
    "task_id": "task-123",
    "goal": "完成 X 功能",
    "scene": "research",        // 可选，缺省 GENERAL
    "sub_scene": "formal",      // 可选
    "round": 1,
    "output_data": { ... }      // 可选，结构化任务输出
}
```

### 6.2 Goal "complete" 流程

```
1. validate_transition(repo_root, task_id, Complete)  ← Stage 1 前置
   ├── !passed → 返回 P0 阻断（transition_validation_blocked）
   └── passed → 继续

2. qg_entry::trigger(repo_root, task_id, scene, goal, sub_scene, round, None, output_data)
   ├── Stage 1: Anti-fraud gate（证据链验证）
   └── Stage 2: QG Route（场景分发 checker 评估）

3. verdict.passed == true → goal 正常完成
   verdict.passed == false → goal 进入 review_pending
```

### 6.3 向后兼容 stdio dispatch

`core/runtime-core/src/framework_runtime/stdio_dispatch.rs` 提供对旧有 `framework_quality_gate` MCP 操作的兼容。固定使用 `scene::GENERAL` 且不传递 `output_data`。

### 6.4 QG State 管理

`core/core-state/src/state_manager/quality_gate_ops.rs` 提供：
- `quality_gate_state_path(repo_root, task_id)`：获取 QG state 文件路径
- `read_quality_gate_state(repo_root, task_id_override)`：读取 QG state 文件
- `deactivate_quality_gate_for_conflict_with_goal_drive(repo_root, task_id)`：标记 QG state 为 `superseded`

---

## 7. Architecture

### 7.1 层叠结构

```
L7 (Application Layer)
├── runtime-core::qg_route   ─── OnceLock<CheckerRegistry>, 外部 checker 桥接
├── runtime-core::qg_entry   ─── 两阶段退出门
├── runtime-core::checkers/  ─── 6 个 in-place GateChecker 实现
└── runtime-core::task_command ── MCP 工具入口，提取 output_data

L5 (Verification Layer)
└── research-harness         ─── 10 个 RESEARCH-scene GateChecker 实现

L4 (Contract / Library Layer)
└── quality-gate crate       ─── GateChecker trait, CheckerRegistry, 聚合逻辑
    ├── checker.rs           ─── GateChecker trait
    ├── registry.rs          ─── CheckerRegistry + evaluate() + aggregate()
    ├── scene.rs             ─── 场景常量与归一化
    └── types.rs             ─── Severity, Finding, CheckResult, GateVerdict, CheckContext

L0 (Kernel / Foundation Layer)
└── core-state               ─── evidence API + QG state 文件管理
```

### 7.2 依赖流向

```
quality-gate (L4) ← 无其他 crate 依赖
    ↑
runtime-core (L7) ─── 持 quality-gate 作为依赖
    │  ├── checkers/ 各 checker 通过 quality-gate::checker::GateChecker 适配
    │  └── qg_entry 通过 quality-gate::types 返回 GateVerdict
    │
    ├── core-state (L0) ─── evidence API
    │
    └── research-harness (L5, optional) ─── 通过 EXTERN_CHECKERS 函数指针注入 10 个外部 checker
```

### 7.3 关键设计决策

- **check() 是同步的**：异步 checker 通过 `CheckContext::runtime_handle` 桥接
- **CheckResult.passed 是自判**：最终 GateVerdict 由 `aggregate()` 按 severity 规则决定
- **无 Checker-level severity**：severity 属于 Finding，不在 CheckResult 层面
- **无 previous_results**：checker 之间不共享结果，纯函数合约
- **Scene 归一化**：未知 scene → GENERAL，不出 panic
- **未初始化 → 静默通过**：符合 P10（函数指针后备语义为 no-op）
- **output_data 纯可选**：checker 缺失数据时输出 C 级 info finding，不阻断

### 7.4 外部 Checker 集成模式

```
应用层（router-rs-cli）:
  set_extern_checkers(research_harness::register_checkers_from_registry)
        │
        ▼
  runtime_core::init_hooks()
        │
        ▼
  qg_route::init_qg_route()
        ├── register_checkers_from_registry()   ← 7 in-place checkers（build.rs 从 JSON 生成）
        └── EXTERN_CHECKERS()                    ← 10 research-harness checkers（build.rs 从 JSON 生成）
```

这种 `OnceLock<fn>` 模式避免 runtime-core (L7) 与 research-harness (L5) 间的循环依赖，由应用层（L7 router-rs-cli）注入。

---

## 8. Migration from Old System

### 8.1 已删除

- **`core/runtime-exit-gate/` crate**（Wave 4a-ii）：旧的质量门状态机完整删除
- **`core/runtime-infra/src/telemetry_emit.rs`**：旧系统的 telemetry 发射器
- **`core/framework-runtime-hooks/` crate**（已删除 Cargo.toml/lib.rs）
- 旧的 `quality_gate_drive` hook 实现
- `deactivate_goal_for_conflict_with_quality_gate()` 函数（Wave 4a-ii → QG 是 Goal 的内部模式）

### 8.2 行为差异

| 方面 | 旧系统 (runtime-exit-gate) | 新系统 (QG Route) |
|------|---------------------------|-------------------|
| 状态机 | 独立 QG 状态机，与 goal 互斥 | QG 作为 Goal 的内部阶段 |
| 检查方式 | 单一大函数 | 可组合 checker 列表（16 个） |
| 场景分发 | 无 | 5 个场景 + sub_scene 过滤 |
| 扩展性 | 需修改状态机 | 注册新增 GateChecker 即可 |
| 防欺诈 | 分离 | Stage 1 内置于 QGEntry |
| 数据传递 | N/A | CheckContext.output_data 结构化传递 |

---

## 附：SKILL.md 场景标注

每个 SKILL.md 的 frontmatter 包含 `scene` 和可选 `sub_scene` 字段，由 `generate.rs` 的 `FRONTMATTER_KEYS` 控制写入。场景分配见 `skills/SKILL_ROUTING_RUNTIME.json` 的 `scene` 列。

接口参考：
- `TaskLedgerCommand::QualityGate` in `core/runtime-core/src/task_command.rs`
- QG state management in `core/core-state/src/state_manager/quality_gate_ops.rs`
- `GateChecker` trait and CheckerRegistry in `core/quality-gate/src/` — see §3.1 Core Types above
