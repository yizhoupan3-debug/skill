# 推理深度契约（非 CoT）

## 原则

**不靠单模型拉长 CoT**；**靠 `review ∥ external → fix → verify` 的结构化分工**，并把验证过程落到 **`EVIDENCE_INDEX`（及每轮 `append_round`）**，形成 **可审计链**。

## 含义

| 做法 | 说明 |
|------|------|
| **分工** | review（内审）与 external（外研）可并行；fix 只动约定范围；verify 只跑约定命令、只报结果（默认不修）。 |
| **深度** | 来自多视角对照 + **可执行验证**，不是单线程 prose 变长。 |
| **审计** | 终端类验证命令在连续性就绪时可由 hook 写入 `EVIDENCE_INDEX`；每轮 RFV 决策必须 `append_round` 落盘。 |

## Supervisor 自检（每轮）

- [ ] A 阶段是否 **并行** 仅包含 **只读** lane（review + optional external）？
- [ ] **verify** 是否对应明确 `verify_commands`，且 **PASS/FAIL** 有命令/日志而非「感觉通过」？
- [ ] 本轮是否写入 **`append_round`**（含 `verify_result`）？

## 可程序化硬门禁（`completion_gates` / `close_gates`）

与 **advisory rollup**（`DepthCompliance` / `resolve_task_view` 的 `depth_score`、continuity digest 一行「深度信号」）分工如下：

| 维度 | advisory rollup | 硬门禁（opt-in） |
|------|-----------------|------------------|
| **默认** | 总是计算、供 digest / statusline 展示 | **默认关闭**；仅当账本中显式写入 gate 对象且 `enabled` 非 `false` 时生效 |
| **真源** | `task_state::depth_compliance_aggregate` → `ResolvedTaskView.depth_compliance` | 同一条聚合链：**不**在 hook bash 复制第二套深度算法 |
| **GOAL** | `depth_score` 等字段仅提示 | `GOAL_STATE.completion_gates`：在 `framework_autopilot_goal` **`operation=complete`** 路径校验；失败返回 **Err**、**不写** `status=completed` |
| **RFV** | 同上 | `RFV_LOOP_STATE.close_gates`：在写入前对「收口轮」预览态跑同一套 `depth_compliance_aggregate` 校验；触发于 **supervisor 显式 `close`/`closed`**，以及 **`max_rounds` 耗尽**（`append_round` 上 `round_n >= max_rounds` 且非 `block`，账本将记 `loop_status=closed`）的自动收口；失败 **Err**、**不写**本轮（磁盘轮次保持追加前状态） |
| **SKIPPED** | 计数进 rollup | 若 `require_last_round_verify_pass` 等约束开启，**显式 close** 或 **`max_rounds` 耗尽自动 closed** 仍可能被拒（由你 opt-in） |

### `completion_gates`（`GOAL_STATE.json`）

可选对象；缺省或 `null` → 关闭。字段（均为可选布尔/数值，语义以 Rust 为准）：

| 字段 | 含义 |
|------|------|
| `enabled` | 缺省 `true`（对象存在时）；显式 `false` 则整段关闭 |
| `min_depth_score` | `0..=3`，要求 `DepthCompliance.depth_score`（受 `ROUTER_RS_DEPTH_SCORE_MODE` 影响）≥ 该值 |
| `require_successful_evidence_row` | 要求 `EVIDENCE_INDEX` 至少一条成功行（与 rollup `has_successful_verification` 对齐） |
| `min_goal_checkpoints` | 要求 `GOAL_STATE.checkpoints` 数组长度 ≥ 该值 |
| `block_on_rfv_pass_without_evidence` | 若 `rfv_pass_without_evidence_count>0` 则拒绝 complete（与 RFV `cross_check` / rollup 对齐） |

### `close_gates`（`RFV_LOOP_STATE.json`）

可选对象；缺省或 `null` → 关闭。

| 字段 | 含义 |
|------|------|
| `enabled` | 同 GOAL |
| `require_last_round_verify_pass` | 收口轮 `verify_result` 必须为 **`PASS`**（`SKIPPED` 显式 close 可被拦下） |
| `min_depth_score` | 对 **预览态** RFV 状态（含本轮拟追加轮次）跑同一套 `depth_compliance_aggregate` 再比阈值 |
| `block_on_rfv_pass_without_evidence` | 与 rollup `rfv_pass_without_evidence_count` 对齐 |
| `require_external_research_object_when_strict_on_close` | 当 `allow_external_research` 且 `external_research_strict` 时，要求收口轮带 **结构化** `external_research` 对象（结构化/strict 形状仍走既有 `append_round` 校验） |

### Closeout **R9**（可选）

将 closeout 记录与「某 `task_id` 上声明的 depth 策略」对齐的 **R9** 规则当前 **明确推迟**：`CloseoutRecord` 为 `deny_unknown_fields` 封闭 schema，引入任务级策略需全链路 schema + 解析任务指针；现阶段用 **`completion_gates` / `close_gates`** 在 GOAL complete / RFV close 上硬拦即可。参见 `closeout_enforcement.rs` 内 R9 注释。

---

## 反模式

- 用外研长文代替 verifier 跑命令。
- 单 agent 在同一上下文里轮流扮演 reviewer/fixer/verifier 却声称「多 lane」。
- 完成叙事不指向 `EVIDENCE_INDEX` 或等价 exit code 记录。
- 在同一上下文里轮流扮演 **事实核查 / 方法论批评 / 利益相关方视角**（或其它多视角），却声称「并行多 lane」——与上条同源：深度退化为文风，而非结构化对照。
- 外研只产出「读起来专业」的综述，没有 **可复跑检索轨迹**、没有 **与主张相悖的证据**，却把本轮标成「调研完成」。

---

## 提升调研深度的 harness 方向（契约级计划）

以下是对 **external（外研）** 与 **检索可审计性** 的加强约定；与 [`lane-templates.md`](lane-templates.md) 中的 **External research — 深度输出** 块对齐。门控上：**Contradiction sweep 与检索轨迹缺失时，supervisor 在 harness 语义上应把「深度调研」判为未完成**，除非本轮显式降级为 fast-check 并写入 `append_round` 理由。

### A. 外研 lane 的输出契约要像 API，不像随笔

与现有 **external** 角色类比：强制产出**结构块**（字段名固定、可机读汇总进 `external_research_summary`），例如：

| 块 | 要求 |
|----|------|
| **Claims** | 可证伪的主张列表；**每条**必须挂 **可追溯来源**（URL / DOI / 章节 / 数据集标识与版本）。 |
| **Contradiction sweep** | **硬性**：列出与 Claim **相悖** 或 **限制适用范围** 的证据与来源。缺此块 → 深度调研在 harness 语义上 **未完成**。 |
| **Unknowns** | 明确哪些问题 **证据不足** 或需额外实验/数据；比「很长的总结」更接近研究质量标准。 |

### B. 检索要「留下可复核轨迹」而非「读起来专业」

将下列内容固定为字段（写入外研 lane 输出，并由 supervisor 压缩进 `append_round` / 必要时指向 `EVIDENCE_INDEX` 同源审计链）：

- **检索式**（或等价查询接口）、**命中筛选规则**、**排除了什么**、**为何如此裁剪**。
- 对 **定量结论**：**数据版本**、**截取窗口**、**复算命令**（即另一类 `verify_commands`：`python` / `R` / `duckdb` 等，而非仅限 `cargo test`）；复算输出可由 verifier 或 supervisor 执行后进入 `EVIDENCE_INDEX`（与 PostTool 启发式命中时自动记账；未命中则人工粘贴或 `hook-evidence-append`）。

### C. 多视角并行，但角色要真分离

- **事实核查**、**方法论批评**、**利益相关方视角**（或其它正交 lens）应为 **多路并行、只读**，汇总后再进入 **fix**；禁止在同一上下文中串行「换帽子」冒充多 lane。
- 若需多于「reviewer ‖ external」两路：每路 **独立 subagent**、**本轮唯一角色**、**禁止**在未汇总前进 `fix_scope`（与 [`agent-swarm-orchestration`](../../../skills/agent-swarm-orchestration/SKILL.md) 只读边界一致）。

---

## 数理推理强度（STEM）

**不靠 prose 堆长推导**；靠 **witness 拆分 + 双轨可执行对照 + 符号 checker 的 PASS/FAIL**，与上文「推理深度」同一理念在数学上的落实。

- **契约长文**：[math-reasoning-harness.md](math-reasoning-harness.md)（中间对象、CAS/SMT、依赖图、反事实探针）。
- **Lane 模板**：[lane-templates.md](lane-templates.md) 中「数理 / STEM 专项」各 lane。
- **宿主续跑短句**：RFV / AUTOPILOT 默认只输出紧凑状态、证据锚点与必要 schema 路径；RFV 会在外研开启时追加 **`retrieval_trace_harness_line`**，在数学 / checker 语境命中时追加 **`math_reasoning_harness_line`**。文案真源为 `configs/framework/HARNESS_OPERATOR_NUDGES.json`。
- **结构化落盘与外研单行提示**：参见 [external-research-harness.md](external-research-harness.md)。
