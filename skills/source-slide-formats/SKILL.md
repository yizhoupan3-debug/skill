---
name: source-slide-formats
description: |
  Build presentation sources in Markdown, Slidev, Marp, or HTML/CSS and export
  them to deck artifacts. Use after the `$slides` gate when the user explicitly
  wants source-authored slides, browser/PDF export, live preview, or a
  repeatable source format instead of native editable `.pptx`.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - source slide formats
  - Markdown slides
  - Slidev
  - Marp
  - HTML slides
  - browser-matched PDF
  - presentation.html
  - source-first slides
  - 用 Markdown 做 slides
  - 根据大纲做 HTML slides
runtime_requirements:
  commands:
    - node
    - npm
    - npx
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - slides
    - markdown
    - html
    - slidev
    - marp
    - source-first
risk: low
source: local

---

# source-slide-formats

This skill owns explicit source-authored slide formats: Markdown, Slidev, Marp,
and HTML/CSS decks. It is not the generic PPT entry point.

## Routing rule

Use `$slides` first for generic "做个PPT" requests. Use this skill only after
the user or plan chooses a source format, live preview, HTML/CSS layout control,
or browser-matched PDF export.

## When to use

- The requested deliverable is a Markdown, Slidev, Marp, or HTML slide source.
- The user wants one editable text source plus repeatable export commands.
- Browser layout fidelity or PDF export from HTML matters.
- The task is cross-format slide maintenance where source consistency matters.
- Existing `.pptx` editability is not the main requirement.

## Do not use

- Generic presentation intake -> use `$slides`.
- Source-first native `.pptx` with `deck.plan.json` -> use `$slides` native PPTX lane.
- LaTeX Beamer source -> use `$ppt-beamer`.
- Existing deck repair where PowerPoint fidelity matters -> use `$slides`.

## Workflow

1. Confirm the source format: Markdown, Slidev, Marp, or HTML.
2. If visual identity, brand consistency, or reusable styling matters, route
   through `$design-md` before writing the source so tokens can become CSS
   variables, Slidev theme values, or Marp/HTML style rules.
3. Keep one source file as truth and make exports reproducible.
4. Use bundled templates when starting from scratch.
5. Render or preview before final export.
6. Report only final source/export links unless the user asks for internals.

## Design skill handoff

Use `$design-md` for HTML/Slidev/Marp decks when the user wants a branded deck,
theme consistency, design tokens, chart palettes, reusable section/title slide
grammar, or acceptance against `DESIGN.md`. Skip it for quick text-only slide
sources.

## Format Notes

- Markdown / Slidev / Marp: use `assets/slides.template.md`,
  `assets/slidev.template.md`, `scripts/setup_slidev.sh`, or
  `scripts/setup_marp.sh` when useful.
- HTML/PDF: use `assets/presentation.template.html`,
  `assets/print_pdf.template.js`, `scripts/export_pdf.js`, and
  `scripts/screenshot_slides.js` when browser-matched output matters.

## References

- [references/workflow.md](./references/workflow.md)
- [references/design-system.md](./references/design-system.md)
- [references/visual-design-principles.md](./references/visual-design-principles.md)
