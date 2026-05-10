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
  - 精准修改
  - 局部润色
  - 不要动结构
  - 大面积重构
  - "edit_scope: surgical"
  - "edit_scope: refactor"
metadata:
  version: "2.13.5"
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

## Edit scope gate

Before rewriting, set **`edit_scope`** per
[`../paper-workbench/references/edit-scope-gate.md`](../paper-workbench/references/edit-scope-gate.md).

- Default **`surgical`**: only the user-confirmed spans; no cross-section
  throughline rewrites unless **`refactor`** was chosen.
- **`refactor`**: allowed only when the user (or `$paper-workbench`) has
  explicitly authorized structural / multi-section narrative work.

If scope is unclear, ask one question before producing text.

For **`surgical`**, follow the expanded precision contract (**hard fail on
out-of-scope edits**, forbidden stealth edits, **no full-manuscript/section dump**,
anchor-before-edit, per-item change cap, **patch/hunk-first delivery**, delivered
change ledger) in
[`../paper-workbench/references/edit-scope-gate.md`](../paper-workbench/references/edit-scope-gate.md).
Do not treat vague polish requests as permission to rewrite unstated spans.
Do not run a **read-through consistency pass** on abstract, intro, or conclusion
when those surfaces are not in **`scope_items`**.

## When to Use

- The user asks to draft or polish a bounded paper section.
- Claims, data, and evidence are already supplied or clearly constrained.
- The task is abstract, introduction, related work wording, discussion, caption,
  cover letter, rebuttal, or response letter prose.
- The user wants tone, flow, concision, hedging, or academic clarity.

## Do Not Use

- Whole-paper judgment, novelty, or experimental validity review -> use `$paper-reviewer` logic mode.
- Programmatic reviewer lists, Major/Minor point-by-point R&R, or meta-review checklists that require **manuscript / figure / appendix / reproducibility closures** -> keep the execution spine under `$paper-workbench` / `$paper-reviser` until point-to-point closure mapping exists; then use this skill only for bounded prose patches inside confirmed scope.
- Literature search/synthesis before writing -> keep the task in `$paper-workbench` until the source-backed story context is fixed.
- Citation formatting or verification -> use `$citation-management`.
- Generic non-academic prose -> use the owning domain skill, or `$documentation-engineering` for project docs.

## Research language norms (long-running)

Default on every manuscript pass unless the user explicitly waives it:

- Follow [`../paper-workbench/references/research-language-norms.md`](../paper-workbench/references/research-language-norms.md)
  for field-standard terminology, anti-coinage, repetition control, reader-facing
  tone (no internal / defensive stacking of `but` / `not` / `rather than`; no
  code names or raw `.csv`/path citations as stand-ins for results), and
  stable wording under **`surgical`**.
- If multi-round work already has `paper_story/TERMINOLOGY_GLOSSARY.md`, treat it
  as authoritative for preferred terms and forbidden aliases.

## Claim-Safety Rules

- Do not **wordsmith a weaker claim** into the manuscript to dodge a gap that
  the reviewer lane still classifies as closable with evidence/analysis; route
  back to `$paper-reviewer` / `$paper-reviser` per
  [`../paper-workbench/references/claim-evidence-ladder.md`](../paper-workbench/references/claim-evidence-ladder.md).
- For **response / rebuttal prose**: do not let polite acknowledgment or a
  longer limitation paragraph **stand in for** the manuscript edits or evidence
  work the comment requires; align with
  [`../paper-workbench/references/research-language-norms.md`](../paper-workbench/references/research-language-norms.md)
  §「修稿与审稿回应：不得以话术防御顶替实质修改」and the R&R section of the same
  claim–evidence ladder.
- **Code or math reviewer comments** are not "tone" tasks: the response text must
  point to **artifacts** (hashes/commands/proof appendices/errata) dictated by
  [`../paper-workbench/references/claim-evidence-ladder.md`](../paper-workbench/references/claim-evidence-ladder.md)
  §「代码/实现质疑」与 **§数学/推导质疑**，not vague release timelines or cautious rephrasing alone.
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
and evidence-led while staying inside the claim ledger.

**Cross-section work (`edit_scope` gate)**：下列「全稿叙事 / 多节对齐」步骤**仅当**
`edit_scope: refactor`，或 **`scope_items` 已列出**本轮会改写的全部相关表面（章节 /
小节 / 锚点）时，才允许执行。若在 **`surgical`** 下且未列出那些表面，则只做已锁定
范围内的局部改写；需要多节 mirror / throughline 时，明确提示用户**升格为
`refactor` 或补全 `scope_items`**，不要偷偷扩范围。

Within the allowed scope:

- lead with contribution, then evidence, then bounded implication
- keep wording reader-facing and science-facing, not process-facing
- keep tone confident within evidence and explicit at scope boundaries
- when multi-section scope is authorized: keep one manuscript throughline visible
  across abstract, introduction, results, discussion, and conclusion

For **authorized** multi-section rewrites, lock one canonical throughline (used by all
paper-writing references):

```text
core_problem -> bottleneck -> paper_move -> decisive_evidence -> bounded_implication
```

Alignment checks (only across sections that appear in **`scope_items`** or under **`refactor`**):

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
6. Mirror check (abstract / introduction / conclusion / captions)：仅当 **`edit_scope:
   refactor`**，或这些表面**全部**已列入 **`scope_items`** 时执行，确认没有表面在
   静默超过允许 claim level。若在 **`surgical`** 且未覆盖上述全部表面，则只对
   **已改写过的表面**做局部一致性检查，或提示升格 / 补全 scope 后再做全 mirror。
7. Return the polished text first; include notes only for important claim risks.

## Output Defaults

- For short passages: polished text only.
- For section rewrites: revised section plus concise rationale if useful.
- For rebuttals: point-by-point response with polite stance and no overpromise;
  each point should **point to a concrete manuscript/supplement change** or an
  explicit cannot-fix reason, not defense-only prose.
- For captions: self-contained caption with variables, cohort/data, and key takeaway.

## References

- [../paper-workbench/references/RESEARCH_PAPER_STACK.md](../paper-workbench/references/RESEARCH_PAPER_STACK.md)
- [../paper-workbench/references/research-language-norms.md](../paper-workbench/references/research-language-norms.md)
- [references/section-by-section.md](./references/section-by-section.md)
- [references/storytelling-patterns.md](./references/storytelling-patterns.md)
- [references/rebuttal-patterns.md](./references/rebuttal-patterns.md)
- [references/revision-playbook.md](./references/revision-playbook.md)
