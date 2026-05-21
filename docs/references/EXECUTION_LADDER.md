# Execution Ladder（完整）

`AGENTS.md` → Execution Ladder 节为摘要；本文件为完整规则。

## 宿主优先级

- **Cursor**：`execution-subagent-gate.mdc` / `review-subagent-gate.mdc` 为执行面差异真源；用户要求不用子代理时豁免。
- **Review 硬点**：`router-rs` 校验 review 证据链。Cursor：`.cursor/hook-state` + Stop `REVIEW_GATE`（wave-2 phase/multiset）。Codex：`.codex/hook-state` + Stop `CODEX_REVIEW_GATE`（wave-2 部分：PostTool 可数证据 + Stop compact/rg_clear；`ROUTER_RS_CODEX_REVIEW_GATE_DISABLE=1` 关闭）。
- **Codex / 无 Cursor 规则**：默认主线程执行；显式 subagent、My 执行区（`/implementx`、`/verifyx`）、`/team` 才 sidecar。

## Review

- 深度 review：**spawn-first 配对审稿**——首轮主线程工具前先 spawn 可数只读 reviewer（`fork_context=false`）；主线程调研须 **另开** 独立 reviewer（`explore` 不计入证据）。细则 `skills/code-review-deep/SKILL.md`。
- lane 闭集见 `host_adapter_contract.md` §0.1；**不提高** wave-2 Stop 清门阈值。
- **窄范围**（单文件路径 review、`small_task`、不用子代理）：不武装 review gate，**不得** Stop-block。
- 默认 **review-only**，禁止默认改代码。
- 清门 token（单独一行）：`small_task`、`shared_context_heavy`、`write_scope_overlap`、`next_step_blocked`、`verification_missing`、`token_overhead_dominates`。

## 并行与 subagent

- 可拆 ≥2 独立子问题时默认并行；通常 3–5 个 `fork_context=false` lane。
- 写入 disjoint；不修改共享 continuity artifact。
- `/team` 仅显式调用或需 worker 协作时。

## 执行循环

- 默认 goal-style：plan → implement → verify → repair → closeout（纯 review 除外）。
- `/implementx`：goal 契约 + `GOAL_STATE.json` + 一口气跑完 `WAVE_STATE`；续跑仅 `framework_goal_drive` stdio + `artifacts/current/<task_id>/` 手动画板（**无** Stop/SessionStart 的 `GOAL_CONTINUE` / digest hook 注入）。Codex 见 [`docs/hosts/codex-cli.md`](../hosts/codex-cli.md)。

## Review 与 GSD 同轮混写（Cursor）

`beforeSubmit`：`review_arms_for_gate = review && !goal_drive_entrypoint`。同条消息里深度 review + `/implementx` 时本回合**不**新武装 review；「先审再 execute」须**拆两轮**。见 [`framework_operator_primer.md`](../framework_operator_primer.md)。
