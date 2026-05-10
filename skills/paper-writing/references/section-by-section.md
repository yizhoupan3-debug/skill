# Section-by-Section Writing Strategy

## Start With the Reader Path

Before rewriting a section, identify the reader's path in one sentence:

```text
Given [field context], readers need to understand [gap/problem], so this section shows [paper's answer] using [evidence].
```

Use that path to decide what to keep, move, cut, or mark as missing. Do not write in the order the authors discovered things; write in the order a reader needs to understand and trust the claim.

For full-manuscript rewrites, reuse the canonical throughline from `../SKILL.md`:

- `core_problem -> bottleneck -> paper_move -> decisive_evidence -> bounded_implication`

Section-level checks:

- each section advances exactly one adjacent link in this chain
- each section ends with a handoff to the next reader question
- no section introduces a second headline contribution

## IMRaD Logic

For empirical papers, keep the Introduction-Methods-Results-Discussion split clean:

| Section | Reader question | Main job | Avoid |
| --- | --- | --- | --- |
| Introduction | Why does this problem matter, and what gap remains? | motivate the gap and preview the answer | full methods detail or result-by-result narration |
| Methods | What exactly was done? | make the approach understandable and trustworthy | selling the contribution with unsupported claims |
| Results | What was observed? | report evidence in a clear order | broad interpretation before readers see the data |
| Discussion | What do the results mean? | interpret, compare, limit, and generalize carefully | adding new results or hiding limitations |

When a section feels muddy, it often contains another section's job.

## Context-Content-Conclusion Pattern

Use the C-C-C pattern at three scales:

| Scale | Context | Content | Conclusion |
| --- | --- | --- | --- |
| Whole paper | broad field and gap | method/results | answer and implication |
| Section | why this section exists | main technical or empirical material | what the reader should carry forward |
| Paragraph | link to previous idea | one point with support | takeaway or transition |

Common failures:
- context missing -> reader asks "why am I being told this?"
- conclusion missing -> reader asks "so what?"
- content before context -> result appears before the reader knows what problem it answers
- too many loose threads -> the paper feels like a list rather than an argument

At section boundaries, use a one-sentence handoff that pulls the same main line
forward:

- Introduction -> Methods: "To resolve this bottleneck, we implement..."
- Methods -> Results: "We evaluate this design under..."
- Results -> Discussion: "These findings imply..."
- Discussion -> Conclusion: "Under this scope, the paper establishes..."

Avoid handoffs that start a new storyline unrelated to the central contribution.

## Abstract

**Goal**: a self-standing miniature paper: background -> gap/objective -> method -> key result -> implication.

**Execution checklist**:
- [ ] Venue word count and structured/unstructured format are known
- [ ] Gap is explicit
- [ ] Method/action is named
- [ ] Key result is concrete; numeric if the field and evidence support it
- [ ] Notation is minimized
- [ ] Implication is present without overclaim

**Rules**:
- Default to a single paragraph unless the venue requires headings
- Check submission guidelines before enforcing a word count
- Prefer concrete numbers, effect sizes, uncertainty, or scope markers over vague "significant improvement"
- Avoid abbreviations not universally known
- Avoid mathematical symbols and dense notation unless indispensable to the claim
- Keep the abstract citation-free unless the venue requires references there
- First sentence = field-level context; avoid empty openers such as "In recent years..."
- Last sentence = significance or broader implication
- Make the abstract understandable to an informed reader outside the immediate subfield
- If drafting from notes, use `[VERIFY: exact number/result]` rather than inventing metrics

**Common pitfalls**:
- Starting with "In this paper, we…" (too self-referential)
- Abstract is just a condensed introduction (should be self-standing)
- Missing quantitative claims (vague "significant improvement")
- Overloading the abstract with symbols, acronyms, or theorem-style shorthand
- Adding citations, undefined abbreviations, or claims that do not appear in the main text

**Common templates**:

For empirical work:

```text
[Context/problem]. However, [specific gap]. Here we [method/action] using [data/setting]. [Key result with scope]. These findings [implication without overclaim].
```

For methods work:

```text
[Problem setting] requires [capability]. Existing methods [limitation]. We introduce [method], which [core mechanism]. On [benchmarks/tasks], [result]. The approach [bounded implication].
```

## Introduction

**Structure** (4–5 paragraph pattern):

1. **Context**: What is the broad problem? Why does it matter?
2. **Specific problem**: Narrow to your exact focus
3. **Existing approaches + limitations**: What has been tried? What gaps remain?
4. **Your contribution**: What do you propose? How does it differ?
5. **Results preview + paper organization** (optional)

**Rules**:
- End the introduction with explicit contributions; bullets are useful when venue/field conventions allow them
- Each contribution must be verifiable in the experiments section
- Match claim strength to evidence strength
- Avoid priority claims such as "the first" unless truly defensible and supported by the literature
- For methods-heavy or engineering papers, make the final paragraph a roadmap of the remaining sections
- The roadmap paragraph should describe section function, not repeat contribution claims
- Keep the introduction selective; each prior-work detail should help define the gap

**Roadmap paragraph pattern**:

> The remainder of this paper is organized as follows. Section II formulates ...

**Execution checklist**:
- [ ] Context is brief
- [ ] Gap is specific
- [ ] Contribution directly answers the gap
- [ ] Claim strength is calibrated
- [ ] Closest prior work is represented fairly
- [ ] Results preview matches the experiments
- [ ] Final roadmap paragraph is present when genre-appropriate
- [ ] Roadmap covers section functions only

**Gap paragraph pattern**:

```text
Prior work has [what it can do]. However, [specific limitation] remains unresolved because [mechanism or evidence]. This gap matters because [consequence]. We address this gap by [paper's move].
```

**Contribution bullet pattern**:

```text
Our contributions are:
- We [technical contribution], enabling [capability] under [scope].
- We [evaluation contribution], showing [result type] on [setting].
- We [analysis/resource contribution], providing [artifact/insight] for [use].
```

## Related Work

**Structure**: Cluster by method family, not chronologically.

**Patterns**:
- "Method family A does X, but suffers from Y. Method family B addresses Y but introduces Z. We bridge A and B by…"
- Each cluster: core idea → representative papers → limitation → transition to next cluster
- Final paragraph: explicit positioning of your work relative to the closest clusters

**Rules**:
- Cover closest competitors and recent strong work; do not cite only old or convenient papers
- Do not weaken competitors unfairly (straw-man descriptions)
- Include negative results, failures, and scope limits in the literature when relevant
- If a citation is missing, write `[VERIFY: citation for ...]`; never fabricate references
- Route detailed literature building to the research synthesis runtime lane.
- Use "diff" language precisely: same problem/different method, same method/different setting, or same goal/different assumption

## Method

**Structure**: Mirror the architecture/pipeline in the order the reader needs.

**Recommended order**:

1. Problem formulation and assumptions
2. Overview of the approach
3. Components in pipeline order
4. Training/inference or implementation details
5. Complexity, guarantees, or reproducibility details when relevant

**Rules**:
- Define all symbols at first use
- Include a notation table if >10 symbols
- Algorithm pseudocode must match equation symbols exactly
- State assumptions explicitly and early
- Separate novel components from established building blocks
- Include complexity analysis where appropriate
- Use forward references sparingly; if readers need a concept now, define it now
- If the method includes prompts, data filters, thresholds, or implementation choices that affect reproducibility, describe them concretely
- Use equations for precision, but surround them with plain-language purpose and interpretation

## Experiments

**Structure**:
1. Setup (datasets, metrics, baselines, implementation details)
2. Main results (comparison tables/figures)
3. Ablation studies (isolate each contribution)
4. Analysis (why it works, failure cases, qualitative examples)

**Rules**:
- For stochastic empirical work, report variation across runs when feasible
- Include statistical tests or uncertainty intervals for close comparisons when expected in the field
- Every claimed contribution should have corresponding evidence, often an ablation or controlled comparison
- Discuss negative results honestly
- Report computational cost when it affects fairness, reproducibility, or practical use
- Results sections describe what happened; save broad interpretation for Discussion unless the venue combines the two
- Keep comparisons fair: same data split, metric, preprocessing, tuning budget, and hardware assumptions where possible

**Results paragraph pattern**:

```text
Table/Figure X compares [methods] on [task/metric]. [Main observation with number/scope]. The improvement is largest/smallest when [condition], suggesting [brief interpretation if appropriate]. [Caveat or transition].
```

## Discussion / Analysis

**Rules**:
- Distinguish observation (what happened) from interpretation (why)
- Address limitations proactively before reviewers raise them
- Connect results back to the research question
- Discuss when/where the method might fail
- Explain how conclusions affect existing assumptions, models, practices, or open questions in the field
- Future work should follow from a limitation or unresolved question, not from generic ambition

**Discussion paragraph pattern**:

```text
The results support [bounded claim], but they do not establish [over-broad claim]. One likely explanation is [mechanism], consistent with [evidence]. This interpretation is limited by [scope], which future work could test by [specific next step].
```

## Conclusion

**Rules**:
- Do not copy-paste the abstract
- Rise to design-principle-level insights
- Acknowledge limitations
- Suggest specific future work (not vague "future work includes…")
- Final sentence: broader impact or take-home message
- Default to one or two paragraphs total
- Paragraph 1: answer the research question and state the contribution boundary
- Optional paragraph 2: limitations, scope conditions, and concrete next steps

**Common pitfalls**:
- Repeating the abstract sentence by sentence
- Ending with unsupported hype or broad societal claims
- Adding new technical results in the closing paragraph
- Claiming generality beyond the studied setting

**Execution checklist**:
- [ ] One or two paragraphs
- [ ] First paragraph states findings and contribution boundary
- [ ] No new results are introduced
- [ ] Limitations or scope are acknowledged
- [ ] Future work is concrete if included

## Captions

**Goal**: Make the figure/table understandable without forcing the reader to hunt through the text.

**Rules**:
- State what is shown, what comparison is made, and what the key takeaway is.
- Define non-obvious abbreviations, axes, units, error bars, and sample sizes.
- For tables, explain bolding, arrows, statistical markers, and missing values.
- Do not over-interpret beyond the visual evidence.

**Template**:

```text
Figure X. [What is plotted] for [data/setting]. [Encoding: axes, colors, markers, error bars]. [Key takeaway with scope].
```

## Claim Strength Ladder

Use appropriate hedging language based on evidence:

| Evidence Level | Language |
|----------------|----------|
| **Strong** (multiple experiments, statistical significance) | "demonstrate", "show", "establish" |
| **Moderate** (consistent trends, one experiment) | "suggest", "indicate", "provide evidence" |
| **Weak** (preliminary, single setting) | "appear to", "may", "initial results hint" |
| **Speculation** (no direct evidence) | "we hypothesize", "it is plausible", "one possibility is" |

Avoid mixing strong verbs with weak evidence. Reviewers catch this immediately.

## Draft-from-Notes Protocol

When the user asks to "write" rather than "polish", proceed only from supplied ingredients:

1. Extract facts: problem, gap, method, evidence, limitations, target venue/audience.
2. Mark missing ingredients as `[VERIFY: ...]`.
3. Draft a clean version without adding citations, metrics, baselines, datasets, or claims.
4. Add a short "needs confirmation" note only if missing facts affect scientific validity.

Useful placeholders:
- `[VERIFY: exact dataset/task]`
- `[VERIFY: numeric result and uncertainty]`
- `[VERIFY: closest prior work citation]`
- `[VERIFY: claim scope]`

## Sentence-Level Clarity (Gopen & Swan 7 Principles)

These principles are based on how readers actually process prose. Violating them forces readers to spend cognitive effort on structure rather than content.

| Principle | Rule | Example |
|-----------|------|---------|
| **Subject-verb proximity** | Keep subject and verb close | ❌ "The model, which was trained on..., achieves" → ✅ "The model achieves... after training on..." |
| **Stress position** | Place emphasis at sentence ends | ❌ "Accuracy improves by 15% when using attention" → ✅ "When using attention, accuracy improves by **15%**" |
| **Topic position** | Put context first, new info after | ✅ "Given these constraints, we propose..." |
| **Old before new** | Familiar info → unfamiliar info | Link backward, then introduce new |
| **One unit, one function** | Each paragraph makes one point | Split multi-point paragraphs |
| **Action in verb** | Use verbs, not nominalizations | ❌ "We performed an analysis" → ✅ "We analyzed" |
| **Context before new** | Set stage before presenting | Explain before showing equation |

## Micro-Level Writing Tips (Ethan Perez)

Small changes that accumulate into significantly clearer prose:

- **Minimize pronouns**: ❌ "This shows..." → ✅ "This result shows..."
- **Verbs early**: Position verbs near sentence start
- **Unfold apostrophes**: ❌ "X's Y" → ✅ "The Y of X" (when awkward)
- **Delete filler words**: "actually," "a bit," "very," "really," "basically," "quite," "essentially"

## Word Choice (Zachary Lipton)

- **Be specific**: ❌ "performance" → ✅ "accuracy" or "latency" (say what you mean)
- **Calibrate hedging**: Use "may/can/suggest" only when uncertainty is real; otherwise use bounded scope markers
- **Match verbs to contribution posture**: choose among "introduce", "extend", "validate", "explain", "operationalize" based on evidence strength
- **Delete intensifiers**: ❌ "provides *very* tight approximation" → ✅ "provides tight approximation"

## Cadence QC (Top-tier Flow)

Before finalizing a rewritten section, run this quick rhythm check:

- sentence-length mix: avoid all-short or all-long sequences
- paragraph-opening mix: rotate claim-led, evidence-led, and contrast-led openings
- end-stress check: place one key idea near sentence end when emphasis matters
- transition variety: mix contrast, cause, scope, and evidence connectors
- repetition check: avoid repeating the same opener pattern for 3+ sentences

## Precision Over Brevity (Jacob Steinhardt)

- **Consistent terminology**: Different terms for same concept creates confusion. Pick one and stick with it.
- **State assumptions formally**: Before theorems, list all assumptions explicitly
- **Intuition + rigor**: Provide intuitive explanations alongside formal proofs

## AI-Assisted Writing Compliance

When the final text may be submitted to a journal or conference:

- Remind the author to check the target venue's AI policy.
- If disclosure is required, keep it factual: tool, scope of use, and human review.
- AI tools are not authors; human authors remain responsible for accuracy, originality, citations, and permissions.
- Do not upload confidential reviewer manuscripts, decision letters, or third-party unpublished material into external AI tools unless the author has authorization.
