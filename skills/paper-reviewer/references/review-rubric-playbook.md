# Paper Reviewer Playbook

This is the source-backed operating rubric for `paper-reviewer`.

## Source Index

- NeurIPS reviewer guidelines: [https://neurips.cc/Conferences/2023/ReviewerGuidelines](https://neurips.cc/Conferences/2023/ReviewerGuidelines)
- ICML reviewer instructions: [https://icml.cc/Conferences/2025/ReviewerInstructions](https://icml.cc/Conferences/2025/ReviewerInstructions)
- Nature referee guidance: [https://www.nature.com/nature/for-referees](https://www.nature.com/nature/for-referees)
- Nature peer review policy: [https://www.nature.com/nature/editorial-policies/peer-review](https://www.nature.com/nature/editorial-policies/peer-review)
- COPE ethical guidelines for peer reviewers: [https://publicationethics.org/files/Ethical_Guidelines_For_Peer_Reviewers.pdf](https://publicationethics.org/files/Ethical_Guidelines_For_Peer_Reviewers.pdf)
- NIH reviewer guidance: [https://grants.nih.gov/policy-and-compliance/policy-topics/peer-review/simplifying-review/reviewer-guidance](https://grants.nih.gov/policy-and-compliance/policy-topics/peer-review/simplifying-review/reviewer-guidance)
- NIH review criteria: [https://www.niaid.nih.gov/research/review-criteria](https://www.niaid.nih.gov/research/review-criteria)

## Primary review order

1. Ethics, integrity, and scope fit
2. Core claim support and soundness
3. Significance and originality
4. Completeness, reproducibility, and experimental adequacy
5. Clarity, presentation, and paper mechanics

## What to look for

- A reviewer report should state what the paper contributes, whether the contribution is worth publishing at the stated venue, and what technical failures block acceptance.
- A good review is specific, evidence-linked, and constructive. It should name the problem, show where the problem appears, and explain why it matters for the decision.
- Treat novelty as a contribution question, not a binary "new method" question. Incremental work can still be significant if the increment is detectable and well supported.
- Do not treat polished prose as evidence that the scientific claim is sound.

## Adversarial top-journal mode

Use this stance when the user asks for 顶刊 / SCI / hostile / most adversarial review, or when the venue bar is explicitly elite journal level.

- Start from the strongest plausible **reject case**, not from “how can this be fixed nicely?”
- Judge the manuscript **as written**. If a claim needs author clarification outside the text, treat the paper as currently unsupported.
- Assume a skeptical specialist with limited patience: missing strongest baselines, fairness controls, robustness checks, or boundary conditions count against acceptance.
- Every major claim should be attacked with the question: “what exact evidence here would convince a hostile reviewer this is not oversold?”
- Prefer decision-relevant weaknesses before polish suggestions.

## Hostile reviewer attack surface

When running adversarial review, explicitly probe:

- contribution detectability against the strongest nearby baseline, not only author-selected baselines
- fairness of comparison: compute budget, tuning budget, pretrained assets, data access, metric definitions
- benchmark hygiene: split leakage, cherry-picked datasets, omitted negative results, fragile cherry-picked seeds
- robustness width: cross-domain, noise, prompt, hyperparameter, and seed sensitivity where relevant
- overclaim risk: causal, theoretical, deployment, generalization, or mechanism claims stronger than the evidence shown
- venue-fit insufficiency: why this may still be below the bar for the target journal even if technically competent
- manuscript self-sufficiency: whether a reviewer can verify the claim chain without private author explanation

## Source-grounded reminders

- NeurIPS and ICML review forms emphasize soundness, significance, originality, clarity/presentation, and completeness.
- Nature referee guidance emphasizes technical failings and whether the manuscript serves the audience it targets.
- COPE emphasizes objectivity, confidentiality, expertise, timeliness, and reporting ethics concerns to the editor instead of investigating privately.
- NIH criteria emphasize importance, rigor and feasibility, and expertise/resources evaluated in context.

## Review output shape

- Start with a one-paragraph summary of what the paper claims to do.
- Then give positives briefly.
- In adversarial top-journal mode, state the one-paragraph **Reject Case** before the detailed ledger.
- Then list findings in severity order with evidence and impact.
- End with an explicit recommendation: ready, conditional, or not ready.
- If a recommendation depends on missing evidence, say exactly what evidence would change it.
- Number findings and cite manuscript locations whenever possible.
- Keep critique focused on the paper, not the authors, and keep any ethics concerns confidential to the editor if the target venue supports that channel.
