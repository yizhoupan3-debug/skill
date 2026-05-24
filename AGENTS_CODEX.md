# Codex Agent Policy

跨宿主协议见 [`AGENTS.md`](AGENTS.md)。本文仅 Codex 宿主差异与编译嵌入。

## 权威分层（改哪里才生效）

| 类别 | 权威落点 |
|------|----------|
| 跨宿主叙述性协议 | 仓库根 [`AGENTS.md`](AGENTS.md) |
| Codex 宿主差异 | **`AGENTS_CODEX.md`**（本文件） |
| Codex 策略快照 | 编译 embed（`AGENTS.md` + 本文件）；项目 sync 材料化 **`AGENTS_CODEX.md`** + `.codex/*` |
| skill 路由 | `skills/SKILL_ROUTING_RUNTIME.json` |
| 框架命令 / CLI | `configs/framework/RUNTIME_REGISTRY.json` |
| hook 行为 | `.codex/hooks.json` + `router-rs` |

**文档地图**：[`docs/harness_architecture.md`](docs/harness_architecture.md) · [`docs/host_adapter_contract.md`](docs/host_adapter_contract.md) · [`docs/hosts/codex-cli.md`](docs/hosts/codex-cli.md)

## Codex 构建快照与同步逻辑（策略 A）

`router-rs` 编译期嵌入 **`AGENTS.md` + 本文件**（见 `build_codex_agent_policy`）。改跨宿主策略 → 改 [`AGENTS.md`](AGENTS.md)；改 Codex 差异 → 改本文件；二者均需 rebuild + sync。

```bash
cargo build --release --manifest-path scripts/router-rs/Cargo.toml
cargo run --release --manifest-path scripts/router-rs/Cargo.toml -- framework sync-entrypoints --repo-root "$PWD"
cargo run --release --manifest-path scripts/router-rs/Cargo.toml -- framework maint install-codex-user-hooks --framework-root "$PWD"
```

### 核心物化与同步机制

- **材料化**：`framework sync-entrypoints` 材料化 **`AGENTS_CODEX.md`**、**`.codex/README.md`** 与项目 `.codex/*`（见 `.codex/host_entrypoints_sync_manifest.json`）；**不** overwrite 仓库根 [`AGENTS.md`](AGENTS.md)（跨宿主内核，人工维护）。勿仅复制本文件到 `~/.codex/AGENTS.md`（会丢失跨宿主内核）。
- **编译嵌入**：`cargo build --release` 将 `AGENTS.md` + `AGENTS_CODEX.md` 静态嵌入二进制；hook 运行期不读盘。用户级 Codex 策略对齐见宿主文档；勿单独 cp 宿主 delta。

## Root

- Codex：`CODEX_HOME`（默认 `~/.codex`）；仓库内优先 `skills/` 与 `skills/SKILL_ROUTING_RUNTIME.json`。

## Knowledge Hygiene

- 本文件是 Codex 地图；跨宿主正文在 [`AGENTS.md`](AGENTS.md)。
