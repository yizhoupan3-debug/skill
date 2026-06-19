# Opencode Agent Policy

跨宿主协议见 [`AGENTS.md`](AGENTS.md)。**双文件注入**：须与 `AGENTS.md` 同时生效，勿单独使用本文件。本文仅 **OpenCode**（`opencode`）transport delta。手册 [`docs/hosts/opencode.md`](docs/hosts/opencode.md) · [`docs/spec.md`](docs/spec.md) §0.1。

## Transport 要点

- **插件 hook + MCP 双通道**：OpenCode 通过 JS/TS 插件系统提供 hook（`tool.execute.before`、`tool.execute.after`、`session.idle` 等），同时通过 `opencode.json` → MCP 提供框架工具。详见 `docs/hosts/opencode.md` §架构。
- **Review / closeout**：清门 **Claude canonical**；Stop review **advisory-only**（MCP `ADVISORY`）；非 my-light 时 MCP 可对**未满足 closeout 证据** hard-block。
- **安装**：`framework host-integration install --to opencode --repo-root "$PWD"`。

## 架构差异

| 维度 | cursor/claude/codex | opencode |
|------|---------------------|----------|
| Hook 运行时 | Rust（`host-projection` crate） | JS/TS 插件系统 |
| Hook 事件 | `PreToolUse`/`UserPromptSubmit`/`PostToolUse`/`Stop` | `tool.execute.before/after`、`session.idle/created`、`permission.asked/replied` |
| Permission guard | Rust 侧 | `permission.asked` / `permission.replied` 插件事件 |
| 权限策略 | fail-closed | **fail-open** |

## 所有者对比

| 能力 | claude | cursor | codex | opencode |
|------|:-----------:|:------:|:-----:|:--------:|
| Hard gate hooks | ✓ | ✓ | ✓ | ✗（fail-open） |
| Closeout evidence hooks | ✓ | ✓ | ✓ | ✗ |
| Review gate | Rust 侧 | Rust 侧 | Rust 侧 | MCP 工具层 |
| Session supervisor | mcp_bridge | ✗ | codex_driver | ✗ |
| Provider trait | ✓ | ✓ | ✓ | ✓ |
