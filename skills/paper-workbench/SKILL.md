---
name: paper-workbench
description: |
  Unified front door for paper work. Use when the user has a manuscript-level
  task and should not have to choose between review, revision, logic, figures,
  or prose lanes first. Good for requests like "帮我看这篇 paper 现在能不能投",
  "根据 reviewer comments 改到能投", "先审再改", "整体推进这篇论文", or
  "这篇稿子现在该怎么处理". This skill picks the right paper lane first,
  then keeps the workflow continuous without making the user switch skills.
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 帮我审这篇 paper
  - 帮我审这篇论文
  - 帮我看这篇 paper 现在能不能投
  - 能不能投
  - 投稿前把关
  - 整篇严审
  - 整篇 review
  - paper review
  - 根据 reviewer comments 修改
  - 根据 reviewer comments 改论文
  - 按审稿意见改论文
  - 按 review 改论文
  - 根据 review 修改论文
  - 根据 reviewer comments 改到能投
  - 先审再改
  - review 完直接改
  - 整体推进这篇论文
  - 帮我处理这篇论文
  - 这篇稿子现在该怎么处理
  - 帮我把这篇 paper 弄到能投
  - 该删就删
  - 藏到附录
  - paper workflow
  - paper workbench
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags: [paper, manuscript, review, revise, submission, orchestrator]
framework_roles:
  - orchestrator
  - planner
  - verifier
framework_phase: 1
framework_contracts:
  emits_findings: true
  consumes_findings: true
  emits_execution_items: true
  consumes_execution_items: false
  emits_verification_results: true
risk: medium
source: local
---

# Paper Workbench

This skill is the one front door for paper work.

It exists so the user does not need to decide first whether the job is
`$paper-reviewer`, `$paper-reviser`, `$paper-logic`, `$paper-writing`, or
`$paper-visuals`.

## Use this when

- The user has a whole-paper task and the first move is still part of the job
- The user wants the paper judged, then possibly revised, in one continuous flow
- The user wants reviewer comments executed without manually picking the next lane
- The user says `先审再改`, `改到能投`, `整体推进这篇论文`, or similarly workflow-shaped asks
- The task may need claim narrowing, appendix routing, figure/table cleanup, or local prose polish after the main decision is clear

## Do not use

- The user explicitly wants only one narrow lane and names it clearly:
  - science defensibility only -> use `$paper-logic`
  - local text polish only -> use `$paper-writing`
  - figure/table presentation only -> use `$paper-visuals`
  - notation consistency only -> use `$paper-notation-audit`

## Default front-door behavior

Pick one external mode first, then keep the rest internal:

1. `整篇判断`
2. `按意见改稿`
3. `单维度会诊`
4. `局部精修`

Rules:

- vague whole-paper asks default to `整篇判断`
- review-driven revision asks default to `按意见改稿`
- explicit dimension asks use `单维度会诊`
- local section rewrite with fixed claim boundary uses `局部精修`

Do not make the user switch skills just because the work naturally moves from
judgment to revision.

## Internal lane map

- strict submission judgment -> `$paper-reviewer`
- claim / novelty / evidence pressure test -> `$paper-logic`
- findings-driven manuscript changes -> `$paper-reviser`
- local prose rewrite after scope is frozen -> `$paper-writing`
- figures / tables / captions / rendered presentation -> `$paper-visuals`
- notation / abbreviations / formula references -> `$paper-notation-audit`

Use [`../PAPER_GATE_PROTOCOL.md`](../PAPER_GATE_PROTOCOL.md) when the work needs
filesystem-backed whole-paper state, frozen gate decisions, or bounded parallel
lanes.

## What this skill should deliver

Keep the user-facing output simple:

1. what mode the paper is in now
2. the real blockers or active edit target
3. the next honest move

Behind the scenes, this skill may switch lanes. The user should not need to.

## Hard rules

- Do not start with prose polish when the real problem is claim or evidence
- Do not force the user to choose reviewer vs reviser before the route is clear
- Do not lose specialist rigor just because the front door is unified
- If the shortest honest path is cut, narrow, hide in appendix, or stop defending, say so plainly
