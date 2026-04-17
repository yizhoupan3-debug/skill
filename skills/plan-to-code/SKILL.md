---
name: plan-to-code
description: |
  按 plan / spec / PRD 直接实现成代码；适合用户不要再规划、要直接落地开发的任务。
  Check this skill early at 每轮对话开始 / first-turn / conversation start for spec-driven execution.
routing_layer: L2
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: preferred
short_description: Implement a concrete plan or spec into integrated code
trigger_phrases:
  - 根据方案实现
  - 按文档开发
  - PRD 落地
  - spec-driven execution
  - 直接做代码
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
- "把这个 PRD 落成可运行代码 / 补齐实现 / 强制高质量代码"
- "接管这个半成品并完成它 / 把剩下的做完"
- The brief is mature enough (Level 2+) for implementation. See [references/input-maturity-levels.md](references/input-maturity-levels.md).

## Do not use

- The user only wants a plan or breakdown → use `$plan-writing`
- The task is debugging an unknown failure → use `$systematic-debugging`
- Fixing a numbered issue list item by item → use `$checklist-fixer`
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

- consult [`$subagent-delegation`](/Users/joe/Documents/skill/skills/subagent-delegation/SKILL.md) for bounded implementation or review sidecars
- keep integration and final completion judgment local

If runtime policy does **not** permit spawning:

- preserve the same execution slices in local-supervisor mode
- execute them sequentially while keeping the same output contracts
- do not inflate the main thread with raw implementation detail

## Core Workflow

1. **Parse & Classify**: Read the brief, classify maturity (Level 1-4), and choose mode (Fast vs Structured).
2. **Inspect & Map**: Locate entrypoints and map necessary changes (Storage -> Logic -> API -> UI).
3. **Implement & Reflect**: Edit files directly; wire all layers; perform **Self-Reflection** to identify bugs/edge cases before concluding implementation.
4. **Audit & Review**: Run spec-compliance checks. If **Reinforced Quality** is requested, trigger `$execution-audit-codex` as a mandatory overlay.
5. **Verify**: Run final builds/tests.

For complex tasks, refer to the [Detailed Implementation Workflow](references/workflow-depth.md).

## Execution Modes

- **Fast Path**: Narrow scope, small surface, low risk. Implement directly in checkable slices.
- **Structured Path**: Complex work, multiple subsystems, or delegation needed. Requires an explicit execution map.
See [references/execution-modes.md](references/execution-modes.md).

## Structured-path Delegation

When using subagents for execution slices or review sidecars:
- `plan-to-code` owns the brief interpretation and integration judgment.
- `$subagent-delegation` owns the sidecar strategy and runtime adaptation.
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
