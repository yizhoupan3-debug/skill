# Team Mode

`$team` is an explicit alias mode that starts at `subagent-delegation`. The gate
decides whether true team orchestration is justified before any controller owns
execution.

Use team mode only when supervisor-led worker lifecycle, integration, QA,
cleanup, and resume/recovery are part of the task. Otherwise choose bounded
subagents or stay local.

Workflow:

1. Scope the task and identify the main-thread blocker.
2. Define lanes with non-overlapping write scopes.
3. Keep shared continuity supervisor-owned.
4. Collect lane-local outputs or deltas.
5. Integrate centrally.
6. Verify before cleanup.

If spawning is unavailable, preserve the same lane plan as a local-supervisor
queue rather than pretending no delegation structure exists.
