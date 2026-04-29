---
name: plan-to-code
description: |
  Repo-local spec-to-code implementation lane for concrete PRD/spec/plan artifacts and explicit
  `$autopilot` / `/autopilot` execution mode. Runtime owns ordinary coding execution; load this
  skill when the retained spec-to-code work-product contract or explicit alias matters.
routing_layer: L2
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
user-invocable: false
disable-model-invocation: true
short_description: Implement a concrete plan or spec into integrated code
trigger_hints:
  - PRD 落地
  - spec-driven execution
  - repo-local spec-to-code
  - plan-to-code
  - $autopilot
  - /autopilot
metadata:
  version: "3.1.0"
  platforms: [codex]
  tags: [implementation, spec-to-code, execution, delivery]
risk: medium
source: local
allowed_tools:
  - shell
  - git
  - python
  - node
approval_required_tools:
  - git push
filesystem_scope:
  - repo
  - artifacts
network_access: conditional
artifact_outputs:
  - SESSION_SUMMARY.md
  - NEXT_ACTIONS.json
  - EVIDENCE_INDEX.json
bridge_behavior: mobile_complete_once

---

# Plan To Code

This skill owns spec-driven implementation: turning a concrete brief into real, integrated, and verified repository code. For mission-critical work, employ **Reinforced Quality** pathways.

## Priority routing rule

If the user's primary artifact is a plan, PRD, spec, or task breakdown **and** the requested deliverable is code, check this skill before generic coding advice.

## When to use

- "根据方案实现 / 按文档开发 / 转成代码"
- "不要只给思路，直接做 / 把思路落成代码 / 极致执行"
- "$autopilot / /autopilot / 一路执行到底" when the user explicitly asks for the repo-native execution alias; use [references/autopilot-mode.md](references/autopilot-mode.md) for mode details.
- "把这个 PRD 落成可运行代码 / 补齐实现 / 强制高质量代码"
- "接管这个半成品并完成它 / 把剩下的做完"
- The brief is mature enough (Level 2+) for implementation. See [references/input-maturity-levels.md](references/input-maturity-levels.md).

## Do not use

- The user only wants a plan, checklist, or breakdown → use `runtime checklist planning`
- The task is debugging an unknown failure → use `$systematic-debugging`
- Fixing a numbered issue list item by item → use `runtime checklist execution`
- The source is still a messy checklist / phase blueprint and the main need is to stabilize serial/parallel boundaries, goals, constraints, acceptance, or update rules before coding → use `runtime checklist planning`
- The request is a review or summary of a document with no implementation intent.

## Primary operating principle

This owner should work as an **implementation node inside the master-control chain**:

1. route supporting work to the narrowest valid skills
2. prefer structured slices over one huge hidden implementation pass
3. keep the main thread to brief interpretation, integration judgment, and verification status
4. if runtime policy blocks subagent spawning, preserve the same slice map in local-supervisor mode
5. sink raw build/test/log detail into artifacts, state, or compact verification notes

## Main-thread compression contract

The main thread should contain only:

- what slice is being implemented
- why the current path was chosen
- integration status
- verification summary
- next blocking decision

## Runtime-policy adaptation

If runtime policy permits delegation:

- consult [`runtime delegation gate`](runtime policy) for bounded implementation or review sidecars
- keep integration and final completion judgment local

If runtime policy does **not** permit spawning:

- preserve the same execution slices in local-supervisor mode
- execute them sequentially while keeping the same output contracts
- do not inflate the main thread with raw implementation detail

## Core Workflow

1. **Parse & Classify**: Read the brief, classify maturity (Level 1-4), and choose mode (Fast vs Structured).
2. **Inspect & Map**: Locate entrypoints and map necessary changes (Storage -> Logic -> API -> UI).
3. **Implement & Reflect**: Edit files directly; wire all layers; perform **Self-Reflection** to identify bugs/edge cases before concluding implementation.
4. **Audit & Review**: Run spec-compliance checks. If **Reinforced Quality** is requested, apply the runtime verification gate as a mandatory overlay.
5. **Verify**: Run final builds/tests.

For complex tasks, refer to the [Detailed Implementation Workflow](references/workflow-depth.md).

## Execution Modes

- **Fast Path**: Narrow scope, small surface, low risk. Implement directly in checkable slices.
- **Structured Path**: Complex work, multiple subsystems, or delegation needed. Requires an explicit execution map.
See [references/execution-modes.md](references/execution-modes.md).

## Alias Modes

- `$autopilot` / `/autopilot`: explicit repo-native end-to-end execution mode. Keep `plan-to-code` as the canonical owner, use the Rust framework alias payload for live state, and follow [references/autopilot-mode.md](references/autopilot-mode.md).

## Structured-path Delegation

When using subagents for execution slices or review sidecars:
- `plan-to-code` owns the brief interpretation and integration judgment.
- `runtime delegation gate` owns the sidecar strategy and runtime adaptation.
- Use prompts from [references/delegation-prompts.md](references/delegation-prompts.md).

## Quality Bar

Before delivery, the implementation must meet these standards:
- **Spec Fidelity**: Every explicit requirement in the brief is addressed.
- **End-to-End Wiring**: The feature is reachable from its entrypoint (routes, exports, registrations).
- **No Placeholders**: No stub files, loose TODOs, or mock-only paths (unless requested).
- **Repository Fit**: Naming, patterns, and error handling align with existing code.
- **Observability**: Error paths are handled; logs or feedback exist for failure modes.
- **Verification**: At least one direct behavior check plus one static check (typecheck/build/lint) is reported.

## Blocking-question policy

Ask a blocking question only if:
- Conflicting business rules exist in the brief.
- Destructive data/auth choices cannot be inferred safely.
- Required external secrets/IDs are missing and undiscoverable.

Otherwise, proceed with minimal, reversible assumptions and state them in the delivery note.

## Trigger Examples

- "根据这个方案文档，把功能直接做出来，不要只给思路。"
- "把这个 PRD 变成可执行、功能齐全的代码。"
- "接手 `feature_plan.md`，直接在当前仓库完成实现。"
- "根据思路文件把剩下的半成品代码补齐。"
