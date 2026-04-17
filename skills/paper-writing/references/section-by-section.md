# Section-by-Section Writing Strategy

## Abstract

**Goal**: One-paragraph pitch — background → gap → method → key result → implication.

**Execution checklist**:
- [ ] One paragraph
- [ ] Gap is explicit
- [ ] Method/action is named
- [ ] At least one key result signal is present
- [ ] Notation is minimized
- [ ] Implication is present without overclaim

**Rules**:
- Default to a single paragraph unless the venue requires a structured abstract
- ≤250 words for most venues; check submission guidelines
- Must contain at least one quantitative result
- Avoid abbreviations not universally known
- Avoid mathematical symbols and dense notation unless indispensable to the claim
- Do not cite references in the abstract (unless venue requires)
- First sentence = field-level context (not "In recent years…")
- Last sentence = significance or broader implication
- Make the abstract understandable to an informed reader outside the immediate subfield

**Common pitfalls**:
- Starting with "In this paper, we…" (too self-referential)
- Abstract is just a condensed introduction (should be self-standing)
- Missing quantitative claims (vague "significant improvement")
- Overloading the abstract with symbols, acronyms, or theorem-style shorthand

## Introduction

**Structure** (4–5 paragraph pattern):

1. **Context**: What is the broad problem? Why does it matter?
2. **Specific problem**: Narrow to your exact focus
3. **Existing approaches + limitations**: What has been tried? What gaps remain?
4. **Your contribution**: What do you propose? How does it differ?
5. **Results preview + paper organization** (optional)

**Rules**:
- End the introduction with explicit contribution bullets
- Each contribution must be verifiable in the experiments section
- Do not oversell: match claim strength to evidence strength
- Avoid "to the best of our knowledge, this is the first…" unless truly defensible
- For methods-heavy or engineering papers, make the final paragraph a roadmap of the remaining sections
- The roadmap paragraph should describe section function, not repeat contribution claims

**Roadmap paragraph pattern**:

> The remainder of this paper is organized as follows. Section II formulates ...

**Execution checklist**:
- [ ] Context is brief
- [ ] Gap is specific
- [ ] Contribution directly answers the gap
- [ ] Claim strength is calibrated
- [ ] Final roadmap paragraph is present when genre-appropriate
- [ ] Roadmap covers section functions only

## Related Work

**Structure**: Cluster by method family, not chronologically.

**Patterns**:
- "Method family A does X, but suffers from Y. Method family B addresses Y but introduces Z. We bridge A and B by…"
- Each cluster: core idea → representative papers → limitation → transition to next cluster
- Final paragraph: explicit positioning of your work relative to the closest clusters

**Rules**:
- Cover the last 2–3 years of strong competitors
- Do not weaken competitors unfairly (straw-man descriptions)
- Cite negative results and failures in the literature when relevant
- Route detailed literature building to `$literature-synthesis`

## Method

**Structure**: Mirror the architecture/pipeline in the order the reader needs.

**Rules**:
- Define all symbols at first use
- Include a notation table if >10 symbols
- Algorithm pseudocode must match equation symbols exactly
- State assumptions explicitly and early
- Separate novel components from established building blocks
- Include complexity analysis where appropriate

## Experiments

**Structure**:
1. Setup (datasets, metrics, baselines, implementation details)
2. Main results (comparison tables/figures)
3. Ablation studies (isolate each contribution)
4. Analysis (why it works, failure cases, qualitative examples)

**Rules**:
- Report Mean ± Std over ≥3 runs (≥5 preferred)
- Include significance tests for close comparisons
- Every claimed contribution must have a corresponding ablation
- Discuss negative results honestly
- Report computational cost (training time, memory, #params)

## Discussion / Analysis

**Rules**:
- Distinguish observation (what happened) from interpretation (why)
- Address limitations proactively before reviewers raise them
- Connect results back to the research question
- Discuss when/where the method might fail

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

**Execution checklist**:
- [ ] One or two paragraphs
- [ ] First paragraph states findings and contribution boundary
- [ ] No new results are introduced
- [ ] Limitations or scope are acknowledged
- [ ] Future work is concrete if included

## Claim Strength Ladder

Use appropriate hedging language based on evidence:

| Evidence Level | Language |
|----------------|----------|
| **Strong** (multiple experiments, statistical significance) | "demonstrate", "show", "establish" |
| **Moderate** (consistent trends, one experiment) | "suggest", "indicate", "provide evidence" |
| **Weak** (preliminary, single setting) | "appear to", "may", "initial results hint" |
| **Speculation** (no direct evidence) | "we hypothesize", "it is plausible", "one possibility is" |

Avoid mixing strong verbs with weak evidence. Reviewers catch this immediately.

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
- **Eliminate hedging**: Drop "may" and "can" unless genuinely uncertain
- **Avoid incremental vocabulary**: ❌ "combine," "modify," "expand" → ✅ "develop," "propose," "introduce"
- **Delete intensifiers**: ❌ "provides *very* tight approximation" → ✅ "provides tight approximation"

## Precision Over Brevity (Jacob Steinhardt)

- **Consistent terminology**: Different terms for same concept creates confusion. Pick one and stick with it.
- **State assumptions formally**: Before theorems, list all assumptions explicitly
- **Intuition + rigor**: Provide intuitive explanations alongside formal proofs
