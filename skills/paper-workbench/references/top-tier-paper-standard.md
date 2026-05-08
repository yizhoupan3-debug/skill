# Top-Tier Paper Standard

Use this reference when the user wants 顶刊, 顶会, CCF-A, Nature/Science/Cell,
NeurIPS/ICML/ICLR, "top-tier paper", or a manuscript that can plausibly survive
a selective venue. It is a quality bar, not a promise that editing alone can
make any project accepted.

## Core Standard

A top-tier paper needs all six layers to survive at once:

- Target contract: venue, track/article type, audience, page/disclosure/artifact rules, and review culture are explicit.
- Contribution right: the paper's move is important to that venue, not merely interesting to the authors.
- Novelty separation: closest prior work is named, compared fairly, and the remaining gap still matters.
- Evidence closure: main claims are backed by decisive experiments, proofs, analysis, or qualitative evidence appropriate to the field.
- Reviewer attack resistance: the easiest reject case has been anticipated and either repaired, narrowed, or disclosed.
- Manuscript surface quality: title, abstract, introduction, figures, tables, limitations, references, and layout all reflect the same supported claim.

## Non-Negotiable Checks

- What is the single contribution reviewers should remember?
- Which accepted papers are the nearest positive examples, and which rejected-style failure does this draft risk?
- What is the strongest prior method, dataset, theorem, baseline, or clinical/empirical comparator that a reviewer will expect?
- Which result would still matter if the weakest supporting experiment were removed?
- Where does the paper overclaim beyond evidence, novelty, or population scope?
- What must be cut, moved to appendix, or downgraded so the remaining paper becomes harder to reject?
- Is the paper reproducible or auditable enough for the target venue's norms?

## Claim Audit Addendum (Anti-Drift)

Use this addendum for multi-round revision and response-letter cycles:

- Keep a stable claim ledger with explicit `claim_id` and allowed claim level.
- Require each claim to point to decisive evidence anchors (figure/table/result/citation).
- Run mirror-surface checks across abstract, intro, conclusion, captions, and rebuttal text.
- Flag any wording that upgrades causality, generality, or superiority without a
  matching evidence-anchor update.
- Reject "style-only" edits that dilute auditable experiment detail below the
  minimum needed for independent scrutiny.

Compact drift card:

```text
claim_ledger_status:
new_claim_ids:
upgraded_claim_phrases:
missing_evidence_anchor:
auditability_loss_risk:
```

## Field-Specific Calibration

- ML / AI conferences: compare against recent proceedings, required baselines, ablations, compute fairness, dataset leakage, reproducibility, code/artifact norms, and negative or failure cases.
- Biomedical / clinical journals: check ethics, cohort definition, endpoints, statistics, confounding, external validation, reporting guidelines, and clinical relevance.
- Natural science journals: check mechanism, controls, sample size, measurement validity, uncertainty, alternative explanations, and whether the result changes the field conversation.
- Theory / math-heavy papers: check theorem necessity, proof closure, assumptions, counterexamples, relation to prior theory, and whether formalism supports the claimed insight.
- Systems / HCI / applied venues: check workload realism, user/task validity, baselines, deployment constraints, cost, scalability, and failure modes.

Cross-venue reporting norms to enforce when applicable:

- Nature Portfolio reporting expectations: sample-size/statistics transparency,
  code availability, and explicit data-availability statements.
- NeurIPS/ICLR checklist expectations: abstract/introduction claims must match
  actual contribution scope; limitations, reproducibility details, and
  compute/training disclosure should be explicit.
- EQUATOR-family guidance (e.g., CONSORT/STROBE/PRISMA where relevant): use as
  reporting completeness checks, not as substitutes for study design quality.

## Workflow Implications

- Start with review, not prose, when contribution/evidence/novelty is unknown.
- Use external calibration when the venue bar, closest work, or required baseline could change the verdict.
- Prefer one strong, defensible story over several weak contribution angles.
- If the evidence cannot support a top-tier claim, choose among new evidence, narrower claim, different venue, appendix routing, or stopping the claim.
- Only after the scientific bar is safe should `paper-writing` optimize story, tone, and sentence-level polish.

## Expression Strength Rule

After claim ceiling is frozen, top-tier writing should be:

- direct in claim presentation,
- affirmative in evidence-backed statements,
- fast in narrative pacing (claim -> evidence -> implication),
- restrained only where scope boundaries truly require it.

Do not default to negative, internal, or defensive rhetoric when the claim is
already supportable.

## Output Card

For top-tier readiness reviews, include this compact card:

```text
target_bar:
top_contribution:
closest_reject_case:
missing_decisive_evidence:
claim_ceiling:
next_honest_move:
```
