# Antigravity 宿主专用策略

跨宿主协议见 [`AGENTS.md`](AGENTS.md)。本文仅 Antigravity **双宿主面** 差异。

## 双宿主（必读）

| 宿主 id | 面 | 手册 | 安装 |
|---------|-----|------|------|
| **`antigravity-cli`** | 终端 JSON **hooks**（`.antigravitycli/`） | [`docs/hosts/antigravity-cli.md`](docs/hosts/antigravity-cli.md) | `install --to antigravity-cli` |
| **`antigravity-app`** | Desktop / **MCP**（`.gemini/`） | [`docs/hosts/antigravity-app.md`](docs/hosts/antigravity-app.md) | `install --to antigravity-app` |
| `antigravity`（别名） | 等同 **app** | 同上 | deprecated，见 [`MIGRATION.md`](MIGRATION.md) |

## Language

- 跨宿主语言规范见 [`AGENTS.md`](AGENTS.md) § Language；Antigravity 宿主强制继承，不得豁免。

## 宿主集成与诊断

```bash
# CLI hooks
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  framework host-integration install --to antigravity-cli --repo-root "$PWD"

# App MCP
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  framework host-integration install --to antigravity-app --repo-root "$PWD"

cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework host-integration status
```

## 并行子代理与 review

- **`my-light`**：两宿主均关闭 hook 硬 `REVIEW_GATE`（CLI 仍可有 Codex 式门控 env）。
- **CLI**：大规模任务优先 `invoke_subagent`；review 证据 PostTool+Stop（见 `code-review-deep` Antigravity CLI 段）。
- **App**：无 shell hooks；review 靠 `review-lanes/*.md` + MCP Hard Block（非 my-light）。

## 连续性

- 真源：`artifacts/current/<task_id>/`；`framework_goal_drive` / `framework_rfv_loop` stdio。
- CLI 与 App **共用** L2 工件；传输不同。

## Knowledge Hygiene

- 索引：[`docs/hosts/antigravity.md`](docs/hosts/antigravity.md)；跨宿主正文 [`AGENTS.md`](AGENTS.md)。
