---
name: design-workflow
description: |
  Own design-process artifacts after the design-system gate: UI-generation
  prompt shaping, acceptance summaries, and design workflow closure. Use when
  the user wants to improve a design prompt or close the loop against an
  already-known DESIGN.md-style contract. For creating, updating, linting,
  diffing, reading, or applying `DESIGN.md`, use `$design-md` first.
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 设计 prompt
  - 设计提示词
  - 设计验收
  - 页面生成提示词
  - design workflow
  - 风格漂移
  - AI 味
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - design
    - design-workflow
    - prompt
    - acceptance
risk: low
source: local
---

# design-workflow

This skill owns design-process artifacts after the design source of truth is
known. It does not own the `DESIGN.md` artifact itself; route that through
`$design-md`.

## Modes

- `prompt generation`: turn a UI goal into a stronger implementation or image
  generation prompt.
- `acceptance verdict`: compare output against DESIGN.md or a visual contract and
  call out drift, AI-generic patterns, or missing states.

## When to use

- The user wants to improve UI generation prompts before implementation
- The user wants a design acceptance checklist or verdict
- The user wants design workflow closure after a `DESIGN.md` or visual contract
  already exists

## Do not use

- `DESIGN.md` creation/update/lint/diff/read/application -> use `$design-md`
- Named-product reference grounding -> use `$design-agent`
- Direct UI redesign / implementation -> use `$frontend-design`
- Screenshot evidence collection -> use `$visual-review`
- High-end motion implementation -> use `$motion-design`

## References

- [references/design-workflow.md](references/design-workflow.md)
