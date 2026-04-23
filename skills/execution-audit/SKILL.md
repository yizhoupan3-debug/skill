---
name: execution-audit
description: |
  Strict implementation-level audit overlay: enforce spec fidelity, robustness,
  runtime evidence, and result idealism with automated verification, compressed main-thread reporting,
  and bounded sidecar evidence collection.
  Use for “强制验收 / 高质量闭环 / 100% 对齐检查 / 零容忍审计 / 最终 sign-off / 主线程只保留结论”.
routing_layer: L1
routing_owner: overlay
routing_gate: none
routing_priority: P1
session_start: n/a
short_description: Audit execution quality with evidence, sidecar-first collection, and compressed sign-off
trigger_hints:
  - execution-audit-codex
  - execution-audit
  - 强制验收
  - 高质量闭环
  - 100% 对齐检查
  - 零容忍审计
  - 最终 sign-off
  - 主线程只保留结论
  - audit
  - quality gate
  - robustness
  - verification
framework_roles:
  - verifier
  - quality-gate
framework_phase: post-implementation
framework_contracts:
  emits_findings: true
  consumes_findings: true
  emits_execution_items: true
  consumes_execution_items: true
  emits_verification_results: true
metadata:
  version: "2.2.0"
  platforms: [codex]
  tags:
    - execution-audit-codex
    - audit
    - quality-gate
    - robustness
    - verification
    - automation
    - subagent
    - sign-off
    - context-compression
risk: medium
source: local
allowed_tools:
  - shell
  - git
  - python
  - browser
approval_required_tools:
  - git push
  - gui automation
filesystem_scope:
  - repo
  - .execution_audit_state.json
  - artifacts
network_access: conditional
artifact_outputs:
  - audit_report.md
  - SESSION_SUMMARY.md
  - NEXT_ACTIONS.json
  - EVIDENCE_INDEX.json
bridge_behavior: mobile_complete_once
---

- **Dual-Dimension Audit (Pre: Spec-Integrity/Logic, Post: Result-Idealism/Runtime Evidence)** → `$execution-audit` [Overlay]

# execution-audit

This skill owns **strict execution auditing** for implementation work: prove that an implementation is not only present, but also fully wired, resilient, measurable, and worthy of final sign-off.

Use it as an **overlay**, not a primary builder. It should sit on top of `idea-to-plan`, `plan-to-code`, `checklist-fixer`, domain implementation skills, or [`$execution-controller-coding`](/Users/joe/Documents/skill/skills/execution-controller-coding/SKILL.md) slices.

## When to use

- “强制高质量执行 / 深度执行审计 / 最终验收 / 帮我做 sign-off”
- “检查是不是 100% 对齐需求 / 有没有漏接线 / 有没有隐藏风险”
- “这个计划是不是只是拆任务，还是已经形成高质量方案”
- The user has **zero tolerance** for placeholders, partial glue code, flaky behavior, or weak runtime evidence
- The implementation spans multiple surfaces such as **logic + API + storage + UI + tests**
- The task needs **automated verification**, artifact capture, and a rework queue instead of casual review comments
- The main thread should contain the final verdict and top findings, not full audit process detail

## Do not use

- High-level architecture critique → use [`$architect-review`](/Users/joe/Documents/skill/skills/architect-review/SKILL.md)
- Style-only or naming-only review → use [`$coding-standards`](/Users/joe/Documents/skill/skills/coding-standards/SKILL.md)
- PR triage or review scoring without strict sign-off semantics → use [`$code-review`](/Users/joe/Documents/skill/skills/code-review/SKILL.md)
- Root cause is still unknown and the real task is diagnosis → use [`$systematic-debugging`](/Users/joe/Documents/skill/skills/systematic-debugging/SKILL.md)
- The task is direct implementation rather than verification → use the relevant coding owner first

## Primary operating principle

This overlay should behave like a **master-control audit layer**:

1. collect evidence as automatically as possible
2. prefer sidecar evidence gathering when runtime policy permits
3. keep detailed evidence out of the main thread
4. reserve the main thread for verdict, severity, and rework scope
5. if subagent spawning is unavailable, keep the same audit structure in local-supervisor mode

## Main-thread compression contract

The main thread should contain only:

- audit scope
- verdict and confidence
- top blocker/risk findings
- rework scope
- sign-off or re-audit next step

Detailed logs, matrices, screenshots, and raw evidence should be stored in artifacts, state files, or compact evidence outputs.

## Overlay input contract

This overlay should receive or derive:

- spec or acceptance criteria
- touched modules or files
- runtime surface involved
- evidence already available
- verification commands
- prior findings if re-auditing

If these are missing, reconstruct them before issuing a verdict.

## Audit intent

This overlay enforces five questions:

1. **Spec fidelity** — was every requirement actually implemented?
2. **Integration completeness** — are all touched layers fully wired end to end?
3. **Robustness** — do edge cases, timeouts, retries, and bad states fail safely?
4. **Runtime evidence** — do tests, logs, browser state, and outputs prove correctness?
5. **Result idealism** — is the result merely functional, or genuinely shippable?

For planning-oriented audits on top of `idea-to-plan`, also enforce:

6. **Strategic clarity** — did the plan separate route selection from execution decomposition?
7. **Decision traceability** — are assumptions, open questions, and rejected routes explicit?
8. **Planning acceptance** — does the plan satisfy the [planning rubric](references/planning-rubric.md)?

## Audit surfaces

1. **Spec Alignment (L1)**
   - Requirement coverage
   - Acceptance criteria mapping
   - Missing glue / unregistered routes / dead branches
   - For planning slices: verify the plan includes goals, non-goals, compared routes, assumptions, open questions, and handoff boundaries
2. **Robustness Audit (L2)**
   - Null / empty / invalid input
   - Timeout / retry / cancellation
   - Race conditions / stale state / partial writes
3. **Efficiency Audit (L3)**
   - Obvious hotspots
   - Memory pressure / redundant work / unnecessary I/O
   - Platform-native execution quality
4. **Operational Audit (L4)**
   - Logs, metrics, traces, warnings, build/test output
   - Regression protection and failure visibility
5. **Superior Quality Audit (L5)**
   - Final pass against [references/superior-quality-bar.md](references/superior-quality-bar.md)
6. **Planning Rubric Audit (L5-P)**
   - When auditing a planning deliverable, run the checklist in [references/planning-rubric.md](references/planning-rubric.md)

## Runtime-policy adaptation

If runtime policy permits delegation:

- prefer sidecars for bounded evidence collection
- keep final synthesis and verdict local

If runtime policy does **not** permit subagent spawning:

- switch to **local-supervisor audit mode**
- run the same evidence slices sequentially
- keep detailed outputs in state/artifacts instead of inflating the conversation
- preserve verdict-first, compressed reporting style

The inability to spawn subagents is a runtime constraint, not a reason to abandon compressed audit orchestration.

## Automation lane

Default to an **automated audit pipeline** rather than manual spot checking.

### Required automation moves

1. Freeze the acceptance contract before reviewing.
2. Auto-collect the smallest sufficient evidence set:
   - tests
   - lint / typecheck / build
   - runtime logs
   - browser or rendered artifact evidence when relevant
3. Auto-generate a **finding matrix** grouped by blocker / risk / polish.
4. Auto-derive a **re-audit scope** so only changed surfaces are rechecked after fixes.
5. Auto-write or refresh `audit_report.md` when the audit is explicit or findings exist.

### Persistent audit state

Maintain `.execution_audit_state.json` when the audit is non-trivial.

Recommended minimum fields:

- target slice
- acceptance criteria
- evidence commands
- evidence collected
- open findings
- sidecar_outputs_or_local_queue
- checkpoint_on: [post-static-audit, post-runtime-audit, pre-signoff]
- pass_criteria
- rework_threshold
- final verdict

### Heartbeat and watchdog

For long audits, emit a compact heartbeat aligned with [`$execution-controller-coding`](/Users/joe/Documents/skill/skills/execution-controller-coding/SKILL.md).

Recommended default:

- `heartbeat_interval_minutes: 20`
- `watchdog_escalation: reroute_or_reaudit`

## Subagent policy

Use [`$subagent-delegation`](/Users/joe/Documents/skill/skills/subagent-delegation/SKILL.md) when the audit burden is large enough to split **evidence collection**, not the final judgment.

### Keep local

- Final PASS / FAIL / REWORK verdict
- Cross-surface synthesis
- Severity normalization
- Rework prioritization

### Good bounded sidecars

- Spec-to-code gap matrix for a bounded module
- Test/log collection for a bounded runtime slice
- Browser or screenshot evidence collection for a single UI flow
- Performance or security evidence gathering for one target path
- Artifact inspection for one deliverable surface

### Do not delegate

- The final sign-off decision
- A vague “check everything” request with no scope contract
- Overlapping write-scope fixes mixed into the audit task
- Urgent blocker analysis that is faster to do locally

### Sidecar contract

Each delegated sidecar should receive:

- target module or flow
- acceptance criteria subset
- required evidence type
- forbidden scope
- expected output format

Example outputs:

- `spec_gap_matrix.md`
- `runtime_evidence.md`
- `ui_verification.md`
- `perf_notes.md`

## Audit lifecycle

`Contract Freeze -> Static Audit -> Evidence Collection -> Sidecar or Local Queue -> Synthesis -> Verdict -> Rework Loop -> Re-Audit -> Sign-off`

## Rework loop

If the verdict is not passable:

1. emit blocker/risk list
2. derive the smallest valid rework scope
3. reroute fixes to the right owner
4. re-audit only changed surfaces unless the contract changed

## Output contract

Produce `audit_report.md` when the user explicitly asks for an audit artifact or when findings exist.

### audit_report_schema

- verdict
- scope
- confidence
- spec fidelity table
- evidence summary
- blocker / risk / polish findings
- rework queue
- sign-off note

```markdown
# Execution Audit Report

## 1. Verdict
- Status: PASS / REWORK / FAIL
- Scope:
- Confidence:

## 2. Spec Fidelity
- Requirement A: OK / Missing / Partial
- Requirement B: OK / Missing / Partial

## 3. Evidence
- Tests:
- Build / Typecheck:
- Runtime Logs:
- UI / Artifact Checks:

## 4. Findings
- [BLOCKER] ...
- [RISK] ...
- [POLISH] ...

## 5. Rework Queue
1. [file or module] ...
2. ...

## 6. Final Quality Note
- Why this is or is not ready for sign-off
```

## Hard constraints

- Do not accept placeholder code paths.
- Do not accept `TODO` without an issue or explicit deferral contract.
- Do not treat “works locally once” as sufficient evidence.
- Verify all touched layers are wired end to end.
- A 90% implementation is still **not** a pass.
- Do not dump full evidence into the main thread when artifacts or state can hold it.
- If the auditor itself is being changed, perform a self-check against [references/superior-quality-bar.md](references/superior-quality-bar.md).

## Trigger examples

- “强制进行审计自检 / 检查核心逻辑完整性与结果可信度。”
- “帮我做最终验收，给我 PASS/REWORK 结论。”
- “主线程只给我结论，细节放到报告里。”
- “Use $execution-audit to audit this execution trace for sign-off readiness.”
