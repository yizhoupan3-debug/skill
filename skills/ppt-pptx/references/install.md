# PPTX Install

Use this reference when setting up a fresh `.pptx` deck workspace or when the Rust `ppt` CLI cannot render or inspect a deck.

## Rust CLI

The ppt-pptx runtime path is the Rust `ppt` binary from this repository:

```bash
cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/pptx_tool_rs/Cargo.toml --bin ppt -- --help
```

For faster repeated use, build the Rust tools and put the resulting binary on `PATH` as `ppt`.

There is no skill-local package install step. The only runtime entry is the Rust
CLI; `ppt init` writes `ppt.commands.json` into each deck workspace as a command
cheat sheet, not a package manifest.

## System Tools For QA

Rendered QA relies on system binaries:

- `soffice` / LibreOffice for PPTX to PDF conversion used by rendering workflows
- Poppler tools for PDF size and raster support used by Rust render paths
- `fc-list` for font inspection

If these are missing, source-plan generation can still work, but rendered QA may not.

Recommended macOS install commands:

```bash
brew install --cask libreoffice
brew install poppler
```

## Rust Office Inspector

For deeper `.pptx` inspection, HTML preview, path-based querying, and structured issue / package checks, use the Rust `ppt office ...` commands. No separate inspector install is required.

Quick check:

```bash
ppt office probe --json
```

What it adds to `ppt-pptx`:

- `view outline` for quick deck shape / text-box counts
- `view issues` for overflow / missing-title / structure diagnostics
- `validate` for core OpenXML package checks
- `get` / `query` for stable-path inspection of existing decks
- `watch` for local HTML preview when iterating on an already-generated `.pptx`

## Recommended Workspace Bootstrap

```bash
ppt init .
ppt outline outline.yaml --output deck.plan.json --bootstrap --build
```

Before the final build, polish the outline text with `$humanizer`,
`$copywriting`, or `$paper-writing` as appropriate, then lock the deck's visual
contract with `$design-md` or `$frontend-design`. The Rust CLI builds and checks
the deck; these companion skills make the text and design intentional.

The expected workspace outputs are:

- `deck.plan.json`
- `deck.pptx`
- `assets/`
- `rendered/`
- `sources.md`
- `ppt.commands.json`

Optional images:

- The deck can build without bundled sample images.
- If files such as `./assets/cover.jpg` or `./assets/placeholder.jpg` are missing, the templates fall back to neutral placeholder panels instead of crashing.
- Add real local images later when visual polish matters.

Cross-platform font default:

- The skill defaults to `Arial` for headings/body and `Courier New` for code so the authored font choice remains valid on both macOS and Windows.
- Avoid swapping templates back to `Helvetica Neue`, `Calibri`, or `Consolas` unless the deck is intentionally platform-specific.

## Failure Patterns

### `ppt` is not found

- build the Rust tool or invoke it through `cargo run --manifest-path ... --bin ppt --`
- confirm the built binary directory is on `PATH` when using optional local scripts

### Render commands fail even though `deck.pptx` builds

- check for missing `soffice` / LibreOffice and Poppler
- deck generation and deck rendering are separate dependency layers

### Font checks report substitutions

- confirm `fc-list` is available
- keep authored defaults to `Arial` and `Courier New` unless the project requires a specific installed font

### Rust inspector diagnostics fail

- run `ppt office probe --json`
- run `ppt extract-structure deck.pptx --output structure.json` to isolate malformed package structure
- rebuild from `deck.plan.json` if the package is missing required OpenXML parts

### A package install step appears in an old workspace

- remove the package wrapper from that deck workspace
- invoke `ppt ...` directly or through the Rust command manifest
- do not add a package manifest back to `skills/ppt-pptx`

Useful Rust inspector commands after generation:

```bash
ppt office doctor deck.pptx --json
ppt office outline deck.pptx --json
ppt office watch deck.pptx --browser
```

Recommended Rust lane commands:

```bash
ppt build-qa --workdir . --entry deck.plan.json --deck deck.pptx --rendered-dir rendered --json
ppt build-qa --workdir . --entry deck.plan.json --deck deck.pptx --rendered-dir rendered --quality strict --json
ppt qa deck.pptx --rendered-dir rendered --fail-on-issues --json
ppt intake old_deck.pptx --json
```
