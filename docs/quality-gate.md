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
}
```

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
        crate::checkers::register_checkers(&mut registry);
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
set_extern_checkers(research_harness::register_qg_checkers);
```

这桥接 `research-harness` crate 的 checker 注入到 QG Route，避免 runtime-core 与 research-harness 间的直接依赖。

---

## 4. QG Entry

`core/runtime-core/src/qg_entry.rs`

两阶段退出门，被 GoalEngine 在 goal "complete" 操作的最后一步调用。

### 4.1 两阶段结构

```
trigger(repo_root, task_id, scene, goal, round, runtime_handle) → GateVerdict
    │
    ├── Stage 1: Anti-fraud gate
    │   ├── 调用 core_state::state_manager::task_evidence_artifacts_summary_for_task()
    │   ├── 存在证据但全失败 → P0 阻断，不执行 Stage 2
    │   └── 无证据 / 证据通过 → 进入 Stage 2
    │
    └── Stage 2: Quality Gate
        ├── 构造 CheckContext（含 evidence_path）
        ├── 调用 evaluate_qg_route(scene, &ctx)
        └── 返回聚合 GateVerdict
```

**Stage 1 逻辑**（D5 规则）：
- `has_evidence && !evidence_ok`：证据已存在但全部失败 → 欺诈嫌疑，P0 阻断
- `!has_evidence`：无证据 → 空 task 列表 = 无欺诈可能，通过
- `has_evidence && evidence_ok`：有通过证据 → 通过

### 4.2 向后兼容的 Hook Wrapper

`quality_gate_hook_wrapper(payload: serde_json::Value)` 适配旧有的 `framework_quality_gate` MCP 工具接口，抽取 payload 中的 `repo_root`/`task_id`/`goal`/`round` 字段后调用 `trigger()`。注：固定使用 `scene::GENERAL`。

---

## 5. Checkers

所有 12 个 checker 位于 `core/runtime-core/src/checkers/`。分为两组：

### 5.1 就地 Checker（Wave 4b，场景——GENERAL / CODE_REVIEW / VISUAL / SLIDES）

| Checker | 文件 | 场景 | 描述 |
|---------|------|------|------|
| `EvidenceChecker` | `evidence_checker.rs` | GENERAL, CODE_REVIEW, RESEARCH | 验证 task 存在且成功（扫描 EVIDENCE_INDEX.json） |
| `AdversarialChecker` | `adversarial_checker.rs` | GENERAL | 通用对抗检查：证据文件存在性、单轮完成警告 |
| `CorrectnessChecker` | `correctness_checker.rs` | CODE_REVIEW | 扫描 unwrap() / todo!() / unimplemented!() 计数 |
| `SecurityChecker` | `security_checker.rs` | CODE_REVIEW | 扫描 unsafe / transmute / Command::new(var) / shell 执行 |
| `ScreenshotLayoutChecker` | `screenshot_layout_checker.rs` | VISUAL | 截图布局一致性验证（当前为 C 级占位） |
| `OverflowChecker` | `overflow_checker.rs` | SLIDES | 检测幻灯片生成的溢出条件（当前为 C 级占位） |

### 5.2 验证技能 Checker 别名（Wave 5b，场景——RESEARCH）

这 6 个 checker 是 `research-harness` 验证技能的 QG 适配别名。当前实现为 C 级占位，实际检查逻辑由独立的 verification 技能模块提供：

| Checker | 文件 | 场景 | 描述 |
|---------|------|------|------|
| `ProseQcChecker` | `prose_qc.rs` | RESEARCH | 文本质量：术语一致性、风格合规、声明漂移检测 |
| `LiteratureGateChecker` | `literature_gate.rs` | RESEARCH | 文献验证：DOI 可达性、引用-声明对齐、矛盾扫描 |
| `StatisticalGateChecker` | `statistical_gate.rs` | RESEARCH | 统计验证：p 值重算、GRIM 检验、效应量报告 |
| `ReproducibilityChecker` | `reproducibility.rs` | RESEARCH | 可重现性：种子锁定、确定性重跑、环境锁定 |
| `StructureGateChecker` | `structure_gate.rs` | RESEARCH | 结构验证：LaTeX 编译、交叉引用一致性、格式合规 |
| `FormalGateChecker` | `formal_gate.rs` | RESEARCH | 形式验证：CAS 恒等式、SMT 一致性、量纲分析 |

### 5.3 注册矩阵

所有 checker 通过 `register_checkers()` 注册到 `CheckerRegistry`：

```rust
pub fn register_checkers(registry: &mut CheckerRegistry) {
    // GENERAL
    registry.register(scene::GENERAL, Box::new(EvidenceChecker));
    registry.register(scene::GENERAL, Box::new(AdversarialChecker));
    // CODE_REVIEW
    registry.register(scene::CODE_REVIEW, Box::new(CorrectnessChecker));
    registry.register(scene::CODE_REVIEW, Box::new(SecurityChecker));
    // VISUAL
    registry.register(scene::VISUAL, Box::new(ScreenshotLayoutChecker));
    // SLIDES
    registry.register(scene::SLIDES, Box::new(OverflowChecker));
    // RESEARCH (6 verification skill adapters)
    registry.register(scene::RESEARCH, Box::new(ProseQcChecker::new()));
    registry.register(scene::RESEARCH, Box::new(LiteratureGateChecker::new()));
    registry.register(scene::RESEARCH, Box::new(StatisticalGateChecker::new()));
    registry.register(scene::RESEARCH, Box::new(ReproducibilityChecker::new()));
    registry.register(scene::RESEARCH, Box::new(StructureGateChecker::new()));
    registry.register(scene::RESEARCH, Box::new(FormalGateChecker::new()));
}
```

---

## 6. GoalEngine Integration

QG 系统与 GoalEngine 通过函数指针注册模式集成。集成点位于 `core/core-state/src/state_manager/goal_ops.rs`。

### 6.1 注册

在 `runtime_core::init_hooks()` 中注册 QGEntry 函数指针：

```rust
core_state::state_manager::register_qg_entry_trigger(
    |repo_root, task_id, scene, goal, round| {
        let verdict = qg_entry::trigger(repo, task_id, scene, goal, round, None);
        serde_json::to_value(&verdict).unwrap_or_else(/* fallback */)
    },
);
```

### 6.2 Goal "complete" 操作

在 `framework_goal_drive_impl()` 的 `"complete"` 分支中：

```
1. 处理 loop goal →
2. 触发 QGEntry (invoke_qg_entry_trigger):
   ├── verdict.passed == true → 正常完成（归档 GOAL_STATE）
   └── verdict.passed == false → review_pending:
       ├── status = "review_pending"
       ├── blockers = verdict.blockers
       ├── 写入 GOAL_STATE.json
       └── 返回操作 "review_pending"
```

### 6.3 "continue_review" / "retry" 操作

当 goal 处于 `review_pending` 状态时允许重试：

```
1. 验证当前 status == "review_pending"
2. 清除 blockers 字段
3. 重置 status = "running"
4. 去活冲突的 QG loop state
5. 写入 GOAL_STATE.json
```

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
├── runtime-core::qg_entry   ─── 两阶段退出门 + 向后兼容 hook wrapper
└── runtime-core::checkers/  ─── 12 个 in-place GateChecker 实现

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
    ├── core-state (L0) ─── QGEntry 在 complete 分支调用 invoke_qg_entry_trigger
    │
    └── research-harness (L5, optional) ─── 通过 EXTERN_CHECKERS 函数指针注入外部 checker
```

### 7.3 关键设计决策

- **check() 是同步的**：异步 checker 通过 `CheckContext::runtime_handle` 桥接
- **CheckResult.passed 是自判**：最终 GateVerdict 由 `aggregate()` 按 severity 规则决定
- **无 Checker-level severity**：severity 属于 Finding，不在 CheckResult 层面
- **无 previous_results**：checker 之间不共享结果，纯函数合约
- **Scene 归一化**：未知 scene → GENERAL，不出 panic
- **未初始化 → 静默通过**：符合 P10（函数指针后备语义为 no-op）

### 7.4 外部 Checker 集成模式

```
应用层（router-rs-cli）:
  set_extern_checkers(research_harness::register_qg_checkers)
        │
        ▼
  runtime_core::init_hooks()
        │
        ▼
  qg_route::init_qg_route()
        ├── register_checkers()  ← 12 in-place checkers
        └── EXTERN_CHECKERS()   ← research-harness checkers (if registered)
```

这种 `OnceLock<fn>` 模式避免 runtime-core (L7) 与 research-harness (L5) 间的循环依赖，与应用层（L7 router-rs-cli）注入。

---

## 8. Migration from Old System

### 8.1 已删除

- **`core/runtime-exit-gate/` crate**（Wave 4a-ii）：旧的质量门状态机完整删除
- **`core/runtime-infra/src/telemetry_emit.rs`**：旧系统的 telemetry 发射器
- **`core/fr-exec/src/telemetry_observer.rs`**：旧系统的 telemetry 观察者
- **`core/framework-runtime-hooks/` crate**（已删除 Cargo.toml/lib.rs）
- 旧的 `quality_gate_drive` hook 实现
- `deactivate_goal_for_conflict_with_quality_gate()` 函数（Wave 4a-ii → QG 是 Goal 的内部模式）

### 8.2 向后兼容

- `quality_gate_hook_wrapper()` 在 `qg_entry.rs` 中提供，适配旧 MCP 工具 JSON 格式（`framework_quality_gate` / `framework_rfv_loop`）
- 注册为 `framework_kernel::runtime_hooks` 中的 `quality_gate_drive` 和 `framework_quality_gate` hook
- 旧 payload 格式：`{ "repo_root": "...", "task_id": "...", "goal": "...", "round": 1 }`

### 8.3 行为差异

| 方面 | 旧系统 (runtime-exit-gate) | 新系统 (QG Route) |
|------|---------------------------|-------------------|
| 状态机 | 独立 QG 状态机，与 goal 互斥 | QG 作为 Goal 的内部阶段 |
| 检查方式 | 单一大函数 | 可组合 checker 列表 |
| 场景分发 | 无 | scene 分发 + sub_scene 过滤 |
| 扩展性 | 需修改状态机 | 注册新增 GateChecker 即可 |
| 防欺诈 | 分离 | Stage 1 内置于 QGEntry |

### 8.4 与架构规约的对照

更多架构层面的描述见 `docs/architecture.md` §7（Quality Gate Route）。

Interfaces documented at:
- `quality_gate_hook_wrapper` in stdio_dispatch
- `TaskLedgerCommand::QualityGate` in `core/runtime-core/src/task_command.rs`
- QG state management in `core/core-state/src/state_manager/quality_gate_ops.rs`
