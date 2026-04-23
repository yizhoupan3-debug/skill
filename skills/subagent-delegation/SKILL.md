---
name: subagent-delegation
description: |
  First-turn gate for deciding whether a complex Codex task stays local,
  uses bounded subagents, or escalates into full team orchestration.
  Must make that decision at 每轮对话开始 / first-turn / conversation start.
routing_layer: L0
routing_owner: gate
routing_gate: delegation
routing_priority: P1
session_start: required
short_description: Decide whether a complex task should stay local, use bounded subagents, or preserve the same structure locally
trigger_hints:
  - 子代理派发
  - sidecar
  - 并行 sidecar
  - delegation
  - local-supervisor
framework_roles:
  - gate
  - runtime-router
framework_phase: delegation-decision
framework_contracts:
  emits_findings: true
  consumes_findings: true
  emits_execution_items: true
  consumes_execution_items: true
  emits_verification_results: false
metadata:
  version: "4.1.0"
  platforms: [codex]
  tags:
    - delegation
    - subagent
    - parallelism
    - codex
    - complex-task
    - runtime-delegation
    - local-supervisor
    - first-turn-routing
risk: medium
source: local
allowed_tools:
  - shell
  - python
approval_required_tools:
  - destructive shell
filesystem_scope:
  - repo
  - .supervisor_state.json
  - artifacts
network_access: conditional
artifact_outputs:
  - SESSION_SUMMARY.md
  - NEXT_ACTIONS.json
  - EVIDENCE_INDEX.json
  - TRACE_METADATA.json
bridge_behavior: mobile_complete_once
---
- **Dual-Dimension Audit (Pre: Complexity-Rubric/Logic, Post: Integration-Fidelity/Trace Results)** → `$execution-audit` [Overlay]

# subagent-delegation

This skill owns the **runtime multi-agent routing decision** inside the current Codex session. It decides whether a task should stay local, split into bounded subagents, or escalate into full team orchestration, while preserving the same execution structure even when the current runtime policy does not permit spawning.

## When to use

- Repository or conversation rules allow or may allow proactive delegation
- The task is complex, multi-phase, or has real parallel sidecars
- The user says to decide automatically when to stay local, use bounded subagents, or enter team mode
- Search, audit, implementation, and verification can run in bounded parallel slices
- The main thread should stay short while execution detail is sunk into sidecar outputs or local-supervisor queues

## Do not use

- The task is tiny, tightly coupled, or vague
- The immediate blocker is faster to do locally
- Multiple workers would need overlapping write scopes
- Multiple workers would need to co-edit shared continuity artifacts such as `SESSION_SUMMARY.md`, `NEXT_ACTIONS.json`, `EVIDENCE_INDEX.json`, `TRACE_METADATA.json`, or `.supervisor_state.json`
- The task already has a checklist / phase plan, but the main need is still to normalize serial/parallel lanes, scope, acceptance, or update rules before any sidecar split → use `$checklist-normalizer`
- The request is about multi-agent product architecture rather than runtime execution routing

## Primary operating principle

This gate should behave like a **multi-agent routing controller**, not a yes/no spawn switch:

1. decide delegation structure before checking whether spawning is currently possible
2. prefer bounded subagents when they materially improve throughput without orchestration overhead
3. escalate to `team` only when supervisor-led orchestration is part of the task
4. compress the main thread into decision summaries rather than process narration
5. if runtime policy forbids spawning, preserve the same chosen structure in local-supervisor mode
6. treat “cannot spawn now” as a runtime constraint, not as proof that delegation thinking was wrong
7. under a `gsd` posture, keep immediate blocker work local and delegate only true sidecars

## Main-thread compression contract

The main thread should contain only:

- whether a delegation plan was created
- whether delegation is justified
- what stays local vs what becomes sidecars
- why the current runtime policy changes or does not change execution mode
- whether the controller continues locally or waits

Detailed task prompts, evidence payloads, and worker traces should live in artifacts, sidecar outputs, or local-supervisor notes.
Shared continuity artifacts stay local to the supervisor / integrator. Bounded subagents and team workers should return lane-local outputs or delta payloads instead of mutating global continuity files.

## Runtime-policy adaptation

Once the complexity rubric passes, decide whether the task should remain local, use bounded subagents, or enter team orchestration.

If runtime policy permits spawning:

- attempt real spawning from that pre-built plan
- use bounded subagents where the critical path benefits
- keep orchestration, synthesis, and final judgment local unless the decision is `team`

If runtime policy does **not** permit spawning:

- switch to **local-supervisor delegation mode**
- preserve the same pre-built subagent or team boundaries and output contracts
- execute those slices sequentially or in a queued local form
- keep the same compressed main-thread reporting style

The inability to spawn workers is a runtime constraint, not a reason to abandon the chosen multi-agent structure.

## Complexity rubric

Escalate beyond local execution when at least two are true, or one is true at high intensity:

1. multi-phase work
2. multi-surface scope
3. clear bounded subagents or team lanes exist
4. clear read/write boundaries exist
5. non-blocking search burden is high
6. validation burden is non-trivial

## Required workflow

1. Normalize the task and decide the **main-thread next step first**.
2. Identify what must stay local on the critical path.
3. Define bounded subagents or team lanes with explicit output contracts.
4. Mark shared continuity artifacts as forbidden scope for every non-integrator lane.
5. Record the routing decision and delegation plan in state or artifacts before any runtime branch.
6. Check runtime policy and attempt spawning when the plan is valid.
7. If spawning is allowed, dispatch bounded subagents or team workers with non-overlapping write scopes and lane-local output paths.
8. If spawning is not allowed, keep the same lane plan as a local-supervisor queue.
9. Review and integrate returned or queued outputs before treating the task as done.

## Output defaults

State:

- whether a delegation plan was created
- whether spawning was attempted
- what routing decision was made: local / subagent / team
- whether delegation structure was used
- whether actual spawning happened or local-supervisor fallback was used
- why spawning was blocked when it did not happen
- which fallback mode was used
- what was delegated vs kept local
- whether shared continuity remained supervisor-only
- which lane-local outputs or delta artifacts were collected
- whether the main thread waited or continued

## References

- [references/delegation-recipes.md](references/delegation-recipes.md)
- [references/runtime-playbook.md](references/runtime-playbook.md)

## Trigger examples
- "强制进行子代理派发深度审计 / 检查派发逻辑与集成完整性。"
- "主线程尽量短，你来决定哪些切 sidecar。"
- "Use $execution-audit to audit this delegation trace for integration-fidelity idealism."
