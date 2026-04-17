# Execution Modes

Choose one mode before implementation begins.

## Fast Path

Use fast path when most of the following are true:

- one primary user flow or one tightly scoped backend/API change
- few touched modules and low coordination overhead
- local patterns already show how to implement it
- no separate rollout, migration, or staged execution is needed
- verification can be done with a small high-signal set of checks

Expected behavior:

- inspect the repo
- define a few implementation slices mentally or in short notes
- implement directly
- run spec review, quality review, and verification

Fast path should still avoid a single giant patch. Even here, work in small,
checkable slices.

## Structured Path

Use structured path when any of the following are true:

- multiple subsystems or layers must change together
- the brief spans several independent capabilities
- there is migration, rollout, or permissions risk
- delegation to subagents would materially help
- there is enough scope that losing the thread is a real risk
- the work benefits from explicit slice ordering and intermediate review

Expected behavior:

1. write a compact execution map
2. split work into execution slices
3. implement slice by slice
4. review each slice for spec compliance and code quality
5. run final verification on the integrated result

## Boundary Rule

If unsure, choose structured path.

The cost of one compact execution map is usually lower than the cost of
recovering from a large, poorly-ordered implementation burst.
