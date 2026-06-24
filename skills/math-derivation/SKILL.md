---
description: Execute rigorous mathematical derivations and proofs
metadata:
  platforms:
  - supported
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
  version: '2.0.0'
name: math-derivation
risk: low
routing_gate: none
routing_layer: L4
routing_owner: owner
routing_priority: P2
session_start: preferred
short_description: Execute rigorous mathematical derivations and proofs
source: project
trigger_hints:
- ODE
- PDE 推导
- 不等式证明
- 公式推导
- 变分推导
- 存在唯一性证明
- 定理证明
- 收敛性证明
- 数学推导
- 线性代数证明
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
  - "审一下这个证明是否正确" (纯数学证明审查，非整篇论文上下文)

## Do not use

- The task is choosing/running a statistical test -> use `$statistical-analysis`
- The task is research-grade technical critique of a method/algorithm -> use `@lane:reviewer` when it is paper-level; otherwise answer in the current task context without invoking a retired research owner
- The task is auditing notation consistency in a paper -> use `@lane:reviewer` notation sweep
- The task is numerical computation or coding -> answer in the current implementation context, or use `$scientific-figure-plotting` when the deliverable is a publication figure
- The task is explaining math intuitively without formal proof → answer directly
- The task is ML model math with coding focus (loss function implementation, gradient code) -> answer in the current implementation context; do not route to a retired AI/research skill
- The task is reviewing paper-level scientific logic -> use `@lane:reviewer` logic mode
- The task is LaTeX compilation or rendering → answer in the current context（（latex-compile-acceleration 已归档，内联处理） 已归档）
- The user wants **multi-round** exploration of new structures, conjectures, or lemmas with parallel falsification lanes → use **`framework_quality_gate`** and [math-reasoning-harness.md](../../docs/architecture.md) §D（**not** this skill as the orchestrator); single-round derive/prove stays here.
- The user wants **mathematical modeling** (formulate ODE/PDE/closure, scaling, regimes) or **deep math background** for unknown properties → use [`$research-discovery`](../../skills/research-discovery/SKILL.md) (`math_modeling` / `math_background_inquiry`) → RFV §F–G; use this skill only for deriving a specific equation or proof step once the model is fixed.

## Derivation workflow

1. Normalize the problem: given conditions, goal, notation, domains, and hidden assumptions.
2. State the proof strategy before the derivation.
3. Build a **witness list** before claiming rigor: special cases, degenerate limits, dimensional checks, boundary values, and expected monotonicity/symmetry.
4. Build an **assumption dependency graph**: each step cites the assumptions, lemmas, or prior numbered steps it depends on.
5. Derive step by step; every equality, inequality, limit exchange, and implication needs a justification.
6. Verify named-theorem hypotheses before applying the theorem.
7. Run the relevant self-checks: special values, boundary cases, assumption audit, inequality direction, and counterexample probe.
8. If the answer is framed as verified, deep-reviewed, or research-grade, include an executable verifier where feasible (SymPy/CAS, Z3/SMT, Lean/Coq, brute-force enumeration, or a deterministic numeric script) or explicitly state the blocker.
9. Close with the formal conclusion and assumptions used.

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
13. **Verified means checker-backed**: Do not call a derivation "verified", "严审通过", or "深度验证" based only on prose. Provide checker output / a runnable command, or mark the verification gap.
14. **Counterexample probe**: For research-grade critique, attempt at least one counterexample or boundary probe before accepting the claim.
15. **Completion gates (harness integration)**: When operating under automated review, set gate requirements `require_successful_evidence_row: true, min_depth_score: 1` to enforce the checker-backed standard. Without these, the harness cannot distinguish verified output from prose. See `docs/architecture.md`.

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
[Witness list / Assumption dependency graph / Special values / Boundary / Direction / Verifier or blocker]
```

For research-grade or multi-round math work, align with
[docs/architecture.md](../../docs/architecture.md):
witnesses, checker-backed PASS/FAIL, dependency graph, counterexample probes, and (when exploring new structures) §D discovery → promotion → STEM falsify via [lane-templates.md](../../docs/architecture.md).

## Common pitfalls

Top pitfalls: division by zero · limit interchange without DCT/MCT · confusing pointwise vs uniform convergence · incomplete induction · circular reasoning · silent regularity assumptions · sign errors · necessary vs sufficient confusion.

For detailed strategies, templates, and pitfalls, load only the needed reference:
`references/proof-strategies.md`, `references/output-template.md`, or
`references/common-pitfalls.md`.
