---
last_verified: "2026-06-22"
depends_on:
  - spec.md
  - README.md
---

# Framework Profile Contract

## Purpose

文档索引：[`README.md`](README.md)（本目录）。

`framework_profile` 是 shared Rust core 的真源。它只定义 runtime、artifact、orchestration、approval、tool、loadout 和 workspace bootstrap 语义；Codex CLI 只能通过显式 host projection 消费，不能反向改写核心含义。

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
- `mcp_servers`
- `workspace_bootstrap`
- `host_capability_requirements`
- `metadata`
- `execution_protocol_contract`
- `execution_controller_contract` (debug contract view only)
- `delegation_contract`
- `supervisor_state_contract`（legacy optional；Codex `session_supervisor` 专用，非 My 手动画板续跑真源）

## Hard Rules

1. `core_capabilities` 必须覆盖 `runtime / artifact / orchestration`，且 framework core 只允许四宿主闭集 host projection：`codex`、`claude`、`cursor`、`opencode`。
2. `router-rs` 是 profile、shared contract、codex profile、workspace bootstrap 和 session normalization 的编译真源；不要新增第二套 helper、emitter 或默认值。
3. `codex_profile` 只能投影 `framework_profile`：`transport`、`context_files`、`mcp_config_paths`、`settings_paths` 等宿主私有字段只能留在 host projection payload 中。
4. `workspace_bootstrap.resources` 是唯一默认 skill resource 来源；不要平行维护第二份投影表。
5. continuity 真源是 task-scoped 手动画板（`artifacts/current/<task_id>/` 下的 `GOAL_STATE.json`、`RFV_LOOP_STATE.json`、`EVIDENCE_INDEX.json` 等）与 `artifacts/current/active_task.json` / `focus_task.json` 指针；`.supervisor_state.json` 仅为 legacy Codex supervisor 可选索引；`artifacts/current/*` root 只能放 pointer、registry 或极薄兼容索引，不再复制整组恢复工件。

## Surface Policy

默认面只保留 `routing / memory / continuity / host_projection` 四轴；research、implementation、audit、framework 和 ops 都必须显式 opt-in。机器可读真源在 `configs/framework/FRAMEWORK_SURFACE_POLICY.json`，`skills/SKILL_TIERS.json` 仅作为可再生成的调试报告，不参与热路由或宿主必需门禁。

相关控制面 JSON（非 profile 字段，但 install/门禁消费）：`configs/framework/RUNTIME_REGISTRY.json`（含 `review_gate`）、`configs/framework/host_projection_narrative.json`（My lifecycle / review 安装文案）、`configs/framework/GENERATED_ARTIFACTS.json`（生成物 drift manifest）。详见 [`spec.md`](spec.md)。

这里的“默认面”不是 `SKILL_ROUTING_RUNTIME.json` 的 hot routing index。默认面只统计 `session_start: required` 的 source/artifact/evidence gate；hot routing index 还会包含少量 `session_start: preferred` owner 和显式 `$` framework command alias，用于首轮路由发现。完整 specialist 只能通过 `SKILL_MANIFEST.json` fallback 被选中，不能被解释成默认加载或恢复旧入口。

## Execution Protocol

默认执行闭环是 `讨论 -> 规划 -> 执行 -> 验证`。它是 runtime / route 的协议，不是 skill owner，也不把 `execution-controller-coding` 设为默认主 owner。内部 route 字段可继续用 `four_step` 作为稳定机器标识。

`execution_protocol_contract` 只表达协议阶段、证据要求和 continuity 边界；`execution_controller_contract` 仅作为显式 debug contract view 保留，不能重新引入 `primary_owner: execution-controller-coding` 或 `gsd` owner boost 语义。

## History

历史迁移、旧 alias 和旧 profile inventory 只放 `MIGRATION.md` / git 历史，不进入 steady-state contract。
