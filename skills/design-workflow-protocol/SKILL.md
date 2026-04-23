---
name: design-workflow-protocol
description: |
  Establish a file-backed design iteration protocol that connects `DESIGN.md`, generation prompts,
  screenshots or rendered evidence, and final audit verdicts. Use when the user wants a durable
  design workflow or artifact contract instead of one-off design advice.
routing_layer: L3
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
trigger_hints:
  - 设计工件协议
  - 设计工作流
  - 设计迭代协议
  - 设计闭环
  - 工作流
  - workflow
  - prompt
  - screenshot
  - verdict
  - design workflow
  - design artifact protocol
  - DESIGN.md 到审计闭环
  - prompt 到 screenshot 到 verdict
  - screenshot 到 verdict
  - 每轮都按这个工作流跑
  - DESIGN.md 到 prompt 到 screenshot 到 verdict 的设计工件协议
metadata:
  version: "0.1.0"
  platforms: [codex, antigravity, claude-code]
  tags:
    - design-workflow
    - design-artifacts
    - protocol
    - ui-design
    - continuity
risk: low
source: local
artifact_outputs:
  - SESSION_SUMMARY.md
  - NEXT_ACTIONS.json
  - EVIDENCE_INDEX.json
---

# design-workflow-protocol

This skill owns the **file-backed workflow contract** for repeated design
iterations. It turns the design chain into a stable set of artifacts instead of
an ad hoc conversation pattern.

## When to use

- The user wants a durable design workflow, loop, or protocol
- The task is to standardize how `DESIGN.md`, design prompts, screenshots, and audit verdicts should be stored and reused
- The user wants later design iterations to be reviewable, resumable, and less context-dependent
- The design work spans multiple rounds or pages and needs stable handoff files
- The user wants a `DESIGN.md -> prompt -> evidence -> verdict` contract

## Do not use

- The user only wants to extract `DESIGN.md` from existing assets -> use `$design-md`
- The user only wants a stronger generation prompt -> use `$design-prompt-enhancer`
- The user only wants post-output design acceptance -> use `$design-output-auditor`
- The user wants direct redesign or implementation -> use `$frontend-design`
- The user wants a generic execution checklist unrelated to design artifacts -> use `$checklist-writting`

## Core workflow

1. Identify the recurring design loop:
   - single page
   - multi-page
   - repeated polish cycles
2. Freeze the minimum artifact set and ownership rules.
3. Define the round order:
   - design system
   - generation prompt
   - rendered evidence
   - design audit
   - verdict / next action
4. Define where each artifact lives and which skill owns it.
5. Define stop / continue rules so later rounds do not drift.
6. Write the protocol in a way future agents can reuse without reopening the whole thread.

## Output contract

Default output should produce:

1. `artifact tree`
2. `ownership map`
3. `round protocol`
4. `stop rules`

## Rules

- Prefer the smallest durable protocol that keeps future rounds stable.
- Keep one source of truth for design language: `DESIGN.md`.
- Keep prompt, evidence, and verdict as separate artifacts.
- Use `EVIDENCE_INDEX.json` for screenshot / render references instead of burying them in prose.
- Make the final verdict machine-readable enough to drive the next round.

## References

- [references/design-artifact-contract.md](references/design-artifact-contract.md)
- [references/design-round-protocol.md](references/design-round-protocol.md)

## Trigger examples

- "给这个项目建立一套设计工件协议。"
- "我想把 DESIGN.md 到截图审计这条链固定成工作流。"
- "帮我把 design prompt、screenshot、audit verdict 这些都规范化。"
