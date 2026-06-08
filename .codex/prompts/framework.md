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

**Default lifecycle: My** — `/discussx` → `/planx` → `/implementx` → `/verifyx`. Goal/RFV via `framework_goal_drive` / `framework_rfv_loop` stdio + manual boards only; no continuity digest or GOAL_CONTINUE/RFV_LOOP_CONTINUE on hooks.

**Code review default (all hosts): findings-only.** Explicit `$code-review-deep` or review requests still apply at skill layer; REVIEW_GATE Stop is advisory-only on all hosts; `my-light` suppresses review Stop nudge and spawn-first. See `skills/code-review-deep/SKILL.md`.

**Language**: enforce 简体中文 per `AGENTS.md` § Language; no host-level exemption.

1) Start from `AGENTS.md`（跨宿主内核）；宿主差异见 `AGENTS_CODEX.md`。
2) Route via `skills/SKILL_ROUTING_RUNTIME.json`.
3) Read only the matched `skill_path`.

Framework root: `${FRAMEWORK_ROOT}`.
Project root: `${PROJECT_ROOT}`.

$ARGUMENTS
