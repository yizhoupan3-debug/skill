---
name: research-discovery
description: |
  Research discovery front door: deep topic investigation, literature/survey,
  theory landscape, theorem applicability, math-background inquiry for unknown
  properties (analogies with breaks_when), and research-question scoping.
  Use for requests like "深度调研这个科研方向", "这个问题该用什么数学理论",
  "未知性质怎么找数学背景", "文献综述", or "做一个 research harness".
routing_layer: L2
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: preferred
user-invocable: false
disable-model-invocation: false
short_description: Research discovery, literature survey, and theory-background router
trigger_hints:
  - 调研方向
  - 文献综述
  - 研究方向
  - 知识地图
  - 主题深挖
  - 研究综述
  - 相关定理
  - 理论背景
  - 数学背景
  - 用什么数学
  - 该用什么理论
  - 深度调研这个科研方向
  - 深调研
  - research workbench
  - research harness
  - 类比
  - 未知性质
  - 性质不清楚
  - 非手稿科研
  - survey
  - landscape
  - related work
  - knowledge gap
  - closest work
  # 日常用语扩展
  - 查论文
  - 找文献
  - 有哪些论文
  - 这个方向
  - 学术调研
  - 学术搜索
  - 最新进展
  - paper search
  - literature
  - state of the art
  # barrier 信号扩展（loop escalation 入口）
  - 突破不了
  - 硬指标
  - 无法突破
  - 瓶颈
  - 卡住了
  - roadblock
  - stuck
  - 这个方向走不通
trigger_hints_long: references/trigger-hints-long.md
metadata:
  version: "1.0.0"
  platforms: [supported]
  tags: [research, literature-research, discovery, survey, theory-background]
risk: low
source: local

---

# Research Discovery

This skill is the **front door for research discovery work**: deep topic
investigation, literature surveys, theory landscape mapping, and
math-background inquiry for unknown properties. It selects the right
discovery lane first, then carries the task through evidence retrieval,
synthesis, and handoff.

Use this skill when the user has a research problem in the **discovery
phase** — scoping questions, literature gaps, theory applicability — and
is **not yet asking to design experiments, run code, or write a manuscript**.

## When to use

- The user asks for deep investigation of a research direction, method family, dataset, or technical landscape.
- The user needs a literature survey, related-work synthesis, or knowledge-gap analysis.
- The user wants to scope a research question, novelty claim, or decision the work must support.
- The user explores **unknown mathematical properties** and needs a **theory landscape / math background** map with traceable sources and analogy limits.
- The user asks "这个问题该用什么数学理论", "未知性质怎么找数学背景", or similar theory-scoping questions.
- The user needs a research harness or verification harness for a non-manuscript discovery task.

## Do not use

- The user wants to design experiments, ablations, benchmarks, or baselines -> use `$research-execution`.
- The user needs method, code, math, and evidence checked together -> use `$research-execution`.
- The user wants mathematical modeling (phenomenon -> equations -> dimensional/regime analysis) -> use `$research-execution`.
- The object is a manuscript, submission, reviewer response, paper structure, or "能不能投" decision -> use `$paper-workbench`.
- The user only asks which statistical test to use -> use `$statistical-analysis`.
- The user only asks for a formal proof, derivation, or pure-math task (数学推导、定理证明、公式推导、不等式证明、收敛性证明、存在唯一性证明、变分推导、线性代数证明) with no project/research orchestration -> use `$math-derivation`.
- The user only asks for citation metadata cleanup or BibTeX formatting -> use `$citation-management`.
- The user only asks for reproducibility hygiene -> use `$experiment-reproducibility`.
- The user asks for ordinary code implementation without research-grade evidence gates -> answer in the current coding context.

## Operating contract

Start by classifying the task into one or more lanes:

- `research_question`: research objective, novelty claim, and decision the work must support.
- `external_research`: literature, standards, datasets, repositories, or prior-art lookup when allowed or necessary. Use [`references/academic-sources.md`](references/academic-sources.md) for the five verified-open retrieval sources (arXiv, OpenAlex, CrossRef, PubMed E-utilities, DOAJ); fan out across sources for thorough coverage.
- `math_background_inquiry`: theory landscape for unknown properties (`theory_background` with `theorem_applicability`, `cross_domain_bridges`, `proof_strategy_hints`, multi-source `retrieval_fanout_plan`); multi-round -> RFV `external_mode=math_background` + [math-background-inquiry.md](../../docs/spec.md); conjectures -> §D `conjecture_list`. **Mandatory**: every analogy has `breaks_when`; named theorems have `applies_when`/`fails_when`; retrieval fans out arXiv+OpenAlex/CrossRef per [academic-sources.md](references/academic-sources.md).
- `paper_handoff`: only when the task becomes manuscript-level; then hand off to `$paper-workbench` with **`language_register`** + link to [`../paper-workbench/references/prose-chain-contract.md`](../paper-workbench/references/prose-chain-contract.md) when prose is in scope.

Prefer the smallest lane set that can answer the user's real question. Do not
invent an experiment or manuscript workflow just because literature or citations
are involved.

## Output defaults

For research discovery or investigation, return:

- `Research objective`: the concrete question or decision.
- `Evidence map`: what is known, unknown, and what must be checked.
- `Method/math risks`: assumptions, derivation gaps, counterexamples, and verifier options.
- `Next executable step`: the smallest command, analysis, or investigation that reduces uncertainty.

For deep external research, include a concise retrieval trace when browsing is
used: source type, query/route, inclusion criteria, and unresolved gaps.

For math-background or theory-landscape work, include a witness list and either
an executable checker suggestion (SymPy/CAS, Z3/SMT, Lean/Coq, deterministic
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

- `$research-execution`: experiment design, code verification, math modeling, reproducibility, and research-grade rigor orchestration.
- `$statistical-analysis`: test choice, effect sizes, uncertainty reporting, power, regression diagnostics.
- `$math-derivation`: strict derivations, theorem proofs, witness/checker-backed math review.
- `$citation-management`: citation metadata truth, DOI/BibTeX/reference-list consistency.
- `$paper-workbench`: manuscript-level review, revision, writing, target-venue strategy, or submission readiness.

## Discovery vs. execution boundary

This skill owns the **discovery phase** of research work. When a task crosses
into execution territory, hand off to `$research-execution`:

| This skill (`research-discovery`) | `$research-execution` |
|---|---|
| Research question scoping | Experiment design, ablations, baselines |
| Literature survey, related work | Code verification, deterministic repro |
| Theory landscape, theorem applicability | Math modeling (phenomenon -> equations) |
| Knowledge gap analysis | Math verification (assumptions, witnesses) |
| Novelty claim framing | Reproducibility planning |

If a task spans both phases, complete the discovery lanes first, then hand off
the execution lanes with the discovery outputs as context.

## Hard constraints

- Keep manuscript work out of this front door; hand it to `$paper-workbench` once the object is a paper.
- Keep experiment design and code/math verification out of this front door; hand them to `$research-execution`.
- Do not turn "deep research" into unsourced speculation. If external lookup is needed and allowed, use it; otherwise mark the evidence gap.
- Do not claim math verification without witnesses plus a checker/verifier or a stated blocker.
- Do not bury the next executable step in prose; make it directly actionable.

## Cross-references

- **Academic sources (verified-open retrieval scaffolding)**: [`references/academic-sources.md`](references/academic-sources.md) — arXiv, OpenAlex, CrossRef, PubMed E-utilities, DOAJ API templates and fan-out patterns for `external_research` lane.
- RFV research harness: `rfv_loop_harness.md@{$FRAMEWORK_DOCS_GIT_REF}`
- External research harness: `references/rfv-loop/external-research-harness.md@{$FRAMEWORK_DOCS_GIT_REF}`
- Math background inquiry (deep): `references/rfv-loop/math-background-inquiry.md@{$FRAMEWORK_DOCS_GIT_REF}`
- Manuscript stack boundary: [`../paper-workbench/references/RESEARCH_PAPER_STACK.md`](../paper-workbench/references/RESEARCH_PAPER_STACK.md)
- **Verification skills** (load when lane requires):
  - `literature_survey` lane → [`../literature-verification/SKILL.md`](../literature-verification/SKILL.md)
  - `math_background_inquiry` lane → [`../formal-verification/SKILL.md`](../formal-verification/SKILL.md)
