---
name: paper-length-tuner
description: |
  Diagnose paper length vs target page/word budget and produce a section-level
  expand-or-cut action plan. Use when the user says "砍到 X 页", "字数超了",
  "page limit", "排版限制", "需要扩展到 X 页", "哪些地方该详写哪些该精简",
  "详略调整", "篇幅不够", "超页了", or faces a fixed page constraint.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 砍到 X 页
  - 字数超了
  - page limit
  - 排版限制
  - 需要扩展到 X 页
  - 哪些地方该详写哪些该精简
  - 详略调整
  - 篇幅不够
  - 超页了
  - paper
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags: [paper, length, page-limit, budget, expand, cut, trim, section-allocation]
framework_roles:
  - planner
  - executor
  - verifier
framework_phase: 1
framework_contracts:
  emits_findings: true
  consumes_findings: false
  emits_execution_items: true
  consumes_execution_items: false
  emits_verification_results: true
risk: low
source: local
---

# Paper Length Tuner

This skill owns **paper length diagnosis and section-level expand/cut planning**
when the manuscript must fit a fixed page or word budget.

## When to use

- The user has a hard page limit (e.g. "AAAI 7 pages", "NeurIPS 9 pages")
- The manuscript is over-length and needs strategic cutting
- The manuscript is under-length and needs meaningful expansion
- The user wants a section-by-section length audit: "哪些地方该详写哪些该精简"
- The user asks "详略调整", "篇幅诊断", "砍到 X 页", "扩展到 X 页"

## Do not use

- The task is purely about prose polish without a length constraint → use `$paper-writing`
- The task is driven by reviewer comments or an issue list → use `$paper-reviser`
- The task is about paper logic or scientific soundness → use `$paper-logic`
- The task is about figure/table presentation → use `$paper-visuals`
- The task is about LaTeX compilation or preview → use `$latex-compile-acceleration`

## Handoff protocol

| Situation | Handoff target |
|---|---|
| Length plan produced; user wants actual rewriting | `$paper-writing` for prose, `$paper-reviser` for multi-surface edits |
| Need to move content into appendix or supplementary | Coordinate with `$latex-compile-acceleration` for layout |
| Expansion involves adding experiments or ablations | Advise the user; this skill does not fabricate data |
| Expansion involves figures/tables | Coordinate with `$paper-visuals` or `$scientific-figure-plotting` |

## Required workflow

### Phase 1: Length Diagnosis

1. **Measure current state.** If the source is LaTeX, run:
   ```bash
   texcount -sub -inc <main.tex>
   ```
   For non-LaTeX sources, count words per section manually or via shell tools.

2. **Record the budget.** Clarify with the user:
   - Target page count or word count
   - Conference/journal template (determines words-per-page)
   - Whether references and appendix count toward the limit
   - Camera-ready vs submission rules

3. **Build the diagnosis table:**

   | Section | Current Words | Current % | Norm % | Δ Pages | Verdict |
   |---|---|---|---|---|---|
   | Abstract | ... | ... | 2-3% | ... | ✅ OK / ⚠️ Over / ⚠️ Under |
   | Introduction | ... | ... | 10-15% | ... | ... |
   | Related Work | ... | ... | 8-12% | ... | ... |
   | Method | ... | ... | 20-30% | ... | ... |
   | Experiments | ... | ... | 25-35% | ... | ... |
   | Discussion | ... | ... | 5-10% | ... | ... |
   | Conclusion | ... | ... | 3-5% | ... | ... |

   See [references/section-budget-norms.md](references/section-budget-norms.md) for
   per-venue page limits and typical section proportions.

### Phase 2: Expand/Cut Assessment Matrix

For each section, assign one of three actions:

| Action | Symbol | Criteria |
|---|---|---|
| **Cut** | 🔻 | Section exceeds norm % AND contains redundancy, excessive background, or low-contribution content |
| **Keep** | ➡️ | Section is within ±2% of norm AND has balanced information density |
| **Expand** | 🔺 | Section is below norm % AND would benefit from deeper analysis, more evidence, or richer explanation |

Prioritization factors:
- **Reviewer reading weight**: Abstract (100%), Intro (90%+), Figures (early scan), Method (only if engaged), Appendix (rarely) — from Neel Nanda's time allocation model
- **Contribution core-ness**: core method/results are protected; background/related work are first to cut
- **Information density**: detect repetition, padding phrases, restatement of the same claim
- **Redundancy across sections**: same fact repeated in intro + method + discussion

### Phase 3: Cutting Strategy (when over-length)

Execute in strict priority order — stop as soon as the target is reached:

| Priority | Operation | Typical Savings | Risk |
|---|---|---|---|
| P1 | **Remove verbose phrases** — replace wordy constructions ("due to the fact that" → "because"; "it is worth noting that" → delete) | 3-8% | Very low |
| P2 | **Merge redundant paragraphs** — consolidate repeated explanations across sections | 5-15% | Low |
| P3 | **Compress transitions and signposting** — trim meta-commentary ("In this section we will discuss...") | 2-5% | Low |
| P4 | **Move content to appendix** — proofs, derivation details, extra ablations, data tables | 10-30% | Medium — reviewers may skip appendix |
| P5 | **Trim background / related work** — reduce literature survey depth; cite-and-move-on | 5-15% | Medium — may weaken positioning |
| P6 | **Remove lowest-priority experiments** — drop weakest ablation or least informative comparison | 10-20% | High — reduces evidence breadth |

> [!WARNING]
> Never use formatting hacks (shrinking margins, reducing font size, abusing `\vspace`) to
> meet page limits. Reviewers and ACs notice and penalize this.

### Phase 4: Expansion Strategy (when under-length)

Execute in priority order — stop as soon as the target is reached:

| Priority | Operation | Typical Gain | Quality Impact |
|---|---|---|---|
| P1 | **Deepen result analysis** — explain what each result means, not just what it is | 5-15% | High — adds interpretive value |
| P2 | **Add ablation studies or parameter sensitivity** — quantify each design choice | 10-20% | High — strengthens evidence |
| P3 | **Enrich discussion** — compare with more baselines, discuss failure modes, limitations | 5-15% | High — shows maturity |
| P4 | **Add visualizations with text** — new figures/tables + accompanying explanation | 10-20% | Medium — if meaningful |
| P5 | **Expand related work** — broaden coverage, add more positioning paragraphs | 5-10% | Low-Medium — may feel padded |

> [!CAUTION]
> Never pad with filler content. Every added sentence must carry new information or analysis.
> Do not fabricate experiments, numbers, or citations.

### Phase 5: Verification

After executing Phase 3 or Phase 4:

1. Re-run `texcount -sub -inc` to verify the new word counts
2. Rebuild the diagnosis table and confirm all sections are within target
3. Check for side effects:
   - Did cutting break any cross-references or forward/backward pointers?
   - Did expansion introduce inconsistency with existing claims?
   - Does the paper still read as a coherent unit?
4. If the paper now compiles to the correct page count, report success
5. If still off-target, iterate with the next priority level

## Output format

### 篇幅诊断报告

```markdown
## 篇幅诊断

**目标**: X pages (YYY Conference template, ~Z words/page)
**当前**: A pages (B words, 超出/不足 C words = ~D pages)

| Section | Words | % | Norm % | Verdict | Action |
|---|---|---|---|---|---|
| ... | ... | ... | ... | ... | 🔻/➡️/🔺 |

## 操作计划

### 需要缩减的 sections (优先级排序)
1. [Section] — [具体操作] — 预计节省 ~N words

### 需要扩展的 sections (优先级排序)
1. [Section] — [具体操作] — 预计增加 ~N words
```

## Hard constraints

- Do not alter scientific claims, experimental results, or mathematical derivations for length
- Do not use formatting tricks to fake compliance (margin hacks, font shrinkage, vspace abuse)
- Do not fabricate experiments, data, or citations to fill space
- Do not cut content that is required by the venue (e.g. ethics statement, reproducibility checklist)
- Preserve figure/table numbering and cross-reference integrity after any cuts
- When uncertain about a cut's impact, flag it for user review rather than executing silently
