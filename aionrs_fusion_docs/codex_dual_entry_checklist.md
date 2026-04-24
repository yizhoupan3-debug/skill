# Codex 双入口兼容清单

> 状态：历史兼容清单。当前活跃工作不再由本文件驱动。

## 已定结论

- `codex_desktop_adapter` 是 canonical desktop adapter。
- `codex_cli_adapter` 是 canonical Codex headless adapter。
- `cli_common_adapter` 是 CLI-family shared contract。
- `codex_desktop_host_adapter` 只允许作为 retired compatibility alias。
- `aionrs_companion_adapter`、`aionui_host_adapter`、`generic_host_adapter` 不进入 default peer set。

## 当前边界

- 主回归基线是 shared contract 与 parity snapshots。
- `upgrade_compatibility_matrix` 只保留 inventory / smoke 价值。
- 旧 `aionrs` / `AionUI` 叙事只能作为 legacy context，不再是路线图。
- 新实现工作必须走 Rust-owned runtime/control-plane/host-integration surface。
