---
name: literature-synthesis
description: |
  Screen, cluster, compare, and synthesize academic literature into a topic
  review, novelty check, related-work section, reading memo, evidence matrix,
  research-gap summary, or comparison table. Also owns paper discovery inside
  literature work, including target-journal reference corpus building.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - 文献梳理
  - 主题综述
  - 研究现状总结
  - 帮我搜论文
  - 找这个方向的文献
  - 下载ref
  - 目标期刊相近文章
  - 找20篇相近文章
  - target journal references
  - comparable papers
  - 搜 arXiv
  - Semantic Scholar 搜索
  - 帮我看看这个思路 别人做过没有
  - 把这批论文做成对比表
  - 写 related work
  - literature review
  - novelty check
  - related work
metadata:
  version: "2.4.0"
  platforms: [codex]
  tags: [literature-review, novelty-check, related-work, synthesis, comparison]
risk: low
source: local

---

# literature-synthesis

This skill owns literature work where individual papers must become a coherent
map: what exists, how approaches differ, what evidence is strong, and where the
gap or contribution may be.

## When to Use

- The user asks for literature search, screening, or paper collection.
- The user wants related work, a topic review, or a research-status summary.
- The user wants novelty or baseline calibration against existing work.
- The user provides papers/notes and wants a synthesis rather than summaries.
- The user needs a target-journal corpus of close references before writing.

## Do Not Use

- Citation formatting or `.bib` cleanup -> use `$citation-management`.
- Manuscript prose polishing after claims are fixed -> use `$paper-writing`.
- General web research unrelated to academic literature -> use `$information-retrieval`.
- Full research-project orchestration -> use `$research-workbench`.

## Evidence Rules

- Separate facts from interpretation.
- Track paper identity, venue/year, method, data, metric, and limitation.
- Do not claim novelty without search scope and comparable-work evidence.
- Prefer a smaller high-signal corpus over a noisy dump of many papers.
- Use current external sources when the question depends on recent literature.

## Workflow

1. Define topic scope, inclusion criteria, and output shape.
2. Search or ingest papers; deduplicate and prioritize close matches.
3. Build a matrix: problem, method, data, result, limitation, relevance.
4. Cluster papers by mechanism or claim, not only by keyword.
5. Synthesize the field into patterns, disagreements, gaps, and next actions.
6. Hand off to `$paper-writing` only after claims and evidence are stable.

## Output Defaults

- For exploration: ranked paper list plus why each matters.
- For synthesis: grouped findings, comparison table, and research gap.
- For target-journal work: close-reference corpus and reusable notes.
- For novelty checks: clear scope, nearest prior work, and confidence limits.

## References

- [references/novelty-check-detail.md](./references/novelty-check-detail.md)
- [references/output-shapes.md](./references/output-shapes.md)
- [references/target-journal-reference-corpus.md](./references/target-journal-reference-corpus.md)
