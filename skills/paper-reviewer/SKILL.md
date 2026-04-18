---
name: paper-reviewer
description: |
  Review a paper or manuscript at whole-paper level and return prioritized findings,
  grounded strengths, and a submission-readiness verdict. Use for “帮我审这篇 paper”,
  “能不能投”, “按 reviewer 视角挑问题”, “看还有哪些硬伤和亮点”, or requests for
  strict / top-journal / adversarial review such as “最严厉审稿”, “对抗性找茬”, and
  “站在 reviewer 角度审”. Route fixes to paper sub-skills; do not use for direct rewriting.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
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
  version: "2.3.3"
  platforms: [codex]
  tags: [paper, manuscript, review, reviewer, submission, critique, strengths, adversarial, top-journal]
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
- The user wants a prioritized issue list across science, writing, visuals, layout, and acceptance-path viability
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
- rejection-risk and acceptance-path prioritization
- cross-dimension issue grouping
- identifying grounded strengths, salvageability, and venue-plausible acceptance paths
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

Use the shared severity spec exactly as written in [`references/severity-spec.md`](references/severity-spec.md); do not invent alternate scales. For the exact output contract, required finding fields, and required reflection fields, follow [`references/review-rubric-playbook.md`](references/review-rubric-playbook.md).

Paper defaults:
- map severities as `P0 → blocker`, `A → high`, `B → medium`, `C → low`
- run a detector pass before final `P0 / A / B / C` mapping across: `novelty`, `theory`, `experiment`, `result-interpretation`, `writing`, `references`, and `visuals`
- use dimension-coded `finding_id` prefixes such as `NOV-01`, `THY-01`, `EXP-01`, `RES-01`, `WRT-01`, `REF-01`, and `VIS-01`
- prefer `text-only`, `local-edit`, `cross-section`, `needs-experiment`, or `needs-human-decision` for `fixability`
- every finding must stay evidence-linked, reviewer-facing, and directly consumable by `$paper-reviser`
- prefer underexploited evidence already present in the manuscript before demanding entirely new work
- always separate visual findings from non-visual findings in both workflow and output

## Review posture contract

Unless the user explicitly asks for a hostile simulation, this skill should review from a **strict but fair specialist reviewer** stance, not a coaching stance.

- Default stance: assess the manuscript as a serious submission, not as a paper that must be rejected
- In adversarial / top-journal mode, explicitly stress-test the strongest reject case without pretending rejection is the only valid outcome
- Give no credit for claims that require author explanation outside the manuscript
- Treat polished prose as presentation only; it never repairs missing evidence
- If the strongest nearby baseline, fairness control, or robustness check is missing, treat that gap as decision-relevant rather than cosmetic
- For each central claim, ask both: what exact reject sentence could a strong reviewer write, and what evidence currently keeps that objection from becoming fatal?
- Surface the paper-killing weakness first, but also identify the narrowest credible path to a positive decision when one exists

## Required workflow

1. Identify context:
   - manuscript stage, target venue/bar, and available artifacts
   - whether the user wants hostile, full, or lightweight review depth

2. Choose mode:
   - `Hostile`: top-journal / adversarial / reject-oriented asks; run Tier-0/1/2/3 plus 5 independent challenge rounds
   - `Full`: normal thorough review; run Tier-0/1/2/3 plus 3 independent challenge rounds
   - `Lightweight`: workshop / internal draft / fast screen; run Tier-0/1 plus 1 strong-objection round
   - Always state the chosen mode in the output

3. Run the detector pass before severity mapping:
   - surface issues across `novelty`, `theory`, `experiment`, `result-interpretation`, `writing`, `references`, and `visuals`
   - then map them into `P0 / A / B / C`
   - keep the review order stable: integrity/scope, claim support, originality/significance, experimental adequacy, accept-path viability, then presentation

4. Apply the tier model:
   - `Tier-0`: reject-on-sight issues such as integrity failures or fatal experimental invalidity
   - `Tier-1`: claim support, novelty/gap positioning, theory consistency, fairness of comparison, statistical sufficiency, venue-fit detectability
   - `Tier-2`: reproducibility, benchmark hygiene, robustness breadth, overclaim risk, underexploited evidence, acceptance-path viability
   - `Tier-3`: writing, references, PDF/layout, visuals, notation, and language consistency
   - Route deep subproblems to the narrower paper skills named above instead of widening this owner

5. Merge into the baseline ledger:
   - group by severity first, then `Non-visual` before `Visual`
   - every material finding must include: `finding_id`, category/dimension, evidence, severity plus native severity, likely reviewer impact, `adversarial_attack`, `fixability`, owner skill, and verification method
   - for each `A`/`B` finding, state the shortest credible repair path and whether it mainly needs evidence reorganization or genuinely new evidence
   - add a short `Grounded Strengths / Accept Path` note alongside the issue ledger

6. Run independent challenge rounds after the baseline ledger exists:
   - each round is a fresh attack pass, not a paragraph rewrite
   - rotate dimensions whenever possible and carry forward only unresolved `P0 / A / decision-relevant B` plus prior deltas
   - valid round archetypes include `Reviewer Simulation`, `Baseline and Fairness Prosecution`, `Reject Case Compression`, `Confidence Calibration`, and `False-Convergence Challenge`
   - `Hostile` must include `Reviewer Simulation`, `Baseline and Fairness Prosecution`, `Reject Case Compression`, and `False-Convergence Challenge`
   - `Full` must include `Reviewer Simulation` plus two non-duplicate dimensions
   - a wording-only round does not count
   - stability requires two consecutive orthogonal null deltas after at least one `False-Convergence Challenge`

7. Deliver the readiness verdict:
   - start with a short neutral summary of the claimed contribution and target audience
   - then give prioritized findings, grounded strengths, top revision priorities, smallest credible accept-path, and remaining defense risks
   - judge readiness against the stated venue bar, or a typical venue-standard bar if the venue is unknown

## Output defaults

Use `论文问题总表`.

**Hard output rule:** Keep the fixed section order and keep `Non-visual` before `Visual` in every severity bucket. Do not reorder sections. Do not merge visual and non-visual findings. For the exact output contract, required finding fields, and required reflection fields, follow [`references/review-rubric-playbook.md`](references/review-rubric-playbook.md).

## Verdict Rules

- `ready`: no P0 or A issues remain, any remaining B items are explicitly non-blocking for the stated venue/bar, and no late-round finding changes the smallest credible accept-path
- `conditional`: the paper has a plausible path to acceptance, but one or more B items, venue-fit uncertainties, or late-round defense risks remain
- `not ready`: any P0 or A issue remains, the evidence is too weak to support the current claims, or a late-round finding materially worsens the cleanest reject case
- In Hostile mode, avoid `ready` unless the Reject Case collapses to non-blocking polish and no central claim depends on unstated author clarification
- In default Full mode, prefer the strictest fair verdict rather than a reflexive reject posture
- If the manuscript is close but not ready, name the smallest set of changes that would most improve the decision
- If the venue asks for major/minor labels, translate them internally to P0/A/B/C rather than mixing schemes in the output

## Next step after review

- If fixes are broad, route the issue list to `$paper-reviser`.
- If fixes are narrow, route each issue to its owner skill directly.

## Hard constraints

- Do not collapse all issues into vague "needs improvement" language.
- Do not call a paper ready just because wording is polished.
- When rendered evidence exists, use artifact-grounded judgment rather than memory.
- Do not fabricate or assume experimental results.
- Be direct and specific; avoid "could consider" or "might benefit from" hedging.
- Do not skip empty template sections; keep the fixed output order and write `- None.` where a severity subsection has no findings.
- **Superior Quality Audit**: For high-fidelity peer review simulation, trigger `$execution-audit-codex` to verify critique against [Superior Quality Bar](../execution-audit-codex/references/superior-quality-bar.md).

## Trigger examples
- "强制进行论文评审深度审计 / 检查评审建议的专业性与可执行性。"
- "Use $execution-audit-codex to audit this review for critique-fidelity idealism."
- "按顶刊最狠审稿人的标准给我找茬，不要留情。"
- "站在 reject reviewer 角度审这篇，给我最刁钻的攻击点。"
