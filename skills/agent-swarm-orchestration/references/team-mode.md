# Team Mode

Use this reference only for explicit `/team` entrypoints or strong full team-orchestration requests.

`agent-swarm-orchestration` owns the delegation decision. The `team` framework command is an alias/runtime controller, not an ordinary skill competing for first-turn ownership.

## Mode Contract

- Prefer local execution or bounded sidecars when the task is small, tightly coupled, or easy to integrate locally.
- Escalate to full team orchestration only when worker lifecycle, lane contracts, integration, QA, cleanup, or resume/recovery are first-class requirements.
- Keep supervisor-owned continuity in the Rust framework; do not create ad hoc plugin state.
- Let `plan-to-code` remain the implementation owner once the team plan resolves into concrete code slices.

## Phases

1. Scoping: define lanes, dependencies, artifacts, and integration owner.
2. Delegation: split only independent slices with disjoint write scopes.
3. Execution: keep workers bounded and evidence-producing.
4. Integration: review and merge outputs locally.
5. QA: run the relevant checks and resolve conflicts.
6. Cleanup: close workers, summarize evidence, and preserve continuity.
