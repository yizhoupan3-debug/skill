# Rust PPT CLI Notes

## When To Read This

Read this file when you need the Rust command surface for `ppt-pptx` authoring,
QA, inspection, or rebuild work.

## Runtime Contract

- The executable path is the Rust `ppt` binary from `rust_tools/pptx_tool_rs`.
- `deck.plan.json` is the source of truth for generated decks.
- `deck.pptx` is written directly as editable OpenXML by Rust.
- The skill directory does not carry JavaScript templates, helper modules, or lockfiles.

## Authoring Commands

- `ppt init <workdir>` creates `outline.json`, `deck.plan.json`, `assets/`, and `rendered/`.
- `ppt outline <outline.yaml|outline.json> --output deck.plan.json --bootstrap --build` turns an outline into a Rust source plan and editable `.pptx`.
- `ppt build-qa --workdir . --entry deck.plan.json --deck deck.pptx --rendered-dir rendered --json` rebuilds and checks the default deliverable.

## QA Commands

- `ppt extract-structure deck.pptx --output structure.json` inspects slide, shape, text, image, chart, table, and notes structure.
- `ppt slides-test deck.pptx --fail-on-overflow` checks whether shapes leave the original slide canvas.
- `ppt render deck.pptx --output-dir rendered` renders slides to PNG evidence.
- `ppt create-montage --input-dir rendered --output-file montage.png` builds a review sheet for long decks.
- `ppt detect-fonts deck.pptx --json` checks authored and rendered font behavior.
- `ppt qa deck.pptx --rendered-dir rendered --json` runs the combined Rust QA path.

## Optional Office Inspection

Use `ppt office ...` only when an existing `.pptx` needs deeper inspection,
stable shape paths, schema validation, or preview support:

- `ppt office doctor deck.pptx --json`
- `ppt office get deck.pptx '/slide[1]' --depth 2 --json`
- `ppt office query deck.pptx 'shape[font=Arial]' --json`
- `ppt office watch deck.pptx --port 18080`

Office inspection is a helper lane; it does not replace `deck.plan.json` as the
source of truth for generated decks.

## Practical Rules

- Keep palette, typography, spacing, and panel styles named by design role.
- Default to cross-platform-safe fonts: `Arial` for general text and `Courier New` for code.
- Naturalize copy before shrinking type or splitting slides.
- Rebuild from source, render evidence, then audit the PNGs.
