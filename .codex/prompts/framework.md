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

**Default lifecycle: GSD** (same chain). Goal drive via `framework_autopilot_goal` stdio and continuity digest; **Codex hooks do not inject `GSD_GOAL_CONTINUE` on Stop** (see `docs/host_adapter_contract.md` §0.1).

**Code review default (all hosts): findings-only.** Review / 代码审查 / audit of code or a change set delivers severity-sorted findings only — no default edits, fixes, commits, or autopilot/GSD-execute/gitx/loop continuation unless the user explicitly asks to implement or fix. See `skills/code-review-deep/SKILL.md`.

1) Start from `AGENTS.md`.
2) Route via `skills/SKILL_ROUTING_RUNTIME.json`.
3) Read only the matched `skill_path`.

Framework root: `${FRAMEWORK_ROOT}`.
Project root: `${PROJECT_ROOT}`.

$ARGUMENTS
