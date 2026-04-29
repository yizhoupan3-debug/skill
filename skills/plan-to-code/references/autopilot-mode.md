# Autopilot Mode

Use this reference only when the user explicitly invokes `$autopilot` or `/autopilot`.

`plan-to-code` remains the canonical owner. The framework command alias is a host entrypoint and runtime state machine, not a competing ordinary skill.

## Mode Contract

- Resolve live state through `router-rs framework alias autopilot` when continuity or resume status matters.
- Route ambiguous work to `idea-to-plan` before implementation.
- Route unknown failures through `systematic-debugging` before changing code.
- Execute through bounded implementation slices, then run verification before closeout.
- Keep state, recovery, and resume anchored in repo-native Rust/continuity artifacts.

## Phases

1. Expansion: extract object, action, constraints, deliverable, and success criteria.
2. Planning: decide the smallest executable slice map.
3. Execution: implement concrete slices directly in the repo.
4. QA: inspect edge cases and integration fit.
5. Validation: run the relevant tests, build, route, or artifact checks.
6. Cleanup: report verification and leave no stale generated drift.
