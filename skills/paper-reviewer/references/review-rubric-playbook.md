# Paper Reviewer Playbook

This is the operating rubric for the gate-chain version of `paper-reviewer`.

## Source index

- NeurIPS reviewer guidelines: [https://neurips.cc/Conferences/2023/ReviewerGuidelines](https://neurips.cc/Conferences/2023/ReviewerGuidelines)
- ICML reviewer instructions: [https://icml.cc/Conferences/2025/ReviewerInstructions](https://icml.cc/Conferences/2025/ReviewerInstructions)
- Nature referee guidance: [https://www.nature.com/nature/for-referees](https://www.nature.com/nature/for-referees)
- Nature peer review policy: [https://www.nature.com/nature/editorial-policies/peer-review](https://www.nature.com/nature/editorial-policies/peer-review)
- COPE ethical guidelines for peer reviewers: [https://publicationethics.org/files/Ethical_Guidelines_For_Peer_Reviewers.pdf](https://publicationethics.org/files/Ethical_Guidelines_For_Peer_Reviewers.pdf)
- NIH reviewer guidance: [https://grants.nih.gov/policy-and-compliance/policy-topics/peer-review/simplifying-review/reviewer-guidance](https://grants.nih.gov/policy-and-compliance/policy-topics/peer-review/simplifying-review/reviewer-guidance)
- NIH review criteria: [https://www.niaid.nih.gov/research/review-criteria](https://www.niaid.nih.gov/research/review-criteria)

## Core stance

- Review by abstract dimension, not by manuscript section.
- Lock the target contract before any scientific judgment.
- If the target contract is missing, infer a provisional bar and continue; do
  not stall the review on a missing venue unless the user asked for exact
  submission compliance.
- External research is allowed and expected when it improves novelty,
  baseline, citation, or venue calibration.
- Use the strictest honest standard at every gate.
- Freeze earlier gate conclusions once they pass.
- Later gates improve quality but do not silently renegotiate evidence, claims,
  or routing decisions made upstream.

## Fast interactive review path

Use this path unless the user explicitly asks for filesystem-backed gate
artifacts or a multi-turn review protocol.

1. **Verdict first**: `可投 / 大修后再投 / 不建议投 / 需要补关键证据`.
2. **Claim map**: main claim, contribution bullets, decisive evidence, and the
   main decision pressure point.
3. **External calibration**: closest prior work, must-have baselines, venue or
   article-type norm, citation currency.
4. **Primary decision risk**: the shortest evidence-based path by which
   reviewers may judge the manuscript unready.
5. **Fix routing**: new evidence, claim narrowing, appendix move, citation
   repair, visual/layout repair, or prose cleanup.

Default user-facing output should be compact:

```text
结论：...
最危险的 3 个问题：...
外部调研校准：...
下一步：...
```

Only expose gate ids, artifact folders, frozen-input language, or per-gate file
names when the user requested protocol artifacts, long-running review, or
repeatable disk state.

## External research discipline

Allowed by default:

- search target venue / journal reviewer and author instructions
- search recent closest prior work and baselines
- verify references through DOI, publisher, proceedings, PubMed/PMC, arXiv, or
  scholarly indexes
- inspect public PDFs, abstracts, code repositories, dataset cards, and artifact
  checklists relevant to review risk

Confidentiality limit:

- do not upload an unpublished full manuscript, private data, or confidential
  review material to public AI tools, plagiarism detectors, or third-party
  services without explicit user approval
- search from title, abstract, keywords, claim snippets, citation strings, and
  public metadata when possible

Research bar:

- prefer official venue / publisher pages and primary paper records
- use Semantic Scholar, OpenAlex, Crossref, Google Scholar, Connected Papers,
  or Litmaps for discovery and expansion, not as the final authority when a
  primary source exists
- label conclusions as provisional when the search is shallow, the field is
  fast-moving, or paywalled metadata prevents confirmation
- cite only sources that materially change the review decision; do not dump a
  bibliography for decoration

## Shared persistence contract

Use the runtime artifact protocol in [`../../PAPER_GATE_PROTOCOL.md`](../../PAPER_GATE_PROTOCOL.md).

Key persistence rules:

- `paper_ref/` is created once and reused unless the target contract changes materially.
- `paper_review_v<N>/` is the overall review-round directory.
- Every turn creates one new gate checklist file.
- Old gate files are never overwritten.

## Scope modes

- `full_chain` is the default when the user does not name a specific gate or dimension.
- `single_gate` is valid only when the user explicitly names one gate or dimension.

In `single_gate`:

- review only that gate
- do not silently backfill other gates
- record upstream assumptions as `assumed_frozen_inputs` when needed
- still create exactly one new non-overwriting gate file

Quick routing map:

- vague review asks such as "帮我审这篇 paper", "能不能投", or "投稿前把关" -> `full_chain`
- explicit dimension asks such as claim, math, citations, appendix routing, front-door text, notation, figures, tables, language, or layout -> `single_gate`
- stress-test wording such as "最狠审稿人", "strict reviewer", or "对抗性找茬" -> same scope selection as above, plus `Stress-Test`

## Review-strength preservation

To prevent repeated review passes from becoming softer over time:

- use a fresh isolated reviewer worker for each gate pass
- pass state only through the markdown packet
- do not rely on accumulated chat memory as the review source of truth

In practice, the markdown packet should contain:

- `paper_ref/TARGET_CONTRACT.md`
- latest ref-pool manifest
- the active gate file
- the upstream gate files named in `Frozen Inputs`

If subagent spawning is unavailable, simulate the same isolation by re-reading
only those markdown files and ignoring prior narrative state.

## Heartbeat wrapper

For autonomous full-chain review mode, the wrapper contract is:

- 5-minute heartbeat cadence
- one gate-round advancement per tick
- fresh isolated reviewer worker each tick
- markdown-only state transfer across ticks

## Gate order and what each gate must prove

### G0 Target Contract + Ref Bootstrap

What must be true:

- target venue, article type, audience, and page/disclosure contract are explicit
- the review is anchored to that contract, not to a generic venue fantasy
- `paper_ref/` exists or is refreshed with target-journal-first local PDFs

Failure signals:

- no target venue or article type
- benchmark pool built from arbitrary topic papers rather than target-venue-near papers
- candidate papers listed but not locally downloadable as PDFs

### G1 Fatal Eligibility

What must be true:

- no integrity, scope, provenance, or required-disclosure fatal
- no result provenance gap that would make the paper unreviewable on arrival

Failure signals:

- hidden provenance or unverifiable results
- missing mandatory disclosure or ethics statement
- article type / venue mismatch that cannot be repaired by prose polish

### G2 Core Evidence Freeze

What must be true:

- main tables, figures, key numbers, and decisive ablations are the strongest
  honest evidentiary backbone available
- comparisons are fair
- the statistical story is self-consistent

Failure signals:

- strongest baseline omitted
- fairness mismatch in compute, tuning, data, or metric definitions
- key ablation or robustness control missing for the central claim
- headline number survives only because the main table is weakly designed

### G3 Claim Ceiling & Article Fit

What must be true:

- `claim_floor`, `claim_ceiling`, and `selected_claim_level` are explicit
- the chosen claim is the highest honest claim that can still support the
  intended article type

Failure signals:

- claim overshoots evidence
- claim is needlessly crushed below what the evidence can honestly support
- contribution bullets are misaligned with the journal or article contract

### G4 Formal / Math Closure without Overmath

What must be true:

- every proof-dependent claim is formally closed when the surviving claim needs it
- unnecessary math has been removed or downgraded

Failure signals:

- theorem or derivation has a logical hole
- mechanism claim depends on math that is only suggestive
- draft carries decorative math that does not strengthen a surviving claim

### G5 Reference Support & Venue Calibration

What must be true:

- citations are real, precise, and venue-calibrated
- citation clusters do not run longer than 3 consecutive references
- the reference mix is recent by default and close to the target venue

Failure signals:

- fake or unverifiable references
- citation dumping instead of claim-level support
- reference portfolio talks to the wrong venue conversation

### G6 Main Text vs Appendix Routing

What must be true:

- main text contains only the material required to support the surviving claim
- extra experiments, extended proofs, and secondary visuals have a deliberate
  appendix or deletion decision

Failure signals:

- main text is burdened by low-leverage side material
- key support is buried in appendix
- appendix is being used to evade an honest claim downgrade

### G7 Narrative Spine & Main-text Flow

What must be true (operational pass bar):

- every main-text paragraph is assigned exactly one role in
  `narrative_spine_map` (`setup`, `method`, `evidence`, `limitation`,
  `transition`, or `takeaway`)
- every `evidence` paragraph is linked to at least one surviving `claim_id` in
  `claim_ledger`
- no adjacent paragraph pair has the same role unless explicitly justified as a
  split continuation in `narrative_spine_map`
- all paragraphs routed to appendix by G6 are absent from the main text in this
  round

Required check artifacts:

- `narrative_spine_map.md`: ordered list `p_id -> section -> role -> claim_id(s)`
- `narrative_flow_breaks.md`: all detected breaks with `p_id`, break type, and
  repair action
- `main_vs_appendix_presence_check.md`: list of G6-routed items and whether they
  still appear in main text (`yes/no`)

Fail criteria (any one triggers fail and backjump to G6, or G3 if claim linkage breaks):

- one or more main-text paragraphs have no role assignment
- one or more `evidence` paragraphs have no linked surviving `claim_id`
- unresolved flow breaks remain in `narrative_flow_breaks.md`
- appendix-routed content still appears in main text

### G8 Front-door Text Gate

What must be true (operational pass bar):

- title, abstract, introduction, and conclusion are all enumerated as
  `front_door_surface` objects
- every claim sentence in those four surfaces is mapped to exactly one
  surviving `claim_id` and `selected_claim_level`
- zero claim sentences in front-door surfaces exceed `selected_claim_level`
- conclusion introduces no new `claim_id` not already present in abstract or
  introduction

Required check artifacts:

- `front_door_claim_matrix.md`: table
  `surface -> sentence_id -> claim_id -> claim_level -> verdict(pass/fail)`
- `front_door_overshoot_report.md`: every overshoot sentence with proposed
  downgrade text
- `front_door_new_claim_check.md`: conclusion-only claims and resolution status

Fail criteria (any one triggers fail and backjump to G3):

- any front-door claim sentence is unmapped to `claim_id`
- any mapped sentence is above `selected_claim_level`
- conclusion contains a new unresolved claim absent from abstract/introduction
- matrix and checks are missing or incomplete

### G9 Mirror & Text Consistency

What must be true (operational pass bar):

- all mirrored surfaces are explicitly enumerated (`abstract`, `contributions`,
  `figure captions`, `table titles`, `limitations`, and any rebuttal carry-over
  text)
- each mirrored statement is mapped to one surviving `claim_id` and one evidence
  anchor from `evidence_anchor_map`
- all figure/table callouts in body text resolve to existing objects and match
  caption claim scope
- zero stale references remain to deleted/narrowed claims

Required check artifacts:

- `mirror_surface_inventory.md`: list `surface -> unit_id -> text_span`
- `mirror_claim_alignment.md`: table
  `unit_id -> claim_id -> evidence_anchor -> status(pass/fail)`
- `callout_caption_consistency.md`: table
  `callout_id -> object_id -> caption_scope_match(pass/fail)`

Fail criteria (any one triggers fail and backjump to G3 or G6 depending on root cause):

- any mirrored statement is unmapped to surviving claim/evidence
- any unit still advertises deleted or narrowed claim language
- any callout/caption mismatch remains unresolved
- required mirror artifacts are missing

### G10 Terminology / Notation / Symbol Consistency

What must be true:

- terminology and symbol usage are globally consistent
- units, equation references, and abbreviations are clean

Failure signals:

- same object named differently across sections
- symbol collision or undefined notation
- table / figure units disagree with body text

### G11 Figure Gate at Final Scale

What must be true:

- every surviving figure is judged at real rendered scale
- single- vs double-column choice is explicit
- single column wins when it remains clear and more compact

Failure signals:

- only the source image looks good; the rendered scale fails
- labels or titles are illegible
- layout feels cramped, obstructed, or visually cheap

### G12 Table Gate at Final Scale

What must be true:

- every surviving table is judged at real rendered scale
- statistical framing and title language are explicit
- wrapping, alignment, and compactness work in the final layout

Failure signals:

- header or cell wrapping destroys readability
- statistic definition is unclear
- the table is visually bloated or badly aligned

### G13 Language Naturalness & Defense Posture

What must be true:

- prose is natural, smooth, and restrained
- tone is not defensive and not promotional

Failure signals:

- AI-sounding transition clusters
- rebuttal-style defensiveness inside the paper
- stiff or over-signposted sentence rhythm

### G14 Rendered Layout & Page Economy

What must be true:

- the final PDF render has no cuts, crowding, hollows, or broken float flow
- page economy and readability are jointly optimized

Failure signals:

- figures/tables fit poorly in the actual PDF
- empty whitespace or clogged pages
- single/double-column decisions work against narrative flow

## Stress-Test overlay

Use stress-test mode only when explicitly requested.

Stress-test mode adds:

- a compact `Decision Risk Case`
- sharper counterfactual challenge checks on the current gate
- stronger pressure on fairness, venue bar, and self-sufficiency

Stress-test mode does not:

- change gate order
- skip G0
- authorize fake certainty
- hide the honest accept path

Stress-test can apply on top of either `full_chain` or `single_gate`, but it still
does not authorize cross-gate drift.

## Gate file quality bar

Every generated `gate_r<M>.md` must:

- be executable as a checklist
- name frozen upstream inputs explicitly
- identify stable `unit_type:unit_id` review objects
- state the hard bar in reviewer language
- constrain the legal decision output to the gate kind
- predeclare the next file for pass and fail paths

## Output shape

Default user-facing response shape:

1. readiness verdict
2. top 3 readiness risks
3. decisive evidence gaps
4. external calibration deltas (only when used)
5. next revision move
6. optional `Decision Risk Case` only when explicitly requested

Protocol response shape (only when protocol artifacts are requested):

1. review scope
2. current review round folder
3. target-contract status
4. benchmark-pool status
5. current gate judgment
6. freeze/backjump state
7. next gate file created
8. stress-test `Decision Risk Case` only when requested

Do not default to protocol state reporting unless explicitly requested.
