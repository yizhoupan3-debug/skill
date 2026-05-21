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

**Default lifecycle: My** (same chain). Goal/RFV via `framework_goal_drive` / `framework_rfv_loop` stdio + `artifacts/current/<task_id>/` manual boards only; Codex hooks do not inject continuity digest, `GOAL_CONTINUE`, or `RFV_LOOP_CONTINUE`.

**Code review default (all hosts): findings-only.** Explicit `$code-review-deep` or review requests still apply; my-light profile does not hard-block Stop on REVIEW_GATE. See `skills/code-review-deep/SKILL.md`.

1) Start from `AGENTS.md`.
2) Route via `skills/SKILL_ROUTING_RUNTIME.json`.
3) Read only the matched `skill_path`.

Framework root: `${FRAMEWORK_ROOT}`.
Project root: `${PROJECT_ROOT}`.

$ARGUMENTS
