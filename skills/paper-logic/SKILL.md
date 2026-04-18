---
name: paper-logic
description: |
  Audit a paper's scientific defensibility under peer review: claims-vs-
  evidence alignment, novelty positioning, experiment coverage, ablation
  isolation, and statistical rigor. Produces a severity-rated 逻辑问题单 that
  separates text-fixable issues from "needs new experiment" blockers. Use when
  the user asks "看逻辑", "修逻辑", "创新性够不够", "实验站不站得住", "claims
  和 evidence 对不对齐", "审稿人会怎么攻击", or wants science-level critique
  rather than wording polish.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
metadata:
  version: "2.1.1"
  platforms: [codex]
  tags: [paper, logic, novelty, experiments, reviewer, evidence, statistics, symbols]
framework_roles:
  - detector
  - executor
framework_phase: 1
framework_contracts:
  emits_findings: true
  consumes_findings: true
  emits_execution_items: false
  consumes_execution_items: false
  emits_verification_results: false
risk: medium
source: local
---
- **Dual-Dimension Audit (Pre: Argument-Chain/Logic, Post: Conclusion-Consistency/Data-Match Results)** → `$execution-audit-codex` [Overlay]

# Paper Logic

This skill owns **scientific-defensibility review and repair** for academic papers.

## Finding-driven framework compatibility

Logic findings produced here should be mappable to the shared
finding-driven framework without flattening paper-native semantics.

Minimum compatibility expectations:
- preserve a stable `finding_id` when the same logic issue persists, including upstream dimension-coded IDs such as `NOV-01`, `THY-02`, `EXP-03`, `RES-04`, `WRT-05`, `REF-06`, and `VIS-07` when they already point to a logic-surface problem
- keep `severity_native` in paper terms (`P0 / A / B / C`)
- include `evidence`, `fixability`, and `recommended_owner_skill`
- preserve richer upstream planning hints such as shortest repair path, `repair_leverage`, and whether the strongest next move is to reorganize underexploited evidence already present
- when the result is a repair task, consume findings from `$paper-reviewer`
  or reviewer comments already normalized into findings

## When to use

- The user wants logic review or logic repair
- The key question is novelty, contribution, evidence chain, or experimental support
- Reviewer comments challenge scientific framing rather than wording
- The user wants to know whether claims are defensible under peer review scrutiny

## Do not use

- The user wants whole-paper triage → use `$paper-reviewer`
- The task is mainly prose polish → use `$paper-writing`
- The task is mainly figure/table presentation → use `$paper-visuals`
- The task is only citation hygiene → use `$citation-management`
- The task is algorithm/code correctness **outside a paper context** → use `$research-engineer`

## Routing clarification: `paper-logic` vs `research-engineer`

| Dimension | `paper-logic` | `research-engineer` |
|-----------|---------------|---------------------|
| **Context** | Paper manuscript for submission | Algorithm / code / spec in isolation |
| **Input** | Paper draft, reviewer comments | Code, algorithm spec, proof, design doc |
| **Output** | 逻辑问题单 with severity ratings | Technical verdict + weakness list |
| **Tone** | Systematic, reviewer-anticipating | Blunt, correctness-first |
| **Typical trigger** | "论文逻辑站不站得住" "审稿人会怎么攻击" | "这个算法对不对" "复杂度分析" |

Rule of thumb: if the question is **about a paper's defensibility under peer review** → `paper-logic`. If the user is asking about **code/algorithm correctness outside a paper** → `research-engineer`.

## Required workflow

1. **Identify scope**:
   - main claim and contribution list
   - claimed novelty and its level (methodological innovation vs engineering improvement)
   - evidence supporting each claim
   - manuscript stage and target venue
   - whether incoming findings already specify shortest repair path, `repair_leverage`, or an underexploited-evidence route that should be honored during logic repair

2. **Audit scientific logic** (Tier-1 checks):

   ### Research Gap & Novelty Positioning
   - Is the logic chain closed: existing limitations → motivation → gap → contributions?
   - Are contributions specific, testable, and non-overlapping?
   - Is the novelty level honestly classified (methodology vs engineering improvement)?
   - Does Related Work cover the last 2 years of strong competitors?
   - Are competitors described fairly (no straw-man weakening)?

   ### Theoretical Depth & Consistency
   - Are embedded priors (ODE/PDE constraints, symmetries, conservation laws) mathematically derived, not just appended to the loss?
   - Is balance between physics and data-driven terms justified (error bounds, convergence proof, Pareto argument)?
   - Are applicability boundaries of theoretical assumptions discussed?
   - Is there "theory washing" (physics component contributes negligibly but is sold as the main novelty)?
   - Run an explicit theory-breakpoint scan: declaration-vs-derivation gaps, local approximation or closure assumptions, unexplained surrogate substitutions, and claims that survive only if the reviewer grants unstated author intuition

   ### Method Rigor & Mathematical Self-Consistency
   - Is derivation from definitions to final algorithm step-by-step complete?
   - For every approximation jump, is the admissible regime or validity window stated?
   - If a closure, proxy relation, or first-order expansion is introduced, does the paper explain where it comes from and what error it may induce?
   - Do pseudocode variables match the math exactly?
   - Are all hyperparameters (schedule, weights, search range, final values) disclosed?
   - Are custom operators explicitly defined at first use?
   - Are loss sub-terms dimensionally consistent or properly normalized?
   - Is gradient conflict across multi-task losses discussed?
   - Do theorem conditions actually match experimental settings?

   ### Experiment Design & Statistical Rigor
   - Are baselines current, sufficient, and complete (no "missing strong baseline" risk)?
   - Do ablations cleanly isolate each claimed contribution?
   - Are all performance comparisons reported as Mean ± Std (≥5 seeds)?
   - Is statistical significance testing explicit (test name, sample size, p-value)?
   - Is there data leakage audit (train/val/test split, temporal leakage, feature leakage)?
   - Are evaluation metrics comprehensive (accuracy, calibration, robustness, efficiency)?
   - Do experiments cover multiple operating conditions (steady/transient/noisy/cross-domain)?

   > For deep statistical method selection, effect size, power analysis, or Bayesian inference questions, route to `$statistical-analysis`.

   ### Abstract & Conclusion Quality
   - Does the abstract meet "elevator pitch" standard: background → pain → method → quantitative breakthrough (at least one number)?
   - Does every abstract claim have a precise experimental match?
   - Does the conclusion avoid copy-pasting the abstract and rise to design-principle-level insights?
   - Does the abstract mention limitations or applicability scope?

3. **Cross-check audit** (systematic checks):

   > **Notation, symbol, abbreviation, and formula consistency** → route to `$paper-notation-audit`

   | # | Check | Pass Criteria |
   |---|---|---|
   | C1 | Abstract vs Experiments | Every abstract claim has exact experimental evidence |
   | C2 | Abstract vs Conclusion | Conclusion has independent insights, not a rewording |
   | C3 | Figure-text alignment | Architecture diagram modules match method text exactly |
   | C4 | Table-text alignment | Stated baseline count = table baseline rows |
   | C5 | Contribution tracking | Every intro contribution lands in method + experiments |
   | C6 | Hyperparameter closure | Every method hyperparameter has experiment value or sensitivity analysis |
   | C7 | Related Work ↔ Baselines | Methods discussed in Related Work appear as baselines or are explained |

4. **Self-challenge checkpoint**:

   Before delivering the logic audit result, explicitly challenge your own judgment:

   - For each "pass" in the cross-check table: state the weakest link that could flip it to "fail"
   - For each claimed novelty: name the closest existing work and explain why the novelty claim still holds despite it
   - For any severity rating below A: justify why it is not more severe
   - If more than 3 checks passed without any reservation, flag this as a review-depth warning and re-examine
   - If a finding is fixable without new experiments, state the shortest credible repair path; if not, say exactly what new evidence would be needed
   - When underexploited evidence already exists in the manuscript, appendix, figures, tables, or notes, prefer reorganizing that evidence before escalating to a `needs new experiment` conclusion

   This step prevents **false security** from systematic checklists that can mask lenient judgment.

5. **Deliver results**:
   - If the user wants critique only: output `逻辑问题单`
   - If the user wants fixes: rewrite claims/framing honestly without inventing evidence
   - Flag blocked issues that require new experiments rather than text edits
   - Preserve `repair_leverage` when incoming findings provide it; otherwise assign `high / medium / low` to help downstream revision planning

## Output defaults

Use `逻辑问题单` or `逻辑修订记录`:
- finding_id
- claim
- weakness
- evidence (present or missing)
- fix (text edit or new experiment needed)
- shortest credible repair path
- repair_leverage: high / medium / low
- severity: P0 (reject-level) / A (core flaw) / B (needs data) / C (text polish)
- whether new evidence is required
- whether the preferred path is `existing evidence reorganized` or `new evidence created`

> Severity definitions follow the shared paper-skill severity spec. See [`$paper-reviewer` references/severity-spec.md](../paper-reviewer/references/severity-spec.md).

## Common Issues and Solutions

**Issue: Abstract too generic**
Delete first sentence if it could be prepended to any ML paper. Start with your specific contribution.

**Issue: Introduction exceeds 1.5 pages**
Split background into Related Work. Front-load contribution bullets. Methods should start by page 2-3.

**Issue: Experiments lack explicit claims**
Add sentence before each experiment: "This experiment tests whether [specific claim]..."

**Issue: Missing statistical significance**
Always include error bars (specify: std dev or std error), number of runs, and statistical tests if comparing methods.

**Issue: Novelty framed as "first to X"**
Dangerous unless truly defensible. Prefer framing as "unlike prior work that Y, we Z" with concrete differentiation.

## Hard constraints

- Do not fabricate missing experiments, data, or evidence.
- Do not solve evidence gaps with wording tricks.
- Do not soften serious scientific flaws into vague "could be improved" language.
- Separate proven facts from inference explicitly.
- **Superior Quality Audit**: For critical scientific validation, trigger `$execution-audit-codex` to verify logic against [Superior Quality Bar](../execution-audit-codex/references/superior-quality-bar.md).

## Domain-specific checklists

The Tier-1 checks in this skill are general-purpose. For domain-specific audit
items covering CV, NLP, RL, Theoretical ML, Physics-Informed ML, and Biomedical AI,
see [references/domain-checklists.md](references/domain-checklists.md).

When auditing a paper, identify the domain first and load the relevant
domain-specific checklist alongside the general audit framework.
