# Codex 双入口 Rust 兼容清单

> 状态：历史兼容视图，已被 `rust_checklist.md` 与 `rust_next_phase_checklist.md` 取代。

## 当前结论

- `cli_common_adapter` 是 CLI-family shared contract 的中心。
- `codex_common_adapter` 只是 Codex 命名兼容视图。
- `codex_desktop_adapter` 是 canonical desktop identity。
- `codex_cli_adapter` 是 headless Codex entrypoint，不是 framework truth。
- `codex_desktop_host_adapter` 只保留为 retired compatibility alias，并且只允许显式 opt-in。
- `aionrs` / `AionUI` 不再是未来主线宿主。

## 已关闭的旧目标

- Rust 已能编译 profile bundle、adapter artifact、parity snapshot、capability discovery、control-plane contract。
- Python artifact emitter / Python-Rust parity report 叙事已退场。
- 旧 `scripts/route.py` 与 `framework_runtime/` Python runtime 包已删除。
- Desktop / CLI / Claude / Gemini 的 shared contract 现在通过 Rust host/profile surfaces 对齐。

## 保留价值

本文只用于解释为什么双入口不走 `codexcli` 主控化、不回到 `aionrs` / `AionUI`、也不把 compatibility matrix 当主基线。
