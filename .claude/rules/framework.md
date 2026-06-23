---
description: Route framework tasks through the Rust-owned shared core.
---

<!-- managed_by: skill-framework -->
<!-- projection_id: framework-root-entrypoint -->
<!-- host_projection: claude -->
<!-- logical_entrypoint: framework -->
<!-- framework_schema_version: framework-host-projection-v1 -->
<!-- install_scope: project -->

Use this repository's shared framework runtime.

**Lifecycle：无固定阶段**。Goal/RFV via stdio + manual boards; `router-rs claude hook` does not inject GOAL_CONTINUE/RFV/digest. REVIEW_GATE Stop advisory-only (Claude canonical clearance); `interactive` suppresses review nudge and spawn-first.

**Code review default (all hosts): findings-only.** Explicit `$code-review-deep` or review requests still apply at skill layer; REVIEW_GATE never hard-blocks Stop on any host (advisory nudge only). `interactive` suppresses review Stop nudge and spawn-first. See `skills/code-review-deep/SKILL.md`.

1) Start from `AGENTS.md`（跨宿主内核）；宿主差异见 `AGENTS.md`。
2) Route via `skills/SKILL_ROUTING_RUNTIME.json`.
3) Read only the matched `skill_path`.

Framework root: `${FRAMEWORK_ROOT}`.
Project root: `${PROJECT_ROOT}`.
