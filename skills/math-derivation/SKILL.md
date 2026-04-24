---
name: math-derivation
description: |
  Execute rigorous mathematical derivations and proofs with full justification.
  Use when the user asks for 数学推导, 定理证明, 公式推导, 不等式证明, ODE/PDE 推导,
  收敛性证明, 存在唯一性证明, 变分推导, 线性代数证明, 概率论推导, 优化问题推导,
  mathematical proof, formal derivation, prove convergence, derive equation.
  Best for strict mathematical rigor with every logical step explicitly justified.
routing_layer: L4
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: n/a
short_description: Execute rigorous mathematical derivations and proofs
trigger_hints:
  - 数学推导
  - 定理证明
  - 公式推导
  - 不等式证明
  - ODE
  - PDE 推导
  - 收敛性证明
  - 存在唯一性证明
  - 变分推导
  - 线性代数证明
metadata:
  version: "2.0.0"
  platforms: [codex]
  tags:
    - mathematics
    - proof
    - derivation
    - theorem
    - LaTeX
    - formal-reasoning
    - inequality
    - ODE
    - PDE
    - convergence
risk: low
source: local
---

# Math Derivation

This skill owns **rigorous mathematical derivation and proof execution**:
theorem proving, formula derivation, inequality arguments, ODE/PDE derivations,
convergence/existence/uniqueness proofs, variational calculus, optimization proofs,
linear algebra proofs, and probability/measure-theoretic arguments.

## When to use

- The user needs a step-by-step mathematical proof or derivation
- The user wants to prove a theorem, lemma, proposition, or corollary
- The user needs formula derivation with full justification
- The user wants inequality proofs (AM-GM, Cauchy-Schwarz, Jensen, Hölder, Minkowski, etc.)
- The user needs ODE/PDE derivation, solution verification, or well-posedness proof
- The user wants convergence, existence, uniqueness, or stability proofs
- The user needs eigenvalue/eigenvector derivations or matrix decomposition proofs
- The user wants variational derivations (Euler-Lagrange, Hamilton, optimal control)
- The user needs probability/measure theory proofs (martingale convergence, CLT, SLLN, etc.)
- The user wants optimization proofs (KKT conditions, duality, convergence of algorithms)
- The user wants to verify or critique a mathematical derivation for correctness
- The user needs to derive a gradient, Hessian, or Jacobian analytically
- Best for requests like:
  - "帮我推导这个公式"
  - "证明这个不等式"
  - "这个 ODE 的解怎么推导"
  - "证明这个映射是压缩映射"
  - "推导 Euler-Lagrange 方程"
  - "prove that this series converges"
  - "推导 KKT 条件"
  - "证明这个算子的谱分解"

## Do not use

- The task is choosing/running a statistical test → use `$statistical-analysis`
- The task is research-grade technical critique of a method/algorithm → use `$research-engineer`
- The task is auditing notation consistency in a paper → use `$paper-notation-audit`
- The task is numerical computation or coding → use `$python-pro` or `$scientific-figure-plotting`
- The task is explaining math intuitively without formal proof → answer directly
- The task is ML model math with coding focus (loss function implementation, gradient code) → use `$ai-research`
- The task is reviewing paper-level scientific logic → use `$paper-logic`
- The task is LaTeX compilation or rendering → use `$latex-compile-acceleration`

## Cross-references

- `$paper-logic` may route here when a paper's core claim requires formal proof verification
- `$research-engineer` may co-invoke this skill for proof-oriented analysis of algorithms
- `$statistical-analysis` handles statistical tests; this skill handles the underlying math (e.g., proving a test statistic's distribution)
- `$paper-notation-audit` audits symbol consistency; this skill proves content correctness

## Derivation workflow

### Step 1 — Problem normalization

- Restate the problem precisely in formal mathematical language
- List all **given conditions** (hypotheses $H_1, H_2, \ldots$) explicitly
- State the **goal** (what to prove / derive) in formal notation
- Define all symbols, their domains, and their types (scalar, vector, operator, etc.)
- Identify the mathematical domain (analysis, algebra, topology, probability, etc.)

### Step 2 — Strategy selection

Choose the proof technique and **state it explicitly before beginning**:

| Technique | When to use |
|-----------|------------|
| Direct proof | Conclusion follows from hypotheses by a chain of implications |
| Proof by contradiction | Direct approach is hard; assume ¬goal and derive contradiction |
| Proof by contrapositive | Prove ¬Q ⟹ ¬P instead of P ⟹ Q |
| Mathematical induction | Statement indexed by ℕ (or well-ordered set) |
| Strong induction | Inductive step needs all prior cases, not just k → k+1 |
| Structural induction | Recursively defined structures (trees, formulas, types) |
| Constructive proof | Need to exhibit an explicit object satisfying a property |
| ε-δ argument | Limits, continuity, uniform convergence |
| Comparison / bounding | Inequalities, convergence of series/integrals |
| Energy / Lyapunov method | Stability of dynamical systems, convergence of iterations |
| Fixed-point argument | Existence via Banach / Brouwer / Schauder |
| Diagonalization | Uncountability, undecidability |
| Symmetry / WLOG | Reduce cases by exploiting invariance |
| Pigeonhole / counting | Combinatorial existence |

> See [references/proof-strategies.md](file:///Users/joe/Documents/skill/skills/math-derivation/references/proof-strategies.md) for detailed strategy guide with templates.

### Step 3 — Step-by-step derivation

Format each step as:

```
**Step k.** [Mathematical statement in LaTeX]
  *Justification:* [Theorem name / Axiom / "by Step j" / "by hypothesis H_i"]
```

Mandatory rules:
- Number every step sequentially
- Every equality, inequality, or logical implication must cite its justification
- Use LaTeX (`$...$` inline, `$$...$$` display) for all mathematical expressions
- Group related steps into logical blocks with descriptive headers
- When applying a named theorem, state its full name and verify its hypotheses are satisfied

### Step 4 — Per-step self-check

After completing the derivation, perform **all applicable** checks:

| Check | Description | When applicable |
|-------|-------------|-----------------|
| **Dimension analysis** | Units/dimensions match on both sides | Physics, engineering |
| **Special value test** | Plug in $x = 0, 1, -1, \infty$ to sanity-check | Always |
| **Boundary cases** | Check behavior at domain boundaries | Analysis, topology |
| **Direction check** | Inequality directions consistent throughout | Inequality proofs |
| **Assumption audit** | Every hypothesis is actually used | Always |
| **Symmetry check** | Result respects expected symmetries | When symmetry exists |
| **Consistency check** | Result degenerates to known results in special cases | Generalizations |
| **Counterexample probe** | Try to construct a counterexample to the claim | Before concluding |

### Step 5 — QED wrap-up

- Restate the conclusion in formal notation
- List **all assumptions/hypotheses** that were used (and note any that were NOT used — this may indicate redundancy or an error)
- Mark with ∎ (or Q.E.D.)

### Step 6 — Robustness review

- **Weakening**: Can any condition be weakened while preserving the result?
- **Converse**: Does the converse hold? If not, provide a counterexample
- **Generalization**: Can the result be generalized to broader spaces/settings?
- **Alternative paths**: Are there independent proof strategies worth noting?
- **Sharpness**: Is the bound/estimate tight? If so, where is equality achieved?

## Hard constraints

> [!CAUTION]
> These are **non-negotiable rules** for every derivation produced by this skill.

1. **No logic jumps**: Every step's premise must be traceable to given conditions or prior steps. No implicit "middle steps" allowed.
2. **No unjustified "obvious"**: If writing "显然" / "clearly" / "trivially", you MUST append a one-line justification or cite a standard result by name.
3. **All math in LaTeX**: Every formula, equation, and expression uses LaTeX notation. No plain-text math like `x^2 + y^2`.
4. **Division safety**: When dividing or inverting, explicitly verify the denominator/element is non-zero. State the reason (e.g., "since $x > 0$ by $H_2$").
5. **Explicit assumptions**: Continuity, differentiability, boundedness, integrability, measurability, compactness, completeness, etc. must be stated, never silently assumed.
6. **Inequality strictness**: Always mark whether an inequality is strict ($<$) or non-strict ($\leq$). When transitioning between strict and non-strict in a chain, justify each transition.
7. **Induction completeness**: Mathematical induction must include (a) base case verification, (b) inductive hypothesis statement, and (c) inductive step proof.
8. **Quantifier discipline**: Clearly distinguish $\forall$ vs $\exists$, specify the domain of each quantified variable, and maintain correct quantifier ordering.
9. **Implication direction**: Clearly distinguish $\Longrightarrow$ (sufficient), $\Longleftarrow$ (necessary), and $\Longleftrightarrow$ (iff). Never write "iff" when only one direction has been proven.
10. **No circular reasoning**: The conclusion must never appear (even in disguised form) in its own proof chain.
11. **Limit interchange justification**: Swapping $\lim$, $\int$, $\sum$, or $\frac{d}{dx}$ requires explicit invocation of the justifying theorem (DCT, MCT, Fubini, Leibniz, etc.) with verification of its hypotheses.
12. **Theorem hypothesis verification**: Before applying any named theorem, explicitly verify that all its hypotheses are satisfied in the current context.

## Domain-specific patterns

### Analysis (Real / Complex / Functional)

- For $\epsilon$-$\delta$ proofs: state the target $\epsilon$, construct $\delta$ explicitly, verify the bound
- For series convergence: choose and justify the test (ratio, root, comparison, integral, alternating series)
- For uniform convergence: use Weierstrass M-test or show the convergence rate is point-independent
- For operator theory: specify the space, norm, and whether the operator is bounded/compact/self-adjoint

### Differential Equations (ODE / PDE)

- State the equation, domain, initial/boundary conditions
- For existence/uniqueness: invoke Picard-Lindelöf (ODE) or appropriate PDE theory (Lax-Milgram, Schauder estimates, etc.)
- For stability: construct a Lyapunov function and verify its properties
- For weak solutions: specify the test function space and derive the weak formulation

### Optimization

- State the objective, constraints, and feasible region
- For convex problems: verify convexity of objective and constraint set
- KKT: state primal feasibility, dual feasibility, complementary slackness, stationarity
- For convergence of algorithms: specify the step-size rule and prove descent/contraction

### Probability / Measure Theory

- Specify the probability space $(\Omega, \mathcal{F}, P)$
- For convergence: distinguish a.s., in probability, in $L^p$, in distribution
- For martingale arguments: verify the filtration, adaptedness, and integrability
- For CLT/SLLN: verify independence, moment conditions, and identical distribution

### Linear Algebra

- Specify the field ($\mathbb{R}$, $\mathbb{C}$, or general), vector space dimension, and inner product if relevant
- For eigenproblems: distinguish eigenvalues of $A$ vs singular values, left vs right eigenvectors
- For decompositions: state the hypotheses (symmetric, positive definite, unitary, etc.)

## Output template

> See [references/output-template.md](file:///Users/joe/Documents/skill/skills/math-derivation/references/output-template.md) for the full Markdown template with examples.

Minimal structure:

```
## Problem Statement
**Given / Goal / Notation**

## Proof Strategy
[Technique + rationale]

## Derivation
**Step 1–N** with justifications

## Conclusion ∎
**Assumptions used**

## Self-check
[Dimension / Special values / Boundary / Direction]
```

## Common pitfalls

> See [references/common-pitfalls.md](file:///Users/joe/Documents/skill/skills/math-derivation/references/common-pitfalls.md) for the full list with examples and fixes.

Top pitfalls: division by zero · limit interchange without DCT/MCT · confusing pointwise vs uniform convergence · incomplete induction · circular reasoning · silent regularity assumptions · sign errors · necessary vs sufficient confusion.

## Trigger examples

- "帮我证明 Cauchy-Schwarz 不等式"
- "推导 Navier-Stokes 方程的弱形式"
- "证明这个级数一致收敛"
- "用反证法证明 √2 是无理数"
- "推导变分法的 Euler-Lagrange 方程"
- "prove the Banach fixed-point theorem"
- "derive the gradient of the cross-entropy loss"
- "推导 KKT 条件的充分性"
- "证明鞅收敛定理"
- "推导 SVD 分解"
