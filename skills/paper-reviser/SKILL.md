---
name: paper-reviser
description: |
  Execute the paper gate ledger one gate at a time. Default to sequential
  revision for requests like "根据 reviewer comments 修改", "按 review 改", or
  "修到能投"; if the user explicitly names one dimension or gate, revise only
  that gate. Choose an honest disposition before editing, such as repair,
  narrow, delete, move_to_appendix, or de_emphasize. Owns rebuttal and
  response-letter orchestration.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 根据审稿意见修改
  - 按这些意见帮我改
  - 按 review 改
  - 按审核单改
  - 按 gate 改
  - 按这个维度改
  - 修到能投
  - 根据 review 修改论文
  - 根据 reviewer comments 修改
  - reviewer comments revise
  - 该删就删
  - 藏到附录
  - 缩口径
  - 降 claim
  - 降级这个 claim
  - 别强调这个点
  - 只改 G8
  - 只改摘要标题引言结论
  - 只改图表维度
  - 只改文献维度
  - 写 rebuttal
  - paper revise
  - gate ledger
  - reviewer comments
metadata:
  version: "3.0.0"
  platforms: [codex]
  tags: [paper, manuscript, revise, gate-executor, rebuttal, response-letter, appendix-routing]
framework_roles:
  - planner
  - executor
  - verifier
framework_phase: 1
framework_contracts:
  emits_findings: true
  consumes_findings: true
  emits_execution_items: true
  consumes_execution_items: false
  emits_verification_results: true
risk: medium
source: local
---

# Paper Reviser

This skill owns gate-execution revision for academic papers. It does not
freestyle across the whole draft. It consumes the active paper gate ledger,
executes one gate at a time, and advances the review filesystem by creating the
next actionable gate round file.

It also respects the review automation contract:

- the gate ledger is the markdown source of truth
- any verification re-review should use the same fresh-isolated reviewer policy
- cross-turn state should travel through markdown docs rather than loose chat memory

## When to use

- The user wants the manuscript changed, not just reviewed
- The user says "根据 reviewer comments 修改", "按 review 改", "修到能投", or "按 gate 改"
- The user provides reviewer comments, a gate ledger, or a `paper_review_v<N>/` folder
- The revision spans logic, writing, figures, tables, notation, and layout under one active gate
- The user wants strategic narrowing such as "删 / 缩 / 藏附录 / 降口径 / 不主动强调"
- The user wants rebuttal / response-letter orchestration tied to actual edits

## Do not use

- The user wants the initial review gate chain built or only wants to know "能不能投" → use `$paper-reviewer`
- The user wants only one review dimension judged, without manuscript edits → use `$paper-reviewer`
- The task is only pure logic critique → use `$paper-logic`
- The task is only prose polish for already-fixed claims and evidence → use `$paper-writing`
- The task is only figure/table polish → use `$paper-visuals`

## Shared protocol

Use the shared contract in [`../PAPER_GATE_PROTOCOL.md`](../PAPER_GATE_PROTOCOL.md).

The active state is defined by:

- current `paper_review_v<N>/`
- active gate file
- frozen upstream gates
- current `target_contract`
- current `object_map`
- the current gate's `selected_decision` options

Respect `review_scope`:

- `full_chain` → execute the current active gate in sequence
- `single_gate` → execute only the explicitly requested gate

Respect transport and verification:

- `transport_contract = md_only`
- gate state comes from the markdown packet, not from remembered thread prose
- when a revision needs re-review, use a fresh isolated reviewer worker if available

## Routing defaults

Choose scope in this order:

1. If the user explicitly names one gate or one dimension to revise, use `single_gate`.
2. If the user asks to revise the paper generally, or says "根据 reviewer comments 修改" without naming one dimension, continue the sequential active gate in `full_chain`.
3. Do not expand an explicit one-gate edit request into multi-gate execution.

Typical wording that should still route here:

- "根据 reviewer comments 修改"
- "按这个 checklist 改"
- "这个弱 claim 该删就删"
- "把这部分藏到附录"
- "这个口径降下来"
- "别再硬修了，直接收口"

## Strategic narrowing boundary

This skill may strategically narrow the manuscript when that is the strongest
honest path to a better submission outcome.

Allowed by default:

- delete a non-core, low-leverage, weak claim when the surviving contribution does not depend on it
- narrow a claim to the highest honest supportable level
- move secondary negative detail, extra comparison, or boundary-case material into appendix or limitation framing
- de-emphasize material that drags the narrative while not being required for the surviving claim

Never allowed:

- hide evidence that directly conflicts with a claim the manuscript still keeps
- delete or move information required to understand the main method, main result, or main conclusion
- use appendix routing as a substitute for an honest claim downgrade
- let a surviving core claim remain unsupported after a `hide` move; if that happens, mark the gate blocked or downgrade it explicitly

## Gate-decision semantics

Gate-level decisions are coarse:

- `ideal`
- `hide`
- `abandon`
- `ideal_only`

Within a decision gate, this skill maps that gate-level decision to concrete
unit-level `Disposition` values:

- `repair`
- `narrow`
- `delete`
- `move_to_appendix`
- `de_emphasize`
- `disclose_as_limitation`
- `block`

Default autonomy:

- if the best honest path is `delete`, `narrow`, `move_to_appendix`, or
  `de_emphasize`, execute it directly unless the user explicitly restricted that
  authority

## Required workflow

### Phase 0: Gate intake

1. Resolve the manuscript workspace root and the active `paper_review_v<N>/`.
2. Load the active gate file. If no gate ledger exists yet, derive it first with `$paper-reviewer`.
3. Read the current gate's:
   - `Goal`
   - `Frozen Inputs`
   - `Review Objects`
   - `Hard Bar`
   - `Decision Slot`
   - `Backjump Rule`
   - `Pass Line`
4. Do not advance to the next gate until the current gate passes, unless the user explicitly requested a one-gate revision slice.

### Phase 1: Current-gate execution

5. Determine the gate kind:
   - `G0` setup → satisfy or fail the target contract bootstrap
   - `G1-G6` decision → choose `ideal`, `hide`, or `abandon`
   - `G7-G14` quality → only `ideal_only`
6. For decision gates, assign a per-unit `Disposition` before editing:
   - `repair`
   - `narrow`
   - `delete`
   - `move_to_appendix`
   - `de_emphasize`
   - `disclose_as_limitation`
   - `block`
7. Use the current gate's primary owner routing:
   - evidence / claims → `$paper-logic`
   - formal closure → `$math-derivation`
   - citations → `$citation-management`
   - prose / flow → `$paper-writing`
   - notation → `$paper-notation-audit`
   - figures / tables → `$paper-visuals`
   - rendered layout → `$pdf`
8. Execute only the active gate's scope plus required mirror cleanup.
9. If the current gate requires re-review after edits, pass only the markdown
   packet back to a fresh isolated reviewer worker; do not rely on the prior
   reviser chat state to judge closure.

### Phase 2: Mirror propagation and integrity

10. If a claim is deleted, narrowed, moved to appendix, or de-emphasized, update
   every mirrored mention in:
   - abstract
   - introduction framing
   - method framing
   - experiment claim sentences
   - conclusion
   - captions
   - rebuttal / response text
11. After every meaningful edit batch, verify:
   - the edit actually applied
   - no orphaned text or duplicate section headers were introduced
   - no dangling claim or callout survived the chosen `Disposition`
   - main-text support still matches the surviving claim

### Phase 3: Pass, fail, or backjump

12. Check the current gate against its `Pass Line`.
13. If the gate passes:
   - freeze it
   - create the next gate's `r1` checklist file
14. If the gate does not pass:
   - create the same gate's next round file
15. If a quality gate discovers an upstream contradiction:
   - set `backjump_gate_on_regression`
   - do not invent a new `hide` or `abandon`
   - create the earlier gate's next round file

### Phase 4: Rebuttal / response letter

16. When reviewer-response prose is needed, structure it by reviewer comment but
   keep it downstream of the gate decisions.
17. Response tone must stay respectful, specific, evidence-linked, and
   non-defensive.
18. If a suggestion is declined, explain it by gate logic and manuscript change,
   not by vague preference.

## Output defaults

### Gate execution record

| Gate | Unit | Exec ID | Gate Decision | Disposition | Action Taken | Owner Skill | Verification | Remaining Risk | Status |
|---|---|---|---|---|---|---|---|---|---|
| G3 | claim:C2 | EXEC-01 | ideal/hide/abandon/ideal_only | repair/narrow/delete/move_to_appendix/de_emphasize/disclose_as_limitation/block | ... | ... | ... | ... | resolved/partial/blocked |

When useful, add one short note under an entry stating whether the fix used
`existing evidence reorganized` or `new evidence created`.

### Rebuttal / response letter

```markdown
## Reviewer #N

> [Reviewer comment quoted]

**Response**: [Action taken and rationale]
- Gate: [which gate decision drove this response]
- Changed: [specific location and edit summary]
- Evidence: [what supports this change]
```

## Hard constraints

- Do not edit a later gate while the current gate is still failing.
- Do not silently expand an explicit one-gate revision request into multi-gate execution.
- Do not pass revision state through free-form chat when the markdown packet can carry it.
- Do not let re-review reuse a stale long-lived reviewer context when a fresh isolated worker is possible.
- Quality gates may not invent a new `hide` or `abandon`; they must backjump.
- Do not fabricate experiments, numbers, citations, or proofs.
- Do not treat cosmetic edits as closure of scientific issues.
- Do not hide evidence that still matters to a surviving core claim.
- When the chosen disposition changes a claim surface, update every mirrored mention rather than leaving a hanging claim elsewhere.
- Verify rendered artifacts when layout or visual quality is part of the gate.
- **No Structural Duplication**: Ensure that no `\section`, `\subsection`, or `\paragraph` name is duplicated by the revision.
