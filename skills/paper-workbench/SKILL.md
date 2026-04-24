---
name: paper-workbench
description: |
  Unified front door for paper work. Use when the user has a manuscript-level
  task and should not have to choose between review, revision, logic, figures,
  or prose lanes first. Good for requests like "帮我看这篇 paper 现在能不能投",
  "根据 reviewer comments 改到能投", "先审再改", "整体推进这篇论文", or
  "这篇稿子现在该怎么处理". Also use when manuscript preparation should start
  from target-journal refs, e.g. "先下载20篇目标期刊相近ref再写" or "学ref讲故事".
  "paper review不好用，彻底优化", or "允许外部调研". This skill picks the right
  paper lane first, allows external literature / venue lookup when useful, and
  keeps the workflow continuous without making the user switch skills.
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
  - paper review 不好用
  - paper review优化
  - paper reviewer优化
  - 外部调研 paper review
  - 允许外部调研
  - 查文献后审 paper
  - review with external research
  - 根据 reviewer comments 修改
  - 根据 reviewer comments 改论文
  - 按审稿意见改论文
  - 按 review 改论文
  - 根据 review 修改论文
  - 根据 reviewer comments 改到能投
  - 先审再改
  - review 完直接改
  - 整体推进这篇论文
  - 这篇论文
  - 这篇论文 该审
  - 这篇论文 该改
  - 该补实验
  - 先下载20篇目标期刊相近ref再写
  - 先找目标期刊ref再改论文
  - 学ref讲故事
  - 目标期刊写作套路
  - 论文故事线整体调整
  - 帮我处理这篇论文
  - 这篇稿子现在该怎么处理
  - 帮我把这篇 paper 弄到能投
  - 该删就删
  - 藏到附录
  - paper workflow
  - paper workbench
metadata:
  version: "1.2.0"
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
- The user wants to prepare or rewrite a manuscript by first learning target-journal reference papers
- The user says `先审再改`, `改到能投`, `整体推进这篇论文`, or similarly workflow-shaped asks
- The task may need claim narrowing, appendix routing, figure/table cleanup, or local prose polish after the main decision is clear

## Do not use

- The user wants to advance a non-manuscript research project, topic, or experiment plan -> use `$research-workbench`
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
5. `先学ref再写`

Rules:

- vague whole-paper asks default to `整篇判断`
- review-driven revision asks default to `按意见改稿`
- target-journal ref-first asks default to `先学ref再写`
- explicit dimension asks use `单维度会诊`
- local section rewrite with fixed claim boundary uses `局部精修`

Do not make the user switch skills just because the work naturally moves from
judgment to revision.

For review-like asks, do not block on missing target venue or reference corpus:
start with a provisional bar, run external calibration when useful, and clearly
separate "known blocker" from "uncertainty that needs lookup".

## Internal lane map

- strict submission judgment -> `$paper-reviewer`
- claim / novelty / evidence pressure test -> `$paper-logic`
- target-journal ref corpus and story-norm extraction -> `$literature-synthesis`, then `$paper-writing`
- external calibration during review -> keep the main owner here or in
  `$paper-reviewer`; use `$literature-synthesis` only when the lookup becomes a
  full corpus / novelty sweep
- findings-driven manuscript changes -> `$paper-reviser`
- local prose rewrite after scope is frozen -> `$paper-writing`
- figures / tables / captions / rendered presentation -> `$paper-visuals`
- notation / abbreviations / formula references -> `$paper-notation-audit`

Use [`../PAPER_GATE_PROTOCOL.md`](../PAPER_GATE_PROTOCOL.md) when the work needs
filesystem-backed whole-paper state, frozen gate decisions, or bounded parallel
lanes.

For target-journal ref-first writing, use
[`references/ref-first-writing-workflow.md`](references/ref-first-writing-workflow.md)
as the compact workflow contract.

## What this skill should deliver

Keep the user-facing output simple:

1. what mode the paper is in now
2. the real blockers or active edit target
3. the next honest move

Behind the scenes, this skill may switch lanes. The user should not need to.

## Ref-first manuscript workflow

When the user wants to learn target-journal references before writing:

1. Route to `$literature-synthesis` to build the 20-paper target-journal corpus and ref-learning brief.
2. Route to `$paper-logic` only if the corpus exposes a claim/evidence or novelty mismatch.
3. Route to `$paper-writing` for story spine, section plan, and bounded prose rewrite.
4. Keep `$citation-management` for final citation truth and `.bib` hygiene, not for the initial story-learning pass.

The handoff artifact should be simple:

```text
target venue -> 20-ref corpus -> venue story norm -> our paper's story spine -> sections to rewrite
```

In filesystem-backed work, the stable artifacts are:

- `refs/ref_learning_brief.md`
- `paper_story/STORY_CARD.md`
- `paper_story/SECTION_REWRITE_PLAN.md`
- rewritten manuscript sections or patch notes

## Hard rules

- Do not start with prose polish when the real problem is claim or evidence
- Do not let ref learning turn into sentence copying or citation padding
- Do not force the user to choose reviewer vs reviser before the route is clear
- Do not lose specialist rigor just because the front door is unified
- Do not turn a normal paper review into a process-heavy gate report; lead with
  verdict, blockers, external calibration, and next honest move
- If the shortest honest path is cut, narrow, hide in appendix, or stop defending, say so plainly
