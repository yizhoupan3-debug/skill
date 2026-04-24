# Rebuttal & Response Letter Patterns

## Core Response Rule

Every response should make the action clear before the explanation:

```text
Concern -> action taken -> evidence/result -> where changed -> remaining scope if any
```

If no manuscript change was made, say why and provide evidence. Do not imply that a change was made when it was only considered.

## Triage Order

Handle comments in this order:

1. Fatal scientific concerns: invalid claim, missing baseline, flawed experiment, unsupported novelty.
2. Fixable evidence gaps: ablation, robustness, significance, reproducibility, additional analysis.
3. Clarity and organization: unclear motivation, confusing method, missing limitations, poor figures.
4. Surface edits: grammar, notation, formatting, minor citation cleanup.

Do not polish a response to a fatal concern before the manuscript action is decided.

## Common Reviewer Attack Patterns and Response Templates

### 1. "Novelty is insufficient"

**Typical phrasing**: "The contribution is incremental", "This is a straightforward combination of X and Y"

**Response strategy**:
- Acknowledge the reviewer's perspective respectfully
- Identify the specific technical novelty that was underappreciated
- Provide a concrete comparison to the closest prior work
- Quantify the difference (performance gap, capability gap, or conceptual gap)
- Revise the manuscript where the novelty was unclear, not just the response letter

**Template**:
> We appreciate the reviewer's concern about novelty. We would like to clarify that our key contribution is [specific novelty], which differs from prior work in the following ways:
> 1. [Concrete difference 1 with reference]
> 2. [Concrete difference 2 with evidence]
> We have revised Section X to make this distinction more explicit.

---

### 2. "Missing baselines / comparisons"

**Typical phrasing**: "The paper should compare with [method X]", "Strong baselines are missing"

**Response strategy**:
- If feasible: run the experiment and add results
- If not feasible: explain why (code unavailable, different setting, etc.) and reference the closest available comparison
- Never dismiss the suggested baseline without justification
- If the baseline is conceptually mismatched, explain the mismatch in task, data, supervision, or evaluation protocol

**Template**:
> Thank you for this suggestion. We have added comparisons with [method X] in Table N (page P). The results show [summary]. We note that [method Y] was not included because [specific reason, e.g., code/data unavailability, incompatible task setting].

---

### 3. "Experiments are not convincing"

**Typical phrasing**: "Results on only one dataset", "No error bars", "Statistical significance not shown"

**Response strategy**:
- Add runs with error bars (Mean ± Std, ≥5 seeds)
- Add significance tests (t-test, Wilcoxon, bootstrap)
- Add additional datasets or ablations if possible
- If dataset addition is infeasible, justify the choice clearly
- If extra experiments are not possible within the revision window, narrow the claim and acknowledge the limitation

**Template**:
> We have strengthened the experimental evaluation as follows:
> - Added error bars over 5 random seeds to all main results (Table N, revised)
> - Performed paired t-tests; all improvements are statistically significant (p < 0.05)
> - Added results on [dataset Z] in Appendix A, showing consistent gains

---

### 4. "Writing quality needs improvement"

**Typical phrasing**: "Hard to follow", "Poor organization", "Numerous grammatical errors"

**Response strategy**:
- Acknowledge and thank
- List the specific sections rewritten
- Highlight structural changes (moved sections, added figures, etc.)
- Have the manuscript proofread (route to `$paper-writing`)
- Avoid claiming the whole manuscript was rewritten unless it was; name the sections actually revised

---

### 5. "Theoretical justification is weak"

**Typical phrasing**: "Why does this work?", "No formal analysis", "Heuristic without justification"

**Response strategy**:
- If theory exists: add formal analysis (convergence, bounds)
- If theory is hard: provide empirical evidence of WHY (ablations, visualization, analysis)
- Acknowledge the limitation honestly if no formal guarantee is possible
- Separate "we added proof" from "we added intuition"; reviewers will treat these differently

---

### 6. "Scalability / efficiency concerns"

**Typical phrasing**: "How does this scale?", "Computational cost is too high", "Not practical"

**Response strategy**:
- Add complexity analysis (time, memory, #params)
- Add scalability experiments if feasible
- Compare computational cost with baselines fairly
- Discuss trade-off between accuracy and efficiency
- State hardware and measurement protocol when reporting runtime

---

### 7. "Scope / limitations are unclear"

**Typical phrasing**: "The claims are too broad", "It is unclear when the method fails"

**Response strategy**:
- Narrow the claim in the abstract/introduction/conclusion
- Add a limitations paragraph with concrete failure modes
- If available, add failure-case examples or subgroup analysis
- Avoid hiding limitations in the response letter only

**Template**:
> We agree that the original wording overstated the scope of our results. We have revised [Section X] to state that the method is validated under [scope]. We also added a limitations paragraph describing [failure mode A] and [failure mode B].

---

### 8. "Missing details for reproducibility"

**Typical phrasing**: "Implementation details are insufficient", "Hyperparameters are missing"

**Response strategy**:
- Add data preprocessing, hyperparameters, prompts, thresholds, seeds, hardware, and code/data availability when appropriate
- Put long details in appendix or supplementary material if the main paper is space-limited
- State any non-public resource constraints plainly

**Template**:
> Thank you for pointing out the missing implementation details. We have added [details] to [Section/Appendix]. These additions specify [key reproducibility items], enabling readers to reproduce the reported setting more directly.

---

## Response Letter Structure

```markdown
# Response to Reviewers

We thank all reviewers for their constructive feedback. We have revised
the manuscript to address every concern raised. Below we provide
point-by-point responses. Major changes are highlighted in blue in the
revised manuscript.

## Reviewer #1

> **R1.1**: [Quoted reviewer comment]

**Response**: [Action taken, evidence, page/line references]

> **R1.2**: [Quoted reviewer comment]

**Response**: [Action taken, evidence, page/line references]

## Reviewer #2
...

## Summary of Changes

| Change | Section | Reviewer |
|--------|---------|----------|
| Added baseline X | Table 3 | R1.1 |
| Rewrote intro | §1 | R2.3 |
| Added ablation | §4.3 | R1.2, R3.1 |
```

## Response Unit Template

Use this compact structure for each comment:

```markdown
> **R1.2**: [short quoted comment or paraphrase]

**Response**: We thank the reviewer for [concern/suggestion]. We have [added/revised/clarified] [specific change] in [location]. [Evidence/result if available]. The revised manuscript now [effect on reader or claim].
```

For disagreement:

```markdown
**Response**: We appreciate this concern. We agree that [shared premise]. In our setting, however, [evidence-backed distinction]. To avoid ambiguity, we have revised [location] to clarify [scope].
```

For infeasible requests:

```markdown
**Response**: We agree that [requested item] would be valuable. We were unable to include it in the current revision because [specific constraint]. To address the concern, we [alternative action], and we now state this limitation in [location].
```

## Tone Principles

1. **Never argue; explain with evidence**
2. **Acknowledge valid concerns explicitly** before responding
3. **Use "We" not "I"** in multi-author papers
4. **Preferred phrases**:
   - "We have revised…" / "We have added…"
   - "We appreciate this suggestion and have…"
   - "We would like to clarify…"
   - "Thank you for pointing this out."
5. **Avoid**:
   - "We disagree" → "We would like to clarify…"
   - "The reviewer misunderstood" → "We realize this was unclear and have revised…"
   - "This is obvious" → [just explain it clearly]

## Claim-Safe Phrases

- Use "we have revised" only when the manuscript changed.
- Use "we have added" only when new material was added.
- Use "we now clarify" when the underlying experiment or method did not change.
- Use "we acknowledge this limitation" when no new evidence can fully resolve the concern.
- Avoid promising future work as a substitute for addressing a current fatal flaw.

## Weak Phrases to Replace

- "We respectfully disagree" -> "We would like to clarify..."
- "The reviewer misunderstood" -> "We realize this was unclear..."
- "Due to space limitations" -> name the actual constraint and where details were added if possible
- "We will consider this in future work" -> explain the current manuscript change or limitation
- "This is beyond the scope" -> define the current scope and why the request falls outside it

## Manuscript-Response Alignment

Before finalizing, check:

- Every "added" or "revised" statement maps to a real manuscript location.
- Every new experiment mentioned in the response appears in the paper, appendix, or supplement.
- Every narrowed claim is narrowed consistently in abstract, introduction, results, discussion, and conclusion.
- The response letter does not contain stronger claims than the manuscript.
- Page, line, section, table, and figure references are placeholders if not yet known: `[VERIFY: page/line]`.

## Confidentiality and AI Use

- Do not paste confidential reviewer text, unpublished competing manuscripts, or private decision letters into external services unless authorized.
- If AI-assisted writing is used for a submitted response, follow the target venue's disclosure policy.
- Human authors remain responsible for factual accuracy, tone, and all manuscript changes.
