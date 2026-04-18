---
name: checklist-writting
description: |
  Write an execution-ready checklist after the strategy is already fixed.
  Use for implementation outlines, experiment roadmaps, claim-closure plans, and multi-agent checklists
  that must state serial vs parallel structure explicitly.
short_description: Write a versioned execution-ready checklist once the strategy is fixed.
allowed_tools:
  - shell
  - git
  - python
approval_required_tools:
  - git push
metadata:
  version: "2.2.0"
  platforms: [codex, claude]
  tags:
    - planning
    - task-breakdown
    - execution-plan
    - verification
framework_roles:
  - planner
framework_phase: 1
framework_contracts:
  emits_findings: false
  consumes_findings: true
  emits_execution_items: true
  consumes_execution_items: false
  emits_verification_results: false
risk: low
source: local
routing_layer: L1
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
trigger_hints:
  - 先写 checklist
  - 写成 checklist md
  - 写成 checklist 文件
  - 放到 checklist 目录
  - 执行清单
  - 拆执行项
---
# Checklist Writting

This skill owns checklist creation when the user needs an explicit,
execution-ready markdown checklist before implementation or review work begins.
Its output should be a versioned checklist file under a `checklist/` directory,
using the naming pattern `cl_v<version>.md`.

This skill is the file-backed downstream for:
- [`$paper-reviewer`](/Users/joe/Documents/skill/skills/paper-reviewer/SKILL.md) when the user wants review findings materialized into a checklist file
- [`$checklist-normalizer`](/Users/joe/Documents/skill/skills/checklist-normalizer/SKILL.md) when the checklist shape is already normalized and now needs a persisted checklist artifact

## When to use

- The user asks for a plan, checklist, breakdown, roadmap, or implementation outline before execution
- The strategic route is already settled and the main need is execution decomposition
- The user wants a markdown checklist written to disk instead of a loose chat answer
- The task spans multiple steps, files, or risks
- The user wants "先出方案", "先拆任务", "先别写代码，先给计划", or "先写 checklist"
- The user wants to review the latest checklist and decide whether to generate the next round or end the task
- The user wants explicit serial vs parallel structure, especially for multi-agent work
- The task is a feature, bug fix, refactor, audit, migration, rollout, experiment push, or claim-closure track that benefits from execution-ready checklist structure
- Best for requests like:
  - "先给我一个实现计划"
  - "把这个需求拆成可执行任务"
  - "先做 checklist，再动代码"
  - "写成 checklist md，并告诉我要开几个 agent"

## Do not use

- The user clearly wants direct implementation with little ambiguity → use the domain skill directly
- The task is trivial enough that a separate checklist adds no value
- The user is still asking which route to take, what assumptions hold, or whether the plan itself is sound → use `$idea-to-plan`
- The main task is to turn an existing PRD/spec directly into working code → use `$plan-to-code`
- The user already has a checklist / phase plan / execution blueprint, and the main need is to normalize serial/parallel structure, goals, constraints, acceptance, or update rules → use `$checklist-normalizer`
- The task is really about delegation strategy for subagents → use `$subagent-delegation`

## Primary operating principle

This owner should produce checklists that are **master-control ready**:

1. optimize for execution handoff, not essay-style explanation
2. assume the strategic route is already fixed and do not reopen route selection
3. write checklist points, not vague paragraphs
4. treat peer checklist points as parallel by default
5. if work is serial, keep the whole ordered chain inside one point no matter how long it is
6. after writing the markdown, explicitly tell the user how many agents to start, which equals the number of parallel points
7. route delegation strategy questions to [`$subagent-delegation`](/Users/joe/Documents/skill/skills/subagent-delegation/SKILL.md)
8. if runtime policy blocks spawning, still design checklists with sidecar-ready slices and local-supervisor fallback

## Main-thread compression contract

The main thread should contain only:

- checklist goal
- parallel points
- serial structure inside each point
- dependencies / risks
- agent count after the markdown is written
- what should happen next

## Runtime-policy adaptation

If the checklist includes parallelizable side work:

- express each parallel point as its own bounded sidecar when runtime policy permits
- otherwise express the same structure as a local-supervisor queue

## File-output rule

Unless the user explicitly asks for chat-only output, write the checklist into a `checklist/` directory with versioned filenames:

- `checklist/cl_v1.md`
- `checklist/cl_v2.md`
- `checklist/cl_v3.md`

If `checklist/` does not exist, create it before writing.

For review requests, inspect the latest checklist file first. Prefer `cl_v*.md`, and if no new-format file exists, fall back to legacy `checklist_v*.md`. Then decide one of:
- `Continue` → write the next round checklist as `cl_v<next>.md`
- `Converged` → explicitly tell the user the task is finished and do not write a new checklist
- `Blocked` → explain what evidence is missing before another round can be planned

After writing the markdown, explicitly tell the user:
- how many agents to start
- which checklist points are parallel
- which ordered steps remain serial inside a single point

## Reference

For detailed workflow, examples, and implementation guidance, see [references/detailed-guide.md](./references/detailed-guide.md) and the versioned checklist templates under [checklist/](./checklist/).
