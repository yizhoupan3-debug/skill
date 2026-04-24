---
name: ppt-pptx
description: |
  Build source-first editable `.pptx` decks from `deck.plan.json` through the
  Rust `ppt` CLI. Use after `$slides` when the user explicitly wants reusable
  PPTX source, outline-to-PPTX generation, Rust-authored decks, or a major
  rebuild where the plan is the source of truth.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - ppt CLI
  - deck.plan.json
  - deck.pptx
  - source-first pptx workflow
  - Rust-authored pptx
  - Rust PPTX
  - editable pptx from outline
  - 可复用 deck.plan.json
  - PPT 源码工作流
  - source first PPTX
runtime_requirements:
  commands:
    - cargo
    - libreoffice
    - soffice
metadata:
  version: "1.1.0"
  platforms: [codex]
  tags: [ppt, pptx, slides, source-first, rust]
risk: medium
source: local
---

# ppt-pptx

This skill owns source-first native PPTX authoring. The reusable
`deck.plan.json` is the source; the `.pptx` is the generated artifact.

Source Contract: `deck.plan.json` stays the source of truth. Typical high-bar
flow is:
outline -> text-owner polish -> DESIGN.md or visual contract -> deck.plan.json -> deck.pptx -> rendered
PNG -> visual-review evidence -> design-output-auditor verdict -> ppt
qa/build-qa sign-off.

## Routing Rule

Generic presentation requests go to `$slides` first. Use this skill only when
the chosen lane is reusable PPTX source, not Markdown/HTML/Beamer or in-place
PowerPoint editing.

## When to Use

- The user wants a reproducible `.pptx` generated from source.
- The deck should be rebuilt from outline, notes, or structured content.
- A reusable `deck.plan.json` is a deliverable or maintenance requirement.
- Rendered QA and design-review handoff matter.

## Do Not Use

- Generic "做个PPT" intake before format choice -> use `$slides`.
- Markdown, Slidev, Marp, or HTML/CSS slides -> use `$source-slide-formats`.
- Beamer `.tex` slides -> use `$ppt-beamer`.
- Surgical edits to an existing `.pptx` where native PowerPoint fidelity is the goal -> use `$slides`.

## Workflow

1. Confirm audience, deck goal, output directory, and source-first requirement.
2. Run the Text And Design Polishing Chain when content or visual quality matters: `$humanizer`, `$copywriting`, or `$paper-writing` for text; `$design-md`, `$frontend-design`, `$visual-review`, `$design-output-auditor`, and `$design-workflow-protocol` for the visual loop.
3. Build or update `deck.plan.json` as the source of truth.
4. Generate `.pptx` through the Rust `ppt` CLI.
5. Inspect and render the deck when layout matters.
6. Fix the source plan rather than patching generated output by hand.
7. Return final `.pptx` and source plan links only when useful.

## Design Rules

- Keep text editable where possible.
- Prefer native charts/tables/shapes over screenshots.
- Keep theme, typography, colors, and spacing consistent.
- Rust inspection boost: use the Rust inspector and rendered evidence before calling a deck done.
- Move detailed layout recipes to references.

## References

- [references/workflow.md](./references/workflow.md)
- [references/checklist.md](./references/checklist.md)
- [references/design-system.md](./references/design-system.md)
- [references/rust-cli.md](./references/rust-cli.md)
- [references/layout-patterns.md](./references/layout-patterns.md)
