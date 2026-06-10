# Antigravity Agent Policy

跨宿主协议见 [`AGENTS.md`](AGENTS.md)。**双文件注入**：须与 `AGENTS.md` 同时生效，勿单独使用本文件。本文仅 **Antigravity**（`antigravity`）transport delta。产品面为 Desktop agent hub；本仓库经 `.gemini/` MCP 投影。手册 [`docs/hosts/antigravity.md`](docs/hosts/antigravity.md) · [`host_adapter_contract.md`](docs/host_adapter_contract.md) §0.1。

## Transport 要点

- **安装**：`framework host-integration install --to antigravity --repo-root "$PWD"`（canonical id `antigravity`；`antigravity-app` deprecated）。
- **MCP**：`router-rs-framework` · `browser-mcp` · `mcp-codegraph` · `paperplain` via `.gemini/mcp.json`；**无 shell hook**。
- **Review**：清门 **Claude canonical**；`review-lanes/*.md` + skill spawn-first；Stop review **advisory-only**（MCP `ADVISORY`）。
- **Closeout**：非 my-light 且 closeout 未满足时 MCP `goal_state_manage complete` / `closeout_gate` 可 hard-block（与 review 分层）。
- **连续性**：`framework_goal_drive` / `framework_rfv_loop` stdio + `artifacts/current/<task_id>/`。
