# Claude Agent Policy

跨宿主协议见 [`AGENTS.md`](AGENTS.md)。**双文件注入**：须与 `AGENTS.md` 同时生效，勿单独使用本文件。本文仅 **Claude**（`claude`）transport delta；review 清门为跨宿主 **canonical** 参考实现。手册 [`docs/hosts/claude.md`](docs/hosts/claude.md) · [`docs/spec.md`](docs/spec.md) §0.1。

## PreToolUse 硬阻断（Claude 独有）

`.claude/settings.json` + `pre-tool-use` hook：未物化 `GOAL_STATE.json` 或未授权执行区 → **硬阻断**（`continue: false`）。遭遇阻断时 `/discussx` 或 `/planx` 自愈，勿盲目重试。`ROUTER_RS_CLAUDE_*` 仅作用于本宿主 hook。

## Transport 要点

- **Review gate（canonical）**：清门语义以 [`host_adapter_contract.md`](docs/spec.md) §0.1 为准；Stop **`REVIEW_GATE` advisory-only**（`router-rs CLAUDE_REVIEW_GATE …`）；**无** `rg_clear` 粘贴面（须完成可数 reviewer lane 或自然语言 override）。
- **框架命令流**：无 `AG_FOLLOWUP` / `updateCurrentStep`；续跑 `framework_goal_drive` + `artifacts/current/<task_id>/` 手动画板。
- **my-light**：suppress spawn-first 与 review Stop nudge（skill 层 findings-only 仍适用）。
