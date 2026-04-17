---
name: subagent-delegation
description: |
  Decide at 每轮对话开始 / first-turn whether a complex Codex task should split into bounded sidecars.
  Preserve the same structure in local-supervisor mode when runtime policy blocks spawning.
  Build the delegation plan first, attempt spawning second, and fall back only if runtime blocks it.
  Use for multi-phase or parallelizable work with compressed main-thread reporting; not for tiny, vague, or tightly coupled tasks.
routing_layer: L0
routing_owner: gate
routing_gate: delegation
routing_priority: P1
session_start: required
short_description: Decide whether to split a complex task across sidecars or preserve the same structure locally
trigger_phrases:
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
approval_required_tools: []
filesystem_scope:
  - repo
  - .supervisor_state.json
  - artifacts
network_access: conditional
artifact_outputs:
  - SESSION_SUMMARY.md
  - NEXT_ACTIONS.json
bridge_behavior: mobile_complete_once
---
- **Dual-Dimension Audit (Pre: Complexity-Rubric/Logic, Post: Integration-Fidelity/Trace Results)** → `$execution-audit-codex` [Overlay]

# subagent-delegation

This skill owns the **runtime delegation decision** inside the current Codex session. It decides whether a task should split into bounded sidecars, what must stay local, and how to preserve orchestration structure even when the current runtime policy does not permit spawning subagents.

## When to use

- Repository or conversation rules allow or may allow proactive delegation
- The task is complex, multi-phase, or has real parallel sidecars
- The user says to decide automatically when to use subagents or sidecars
- Search, audit, implementation, and verification can run in bounded parallel slices
- The main thread should stay short while execution detail is sunk into sidecar outputs or local-supervisor queues

## Do not use

- The task is tiny, tightly coupled, or vague
- The immediate blocker is faster to do locally
- Multiple workers would need overlapping write scopes
- The request is about multi-agent product architecture rather than runtime delegation

## Primary operating principle

This gate should behave like a **delegation controller**, not a yes/no spawn switch:

1. decide delegation structure before checking whether spawning is currently possible
2. prefer bounded sidecars when they materially improve throughput
3. keep critical-path decisions local
4. compress the main thread into decision summaries rather than process narration
5. if runtime policy forbids subagent spawning, preserve the same sidecar structure in local-supervisor mode
6. treat “cannot spawn now” as a runtime constraint, not as proof that delegation thinking was wrong

## Main-thread compression contract

The main thread should contain only:

- whether a delegation plan was created
- whether delegation is justified
- what stays local vs what becomes sidecars
- why the current runtime policy changes or does not change execution mode
- whether the controller continues locally or waits

Detailed task prompts, evidence payloads, and worker traces should live in artifacts, sidecar outputs, or local-supervisor notes.

## Runtime-policy adaptation

Once the complexity rubric passes, create the bounded sidecar plan first.

If runtime policy permits spawning:

- attempt real spawning from that pre-built plan
- use bounded sidecars where the critical path benefits
- keep orchestration, synthesis, and final judgment local

If runtime policy does **not** permit spawning:

- switch to **local-supervisor delegation mode**
- preserve the same pre-built sidecar boundaries and output contracts
- execute those slices sequentially or in a queued local form
- keep the same compressed main-thread reporting style

The inability to spawn subagents is a runtime constraint, not a reason to abandon delegation structure.

## Complexity rubric

Delegate or preserve sidecar structure when at least two are true, or one is true at high intensity:

1. multi-phase work
2. multi-surface scope
3. clear parallel sidecars exist
4. clear read/write boundaries exist
5. non-blocking search burden is high
6. validation burden is non-trivial

## Required workflow

1. Normalize the task and decide the **main-thread next step first**.
2. Identify what must stay local on the critical path.
3. Define bounded sidecars with explicit output contracts.
4. Record the delegation plan in state or artifacts before any runtime branch.
5. Check runtime policy and attempt spawning when the plan is valid.
6. If spawning is allowed, dispatch sidecars with non-overlapping write scopes.
7. If spawning is not allowed, keep the same sidecar plan as a local-supervisor queue.
8. Review and integrate returned or queued outputs before treating the task as done.

## Output defaults

State:

- whether a delegation plan was created
- whether spawning was attempted
- whether delegation structure was used
- whether actual spawning happened or local-supervisor fallback was used
- why spawning was blocked when it did not happen
- which fallback mode was used
- what was delegated vs kept local
- whether the main thread waited or continued

## References

- [references/delegation-recipes.md](references/delegation-recipes.md)
- [references/runtime-playbook.md](references/runtime-playbook.md)

## Trigger examples
- "强制进行子代理派发深度审计 / 检查派发逻辑与集成完整性。"
- "主线程尽量短，你来决定哪些切 sidecar。"
- "Use $execution-audit-codex to audit this delegation trace for integration-fidelity idealism."
