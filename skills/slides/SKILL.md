---
name: slides
description: Route presentation, PPT, PPTX, and slide deck tasks.
routing_layer: L3
routing_owner: gate
routing_gate: artifact
routing_priority: P1
session_start: required
trigger_hints:
  - PPT
  - pptx
  - 做个PPT
  - 生成演示文稿
  - slides
  - PowerPoint
  - presentation deck
  - presentation
  - artifact tool
metadata:
  version: "2.1.0"
  platforms: [codex]
  tags: [powerpoint, ppt, pptx, slides, presentation, rust-ppt, proxy]
risk: medium
source: local
allowed_tools:
  - shell
  - cargo
approval_required_tools:
  - file overwrite
filesystem_scope:
  - repo
  - workspace
  - temp
  - artifacts
network_access: conditional
artifact_outputs:
  - final_deck.pptx
  - deck.plan.json
  - rendered/*.png
  - montage.png
  - EVIDENCE_INDEX.json

---

# slides

This file is the root-level entry point required by the repository policy:
after `skills/SKILL_ROUTING_RUNTIME.json` routes to `slides`, the next file to
open is `skills/slides/SKILL.md`.

This skill owns the presentation entry gate for artifact-first slide work. It
must convert a generic PPT request into an executable lane without blocking on
questions that can be answered by reasonable assumptions.

## When to use

- The primary artifact is a slide deck, presentation, or `.pptx` file.
- The user says "做个PPT", "生成演示文稿", or otherwise asks for a presentation before the source format is chosen.
- The user wants to create, revise, render-check, or export editable slides.
- The task needs deck structure, slide-level layout QA, and final PowerPoint fidelity.
- The request mentions PowerPoint, PPT, PPTX, slide deck, or presentation output.

## Do not use

- The user explicitly wants Markdown, Slidev, Marp, or HTML/CSS source slides; reroute to `source-slide-formats` only after it is selected by the router or fallback manifest.
- The user explicitly wants LaTeX Beamer source plus PDF; reroute to `ppt-beamer` only after it is selected by the router or fallback manifest.
- The request is for a PDF/document rather than a presentation artifact.
- The task is only about static screenshots or image collage output.

## Core Rules

- Treat this as the canonical first check for generic PPT / presentation asks before any narrower owner claims the request.
- Do not stop to ask for goal, audience, visual bar, or format when a safe default exists. Assume a broad professional audience, presentation-grade visual quality, editable `.pptx`, and the current/requested output directory; state those assumptions while proceeding.
- Ask the user only when the next action would overwrite an existing deliverable, use private/paid/network assets, or choose between materially different source formats.
- If the user explicitly names Markdown / Slidev / Marp, HTML slide export, or Beamer source, do not invent the handoff. Re-run routing or consult the fallback manifest for that exact owner, then read only that owner `SKILL.md`.
- Use the Rust `ppt` CLI from `rust_tools/pptx_tool_rs` for editable `.pptx` generation, inspection, render checks, and strict QA unless the task is an in-place edit of an existing `.pptx`.
- Run builders from a writable workspace, temp, or artifact directory; never from managed dependency directories. Put final files in the requested output directory and keep scratch files under the task workspace or `artifacts/scratch`.
- Keep generated-deck authoring source-first and deterministic: update `deck.plan.json`, rebuild, then verify rather than hand-editing generated output.
- Keep support artifacts private unless the user explicitly asks for them.

## Required workflow

1. Intake quickly: extract goal, audience, source material, requested output path, and format. If any item is missing but safely assumable, proceed with the defaults in Core Rules instead of asking.
2. If the deck needs a reusable visual identity, brand consistency, high-design
   theme, or acceptance against a style contract, route through `$design-md`
   before authoring slides.
3. If the user explicitly wants Markdown / HTML source, reroute through `source-slide-formats` only after router/fallback-manifest selection; if they want Beamer, do the same for `ppt-beamer`.
4. Otherwise choose the native `.pptx` lane:
   - New or rebuilt deck: use this skill's native PPTX lane and `deck.plan.json` as source of truth.
   - Existing deck with small in-place edits: keep the `.pptx` as source of truth, inspect first, then patch the smallest safe surface.
5. Build slides quickly with editable objects first: text, shapes, tables, and native charts.
6. Render previews and run the verification gates below.
7. Export the final `.pptx` into the requested output directory and update evidence.
8. Finalize once the deck is correct, legible, exported, and the final response reports the evidence used.

## Executable Native PPTX Lane

Use these commands from the deck workspace. If `ppt` is not already on `PATH`,
run it through Cargo:

```bash
cargo run --manifest-path rust_tools/pptx_tool_rs/Cargo.toml --bin ppt -- <command>
```

New deck or rebuild:

```bash
ppt init .
ppt outline outline.json --output deck.plan.json --bootstrap --build --json
ppt build-qa --workdir . --entry deck.plan.json --deck deck.pptx --rendered-dir rendered --quality strict --json
```

Existing deck intake:

```bash
ppt intake input.pptx --json
ppt office doctor input.pptx --json
ppt render input.pptx --output-dir rendered
```

Verification:

```bash
ppt office doctor deck.pptx --json
ppt slides-test deck.pptx --fail-on-overflow
ppt render deck.pptx --output-dir rendered
ppt detect-fonts deck.pptx --json
ppt qa deck.pptx --rendered-dir rendered --fail-on-issues --json
```

For decks longer than 8 slides, add:

```bash
ppt create-montage --input-dir rendered --output-file montage.png --label-mode number
```

## Design skill handoff

Use `$design-md` before slide authoring when the user asks for:

- a branded PPT, visual system, theme, or "统一设计规范"
- reusable deck styling across many slides
- theme colors, typography, chart palette, callout boxes, icon style, or title/section slide grammar
- "less AI", "高级感", "像一个完整产品发布会", or acceptance against an existing `DESIGN.md`

Do not route through `$design-md` for quick utility decks where speed matters
more than a reusable visual contract.

## Native PPTX References

The old standalone native PPTX entry has been folded into this skill. Keep
native PPTX implementation details in references so the user-facing route stays
`$slides`:

- [references/native-pptx/workflow.md](references/native-pptx/workflow.md)
- [references/native-pptx/checklist.md](references/native-pptx/checklist.md)
- [references/native-pptx/design-system.md](references/native-pptx/design-system.md)
- [references/native-pptx/layout-patterns.md](references/native-pptx/layout-patterns.md)
- [references/native-pptx/rust-cli.md](references/native-pptx/rust-cli.md)

## Existing PPTX Safety

- Before modifying an existing `.pptx`, run `ppt intake` and `ppt office doctor`
  to map slide count, shapes, notes, charts, tables, media, and validation
  issues.
- Preserve the original file by writing a new output path unless the user
  explicitly asked for overwrite.
- For minor edits, keep masters, layouts, notes, animations, images, charts, and
  unknown OOXML parts in place; only patch the targeted text/table/chart values.
- For major redesigns, treat the old deck as source material, extract structure
  and assets, then rebuild through `deck.plan.json`.
- If a requested edit depends on unsupported animation, embedded media, macros,
  or proprietary effects, report that limitation and preserve those parts rather
  than flattening them into screenshots.

## Verification Standard

- Render every slide for decks up to 12 slides. For longer decks, render every
  slide and review a montage plus all cover, section, dense chart/table, and
  changed slides.
- Layout passes only when `ppt slides-test --fail-on-overflow` reports no
  out-of-bounds content and rendered previews show no obvious overlap, clipping,
  unreadable text, or broken image crop.
- Editability passes when important text is native text and simple charts,
  tables, and shapes remain editable unless the user accepted raster assets.
- Chart sanity passes when axis labels, legends, units, data labels, and visual
  emphasis match the slide claim and do not use unstyled default Office colors.
- Font sanity passes when `ppt detect-fonts --json` has no missing or substituted
  important fonts, or the fallback is explicitly accepted.

## Evidence Index

When this skill creates or materially changes a deck, write or update
`EVIDENCE_INDEX.json` in the deck workspace or artifact directory. Keep it
compact:

```json
{
  "artifacts": [
    {"path": "deck.pptx", "kind": "pptx", "status": "final"},
    {"path": "deck.plan.json", "kind": "source", "status": "source-of-truth"},
    {"path": "rendered", "kind": "rendered-preview", "status": "checked"},
    {"path": "montage.png", "kind": "review-image", "status": "checked"}
  ],
  "checks": [
    {"command": "ppt qa deck.pptx --rendered-dir rendered --fail-on-issues --json", "status": "passed"}
  ]
}
```

## Hard constraints

- Do not use legacy JS/Python deck generators or screenshot-only slide authoring as the default path here.
- Do not mention internal builder/package details in the final response unless the user asks.
- Do not surface scratch previews, verification images, or support files unless requested.
- Do not search or preload the rest of `skills/`.

## Completion criteria

- Final `.pptx` exported successfully.
- Important slide text remains editable and readable.
- Charts, tables, and key layout blocks render correctly in preview.
- `EVIDENCE_INDEX.json` records final artifacts and checks when a deck was created or materially changed.
- Final response stays concise but includes the `.pptx` link and the verification evidence used, or an explicit blocker if verification could not run.
