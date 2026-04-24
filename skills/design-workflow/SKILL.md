---
name: design-workflow
description: |
  Own design-process artifacts such as DESIGN.md capture, UI-generation prompt
  shaping, acceptance verdicts, and design workflow closure. Use when the user
  wants to first document the design system, improve a design prompt, or audit
  output against a DESIGN.md-style contract.
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - DESIGN.md
  - 设计 prompt
  - 设计提示词
  - 设计验收
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

This skill owns design-process artifacts, not direct UI implementation.

## Modes

- `capture DESIGN.md`: extract a reusable design system from existing screens,
  screenshots, or frontend code.
- `prompt generation`: turn a UI goal into a stronger implementation or image
  generation prompt.
- `acceptance verdict`: compare output against DESIGN.md or a visual contract and
  call out drift, AI-generic patterns, or missing states.

## When to use

- The user asks to first create or update `DESIGN.md`
- The user wants to improve UI generation prompts before implementation
- The user wants a design acceptance checklist or verdict
- The user wants design workflow closure rather than direct visual redesign

## Do not use

- Named-product reference grounding -> use `$design-agent`
- Direct UI redesign / implementation -> use `$frontend-design`
- Screenshot evidence collection -> use `$visual-review`
- High-end motion implementation -> use `$motion-design`

## References

- [references/design-workflow.md](references/design-workflow.md)
