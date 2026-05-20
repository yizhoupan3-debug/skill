# Codex CLI 宿主操作手册

**权威能力矩阵**：`configs/framework/RUNTIME_REGISTRY.json` → `host_projections.codex-cli`  
**接入契约**：`docs/host_adapter_contract.md`  
**官方文档**：[Hooks](https://developers.openai.com/codex/hooks) · [AGENTS.md](https://developers.openai.com/codex/guides/agents-md)

## 安装 scope

| 组件 | Scope | 路径 |
|------|-------|------|
| `AGENTS.md`、`.codex/hooks.json` | **Project**（trusted project） | 仓库根 |
| Framework prompt 快照 | **Project** | `.codex/prompts/framework.md` |
| 全局 skill surface | **User** | `$CODEX_HOME/skills` → 仓库 `artifacts/codex-skill-surface/skills` |

## 何时跑 `codex sync` / `sync-entrypoints`

**要跑**：改了 `router-rs` 嵌入的 AGENTS 文本、Codex hook 模板、或需重材料化 `.codex/*` 入口。  
**不要跑**：仅为刷新 Cursor user `framework.mdc`（用 `host-integration install --to cursor --scope user`）。  
**不要**把 sync 当作「三端强行对齐」的默认日常步骤。

```bash
cargo run --manifest-path scripts/router-rs/Cargo.toml -- codex sync --repo-root "$PWD"
# 或
cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework sync-entrypoints --repo-root "$PWD"
```

磁盘上 **已存在** 的 `AGENTS.md` 不应被旧二进制 bootstrap 覆盖（官方近源优先）。

## 默认工作流

与全宿主相同：**GSD** 默认链；执行区 `/gsd-execute-phase` 启动 goal-style 连续执行（`/autopilot` 已退役）。

**续跑差异（重要）**：Codex **`Stop` 不注入** Cursor 式 `GSD_GOAL_CONTINUE`（`codex_hooks` 无该段落）。连续执行依赖：`GOAL_STATE.json` 盘上契约、Stop 自动 checkpoint（`ROUTER_RS_CONTINUITY_STOP_CHECKPOINT`）、以及你在下一条消息中继续 `/gsd-execute-phase` 或带 goal 上下文。对照 Cursor：见 [`framework_operator_primer.md`](../framework_operator_primer.md)「混用时的实际武装顺序」。

## Hook 能力

- 多层 hook **全部加载、并发**；项目 hook 需 trusted project
- PostTool / Stop / SessionStart → 证据与 continuity digest
- REVIEW_GATE 可数 lane：`review_gate.deep_gate_lanes`（与 Cursor 相同；**不含** Claude 的 `review`/`reviewer` 等）。单次 PostToolUse 证据，无 multiset

## 独有

- **`session_supervisor`** / tmux 长会话（见 `rust-session-supervisor` skill）
- `$CODEX_HOME/skills` 为 **27 pinned** surface（`skills/SKILL_ROUTING_RUNTIME.json` `hot_skill_count`），≠ 全量 **52** on-disk skills（`manifest_skill_count`）。仓库内投影目录 `artifacts/codex-skill-surface/skills` 须 `just publish` 或 `framework maint update-one-shot` 生成，克隆后可能为空。

## 自检

```bash
cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework maint verify-codex-hooks
cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework skills validate
```
