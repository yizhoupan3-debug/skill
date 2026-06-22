---
description: Route framework tasks through the Rust-owned shared core.
argument-hint: "[framework task...]"
---

<!-- managed_by: skill-framework -->
<!-- projection_id: framework-root-entrypoint -->
<!-- host_projection: codex -->
<!-- logical_entrypoint: framework -->
<!-- framework_schema_version: framework-host-projection-v1 -->
<!-- install_scope: project -->

Use `$framework` semantics via the Rust-owned shared core.

**Default lifecycle: My** (same chain). Goal/Quality Gate via `framework_goal_drive` / `framework_quality_gate` stdio + manual boards only; no continuity digest or GOAL_CONTINUE/RFV_LOOP_CONTINUE on hooks. REVIEW_GATE Stop advisory-only; `my-light` suppresses review nudge and spawn-first.

**Code review default (all hosts): findings-only.** Explicit `$code-review-deep` or review requests still apply at skill layer; REVIEW_GATE never hard-blocks Stop on any host (advisory nudge only). `my-light` suppresses review Stop nudge and spawn-first. See `skills/code-review-deep/SKILL.md`.

1) Start from `AGENTS.md`（跨宿主内核，宿主差异见该文件内「宿主行为差异」节）。
2) Route via `skills/SKILL_ROUTING_RUNTIME.json`.
3) Read only the matched `skill_path`.

Framework root: `${FRAMEWORK_ROOT}`.
Project root: `${PROJECT_ROOT}`.

$ARGUMENTS
