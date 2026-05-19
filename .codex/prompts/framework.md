---
description: Route framework tasks through the Rust-owned shared core.
argument-hint: "[framework task...]"
---

<!-- managed_by: skill-framework -->
<!-- projection_id: framework-root-entrypoint -->
<!-- host_projection: codex-cli -->
<!-- logical_entrypoint: framework -->
<!-- framework_schema_version: framework-host-projection-v1 -->
<!-- install_scope: project -->

Use `$framework` semantics via the Rust-owned shared core.

**Default lifecycle (all supported hosts): GSD** (`/gsd-new-project` → `/gsd-plan-phase` → `/gsd-execute-phase` → `/gsd-verify-work` → `/gsd-discuss-phase` → `/gsd-ship`). See `skills/gsd/SKILL.md`. `/autopilot` remains opt-in legacy goal-style execution (`skills/autopilot/SKILL.md`).

1) Start from `AGENTS.md`.
2) Route via `skills/SKILL_ROUTING_RUNTIME.json`.
3) Read only the matched `skill_path`.

Framework root: `${FRAMEWORK_ROOT}`.
Project root: `${PROJECT_ROOT}`.

$ARGUMENTS
