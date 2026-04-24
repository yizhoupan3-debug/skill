# Autopilot Mode

`$autopilot` is an explicit alias mode of `execution-controller-coding`, not a
separate owner.

Workflow:

1. Expansion: restate the bounded task and success criteria.
2. Planning: choose the smallest executable path and verification route.
3. Execution: make real changes; keep clear, reversible steps moving.
4. QA: build, test, inspect, and fix until the signal is stable.
5. Validation: review the result against the original acceptance criteria.
6. Cleanup: leave evidence and recovery anchors; remove run-only residue.

Rules:

- Clarify only when the implementation would materially change.
- Reroute unknown root cause to `systematic-debugging`.
- Add `execution-audit` for strong acceptance.
- Do not announce completion without verification evidence.
