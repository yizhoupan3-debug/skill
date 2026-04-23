# PPTX Install

Use this reference when setting up a fresh `.pptx` deck workspace or when the template fails because dependencies are missing.

## Official Base Install

Per the official PptxGenJS installation docs, the Node install is:

```bash
npm install pptxgenjs
```

Official references:

- [PptxGenJS installation docs](https://gitbrent.github.io/PptxGenJS/docs/installation/)
- [PptxGenJS npm package](https://www.npmjs.com/package/pptxgenjs)

## Practical Install For This Skill

This skill's template and helper bundle use more than `pptxgenjs` alone. For the current helper set, install:

```bash
npm install pptxgenjs skia-canvas linebreak fontkit prismjs mathjax-full js-yaml
```

Smoke-tested locally on March 18, 2026 with:

- `pptxgenjs@4.0.1`
- `skia-canvas@3.0.8`
- `linebreak@1.1.0`
- `fontkit@2.0.4`
- `prismjs@1.30.0`
- `mathjax-full@3.2.1`
- `js-yaml@4.1.1`

## System Tools For QA

Some QA scripts rely on system binaries:

- `soffice` / LibreOffice for PPTX to PDF conversion used by rendering workflows
- Poppler tools for PDF raster support used by `pdf2image`
- `fc-list` for font inspection

If these are missing, deck generation can still work, but rendered QA may not.

Recommended macOS install commands:

```bash
brew install --cask libreoffice
brew install poppler
```

## Optional OfficeCLI Install

For deeper `.pptx` inspection, HTML preview, path-based querying, and structured issue / schema checks, this skill can also use local `officecli`.

On this machine, `officecli --version` resolved successfully and reported `1.0.53`.

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

## Python QA Dependencies

The bundled QA scripts also expect Python packages that are separate from the Node deck toolchain:

```bash
python3 -m pip install --user pdf2image python-pptx
```

If `Pillow` or `numpy` are missing in the active Python environment, install them too:

```bash
python3 -m pip install --user pillow numpy
```

## Recommended Workspace Bootstrap

```bash
npm init -y
npm install pptxgenjs skia-canvas linebreak fontkit prismjs mathjax-full js-yaml
mkdir -p assets rendered scripts
```

Optional smoke test from the skill root:

```bash
node scripts/smoke_test.js
```

Then copy into the workspace:

- `assets/deck.template.js` as `deck.js`
- `assets/pptxgenjs_helpers/`
- `scripts/pptx_tool.js`
- optional OfficeCLI-backed inspection helpers: call via `node scripts/pptx_tool.js office ...` when needed

Optional images:

- The deck can now build without bundled sample images.
- If files such as `./assets/cover.jpg` or `./assets/placeholder.jpg` are missing, the templates fall back to neutral placeholder panels instead of crashing.
- Add real local images later when visual polish matters.

Cross-platform font default:

- The skill defaults to `Arial` for headings/body and `Courier New` for code so the authored font choice remains valid on both macOS and Windows.
- Avoid swapping templates back to `Helvetica Neue`, `Calibri`, or `Consolas` unless the deck is intentionally platform-specific.

## Failure Patterns

### `Cannot find module 'pptxgenjs'`

- install `pptxgenjs`
- confirm you are running `node deck.js` from the workspace that contains `node_modules`

### `Cannot find module 'skia-canvas'` or `linebreak` or `fontkit`

- install the helper dependencies above
- this skill's helper bundle imports measurement utilities eagerly, so partial installs are not enough

### `mathjax-full is not installed`

- install `mathjax-full`
- or avoid helper calls that require LaTeX rendering

### `js-yaml is required for YAML input`

- install `js-yaml`
- or use JSON input with `outline_to_deck.js`

### render scripts fail even though `deck.pptx` builds

- check for missing `soffice` / LibreOffice and Poppler
- deck generation and deck rendering are separate dependency layers

### OfficeCLI-backed diagnostics are missing

- check `officecli --version`
- if unavailable, the deck can still build and render; only the deeper OfficeCLI audit / watch path is unavailable
- `node scripts/pptx_tool.js office probe --json` is the quickest local check

## Smoke-Test Result

Verified locally on this machine on March 18, 2026:

- `brew install --cask libreoffice` succeeded
- `soffice --version` reports `LibreOffice 26.2.1.2`
- `python3 -m pip install --user pdf2image python-pptx` succeeded
- the template generated a real `.pptx`
- `node scripts/pptx_tool.js render` successfully rendered that `.pptx` into slide PNGs

Useful OfficeCLI audit commands after generation:

```bash
node scripts/pptx_tool.js office doctor deck.pptx --json
node scripts/pptx_tool.js office outline deck.pptx --json
node scripts/pptx_tool.js office watch deck.pptx --port 18080
```

Recommended mixed-lane commands:

```bash
node scripts/pptx_tool.js build-qa --workdir . --entry deck.js --deck deck.pptx --rendered-dir rendered --json
node scripts/pptx_tool.js qa deck.pptx --rendered-dir rendered --json
node scripts/pptx_tool.js intake old_deck.pptx --json
```
