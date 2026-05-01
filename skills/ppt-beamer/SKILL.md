---
name: ppt-beamer
description: |
  Create, revise, and compile presentation decks with LaTeX Beamer when you want editable
  `.tex` plus PDF instead of HTML or `.pptx`. Use when the user asks for Beamer slides,
  LaTeX 幻灯片, 学术 PPT, or needs to convert notes/papers into slides, adjust themes, fix
  Beamer compile/layout issues, or build a repeatable citation-aware slide workflow. For
  generic LaTeX compile-speed or figure-externalization work, use
  `$latex-compile-acceleration`.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - Beamer slides
  - LaTeX 幻灯片
  - 学术 PPT
  - theme 调整
  - Beamer 编译
  - papers into slides
  - adjust themes
  - fix Beamer compile
  - layout issues
  - build a repeatable citation-aware slide workflow
runtime_requirements:
  commands:
    - latexmk
    - npx
    - rsvg-convert
metadata:
  version: "2.0.0"
  platforms: [codex]
  tags:
    - beamer
    - latex
    - presentation
    - slides
    - academic

---

# PPT Beamer

## Overview

Build slide decks as LaTeX Beamer projects first, compile early, and audit the
generated PDF page by page. Default visual direction: restrained academic /
technical Beamer with deliberate cover, footline, color roles, and emphasis
boxes. Avoid startup-deck aesthetics unless the user explicitly asks for them.

## When to use

- Building or revising a presentation in LaTeX Beamer
- Converting notes, outlines, or a paper into editable `.tex` slides plus PDF
- Producing citation-heavy, academic, or technical slide decks in a source-first workflow
- Compiling a deck that should remain versionable in Git

## Do not use

- Do not use when the user wants an editable PowerPoint deck; use `$slides`
- Do not use when the final output should be HTML slides plus browser-matched PDF; use `$source-slide-formats`
- Do not use when the user wants a fast Markdown-to-slides workflow with Slidev or Marp; use `$source-slide-formats`
- Do not use when the main task is generic LaTeX compile-speed optimization across papers / books / theses / Beamer; use `$latex-compile-acceleration`

## Workflow

1. Create the Beamer workspace (`main.tex`, `assets/`, `build/`, source log, optional `refs.bib`).
2. Turn the outline into a slide plan before styling.
3. If the deck needs reusable visual identity or brand consistency, route
   through `$design-md` before theme macros so colors, typography roles,
   callout boxes, chart colors, and cover/section grammar are explicit.
4. Define the visual system early: theme, footline policy, callout boxes, cover/closing rules, type scale.
5. Choose local assets and the compilation stack (XeLaTeX by default for Chinese / mixed-language decks).
   - If compile latency is the main complaint rather than slide authoring, route to `$latex-compile-acceleration`.
6. Start from the template, compile early, and fix density in source rather than shrinking text.
7. Render/audit the PDF page by page and use `$visual-review` for overlap / hierarchy / clipping checks.
8. Deliver the `.tex`, compiled PDF, local assets, and source log together.

For the detailed compile / density / QA / Mermaid path, see
[references/workflow.md](./references/workflow.md).

## Non-Negotiables (summary)

Core rules enforced on every Beamer deck. For the full categorized list, read [references/checklist.md](./references/checklist.md).

- One message per frame — split when two unrelated points share a slide.
- Local paths only for all figures and assets.
- 16:9 aspect ratio by default; XeLaTeX + `ctex` for Chinese decks.
- Deliberate visual system in source: cover, footline, colors, emphasis boxes, closing.
- When a `DESIGN.md` exists, map it into Beamer color/font/callout macros
  before inventing new theme styling.
- Readable type outranks maximal packing — `12pt` base, `\normalsize`–`\large` body.
- No cover footer unless explicitly requested; page number centered in its footline block.
- Never fabricate results. Label missing data as planned/pending/hypothesis.
- No overflow hacks (`\tiny`, `\resizebox`). Fix in source first.
- No Chinese orphan lines (1–2 chars alone); keep mixed-language tokens intact.
- Visual QA with `$visual-review` is mandatory — log-only inspection is insufficient.

## Resource Guide

- [references/workflow.md](./references/workflow.md) — end-to-end build / compile / QA flow
- [references/design-system.md](./references/design-system.md) — theme and visual-system rules
- [references/visual-qa.md](./references/visual-qa.md) — rendered-page QA guidance
- [references/checklist.md](./references/checklist.md) — full sign-off checklist
- [`../latex-compile-acceleration/SKILL.md`](../latex-compile-acceleration/SKILL.md) — generic LaTeX compile-speed and watch-loop optimization

## Practical Defaults

- Output: editable Beamer source plus compiled PDF beside the source file
- Engine: XeLaTeX + `ctex` by default
- Layout: 16:9, one strong message per frame
- Theme: restrained academic, readable type, deliberate footline/callout system
- QA: compile → log → render PNGs → `$visual-review`

## Final Checks (summary)

See [references/checklist.md](./references/checklist.md) for the complete sign-off checklist. Key gates:

- Frame count matches PDF page count; `latexmk` exits clean.
- Rendered slides inspected through `$visual-review`; overlap explicitly checked.
- No tiny-text workarounds; body readable without zooming.
- No Chinese orphan lines; titles balanced; mixed-language tokens intact.
- Cover has deliberate hierarchy, no accidental footer; closing frame present.
- All assets local; all claims traceable; no fabricated experimental results.
