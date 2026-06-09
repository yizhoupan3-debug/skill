---
last_verified: "2026-06-02"
depends_on:
  - ../../AGENTS.md
---

# Execution Ladder（完整）

`AGENTS.md` → Execution Ladder 节为摘要；本文件为完整规则。

## 宿主优先级

- **Cursor**：`execution-subagent-gate.mdc` / `review-subagent-gate.mdc` 为执行面差异真源；用户要求不用子代理时豁免。
- **Review 硬点**：`router-rs` 校验 review 证据链。Cursor：`.cursor/hook-state` + Stop `REVIEW_GATE`（wave-2 phase/multiset）。Codex：`.codex/hook-state` + Stop `CODEX_REVIEW_GATE`（wave-2 部分：PostTool 可数证据 + Stop compact/rg_clear；`ROUTER_RS_CODEX_REVIEW_GATE_DISABLE=1` 关闭）。Antigravity：依赖 MCP `goal_state_manage` 的强拦截与物理 `review-lanes/*.md` 文件的扫描。
- **Codex**：积极鼓励多代理并行（详见 [`docs/hosts/codex.md` § 多代理编排](../hosts/codex.md)）。`/implementx` 执行区 `parallel` 模式应主动 spawn 子代理 lane；深度 review 默认 spawn-first（`fork_context=false`）。无 hook 级子代理事件，行为由文档契约约束。

## Review

- 深度 review：**spawn-first 配对审稿**——首轮主线程工具前先 spawn 可数只读 reviewer（`fork_context=false`）；主线程调研须 **另开** 独立 reviewer（`explore` 不计入证据）。Cursor 可显式 `Task` + `subagent_type=deep-reviewer`（见 [`.cursor/agents/deep-reviewer.md`](../../.cursor/agents/deep-reviewer.md)）。细则 `skills/code-review-deep/SKILL.md`。
- lane 闭集见 `docs/host_adapter_contract.md` §0.1；**不提高** wave-2 Stop 清门阈值。
- **窄范围**（单文件路径 review、`small_task`、不用子代理）：不武装 review gate，**不得** Stop-block。
- 默认 **review-only**，禁止默认改代码。
- 清门 token（单独一行）：`small_task`、`shared_context_heavy`、`write_scope_overlap`、`next_step_blocked`、`verification_missing`、`token_overhead_dominates`。

## 并行与 subagent

- **Cursor 模型**：并行 `Task`/子代理 **默认继承主会话模型**（省略 `model`）；禁止默认 Claude/Sonnet，除非主会话已选 Anthropic。见 `.cursor/rules/subagent-model-inherit.mdc`、`docs/hosts/cursor.md`（`Couldn't start` / region）。
- 可拆 ≥2 独立子问题时默认并行；通常 3–5 个 `fork_context=false` lane。
- 写入 disjoint；不修改共享 continuity artifact。
- `/workflow`：跨宿主 JS workflow 编排（`agent-swarm-orchestration`；Claude Code 可 native，其它宿主 supervisor）。
- `/team` 已退役（2026-06）；多 agent 显式入口为 `/workflow`（`agent-swarm-orchestration`）。

## 执行循环

- 默认 goal-style：plan → implement → verify → repair → closeout（纯 review 除外）。
- `/implementx`：goal 契约 + `GOAL_STATE.json` + 一口气跑完 `WAVE_STATE`；续跑仅 `framework_goal_drive` stdio + `artifacts/current/<task_id>/` 手动画板（**无** Stop/SessionStart 的 `GOAL_CONTINUE` / digest hook 注入）。Codex 见 [`docs/hosts/codex.md`](../hosts/codex.md)。

## Review 与 My implement 同轮混写（Cursor）

`beforeSubmit`：`review_arms_for_gate = review && !goal_drive_entrypoint`。同条消息里深度 review + `/implementx` 时本回合**不**新武装 review；「先审再 execute」须**拆两轮**。见 [`framework_operator_primer.md`](../framework_operator_primer.md)。
