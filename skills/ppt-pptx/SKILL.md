---
name: ppt-pptx
description: |
  Create source-first `deck.js` plus editable `.pptx` decks with PptxGenJS,
  theme-driven styling, local assets, and rendered QA. Use after the `$slides`
  gate when the user explicitly wants a reusable code-authored PPTX workflow,
  outline-to-`deck.js` generation, or a major rebuild where `deck.js` should
  become the source of truth. Not for generic PPT intake or surgical in-place
  edits of an existing deck.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - PptxGenJS
  - deck.js
  - source-first pptx workflow
  - code-authored pptx
  - 重做这份 deck 成 deck.js
  - 可复用 deck.js
  - PPT 源码工作流
runtime_requirements:
  commands:
    - cargo
    - fc-list
    - gs
    - heif-convert
    - inkscape
    - libreoffice
    - magick
    - node
    - npm
    - soffice
metadata:
  version: "1.0.0"
  platforms: [codex, antigravity]
  tags:
    - ppt
    - pptx
---

# PPT PPTX

Build presentations as reusable `deck.js` sources that emit real editable `.pptx` decks. Use PptxGenJS, explicit theme fonts, reusable layout helpers, and rendered review so the delivered deck stays editable while the authoring workflow stays reproducible.

When an existing `.pptx` is the rebuild input, this skill can also use local `officecli` as an optional deep inspection / preview / patch layer before or alongside the rebuild.

Do not claim generic "PPT / PowerPoint / pptx" intake. Let `$slides` absorb those requests first, then use this owner only when the task is explicitly the code-authored `deck.js` lane or a rebuild into that lane.

Quick lane rule:

- Generic PPT request, existing deck, or format still unclear -> `$slides`
- Explicit `deck.js` / PptxGenJS rebuild -> `ppt-pptx`

## When to use

- The user wants a native editable `.pptx` file
- The deck will be edited by non-technical collaborators in PowerPoint
- Visual polish, overflow detection, and font QA are required
- The user provides a YAML/JSON outline and wants automated deck generation
- The user wants to rebuild or substantially redesign an existing deck into a cleaner source-first `deck.js` + `.pptx` workflow
- The user explicitly asks for PptxGenJS, `deck.js`, a reusable PPT source workflow, or a code-owned rebuild

## Do not use

- Do not use for generic "做个PPT" / "PowerPoint" / "pptx" requests with no workflow decision yet; check `$slides` first
- Do not use when the user wants HTML slides plus browser-matched PDF; use `$ppt-html-export`
- Do not use when the user wants LaTeX Beamer source plus PDF; use `$ppt-beamer`
- Do not use when the user wants a fast Markdown-to-slides workflow with Slidev or Marp; use `$ppt-markdown`
- Do not use when the main job is to inspect, query, patch, or batch-edit an existing `.pptx` in place; keep it on the native `$slides` lane
- Do not use for requests like "把第 7 页标题改一下", "批量替换这份 deck 里的年份", "检查这个现有 PPT 有没有溢出", or "把这个表格宽一点"

## Overview

Default visual direction: black-luxury, editable, and presentation-distance
readable. Think dark canvas, intentional image embedding, restrained accents,
and a closing slide that echoes the cover. If the user wants seminar /
论文汇报气质 by default, they usually want `$ppt-beamer` instead.

### Template Gallery

- **Dark Luxury** → `assets/deck.template.js` (default)
- **Light Academic** → `assets/template_light.js`
- **Corporate** → `assets/template_corporate.js`

Pick the template that matches the tone; see `references/design-system.md` for
the full style guidance.

## Workflow

1. Create the deck workspace (`deck.js`, `deck.pptx`, `assets/`, rendered QA outputs, source log).
2. Turn the outline into a slide plan before styling.
3. Define the visual system early: palette, fonts, 2–3 reusable layout families, cover/closing language.
4. Collect local assets first; do not leave remote image URLs in the final deck.
5. Build in PptxGenJS from the bundled templates/helpers and keep everything editable.
6. Render, test overflow/fonts, and audit slide PNGs with `$visual-review`.
7. Deliver the `.pptx`, authoring `.js`, local assets, and source log together.

Default CLI:

- use `ppt init <workdir>` to create a deck workspace from the Rust CLI
- use `ppt outline <outline.yaml|outline.json> --bootstrap --build` to turn an outline into `deck.js` and `deck.pptx`

Optional boost for rebuild / audit work:

- use `ppt office doctor` for outline + issues + validation in one pass
- use `ppt office get/query` when you need stable `shape[@id=...]` addressing from an old deck
- use `ppt office watch` for live HTML preview of an already-generated `.pptx`

Default mixed lane:

- use `ppt build-qa --workdir .` after `deck.js` authoring
- use `ppt qa <deck.pptx> --rendered-dir rendered` for an existing generated deck
- use `ppt intake <deck.pptx>` before rebuilding an old `.pptx`

For the detailed source-first workflow, rebuild path for existing decks, notes /
transitions guidance, and QA sequence, see [references/workflow.md](./references/workflow.md).

Layering rule:

- `ppt-pptx` owns new deck authoring and big redesigns where the source of truth should become `deck.js`
- `$slides` is the companion gate for existing-deck inspection, surgical edits, and native editable-PowerPoint workflows that should stay artifact-first

## Non-Negotiables (summary)

Core rules for every PPTX deck. Full categorized list in [references/checklist.md](./references/checklist.md).

- Deliver a real editable `.pptx`, not only PDF or screenshots.
- Local paths for all assets; explicit theme fonts.
- Default to cross-platform fonts that exist on both macOS and Windows.
- Prefer `Arial` for UI/body/headings and `Courier New` for code; do not hardcode `Helvetica Neue`, `Calibri`, or `Consolas` as deck defaults.
- Declared visual system before dense content; 2–3 reusable layout families.
- Cover: softened/blurred background + dark protection; closing echoes cover.
- No Chinese orphan lines (1–2 chars). Proactively rewrite to fix.
- Mixed-language tokens stay intact; headings visually balanced.
- Images feel embedded (framed, overlayed), not pasted. Intentional crops only.
- On dark slides, readability is a hard constraint — no gray-on-black body text.
- Include `warnIfSlideHasOverlaps()` and `warnIfSlideElementsOutOfBounds()`.
- Rendered-slide QA with `$visual-review` before delivery.

## Resource Guide

- [references/workflow.md](./references/workflow.md) — end-to-end build / rebuild / QA flow
- [references/design-system.md](./references/design-system.md) — aesthetics, hierarchy, template choice
- [references/layout-patterns.md](./references/layout-patterns.md) — reusable slide compositions
- [references/visualization_patterns.md](./references/visualization_patterns.md) — chart / diagram selection
- [references/pptxgenjs-helpers.md](./references/pptxgenjs-helpers.md) — helper APIs and scripts
- [references/install.md](./references/install.md) — setup and dependency fixes
- [references/checklist.md](./references/checklist.md) — full sign-off checklist

## Practical Defaults

- Output: polished `.pptx` plus matching authoring `.js`
- Engine: PptxGenJS, 16:9 wide
- Visual default: black-luxury; use template variants only when tone demands it
- Density default: 2–4 panels or one wide evidence surface per slide
- QA default: render → overflow/font checks → `$visual-review` → sign-off

## Final Checks (summary)

See [references/checklist.md](./references/checklist.md) for the complete sign-off checklist. Key gates:

- Delivered `.pptx` is real and editable; slide count matches plan.
- No tiny-text workarounds; no Chinese orphan lines; titles balanced.
- Images intentional; fonts correct; overlap/bounds checks passed.
- Rendered slides reviewed through `$visual-review`.
- Cover/section/closing feel like one deck; no decorative empty space.
- Dark-slide body text has strong contrast for projector readability.
