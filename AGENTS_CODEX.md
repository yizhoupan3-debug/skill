# Codex Agent Policy

跨宿主协议见 [`AGENTS.md`](AGENTS.md)。**双文件注入**：编译期嵌入 `AGENTS.md` + 本文件；勿单独使用本文件。本文仅 **Codex**（`codex`）transport delta。手册 [`docs/hosts/codex.md`](docs/hosts/codex.md) · [`host_adapter_contract.md`](docs/spec.md) §0.1。

## 策略嵌入与同步

`router-rs` 编译期 **`include_str!`** 嵌入 **`AGENTS.md` + 本文件**（`policy_embed.rs` → `codex_agent_policy`）；hook 运行期不读盘。`framework sync-entrypoints` 材料化 **本文件** + `.codex/*`；**不** overwrite 仓库根 [`AGENTS.md`](AGENTS.md)。

```bash
cargo build --release --manifest-path core/router-rs/Cargo.toml
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework sync-entrypoints --repo-root "$PWD"
```

勿将本文件 alone 复制到 `~/.codex/AGENTS.md`（会丢失跨宿主内核）。

## Transport 要点

- **Hook**：`.codex/hooks.json` + `router-rs codex hook`；清门 **Claude canonical**；Stop **advisory-only** `CODEX_REVIEW_GATE`；`subagent_start_count` **仅遥测**；`rg_clear` / reject token 粘贴面。
- **多代理**：`/implementx` 且 `execution_mode=parallel` 时应 spawn lane；深度 review spawn-first（`fork_context=false`）。细则 [`docs/hosts/codex.md`](docs/hosts/codex.md)、[`skills/implementx/SKILL.md`](skills/implementx/SKILL.md)。
- **stdio 替代 MCP 工具**：`framework_goal_drive` / `framework_rfv_loop`；证据 PostTool 追加。对照 [`docs/hosts/codex.md`](docs/hosts/codex.md)。
