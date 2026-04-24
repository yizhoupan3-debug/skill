# Runtime Playbook

Use this reference only after `execution-controller-coding` has already been selected.

## Execution Loop

1. Restore or initialize `.supervisor_state.json`.
2. Normalize the execution contract:
   - goal
   - phase
   - scope / forbidden scope
   - acceptance criteria
   - evidence required
3. Check `$subagent-delegation`.
4. Build bounded sidecars or the equivalent local-supervisor queue.
   - assign exclusive scope and forbidden scope for every lane
   - reserve shared continuity artifacts for the active supervisor / integrator
   - define lane-local output or delta artifact paths before execution starts
5. Integrate slice-by-slice and checkpoint after each major merge.
6. Flush task-scoped `artifacts/current/<task_id>/SESSION_SUMMARY.md`, `NEXT_ACTIONS.json`, `EVIDENCE_INDEX.json`, and `TRACE_METADATA.json` before sign-off.

## Single-writer continuity rule

Treat these as supervisor-owned continuity artifacts:

- `.supervisor_state.json`
- `artifacts/current/<task_id>/SESSION_SUMMARY.md`
- `artifacts/current/<task_id>/NEXT_ACTIONS.json`
- `artifacts/current/<task_id>/EVIDENCE_INDEX.json`
- `artifacts/current/<task_id>/TRACE_METADATA.json`

Only the active supervisor / integrator may write them.

Parallel lanes should emit lane-local outputs such as:

- `artifacts/lanes/<lane-id>/summary.md`
- `artifacts/lanes/<lane-id>/evidence.json`
- `artifacts/lanes/<lane-id>/next_actions.delta.json`

At integration time:

1. review the lane-local outputs
2. merge accepted deltas into the supervisor state
3. flush task-scoped continuity artifacts once

## State Minimum

- `schema_version`
- `task_summary`
- `active_phase`
- `execution_contract`
- `delegation`
- `verification`
- `blockers`
- `next_actions`

## Default Reroutes

- Unknown root cause: `$systematic-debugging`
- Strategy still undefined: `$idea-to-plan`
- Sign-off / hostile audit dominant: `$execution-audit`
