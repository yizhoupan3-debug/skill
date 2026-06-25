---
description: 'Research execution lanes: experiment design, math verification, math modeling, code verification, and reproducibility. Carries a research task through evidence, math, code, and reproducibility checks.'
metadata:
  platforms:
  - supported
  tags:
  - research
  - experiment-design
  - math
  - modeling
  - reproducibility
  - execution
  version: '1.0.0'
name: research-execution
risk: low
routing_gate: none
routing_layer: L2
routing_owner: owner
routing_priority: P2
session_start: preferred
short_description: Research execution lanes — experiment design, math, code, reproducibility
source: local
trigger_hints:
- ablation 方案
- baseline 设计
- benchmark 方案
- governing equation
- modeling
- 不确定性管理
- 代码和数学联合核查
- 失败复盘
- 实验方案设计
- 实验设计
- 控制方程
- 推导方法正确性
- 数学建模
- 方法正确性核查
- 无量纲化
- 本构方程
- 模型搭建
- 研究路线设计
- 量纲分析
---
# Research Execution

This skill handles the **execution lanes** of non-manuscript research work. It
covers experiment design, math verification, math modeling, code verification,
and reproducibility checks. Discovery, literature survey, and math-background
inquiry are handled by `research-discovery`; manuscript work by
`$paper-workbench`.

## When to use

- The user wants to design an experiment, ablation, benchmark, evaluation plan, or research protocol.
- The user needs method, code, math, and evidence checked together rather than as isolated tasks.
- The user wants to validate whether a proposed method is correct, novel enough, reproducible, or worth pursuing.
- The user wants **mathematical modeling** (phenomenon -> equations -> dimensional/regime analysis) with checkable witnesses.
- The user needs a project-level research plan with concrete next experiments, blockers, and verification commands.

## Do not use

- The user wants deep investigation of a research direction, method family, dataset, or technical landscape (literature + theory landscape) -> use `research-discovery`.
- The user explores **unknown mathematical properties** and needs a **theory landscape / math background** map -> use `research-discovery`.
- The object is a manuscript, submission, reviewer response, paper structure, or "能不能投" decision -> use `$paper-workbench`.
- The user only asks which statistical test to use -> use `$statistical-analysis`.
- The user only asks for a formal proof, derivation, or pure-math task (数学推导、定理证明、公式推导、不等式证明、收敛性证明、存在唯一性证明、变分推导、线性代数证明) with no project/research orchestration -> use `$math-derivation`.
- The user only asks for citation metadata cleanup or BibTeX formatting -> use `$citation-management`.
- The user only asks for reproducibility hygiene -> use `$experiment-reproducibility`.
- The user asks for ordinary code implementation without research-grade evidence gates -> answer in the current coding context.

## Operating contract

Start by classifying the task into one or more lanes:

- `experiment_design`: variables, controls, ablations, baselines, metrics, sample size, and failure criteria.
- `math_verification`: assumptions, derivation witnesses, theorem/lemma dependencies, and checker options.
- `math_modeling`: build/check a `model_spec` (variables, equations, closures, nondimensional groups, regime chart); multi-round -> `framework_quality_gate` with `external_mode=modeling` + [math-reasoning-harness.md](../../docs/architecture.md) section F.
- `code_verification`: implementation audit, tests, deterministic repro, and benchmark commands.
- `reproducibility`: environment, data/versioning, seeds, configs, and artifact trace.

Prefer the smallest lane set that can answer the user's real question. If the
task requires discovery or literature work, hand off to `research-discovery`
first and resume execution when the discovery lanes return.

## Output defaults

For research planning or review, return:

- `Research objective`: the concrete question or decision.
- `Evidence map`: what is known, unknown, and what must be checked.
- `Method/math risks`: assumptions, derivation gaps, counterexamples, and verifier options.
- `Experiment plan`: baselines, controls, metrics, ablations, sample size/power concerns, and stopping criteria.
- `Reproducibility plan`: environment/data/config/seed/artifact requirements.
- `Next executable step`: the smallest command, analysis, or experiment that reduces uncertainty.

For math-heavy or method-correctness work, include a witness list and either an
executable checker suggestion (SymPy/CAS, Z3/SMT, Lean/Coq, deterministic
numeric probe, brute-force enumeration) or a clear blocker. Do not label a
result "verified", "严审通过", or "research-grade" on prose alone.

## Verification and failure contract

- Treat executable evidence as the default closeout path: commands, notebooks,
  deterministic probes, benchmark scripts, artifact hashes, or a cited external
  source trace. Name how to verify the claim before marking it complete.
- If a lane cannot be verified, return a blocker with the missing input,
  unavailable source, or unrun command; do not convert it into a confident
  research conclusion.
- For tool or data failures, preserve the smallest useful error summary and the
  next retry path in the evidence map instead of pasting long logs into context.

## Lane routing logic

Execute only the lanes above. When the task also requires discovery or
literature lanes, pause and hand off to `research-discovery`:

| User need | Lane in this skill | Hand-off |
|---|---|---|
| Experiment design / ablation / benchmark | `experiment_design` | -- |
| Method correctness / derivation check | `math_verification` | -- |
| Equation building / dimensional analysis | `math_modeling` | -- |
| Implementation audit / tests / repro | `code_verification` | -- |
| Environment / versioning / artifact trace | `reproducibility` | -- |
| Literature / prior-art / dataset lookup | -- | `research-discovery` -> `external_research` |
| Theory landscape / math background map | -- | `research-discovery` -> `math_background_inquiry` |
| Manuscript-level work | -- | `$paper-workbench` |

## Do-not-use boundaries

- Keep manuscript work out of this skill; hand it to `$paper-workbench` once the object is a paper.
- Do not turn "deep research" into unsourced speculation. If external lookup is needed, hand off to `research-discovery`; otherwise mark the evidence gap.
- Do not claim math verification without witnesses plus a checker/verifier or a stated blocker.
- Do not claim experimental validity without baselines, controls, metrics, and reproducibility requirements.
- Do not bury the next executable step in prose; make it directly actionable.

## Division of work with research-discovery

This skill and `research-discovery` are complementary halves of the research
workbench:

| Concern | `research-execution` (this skill) | `research-discovery` |
|---|---|---|
| Experiment design, ablation, baselines | Primary | -- |
| Math verification (checker, witnesses) | Primary | -- |
| Math modeling (equations, closures, nondimensional) | Primary | -- |
| Code verification (audit, tests, benchmarks) | Primary | -- |
| Reproducibility (env, seeds, artifact trace) | Primary | -- |
| Literature / prior-art retrieval | -- | Primary |
| Theory landscape / math background inquiry | -- | Primary |
| Research question framing | Advisory | Primary |
| Novelty / significance assessment | Advisory | Primary |

When a task spans both skills, the recommended flow is:

1. `research-discovery` runs `external_research` and/or `math_background_inquiry` first.
2. Discovery outputs feed into this skill's execution lanes (e.g., a theory landscape informs `math_modeling`; prior art informs `experiment_design`).
3. If execution reveals new unknowns, loop back to `research-discovery` for targeted retrieval.

## Lane handoffs

- `$statistical-analysis`: test choice, effect sizes, uncertainty reporting, power, regression diagnostics.
- `$experiment-reproducibility`: environment capture, seeds, data versioning, experiment tracking, protocol locking.
- `$math-derivation`: strict derivations, theorem proofs, witness/checker-backed math review.
- `$citation-management`: citation metadata truth, DOI/BibTeX/reference-list consistency.
- `$paper-workbench`: manuscript-level review, revision, writing, target-venue strategy, or submission readiness.
- `$code-review-deep`: adversarial code/repo review when implementation correctness is the central risk.

## Hard constraints

- Do not claim math verification without witnesses plus a checker/verifier or a stated blocker.
- Do not claim experimental validity without baselines, controls, metrics, and reproducibility requirements.
- Do not bury the next executable step in prose; make it directly actionable.
- Do not perform discovery or literature work in this skill; hand off to `research-discovery`.

## Cross-references

- Quality Gate research harness: `docs/architecture.md` §Quality Gate
- Math reasoning harness: `docs/math-reasoning-harness.md`
- Manuscript stack boundary: [`../paper-workbench/references/RESEARCH_PAPER_STACK.md`](../paper-workbench/references/RESEARCH_PAPER_STACK.md)
- Reproducibility minimum record: [`../experiment-reproducibility/references/research-record-minimum.md`](../experiment-reproducibility/references/research-record-minimum.md)
- Discovery counterpart: [`../research-discovery/SKILL.md`](../research-discovery/SKILL.md)
- **Verification skills** (load when lane requires):
  - `math_verification` / `math_modeling` lane → [`../formal-verification/SKILL.md`](../formal-verification/SKILL.md)
  - `reproducibility` lane → [`../reproducibility-verification/SKILL.md`](../reproducibility-verification/SKILL.md)
  - `code_verification` lane → [`../structure-verification/SKILL.md`](../structure-verification/SKILL.md)
