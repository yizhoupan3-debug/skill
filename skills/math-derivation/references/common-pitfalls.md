# Common Pitfalls in Mathematical Derivation

LLM-generated mathematical proofs frequently exhibit these errors. This checklist
helps catch them before finalizing a derivation.

## 1. Division by Zero

**Error:** Dividing both sides by an expression that could be zero.

**Example:** From $ax = bx$, concluding $a = b$ without checking $x \neq 0$.

**Fix:** Before dividing by any expression, explicitly state and verify it is non-zero.

## 2. Logic Jumps (Unjustified Steps)

**Error:** Skipping intermediate steps with "clearly" or "it follows that" when the
implication is non-trivial.

**Example:** "Clearly $f$ is uniformly continuous" without invoking Heine-Cantor or
checking the domain is compact.

**Fix:** Every non-trivial claim must cite its justification (theorem name, axiom, or
prior step number).

## 3. Interchange of Limits

**Error:** Swapping $\lim$ with $\int$, $\sum$, or $\frac{d}{dx}$ without justification.

**Common instances:**
- $\lim_{n\to\infty} \int f_n \neq \int \lim_{n\to\infty} f_n$ in general
- $\frac{d}{dx} \int f(x,t)\,dt$ requires Leibniz integral rule conditions
- $\sum_{n} \int f_n \neq \int \sum_{n} f_n$ without dominated convergence or monotone convergence

**Fix:** Explicitly invoke the justifying theorem (Dominated Convergence, Monotone
Convergence, uniform convergence, Leibniz rule) and verify its conditions.

## 4. Confusing Pointwise and Uniform Convergence

**Error:** Using pointwise convergence where uniform convergence is needed
(e.g., to interchange limit and integral).

**Fix:** Check whether the convergence rate depends on the point. If it does,
it is only pointwise.

## 5. Misapplying L'Hôpital's Rule

**Error:** Applying L'Hôpital when the limit is not indeterminate (0/0 or ∞/∞),
or when the derivative of the denominator is zero.

**Fix:** Verify the indeterminate form before applying. Check that g'(x) ≠ 0
in a neighborhood of the limit point.

## 6. Inequality Direction Errors

**Error:** Flipping an inequality direction during a chain of inequalities.

**Example:** From $a \leq b$ and $b \leq c$, mistakenly writing $c \leq a$.

**Fix:** Track inequality directions explicitly. When multiplying by a
negative number, reverse the inequality.

## 7. Circular Reasoning

**Error:** Assuming the conclusion (or an equivalent statement) as part of
the proof.

**Example:** Proving $A \iff B$ by assuming $B$ to prove $A$, then using $A$
to prove $B$ where the second step relies on the first.

**Fix:** Ensure forward direction (⟹) and backward direction (⟸) are
independently proven.

## 8. Incomplete Induction

**Error:** Providing the inductive step but omitting the base case, or vice versa.

**Fix:** Always explicitly verify both base case and inductive step.

## 9. Existential vs. Universal Confusion

**Error:** Proving something for one specific case (∃) but claiming it holds
for all cases (∀), or vice versa.

**Fix:** Match quantifiers precisely. To disprove ∀, one counterexample
suffices. To prove ∃, one witness suffices.

## 10. Domain Violations

**Error:** Applying a theorem outside its domain of validity.

**Common instances:**
- Using continuity-based theorems on discontinuous functions
- Applying finite-dimensional results to infinite-dimensional spaces
- Using real-valued theorems for complex-valued functions

**Fix:** Always verify the hypotheses of any theorem before applying it.

## 11. Silent Assumption of Regularity

**Error:** Assuming a function is continuous, differentiable, bounded, or
measurable without stating it.

**Fix:** List all regularity assumptions in the problem statement. If an
assumption is needed mid-proof, state it explicitly.

## 12. Sign Errors in Algebra

**Error:** Dropping negative signs during algebraic manipulation, especially
with products of negatives, complex conjugates, or integration by parts.

**Fix:** Track signs carefully. When in doubt, verify by substituting
specific values.

## 13. Norm vs. Absolute Value Confusion

**Error:** Using absolute value notation $|x|$ when the argument lives
in a normed space where $\|x\|$ is appropriate, or confusing operator
norm with element norm.

**Fix:** Use $\|\cdot\|$ for norms in general spaces. Specify which norm
when multiple norms are in play.

## 14. Forgetting Boundary Terms

**Error:** In integration by parts or divergence theorem applications,
dropping the boundary term.

**Fix:** Always write the full formula including boundary terms, then
evaluate whether they vanish (and why).

## 15. Treating Necessary Conditions as Sufficient

**Error:** Showing that a candidate solution satisfies necessary conditions
(e.g., Euler-Lagrange equation) without verifying sufficiency (e.g.,
second-order conditions or convexity).

**Fix:** Explicitly verify sufficiency conditions or note that only
necessary conditions have been checked.
