# Literature Synthesis — Output Shape Reference

Quick reference for choosing the right output format.

## Output Shape Decision Tree

| User Request | Output Shape |
|---|---|
| 快速概览 / field overview | A. Topic Scan |
| 先下载ref / 目标期刊相近20篇 | B. Target-Journal Reference Corpus |
| 这个思路行不行 / novelty check | C. Idea Novelty Check |
| 对比表 / comparison | D. Comparison Matrix |
| related work 初稿 | E. Related Work Draft |
| 还有什么可以做 / gap analysis | F. Research-Gap Memo |

## Novelty Risk Levels

| Level | Icon | Meaning |
|---|---|---|
| High overlap | 🔴 | Published paper covers same claim |
| Partial overlap | 🟡 | Same method/different task, or vice versa |
| Low overlap | 🟢 | No close match found |

## Comparison Matrix Template

| Paper | Task | Core Method | Data | Main Strength | Main Limitation |
|---|---|---|---|---|---|

### A. Topic Scan
1. Scope definition
2. Main clusters or schools
3. Representative papers
4. What the field already does well
5. Current bottlenecks
6. Worthwhile next reading

### B. Target-Journal Reference Corpus
Structure:
1. Target venue and manuscript scope
2. Search queries and source types
3. 20-paper retained corpus split into target-neighbor, competitor, and canonical buckets
4. Venue story norm: abstract shape, intro funnel, evidence order, limitation style
5. Must-match baselines, metrics, terminology, and citation anchors
6. Writing implications for the user's paper

### C. Idea Novelty Check
**5-phase protocol:**
1. **Claim Extraction**: decompose idea into 3-7 atomic novelty claims with axes
2. **Systematic Search**: broad-to-narrow search per claim via Semantic Scholar, Google Scholar, arXiv
3. **Claim-by-Claim Comparison**: overlap level (🔴 high / 🟡 medium / 🟢 low) per claim vs closest prior work
4. **Novelty Scoring Matrix**: verdicts per claim (Novel / Defensible / Risky / Not novel)
5. **Novelty Risk Report**: overall assessment, strongest claims, claims to drop, positioning strategy

### D. Comparison Matrix
Recommended columns:
| Paper | Task | Core Method | Data / Benchmark | Main Strength | Main Limitation | Best Use Case |
| --- | --- | --- | --- | --- | --- | --- |
Add or replace columns only when the topic requires it.

### E. Related Work Draft
Structure:
1. One framing paragraph for the problem
2. 2-4 clusters of prior work
3. Each cluster: core idea, representative papers, strengths, limitations
4. Final contrast paragraph that positions the target work

### F. Research-Gap Memo
Structure:
1. Stable findings from the literature
2. Repeated weaknesses or blind spots
3. Contradictions or unresolved debates
4. High-value open questions
5. Suggested project angles ranked by feasibility and novelty
