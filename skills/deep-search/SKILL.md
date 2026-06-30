---

description: Deep research harness — fan-out web searches, fetch sources, verify claims, synthesize cited report.
metadata:
  platforms:
  - supported
  tags:
  - research
  - harness
  - web
  - fact-check
  - multi-source
  version: '1.1.0'
name: deep-search
scene: general
risk: low
routing_gate: approve
routing_layer: L3
routing_owner: user
routing_priority: P2
session_start: preferred
short_description: 通用深度搜索引擎 — web 多源覆盖+事实核查+综合报告
source: local
trigger_hints:
- deep research
- deep-search
- web research
- fact check
- 网络调研
- 信息收集
- 多源验证
- 网络信息验证
- 帮我查一下
- 搜索并汇总
- web search report
- investigate claims
- verify claims
---
# Deep Search

This skill provides a **web-first deep research harness** that fans out across
multiple web searches, fetches source documents, adversarially verifies claims,
and synthesizes a cited report. It is the general-purpose answer to "deeply
research this topic for me" when the task does not require literature-survey
scoping, experiment design, or manuscript work.

## When to use

- The user asks for a deep, multi-source, fact-checked research report on any topic.
- The user says "帮我深度研究一下 XX", "做一个关于 XX 的全面调研", "全面调查一下", or similar.
- The user wants a web-first investigation with citation-backed findings.
- The user asks to verify a set of claims against multiple web sources.
- The user needs an overview of a non-specialist topic (tech trends, product comparisons, policy analysis, market landscape) that benefits from broad web retrieval.

## Do not use

- The user needs a **literature survey, theory landscape, or math-background inquiry** (academic discovery phase) → use `$research` (discovery lane).
- The user wants to **design experiments, ablations, benchmarks, or math modeling** (execution phase) → use `$research` (execution lane).
- The object is a **manuscript, submission, reviewer response, paper structure, or "能不能投" decision** → use `$research` (paper-workbench lane).
- The user only asks which **statistical test** to use → use `$statistical-analysis`.
- The user only asks for a **formal proof, derivation, or pure-math task** (数学推导、定理证明、公式推导、不等式证明) with no research orchestration → use `$math-derivation`.
- The user only asks for **citation metadata cleanup** or BibTeX formatting → use `$citation-management`.
- The user only asks for **reproducibility hygiene** → use `$experiment-reproducibility`.
- The user asks for **ordinary code implementation** without research-grade evidence gates → answer in the current coding context.
- The task is a **deep internal codebase exploration** (finding all callers, understanding architecture) → answer in the current coding context or use `$code-review-deep`.

## Input

The user provides a topic or question to research. The harness accepts:

- A free-text research question or topic description.
- Optional constraints: time range, geographic scope, preferred sources, language preferences.

If the question is underspecified (e.g., "what car to buy" without budget, use case, or region), ask 2–3 clarifying questions before launching the harness.

## Execution workflow

**并行 Agent 编排**：使用并行 agent 执行以下阶段
（宿主支持时，搜索阶段可并行 fan-out，验证阶段串行），或按以下阶段顺序执行为一个
紧凑研究流程。

The harness runs as a multi-stage execution using `WebSearch` and `WebFetch` tools:
- Search and Extract phases use parallel agents for throughput
- Verify and Synthesize phases run sequentially after evidence is gathered

### Phase 1: Plan — Decompose into search vectors

1. Analyze the research question and identify 3–5 distinct search angles.
2. Each angle should target a different aspect: definitions, recent developments, competing viewpoints, data/statistics, expert opinions.
3. Generate specific, keyword-rich search queries for each angle.
4. If the topic has a temporal dimension, include date-range constraints.

### Phase 2: Search — Fan out across the web

1. Execute all search queries in parallel using `WebSearch`.
2. Collect the top 3 URLs per query (up to 15 candidates).
3. Deduplicate by URL and filter obviously irrelevant results (e.g., ads, thin content).
4. Cap at 10 unique URLs for the fetch phase.

### Phase 3: Extract — Fetch and read sources

1. Fetch each URL in parallel using `WebFetch`.
2. For each page, extract:
   - **Claims**: factual assertions relevant to the research question.
   - **Evidence**: direct quotes, data points, or specific context supporting each claim.
   - **Source metadata**: author (if available), publication date, domain authority signals.
3. Discard pages that return errors, are paywalled with no accessible content, or contain no relevant claims.

### Phase 4: Verify — Adversarial claim verification

1. **Deduplicate claims**: merge overlapping or restated claims across sources.
2. **Cross-reference**: check whether each claim is supported by ≥2 independent sources.
3. **Adversarial check**: for each claim, consider:
   - Is this factually sound and logically coherent?
   - Is it generally accepted or highly contested?
   - Are there known counterarguments or caveats?
4. Classify each claim as: `verified` (multi-source support), `contested` (mixed evidence), or `refuted` (contradicted by reliable sources).
5. Preserve refuted claims for the report — they may be relevant as misconceptions to address.

### Phase 5: Synthesize — Write the cited report

1. Structure the report with:
   - **Executive Summary**: 2–3 paragraph overview.
   - **Detailed Findings**: organized by theme, not by source.
   - **Nuances & Caveats**: contested claims, limitations, open questions.
   - **References**: list of all cited URLs with brief descriptions.
2. Every factual claim must cite its source(s) inline using markdown links.
3. Write in simplified Chinese (面向用户的可见输出使用简体中文) unless the user requests otherwise.
4. Do NOT include unverified claims in the main body; mention them only in the caveats section.

## Output defaults

Return:

- `Research objective`: the concrete question being answered.
- `Search plan`: the search vectors used and why.
- `Source inventory`: URLs fetched, inclusion criteria, and exclusions.
- `Verified claims`: each with source citations and confidence level.
- `Contested/refuted claims`: with explanation of why they are disputed.
- `Report`: the synthesized narrative with inline citations.
- `Open questions`: gaps in coverage or areas needing deeper investigation.
- `Recovery trace`: what searches were run, which yielded results, and what was missed.

## Verification and failure contract

- Treat the final cited report as the deliverable. All claims in the report body must
  have at least one source citation.
- If the web search or fetch fails for a critical source, note the failure and
  adjust the report scope — do not fabricate claims to fill gaps.
- If fewer than 3 unique sources are found, warn the user that coverage is thin
  and the report may be incomplete.
- Preserve the smallest useful error summary (search returned 0 results, fetch
  timeout, paywall block) in the recovery trace rather than pasting long logs.

## Hard constraints

- **并行 Agent 编排**：搜索阶段使用并行 subagents 加速；验证/综合阶段串行。
- Do not fabricate claims or citations. Every factual assertion in the report must trace to a fetched source.
- Do not present unverified or single-source claims as established facts; label them clearly.
- Do not skip the adversarial verification phase — every claim must pass cross-reference before appearing in the report body.
- Do not scope-creep into experiment design, literature survey, or manuscript work; hand off to the appropriate skill.
- Do not bury the next executable step in prose; make it directly actionable.
- Do not use academic API endpoints (arXiv, OpenAlex, CrossRef, PubMed) unless the research topic specifically requires academic sources; use `WebSearch` as the primary retrieval backbone.

## Lane handoffs

- `$research` (discovery lane): when the task needs literature survey, theory landscape, or math-background inquiry.
- `$research` (execution lane): when the task needs experiment design, code/math verification, or reproducibility checks.
- `$research` (paper-workbench lane): when the object is a manuscript, submission, or paper review.
- `$statistical-analysis`: when the task narrows to statistical test choice or uncertainty reporting.
- `$math-derivation`: when the task narrows to formal proof or derivation.
- `$citation-management`: when the task narrows to citation metadata cleanup.

## Cross-references

- Academic sources (when academic APIs are needed): [`../research/references/academic-sources.md`](../research/references/academic-sources.md) — arXiv, OpenAlex, CrossRef, PubMed E-utilities, DOAJ API templates. 仅限于科研需求时查阅。
- Team orchestration API: `core/session-supervisor/src/team_manager.rs` — team-based multi-agent orchestration.
- Agent lifecycle tracking: `core/session-supervisor/src/process.rs` — agent health registry.
- 科研统一前门: [`../research/SKILL.md`](../research/SKILL.md)
