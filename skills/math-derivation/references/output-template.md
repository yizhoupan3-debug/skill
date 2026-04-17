# Output Template for Mathematical Derivation

Use this template for all derivations produced by the `math-derivation` skill.

## Full Template

````markdown
## Problem Statement

[Precise restatement in formal mathematical language]

**Given:**
- $H_1$: [First hypothesis]
- $H_2$: [Second hypothesis]
- ...

**Goal:** [What to prove or derive, in formal notation]

**Notation:**
- $x \in X$: [definition]
- $f: X \to Y$: [definition]
- ...

## Proof Strategy

**Technique:** [e.g., Direct proof / Contradiction / Induction / ε-δ argument]

**Rationale:** [Why this technique is appropriate for this problem]

**Outline:**
1. [High-level step 1]
2. [High-level step 2]
3. ...

## Derivation

### [Block 1 title, e.g., "Establishing boundedness"]

**Step 1.** $[mathematical statement]$
  *Justification:* By hypothesis $H_1$.

**Step 2.** $[mathematical statement]$
  *Justification:* By [Theorem Name] applied to Step 1, noting that [hypothesis verification].

**Step 3.** $[mathematical statement]$
  *Justification:* By Steps 1–2 and [algebraic manipulation / named identity].

### [Block 2 title, e.g., "Deriving the main inequality"]

**Step 4.** $[mathematical statement]$
  *Justification:* Applying [Theorem Name] to Step 3. Hypotheses verified: [list].

...

### [Final block, e.g., "Concluding the proof"]

**Step N.** $[final mathematical statement = goal]$
  *Justification:* By Steps [list] and [final argument].

## Conclusion

[Restate the proven result in one sentence.]

$$[final equation/inequality]$$

∎

**Assumptions used:** $H_1$ (Step 1), $H_2$ (Steps 3, 5), $H_3$ (Step 7)

**Assumptions NOT used:** [List any if applicable — flag for possible error]

## Self-check

| Check | Result | Detail |
|-------|--------|--------|
| Dimension / units | ✓ | Both sides have units of [unit] |
| Special value ($x = 0$) | ✓ | LHS = RHS = [value] |
| Special value ($x = 1$) | ✓ | LHS = RHS = [value] |
| Boundary ($x \to \infty$) | ✓ | Both sides → [value] |
| Inequality directions | ✓ | All ≤ consistent; strict at Step [k] |
| Assumption audit | ✓ | All hypotheses used |
| Counterexample probe | ✓ | Removing $H_2$ gives counterexample: [brief] |

## Robustness Review

- **Weakening:** $H_3$ (boundedness) can be relaxed to [weaker condition] — proof still holds with minor modification at Step [k].
- **Converse:** The converse does NOT hold. Counterexample: [brief].
- **Sharpness:** The bound is tight; equality is achieved when [condition].
- **Alternative proof:** [Brief note on alternative approach, if relevant].
````

## Compact Template (for shorter problems)

When the problem is straightforward (≤ 5 steps), use this compact variant:

````markdown
## [Problem title]

**Given:** [hypotheses] | **Goal:** [target]

**Strategy:** [technique]

**Proof.**

**Step 1.** ... *([justification])*

**Step 2.** ... *([justification])*

...

**Conclusion:** [result]. ∎ — Used: $H_1, H_2$.

**Check:** $x=0$ ✓ | $x=1$ ✓ | Dimensions ✓
````
