---
description: Deep research harness — fan-out web searches, fetch sources, verify claims, synthesize cited report. 支持 Agent Reach 多平台源。
metadata:
  platforms:
  - supported
  tags:
  - research
  - harness
  - web
  - fact-check
  - multi-source
  version: '1.2.0'
name: deep-search
scene: general
risk: low
routing_gate: approve
routing_layer: L3
routing_owner: owner
routing_priority: P2
session_start: preferred
short_description: 通用深度搜索引擎 — 多源覆盖（Web/学术/社交/视频）+ 事实核查 + 综合报告
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

# Deep Search — 多平台深度调研

This skill provides a **web-first deep research harness** that fans out across
multiple web searches, fetches source documents, adversarially verifies claims,
and synthesizes a cited report. It supports multi-platform source routing using
Agent Reach channels when available.

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
- The user only asks for a **formal proof, derivation, or pure-math task** → use `$math-verify` or `$math-explore`.
- The user only asks for **citation metadata cleanup** or BibTeX formatting → use `$citation-management`.
- The user only asks for **reproducibility hygiene** → use `$experiment-reproducibility`.
- The user asks for **ordinary code implementation** without research-grade evidence gates → answer in the current coding context.
- The task is a **deep internal codebase exploration** → answer in the current coding context or use `$code-review-deep`.

## Input

The user provides a topic or question to research. The harness accepts:

- A free-text research question or topic description.
- Optional constraints: time range, geographic scope, preferred sources, language preferences.

If the question is underspecified (e.g., "what car to buy" without budget, use case, or region), ask 2–3 clarifying questions before launching the harness.

## Source routing table

Deep Search 会根据研究主题类型自动选择最佳数据源。优先使用 Agent Reach 渠道（若已安装并配置），降级到 WebSearch / web_fetch。

| 主题类型 | 信号 | 首选源 | 备选源 | 降级路径 |
|----------|------|--------|--------|---------|
| 技术/代码 | technology, code, framework, 技术, 框架 | Exa AI 搜索 (`mcporter`) | GitHub 搜索 (`gh`), WebSearch | WebSearch |
| 产品评测/口碑 | 产品, review, 评测, 怎么样, 口碑 | 小红书 | B站, V2EX, WebSearch | WebSearch |
| 学术/论文 | paper, 论文, 文献, DOI, 研究 | Semantic Scholar / arXiv | Exa, WebSearch | WebSearch |
| 全球热点 | trending, news, 最新, twitter | Twitter/X | Reddit, YouTube, WebSearch | WebSearch |
| 国内话题 | 国内, 中文社区, bilibili | B站 | V2EX, 雪球, WebSearch | WebSearch |
| 视频内容 | video, tutorial, 教程, 视频, youtube | YouTube 字幕 (`yt-dlp`) | B站 | WebSearch |
| 通用网页 | (default) | Jina Reader (`r.jina.ai`) | curl, web_fetch | web_fetch |

### 能力检测与降级

在执行多平台调研前，尝试检测可用源：

```bash
# 检测 Agent Reach 可用性
agent-reach doctor --json 2>/dev/null || echo "agent-reach not installed"
```

- **Agent Reach 可用**：按源路由表选择 2-3 个并行源
- **Agent Reach 不可用**：退回到标准 `WebSearch` + `web_fetch`

## Execution workflow

**并行 Agent 编排**：使用并行 agent 执行以下阶段
（宿主支持时，搜索阶段可并行 fan-out，验证阶段串行），或按以下阶段顺序执行为一个
紧凑研究流程。

### Phase 1: Plan — Decompose into search vectors

1. Analyze the research question and identify 3–5 distinct search angles.
2. Each angle should target a different aspect: definitions, recent developments, competing viewpoints, data/statistics, expert opinions.
3. Generate specific, keyword-rich search queries for each angle.
4. **Determine source type** from the Source Routing Table above — pick 2-3 parallel sources.
5. If the topic has a temporal dimension, include date-range constraints.

### Phase 2: Search — Multi-platform parallel fan-out

**Capability-aware strategy**:

1. **Source inventory**: Check if Agent Reach channels are available
   - `agent-reach doctor --json` → available channels with `active_backend`
   - If Agent Reach is NOT installed → fall back to `WebSearch`
2. **Source selection**: Based on topic (from phase 1), pick 2-3 parallel sources from the routing table
3. **Parallel execution**: Run searches across selected sources simultaneously
   - **Web search**: Exa (`mcporter call 'exa.web_search_exa(...)'`) OR `WebSearch`
   - **GitHub**: `gh search repos "query" --sort stars --limit 5`
   - **YouTube**: `yt-dlp --write-sub --skip-download`
   - Each platform source → platform-specific command
4. Collect the top 3-5 results per source (up to 15 candidates).
5. Deduplicate by URL and filter obviously irrelevant results.
6. Cap at 10 unique URLs for the fetch phase.

### Phase 3: Extract — Multi-protocol fetch and read

1. **Primary**: Fetch each URL via Jina Reader (`curl https://r.jina.ai/URL`) — returns clean Markdown
2. **Fallback**: Use `web_fetch` when Jina Reader is unavailable
3. **Special formats**:
   - YouTube subtitles: `yt-dlp --write-sub --skip-download -o "/tmp/%(id)s" "URL"`
   - GitHub repos: `gh repo view owner/repo`
4. For each page, extract:
   - **Claims**: factual assertions relevant to the research question.
   - **Evidence**: direct quotes, data points, or specific context supporting each claim.
   - **Source metadata**: author (if available), publication date, domain authority signals.
5. Discard pages that return errors, are paywalled with no accessible content, or contain no relevant claims.

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
   - **Sources**: list of all cited sources with brief descriptions (include platform marker).
2. Every factual claim must cite its source(s) inline using markdown links.
3. Write in simplified Chinese (面向用户的可见输出使用简体中文) unless the user requests otherwise.
4. Do NOT include unverified claims in the main body; mention them only in the caveats section.

## Output defaults

Return:

- `Research objective`: the concrete question being answered.
- `Search plan`: the search vectors used and why (including platform selection).
- `Source inventory`: URLs fetched, platforms used, inclusion criteria, and exclusions.
- `Verified claims`: each with source citations and confidence level.
- `Contested/refuted claims`: with explanation of why they are disputed.
- `Report`: the synthesized narrative with inline citations.
- `Open questions`: gaps in coverage or areas needing deeper investigation.
- `Recovery trace`: what searches were run, which yielded results, what was missed, and Agent Reach availability status.

## Verification and failure contract

- Treat the final cited report as the deliverable. All claims in the report body must have at least one source citation.
- If a critical source fails (Exa API down, Agent Reach unavailable), note the failure and fall back to the next available source — do not fabricate claims to fill gaps.
- If fewer than 3 unique sources are found, warn the user that coverage is thin and the report may be incomplete.
- Preserve the smallest useful error summary (search returned 0 results, fetch timeout, paywall block) in the recovery trace rather than pasting long logs.

## Hard constraints

- **并行 Agent 编排**：搜索阶段使用并行 subagents 加速；验证/综合阶段串行。
- Do not fabricate claims or citations. Every factual assertion in the report must trace to a fetched source.
- Do not present unverified or single-source claims as established facts; label them clearly.
- Do not skip the adversarial verification phase — every claim must pass cross-reference before appearing in the report body.
- Do not scope-creep into experiment design, literature survey, or manuscript work; hand off to the appropriate skill.
- Do not bury the next executable step in prose; make it directly actionable.
- Do not use academic API endpoints (arXiv, OpenAlex, CrossRef, PubMed) unless the research topic specifically requires academic sources.
- **Agent Reach 不是硬依赖**：若不可用，必须降级到 WebSearch/web_fetch。
- **Jina Reader 不是硬依赖**：若不可用，降级到 web_fetch。

## Lane handoffs

- `$research` (discovery lane): when the task needs literature survey, theory landscape, or math-background inquiry.
- `$research` (execution lane): when the task needs experiment design, code/math verification, or reproducibility checks.
- `$research` (paper-workbench lane): when the object is a manuscript, submission, or paper review.
- `$statistical-analysis`: when the task narrows to statistical test choice or uncertainty reporting.
- `$math-verify`: when the task narrows to formal proof verification or derivation checking.
- `$math-explore`: when the task is mathematical exploration, pattern discovery, or conjecture generation.
- `$citation-management`: when the task narrows to citation metadata cleanup.

## Cross-references

- **Source routing detail**: [`references/source-routing.md`](references/source-routing.md) — per-platform command templates and failure patterns.
- Academic sources (when academic APIs are needed): [`../research/references/academic-sources.md`](../research/references/academic-sources.md)
- Team orchestration API: `core/session-supervisor/src/team_manager.rs`
- Agent lifecycle tracking: `core/session-supervisor/src/process.rs`
- 科研统一前门: [`../research/SKILL.md`](../research/SKILL.md)
