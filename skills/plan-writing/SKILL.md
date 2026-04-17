---
name: plan-writing
description: |
  Write concise execution plans when the user wants a plan before implementation or review.
  Use to break down multi-step features, fixes, migrations, audits, or rollouts.
  Do not use when direct implementation is clearer or the task is trivial.
short_description: Write an execution-ready plan before implementation
allowed_tools:
  - shell
  - git
  - python
approval_required_tools:
  - git push
metadata:
  version: "2.1.0"
  platforms: [codex]
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
---
# Plan Writing

This skill owns plan creation when the user needs an explicit, execution-ready
task breakdown before implementation or review work begins.

## When to use

- The user asks for a plan, breakdown, roadmap, or implementation outline
- The task spans multiple steps, files, or risks
- The user wants "先出方案", "先拆任务", or "先别写代码，先给计划"
- The task is a feature, bug fix, refactor, audit, migration, or rollout that benefits from ordered execution
- Best for requests like:
  - "先给我一个实现计划"
  - "把这个需求拆成可执行任务"
  - "先做 plan，再动代码"

## Do not use

- The user clearly wants direct implementation with little ambiguity → use the domain skill directly
- The task is trivial enough that a separate plan adds no value
- The main task is to turn an existing PRD/spec directly into working code → use `$plan-to-code`
- The task is really about delegation strategy for subagents → use `$subagent-delegation`

## Primary operating principle

This owner should produce plans that are **master-control ready**:

1. optimize for execution handoff, not essay-style explanation
2. keep the main thread to plan shape, sequencing, and decision rationale
3. route delegation strategy questions to [`$subagent-delegation`](/Users/joe/Documents/skill/skills/subagent-delegation/SKILL.md)
4. if runtime policy blocks spawning, still design plans with sidecar-ready slices and local-supervisor fallback

## Main-thread compression contract

The main thread should contain only:

- goal
- ordered steps
- dependencies / risks
- execution owner hints
- what should happen next

## Runtime-policy adaptation

If the plan includes parallelizable side work:

- express it as bounded sidecars when runtime policy permits
- otherwise express the same structure as a local-supervisor queue

## Reference

For detailed workflow, examples, and implementation guidance, see [references/detailed-guide.md](./references/detailed-guide.md).
