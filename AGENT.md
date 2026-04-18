# Shared Agent Policy

This repository is designed to be entered from `AGENTS.md` (Codex), `CLAUDE.md`
(Claude Code), or `GEMINI.md` (Gemini CLI). These files must project one shared
framework policy instead of forking per-host routing or memory rules.

## Default Behavior

- Reply in Chinese unless the user asks for another language.
- Keep answers direct and concise.
- Execute safe read/search/test/build commands directly when the runtime allows.
- Ask before destructive actions, external publishing, or account-impacting work.

## Turn-Start Routing

1. Extract `object / action / constraints / deliverable`.
2. Check gates before owners.
3. Use the narrowest matching skill.
4. Read the chosen `SKILL.md` before analysis, search, coding, or edits.
5. If no skill matches, consult `skills/SKILL_ROUTING_RUNTIME.json`, then
   `skills/SKILL_ROUTING_INDEX.md`.
6. Keep exactly one primary owner and at most one overlay.
7. For high-load, cross-file, or long-running tasks, invoke
   `execution-controller-coding` and maintain `.supervisor_state.json`.
8. For complex tasks, check `subagent-delegation` before deciding whether to
   split bounded sidecars.

## Shared Runtime Contract

- The shared runtime truth lives in `skills/`, task artifacts, and
  `.supervisor_state.json`.
- Host-specific entry files are thin projections only. They must not fork the
  routing truth, memory schema, or artifact contract.
- Complex tasks should externalize state into:
  `SESSION_SUMMARY.md`, `NEXT_ACTIONS.json`, `EVIDENCE_INDEX.json`,
  `TRACE_METADATA.json`, and `.supervisor_state.json`.
- Task-scoped continuity lives under `artifacts/current/<task_id>/`.
  Root-level continuity files and `artifacts/current/` are current-task mirrors
  or pointer surfaces only; they must not act as cross-task global truth.
  `artifacts/current/` may contain only `active_task.json`, the four mirror
  files, and task-scoped continuity directories. Bootstrap payloads belong in
  `artifacts/bootstrap/`, memory-automation diagnostics belong in
  `artifacts/ops/memory_automation/`, evidence belongs in `artifacts/evidence/`,
  and scratch or demo outputs belong in `artifacts/scratch/`.
- Shared continuity artifacts are a **single-writer surface**. Only the active
  supervisor / integrator may write `SESSION_SUMMARY.md`, `NEXT_ACTIONS.json`,
  `EVIDENCE_INDEX.json`, `TRACE_METADATA.json`, and `.supervisor_state.json`.
  Parallel lanes must emit lane-local summaries or delta artifacts and leave
  global continuity flushes to the integration step.
- Claude host hooks may refresh imported host projections, and `SessionEnd`
  may consolidate the project-local memory bundle, but they must not rewrite
  root continuity artifacts or take over supervisor integration.

## Memory Contract

- Long-term framework memory remains project-local at `./.codex/memory/`
  unless tooling explicitly switches roots.
- In this repository, `./.codex/memory/` is the logical framework path and
  currently resolves via symlink to `./memory/`; treat that as one shared root,
  not two independent memory trees.
- Default recall reads only the stable layer: `MEMORY.md`, `preferences.md`,
  `decisions.md`, `lessons.md`, `runbooks.md`, plus a freshness-gated active
  task summary only when the query clearly targets the current task.
- Historical/debug snapshots such as old session notes, legacy SQLite rows, and
  previous automation snapshots must live under `memory/archive/` or
  `artifacts/ops/memory_automation/`; they are not part of the normal prompt
  path.
- This path is shared framework state, not a Codex-only policy claim.
- Host entry files may reference framework memory, but must not redefine its
  schema or ownership.

## Workspace Binding

- When the user says `绑定xx目录` and the path is relative, resolve it under
  `/Users/joe/Documents`.
- Example: `绑定research/made` means `/Users/joe/Documents/research/made`.
- If the relative path does not exist there, ask for clarification instead of
  guessing across other roots.

## Runtime Sources Of Truth

- `skills/SKILL_ROUTING_RUNTIME.json`: machine-readable routing truth
- `skills/SKILL_ROUTING_INDEX.md`: human quick reference
- `skills/SKILL_ROUTING_LAYERS.md`: owner and reroute map
- `skills/SKILL_SOURCE_MANIFEST.json`: source precedence
- `skills/SKILL_SHADOW_MAP.json`: shadow audit
- `skills/SKILL_LOADOUTS.json`: loadout definitions
- `skills/SKILL_APPROVAL_POLICY.json`: approval policy registry

## Host Entry Files

- Codex: `AGENTS.md`
- Claude Code: `CLAUDE.md`, `.claude/CLAUDE.md`, `.claude/settings.json`
- Gemini CLI: `GEMINI.md`, `.gemini/settings.json`

These entry files must stay thin and point back to this shared policy.
