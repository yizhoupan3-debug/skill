# Proof Strategies Reference

Quick reference for choosing the right proof technique. Organized by frequency of use in mathematical derivation tasks.

---

## Tier 1: Fundamental Techniques

### Direct Proof

**Pattern:** Assume hypotheses → chain of implications → conclude goal.

**When:** The logical path from premises to conclusion is clear and constructive.

**Template:**
```
Let [hypotheses]. By [definition/theorem], [intermediate result].
Therefore, by [chain of reasoning], [goal]. ∎
```

**Checklist:**
- [ ] Each implication is justified
- [ ] No implicit case splits
- [ ] Conclusion exactly matches the goal

### Proof by Contradiction (Reductio ad Absurdum)

**Pattern:** Assume ¬(goal) → derive a contradiction with hypotheses or known facts.

**When:** Direct approach is blocked; negating the goal gives a useful working assumption.

**Template:**
```
Assume for contradiction that ¬[goal].
Then by [reasoning], [intermediate result].
But this contradicts [hypothesis / known fact].
Therefore [goal] must hold. ∎
```

**Pitfalls:**
- Ensure the contradiction is genuine (not a computational error)
- Be careful with nested contradictions — state clearly what is being assumed at each level

### Proof by Contrapositive

**Pattern:** Prove ¬Q ⟹ ¬P instead of P ⟹ Q.

**When:** The contrapositive is easier to work with (often when Q is a "non-existence" or "unbounded" statement).

**Relationship to contradiction:** Logically equivalent to direct proof of the contrapositive, NOT the same as contradiction.

### Mathematical Induction

**Pattern:**
1. **Base case:** Verify P(n₀)
2. **Inductive hypothesis:** Assume P(k) for arbitrary k ≥ n₀
3. **Inductive step:** Prove P(k+1) using the hypothesis

**When:** Statement is parameterized by ℕ (or any well-ordered set).

**Variants:**

| Variant | Key difference | Best for |
|---------|---------------|----------|
| **Weak** | Assume P(k), prove P(k+1) | Simple recursive structures |
| **Strong** | Assume P(n₀), ..., P(k), prove P(k+1) | When step depends on multiple predecessors |
| **Structural** | Induction on recursive data structure | Trees, formulas, types |
| **Transfinite** | Well-ordered sets beyond ℕ | Ordinal-indexed arguments |
| **Backward** | Prove P(n) for infinite subsequence + P(k) ⟹ P(k-1) | AM-GM via Cauchy's trick |

**Critical rule:** Missing base case = INVALID proof.

---

## Tier 2: Analysis-Centric Techniques

### ε-δ Arguments

**Pattern:** For every ε > 0, find δ > 0 (or N ∈ ℕ) such that a bound holds.

**When:** Limits, continuity, uniform continuity, convergence of sequences/series/functions.

**Key discipline:**
- δ (or N) must depend only on ε and possibly the point (for pointwise results)
- For **uniform** results, δ must NOT depend on the point
- Construction of δ should be explicit and justified

**Template (continuity):**
```
Let ε > 0 be given. Choose δ = [explicit expression in ε].
Then for all x with |x - a| < δ, we have:
|f(x) - f(a)| = ... ≤ ... < ε.
```

### Comparison and Bounding

**Pattern:** Bound the target quantity above/below by a known convergent/divergent quantity.

**When:** Series convergence, integral estimates, norm bounds.

**Toolkit:**

| Tool | When to use |
|------|------------|
| Comparison test | Series with known majorant/minorant |
| Squeeze theorem | Tight two-sided bound |
| Triangle inequality | Norm/absolute-value estimates |
| Cauchy-Schwarz | Inner product / expectation bounds |
| AM-GM | Products vs. sums |
| Hölder's inequality | $L^p$ estimates |
| Jensen's inequality | Convex function of expectation |
| Young's inequality | Product decomposition: $ab \leq \frac{a^p}{p} + \frac{b^q}{q}$ |
| Grönwall's inequality | ODE/integral inequality bounds |

### Energy / Lyapunov Methods

**Pattern:** Construct a non-negative functional V that decreases along trajectories.

**When:** Stability of ODE/PDE systems, convergence of iterative methods, dissipative systems.

**Steps:**
1. Propose candidate V ≥ 0
2. Compute dV/dt (or V(x_{k+1}) - V(x_k))
3. Show dV/dt ≤ -c·V or dV/dt ≤ 0 with LaSalle invariance

---

## Tier 3: Existence and Structure

### Constructive Proof

**Pattern:** Explicitly exhibit the object claimed to exist.

**When:** An existence statement (∃x : P(x)) where you can produce a witness.

**Advantage:** Provides an algorithm or explicit formula, not just existence.

### Fixed-Point Theorems

| Theorem | Setting | Key condition | Output |
|---------|---------|---------------|--------|
| Banach (contraction) | Complete metric space | Lipschitz constant L < 1 | Unique fixed point + convergence rate |
| Brouwer | Compact convex subset of ℝⁿ | Continuous map | Existence (non-constructive) |
| Schauder | Compact convex subset of Banach space | Continuous, compact image | Existence |
| Kakutani | Compact convex, set-valued | Upper hemicontinuous, convex values | Existence |
| Krasnoselskii | Banach space | Sum of contraction + compact | Existence |

**Template (Banach):**
```
1. Show (X, d) is a complete metric space.
2. Show T: X → X (self-mapping).
3. Show d(Tx, Ty) ≤ L·d(x, y) with L < 1 for all x, y.
4. By Banach fixed-point theorem, ∃! x* with T(x*) = x*. ∎
```

### Variational Methods

**Pattern:** Characterize solutions as minimizers/critical points of a functional.

**When:** Euler-Lagrange equations, calculus of variations, PDE weak solutions.

**Steps:**
1. Define the functional $J[u] = \int L(x, u, u') \, dx$
2. Take a variation: $u \mapsto u + \epsilon \eta$ with $\eta$ in the test function space
3. Compute $\frac{d}{d\epsilon} J[u + \epsilon \eta] \big|_{\epsilon=0} = 0$
4. Apply integration by parts to isolate $\eta$
5. By the fundamental lemma of calculus of variations, derive the Euler-Lagrange equation
6. Verify existence of minimizer (direct method: coercivity + lower semicontinuity + compactness)

### Diagonalization

**Pattern:** Construct an object that differs from every element of a countable list.

**When:** Uncountability proofs (Cantor), incompleteness (Gödel), undecidability (Turing), non-compactness in function spaces (Arzela-Ascoli failure).

---

## Tier 4: Combinatorial and Algebraic

### Pigeonhole Principle

**Pattern:** If n+1 objects are placed in n boxes, at least one box has ≥ 2 objects.

**When:** Combinatorial existence proofs, Dirichlet's approximation theorem.

**Generalized version:** If kn+1 objects are placed in n boxes, at least one box has ≥ k+1 objects.

### Symmetry and WLOG (Without Loss of Generality)

**Pattern:** Reduce cases by observing the problem is invariant under permutation/transformation.

**When:** Multiple variables play symmetric roles; reducing to one case simplifies the argument.

**Caution:** Always explicitly verify the symmetry claim. "WLOG" is not a magic word — it requires justification that the general case reduces to the claimed special case.

### Double Counting / Bijective Proof

**Pattern:** Count the same quantity in two different ways, or establish a bijection between two sets.

**When:** Combinatorial identities, binomial coefficient identities.

### Polynomial / Power Series Methods

**Pattern:** Use generating functions, Taylor expansion, or polynomial identity to establish results.

**When:** Recurrence relations, combinatorial identities, analytic number theory.

---

## Strategy Selection Flowchart

```
Is the statement parameterized by ℕ?
  YES → Induction (weak / strong / structural)
  NO ↓

Is it an existence claim (∃)?
  YES → Can you exhibit a witness?
    YES → Constructive proof
    NO  → Fixed-point / compactness / Zorn's lemma
  NO ↓

Is it a universal claim (∀)?
  YES → Direct proof or ε-δ argument
  NO ↓

Is a bound or inequality needed?
  YES → Comparison / bounding toolkit
  NO ↓

Is direct approach difficult?
  YES → Contradiction or contrapositive
  NO  → Direct proof
```
