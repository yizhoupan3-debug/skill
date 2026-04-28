---
name: checklist-planner
description: |
  Create or normalize execution-ready checklists. Use when the user asks for
  a checklist, roadmap, implementation outline, or wants an existing checklist
  cleaned up with serial/parallel boundaries, acceptance criteria, constraints,
  stop conditions, and update rules.
routing_layer: L1
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
trigger_hints:
  - checklist
  - 先给我一个 checklist
  - 写成 checklist
  - execution-ready checklist
  - 规范化 checklist
  - 串行的写在一点
  - 并行的拆开
  - 补齐验收和约束
short_description: Create or normalize execution-ready checklists before execution starts.
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - checklist
    - planning
    - normalization
    - execution-plan
    - acceptance-criteria
risk: low
source: local
allowed_tools:
  - shell
  - git
  - python
approval_required_tools:
  - git push
---

# checklist-planner

This skill is the single owner for checklist planning shape work. It replaces
separate "write a checklist" and "normalize a checklist" routes.

## Modes

- `new checklist`: the user has a goal/spec but no execution-ready checklist yet.
- `normalization`: the user has a messy checklist, phase plan, roadmap, or lane
  sketch that needs serial/parallel boundaries, acceptance, constraints, or
  update rules.
- `versioned file`: the user wants the checklist persisted under `checklist/`
  as `cl_v<N>.md`.

## When to use

- The user asks for a plan, checklist, breakdown, roadmap, or implementation outline before execution
- The user wants a markdown checklist written to disk
- The user wants serial work kept in one point and parallel work split apart
- The checklist is missing goals, constraints, deliverables, acceptance, exit conditions, or stop conditions
- The user wants agent grouping or post-execution update rules made explicit

## Do not use

- The user wants to execute checklist items now -> use `$checklist-fixer`
- The user wants direct implementation from a stable spec -> use `$plan-to-code`
- The strategy is still unsettled -> use `$idea-to-plan`
- Root cause is unknown and investigation is needed -> use `$systematic-debugging`
- The task is really about runtime delegation strategy -> use `$subagent-delegation`

## Output rules

Unless the user explicitly asks for chat-only output, write the checklist under
`checklist/` using the next `cl_v<N>.md` filename. Peer checklist points are
parallel by default unless an explicit dependency says otherwise. Long serial
chains belong inside one checklist point.

After writing the checklist, state the recommended execution shape: which points
are parallel, which ordered steps remain serial, and whether execution should
move to `$checklist-fixer`.

## References

- [references/checklist-template.md](references/checklist-template.md)
