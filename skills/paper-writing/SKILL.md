---
name: paper-writing
description: |
  Write, restructure, or polish bounded academic-paper prose after the
  claim/evidence boundary is known. Use for 论文写作, 英文论文润色, SCI润色,
  abstract/introduction/related-work/caption/rebuttal/cover-letter drafting,
  academic storytelling, paragraph flow, and "只改表达不改 claim". For
  顶刊/顶会/top-tier writing, use only after the contribution, evidence, and
  claim ceiling are fixed.
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: preferred
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - 论文写作
  - 写论文
  - 论文润色
  - 英文论文润色
  - SCI润色
  - 顶刊写作
  - 顶会写作
  - 顶刊顶会写作
  - top-tier academic writing
  - top conference writing
  - top journal writing
  - 学术润色
  - manuscript editing
  - academic writing
  - scientific writing
  - 写摘要
  - 帮我写 abstract
  - 写 introduction
  - 改 related work 文字
  - 改 caption
  - cover letter
  - response to reviewers
  - 只改表达不改 claim
  - 科研讲故事
  - 论文故事线
metadata:
  version: "2.9.0"
  platforms: [codex]
  tags: [paper, writing, rewrite, abstract, introduction, caption, rebuttal]
framework_roles:
  - executor
  - verifier
framework_phase: 1
framework_contracts:
  emits_findings: false
  consumes_findings: true
  emits_execution_items: true
  consumes_execution_items: false
  emits_verification_results: true
risk: low
source: local

---

# paper-writing

This skill owns reader-facing manuscript prose: section purpose, paragraph
flow, academic tone, claim calibration, and sentence clarity. It must not invent
science, citations, results, or reviewer-facing promises.

## When to Use

- The user asks to draft or polish a bounded paper section.
- Claims, data, and evidence are already supplied or clearly constrained.
- The task is abstract, introduction, related work wording, discussion, caption,
  cover letter, rebuttal, or response letter prose.
- The user wants tone, flow, concision, hedging, or academic clarity.

## Do Not Use

- Whole-paper judgment, novelty, or experimental validity review -> use `$paper-reviewer` logic mode.
- Literature search/synthesis before writing -> keep the task in `$paper-workbench` until the source-backed story context is fixed.
- Citation formatting or verification -> use `$citation-management`.
- Generic non-academic prose -> use the owning domain skill, or `$documentation-engineering` for project docs.

## Claim-Safety Rules

- Do not add unsupported novelty, superiority, causality, or clinical/practical impact.
- Keep hedging aligned with evidence strength.
- Preserve methods, results, numbers, abbreviations, and citation intent.
- If the requested prose needs missing evidence, ask or mark the gap.
- Never fabricate references or reviewer commitments.
- In multi-round editing, never "upgrade by wording" after claim ceiling is frozen.

## Multi-Round Claim Lock

When a manuscript is revised across turns, keep a visible claim lock:

1. Create or update a compact `claim_ledger` before rewriting:
   - claim_id
   - allowed_claim_level
   - required_evidence_anchor
   - forbidden_upgrade_terms
2. Treat the ledger as authoritative for wording choices.
3. If requested edits conflict with the ledger, stop and route to
   `$paper-reviewer`/`$paper-reviser` for a claim decision first.
4. Do not hide missing evidence in "future work" wording if the current sentence
   still implies support.

## Top-tier Writing Rules

Use [`../paper-workbench/references/top-tier-paper-standard.md`](../paper-workbench/references/top-tier-paper-standard.md)
as a guardrail when the user wants 顶刊/顶会/top-tier writing.

- Do not upgrade contribution language beyond the frozen claim ceiling.
- Make the one defensible contribution unmistakable before adding secondary angles.
- Shape abstract and introduction around the venue-calibrated gap, not generic importance.
- Keep closest-work contrast precise; do not imply novelty that the citation set has not established.
- Surface limitations where they protect the claim instead of weakening it.
- If top-tier readiness depends on missing evidence, stop and route back to
  `$paper-reviewer` or `$paper-reviser` rather than polishing around the gap.

## Top-tier Narrative Style

When the user asks for stronger writing style, keep the prose contribution-first
and evidence-led while staying inside the claim ledger:

- lead with contribution, then evidence, then bounded implication
- keep wording reader-facing and science-facing, not process-facing
- keep tone confident within evidence and explicit at scope boundaries
- keep one manuscript throughline visible across abstract, introduction, results,
  discussion, and conclusion

For multi-section rewrites, lock one canonical throughline (used by all
paper-writing references):

```text
core_problem -> bottleneck -> paper_move -> decisive_evidence -> bounded_implication
```

Alignment checks:

- every rewritten section advances the same core move
- no section introduces a competing headline contribution
- each section closes with a handoff to the next reader question

Canonical slot checks:

- core_problem: names the field-level need being served
- bottleneck: states the concrete blocker under scope
- paper_move: states what this paper changes
- decisive_evidence: points to the strongest proof unit (table/figure/theorem/analysis)
- bounded_implication: states why it matters without exceeding evidence

## Workflow

1. Identify section type, target audience, journal/register, and allowed claims.
2. Extract supplied facts, evidence, and constraints before rewriting.
3. For multi-round work, refresh `claim_ledger` and check proposed edits against it.
4. Choose the section move: motivate, gap, method, result, implication, or response.
5. Rewrite for flow and precision while keeping claim ceiling intact.
6. Run a mirror check on abstract/introduction/conclusion/captions to ensure no
   surface silently exceeds the allowed claim level.
7. Return the polished text first; include notes only for important claim risks.

## Output Defaults

- For short passages: polished text only.
- For section rewrites: revised section plus concise rationale if useful.
- For rebuttals: point-by-point response with polite stance and no overpromise.
- For captions: self-contained caption with variables, cohort/data, and key takeaway.

## References

- [references/section-by-section.md](./references/section-by-section.md)
- [references/storytelling-patterns.md](./references/storytelling-patterns.md)
- [references/rebuttal-patterns.md](./references/rebuttal-patterns.md)
- [references/revision-playbook.md](./references/revision-playbook.md)
