---
last_verified: "2026-06-09"
---

# 安装、升级与多机同步

## 首次安装

```bash
git clone <repo-url> && cd skill

CARGO_TARGET_DIR="$PWD/core/router-rs/target" \
  cargo build --release --manifest-path core/router-rs/Cargo.toml

# 宿主 id 见 RUNTIME_REGISTRY.json → host_targets.supported
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  framework host-integration install --to <host_id> --repo-root "$PWD"

cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  framework doctor --repo-root "$PWD"
```

Claude Code 可选用 `./scripts/install-claude.sh`（等价 `install --to claude-code`）。

## 版本升级

```bash
git pull
CARGO_TARGET_DIR="$PWD/core/router-rs/target" \
  cargo build --release --manifest-path core/router-rs/Cargo.toml
# 对仍在使用的每个宿主重跑 install / sync（Codex: framework sync-entrypoints --host-id codex）
```

## 跨项目引导

```bash
export SKILL_FRAMEWORK_ROOT=/path/to/skill

./scripts/claude-bootstrap-framework.sh --framework-root "$SKILL_FRAMEWORK_ROOT"
./scripts/install-claude.sh --scope user

./scripts/cursor-bootstrap-framework.sh --framework-root "$SKILL_FRAMEWORK_ROOT"
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  framework host-integration install --to cursor --scope user
```

## Python 环境（macOS）

uv-only、Python 3.12：见 [`skills/python-env-management/SKILL.md`](../../skills/python-env-management/SKILL.md)。禁止全局 `pip`。

## Office CLI（可选）

```bash
bash scripts/install-pdf-tool.sh
bash scripts/install-ooxml-tool.sh
bash scripts/install-ppt-tool.sh
export PATH="$HOME/.local/bin:$PATH"
```

详见 [`../references/office-document-clis.md`](../references/office-document-clis.md)。

## 多机同步

| 类别 | 方式 |
|------|------|
| 仓库（含 `.claude/`、`.cursor/`、`.codex/` 等投影） | Git |
| 用户级 rules（`~/.claude/rules`、`~/.cursor/rules/framework.mdc`） | 每台机重跑 `install --scope user` |
| `router-rs` 稳定二进制 | **不**随 Git；每台机编译或 `framework self install` |

新机器：`git clone` → `cargo build` → 按需 `host-integration install` → `framework doctor`。

## 性能基准（hook）

```bash
./scripts/bench-hooks.sh
```
