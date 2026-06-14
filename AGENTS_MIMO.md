# MIMO Agent Policy

跨宿主协议见 [`AGENTS.md`](AGENTS.md)。**双文件注入**：须与 `AGENTS.md` 同时生效，勿单独使用本文件。本文仅 **MIMO**（`mimo`）transport delta。

## Transport 要点

- **框架命令流**：无 `AG_FOLLOWUP` / `updateCurrentStep`；续跑 `framework_goal_drive` + `artifacts/current/<task_id>/` 手动画板。
- **my-light**：suppress spawn-first 与 review Stop nudge（skill 层 findings-only 仍适用）。