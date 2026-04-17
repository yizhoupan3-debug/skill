# Rebuttal & Response Letter Patterns

## Common Reviewer Attack Patterns and Response Templates

### 1. "Novelty is insufficient"

**Typical phrasing**: "The contribution is incremental", "This is a straightforward combination of X and Y"

**Response strategy**:
- Acknowledge the reviewer's perspective respectfully
- Identify the specific technical novelty that was underappreciated
- Provide a concrete comparison to the closest prior work
- Quantify the difference (performance gap, capability gap, or conceptual gap)

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

---

### 5. "Theoretical justification is weak"

**Typical phrasing**: "Why does this work?", "No formal analysis", "Heuristic without justification"

**Response strategy**:
- If theory exists: add formal analysis (convergence, bounds)
- If theory is hard: provide empirical evidence of WHY (ablations, visualization, analysis)
- Acknowledge the limitation honestly if no formal guarantee is possible

---

### 6. "Scalability / efficiency concerns"

**Typical phrasing**: "How does this scale?", "Computational cost is too high", "Not practical"

**Response strategy**:
- Add complexity analysis (time, memory, #params)
- Add scalability experiments if feasible
- Compare computational cost with baselines fairly
- Discuss trade-off between accuracy and efficiency

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
