---
name: paper-reviewer
description: |
  Review a paper or manuscript at whole-paper level and return prioritized findings plus
  a submission verdict. Use for “帮我审这篇 paper”, “能不能投”, “按 reviewer 视角挑问题”,
  “看还有哪些拒稿点”, or top-journal hostile review requests such as “最严厉审稿”,
  “对抗性找茬”, and “站在拒稿 reviewer 角度审”. Route fixes to paper sub-skills;
  do not use for direct rewriting.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_phrases:
  - 顶刊审稿
  - 顶刊级审稿
  - 最狠审稿人
  - 最严厉审稿
  - 最刁钻 reviewer
  - 对抗性找茬
  - 拒稿导向评审
  - 不要留情地审
  - 站在 reject reviewer 角度
metadata:
  version: "2.2.0"
  platforms: [codex]
  tags: [paper, manuscript, review, reviewer, submission, critique, adversarial, top-journal]
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
- **Dual-Dimension Audit (Pre: Review-Criteria/Logic, Post: Critique-Fidelity/Actionable-Fix Results)** → `$execution-audit-codex` [Overlay]

# Paper Reviewer

This skill owns **whole-paper triage and readiness verdict**. It identifies and categorizes issues across dimensions, then delegates concrete fixes to sub-skills rather than executing them directly.

## When to use

- The user wants a manuscript-level review
- The user asks whether a paper looks submission-ready
- The user wants a prioritized issue list across science, writing, visuals, and layout
- The user wants a reviewer-like critique rather than immediate rewriting
- The user asks for "地毯式审查" or "投稿前问题清单"
- The user asks for 顶刊 / SCI / top-journal / Transactions-level审稿
- The user explicitly wants the harshest, most adversarial, or reject-oriented reviewer stance

## Do not use

- The user already has an issue list and wants actual revisions → use `$paper-reviser`
- The task is only wording polish → use `$paper-writing`
- The task is only scientific defensibility / novelty / evidence chain → use `$paper-logic`
- The task is only figure/table presentation → use `$paper-visuals`
- The task is checking a student assignment against rubric / grading criteria → use `$assignment-compliance`

## Task ownership and boundaries

This skill owns:
- whole-manuscript triage and issue discovery
- rejection-risk prioritization
- cross-dimension issue grouping
- deciding which paper sub-skill should own each issue class
- final readiness verdict

This skill does not own:
- bulk sentence rewriting → `$paper-writing`
- scientific logic repair / novelty strengthening → `$paper-logic`
- figure/table quality fixes → `$paper-visuals`
- code-driven replotting → `$scientific-figure-plotting`
- symbol/notation cleanup → `$paper-notation-audit`
- artifact-native PDF extraction mechanics → `$pdf`

## Finding-driven framework role

This skill is a **Phase-1 detector/planner anchor** in the shared finding-driven framework. It should emit structured findings that downstream paper skills can consume. Use the shared fields in [`../SKILL_FRAMEWORK_PROTOCOLS.md`](../SKILL_FRAMEWORK_PROTOCOLS.md) while preserving paper-native semantics.

Use the shared severity spec exactly as written in [`references/severity-spec.md`](references/severity-spec.md); do not invent alternate scales.

Minimum fields for each material issue:
- `finding_id`
- `category`
- `severity` + `severity_native` (`P0 / A / B / C`)
- `evidence`
- `impact`
- `adversarial_attack`
- `fixability`
- `recommended_owner_skill`
- `recommended_executor_skill` (default: `$paper-reviser` for coordinated revision)
- `verification_method`
- `status` (default: `open`)

Paper defaults:
- map severities as `P0 → blocker`, `A → high`, `B → medium`, `C → low`
- use `fixability` such as `text-only`, `local-edit`, `cross-section`, `needs-experiment`, or `needs-human-decision`
- every finding should name the evidence location and the reviewer-facing consequence in one sentence
- every `P0` / `A` / `B` finding should include a one-line `adversarial_attack` phrased as the sharpest defensible reviewer objection
- if the user only wants critique, stop after findings + verdict
- if the user wants changes, hand the findings to `$paper-reviser` or the narrower paper owner
- structure `论文问题总表` so `$paper-reviser` can consume it directly without re-classifying from scratch

## Review posture contract

Unless the user explicitly asks for a lighter tone, this skill should review from a **skeptical specialist reviewer** stance, not a coaching stance.

- In adversarial / top-journal mode, start from **reject unless the manuscript disproves the rejection case**
- Give **zero credit** for claims that require author explanation outside the manuscript
- Treat polished prose as presentation only; it never repairs missing evidence
- If the strongest nearby baseline, fairness control, or robustness check is missing, treat that gap as decision-relevant rather than cosmetic
- For each central claim, ask: what exact sentence would appear in a strong reject review?
- Prefer surfacing the **paper-killing weakness** before enumerating smaller fixes

## Required workflow

1. **Identify context**:
   - manuscript stage (draft / revision / camera-ready)
   - target venue and its bar (if known)
   - available artifacts: text, PDF, screenshots, figures, tables, reviewer comments

2. **Choose execution depth**:

   #### Hostile mode (default for 顶刊 / SCI / top-journal / adversarial requests)
   Run all Tier-0/1/2/3 checks, apply the review posture contract above, and complete 5 rounds of reflective self-challenge.
   Use for: Nature-family, Science-family, IEEE Transactions, flagship journals, or whenever the user asks for “最狠/最严厉/最刁钻/对抗性/拒稿导向”审稿.
   In hostile mode:
   - assume the reviewer is looking for the cleanest defensible reject case
   - treat an unsupported central contribution claim as at least `A` unless the manuscript itself clearly narrows the claim
   - treat missing strongest baselines / fairness controls / robustness evidence as acceptance risks, not optional suggestions
   - require an explicit `Reject Case` summary in the output

   #### Full mode
   Run all Tier-0/1/2/3 checks and 3 rounds of reflective self-challenge.
   Use for: normal submission-readiness review when the user wants a thorough but not explicitly hostile reviewer simulation.

   #### Lightweight mode
   Run Tier-0/1 checks and 1 round of reflective challenge (Round 2: Reviewer Simulation only).
   Use for: workshops, regional conferences, course papers, internal drafts.
   In lightweight mode, skip Tier-3 polish checks and reduce self-challenge to 1 round.

   Explicitly state which mode was used in the output.

3. **Review by dimension** — systematically check each tier:

   Follow the source-backed reviewer playbook in [`references/review-rubric-playbook.md`](references/review-rubric-playbook.md) and keep the main review order stable:
   1. ethics / integrity / scope fit
   2. core claim support and soundness
   3. significance and originality
   4. completeness, reproducibility, and experimental adequacy
   5. clarity, presentation, and paper mechanics

   Treat `P0` as an immediate stop. Treat `A` as a core acceptance blocker. Treat `B` as evidence missing for the current claim. Treat `C` as non-blocking polish.

   ### Tier-0: Reject-on-sight (any one → reject)
   - Academic integrity: plagiarism, data provenance, unattributed figures
   - Critical experimental flaws that invalidate the entire study

   ### Tier-1: Core acceptance dimensions
   Route detailed audit to `$paper-logic`, but verify:
   - Research gap and novelty positioning
   - Whether the claimed delta is actually detectable against the strongest adjacent baseline, not only against weaker baselines the authors selected
   - Theoretical consistency and method rigor
   - Experiment design and statistical rigor (Mean±Std, significance tests, ablations)
   - Fairness of comparison: same data regime, same tuning budget, same pretrained resources, same metric definitions
   - Abstract quantification and conclusion quality
   - Whether the paper makes a detectable contribution for the stated venue, even if the method is incremental
   - Whether claims are supported by the evidence actually shown, not by what the authors say in prose
   - Whether the venue-fit claim survives a skeptical expert asking “why is this enough for this venue rather than a lower-tier venue?”

   > For deep statistical questions, route to `$statistical-analysis`.

   ### Tier-2: Common reviewer attack points
   - Symbol system global consistency → `$paper-notation-audit`
   - Reproducibility and engineering transparency
   - Algorithm complexity and deployment feasibility
   - Uncertainty quantification rigor (if applicable)
   - Limitations honesty and boundary defense
   - Narrative coherence across sections
   - Completeness of the submission package: missing baselines, ablations, appendix material, or key implementation details
   - Whether the review can point to concrete manuscript locations instead of generic criticism
   - Benchmark hygiene: cherry-picked comparisons, suspicious split choices, data leakage risks, or omitted failure cases
   - Robustness breadth: sensitivity to seeds, hyperparameters, domains, noise, or prompt templates when relevant
   - Overclaim detection: causal, theoretical, generalization, or deployment claims that outrun the evidence shown
   - Negative-result exposure: whether an adversarial reviewer could ask for the exact experiment the manuscript currently avoids

   ### Tier-3: Professional polish
   - Academic writing quality → `$paper-writing`
   - Citation and reference management → `$citation-management`
   - Typesetting and layout → `$pdf`
   - Figures, tables, and captions → `$paper-visuals`
   - Abbreviation first-use expansion and notation consistency → `$paper-notation-audit`
   - Keywords and metadata completeness
   - Language consistency (spelling convention, number format, hyphenation)

4. **Merge into prioritized list**:
   - Separate fatal issues (P0) from core flaws (A), missing data/experiments (B), and text polish (C)
   - Normalize each material issue into a finding entry with: `finding_id`, dimension/category, evidence, severity + native severity, likely reviewer impact, `adversarial_attack`, `fixability`, recommended owner/executor skill, and verification method
   - Keep the finding list grouped by severity first, then by review dimension, then by manuscript location
   - If the same defect appears in multiple sections, record one primary finding and mention the spread in the evidence field

5. **Reflective Self-Challenge** — run 3 rounds in Full mode and 5 rounds in Hostile mode before delivering the verdict:

   #### Round 1: Devil's Advocate Pass
   Re-read the merged issue list and ask for each:
   - Did I miss a worse version of this problem?
   - Is there a related issue I failed to flag?
   - Am I being too lenient on issues because the writing is polished?

   If new issues surface, add them to the list with marker `[R1]`.

   #### Round 2: Reviewer Simulation
   Imagine the **most adversarial reviewer** at the target venue. Ask:
   - What is the single strongest objection this reviewer would raise?
   - Is there a fatal flaw I rationalized away?
   - Would this reviewer accept the current novelty claim?

   If the adversarial reviewer would raise issues not in the list, add them with marker `[R2]`.

   #### Round 3: Baseline and Fairness Prosecution
   Ask:
   - What is the strongest missing baseline, sanity check, or fairness control?
   - Could the claimed gain disappear under a fairer comparison or harder split?
   - Did I silently trust the authors on tuning, compute, or data curation?

   If new issues surface, add them with marker `[R3]`.

   #### Round 4: Reject Case Compression
   Write the one-paragraph strong-reject review this paper would most plausibly receive.
   - If that paragraph introduces a sharper concern than the ledger, add it with marker `[R4]`.
   - If the paragraph cannot cite concrete evidence locations, re-audit for missing evidence links before continuing.

   #### Round 5: Confidence Calibration
   For each issue in the final list:
   - Assign a confidence level: `high` / `medium` / `low`
   - For `low` confidence issues, state explicitly what evidence would raise confidence
   - If the overall verdict changed across rounds, note the shift and explain why

   #### Reflection termination
   - In Full mode, if Round 2 and Round 3 add zero new P0 or A issues, the review is stable
   - In Hostile mode, if Round 2 through Round 5 add zero new P0 or A issues, the review is stable
   - If any round adds a P0, restart the merge (Step 3) with the expanded list
   - Record the round count and any shifts in the output

6. **Deliver readiness verdict**:
   - Open with a short neutral summary of the manuscript's claimed contribution and target audience
   - List a few genuine strengths before the critical findings if the paper has them
   - Clear yes/no/conditional recommendation
   - Top 3 revision priorities if not ready
   - Remaining defense risks even if mostly ready
   - State whether the paper is ready for the stated venue bar, not just whether it is "interesting"
   - If the venue is unknown, phrase the verdict as conditional on the paper meeting a typical venue-standard bar

## Output defaults

Use `论文问题总表`:

### Reject Case
> One short paragraph: the cleanest defensible rejection rationale a hostile reviewer would write today.

### P0: 一票否决 — 不修则拒
> Fatal issues: data integrity, academic honesty, hard theoretical errors.

### A: 核心硬伤
> Model weaknesses or failures to meet top-venue bar.

### B: 需补充数据/实验
> Issues that require new data, baselines, ablations, or statistical validation.

### C: 文本打磨
> Logic restructuring, symbol cleanup, writing polish.

> For full severity definitions and cross-skill consistency rules, see [references/severity-spec.md](references/severity-spec.md).

## Verdict Rules

- `ready`: no P0 or A issues remain, and any remaining B items are explicitly non-blocking for the stated venue/bar
- `conditional`: the paper has a plausible path to acceptance, but one or more B items or venue-fit uncertainties remain
- `not ready`: any P0 or A issue remains, or the evidence is too weak to support the current claims
- In Hostile mode, avoid `ready` unless the Reject Case collapses to non-blocking polish and no central claim depends on unstated author clarification
- If the manuscript is close but not ready, name the smallest set of changes that would most improve the decision
- If the venue asks for major/minor labels, translate them internally to P0/A/B/C rather than mixing schemes in the output

### Finding Ledger Minimum Fields

| Finding ID | Category | Severity | Evidence | Adversarial Attack | Fixability | Recommended Owner | Recommended Executor | Verification | Status |
|---|---|---|---|---|---|---|---|---|---|
| PR-01 | novelty-positioning | A / high | Related work omits strongest 2025 baseline | “The claimed novelty collapses once the obvious 2025 comparator is included.” | needs-experiment | `$paper-logic` | `$paper-reviser` | re-run novelty / baseline audit | open |

### Reflection Summary
- Rounds completed: N
- Issues added in R1: [count]
- Issues added in R2: [count]
- Issues added in R3: [count or "n/a"]
- Issues added in R4: [count or "n/a"]
- Confidence shifts: [description or "none"]
- Adversarial reviewer's top concern: [one line]

## Next step after review

- If fixes are broad, route the issue list to `$paper-reviser`.
- If fixes are narrow, route each issue to its owner skill directly.

## Hard constraints

- Do not collapse all issues into vague "needs improvement" language.
- Do not call a paper ready just because wording is polished.
- When rendered evidence exists, use artifact-grounded judgment rather than memory.
- Do not fabricate or assume experimental results.
- Be direct and specific; avoid "could consider" or "might benefit from" hedging.
- **Superior Quality Audit**: For high-fidelity peer review simulation, trigger `$execution-audit-codex` to verify critique against [Superior Quality Bar](../execution-audit-codex/references/superior-quality-bar.md).

## Trigger examples
- "强制进行论文评审深度审计 / 检查评审建议的专业性与可执行性。"
- "Use $execution-audit-codex to audit this review for critique-fidelity idealism."
- "按顶刊最狠审稿人的标准给我找茬，不要留情。"
- "站在 reject reviewer 角度审这篇，给我最刁钻的攻击点。"
