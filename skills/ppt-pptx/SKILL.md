---
name: ppt-pptx
description: |
  Build reproducible Rust-authored `deck.plan.json` sources and editable
  `deck.pptx` decks through the `ppt` CLI, with local assets, theme-driven
  styling, Rust inspection, rendered QA, and design-review handoff. Use after
  the `$slides` gate when the user explicitly wants a source-first PPTX
  workflow, outline-to-PPTX generation, or a major rebuild where the plan
  becomes the source of truth. Not for generic PPT intake or surgical in-place
  edits of an existing deck.
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
  - 重做这份 deck 成 Rust source plan
  - 可复用 deck.plan.json
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
    - soffice
metadata:
  version: "1.0.0"
  platforms: [codex, antigravity]
  tags:
    - ppt
    - pptx
---

# PPT PPTX

Build reusable Rust-authored source plans that emit real editable `.pptx` decks through the `ppt` CLI. The core promise is simple: `deck.plan.json` stays the source of truth, `deck.pptx` stays PowerPoint-editable, and rendered QA catches layout, font, and visual drift before delivery.

When an existing `.pptx` is the rebuild input, this skill uses the Rust `ppt office ...` inspector for outline, issue, validation, query, and preview helpers before or alongside the rebuild.

Do not claim generic "PPT / PowerPoint / pptx" intake. Let `$slides` absorb those requests first, then use this owner only when the task is explicitly the Rust-authored source-plan lane or a rebuild into that lane.

Quick routing rule:

- Generic PPT request, existing deck, or format still unclear -> `$slides`
- Existing deck needs small direct edits -> `$slides`
- Explicit Rust source-plan / `ppt` CLI rebuild -> `ppt-pptx`
- Existing deck should become a reproducible `deck.plan.json` workflow -> `ppt-pptx`

## When to use

- The user wants a native editable `.pptx` plus a reusable source plan
- The deck will be regenerated or substantially revised later
- Visual polish, overflow detection, font QA, and rendered review are required
- The user provides a YAML/JSON outline and wants automated deck generation
- The user wants to rebuild or redesign an existing deck into a cleaner source-first workflow
- The user explicitly asks for `ppt` CLI, `deck.plan.json`, a reusable PPT source workflow, or a Rust-owned rebuild

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

### Source Contract

Every serious deck workspace should keep these surfaces together:

- `deck.plan.json` -> source of truth for content, layout intent, theme roles, and local assets
- `deck.pptx` -> generated editable PowerPoint deliverable
- `assets/` -> local images and supporting files only; no remote URLs in final plans
- `rendered/` -> PNG evidence for visual review
- `sources.md` -> citations, asset provenance, and review notes
- `ppt.commands.json` -> Rust-generated command cheat sheet from `ppt init`

Never hand-edit generated `.pptx` as the long-term truth in this lane. If the user needs artifact-first edits, route back to `$slides`.

### Design Quality Gates

`ppt-pptx` owns the `ppt` CLI / source plan / editable `.pptx` lane. Borrow the
design skills when the deck needs a stronger visual contract:

- Use `$design-md` when an old deck, screenshots, or brand materials should
  become a reusable `DESIGN.md`.
- Use `$frontend-design` when the deck needs a high-end visual direction before
  authoring.
- Use `$design-workflow-protocol` for multi-round design loops with prompt,
  render evidence, and verdict artifacts.
- Use `$visual-review` for rendered-slide evidence.
- Use `$design-output-auditor` after render to catch visual drift, AI-slop, and
  anti-pattern relapse against `DESIGN.md` or the declared visual contract.

### Text And Design Polishing Chain

Do not rely on the Rust builder alone to make content feel finished. Before
layout, run the right text owner on the outline:

- Use `$humanizer` for ordinary prose naturalization, de-template wording, and
  slide-note cleanup.
- Use `$copywriting` when the deck is a pitch, product story, sales narrative,
  landing-deck, or CTA-heavy presentation.
- Use `$paper-writing` when the deck is a research talk, academic report, or
  manuscript-derived presentation.

Then lock the design lane:

- Source materials / old deck / brand screenshots -> `$design-md` first.
- No source style but high-end visual goal -> `$frontend-design` first.
- Multi-round deck polish -> `$design-workflow-protocol` to keep prompt,
  render evidence, and verdict artifacts connected.

Default polish chain: `outline -> text-owner polish -> DESIGN.md or visual contract -> deck.plan.json -> deck.pptx -> rendered
PNG -> visual-review evidence -> design-output-auditor verdict -> ppt
qa/build-qa sign-off`.

### Template Gallery

- **Dark Luxury** -> default `ppt` template
- **Light Academic** -> `ppt --template light`
- **Corporate** -> `ppt --template corporate`

Pick the template that matches the tone; see `references/design-system.md` for
the full style guidance.

## Workflow

1. Decide lane: source-first rebuild here, artifact-first edits in `$slides`.
2. Bootstrap or reuse a workspace with `deck.plan.json`, `deck.pptx`, `assets/`, `rendered/`, `sources.md`, and `ppt.commands.json`.
3. Convert outline / old deck structure into a slide plan before styling.
4. Send outline text through `$humanizer`, `$copywriting`, or `$paper-writing`
   as appropriate before layout.
5. Lock the visual system through `$design-md` or `$frontend-design`: palette,
   fonts, 2-3 reusable layout families, cover/closing grammar.
6. Build with the Rust `ppt` CLI, then run Rust inspection, overflow/font checks, and rendered review.
7. Fix the source plan, not the generated deck, then rebuild until QA passes.
8. Deliver `.pptx`, `deck.plan.json`, local assets, rendered evidence when useful, `sources.md`, and `ppt.commands.json`.

Default CLI:

- use `ppt init <workdir>` to create a deck workspace from the Rust CLI
- use `ppt outline <outline.yaml|outline.json> --output deck.plan.json --bootstrap --build` to turn an outline into `deck.plan.json` and `deck.pptx`
- use `ppt build-qa --workdir . --entry deck.plan.json --deck deck.pptx --rendered-dir rendered --json` for the default build and QA pass
- use `ppt build-qa --workdir . --entry deck.plan.json --deck deck.pptx --rendered-dir rendered --quality strict --json` when delivery should fail fast on QA issues

Rust inspection boost for rebuild / audit work:

- use `ppt office doctor` for outline + issues + validation in one pass
- use `ppt office get/query` when you need stable `shape[@id=...]` addressing from an old deck
- use `ppt office watch` for local HTML preview of an already-generated `.pptx`

Default Rust QA lane:

- use `ppt build-qa --workdir . --entry deck.plan.json` after source-plan authoring
- use `ppt qa <deck.pptx> --rendered-dir rendered --fail-on-issues` for an existing generated deck when issues should stop delivery
- use `ppt intake <deck.pptx>` before rebuilding an old `.pptx`

For the detailed source-first workflow, rebuild path for existing decks, notes /
transitions guidance, and QA sequence, see [references/workflow.md](./references/workflow.md).

Layering rule:

- `ppt-pptx` owns new deck authoring and big redesigns where the source of truth should become `deck.plan.json`
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
- Lower AIGC signals in generated titles, bullets, and speaker notes: avoid
  "本页展示 / 核心观点如下 / 具有重要意义" style filler; preserve facts, numbers,
  and domain terms while making the copy more concrete.
- Mixed-language tokens stay intact; headings visually balanced.
- Images feel embedded (framed, overlayed), not pasted. Intentional crops only.
- On dark slides, readability is a hard constraint — no gray-on-black body text.
- Run the Rust `ppt slides-test` / `ppt qa` checks for bounds, overflow, fonts,
  and generated-deck health.
- Rendered-slide QA with `$visual-review` before delivery; use
  `$design-output-auditor` when a `DESIGN.md` or visual contract exists.

## Resource Guide

- [references/workflow.md](./references/workflow.md) — end-to-end build / rebuild / QA flow
- [references/design-system.md](./references/design-system.md) — aesthetics, hierarchy, template choice
- [references/layout-patterns.md](./references/layout-patterns.md) — reusable slide compositions
- [references/visualization_patterns.md](./references/visualization_patterns.md) — chart / diagram selection
- [references/rust-cli.md](./references/rust-cli.md) — Rust CLI commands and authoring contract
- [references/install.md](./references/install.md) — setup and dependency fixes
- [references/checklist.md](./references/checklist.md) — full sign-off checklist

## Practical Defaults

- Output: polished `.pptx` plus matching `deck.plan.json`
- Engine: Rust `ppt` CLI, 16:9 wide
- Visual default: black-luxury; use template variants only when tone demands it
- Density default: 2–4 panels or one wide evidence surface per slide
- QA default: render → overflow/font checks → `$visual-review` → sign-off

## Final Checks (summary)

See [references/checklist.md](./references/checklist.md) for the complete sign-off checklist. Key gates:

- Delivered `.pptx` is real and editable; slide count matches plan.
- `deck.plan.json` remains the reproducible source of truth.
- No tiny-text workarounds; no Chinese orphan lines; titles balanced.
- Images intentional; fonts correct; overlap/bounds checks passed.
- Rendered slides reviewed through `$visual-review`.
- Cover/section/closing feel like one deck; no decorative empty space.
- Dark-slide body text has strong contrast for projector readability.
