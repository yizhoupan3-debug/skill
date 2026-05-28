# Antigravity CLI 宿主操作手册

**闭集 id**：`antigravity-cli` · **传输**：JSON hooks（Gemini CLI 继任）· **配置根**：`.antigravitycli/`

## 能力

- **Hooks**：`SessionStart`、`PreToolUse`、`UserPromptSubmit`、`PostToolUse`、`Stop`（与 Codex CLI 同表；见 `artifacts/.../lane-notes/w0-hook-schema.md`）
- **Skills / Subagents**：官方 harness；用户技能路径见 w0 矩阵（`~/.gemini/antigravity-cli/skills/` 与 `.agents/skills/`）
- **门控**：`router-rs host antigravity-cli hook lifecycle-context`；REVIEW 为 Codex 式 PostTool+Stop（**非** Cursor `subagentStart`/`subagentStop`）
- **关闭 REVIEW 硬门**：`ROUTER_RS_ANTIGRAVITY_CLI_REVIEW_GATE_DISABLE=1`（与 Codex `ROUTER_RS_CODEX_REVIEW_GATE_DISABLE` 对称；`my-light` lifecycle 亦 suppress hook 硬拦）

## 安装

```bash
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  framework host-integration install --to antigravity-cli --repo-root "$PWD"
# 或
router-rs host antigravity-cli install-hooks --apply --repo-root "$PWD"
```

写入 `<repo>/.antigravitycli/hooks.json`（project）或 `$ANTIGRAVITY_CLI_HOME/hooks.json`（user）。

## 与 App 区分

| | CLI | App (`antigravity-app`) |
|---|-----|-------------------------|
| 手册 | 本文件 | [`antigravity-app.md`](antigravity-app.md) |
| 传输 | hooks | MCP + Planning Mode |
| 配置 | `.antigravitycli/` | `.gemini/` |

## 生命周期

与 Cursor/Codex 相同 My 链：`/discussx` → `/planx` → `/implementx` → `/verifyx`；`framework_goal_drive` stdio + `artifacts/current/<task_id>/`。
