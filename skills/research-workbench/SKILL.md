---
name: research-workbench
description: Coordinate non-manuscript research project workflows.
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
  - 科研 skill 不好用
  - 科研相关 skill 不好用
  - 科研工作流优化
  - 持续优化科研流程
  - 科研外部调研校准
metadata:
  version: "1.2.0"
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
`$literature-synthesis`, `$autoresearch`,
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
- The user says the research skills are not working well and asks for continued optimization of the research workflow
- External literature or venue lookup is allowed and can change the lane decision, novelty judgment, or experiment priority

## Do not use

- The user clearly names a narrow research lane and only wants that slice:
  - early ideation only -> keep it in this workbench unless the user asks for a standalone brainstorming artifact
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
- if the user says `允许外部调研`, run only the lookup needed to change the lane decision; do not turn the whole task into generic web research
- if the user complains that a research skill is bad, treat it as a routing/workflow repair signal: identify the broken handoff, tighten the next active lane, and preserve the fix in the relevant skill or routing case

Do not make the user switch skills just because the work naturally moves from
idea to literature to experiment.

## Anti-bad-output rules

Avoid the failure modes that make research skills feel useless:

- Do not return a generic "research plan" when one concrete next action can change the project state.
- Do not ask the user to choose between literature, novelty, or experiment lanes unless that choice depends on a value judgment only they can make.
- Do not call an idea novel before checking close prior work; label it `provisional` until the nearest papers are known.
- Do not recommend experiments without naming the hypothesis, control, baseline, success metric, and failure interpretation.
- Do not bury the decision under a long taxonomy; lead with the phase, blocker, decision, and next action.
- Do not let external research become citation hoarding; use it to calibrate field state, baselines, venue expectations, or feasibility.

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

- idea expansion -> `$research-workbench`
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

When taking action rather than only advising, the minimum useful artifact is a
four-field lane card:

```text
phase:
blocker:
decision:
next_action:
```

When filesystem-backed work is needed, keep the long notes in local artifacts and
return only the phase, blocker, decision, and next action in chat.
