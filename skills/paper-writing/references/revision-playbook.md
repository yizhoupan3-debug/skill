# Paper Revision Playbook

Use this file when a task is more than light grammar polish: drafting from notes,
structural rewriting, paper-specific de-AI cleanup, or submission-facing prose.

## Rewrite Ladder

Move down this ladder only as far as the text needs:

1. **Purpose**: What is this section or paragraph trying to make the reader believe or understand?
2. **Reader path**: Does the order match how a skeptical reader builds trust?
3. **Claim boundary**: Are verbs and scope markers calibrated to the evidence?
4. **Paragraph job**: Does each paragraph have one job, one topic sentence, and one takeaway?
5. **Sentence clarity**: Are subject, verb, and emphasis easy to find?
6. **Surface polish**: Grammar, concision, rhythm, terminology, and transitions.

Do not spend the whole pass at level 6 if the problem is level 1-3.

For whole-manuscript tightening, run one coherence pass before sentence polish:

1. lock the canonical throughline from `../SKILL.md`
   (`core_problem -> bottleneck -> paper_move -> decisive_evidence -> bounded_implication`)
2. ensure each section has one job on that chain
3. remove or relocate paragraphs that open a second headline story
4. add section handoff lines that keep momentum

## Input Triage

Extract these before writing:

- **Target unit**: abstract, introduction, related work, method, result, discussion, conclusion, caption, response letter.
- **Mode**: polish, structural rewrite, draft from notes, response prose, paper de-AI cleanup.
- **Audience**: specialist reviewers, interdisciplinary readers, journal editors, conference reviewers, thesis committee.
- **Allowed facts**: manuscript text, author notes, tables/figures, verified citations, reviewer comments.
- **Forbidden changes**: claims not to alter, terms to preserve, citations not to invent, length/format constraints.
- **Risk flags**: missing result, missing citation, unsupported novelty, confidentiality, venue AI policy.
- **Story assets**: target-journal ref-learning brief, closest competitors, intended contribution posture, and one-sentence story spine.

If the target venue is known, adapt to its word limits, abstract structure, figure/caption conventions, response-letter format, and AI-use policy.

## Common User Phrases

Treat these as paper-writing requests when the text unit is bounded and the science is supplied:

- Chinese: 论文润色, 英文论文润色, 学术润色, SCI润色, SCI论文写作, 论文修改, 论文精修, 论文改写, 论文表达优化, 语言润色, 学术表达润色, 段落衔接, 逻辑衔接, 摘要重写, 引言重写, 图注润色, 表注润色, 审稿回复润色, 回复审稿人, 投稿 cover letter, 论文降AI味, 去AI味, AIGC润色.
- English: manuscript editing, manuscript polish, paper editing, paper polishing, academic writing, scientific writing, research paper writing, language editing, proofread manuscript, copyedit manuscript, improve clarity, improve flow, improve coherence, claim calibration, hedge claims, abstract rewrite, introduction editing, methods writing, results writing, discussion writing, conclusion rewrite, rebuttal letter, response to reviewers, point-by-point response, submission cover letter.

Near misses:

- "找文献", "related work 缺文献" -> keep the work under `$paper-workbench` (ref-first / external calibration); use `$citation-management` only when the active object is bibliography hygiene.
- "创新性够不够", "实验站不站得住" -> route to paper logic/reviewer skills.
- "整篇能不能投", "投稿前把关" -> route to paper workbench/reviewer.
- "查重降重" with intent to conceal copying -> refuse that part; offer legitimate clarity/originality revision.

## Output Patterns

### Simple Polish

Return only:

```text
[Revised text]
```

Add notes only if the source has unsupported claims, ambiguous facts, or missing placeholders.

### Structural Rewrite

Return:

```text
[Revised text]

Notes:
- [Only unresolved scientific or verification risk]
```

Do not explain every wording choice.

### Storyline Rewrite

Return:

```text
Story spine:
[one sentence]

[Revised text]

Notes:
- [Only unresolved story, evidence, or venue-fit risk]
```

Use this when the main problem is not grammar but the reader cannot see why the work matters, why the gap survives prior work, or why the evidence supports the takeaway.

### Ref-Guided Venue Rewrite

Return:

```text
Ref-learned pattern:
- [3-5 bullets about target-journal story norms]

[Revised text]

Guardrails:
- [VERIFY: missing fact/citation/result]
```

Do not cite the 20-paper corpus mechanically. Use it to shape architecture, claim tone, evidence order, and section balance.

### Draft From Notes

Return:

```text
[Draft text with [VERIFY: ...] placeholders]

Needs confirmation:
- [Missing fact that affects correctness]
```

Never fill a missing metric, citation, dataset, baseline, or limitation from general knowledge.

### Rebuttal / Response Letter

Return action-first prose:

```text
We have [added/revised/clarified] [specific change] in [location]. The revised text now [effect]. [Evidence or result if available].
```

Use "clarified" when the work did not change, "added" only when new material exists, and "revised" only when the manuscript text changed.

## Paper Voice

Good academic prose is specific, modest, and easy to verify.

For top-tier narrative pacing, make the prose direct and forward-driving:

- open with contribution or finding, then evidence, then implication;
- avoid defensive framing as the default sentence posture;
- keep paragraph momentum by limiting throat-clearing transitions.

Prefer:
- concrete nouns over vague bundles such as "this issue", "these aspects", "the aforementioned";
- active verbs when the actor matters;
- scope markers such as "in this setting", "on these datasets", "under this assumption";
- short transitions that explain the logical relation, not generic connectors;
- repeated key terms over synonym swapping when precision matters.

Prefer replacing weak patterns with stronger ones:
- replace empty openings with concrete context in one clause;
- replace hype words with measurable findings or bounded capability statements;
- replace throat-clearing with direct logical transitions;
- replace vague summaries with named variables, datasets, methods, or mechanisms;
- replace unsupported priority words with contribution posture grounded in evidence;
- replace defensive scaffolding with contribution-first plus explicit boundary cues;
- replace process narration with manuscript-facing scientific statements.

Exception rule:

- these are defaults, not absolute bans; keep wording required for factual limitation disclosure, venue conventions, or precise reviewer-facing scope clarification.

QC checks:

- cadence: vary sentence lengths and paragraph openings; avoid repetitive transitions
- unity: summarize the manuscript in one sentence without contradiction
- mapping: each results block maps to one promised contribution
- closure: discussion preserves scope while reinforcing the same main line

## Paragraph Repair Patterns

### Missing Topic Sentence

Bad shape:

```text
[detail] [detail] [method] [result]
```

Repair:

```text
[Claim/topic sentence]. [Evidence or method detail]. [Result or implication]. [Transition if needed].
```

### Zig-Zag Logic

If a paragraph alternates problem -> method -> problem -> result, regroup it:

```text
Problem/gap -> why existing work falls short -> proposed move -> evidence/takeaway
```

### Weak Transition

Replace generic transitions with relation-specific ones:

- contrast: "However, this assumption breaks down when..."
- consequence: "This limitation makes it difficult to..."
- bridge: "To address this gap, we..."
- scope: "In the setting considered here,..."
- evidence: "Empirically, this design leads to..."

## Claim Calibration

Match verbs to evidence:

- **Directly proven or strongly replicated**: demonstrate, establish, show
- **Consistent empirical pattern**: indicate, suggest, provide evidence that
- **Single setting or preliminary evidence**: suggest, appear to, are consistent with
- **Interpretation**: may explain, could reflect, is one possible reason
- **No direct evidence**: we hypothesize, we conjecture, future work should test

Top-tier force rule:

- If evidence is strong enough for "show/demonstrate/establish", do not downgrade
  to weaker verbs just to sound cautious.
- If evidence is not strong enough, narrow scope explicitly rather than writing a
  long defensive disclaimer.

Scope markers are often better than hedging:

- "improves accuracy on all three evaluated datasets" is clearer than "may improve performance".
- "under the assumptions of Section 3" is safer than "generally works".

## Multi-Round Drift Guard

Before each new revision round, run this quick guard:

1. Compare changed sentences against the current claim ledger.
2. Check whether any verb/scope marker implies stronger causality or
   generalization.
3. Verify each strengthened phrase has a matching evidence anchor update.
4. If not, revert wording strength and emit `[VERIFY: claim drift risk]`.

Never trade away auditable experiment detail just to improve narrative smoothness.

## De-AI Cleanup

Paper-specific de-AI cleanup should improve truthfulness and specificity, not merely evade detection.

Steps:

1. Remove formulaic openings and vague global claims.
2. Replace broad nouns with field-specific objects, variables, datasets, or mechanisms from the supplied text.
3. Break over-balanced "not only... but also..." structures when they do not reflect real contrast.
4. Add real logical relations: cause, contrast, scope, evidence, limitation.
5. Preserve repeated technical terms; do not add synonym variety that blurs meaning.
6. Keep modest tone. Do not make prose more confident than the evidence.

## Submission and Ethics Checks

Flag these when visible:

- target venue may require AI-writing disclosure;
- manuscript or reviewer material may be confidential;
- citations, images, datasets, or third-party text may need permission;
- word count, structured abstract, or response-letter formatting may be venue-specific;
- generated text contains `[VERIFY: ...]` placeholders that must be resolved before submission.
