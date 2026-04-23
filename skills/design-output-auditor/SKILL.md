---
name: design-output-auditor
description: |
  Audit generated or redesigned UI outputs against `DESIGN.md`, prompt guardrails, and anti-pattern
  bans. Use when the user wants design acceptance, visual drift detection, AI-slop checks, or a
  structured verdict on whether a page still matches the intended design system.
routing_layer: L3
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
trigger_hints:
  - 设计审计
  - 设计验收
  - 设计系统验收
  - UI 验收
  - 风格漂移
  - 风格检查
  - AI 味
  - anti-pattern
  - 反模式回流
  - 按 DESIGN.md 审计
  - 按设计系统审计
  - 按设计系统做风格漂移检查
  - design drift
  - visual drift
metadata:
  version: "0.1.0"
  platforms: [codex, antigravity, claude-code]
  tags:
    - design-audit
    - ui-design
    - visual-quality
    - acceptance
    - design-system
risk: low
source: local
artifact_outputs:
  - design_audit.md
---

# design-output-auditor

This skill decides whether a produced UI still matches the intended design
system. It is the **post-generation design acceptance** layer.

## When to use

- The user wants to know whether a page still matches `DESIGN.md`
- The user asks for design acceptance, UI sign-off, or structured design QA
- The task is to catch visual drift, AI-slop, anti-pattern relapse, or style inconsistency
- The user wants a verdict like pass / drift / fail instead of another redesign pass
- The output already exists as screenshots, rendered pages, HTML, or implemented UI

## Do not use

- The user needs visible evidence gathered first from screenshots or rendered pages at conversation start -> use `$visual-review`
- The user wants to create `DESIGN.md` from existing assets -> use `$design-md`
- The user wants to strengthen the generation prompt before creating output -> use `$design-prompt-enhancer`
- The user wants direct redesign or implementation -> use `$frontend-design`
- The task is generic implementation sign-off across code/runtime layers -> add `$execution-audit`

## Core workflow

1. Load the intended design contract:
   - `DESIGN.md`
   - generation guardrails
   - anti-pattern bans
   - relevant prompt block if available
2. Inspect the produced artifact:
   - screenshot
   - rendered page
   - HTML/CSS
   - implemented UI
3. Compare target vs output across:
   - atmosphere and density
   - palette discipline
   - typography hierarchy
   - component signatures
   - layout grammar
   - anti-pattern relapse
4. Classify each issue:
   - `match`
   - `minor drift`
   - `material drift`
   - `hard fail`
5. Return a compact verdict with the smallest rework set needed to recover design fidelity.
6. When the workflow is file-backed, write the verdict in a reusable artifact form for the next round.

## Output contract

Default output should include:

1. `overall verdict`
2. `top drift findings`
3. `anti-pattern findings`
4. `smallest rework set`

## Rules

- Judge against the declared design system, not personal taste alone.
- Separate visible evidence from inference.
- Penalize drift more than novelty when consistency is the goal.
- Anti-pattern relapse is high-severity even when the page looks superficially polished.
- Prefer the smallest fix set that restores the system.

## References

- [references/design-audit-rubric.md](references/design-audit-rubric.md)
- [references/anti-pattern-catalog.md](references/anti-pattern-catalog.md)

## Trigger examples

- "按 `DESIGN.md` 审计这个页面有没有风格漂移。"
- "这版 UI 有点 AI 味，帮我做设计验收。"
- "检查这次产物有没有反模式回流，给我一个通过/不通过结论。"
