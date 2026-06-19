# Codex Agent Policy

跨宿主协议见 [`AGENTS.md`](AGENTS.md)。**双文件注入**：编译期嵌入 `AGENTS.md` + 本文件；勿单独使用本文件。本文仅 **Codex**（`codex`）transport delta。手册 [`docs/hosts/hook-hosts.md`](docs/hosts/hook-hosts.md) · [`docs/spec.md`](docs/spec.md) §0.1。

## 策略嵌入与同步

`router-rs` 编译期 **`include_str!`** 嵌入 **`AGENTS.md` + 本文件**（`policy_embed.rs` → `codex_agent_policy`）；hook 运行期不读盘。`framework sync-entrypoints` 材料化 **本文件** + `.codex/*`；**不** overwrite 仓库根 [`AGENTS.md`](AGENTS.md)。

```bash
cargo build --release --manifest-path core/router-rs/Cargo.toml
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework sync-entrypoints --repo-root "$PWD"
```

勿将本文件 alone 复制到 `~/.codex/AGENTS.md`（会丢失跨宿主内核）。

## Transport 要点

- **Hook**：`.codex/hooks.json` + `router-rs codex hook`；清门 **Claude canonical**；Stop **advisory-only** `CODEX_REVIEW_GATE`；`subagent_start_count` **仅遥测**；`rg_clear` / reject token 粘贴面。
- **多代理**：`/implementx` 且 `execution_mode=parallel` 时应 spawn lane；深度 review spawn-first（`fork_context=false`）。细则 [`docs/hosts/hook-hosts.md`](docs/hosts/hook-hosts.md)、[`skills/implementx/SKILL.md`](skills/implementx/SKILL.md)。
- **stdio 替代 MCP 工具**：`framework_goal_drive` / `framework_rfv_loop`；证据 PostTool 追加。对照 [`docs/hosts/hook-hosts.md`](docs/hosts/hook-hosts.md)。

## CodeGraph 自动触发（Codex 执行细则）

**跨宿主规则见 [`AGENTS.md`](AGENTS.md) § CodeGraph 自动触发规则**

Codex 宿主执行要点：
1. **自动识别**：从用户输入中识别触发词（重构、删除、跨模块等），自动调用对应codegraph工具
2. **无需询问**：直接调用工具，不询问用户是否要使用codegraph
3. **结果整合**：将工具结果整合到响应中，说明影响范围和风险
4. **强制执行**：无论是否触发特定技能，都必须执行自动触发规则

**示例场景**：
```
用户：帮我重构这个函数
Codex：（自动调用codegraph_impact分析影响范围）→ 根据结果制定重构计划
```
