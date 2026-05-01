# Rust PPT CLI Notes

## When To Read This

Read this file when you need the Rust command surface for `native PPTX lane` authoring,
QA, inspection, or rebuild work.

## Runtime Contract

- The executable path is the Rust `ppt` binary from `rust_tools/pptx_tool_rs`.
- `deck.plan.json` is the source of truth for generated decks.
- `deck.pptx` is written directly as editable OpenXML by Rust.
- Rust `ppt office ...` owns inspection, issue discovery, package validation, query, and preview helpers.
- The skill directory does not carry alternate script templates, helper modules, package manifests, or lockfiles.
- Text and design polishing are skill-orchestrated before Rust build:
  built-in Rust copy naturalization plus `$copywriting` / `$paper-writing` for
  copy, then `$design-md` for visual direction, followed
  by `$visual-review` and `$design-md` verdicts on rendered evidence.

## Authoring Commands

- `ppt init <workdir>` creates `outline.json`, `deck.plan.json`, `assets/`, `rendered/`, `sources.md`, and `ppt.commands.json`.
- `ppt outline <outline.yaml|outline.json> --output deck.plan.json --bootstrap --build` turns an outline into a Rust source plan and editable `.pptx`.
- `ppt build-qa --workdir . --entry deck.plan.json --deck deck.pptx --rendered-dir rendered --json` rebuilds and checks the default deliverable.
- `ppt build-qa --workdir . --entry deck.plan.json --deck deck.pptx --rendered-dir rendered --quality strict --json` fails the command when overflow, font, or inspector checks fail.

The reusable command manifest is generated into each deck workspace as
`ppt.commands.json`; it is data for humans and agents, not a package wrapper or
a second runtime.

## QA Commands

- `ppt extract-structure deck.pptx --output structure.json` inspects slide, shape, text, image, chart, table, and notes structure.
- `ppt slides-test deck.pptx --fail-on-overflow` checks whether shapes leave the original slide canvas.
- `ppt render deck.pptx --output-dir rendered` renders slides to PNG evidence.
- `ppt create-montage --input-dir rendered --output-file montage.png` builds a review sheet for long decks.
- `ppt detect-fonts deck.pptx --json` checks authored and rendered font behavior.
- `ppt qa deck.pptx --rendered-dir rendered --json` runs the combined Rust QA path.
- `ppt qa deck.pptx --rendered-dir rendered --fail-on-issues --json` turns the combined QA result into a hard gate.

## Rust Office Inspection

Use `ppt office ...` when an existing `.pptx` needs Rust-native inspection,
stable shape paths, package validation, or preview support:

- `ppt office doctor deck.pptx --json`
- `ppt office get deck.pptx '/slide[1]' --depth 2 --json`
- `ppt office query deck.pptx 'shape[font=Arial]' --json`
- `ppt office watch deck.pptx --browser`

The office inspector is a helper lane; it does not replace `deck.plan.json` as the
source of truth for generated decks.

## Practical Rules

- Keep palette, typography, spacing, and panel styles named by design role.
- Default to cross-platform-safe fonts: `Arial` for general text and `Courier New` for code.
- Naturalize copy before shrinking type or splitting slides.
- Rebuild from source, run Rust inspector checks, render evidence, then audit the PNGs.
