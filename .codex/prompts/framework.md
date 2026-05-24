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

**Default lifecycle: My** (same chain). Goal/RFV via `framework_goal_drive` / `framework_rfv_loop` stdio + manual boards only; no continuity digest or GOAL_CONTINUE/RFV_LOOP_CONTINUE on hooks.

**Code review default (all hosts): findings-only.** Explicit `$code-review-deep` or review requests still apply at skill layer; under `my-light`, Cursor/Codex hooks do not hard-block Stop on REVIEW_GATE or inject spawn-first nudge. See `skills/code-review-deep/SKILL.md`.

1) Start from `AGENTS.md`（跨宿主内核）；宿主差异见 `AGENTS_CODEX.md`。
2) Route via `skills/SKILL_ROUTING_RUNTIME.json`.
3) Read only the matched `skill_path`.

Framework root: `${FRAMEWORK_ROOT}`.
Project root: `${PROJECT_ROOT}`.

$ARGUMENTS
