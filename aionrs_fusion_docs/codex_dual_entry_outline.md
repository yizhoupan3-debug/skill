# Codex 双入口兼容蓝图

> 状态：历史兼容视图。当前主线已经是 `cli_common_adapter` + Rust-owned contract truth。

## 当前路线

- framework core 是唯一真源。
- Codex Desktop 使用 `codex_desktop_adapter`。
- Codex headless 使用 `codex_cli_adapter`。
- CLI-family shared contract 使用 `cli_common_adapter`。
- `codex_common_adapter` 只是 Codex 命名兼容视图。
- `codex_desktop_host_adapter` 只是 retired compatibility alias。

## 不再走的路线

- 不继续把 `aionrs` / `AionUI` 作为未来主线宿主。
- 不让 `codexcli` 变成 framework controller。
- 不把 `upgrade_compatibility_matrix` 当主回归基线。
- 不恢复 Python artifact emitter 或 Python/Rust parity lane。

## 保留本文的原因

本文解释双入口从旧兼容叙事收口到 CLI-family shared contract 的历史原因。真正的新工作应进入根部 `rust_next_phase_checklist.md`。
