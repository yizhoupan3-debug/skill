---
name: paper-reviewer
description: |
  Judge whether a paper is ready to submit. Use for requests like "帮我审这篇
  paper", "能不能投", "投稿前把关", "整篇严审", "paper review", "review 这篇
  paper", or "只看某个维度能不能过". Default to a strict whole-paper review; if
  the user explicitly names one dimension such as claim, math, references,
  figures, tables, language, or layout, review only that slice. This skill
  reviews and decides; it does not directly rewrite.
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 帮我审这篇 paper
  - 帮我审这篇论文
  - review 这篇 paper
  - review this paper
  - paper review
  - paper review 一下这篇稿子
  - 能不能投
  - 投稿前把关
  - 整篇严审
  - 整篇 review
  - 全文审核
  - 全流程审稿
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
  version: "3.1.0"
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

This skill owns the paper-facing judgment step: can this manuscript survive
review, what are the real blockers, and which parts should be cut, narrowed, or
left alone.

The execution model is:

- main judgment chain = serial
- evidence collection and local audits = bounded parallel sidecars
- merge-back = local to the main thread

## Use this when

- The user wants a submission-facing review, not direct editing
- The user asks whether the paper is ready, risky, or worth submitting
- The user uses mixed phrasing like `paper review`, `review this paper`, or `整篇 review`
- The user wants a strict whole-paper pass before revision
- The user explicitly wants only one review dimension judged

## Do not use

- The user wants the paper changed now -> use `$paper-reviser`
- The user wants only local wording polish -> use `$paper-writing`
- The user wants only science-level defensibility or claim-vs-evidence analysis -> use `$paper-logic`
- The user wants only figure or table presentation polish -> use `$paper-visuals`

## User-facing modes

Use one of only two external modes:

- `整篇严审`: the default for vague asks such as "帮我审一下" or "能不能投"
- `单维度审`: only when the user explicitly names one dimension such as claim, math, references, figures, tables, language, or layout

Do not expose internal gate jargon unless the user explicitly asks for it.

## What this skill should deliver

Default output should stay simple:

1. overall judgment
2. top blockers
3. what to fix, cut, hide in appendix, or stop defending

If the user wants a filesystem-backed review workflow, use the shared protocol in
[`../PAPER_GATE_PROTOCOL.md`](../PAPER_GATE_PROTOCOL.md). Treat the gate chain
as internal machinery, not as the main user interface.

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
- Use `$paper-logic` for claim, novelty, evidence, and experiment-depth subanalysis
- Use `$citation-management` for citation truth and venue calibration
- Use `$paper-visuals`, `$visual-review`, and `$pdf` for final-scale figure, table, and layout checks

## Hard rules

- Review before rewriting
- Use the hardest honest standard, not a comforting one
- Do not parallelize multiple decision gates at once
- Do not turn weak claims into wording advice
- If the strongest honest move is to cut, narrow, or move something to appendix, say so plainly
- Do not blur whole-paper review and local text polish into one owner
