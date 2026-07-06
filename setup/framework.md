---
description: Route framework tasks through the Rust-owned shared core.
---

<!-- managed_by: skill-framework -->
<!-- projection_id: framework-root-entrypoint -->
<!-- host_projection: claude -->
<!-- logical_entrypoint: framework -->
<!-- framework_schema_version: framework-host-projection-v1 -->
<!-- install_scope: user -->

Use this repository's shared framework runtime.

**Default lifecycle:** task — Goal/Quality Gate via stdio + manual boards; `router-rs claude hook` does not inject GOAL_CONTINUE/QUALITY_GATE/digest. REVIEW_GATE Stop advisory-only (Claude canonical clearance); `task` suppresses review nudge and spawn-first.

**Code review default (all hosts): findings-only.** Explicit code-review-deep or review requests still apply at skill layer; REVIEW_GATE never hard-blocks Stop on any host (advisory nudge only). `task` suppresses review Stop nudge and spawn-first. See `skills/code-review-deep/SKILL.md`.

1) Start from `AGENTS.md` (copy to `.claude/CLAUDE.md` or use the project CLAUDE.md).
2) Route via `skills/SKILL_ROUTING_RUNTIME.json`.
3) Read only the matched `skill_path`.

Framework root: `${FRAMEWORK_ROOT}` — set this to your repo root.
Project root: `${PROJECT_ROOT}` — also the repo root.
