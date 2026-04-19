---
name: paper-reviewer
description: |
  Review a paper by abstract dimensions, not by sections. Default to the full
  `G0-G14` gate chain for requests like "帮我审这篇 paper", "投稿前把关", or
  "做整套 review"; if the user explicitly names one dimension such as claim,
  math, references, figures, tables, front-door text, or layout, review only
  that gate and write a non-overwriting gate checklist file. Hostile
  reject-reviewer pressure is explicit-only. Do not use for direct rewriting.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 帮我审这篇 paper
  - 帮我审这篇论文
  - 帮我 review 这篇 paper
  - 审这篇稿子
  - 能不能投
  - 值不值得投
  - 投稿前 review
  - 投稿前把关
  - 全文审核
  - 全流程审稿
  - 整套审核流程
  - 走完整 review 流程
  - 维度审核
  - 单点审核
  - 审核这个维度
  - 只审这个维度
  - 只看 G3
  - 只看这个 gate
  - 只审 claim
  - 只审 claim ceiling
  - 只审 contribution
  - 只审数学闭环
  - 只审公式推导
  - 只审文献维度
  - 只审引用
  - 只看 figure gate
  - 只审图
  - 只看 table gate
  - 只审表
  - 只审标题摘要引言结论
  - 只审正文附录分配
  - 只审符号一致性
  - 只审语言自然度
  - 只审 PDF 排版
  - 顶刊审稿
  - 顶刊级审稿
  - 最狠审稿人
  - 最严厉审稿
  - 最刁钻 reviewer
  - 对抗性找茬
  - 站在 reject reviewer 角度
metadata:
  version: "3.0.0"
  platforms: [codex]
  tags: [paper, manuscript, review, reviewer, gate-chain, benchmark-pool, adversarial, top-journal]
framework_roles:
  - detector
  - planner
framework_phase: 1
framework_contracts:
  emits_findings: true
  consumes_findings: false
  emits_execution_items: false
  consumes_execution_items: false
  emits_verification_results: false
risk: medium
source: local
---
- **Dual-Dimension Audit (Pre: Gate-Order/Review-Logic, Post: Gate-Fidelity/Actionable-Execution Results)** → `$execution-audit-codex` [Overlay]

# Paper Reviewer

This skill owns gate-first whole-paper review. It does not walk section by
section. It locks the target journal contract, builds a reusable benchmark pool,
maps the manuscript into abstract review objects, and advances a fixed gate
chain whose earlier conclusions become frozen for later gates.

This skill also owns the review-strength preservation contract:

- repeated reviews should not soften because the same agent keeps too much prior context
- each review pass should start from a fresh isolated reviewer worker
- cross-turn transfer should happen through markdown artifacts, not loose chat memory

## When to use

- The user wants a submission-facing paper review rather than a rewrite
- The user asks "能不能投", "投稿前把关", "帮我审这篇 paper", or "做整套 review"
- The user wants the paper reviewed by abstract dimensions rather than by sections
- The user wants a reusable review filesystem with `paper_ref/`, `paper_review_v<N>/`, and gate round files
- The user explicitly wants only one review dimension or one named gate audited, such as claim, math, citations, figures, tables, front-door text, notation, language, or layout
- The user asks for top-journal / hostile / reject-reviewer stress testing

## Do not use

- The user already has the gate ledger or reviewer comments and wants the paper changed now → use `$paper-reviser`
- The user says "帮我改", "根据 reviewer comments 修改", or otherwise wants execution rather than review → use `$paper-reviser`
- The task is only scientific defensibility or claim-vs-evidence logic without the full paper gate flow → use `$paper-logic`
- The task is only wording polish for fixed text blocks → use `$paper-writing`
- The task is only figure/table polish → use `$paper-visuals`
- The task is a student rubric or assignment-grade audit → use `$assignment-compliance`

## Artifact ownership

This skill operates in the manuscript workspace root, not in the skill repo.

It owns creation or continuation of:

- `paper_ref/`
- `paper_review_v<N>/`
- one new actionable gate file per turn

It does not overwrite prior gate files.

## Shared protocol

Use the shared paper gate contract in [`../PAPER_GATE_PROTOCOL.md`](../PAPER_GATE_PROTOCOL.md).

Treat the following as first-class shared state:

- `target_contract`
- `benchmark_ref_pool`
- `object_map`
- gate identity and freeze state
- claim ceiling fields
- appendix routing fields
- backjump targets

## Review posture

Default posture is strict gate-first reviewer:

- every gate uses the hardest honest standard
- the review order is fixed by dimension, not by paper section
- once a gate passes, later gates do not silently revise its conclusion
- the normal path does not emit a standalone reject-first paragraph

`Hostile` is an explicit overlay only for user requests such as "顶刊最狠审稿",
"reject reviewer", or "对抗性找茬".

`Hostile` changes the attack intensity, not the gate order:

- keep the same `G0-G14` chain
- sharpen the strongest reject sentence for the current gate
- output `Reject Case` only in hostile mode
- never let hostile mode become an excuse to skip the honest accept path

## Review isolation and transport

Every review pass should preserve reviewer harshness by default:

- `review_worker_mode = fresh_isolated_subagent`
- `context_isolation_policy = no_prior_review_context_except_md_packet`
- `transport_contract = md_only`

That means:

- a new gate review or re-review should be executed by a fresh reviewer worker
- the worker should receive only markdown packet state plus manuscript artifacts
- prior review chat should not leak into the next pass unless it is written into the markdown packet
- if runtime cannot actually spawn a subagent, simulate the same isolation by reloading only the markdown docs from disk and ignoring free-form prior narration

Default markdown packet:

- `paper_ref/TARGET_CONTRACT.md`
- latest `paper_ref/ref_pool_manifest_v<N>.md`
- active gate file
- upstream gate files listed in `Frozen Inputs`
- manuscript-path notes needed to locate the paper artifacts

## Routing defaults

Choose scope in this order:

1. If the user explicitly names one gate or dimension, use `single_gate`.
2. If the user asks for "全文审核", "整套审核流程", "帮我审这篇 paper", or any unspecified manuscript review, use `full_chain`.
3. Do not infer `single_gate` from vague wording. A vague review request stays `full_chain`.

Common dimension-to-gate shortcuts:

- claim / contribution / novelty / article fit -> `G3`
- math / theorem / derivation / formal rigor -> `G4`
- references / citations / related work support -> `G5`
- main text vs appendix / 正文附录分配 -> `G6`
- title / abstract / intro / conclusion -> `G8`
- terminology / notation / symbols / 缩写 -> `G10`
- figures / visuals / plots / charts -> `G11`
- tables / tabular results -> `G12`
- language naturalness / AI 味 / defense posture -> `G13`
- PDF layout / page economy / rendered floats -> `G14`

## Heartbeat wrapper

When the user wants the full review flow automated, this skill should expose an
internal wrapper contract:

- `automation_wrapper = heartbeat_5m_full_chain`
- `automation_tick_goal = advance_at_most_one_gate_round`

Each heartbeat tick should:

1. read only the markdown packet
2. resolve the active gate
3. launch a fresh isolated reviewer worker for that gate
4. create exactly one new non-overwriting gate markdown file
5. stop after that one bounded advancement

## Scope selection

Use one of two scope modes:

- `full_chain`: default when the user does not explicitly name a gate or review dimension
- `single_gate`: only when the user explicitly names one gate or dimension and wants that surface reviewed in isolation

Common aliases:

| Gate | Common user phrasing |
|---|---|
| `G0` | target contract, benchmark pool, 对标语料 |
| `G1` | fatal eligibility, integrity, disclosure, provenance |
| `G2` | core evidence, 主表主图, key ablation, strongest baseline |
| `G3` | claim ceiling, contribution level, article fit |
| `G4` | math closure, formal rigor, theorem, derivation |
| `G5` | references, citations, 文献维度, venue calibration |
| `G6` | appendix routing, 正文还是附录 |
| `G7` | narrative spine, main-text flow |
| `G8` | title / abstract / intro / conclusion, front-door text |
| `G9` | mirror consistency, 文本一致性 |
| `G10` | notation, terminology, symbols, 缩写 |
| `G11` | figure gate, 图, 可视化 |
| `G12` | table gate, 表格 |
| `G13` | language naturalness, defense posture, AI 味 |
| `G14` | rendered layout, page economy, PDF 排版 |

## Gate chain

| Gate | Kind | Primary review object | Main owner |
|---|---|---|---|
| `G0 Target Contract + Ref Bootstrap` | setup | target venue contract + local benchmark pool | `paper-reviewer` + `academic-search` |
| `G1 Fatal Eligibility` | decision | integrity, scope, disclosure, provenance | `paper-reviewer` |
| `G2 Core Evidence Freeze` | decision | core tables, figures, key numbers, key baselines, key ablations | `paper-reviewer` -> `paper-logic` |
| `G3 Claim Ceiling & Article Fit` | decision | claim ceiling, contribution bullets, novelty statement, article-type fit | `paper-reviewer` |
| `G4 Formal / Math Closure without Overmath` | decision | theorem, derivation, mechanism, proof-dependent claim | `paper-logic` -> `math-derivation` |
| `G5 Reference Support & Venue Calibration` | decision | citation clusters, claim-support refs, related-work calibration | `citation-management` |
| `G6 Main Text vs Appendix Routing` | decision | what stays in main text vs appendix vs deletion | `paper-reviewer` + `paper-reviser` |
| `G7 Narrative Spine & Main-text Flow` | quality | main-text order, pacing, burden distribution | `paper-writing` + `paper-length-tuner` |
| `G8 Front-door Text Gate` | quality | title, abstract, intro, conclusion | `paper-writing` |
| `G9 Mirror & Text Consistency` | quality | mirrored claim surfaces across the draft | `paper-reviser` + `paper-writing` |
| `G10 Terminology / Notation / Symbol Consistency` | quality | terminology, symbols, units, formula references | `paper-notation-audit` |
| `G11 Figure Gate at Final Scale` | quality | each surviving figure at real scale | `paper-visuals` + `visual-review` + `pdf` |
| `G12 Table Gate at Final Scale` | quality | each surviving table at real scale | `paper-visuals` + `visual-review` + `pdf` |
| `G13 Language Naturalness & Defense Posture` | quality | prose naturalness, smooth transitions, non-defensive tone | `paper-writing` |
| `G14 Rendered Layout & Page Economy` | quality | rendered PDF, floats, page flow, column economy | `pdf` + `visual-review` + `paper-length-tuner` |

## Required workflow

1. Resolve the manuscript workspace root.
2. Decide the scope mode:
   - `full_chain` if the user did not explicitly name one gate or dimension
   - `single_gate` if the user explicitly named one gate or dimension
3. Decide whether this is:
   - a new whole-paper review cycle → create the next `paper_review_v<N>`
   - a continuation of the active unfinished cycle → stay in the current `paper_review_v<N>`
4. In `full_chain`, run `G0` first:
   - lock `target_contract`
   - if the target venue or article contract is still missing, `G0` fails and later gates do not start
   - call `$academic-search` to create or refresh `paper_ref/`
5. In `single_gate`, do not silently backfill the rest of the chain:
   - resolve the requested gate from the explicit dimension name
   - load upstream frozen inputs if they already exist
   - if upstream gates are not actually frozen, record them as `assumed_frozen_inputs`
   - review only the requested gate for this turn
   - still use a fresh isolated reviewer worker and markdown-only transport
6. Build or refresh `object_map` using abstract review units such as claims,
   figures, tables, citation clusters, theorem units, front-door text blocks,
   notation sets, and layout surfaces.
7. Determine the current active gate:
   - in `full_chain`, the earliest gate not yet frozen as passed
   - in `single_gate`, the explicitly requested gate
8. Review the current gate only.
   - decision gates `G1-G6` may choose only `ideal`, `hide`, or `abandon`
   - quality gates `G7-G14` may choose only `ideal_only`
   - if a quality gate finds an upstream contradiction, it must set
     `backjump_gate_on_regression` and reopen the earlier gate instead of
     inventing a new `hide` or `abandon`
9. Record the gate judgment with the shared fields:
   - `review_scope`, `requested_gate_scope`, and `assumed_frozen_inputs` when relevant
   - `review_worker_mode`, `context_isolation_policy`, `transport_contract`, `transport_docs`
   - `automation_wrapper`, `automation_tick_goal` when autonomous mode is in scope
   - `gate_id`, `gate_kind`, `gate_order`
   - `unit_type`, `unit_id`
   - `anchor_evidence`
   - `selected_decision`
   - `claim_floor`, `claim_ceiling`, `selected_claim_level` when relevant
   - `math_closure_required`, `overmath_risk` when relevant
   - `appendix_routing` when relevant
   - `freeze_after_pass`
10. Create the next actionable gate file under `paper_review_v<N>/`:
   - if the current gate did not pass, create the same gate's next round file
   - if the current gate passed, create the next gate's `r1` file
   - if a backjump was triggered, create the earlier gate's next round file
   - never overwrite an old gate file
11. The gate file must follow the exact checklist template in
   [`../PAPER_GATE_PROTOCOL.md`](../PAPER_GATE_PROTOCOL.md):
   - `Goal`
   - `Frozen Inputs`
   - `Review Objects`
   - `Hard Bar`
   - `Checklist`
   - `Decision Slot`
   - `Backjump Rule`
   - `Pass Line`
   - `Next File If Pass`
   - `Next File If Fail`

## Gate-specific expectations

- `G0`: no target contract, no review. The benchmark pool must be target-journal
  first, local-PDF only, and reusable across later turns.
- `G1`: fatal integrity, scope, disclosure, or provenance problems are never
  downgraded into polish.
- `G2`: freeze the real evidentiary backbone before rhetoric changes it.
- `G3`: choose the highest honest claim ceiling that can still support the
  intended article; do not reflexively over-compress the claim.
- `G4`: call `$math-derivation` when formal closure matters; close the logic
  completely but remove math that the surviving claim does not need.
- `G5`: enforce real citations, venue calibration, and cluster discipline.
- `G6`: decide what belongs in main text, what belongs in appendix, and what
  should disappear entirely.
- `G7-G14`: these are polish-to-ideal gates only. They may tighten the surviving
  manuscript, but they may not silently renegotiate earlier decision gates.

## Output defaults

Primary output is `Gate Ledger Update`, not a flat severity-first issue list.

The output should summarize:

- review scope: `full_chain` or `single_gate`
- current `paper_review_v<N>` folder
- current gate reviewed
- selected decision and why
- frozen upstream gates
- any `assumed_frozen_inputs` used in single-gate mode
- whether `paper_ref/` was created, reused, or version-bumped
- whether a backjump was triggered
- the exact new gate file name created for the next turn

If internal findings are needed, treat them as anchor evidence inside the
current gate, not as the primary contract.

In hostile mode only, append a compact `Reject Case` paragraph after the gate
summary.

## Next step after review

- If the user wants execution, hand the active gate ledger to `$paper-reviser`.
- If the user wants only a single gate audited more deeply, stay within that gate
  and create the next round file for it.
- If the current blocker is just one narrow sub-surface, route that subproblem to
  the gate's narrower owner while keeping the gate ledger authoritative.

## Hard constraints

- Do not start reviewing before `G0` locks the target contract.
- Do not silently convert an explicit single-gate request into a full-chain review.
- Do not reuse one long-lived reviewer context across many review passes when a fresh isolated worker is possible.
- Do not pass state across rounds or ticks through chat memory when the markdown packet can carry it.
- Do not run the manuscript section by section as the primary order.
- Do not let downstream prose polish rewrite upstream evidentiary decisions.
- Do not default to over-lowering the claim ceiling in `G3`.
- Do not use `G4` to add decorative math.
- Do not call `paper_ref/` complete unless the retained items are actually local PDFs.
- Do not collapse the result into a generic issue list once the gate chain has been selected.
- **Superior Quality Audit**: For framework-faithful gate reviews, trigger `$execution-audit-codex` to verify critique against [Superior Quality Bar](../execution-audit-codex/references/superior-quality-bar.md).
