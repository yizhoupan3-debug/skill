---

description: 'Research discovery front door: deep topic investigation, literature/survey, theory landscape, theorem applicability, math-background inquiry for unknown properties, and research-question scoping.'
metadata:
  platforms:
  - supported
  tags:
  - research
  - literature-research
  - discovery
  - survey
  - theory-background
  version: '1.0.0'
name: research-discovery
scene: research
risk: low
routing_gate: none
routing_layer: L2
routing_owner: owner
routing_priority: P2
session_start: preferred
short_description: Research discovery, literature survey, and theory-background router
source: local
trigger_hints:
- closest work
- knowledge gap
- landscape
- related work
- survey
- 主题深挖
- 性质不清楚
- 数学背景
- 文献综述
- 未知性质
- 学术深调研
- 学术调研方向
- 理论背景
- 用什么数学
- 相关定理
- 知识地图
- 研究方向
- 研究综述
- 类比
- 该用什么理论
- 调研方向
- 非手稿科研
- research-discovery
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
- `math_background_inquiry`: theory landscape for unknown properties — candidate theorems with `applies_when` / `fails_when` + cross-domain bridges (theorem level, no proof strategy); multi-source `retrieval_fanout_plan`; multi-round -> RFV `external_mode=math_background` + [docs/math-reasoning-harness.md](../../docs/math-reasoning-harness.md); conjectures -> §D `conjecture_list`. **Mandatory**: every analogy has `breaks_when`; named theorems have `applies_when`/`fails_when`; retrieval fans out arXiv+OpenAlex/CrossRef per [academic-sources.md](references/academic-sources.md). **Boundary contract**: see [discovery-execution-boundary-contract.md §3](references/discovery-execution-boundary-contract.md).
- `paper_handoff`: only when the task becomes manuscript-level; then hand off to `$paper-workbench` with **`language_register`** + link to [`../paper-workbench/references/prose-chain-contract.md`](../paper-workbench/references/prose-chain-contract.md) when prose is in scope.

Prefer the smallest lane set that can answer the user's real question. Do not
invent an experiment or manuscript workflow just because literature or citations
are involved.

## Output defaults

For research discovery or investigation, return:

- `Research objective`: the concrete question or decision.
- `Evidence map`: what is known, unknown, and what must be checked.
- `Theory risks`: theorem-choice errors, analogy limits, evidence gaps, and unfalsifiable assumptions.
- `Next executable step`: the smallest command, analysis, or investigation that reduces uncertainty.

For deep external research, include a concise retrieval trace when browsing is
used: source type, query/route, inclusion criteria, and unresolved gaps.

For math-background or theory-landscape work, include a witness list and either
an executable checker suggestion (SymPy/CAS, Z3/SMT, Lean/Coq, deterministic
numeric probe, brute-force enumeration) or a clear blocker. Do not label a
result "verified", "严审通过", or "research-grade" on prose alone.

## Verification and failure contract

Follow the shared verification contract defined in [discovery-execution-boundary-contract.md §5](references/discovery-execution-boundary-contract.md).

Discovery-specific requirement: without external retrieval (when needed and allowed), do not turn "deep research" into unsourced speculation — mark the evidence gap instead.

## Lane handoffs

- `$research-execution`: experiment design, code verification, math modeling, reproducibility, and research-grade rigor orchestration.
- `$statistical-analysis`: test choice, effect sizes, uncertainty reporting, power, regression diagnostics.
- `$math-derivation`: strict derivations, theorem proofs, witness/checker-backed math review.
- `$citation-management`: citation metadata truth, DOI/BibTeX/reference-list consistency.
- `$paper-workbench`: manuscript-level review, revision, writing, target-venue strategy, or submission readiness.

## Upstream skill integration: good-question

When the user has a **vague interest, rough idea, or unclear research direction** rather than a specific question or a surveyable topic, call `$good-question` first to sharpen the question before entering discovery.

**When to route upstream**:
- User has a broad area but no clear question → `$good-question` for problem sharpening first
- User has a literature gap but hasn't verified if it's consequential → `$good-question` to stress-test the gap
- User says "选题", "开题", "想法不成形", or "问题打磨" → `$good-question` is the owner, not research-discovery
- After `$good-question` outputs a Good Question Card, research-discovery can take the sharpened question and do targeted literature survey

**Consuming the Good Question Card**:
The Good Question Card (output of `$good-question`) provides structured input for research-discovery:
- `Core assumption challenged` → seeds the literature survey's hypothesis scope
- `Competing hypotheses` → frames the contradiction-finding and closest-work identification
- `Key discriminating evidence` → focuses the retrieval on decisive experiments/data
- `Two-week pilot` → defines the feasible scope for discovery work

**Boundary**:
- `$good-question` replaces research-discovery when the task is "find the right question" not "survey an existing question or theory area"
- If the user already has a concrete question (or even a rough prototype), research-discovery is the right front door first, with `$good-question` as optional refinement later
- Do not route to `$good-question` when: the user already has a settled research question and only needs literature/theory survey

**Downstream**:
- `$good-question` does not require research-discovery; it can work standalone for users who only need question sharpening
- research-discovery remains the front door for literature survey and theory landscape; `$good-question` is a pre-processing step

## Discovery vs. execution boundary

This skill owns the **discovery phase** of research work. When a task crosses
into execution territory, hand off to `$research-execution`:

| This skill (`research-discovery`) | `$research-execution` |
|---|---|
| Research question scoping | Experiment design, ablations, baselines |
| Literature survey, related work | Code verification, deterministic repro |
| **Candidate theorem identification** | **Math verification (derivation strategy + checker execution)** |
| **Novelty / significance assessment (Primary)** | — (hand off to discovery) |
| **Dataset identification & curation (哪些存在、各自属性)** | **Dataset selection & justification (选哪个做实验)** |
| **Model selection justification (该领域有哪些 SOTA/候选模型)** | **Model selection & justification (选哪个、为什么、对比条件)** |
| Knowledge gap analysis | Reproducibility planning |

If a task spans both phases, complete the discovery lanes first, then hand off
the execution lanes with the discovery outputs as context. See the full
boundary contract in [discovery-execution-boundary-contract.md](references/discovery-execution-boundary-contract.md).

## Hard constraints

- Keep manuscript work out of this front door; hand it to `$paper-workbench` once the object is a paper.
- Keep experiment design and code/math verification out of this front door; hand them to `$research-execution`.
- **Math boundary**: do not produce `proof_strategy_hints` (derivation roadmaps) in `math_background_inquiry` — that belongs to `$research-execution` `math_verification`. See [discovery-execution-boundary-contract.md §3](references/discovery-execution-boundary-contract.md).
- **Loop-back handling**: when `research-execution` sends a loop-back handoff (see [boundary contract §4](references/discovery-execution-boundary-contract.md)), validate the `hypothesis`, run targeted `external_research` or `math_background_inquiry` per the `retrieval_scope`, and return findings to execution. Do not start a full discovery cycle — respond only to the loop-back question.
- Do not turn "deep research" into unsourced speculation. If external lookup is needed and allowed, use it; otherwise mark the evidence gap.
- Do not claim math verification without witnesses plus a checker/verifier or a stated blocker.
- Do not bury the next executable step in prose; make it directly actionable.

## Cross-references

- **Academic sources (verified-open retrieval scaffolding)**: [`references/academic-sources.md`](references/academic-sources.md) — arXiv, OpenAlex, CrossRef, PubMed E-utilities, DOAJ API templates and fan-out patterns for `external_research` lane.
- **Boundary contract (discovery vs execution)**: [`references/discovery-execution-boundary-contract.md`](references/discovery-execution-boundary-contract.md) — canonical ownership table, math work split, handoff protocols, scenario routing.
- Quality Gate research harness: [`../../docs/quality-gate.md`](../../docs/quality-gate.md) §Quality Gate
- Math reasoning harness: [`../../docs/math-reasoning-harness.md`](../../docs/math-reasoning-harness.md) — math background inquiry deep dive (§C)
- Manuscript stack boundary: [`../paper-workbench/references/RESEARCH_PAPER_STACK.md`](../paper-workbench/references/RESEARCH_PAPER_STACK.md)
- **Verification skills** (load when lane requires):
  - `external_research` / `literature_survey` lane → [`../../quality-gates/literature-verification/SKILL.md`](../../quality-gates/literature-verification/SKILL.md)
  - `math_background_inquiry` lane → [`../../quality-gates/formal-verification/SKILL.md`](../../quality-gates/formal-verification/SKILL.md)
  - `reproducibility` lane → [`../../quality-gates/reproducibility-verification/SKILL.md`](../../quality-gates/reproducibility-verification/SKILL.md)
  - `prose` lane (when output contains expository text) → [`../../quality-gates/prose-verification/SKILL.md`](../../quality-gates/prose-verification/SKILL.md)
  - `code_verification` lane → [`../code-review-deep/SKILL.md`](../code-review-deep/SKILL.md)
