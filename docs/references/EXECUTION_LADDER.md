# Execution Ladder（完整）

`AGENTS.md` → Execution Ladder 节为摘要；本文件为完整规则。

## 宿主优先级

- **Cursor**：`execution-subagent-gate.mdc` / `review-subagent-gate.mdc` 为执行面差异真源；用户要求不用子代理时豁免。
- **Review 硬点**：`router-rs` 校验 review 证据链；见 `.cursor/hook-state` 与 Stop 短码 `REVIEW_GATE`。
- **Codex / 无 Cursor 规则**：默认主线程执行；显式 subagent、GSD 执行区（`/gsd-execute-phase` 等）、`/team` 才 sidecar。

## Review

- 深度 review：`fork_context=false` 只读 reviewer；lane 闭集见 `host_adapter_contract.md` §0.1。
- 默认走 `skills/code-review-deep/SKILL.md`：**review-only**，禁止默认改代码。
- 清门 token（单独一行）：`small_task`、`shared_context_heavy`、`write_scope_overlap`、`next_step_blocked`、`verification_missing`、`token_overhead_dominates`。

## 并行与 subagent

- 可拆 ≥2 独立子问题时默认并行；通常 3–5 个 `fork_context=false` lane。
- 写入 disjoint；不修改共享 continuity artifact。
- `/team` 仅显式调用或需 worker 协作时。

## 执行循环

- 默认 goal-style：plan → implement → verify → repair → closeout（纯 review 除外）。
- `/gsd-execute-phase`：goal 契约 + `GOAL_STATE.json` + 连续执行至 blocker 或完成（Cursor Stop：`GSD_GOAL_CONTINUE`；Codex 见 `docs/hosts/codex-cli.md`）。

## Review 与 GSD 同轮混写（Cursor）

`beforeSubmit`：`review_arms_for_gate = review && !goal_drive_entrypoint`。同条消息里深度 review + `/gsd-execute-phase` 时本回合**不**新武装 review；「先审再 execute」须**拆两轮**。见 [`framework_operator_primer.md`](../framework_operator_primer.md)。
