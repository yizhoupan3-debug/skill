---
name: paper-writing
description: |
  Write, restructure, or polish bounded academic-paper prose after the
  claim/evidence boundary is known. Use for 论文写作, 英文论文润色, SCI润色,
  abstract/introduction/related-work/caption/rebuttal/cover-letter drafting,
  academic storytelling, paragraph flow, and "只改表达不改 claim".
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - 论文写作
  - 写论文
  - 论文润色
  - 英文论文润色
  - SCI润色
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
  version: "2.8.0"
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
- Literature search/synthesis before writing -> use `$literature-synthesis`.
- Citation formatting or verification -> use `$citation-management`.
- Generic non-academic prose -> use the owning domain skill, or `$documentation-engineering` for project docs.

## Claim-Safety Rules

- Do not add unsupported novelty, superiority, causality, or clinical/practical impact.
- Keep hedging aligned with evidence strength.
- Preserve methods, results, numbers, abbreviations, and citation intent.
- If the requested prose needs missing evidence, ask or mark the gap.
- Never fabricate references or reviewer commitments.

## Workflow

1. Identify section type, target audience, journal/register, and allowed claims.
2. Extract supplied facts, evidence, and constraints before rewriting.
3. Choose the section move: motivate, gap, method, result, implication, or response.
4. Rewrite for flow and precision while keeping claim ceiling intact.
5. Return the polished text first; include notes only for important claim risks.

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
