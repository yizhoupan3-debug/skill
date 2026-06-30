---

description: Deep web search harness — fan-out web searches, fetch sources, verify claims, synthesize cited report.
metadata:
  platforms:
  - supported
  tags:
  - search
  - harness
  - web
  - fact-check
  - multi-source
  version: '1.2.0'
name: deep-search
scene: general
risk: low
routing_gate: none
routing_layer: L2
routing_owner: user
routing_priority: P2
session_start: preferred
short_description: Deep web search harness — web-first multi-source fact-checked report
source: local
trigger_hints:
- deep search
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

This skill provides a **web-first deep search harness** that fans out across
multiple web searches, fetches source documents, adversarially verifies claims,
and synthesizes a cited report. It is the general-purpose answer to "deeply
investigate this topic for me" when the task does not require academic literature-survey
scoping, experiment design, or manuscript work.

**跨宿主统一**：NL 热路由与本 skill 相同；非手稿科研总地图见
[`../research-discovery/SKILL.md`](../research-discovery/SKILL.md) 与
[`../research-execution/SKILL.md`](../research-execution/SKILL.md)。

## When to use

- The user asks for a deep, multi-source, fact-checked report on any topic.
- The user says "帮我深度搜索一下 XX", "做一个关于 XX 的全面调研", "全面调查一下", or similar.
- The user wants a web-first investigation with citation-backed findings.
- The user asks to verify a set of claims against multiple web sources.
- The user needs an overview of a non-specialist topic (tech trends, product comparisons, policy analysis, market landscape) that benefits from broad web retrieval.

## Do not use

- The user needs a **literature survey, theory landscape, or math-background inquiry** (academic discovery phase) → use `$research-discovery`.
- The user wants to **design experiments, ablations, benchmarks, or math modeling** (execution phase) → use `$research-execution`.
- The object is a **manuscript, submission, reviewer response, paper structure, or "能不能投" decision** → use `$paper-workbench`.
- The user only asks which **statistical test** to use → use `$statistical-analysis`.
- The user only asks for a **formal proof, derivation, or pure-math task** (数学推导、定理证明、公式推导、不等式证明) with no research orchestration → use `$math-derivation`.
- The user only asks for **citation metadata cleanup** or BibTeX formatting → use `$citation-management`.
- The user only asks for **reproducibility hygiene** → use `$experiment-reproducibility`.
- The user asks for **ordinary code implementation** without web-evidence gates → answer in the current coding context.
- The task is a **deep internal codebase exploration** (finding all callers, understanding architecture) → answer in the current coding context or use `$code-review-deep`.

## Input

The user provides a topic or question to investigate. The harness accepts:

- A free-text research question or topic description.
- Optional constraints: time range, geographic scope, preferred sources, language preferences.

If the question is underspecified (e.g., "what car to buy" without budget, use case, or region), ask 2–3 clarifying questions before launching the harness. Do not proceed with underspecified questions.

## Execution workflow

The harness runs as a 5-phase execution using `WebSearch` and `WebFetch` tools.
Each phase has entry/exit **Gates** that may stop early.
Search and Extract phases use parallel agents for throughput;
Verify and Synthesize phases run with adversarially separated agents.

### Phase 1: Plan — Decompose into search vectors

**Gate（进入条件）：** 问题必须足够具体。如果太模糊（"帮我查点信息"），先问 2-3 个确认问题再进入本阶段。

1. Analyze the question and identify 3–5 distinct search angles.
2. Each angle should target a different aspect: definitions, recent developments, competing viewpoints, data/statistics, expert opinions.
3. Generate specific, keyword-rich search queries for each angle.
4. If the topic has a temporal dimension, include date-range constraints.

**Gate（出口条件）：**
- ✅ search_angles ≥ 3 → 进入 Phase 2
- ❌ search_angles < 3 → 停止，回复 "无法将问题分解为≥3个有效搜索角度，请提供更具体的查询"

### Phase 2: Search — Fan out across the web

**约束（硬）：**
- 最多执行 **5 条搜索查询**
- 每查询最多收集 **top 3 URL**，总计 ≤ 12 个候选

1. Execute all search queries in parallel using `WebSearch`.
2. Collect the top 3 URLs per query.
3. Deduplicate by URL and filter irrelevant results (ads, thin content, paywall-only).
4. Pick up to 10 unique URLs for the fetch phase.

**Gate（出口条件）：**
- ✅ effective_urls ≥ 3 → 进入 Phase 3
- ✅ 2 ≤ effective_urls < 3 → 进入 Phase 3，但输出时标注 "覆盖率偏薄（仅X个来源）"
- ❌ effective_urls < 2 → 跳过 Phase 3-5，回复 "搜索结果不足（仅X个有效URL），无法完成有意义的调查"

### Phase 3: Extract — Fetch and read sources

**内容预算（硬）：**
- 总提取内容 ≤ **30,000 字符**（约 7500 tokens）
- 每页先用前 **2000 字符**判断相关性：
  - 不相关 → 跳过整页（不计入预算）
  - paywalled / 4xx+ → 标记状态后跳过
- 每页提取正文 ≤ **4000 字符**
- 达到总预算上限 → 停止 fetch 剩余 URL

1. Fetch each URL in parallel using `WebFetch`, respecting content budget.
2. For each page, extract:
   - **Claims**: factual assertions relevant to the research question.
   - **Evidence**: direct quotes, data points, or specific context supporting each claim.
   - **Source metadata**: author (if available), publication date, domain authority signals.

**Gate（出口条件）：**
- ✅ 有效来源（含可提取 claim）≥ 2 → 进入 Phase 4
- ❌ 有效来源 < 2 → 跳过 Phase 4-5，回复 "无法从找到的来源中提取有效信息"

### Phase 4: Verify — Adversarial claim verification

**约束：验证必须由独立的 skeptical subagent 执行，不得由当前 agent 自检。**

1. **Prepare claim ledger**: build a deduplicated list of claims + evidence from Phase 3.
2. **Spawn a skeptical subagent** with the instruction:
   - 对每个 claim，试图找到反论、漏掉的上下文或来源矛盾
   - 标记支持证据不足的 claim
   - 标记跨来源矛盾的 claim（A 源说 X，B 源说 ¬X）
3. **Skeptical agent 输出结构化分类：**
   - `verified` — 有 ≥2 独立来源支持，无严重矛盾
   - `contested` — 来源之间矛盾或证据强度不足
   - `refuted` — 被可靠来源明确否定
4. 保留 refuted claims——放入最终报告的 Caveats 节。

**Gate（出口条件）：**
- ✅ verified claims ≥ 1 → 进入 Phase 5，verified claims 写入报告主体
- ❌ verified claims = 0 → 跳过 Phase 5 报告主体，仅输出 Caveats + 搜索追溯，回复 "未能找到可验证的信息"

### Phase 5: Synthesize — Write the cited report

**约束：** Phase 3 原始内容已不在当前上下文中——仅使用 Phase 4 输出的结构化 claim ledger 进行合成。

1. Structure the report with:
   - **Executive Summary**: 2–3 paragraph overview.
   - **Detailed Findings**: organized by theme, not by source. Only `verified` claims.
   - **Nuances & Caveats**: `contested` and `refuted` claims, limitations, open questions.
   - **References**: list of all cited URLs with brief descriptions.
2. Every factual claim must cite its source(s) inline using markdown links.
3. Write in simplified Chinese (面向用户的可见输出使用简体中文) unless the user requests otherwise.
4. Do NOT include unverified claims in the main body; mention them only in the caveats section.

## Output defaults

Return:

- `Search objective`: the concrete question being answered.
- `Search plan`: the search vectors used and why.
- `Source inventory`: URLs fetched, inclusion criteria, and exclusions.
- `Verified claims`: each with source citations and confidence level.
- `Contested/refuted claims`: with explanation of why they are disputed.
- `Report`: the synthesized narrative with inline citations.
- `Open questions`: gaps in coverage or areas needing deeper investigation.
- `Recovery trace`: what searches were run, which yielded results, what gates were triggered, and what was missed.

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

- **No Workflow orchestration**: do not call `Workflow` tool or any multi-agent orchestration framework. Use direct `Agent` spawns and inline execution only.
- Do not fabricate claims or citations. Every factual assertion in the report must trace to a fetched source.
- Do not present unverified or single-source claims as established facts; label them clearly.
- Do not skip the adversarial verification phase — every claim must pass cross-reference before appearing in the report body.
- Do not scope-creep into experiment design, literature survey, or manuscript work; hand off to the appropriate skill.
- Do not bury the next executable step in prose; make it directly actionable.
- Do not use academic API endpoints (arXiv, OpenAlex, CrossRef, PubMed) unless the topic specifically requires academic sources; use `WebSearch` as the primary retrieval backbone.

## Division of work with peer skills

This skill and `research-discovery` / `research-execution` are complementary:

| Concern | `deep-search` (this skill) | `research-discovery` | `research-execution` |
|---|---|---|---|
| Web-first general investigation report | Primary | -- | -- |
| Claim verification via multi-source cross-reference | Primary | -- | -- |
| Literature survey, related-work synthesis | -- | Primary | -- |
| Theory landscape, math background inquiry | -- | Primary | -- |
| Experiment design, ablation, baselines | -- | -- | Primary |
| Math verification (checker, witnesses) | -- | -- | Primary |
| Code verification (audit, tests, repro) | -- | -- | Primary |

If the question turns out to require academic discovery or experiment
execution, complete the web search phase first, then hand off with the
findings as context.

## Lane handoffs

- `$research-discovery`: when the task needs literature survey, theory landscape, or math-background inquiry.
- `$research-execution`: when the task needs experiment design, code/math verification, or reproducibility checks.
- `$paper-workbench`: when the object is a manuscript, submission, or paper review.
- `$statistical-analysis`: when the task narrows to statistical test choice or uncertainty reporting.
- `$math-derivation`: when the task narrows to formal proof or derivation.
- `$citation-management`: when the task narrows to citation metadata cleanup.

## Cross-references

- Academic sources (when academic APIs are needed): [`../research-discovery/references/academic-sources.md`](../research-discovery/references/academic-sources.md) — arXiv, OpenAlex, CrossRef, PubMed E-utilities, DOAJ API templates.
- Manuscript stack boundary: [`../paper-workbench/references/RESEARCH_PAPER_STACK.md`](../paper-workbench/references/RESEARCH_PAPER_STACK.md)
- Discovery counterpart: [`../research-discovery/SKILL.md`](../research-discovery/SKILL.md)
- Execution counterpart: [`../research-execution/SKILL.md`](../research-execution/SKILL.md)
