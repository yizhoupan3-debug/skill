---
name: paper-reviewer
description: |
  Specialist review lane behind `$paper-workbench`. Use when the user clearly
  wants review-only judgment, or explicitly asks for one review dimension such
  as claim, novelty, evidence, math, references, figures, tables, language, or
  layout. Also use for "paper review", "审稿", "严审", "能不能投", "投稿前把关",
  or reviewer-style critique where external literature / venue research is
  allowed to calibrate the bar. Also use when the requested review standard is
  顶刊, 顶会, CCF-A, or top-tier selective-venue acceptance. This skill reviews
  and decides; it does not directly rewrite unless the user asks to switch into
  revision.
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - $paper-reviewer
  - paper-reviewer
  - paper review
  - paper reviewer
  - 审稿
  - 审一下论文
  - 审一下 paper
  - 严审
  - 整篇严审
  - 能不能投
  - 投稿前把关
  - 投稿前 review
  - 顶刊审稿标准
  - 顶会审稿标准
  - 顶刊顶会 review
  - top-tier review
  - top conference review
  - top journal review
  - 外部调研严审
  - 查文献后审
  - reviewer-style critique
  - 只做整篇严审不改稿
  - 只做投稿判断
  - 单独做 reviewer lane
  - 全文审核
  - 只审 claim
  - 只审图表
  - 只审排版
  - 图表和排版
  - 全文 review
  - 只审数学
  - 只审引用
  - 只审语言
  - 最严厉审稿
  - reject reviewer
metadata:
  version: "4.1.0"
  platforms: [codex]
  tags: [paper, manuscript, review, reviewer, submission, gate-chain, top-journal]
framework_roles:
  - detector
  - planner
framework_phase: 1
framework_contracts:
  emits_findings: true
  consumes_findings: false
  emits_execution_items: false
  consumes_execution_items: false
  emits_verification_results: false
risk: medium
source: local

---

# Paper Reviewer

This skill is the review specialist lane behind `$paper-workbench`.

It owns the paper-facing judgment step: can this manuscript survive review,
what are the real blockers, and which parts should be cut, narrowed, or left
alone.

Default posture:

- give a usable reviewer verdict first, not a process report
- use external research when it helps judge novelty, baseline expectations,
  venue fit, or citation truth
- keep unpublished manuscript content confidential; search from title,
  abstract, keywords, visible claims, and user-approved snippets rather than
  uploading the whole draft to public third-party services
- treat target venue and article type as the bar; if missing, proceed with a
  provisional bar and label it as provisional instead of blocking
- when the user asks for top-tier readiness, apply the selective-venue bar from
  `$paper-workbench` before allowing language or layout issues to dominate
- review before rewriting; switch to `$paper-reviser` only after findings are
  accepted or the user explicitly asks for edits

## Use this when

- The user explicitly wants review-only judgment, not a continuous review-to-edit workflow
- The user asks whether the paper is ready, risky, or worth submitting
- The user wants a strict whole-paper pass before any edit decisions are opened
- The user explicitly wants only one review dimension judged
- The user allows or requests external research for related work, novelty,
  target-journal norms, or baseline expectations
- The user asks whether the paper reaches 顶刊/顶会/top-tier standards

## Do not use

- The user wants the front door for a paper task -> use `$paper-workbench`
- The user wants the paper changed now -> use `$paper-reviser`
- The user wants only local wording polish -> use `$paper-writing`
- The user wants only local wording polish -> use `$paper-writing`

## User-facing modes

Use one of only two external modes:

- `整篇严审`: the default for vague asks such as "帮我审一下" or "能不能投"
- `单维度审`: only when the user explicitly names one dimension such as claim, math, references, figures, tables, language, or layout

Do not expose internal gate jargon unless the user explicitly asks for it.

For single-dimension checks, use
[`references/review-dimensions.md`](references/review-dimensions.md).

## What this skill should deliver

Default output should be decision-first and short enough to act on:

1. verdict: `可投 / 大修后再投 / 不建议投 / 需要补关键证据`
2. top blockers: the few issues most likely to trigger rejection
3. evidence gap: what is missing, unfair, weakly controlled, or overclaimed
4. external calibration: closest prior work / venue norm / baseline expectation
   only when external research was used
5. next honest move: fix, cut, narrow, move to appendix, or stop defending

For 顶刊/顶会/top-tier asks, also include the compact card from
[`../paper-workbench/references/top-tier-paper-standard.md`](../paper-workbench/references/top-tier-paper-standard.md):

```text
target_bar:
top_contribution:
closest_reject_case:
missing_decisive_evidence:
claim_ceiling:
next_honest_move:
```

Use severity only as plain reviewer priority:

- `A 致命`: likely reject unless repaired or narrowed
- `B 需补`: fixable but needs data, analysis, baseline, citation, or proof
- `C 表达/呈现`: wording, organization, figure/table, or layout issue after the
  claim boundary is safe

If the user wants a filesystem-backed review workflow, or the manuscript review
will span multiple turns, use the shared protocol in
[`../PAPER_GATE_PROTOCOL.md`](../PAPER_GATE_PROTOCOL.md). Treat the gate chain
as internal machinery, not as the main user interface.

## External research rules

External research is allowed by default for review calibration when network
access is available.

Use it for:

- target venue scope, article type, page / disclosure / artifact expectations
- closest prior work, recent competing papers, and required baselines
- citation existence, citation precision, and whether references are current
- field-specific review norms such as reproducibility, ethics, statistics, or
  data availability

Do not use it to:

- upload an unpublished full manuscript, confidential data, or private review
  material to public AI / plagiarism / detector tools without explicit approval
- pad the review with generic source lists
- replace reading the manuscript's own claims, methods, figures, and tables

Prefer official venue pages, publisher reviewer guidance, DOI/proceedings pages,
PubMed/PMC where relevant, arXiv only when the field moves fast, and scholarly
discovery indexes for expansion.

## Review workflow

For normal interactive review, use this compressed order:

1. Lock the bar: target venue, article type, audience, and constraints. If
   absent, infer a provisional bar and say so.
2. Extract the paper's claim map: main claim, contribution bullets, decisive
   evidence, figures/tables, baselines, and limitations.
3. Run external calibration: closest prior work, expected baselines, recent
   norms, and venue fit.
4. Make the kill decision: identify the shortest reviewer path to reject and
   whether it is genuinely fatal.
5. Separate fix types: new evidence, claim narrowing, appendix routing,
   citation repair, figure/table/layout repair, or prose cleanup.
6. Report only the actionable conclusion unless the user asks for the full
   audit trail.

In protocol mode, prefer `串行主链 + 并行 sidecar lane`:

- keep claim and disposition decisions on the main chain
- spin up bounded sidecar lanes for citations, figures, tables, notation, layout, and mirror-surface checks
- merge sidecar outputs locally before deciding pass, fail, or backjump

Use the bundled scaffold helper when you need to materialize a parallel batch on
disk:

`python3 /Users/joe/Documents/skill/scripts/paper_lane_scaffold.py ...`

## Internal routing notes

- For whole-paper review, use the protocol-backed full-chain flow
- For explicit dimension review, inspect only that slice and do not silently expand scope
- For whole-paper review, parallelize only bounded audit lanes under the current active gate
- Use `logic mode` for claim, novelty, evidence, and experiment-depth subanalysis
- Keep heavier external corpus or novelty sweeps inside the current paper
  workflow; do not force the user to switch skills for a normal reviewer lookup
- When a review reveals story/positioning weakness but the user wants target-journal imitation before rewriting, hand back to `$paper-workbench` ref-first workflow rather than doing prose edits here
- Use `$citation-management` for citation truth and venue calibration
- Use `figure-table mode`, `$visual-review`, and `$pdf` for final-scale figure, table, and layout checks

## Hard rules

- Review before rewriting
- Use the hardest honest standard, not a comforting one
- Do not parallelize multiple decision gates at once
- Do not turn weak claims into wording advice
- Do not block the review just because target venue or reference set is missing;
  proceed provisionally and mark the uncertainty
- Do not make the final answer a gate-progress report unless the user asked for
  protocol artifacts
- If the strongest honest move is to cut, narrow, or move something to appendix, say so plainly
- Do not blur whole-paper review and local text polish into one owner
- Do not call a paper "top-tier ready" unless contribution, closest-work
  separation, decisive evidence, and reviewer attack resistance all survive.
