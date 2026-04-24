# Framework Profile Contract

## Purpose

`framework_profile` 是 Codex 框架真源。它只定义 runtime、memory、artifact、orchestration、approval、tool、loadout 和 workspace bootstrap 语义；宿主只能消费，不能反向改写核心含义。

## Canonical Fields

- `profile_id`
- `display_name`
- `framework_profile_version`
- `runtime_family`
- `host_family`
- `core_capabilities`
- `optional_capabilities`
- `rules_bundle`
- `skill_bundle`
- `session_policy`
- `tool_policy`
- `approval_policy`
- `loadout_policy`
- `framework_surface_policy`
- `artifact_contract`
- `model_policy`
- `memory_mounts`
- `mcp_servers`
- `workspace_bootstrap`
- `host_capability_requirements`
- `metadata`
- `execution_controller_contract`
- `delegation_contract`
- `supervisor_state_contract`

## Hard Rules

1. `core_capabilities` 必须覆盖 `runtime / memory / artifact / orchestration`，且 framework core 只允许 Codex 宿主。
2. `router-rs` 是 profile、shared contract、adapter artifact、workspace bootstrap、memory policy 和 session normalization 的编译真源；不要新增 Python bridge/helper parity、fallback emitter 或第二套默认值。
3. adapter 只能投影 `framework_profile`：`transport`、`context_files`、`mcp_config_paths`、`settings_paths` 等宿主私有字段只能留在 Codex adapter payload。
4. `workspace_bootstrap.bridges` 是唯一 bridge 默认来源；`bridge_contract` 只能从它投影，不能平行维护第二份 bridge 表。
5. continuity 真源是 task-scoped artifacts、`artifacts/current/active_task.json` 和 `.supervisor_state.json`；`artifacts/current/*` root 只能放 pointer、registry 或极薄兼容索引，不再复制整组恢复工件。

## Surface Policy

默认面只保留 `routing / memory / continuity / host_projection` 四轴；research、implementation、audit、framework、ops 和 compatibility 都必须显式 opt-in。机器可读真源在 `configs/framework/FRAMEWORK_SURFACE_POLICY.json`，并由 `skills/SKILL_LOADOUTS.json` 与 `skills/SKILL_TIERS.json` 支撑。

## History

历史迁移、retired Python surface、compatibility alias 和旧 adapter inventory 只放 audit/history 文档或显式 retirement/compatibility artifacts，不进入 steady-state contract。
