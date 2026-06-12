# Opencode Agent Policy

跨宿主协议见 [`AGENTS.md`](AGENTS.md)。**双文件注入**：须与 `AGENTS.md` 同时生效，勿单独使用本文件。本文仅 **OpenCode**（`opencode`）transport delta。手册 [`docs/hosts/opencode.md`](docs/hosts/opencode.md) · [`docs/spec.md`](docs/spec.md) §0.1。

## Transport 要点

- **插件 hook + MCP 双通道**：OpenCode 通过 JS/TS 插件系统提供 hook（`tool.execute.before`、`tool.execute.after`、`session.idle` 等），同时通过 `opencode.json` → MCP 提供框架工具。插件目录：`~/.config/opencode/plugins/` + `.opencode/plugins/`。
- **安装**：`framework host-integration install --to opencode --repo-root "$PWD"`。
- **Task 子代理**：`WAVE_STATE` 中 `execution_mode=parallel` 时应 spawn；输出 `artifacts/current/<task_id>/lane-notes/<lane_id>.md`。
- **Review / closeout**：清门 **Claude canonical**；Stop review **advisory-only**（MCP `ADVISORY`）；非 my-light 时 MCP 可对**未满足 closeout 证据** hard-block（与 review 分层）。`ROUTER_RS_CLOSEOUT_ENFORCEMENT` 见 [`docs/references/AGENTS_OPERATOR_SURFACE.md`](docs/references/AGENTS_OPERATOR_SURFACE.md)。
- **联网**：`browser-mcp` MCP 提供浏览器自动化（`host-integration install` 自动注册）。
