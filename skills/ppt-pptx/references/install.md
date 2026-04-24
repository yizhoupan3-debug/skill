# PPTX Install

Use this reference when setting up a fresh `.pptx` deck workspace or when the Rust `ppt` CLI cannot render or inspect a deck.

## Rust CLI

The ppt-pptx runtime path is the Rust `ppt` binary from this repository:

```bash
cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/pptx_tool_rs/Cargo.toml --bin ppt -- --help
```

For faster repeated use, build the Rust tools and put the resulting binary on `PATH` as `ppt`.

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

## Optional OfficeCLI Install

For deeper `.pptx` inspection, HTML preview, path-based querying, and structured issue / schema checks, this skill can also use local `officecli`.

Quick check:

```bash
officecli --version
officecli pptx --help
```

What it adds to `ppt-pptx`:

- `view outline` for quick deck shape / text-box counts
- `view issues` for overflow / missing-title / structure diagnostics
- `validate` for OpenXML schema checks
- `get` / `query` for stable-path inspection of existing decks
- `watch` for live HTML preview when iterating on an already-generated `.pptx`

## Recommended Workspace Bootstrap

```bash
ppt init .
ppt outline outline.yaml --output deck.plan.json --bootstrap --build
```

The expected workspace outputs are:

- `deck.plan.json`
- `deck.pptx`
- `assets/`
- `rendered/`
- `sources.md`

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
- confirm the built binary directory is on `PATH` when using package scripts

### Render commands fail even though `deck.pptx` builds

- check for missing `soffice` / LibreOffice and Poppler
- deck generation and deck rendering are separate dependency layers

### Font checks report substitutions

- confirm `fc-list` is available
- keep authored defaults to `Arial` and `Courier New` unless the project requires a specific installed font

### OfficeCLI-backed diagnostics are missing

- check `officecli --version`
- if unavailable, the deck can still build and render; only the deeper OfficeCLI audit / watch path is unavailable
- `ppt office probe --json` is the quickest local check

Useful OfficeCLI audit commands after generation:

```bash
ppt office doctor deck.pptx --json
ppt office outline deck.pptx --json
ppt office watch deck.pptx --port 18080
```

Recommended mixed-lane commands:

```bash
ppt build-qa --workdir . --entry deck.plan.json --deck deck.pptx --rendered-dir rendered --json
ppt qa deck.pptx --rendered-dir rendered --json
ppt intake old_deck.pptx --json
```
