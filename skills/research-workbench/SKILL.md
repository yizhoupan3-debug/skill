---
name: research-workbench
description: |
  Unified front door for non-manuscript research work: research project planning,
  experiment design, deep topic investigation, method/code/math joint review,
  and research-grade verification orchestration. Use for requests like "帮我设计实验",
  "深度调研这个科研方向", "推导方法正确性", "科研项目怎么推进",
  "帮我核查这个方法和代码是否可靠", or "做一个 research harness".
routing_layer: L2
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: preferred
user-invocable: false
disable-model-invocation: true
short_description: Non-manuscript research workbench and rigor router
trigger_hints:
  - 科研项目推进
  - 设计实验
  - 实验方案设计
  - 深度调研这个科研方向
  - 深调研
  - research workbench
  - research harness
  - 方法正确性核查
  - 代码和数学联合核查
  - 推导方法正确性
  - 研究路线设计
  - ablation 方案
  - benchmark 方案
  - 非手稿科研
do_not_use:
  - 数学推导
  - 定理证明
  - 公式推导
  - 不等式证明
  - 收敛性证明
  - 存在唯一性证明
  - 变分推导
  - 线性代数证明
metadata:
  version: "1.0.0"
  platforms: [supported]
  tags:
    - research
    - experiment-design
    - literature-research
    - rigor
    - math
    - reproducibility
risk: low
source: local

---

# Research Workbench

This skill is the **front door for non-manuscript research work**. It keeps
research tasks from falling back into ordinary chat by selecting the right rigor
lane first, then carrying the task through evidence, math, code, and
reproducibility checks.

Use this skill when the user has a research problem but is **not yet asking to
review, revise, write, or submit a manuscript**.

## When to use

- The user wants to design an experiment, ablation, benchmark, evaluation plan, or research protocol.
- The user asks for deep investigation of a research direction, method family, dataset, or technical landscape.
- The user needs method, code, math, and evidence checked together rather than as isolated tasks.
- The user wants to validate whether a proposed method is correct, novel enough, reproducible, or worth pursuing.
- The user asks for a research harness, verification harness, or adversarial review of a non-manuscript research artifact.
- The user needs a project-level research plan with concrete next experiments, blockers, and verification commands.

## Do not use

- The object is a manuscript, submission, reviewer response, paper structure, or "能不能投" decision -> use `$paper-workbench`.
- The user only asks which statistical test to use -> use `$statistical-analysis`.
- The user only asks for a formal proof or derivation with no project/research orchestration -> use `$math-derivation`.
- The user only asks for citation metadata cleanup or BibTeX formatting -> use `$citation-management`.
- The user only asks for reproducibility hygiene -> use `$experiment-reproducibility`.
- The user asks for ordinary code implementation without research-grade evidence gates -> answer in the current coding context.

## Operating contract

Start by classifying the task into one or more lanes:

- `research_question`: research objective, novelty claim, and decision the work must support.
- `experiment_design`: variables, controls, ablations, baselines, metrics, sample size, and failure criteria.
- `external_research`: literature, standards, datasets, repositories, or prior-art lookup when allowed or necessary.
- `math_verification`: assumptions, derivation witnesses, theorem/lemma dependencies, and checker options.
- `code_verification`: implementation audit, tests, deterministic repro, and benchmark commands.
- `reproducibility`: environment, data/versioning, seeds, configs, and artifact trace.
- `paper_handoff`: only when the task becomes manuscript-level; then hand off to `$paper-workbench`.

Prefer the smallest lane set that can answer the user's real question. Do not
invent a manuscript workflow just because literature or citations are involved.

## Output defaults

For research planning or review, return:

- `Research objective`: the concrete question or decision.
- `Evidence map`: what is known, unknown, and what must be checked.
- `Method/math risks`: assumptions, derivation gaps, counterexamples, and verifier options.
- `Experiment plan`: baselines, controls, metrics, ablations, sample size/power concerns, and stopping criteria.
- `Reproducibility plan`: environment/data/config/seed/artifact requirements.
- `Next executable step`: the smallest command, analysis, or experiment that reduces uncertainty.

For deep external research, include a concise retrieval trace when browsing is
used: source type, query/route, inclusion criteria, and unresolved gaps.

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

## Lane handoffs

- `$statistical-analysis`: test choice, effect sizes, uncertainty reporting, power, regression diagnostics.
- `$experiment-reproducibility`: environment capture, seeds, data versioning, experiment tracking, protocol locking.
- `$math-derivation`: strict derivations, theorem proofs, witness/checker-backed math review.
- `$citation-management`: citation metadata truth, DOI/BibTeX/reference-list consistency.
- `$paper-workbench`: manuscript-level review, revision, writing, target-venue strategy, or submission readiness.
- `$code-review-deep`: adversarial code/repo review when implementation correctness is the central risk.

## Hard constraints

- Keep manuscript work out of this front door; hand it to `$paper-workbench` once the object is a paper.
- Do not turn "deep research" into unsourced speculation. If external lookup is needed and allowed, use it; otherwise mark the evidence gap.
- Do not claim math verification without witnesses plus a checker/verifier or a stated blocker.
- Do not claim experimental validity without baselines, controls, metrics, and reproducibility requirements.
- Do not bury the next executable step in prose; make it directly actionable.

## Cross-references

- RFV research harness: [`../../docs/rfv_loop_harness.md`](../../docs/rfv_loop_harness.md)
- External research harness: [`../../docs/references/rfv-loop/external-research-harness.md`](../../docs/references/rfv-loop/external-research-harness.md)
- Math reasoning harness: [`../../docs/references/rfv-loop/math-reasoning-harness.md`](../../docs/references/rfv-loop/math-reasoning-harness.md)
- Manuscript stack boundary: [`../paper-workbench/references/RESEARCH_PAPER_STACK.md`](../paper-workbench/references/RESEARCH_PAPER_STACK.md)
- Reproducibility minimum record: [`../experiment-reproducibility/references/research-record-minimum.md`](../experiment-reproducibility/references/research-record-minimum.md)
