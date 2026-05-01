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
user-invocable: false
disable-model-invocation: true
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

- The task is choosing/running a statistical test -> use `$statistical-analysis`
- The task is research-grade technical critique of a method/algorithm -> use `$paper-reviewer` when it is paper-level; otherwise answer in the current task context without invoking a retired research owner
- The task is auditing notation consistency in a paper -> use `$paper-reviewer` notation sweep
- The task is numerical computation or coding -> answer in the current implementation context, or use `$scientific-figure-plotting` when the deliverable is a publication figure
- The task is explaining math intuitively without formal proof → answer directly
- The task is ML model math with coding focus (loss function implementation, gradient code) -> answer in the current implementation context; do not route to a retired AI/research skill
- The task is reviewing paper-level scientific logic -> use `$paper-reviewer` logic mode
- The task is LaTeX compilation or rendering → use `$latex-compile-acceleration`

## Derivation workflow

1. Normalize the problem: given conditions, goal, notation, domains, and hidden assumptions.
2. State the proof strategy before the derivation.
3. Derive step by step; every equality, inequality, limit exchange, and implication needs a justification.
4. Verify named-theorem hypotheses before applying the theorem.
5. Run the relevant self-checks: special values, boundary cases, assumption audit, inequality direction, and counterexample probe.
6. Close with the formal conclusion and assumptions used.

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

## Output template

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

Top pitfalls: division by zero · limit interchange without DCT/MCT · confusing pointwise vs uniform convergence · incomplete induction · circular reasoning · silent regularity assumptions · sign errors · necessary vs sufficient confusion.

For detailed strategies, templates, and pitfalls, load only the needed reference:
`references/proof-strategies.md`, `references/output-template.md`, or
`references/common-pitfalls.md`.
