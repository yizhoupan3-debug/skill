# RESEARCH：Harness 深度调研 / 长时运行 / 数理推理（内外对照）

> 调研执行日期：2026-05-11。范围对齐 Cursor Plan「Harness 深度/长时/数理能力：内外部调研审查」；**未改计划文件**；本文件为交付物。

---

## 1. L3 vs L5 与 advisory / 硬门禁（一页对照）

| 维度 | L5（契约/技能长文） | L3（router-rs + hook） |
|------|---------------------|-------------------------|
| **「何为深度」语义** | `docs/references/rfv-loop/reasoning-depth-contract.md`：分工 `review ∥ external → fix → verify`、外研 API 式块、反模式列表 | 不承载领域长文；仅合并续跑块、PostTool 证据、`DepthCompliance` rollup |
| **数理语义** | `math-reasoning-harness.md`：witness、CAS/SMT、依赖图、反事实探针 | `HARNESS_OPERATOR_NUDGES.json` → `math_reasoning_harness_line` 为**短提醒**；非证明器 |
| **硬门禁** | 契约定义 `completion_gates` / `close_gates` 字段语义 | `autopilot_goal.rs` complete 路径、`rfv_loop.rs` append_round 预览路径调用同一 `depth_compliance_aggregate` |
| **叙事型 nudge** | 应进统一配置面（`HARNESS_OPERATOR_NUDGES.json`） | `harness_operator_nudges.rs` 合并 JSON + 内置默认；受 `ROUTER_RS_HARNESS_OPERATOR_NUDGES` 与 `ROUTER_RS_OPERATOR_INJECT` 闸断 |
| **digest 深度一行** | 契约指向 rollup 含义 | `task_state::depth_compliance_refresh_hint` → `深度信号: dN/3…`；**不受** `ROUTER_RS_HARNESS_OPERATOR_NUDGES=0` 关闭（见 `docs/harness_architecture.md` §8 表） |

**双真源 / 叙事风险（≤3 条）**

1. **digest「深度信号」与 operator nudge 闸断不对称**：关 nudge 仍见 `深度信号`，易误解为「关深度」；文档 §8 已说明，依赖读者读表。
2. **`close_gates` 触发路径**：~~曾写仅显式 close~~ → **已对齐**：契约与 `rfv_loop.rs` 一致，`max_rounds` 耗尽自动 closed 亦跑 gates（**Option B**，见 `docs/plans/ADR_rfv_close_gates_max_rounds.md`）。
3. **L4 bash**：`harness_architecture.md` 禁止在 L4 复制 L3 门控；新增宿主时回归依赖 checklist，无自动化「防复制」检测。

---

## 2. RFV 长时运行：`append_round`、close、close_gates

### 2.1 流程（实现真源 `scripts/router-rs/src/rfv_loop.rs`）

- **`append_round`**：规范化 `verify_result` ∈ {PASS,FAIL,SKIPPED,UNKNOWN}；写 `cross_link_evidence` 的 `evidence_refs` / `cross_check`；合并 `external_research`；将 round 推入 `rounds`；更新 `current_round`、`updated_at`、`loop_status`。
- **`loop_status`**：`supervisor_decision` 为 `close|closed` → `closed`；`block|blocked` → `blocked`；否则若 `round_n >= max_rounds` → `closed`，否则 `active`。
- **显式 close**：`supervisor_closes` 为真时，若配置了 `close_gates`，在**写入前**对「含本 entry 的预览态 state」跑 `enforce_rfv_close_gates`。
- **max_rounds 耗尽**：`closes_due_to_round_cap`（非 close/block 且 `round_n >= max_rounds`）时，**同样**在写入前跑 `enforce_rfv_close_gates`（若配置了 gates）。

### 2.2 与契约差异（审查结论）— **已关闭**

曾存在「契约写不触发 max_rounds 路径 / 代码仍触发」张力；执行决议为 **Option B（以代码为准，更新契约）**：见 **`docs/plans/ADR_rfv_close_gates_max_rounds.md`**；单测覆盖显式 close 与 round-cap 两条路径。

### 2.3 相关单元测试（`rfv_loop.rs` `mod tests`）

| 测试函数 | 主题 |
|----------|------|
| `rfv_start_append_roundtrip` | start + append + followup 消息含 RFV 与 nudge |
| `append_round_rejects_unknown_verify_result` | verify 枚举 |
| `append_round_marks_pass_without_evidence` | cross_check / PASS 无证据窗 |
| `rfv_start_clears_goal_same_task` | 与 GOAL 同 task 交互 |
| `source_traceable_heuristic_matrix` / `validate_external_research_strict_matrix` / `validate_external_research_struct_matrix` | 外研 strict/struct |
| `append_round_strict_rejects_without_round_write` 等 | strict/legacy 外研 |
| `append_round_persists_valid_external_research` | 合法结构化落盘 |
| `rfv_prefers_structured_hint_line_when_configured_and_last_round_gap` 等 | 结构化 hint |
| `append_round_close_gates_reject_skipped_when_require_pass` | close_gates + verify PASS |
| `append_round_close_gates_enforced_on_max_rounds_cap` / `append_round_max_rounds_cap_passes_close_gates_when_verify_pass` | close_gates + **max_rounds** 自动收口 |

---

## 3. `DepthCompliance`：strict / legacy 与数理记账缺口

### 3.1 Rollup 规则摘要（`task_state.rs`）

- **1 分**：至少一轮 RFV `verify_result=PASS`。
- **1 分**：`evidence_ok`（`EVIDENCE_INDEX` 存在成功行，`task_evidence_artifacts_summary_for_task`）。
- **1 分（第三分）**：
  - **legacy**：`goal_checkpoint_count > 0` **或** `rfv_adversarial_round_count > 0`。
  - **strict**（`ROUTER_RS_DEPTH_SCORE_MODE=strict`）：legacy **或** `rfv_falsification_test_count > 0` **或**（任务 `external_research_strict` 且 `rfv_external_strict_ok_round_count > 0`）。

计数器另含：`rfv_pass_without_evidence_count`（与 `cross_check=no_evidence_window` 对齐）、外研结构化轮次等。

### 3.2 数理 witness + verifier vs `EVIDENCE_INDEX` 自动记账

- **契约**：`math-reasoning-harness.md` 要求 SymPy / Z3 / `lake build` 等 **exit code** 为真值来源。
- **PostTool 启发式**（`framework_runtime/mod.rs` `shell_command_looks_like_verification`）：在原有 `cargo test` / `pytest` 等之外，已增加 **窄域** `sympy`、`z3`、`lean`/`lean4`、`coqc`、`lake build`/`lake test` 等子串（**仍避免**裸 `python` 作为唯一命中）；长尾仍可用 `hook-evidence-append`。
- **残余缺口**：任意 `python path/verify.py` 且路径不含关键字时仍可能不落 PostTool 行 → 改写命令或显式 append。

---

## 4. Goal `completion_gates` 与 nudge 注入链

### 4.1 `completion_gates` 字段（`task_state::GoalCompletionGates`）

| 字段 | 含义 |
|------|------|
| `enabled` | `false` → 校验 no-op |
| `min_depth_score` | 与 rollup `depth_score` 比较 |
| `require_successful_evidence_row` | `EvidenceRollup.has_successful_verification` |
| `min_goal_checkpoints` | `GOAL_STATE.checkpoints` 长度 |
| `block_on_rfv_pass_without_evidence` | `rfv_pass_without_evidence_count > 0` 则 Err |

**测试**：`autopilot_goal.rs` 中 `goal_complete_rejected_when_completion_gates_depth_not_met`、`goal_complete_allowed_when_completion_gates_satisfied`。

### 4.2 Nudge 注入面（`harness_operator_nudges.rs`）

- 真源：`configs/framework/HARNESS_OPERATOR_NUDGES.json`；`ROUTER_RS_HARNESS_OPERATOR_NUDGES=0` 或 `ROUTER_RS_OPERATOR_INJECT=0` → 空串。
- **RFV 续跑**：`build_rfv_loop_followup_message*` 合并推理深度 + `push_math_reasoning_line` + `push_retrieval_trace_line` + 条件 `push_rfv_external_struct_hint_line`。
- **Autopilot 续跑**：verbose/compact 推理深度键 + 同上数理/检索行（由 `rfv_loop.rs` 测试间接断言续跑含「数理」「检索」）。
- **Continuity digest GOAL 段**：`continuity_digest.rs` 使用同一 `ResolvedHarnessNudges`，并追加「深度自检」列表项 + 主线 `depth_compliance_refresh_hint`（§1 闸断不对称）。

---

## 矩阵

行：能力维度；列：本仓库落地柱。单元格：**满足 / 部分 / 缺口 / 风险** + 指针。

|  | L5 契约 | Rust 账本 / hook | 配置面 | 测试覆盖 | 外部对标 |
|--|---------|------------------|--------|----------|----------|
| **深度外研** | **满足** — `reasoning-depth-contract` §A–C、`external-research-harness.md`、`RFV_EXTERNAL_RESEARCH.schema.json` | **部分** — `validate_external_research_strict` + `append_round` 强校验；`cross_link_evidence` 窗口对齐 | **满足** — `HARNESS_OPERATOR_NUDGES.json` 检索/trace 行、`ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT` | **满足** — struct/strict 矩阵与 close_gates 用例 | **部分** — CRITIC / AutoGen 重工具与对话编排；本仓库更强求 **可执行 verify** 与磁盘 schema（见 Bibliography） |
| **长时运行** | **满足** — `rfv_loop_harness.md` max_rounds、停轮条件、`GOAL` 地平线 | **满足** — `RFV_LOOP_STATE.json` + `max_rounds` cap 1000；续跑 hook | **部分** — env 控制续跑注入；无内置「周级」调度器 | **部分** — roundtrip 与 close_gates；缺「跨进程崩溃恢复」E2E | **部分** — Voyager 技能库 + 课程；本仓库用 **git 真源 + artifacts** 而非环境模拟器状态 |
| **数理推理** | **满足** — `math-reasoning-harness.md` witness + checker 分层 | **部分** — rollup 仍仅用 falsification_tests；无 CAS 专用字段；续跑已注入 `math_reasoning_harness_line`（`harness_context_signals` + RFV/Autopilot） | **满足** — `math_reasoning_harness_line` / `retrieval_trace_harness_line` / struct 提示 wired | **部分** — PostTool 已收窄扩展数理子串 + 单测矩阵；裸 `python` 脚本名仍可能漏记 | **部分** — LeanDojo 重 Lean 数据与证明检索；本 harness 明确 **不** 在 L3 做 ATP（契约非目标） |

---

## Open questions（≤10，含 owner）

| # | 问题 | 建议 owner |
|---|------|------------|
| 1 | **已关闭。** `close_gates` 在 **`max_rounds` 耗尽自动 closed** 与显式 close 两路径均校验（**Option B**）；契约见 `reasoning-depth-contract` L30，实现见 `rfv_loop.rs`，决议见 [`ADR_rfv_close_gates_max_rounds`](ADR_rfv_close_gates_max_rounds.md)。原表「与实现矛盾」为历史措辞。 | `doc` + `router-rs`（归档） |
| 2 | **已解决。** PostTool 窄域子串（如 `z3`/`lean`/`sympy` 等）已在 `framework_runtime::shell_command_looks_like_verification` 落地 + 单测矩阵；**残余**长尾 `python path/verify.py`（路径无关键字）仍可能漏记 → §3.2 / `hook-evidence-append`。 | `router-rs`（残余长尾） |
| 3 | `深度信号` 是否应受 `ROUTER_RS_HARNESS_OPERATOR_NUDGES` 统一闸断（Breaking 风险）？ | `doc` / 产品 |
| 4 | R9（closeout 与 depth 策略对齐）推迟后，是否用文档指针替代 `closeout_enforcement` 内注释扩散？ | `router-rs` |
| 5 | 外研 `contradiction_sweep` 条数与 claims 的 strict 规则，是否需在 `DepthCompliance` 中单列计数（现依赖 strict_ok 轮次）？ | `router-rs` |
| 6 | **部分落地。** `codex_compact_contexts` 多段 merge 后再截断的 **前缀 / join 顺序** 已有回归：`codex_hooks::tests::codex_compact_contexts_preserves_join_order_under_small_budget`（`ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX=256`）。**未**覆盖全量 SessionStart 上 digest+nudge+goal 合成截断优先级 fixture。 | `router-rs` |
| 7 | 多宿主（Claude Code）与 Cursor 对 PostTool 事件形状差异，是否影响证据窗 `cross_link` 公平性？ | `router-rs` |
| 8 | `external_research_strict` 默认 true 迁移后，旧账本「宽松 blob」混跑策略文档是否足够显眼？ | `doc` |

---

## 6 PostTool 事件形状与证据窗（Open #7，简表）

| 宿主 / 入口 | 典型 tool 名字段 | `command` 预览提取 | 写入 `EVIDENCE_INDEX` 前置 |
|-------------|------------------|---------------------|---------------------------|
| Codex hook | `tool` / `tool_name` + shell 族名 | `tool_input.command`（及 `cmd` / `script` 回退） | `ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE` 开、连续性就绪、`shell_command_looks_like_verification` |
| Cursor hook | 由 `cursor_hooks` 归一到 shell 类名 | 同源预览字段 | 同上 |
| Claude Code（若接入） | 以 `RUNTIME_REGISTRY` 闭集为准 | 依赖各宿主 payload 映射；**未统一** | 长尾建议 **`hook-evidence-append`** |

**对 `cross_link_evidence` 的含义**：窗口键为「上一轮 `at`」与 `recorded_at` 字符串比较；不同宿主若缺一致时间戳，可能出现 `no_evidence_window` 审计标签偏多 — **不阻断** `append_round`，由 supervisor 人工核对。

---

## 7 执行决议（对齐附件计划）

| Open # | 处理 |
|--------|------|
| **#1** `close_gates` vs `max_rounds` | **Option B**：保留代码双路径，文档与 rustdoc 对齐；见 **`docs/plans/ADR_rfv_close_gates_max_rounds.md`**。 |
| **#2** PostTool 数理子串 | 已在 `framework_runtime::shell_command_looks_like_verification` 收窄扩展 + 单测矩阵。 |
| **#3** digest `深度信号` 与 `ROUTER_RS_HARNESS_OPERATOR_NUDGES` | **文档脚注**：`docs/harness_architecture.md` §8 表下脚注 — 当前 **不**随 nudge 闸断；Breaking 另议。 |
| **#4** R9 注释扩散 | `closeout_enforcement.rs` R9 注释已加 ADR / 契约指针。 |
| **#5** `contradiction_sweep` 单列计数 | **推迟**：`DepthCompliance` 仍以 `rfv_external_strict_ok_round_count` 等聚合为主；单列 sweep 条数需 product 再定。 |
| **#6** SessionStart 640 截断优先级 | **部分落地**：`codex_compact_contexts` 多段 join + `truncate_codex_additional_context` 前缀顺序见单测 `codex_compact_contexts_preserves_join_order_under_small_budget`；全 digest+nudge+goal 合成路径仍待专门 fixture（若需再开 execution）。 |
| **#7** 多宿主 PostTool | 见上 **§6** 简表。 |
| **#8** strict 默认迁移显眼度 | **`docs/rfv_loop_harness.md`** 增补 **`external_research_strict` 迁移** 小节。 |

---

## 8. Bibliography（可复核检索）

每条：**检索式** → **选用理由**。

| # | 来源 | 检索式 | 选用理由 |
|---|------|--------|----------|
| 1 | https://arxiv.org/abs/2303.11366 | `Reflexion verbal reinforcement learning arxiv` | 长程试错 + 语言反思记忆；对照本仓库「不靠单轮 CoT」时反思文本与 **可执行 verify** 的边界 |
| 2 | https://arxiv.org/abs/2305.11738 | `CRITIC tool-interactive self-correct arxiv` | 工具交互验证与修正循环；对齐 RFV verify 阶段与 CRITIC 的 tool 多样性差异 |
| 3 | https://arxiv.org/pdf/2305.16291 | `Voyager Minecraft skill library arxiv` | 长时 embodied 任务 + 可复用技能库；对照 `artifacts/current` 与「技能=代码」迁移 |
| 4 | https://arxiv.org/abs/2306.15626 | `LeanDojo theorem proving retrieval arxiv` | 形式化 + 检索；对照 `math-reasoning-harness` 的 Lean 层与仓库默认 **轻量 checker** 定位 |
| 5 | https://arxiv.org/pdf/2308.08155 | `AutoGen multi-agent conversation arxiv` | 多 agent 对话编排；对照 `review ∥ external` **真并行** 与 AutoGen「群聊」上下文隔离程度 |
| 6 | https://arxiv.org/abs/2303.17651 | `Self-Refine iterative refinement NeurIPS` | 单模型 FEEDBACK→REFINE；作**反例对照**说明与「多 lane 真分离」 harness 的哲学差异 |
| 7 | https://proceedings.neurips.cc/paper/2023/hash/1b44b878bb782e6954cd888628510e90-Abstract-Conference.html | `Reflexion NeurIPS 2023 proceedings` | 正式出版信息，补充 bibtex |
| 8 | https://github.com/MineDojo/Voyager | `Voyager github MineDojo` | 开源实现指针，长程实验可复现性对照 |

**外部实践摘要（计划要求每列 ≥2）**

- **长程状态**：Voyager 技能库 + 自动课程；Reflexion episodic verbal memory。→ 本仓库用 **磁盘账本 + GOAL checkpoint** 替代神经网络化记忆。
- **验证优先**：CRITIC 多工具验证；Reflexion 环境标量/语言反馈。→ 本仓库 **exit code + EVIDENCE_INDEX** 更窄、更可审计。
- **数理工具**：LeanDojo / ReProver；CRITIC 在数学程序合成上评测。→ 本仓库契约鼓励 SymPy/Z3/Lean，但 **L3 不自动选工具**。
- **多智能体只读**：AutoGen 可配置群聊；CRITIC 分离 critique 与 tool。→ 本仓库用 **lane 模板 + REVIEW_GATE** 强调 disjoint scope 与 hook 证据链。

---

## 9. Plan vs actual（对照附件计划 todos）

| Plan todo id | 状态 | 证据 |
|---------------|------|------|
| internal-layering | Done | §1 |
| internal-rfv-longrun | Done | §2 + 测试表 |
| internal-depth-math | Done | §3 |
| internal-goal-hooks | Done | §4 |
| external-benchmark | Done | §8 Bibliography 与「外部实践摘要」 |
| synthesis-matrix | Done | 「矩阵」「Open questions」两节 |
| closeout-gitx | Done | 本文件已落盘；`git status` 见执行步骤。**`/gitx plan`**：按 `skills/gitx/SKILL.md` 须在 **Cursor 宿主** 对本次变更做人工一条龙收口（本 CLI 无等价子命令）；以下为仓库内只读/记录命令结果。 |

---

## 10. Verify 命令记录（调研阶段）

```text
rg "close_gates|enforce_rfv" scripts/router-rs/src/rfv_loop.rs
rg "DepthCompliance|depth_score_mode" scripts/router-rs/src/task_state.rs scripts/router-rs/src/router_env_flags.rs
rg "completion_gates|math_reasoning_harness" scripts/router-rs/src/autopilot_goal.rs scripts/router-rs/src/harness_operator_nudges.rs
```

（可选）`cargo test --manifest-path scripts/router-rs/Cargo.toml rfv_start_append_roundtrip -- --nocapture` 用于本地回归单测；本调研未强制全量 `cargo test`。

---

## 11. 执行收尾（计划「Harness执行收尾上架」）

| 项 | 结果 |
|----|------|
| ADR / 契约与实现对照 | 已通过 `rg` 核对 `reasoning-depth-contract.md` 与 [ADR_rfv_close_gates_max_rounds.md](ADR_rfv_close_gates_max_rounds.md) 一致（Option B）。 |
| Codex `AGENTS.md` 同步 | `AGENTS.md` 有未提交改动时已执行：`cargo build --release --manifest-path scripts/router-rs/Cargo.toml` + `/tmp/skill-cargo-target/release/router-rs codex sync --repo-root "$PWD"`（exit 0）。 |
| `cargo test`（router-rs 全量） | **515 passed**（约 24–26s）。 |
| `docs/README.md` 索引 | 已在「按主题」RFV 行链到 ADR（`plans/ADR_rfv_close_gates_max_rounds.md`）。 |
| `/gitx plan` | **须在 Cursor 宿主人工执行**（见 `skills/gitx/SKILL.md`）；本段替代不了 Git 提交/推送。 |
