# Codex 双入口下一阶段兼容视图

> 状态：历史兼容视图。当前活跃执行清单见仓库根部 `rust_next_phase_checklist.md`。

## 当前结论

- 下一阶段不再围绕 `aionrs` / `AionUI` 做主线规划。
- 不再围绕 Python artifact emitter、Python/Rust parity tests、Python live fallback 做执行计划。
- Codex dual-entry 只作为 compatibility view 保留；canonical CLI-family baseline 是 `cli_common_adapter` + `cli_family_parity_snapshot`。
- `upgrade_compatibility_matrix` 只保留 secondary compatibility inventory / smoke 角色。

## 已完成的迁移口径

- `codex_desktop_adapter` 是正式 desktop adapter。
- `codex_cli_adapter` 是正式 headless adapter。
- `codex_desktop_host_adapter` 是 retired compatibility alias。
- `generic_host_adapter`、`aionrs_companion_adapter`、`aionui_host_adapter` 不进入 default peer set。
- route/search CLI、runtime contract、memory recall、host entrypoint sync、artifact writer、native integration 由 Rust surfaces 承担。

## 后续只允许做什么

- 维护 parity snapshot 与 shared contract 一致性。
- 在显式 compatibility lane 中说明 legacy alias 或 legacy inventory。
- 把真正的新工作放到 Rust-owned runtime/control-plane/host-integration 清单，而不是恢复本文件的旧波次编号。
