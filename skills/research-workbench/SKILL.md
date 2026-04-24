---
name: research-workbench
description: |
  Unified front door for non-manuscript research-project work. Use when the user
  has a 科研项目 / 课题 / research direction task and should not have to choose first
  between brainstorming, literature search, novelty checking, experiment
  planning, reproducibility, statistics, figures, technical critique, or AI/ML
  research engineering. Good for asks like
  "帮我推进这个科研方向", "这条研究线下一步怎么做", "从想法到实验一起编排", "整体推进这个 research project",
  or "这个课题现在该先搜文献还是先做实验". This skill picks the right research
  lane first, then keeps the workflow continuous without making the user switch
  skills.
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: preferred
trigger_hints:
  - 帮我推进这个科研方向
  - 这条研究线下一步怎么做
  - 从想法到实验一起编排
  - 整体推进这个 research project
  - 这个课题现在该先搜文献还是先做实验
  - research workflow
  - research workbench
  - research project
  - 科研项目
  - 科研编排
  - 科研方向
  - 课题下一步
  - 研究路线
  - 实验设计
  - 补实验
  - novelty gate
metadata:
  version: "1.1.0"
  platforms: [codex]
  tags: [research, workflow, orchestrator, literature, experiments, statistics]
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

# Research Workbench

This skill is the one front door for non-manuscript research-project work.

It exists so the user does not need to decide first whether the job is
`$brainstorm-research`, `$literature-synthesis`, `$autoresearch`,
`$ai-research`, `$research-engineer`,
`$experiment-reproducibility`, `$statistical-analysis`, or
`$scientific-figure-plotting`.

It should not swallow paper-manuscript work. If the object is a draft paper,
reviewer comments, submission readiness, related sections, figures inside a
manuscript, or "这篇论文", route to `$paper-workbench` first.

## Use this when

- The user has a research-project task and the first move is still part of the job
- The user wants help deciding whether to brainstorm, search, synthesize, implement, critique, or run experiments next
- The task naturally crosses multiple research phases in one flow
- The user says `整体推进这个 research project`, `从想法到实验一起编排`, `这条研究线下一步怎么做`, or similarly workflow-shaped asks
- The task may need literature search, novelty checking, experiment planning, reproducibility, statistics, or figure work, but the first active lane is not obvious yet
- The user says `课题`, `科研方向`, `研究路线`, `补实验`, or `下一步怎么做` without making the object a manuscript

## Do not use

- The user clearly names a narrow research lane and only wants that slice:
  - early ideation only -> use `$brainstorm-research`
  - paper search only -> use `$literature-synthesis`
  - literature synthesis or novelty check only -> use `$literature-synthesis`
  - autonomous experiment loop -> use `$autoresearch`
  - AI/ML system implementation -> use `$ai-research`
  - theory-heavy critique -> use `$research-engineer`
  - statistics only -> use `$statistical-analysis`
  - code-generated figures only -> use `$scientific-figure-plotting`
- The user has a manuscript-level task -> use `$paper-workbench`
- The user wants citation/reference hygiene only -> use `$citation-management`
- The user wants generic non-academic external research, product comparison, or web source gathering -> use `$information-retrieval`
- The user wants a homework/course-project compliance check rather than research ownership -> use `$assignment-compliance`

## Default front-door behavior

Pick one external mode first, then keep the rest internal:

1. `先定方向`
2. `先找文献`
3. `先判新意`
4. `先做实验`
5. `先补严谨性`

Rules:

- vague project asks default to `先定方向`
- corpus-first asks default to `先找文献`
- novelty/risk asks default to `先判新意`
- execution-first asks default to `先做实验`
- rigor/failure-analysis asks default to `先补严谨性`

Do not make the user switch skills just because the work naturally moves from
idea to literature to experiment.

## Boundary gates

Use these gates before picking a lane:

| Object in the request | Route |
|---|---|
| `这篇论文`, manuscript, reviewer comments, submission readiness | `$paper-workbench` |
| bibliography, `.bib`, DOI, citation style, reference truth | `$citation-management` |
| academic paper search, related work, novelty check | `$literature-synthesis` |
| autonomous repeated hypothesis experiments | `$autoresearch` |
| generic web/product/technology research | `$information-retrieval` |
| research-project next-step orchestration | `$research-workbench` |

## Internal lane map

- idea expansion -> `$brainstorm-research`
- paper discovery / synthesis / novelty / related work -> `$literature-synthesis`
- autonomous experiment loop -> `$autoresearch`
- AI/ML build and evaluation -> `$ai-research`
- technical correctness critique -> `$research-engineer`
- reproducibility discipline -> `$experiment-reproducibility`
- statistics and uncertainty -> `$statistical-analysis`
- code-generated figures -> `$scientific-figure-plotting`
- manuscript-facing work -> `$paper-workbench`
- citation truth / bibliography hygiene -> `$citation-management`

## What this skill should deliver

Keep the user-facing output simple:

1. what phase the research task is really in now
2. the real blocker or next active lane
3. the next honest move

When filesystem-backed work is needed, keep the long notes in local artifacts and
return only the phase, blocker, decision, and next action in chat.
