---
name: literature-synthesis
description: |
  Systematically screen, cluster, compare, and synthesize academic literature
  into a topic review, novelty check, related-work section, reading memo,
  evidence matrix, research-gap summary, or structured comparison table.
  This skill also owns structured paper discovery and retrieval as the search
  phase inside literature work, including building a target-journal corpus of
  about 20 close reference papers before paper writing.
  Use when the user asks for "文献梳理", "主题综述", "研究现状总结", "帮我看看这个思路
  别人做过没有", "把这批论文做成对比表", "写 related work", "给我文章相关工作", "帮我搜论文",
  "找这个方向的文献", "下载ref", "目标期刊相近文章", "找20篇相近文章", "搜 arXiv", "Semantic Scholar 搜索",
  or needs paper-by-paper notes turned into a coherent synthesis rather than
  isolated summaries.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 文献梳理
  - 主题综述
  - 研究现状总结
  - 帮我搜论文
  - 找这个方向的文献
  - 下载ref
  - 下载 reference
  - 目标期刊相近文章
  - 找20篇相近文章
  - target journal references
  - comparable papers
  - reference corpus
  - 搜 arXiv
  - Semantic Scholar 搜索
  - 帮我看看这个思路 别人做过没有
  - 把这批论文做成对比表
  - 写 related work
  - 给我文章相关工作
  - academic search
  - find papers on
  - search literature
  - literature review
  - novelty check
  - related work
metadata:
  version: "2.2.0"
  platforms: [codex]
  tags:
    - literature-review
    - novelty-check
    - related-work
    - synthesis
    - comparison
    - reference-corpus
risk: low
source: local
---

# Literature Synthesis

## Overview

Turn scattered papers, citations, PDFs, notes, or a rough topic into a usable literature synthesis. Focus on extracting the field structure, method families, evidence quality, comparison dimensions, and research gaps rather than stacking one-paper summaries.

## Typical Modes

Default to one of these modes based on the user's request:

1. Target-journal reference corpus: retrieve about 20 close papers from the target journal or nearest venues before writing.
2. Search and retrieval: build queries, search across scholarly sources, normalize metadata, and form a usable paper set.
3. Topic review: summarize a field by clusters, timeline, and open problems.
4. Idea novelty check: read a topic note, proposal fragment, or method sketch and determine which parts are already covered in prior work, which parts are common combinations, and which parts may still be underexplored.
5. Related-work drafting: turn a paper topic or idea draft into a reusable related-work section with clear positioning.

## Operating Mode

- Default to synthesis, not paraphrase.
- If the user provides only a topic, infer a review frame and proceed.
- If the user provides a paper list, normalize it into a comparison set before writing.
- Separate what each paper claims from what the evidence actually supports.
- Be explicit about the review scope: task, method family, data domain, time window, and evaluation criteria.
- Prefer tight comparison dimensions over long descriptive prose.

## Workflow

### 1. Frame the review task

Identify the real deliverable first:

- topic scan or theme review
- target-journal reference corpus
- idea novelty check
- related-work section
- structured literature review
- paper comparison table
- annotated reading notes
- research-gap memo
- future-work directions

When scope is underspecified, infer:

- target topic
- likely audience
- time range if recency matters
- expected output depth
- whether the user needs overview, novelty judgment, or writing-ready related work

State assumptions briefly and continue.

### 2. Build the paper set

Normalize the input into a clean corpus:

- deduplicate papers
- capture title, year, venue, task, method, dataset, metrics, and key claim
- group by method family, application setting, or research question
- flag papers that look peripheral, outdated, weakly matched, or non-primary

If the user gives mixed materials such as PDFs, screenshots, notes, and links, consolidate them into one comparable inventory before analysis.
If screenshots, rendered figures, or chart images contain evidence that matters to the synthesis, call `$visual-review` first to extract visible labels, claims, anomalies, or targeted defects before merging them into the literature inventory.

If the user gives an idea draft rather than papers, extract and normalize:

- target problem
- claimed novelty points
- method components
- datasets or settings implied
- baseline families that must be checked

If the user starts with only a topic and no paper set, this skill owns the
search phase too:

- decompose the topic into core concept, task, domain, and time window
- build broad, focused, and narrow queries
- search across scholarly sources in priority order
- normalize title, authors, year, venue, abstract, DOI, and source
- deduplicate before synthesis instead of treating search and synthesis as two user-facing lanes

If the user is preparing a manuscript for a target journal, default to a
20-paper close-reference corpus before paper-writing:

- 12 papers from the target journal or closest sister venues in the last 3-5 years
- 4 closest method/task competitors, even if from other strong venues
- 4 canonical or highly cited papers needed to anchor the field
- keep only papers that can teach positioning, story structure, comparison norms, or required baselines
- record why each paper was kept; do not treat download volume as quality

For the detailed corpus-building workflow, use
[`references/target-journal-reference-corpus.md`](references/target-journal-reference-corpus.md).

### 3. Choose comparison dimensions

Use dimensions that expose actual differences. Common dimensions:

- problem setting
- core method or architecture
- theoretical assumption
- data source or benchmark
- evaluation protocol
- performance claims
- robustness or generalization
- interpretability
- efficiency or deployment cost
- limitations and failure cases

Do not compare papers on dimensions that are irrelevant to the user's topic.

### 4. Synthesize instead of listing

Move from "paper A says X" to field-level structure:

- what schools of thought exist
- how methods evolved over time
- what tradeoffs define the space
- where results are not directly comparable
- which claims are well supported
- which gaps remain open

Prefer patterns such as:

- evolution by generation
- taxonomy by method family
- contrast by assumption
- contrast by scenario or dataset
- contrast by strength vs weakness

For idea novelty checks, explicitly split the result into:

- clearly already done
- partially done or adjacent
- likely not well covered yet
- uncertain areas that need stronger verification

### 5. Write with evidence discipline

- Attribute concrete claims to the correct paper or paper cluster.
- Distinguish facts, interpretations, and your synthesis.
- Do not overstate a trend when the corpus is thin or inconsistent.
- If evidence is mixed, say so directly.
- When identifying research gaps, tie them to specific weaknesses in the reviewed literature.

## Default Output Shapes

Choose the smallest output that fully serves the request. For full structural details, refer to [references/output-shapes.md](references/output-shapes.md):

- **A. Topic Scan**: Scope definition, clusters, what field does well vs bottlenecks.
- **B. Target-Journal Reference Corpus**: 20-paper close-ref inventory, story patterns, venue norms, and required comparators.
- **C. Search Inventory**: Query set, sources searched, retained paper pool, and metadata table.
- **D. Idea Novelty Check**: Claim extraction, search, comparison, scoring matrix (details in `novelty-check-detail.md`).
- **E. Comparison Matrix**: Table of Paper vs Method vs Strength/Weakness.
- **F. Related Work Draft**: Framing paragraph, clusters of prior work, contrast paragraph positioning target work.
- **G. Research-Gap Memo**: Findings, blind spots, high-value open questions, suggested angles.

## Quality Bar

- Do not reduce literature review to isolated summaries.
- Do not treat generic web search as enough for an academic claim when scholarly sources are available.
- Do not call something a "gap" if it is only an untested idea without clear grounding.
- Do not call an idea "novel" just because the exact wording is new.
- Do not merge incomparable results as if they were directly benchmarked.
- Prefer 5 strong dimensions over 15 weak ones.
- For target-journal corpora, prioritize closeness to venue, problem, method, and reader expectations over citation count alone.
- Surface benchmark mismatch, dataset leakage risk, and unfair comparison when visible.
- When a claim depends on what a screenshot, figure, or rendered page visibly shows, route that visual judgment through `$visual-review` instead of ad hoc image interpretation.
- If recent papers materially change the landscape, reflect that in the synthesis.
- Keep the final output easy to reuse in a paper, proposal, or reading report.

## Academic Source Discipline

Prefer sources in this order when building a paper set:

1. peer-reviewed venue pages, DOI records, PubMed/PMC where relevant, and official proceedings
2. arXiv / bioRxiv / SSRN preprints when the field moves fast or no final version exists
3. Semantic Scholar, OpenAlex, Crossref, Google Scholar, Connected Papers, or Litmaps for discovery and citation graph expansion
4. lab blogs, repositories, and project pages only as supporting implementation context

For novelty or related-work claims, record the closest prior work, not just the
most convenient citation. If the search is incomplete, label the conclusion as
`provisional` and name the missing search direction.

## Response Patterns

Use patterns like these when suitable:

- "按方法路线梳理"
- "看这个思路别人做过没有"
- "思路稿 novelty check"
- "按时间脉络梳理"
- "按任务场景梳理"
- "对比表 + 结论摘要"
- "目标期刊20篇相近文章 + 写作套路提炼"
- "相关工作小节初稿"
- "研究空白与可切入方向"
- "逐篇笔记压缩成综述"

## Trigger Examples

Use this skill for prompts such as:

- "帮我做一个这个方向的文献梳理。"
- "给你一个思路稿，帮我看哪些别人做过，哪些还没人做透。"
- "把这几篇论文整理成对比表。"
- "先帮我按目标期刊下载和整理20篇最相近的 ref，再学习它们怎么讲故事。"
- "我想写 related work，先帮我把文献脉络理清。"
- "根据我的文章思路，给我一版相关工作。"
- "根据这些 PDF 做研究现状总结。"
- "给我一个研究空白分析，不要只逐篇总结。"
- "Turn these papers into a structured literature review and gap analysis."

## When to use

- The user wants to screen, cluster, compare, or synthesize academic literature
- The user wants paper discovery, academic database search, or a normalized paper inventory
- The user wants to build a target-journal reference corpus before paper writing
- The task involves related work writing, novelty checking, evidence matrices, or research gap analysis
- The user says "文献梳理", "研究现状", "related work", "对比表", "novelty check", "帮我搜论文", "下载ref", "找20篇相近文章", "搜 arXiv", or "Semantic Scholar 搜索"
- The user wants to produce a structured literature review, reading memo, or topic overview
- The task requires comparing multiple papers to map a research landscape

## Do not use

- The user wants one front door for a research-project task -> use `$research-workbench`
- The user is in early-stage ideation wanting many divergent directions → use `$brainstorm-research`
- The user wants gap-driven **direction generation** from thin input (no existing literature corpus) → use `$brainstorm-research`
- The user wants autonomous multi-hypothesis experiment orchestration → use `$autoresearch`
- The user wants manuscript prose revision or paper logic review → use `$paper-writing` or `$paper-logic`
- The task is about reviewing a specific paper's submission readiness → use `$paper-reviewer`

## Boundary clarification: gap memo vs brainstorm

| Dimension | `literature-synthesis` research-gap mode | `brainstorm-research` |
|-----------|-------------------------------|----------------------|
| **Input** | Existing literature corpus or focused topic | Thin seed, vague idea, undeveloped proposal |
| **Method** | Extract gaps from reviewed papers | Divergent expansion across axes |
| **Output** | Evidence-grounded gap list | Many distinct research bets |
| **Typical trigger** | "这个方向还有什么可以做" + has papers | "帮我 brainstorm 研究点" + thin idea |

Rule of thumb: if the user has **papers / literature** and wants to find gaps → `literature-synthesis`. If the user has a **thin idea** and wants to expand it → `brainstorm-research`.

## Cross-references

- `$research-workbench` uses this skill as the synthesis / novelty lane
- `$autoresearch` novelty gate calls the idea novelty check mode of this skill
- `$brainstorm-research` may use this skill for quick verification searches once an idea needs evidence grounding
- `$paper-reviewer` may use this skill to bootstrap the benchmark paper set before deeper manuscript review
- `$paper-writing` consumes this skill's target-journal corpus when prose needs journal-matched storytelling rather than generic polishing
