# Cursor Agent Policy

跨宿主协议见 [`AGENTS.md`](AGENTS.md)。**双文件注入**：须与 `AGENTS.md` 同时生效，勿单独使用本文件。本文仅 **Cursor**（`cursor`）transport delta。手册 [`docs/hosts/cursor.md`](docs/hosts/cursor.md) · [`docs/spec.md`](docs/spec.md) §0.1。

## Transport 要点

- **Hook**：`.cursor/hooks.json` + `router-rs cursor hook`（7 事件闭集）；清门 **Claude canonical**；Stop **advisory-only**。门控细则 [`.cursor/rules/review-subagent-gate.mdc`](.cursor/rules/review-subagent-gate.mdc)、[`.cursor/rules/execution-subagent-gate.mdc`](.cursor/rules/execution-subagent-gate.mdc)。
- **机读短码**：`REVIEW_GATE`、`AG_FOLLOWUP`、`CLOSEOUT_FOLLOWUP`（须 `router-rs ` 前缀）；**my-light** suppress `REVIEW_GATE` / `AG_FOLLOWUP`。清门粘贴 **`rg_clear`** 或拒因 token（见 [`AGENTS.md`](AGENTS.md) Execution Ladder）。
- **`updateCurrentStep`**：禁止空载荷；须含可机读步骤或状态。
- **子代理模型**：并行 `Task` 默认继承主会话（省略 `model`）；见 [`.cursor/rules/subagent-model-inherit.mdc`](.cursor/rules/subagent-model-inherit.mdc)。
- **用户级 framework 规则**：`framework host-integration install --to cursor --scope user` → `~/.cursor/rules/framework.mdc`。
