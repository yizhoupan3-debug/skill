---
name: academic-search
description: |
  Execute structured academic literature searches using Semantic Scholar, arXiv, Google
  Scholar, PubMed, Crossref, and other scholarly APIs. Build search queries, normalize
  results, and produce paper inventories. Use when the user asks '帮我搜论文', '找这个方向的文献', '搜
  arXiv', 'Semantic Scholar 搜索', 'academic search', 'find papers on', 'search literature',
  or needs paper discovery before synthesis, novelty checking, or related work drafting.
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 帮我搜论文
  - 找这个方向的文献
  - 搜 arXiv
  - Semantic Scholar 搜索
  - academic search
  - find papers on
  - search literature
  - novelty checking
  - related work drafting
  - semantic scholar
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - academic-search
    - semantic-scholar
    - arxiv
    - literature
    - paper-discovery
    - api
risk: low
source: local
---

- **Dual-Dimension Audit (Pre: Search-Query/Source, Post: Citation-Relevance/DB-Coverage Results)** → `$execution-audit-codex` [Overlay]

# Academic Search

This skill owns **structured academic literature search and retrieval**.

## When to use

- The user needs to find papers on a specific topic
- The user wants to search across multiple academic databases
- The user needs structured search queries for literature discovery
- The user wants paper metadata normalized for downstream synthesis
- The skill is invoked as a sub-routine by `$literature-synthesis` or `$brainstorm-research`

## Do not use

- The user already has papers and wants synthesis → use `$literature-synthesis`
- The user wants novelty checking → use `$literature-synthesis` Mode B
- The user wants to manage citations/references → use `$citation-management`
- The task is general web search → use standard web search

## Cross-references

- `$literature-synthesis` Phase 2 (Systematic Search) can optionally delegate to this skill for structured multi-source retrieval
- `$brainstorm-research` novelty gate can invoke this skill for quick search passes

## Search Source Priority

| Source | Best for | API Available |
|--------|----------|---------------|
| **Semantic Scholar** | Citation graphs, abstract search, related papers | `api.semanticscholar.org` |
| **arXiv** | Latest preprints in CS/ML/Physics/Math | `export.arxiv.org/api/query` |
| **Google Scholar** | Broadest coverage, cross-disciplinary | No official API (use web search) |
| **PubMed / PMC** | Biomedical literature | `eutils.ncbi.nlm.nih.gov` |
| **Crossref** | DOI resolution, metadata verification | `api.crossref.org` |
| **DBLP** | Computer science venues | `dblp.org/search/publ/api` |
| **Connected Papers** | Citation network visualization | Web interface |
| **Research Rabbit** | Recommendation-based discovery | Web interface |

## Search Strategy Framework

### Step 1: Query Decomposition

Break the search topic into components:
- **Core concept**: The main idea or method
- **Task / application**: What problem it solves
- **Domain / field**: Where it is applied
- **Time window**: How recent must results be?
- **Scope**: Broad survey vs narrow verification

### Step 2: Query Formulation

Build queries at 3 levels:
1. **Broad query**: Core concept only → high recall, low precision
2. **Focused query**: Core concept + task + domain → balanced
3. **Narrow query**: Specific method name + specific setting → high precision

Example for "transformers for protein structure prediction":
1. Broad: `transformer protein`
2. Focused: `transformer protein structure prediction`
3. Narrow: `AlphaFold attention mechanism tertiary structure`

### Step 3: Multi-Source Search

Run queries across sources in priority order:
1. Semantic Scholar → get citation counts and influential citations
2. arXiv → get latest preprints not yet indexed
3. Domain-specific source (PubMed, DBLP) → fill domain gaps
4. Google Scholar → catch anything missed

### Step 4: Result Normalization

Normalize all results into a standard inventory:

| Field | Required | Source |
|-------|----------|--------|
| Title | ✅ | API response |
| Authors | ✅ | API response |
| Year | ✅ | API response |
| Venue | ✅ | API response + manual verify |
| Abstract | ✅ | API response |
| DOI | Preferred | API response |
| Citation count | Preferred | Semantic Scholar |
| Influential citations | Optional | Semantic Scholar |
| arXiv ID | If available | arXiv |
| PDF link | If available | API response |

### Step 5: Deduplication & Ranking

- Merge duplicates across sources (same paper, different sources)
- Rank by: citation count × recency × relevance to query
- Flag: preprints, workshop papers, non-peer-reviewed sources
- Note: papers with <5 citations from current year are OK (too new)

## Semantic Scholar API Quick Reference

```bash
# Search papers
curl "https://api.semanticscholar.org/graph/v1/paper/search?query=transformer+protein&limit=10&fields=title,authors,year,venue,abstract,citationCount,influentialCitationCount,externalIds"

# Get paper details
curl "https://api.semanticscholar.org/graph/v1/paper/{paper_id}?fields=title,authors,year,abstract,references,citations"

# Get related papers (recommendations)
curl "https://api.semanticscholar.org/recommendations/v1/papers/forpaper/{paper_id}?limit=10&fields=title,authors,year,citationCount"
```

## arXiv API Quick Reference

```bash
# Search papers
curl "http://export.arxiv.org/api/query?search_query=all:transformer+AND+all:protein&start=0&max_results=10&sortBy=submittedDate&sortOrder=descending"
```

## Output Format

### Paper Inventory Table

| # | Title | Authors | Year | Venue | Citations | Relevance | Source |
|---|-------|---------|------|-------|-----------|-----------|--------|
| 1 | ... | ... | 2024 | NeurIPS | 150 | High | S2 |
| 2 | ... | ... | 2025 | arXiv | 5 | High | arXiv |

### Search Metadata

```
Search topic: ...
Queries used: [list]
Sources searched: [list]
Total results scanned: N
Papers retained: M
Time window: YYYY-YYYY
Date of search: YYYY-MM-DD
```

## Hard Constraints

- Do not fabricate paper titles, authors, or venues
- Do not make up citation counts or DOIs
- Mark unverifiable results explicitly
- Always record the search date (results change over time)
- Distinguish peer-reviewed papers from preprints
- Do not assume a paper exists just because the topic makes sense
- When API access is unavailable, use web search and note the limitation
- **Superior Quality Audit**: For systematic literature reviews, trigger `$execution-audit-codex` to verify search breadth against [Superior Quality Bar](../execution-audit-codex/references/superior-quality-bar.md).

## Trigger examples

- "帮我搜一下 transformer 在蛋白质预测方向的最新论文"
- "用 Semantic Scholar 搜这个 topic 的 top cited papers"
- "arXiv 上最近三个月有什么相关的新工作"
- "Find the 20 most cited papers on federated learning in healthcare"
- "帮我找这个方向的文献"
- "搜一下有没有类似的工作"
- "给我做个系统的文献检索"
- "PubMed 搜一下这个药物的临床试验论文"
- "强制进行学术搜索深度审计 / 检查检索词质量与文献相关性。"
- "Use $execution-audit-codex to audit this search strategy for coverage idealism."
