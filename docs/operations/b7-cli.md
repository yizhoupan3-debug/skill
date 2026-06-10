---
last_verified: "2026-06-09"
plate: B7
---

# B7 — CLI

## 职责

`router-rs` 命令行接入层：子命令解析、薄壳转发至 B3 stdio/runtime、B4 host-integration、B5 browser、B10 codegraph（feature）。目标：接入代码 < 1,500L（P7 已大幅下沉至 `framework_runtime/`）。

稳定安装路径：`~/.local/share/skill-framework/bin/router-rs`（可选；开发期多用 `cargo run`）。

## 启动 / 配置

```bash
# 常用入口
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework doctor
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework snapshot
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework alias <slug>
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework sync-entrypoints --repo-root "$PWD"

# 自安装（P0 router_self smoke）
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework self install
```

`ROUTER_RS_BIN`：hook 子进程查找二进制；缺失时 critical 事件 **fail-closed**。

Framework 斜杠命令真源：`RUNTIME_REGISTRY.json` → `framework_commands.*.host_entrypoints`。

## 排障

| 现象 | 处理 |
|------|------|
| 子命令不存在 / 行为变化 | `cargo test -p router-rs smoke_cli_backward_compat` |
| hook 找不到 binary | 设置 `ROUTER_RS_BIN` 或 `framework self install` |
| `runtime_ops.inc` 回归 | P7 后逻辑在 `stdio_dispatch.rs` / `live_execute.rs`；查对应测试 |
| 构建慢 | `CARGO_TARGET_DIR="$PWD/core/router-rs/target"` 固定 target 目录 |

## 相关路径

- `core/runtime-core/src/cli/`
- `core/runtime-core/src/framework_runtime/router_command_dispatch.rs`
- `docs/architecture/components.md` §框架维护命令
- `docs/framework_operator_primer.md` §自检
