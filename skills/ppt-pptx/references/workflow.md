# PPTX Workflow

Use this reference when building a PowerPoint deck from scratch, refactoring an HTML-first habit into a `.pptx` workflow, or checking whether a deck is both editable and presentation-grade.

## Lane Decision

Use this quick split before touching files:

- Generic PPT ask or existing deck artifact with workflow still unclear -> start at `slides`
- New deck from outline / notes / YAML / structured content where `deck.plan.json` should be the source of truth -> use `ppt-pptx`
- Existing deck needs substantial redesign, new visual system, or source-of-truth rebuild -> use `ppt-pptx`
- Existing `.pptx` needs in-place text edits, table/chart tweaks, inspection, or batch patches while the file stays the source of truth -> use `slides`

Practical rule:

- If the desired source of truth is a Rust-authored `deck.plan.json`, stay in `ppt-pptx`
- If the desired source of truth remains the `.pptx` file itself, start with `slides`

Fast examples:

- "按这份提纲出一版董事会汇报" -> `ppt-pptx`
- "把这份旧 deck 全部重做成统一视觉" -> `ppt-pptx`
- "把第 3 页标题和图表数字改掉" -> `slides`
- "检查这个现成 PPT 有没有溢出和布局问题" -> `slides`

## Project Shape

Create or reuse:

```text
project/
├── deck.plan.json
├── deck.pptx
├── assets/
├── rendered/
├── sources.md
└── ppt.commands.json
```

Fast bootstrap:

```bash
ppt init .
```

Notes:

- `assets/` can start empty; the Rust `ppt` templates fall back to placeholder panels when sample images are missing.
- Add real local images later for final polish rather than blocking the first successful build.

## Engine Choice

- Prefer the Rust `ppt` CLI for deck generation.
- Do not introduce a parallel authoring engine for this lane.
- Keep editable output driven by `deck.plan.json` so layout logic stays visible and reproducible.

## Existing Decks

When the input is an existing `.pptx`, choose one of two modes:

1. In-place edit mode
   - Best for copy fixes, slide reordering, shape/table/chart property changes, and repeated patch operations.
   - Start with `slides`, because the file itself stays the editable source of truth.
2. Rebuild mode
   - Best for major redesign, consistent visual system, repeated generation, or a deck that should become reproducible from code.
   - Extract structure/assets if helpful, then rebuild through the Rust `ppt` CLI and keep `deck.plan.json` as the source of truth.
   - Use the Rust `ppt office doctor|get|query` inspector first when you need stable IDs, shape paths, or a quick structural map from the old deck.

Use the rebuild path when the current deck is just raw material and the real goal is a cleaner long-term authoring workflow.

## Visual System First

Before filling the slides, lock these decisions:

- dominant palette
- body and display font system
- cover structure
- footer / source-note treatment
- section-opener treatment
- one primary card style and one emphasis style

Beauty comes from repeated visual logic, not from piling effects onto isolated slides.

## Design Skill Loop

When the deck starts from old materials, brand examples, or screenshots, produce
or reuse `DESIGN.md` before authoring. Keep the chain explicit:

```text
outline / notes / old deck structure
-> text-owner polish
-> DESIGN.md / visual contract
-> deck.plan.json
-> deck.pptx
-> rendered PNG evidence
-> visual-review notes
-> design-md verdict
-> ppt qa / build-qa sign-off
```

Use `$design-md` for extracting a deck design system or defining a new premium
direction, and `$visual-review` plus `$design-md` for repeatable
multi-round artifact tracking plus final drift / AI-slop / anti-pattern checks.

## Text Skill Loop

The text pass happens before layout. Use built-in Rust copy naturalization for
ordinary prose, pick a specialist only when the deck needs one, then encode the
improved copy back into `deck.plan.json`:

- `$copywriting`: pitch decks, product decks, sales decks, fundraising decks,
  landing narratives, taglines, CTAs, and persuasive section titles.
- `$paper-writing`: academic talks, research reports, manuscript-to-slide
  conversion, methods/results framing, and citation-sensitive wording.

Use the text owner to make titles shorter, bullets concrete, and speaker notes
less generic. Then use the Rust naturalization pass as a safety net, not as the
only writing step.

## Copy Naturalization First

Run a light text pass before layout, especially for outline-generated decks:

- Replace meta narration with direct claims.
- Keep one clear judgment per slide; make bullets concrete enough to stand on
  their own.
- Vary sentence length and bullet openings; do not let every line follow the
  same "verb + noun + result" shape.
- Remove filler such as "本页展示", "核心观点如下", "具有重要意义", "赋能", and
  "显著提升" unless a concrete number or mechanism follows.
- Preserve facts, citations, numbers, names, and technical terms; do not invent
  anecdotes just to sound human.

## Beauty Rules

- Use contrast intentionally: a strong title, a calmer subtitle, and lower-energy metadata.
- Limit the palette. One dominant family plus one accent is usually enough.
- Use fewer but larger elements. Do not solve uncertainty with more cards.
- Let one element lead on each slide: hero image, big number, claim title, or key comparison.
- If everything has a border, shadow, accent, and label, nothing is important.
- Avoid equal-height box farms unless the content is truly symmetric.
- Vary slide structure across the deck, but keep the underlying visual grammar stable.

## Authoring Rules

- Set theme fonts explicitly before measuring or sizing text.
- Encode image crop/contain intent and text-box sizing in `deck.plan.json` instead of hand-tuning generated output.
- Keep growing content boxes top-aligned in the source plan.
- Prefer editable chart/table structures for simple visuals, then restyle them to match the deck palette and hierarchy.
- Use local vector or raster assets for diagrams when they improve editability or render fidelity.
- Reserve a bottom safe area before placing the lowest chart, caption, or text block.
- Rewrite copy before shrinking type. If a title wraps badly, shorten it or widen the box.

## Typography Rules

- Body text should usually stay comfortably readable at normal presentation zoom.
- Do not accept one-or-two-character Chinese widows.
- Do not accept tiny trailing title lines.
- Avoid awkward breaks in mixed-language strings, units, and bracketed citations.
- If a slide carries explanatory prose, convert some of it into structure: metric strip, takeaway band, side annotation, or comparison panel.

## Image Rules

- Pick a focal point before placing the image.
- Use crop when the subject is clear and can fill the frame.
- Use contain when the full diagram or map must remain visible.
- Do not stretch images.
- If a background image weakens contrast, add an overlay or switch to a cleaner composition.

## QA Loop

1. Generate the `.pptx`.
2. Run `ppt office doctor` for Rust outline, issue, and package validation.
3. Run `ppt slides-test` when the slide is dense or edge-tight.
4. Render the deck to PNGs with `ppt render`.
5. Build a montage with `ppt create-montage` when the deck is long.
6. Run `ppt detect-fonts` when typography is part of the design.
7. Call `$visual-review` on suspicious slides or the montage.
8. Fix the source plan and repeat.

If a `DESIGN.md` or visual contract exists, add one more pass after rendered
review: use `$design-md` verdict language (`match / minor drift / material drift /
hard fail`), then fix only the smallest set needed to restore fidelity.

If the deck is using fallback placeholder panels because images are missing, treat the build as structurally valid but not visually final.

## Rust Inspector Boost

Use the Rust inspector lane when the deck already exists and you need stronger inspection than rendered screenshots alone.

Recommended commands:

```bash
ppt office doctor deck.pptx --json
ppt office get deck.pptx '/slide[1]' --depth 2 --json
ppt office query deck.pptx 'shape[font=Arial]' --json
ppt office watch deck.pptx --browser
```

Use this lane for:

- fast outline / issue / validation scans on a generated deck
- stable `shape[@id=N]` addressing before a rebuild
- watching an already-generated `.pptx` in HTML while iterating
- read-only batch plan checks against an existing artifact before fully rebuilding the source

Do not let the inspector replace `deck.plan.json` as the source of truth for new-code-authored decks. It is the inspection / preview boost, not the authoring core.

## Rust Default

The strongest default in this skill is now one Rust-owned lane:

- `deck.plan.json` remains the authoring source of truth
- Rust tools handle render / structure / image-side QA
- Rust `ppt office ...` handles issue discovery, package validation, stable path lookup, and preview

Recommended commands:

```bash
ppt build-qa --workdir . --entry deck.plan.json --deck deck.pptx --rendered-dir rendered --json
ppt build-qa --workdir . --entry deck.plan.json --deck deck.pptx --rendered-dir rendered --quality strict --json
ppt qa deck.pptx --rendered-dir rendered --fail-on-issues --json
ppt intake old_deck.pptx --json
```

Use `build-qa` for code-authored decks and `intake` for existing-deck rebuilds.

For a full regression pass from the skill root, run:

```bash
cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/pptx_tool_rs/Cargo.toml --bin ppt -- --help
```

## What To Look For In Rendered Slides

- awkward crops
- font substitution
- tiny text hidden inside a large card
- equal-weight grids that flatten hierarchy
- cover slides that look like default office templates
- inconsistent page furniture across slides
- titles with a weak trailing line
- lines that leave only one or two Chinese characters
- empty blocks that exist only for decoration
- tables and charts that still look like untouched defaults

## Delivery

Deliver:

- `deck.pptx`
- `deck.plan.json`
- `assets/`
- `sources.md`
- `ppt.commands.json`
- rendered PNGs or montage when visual QA mattered

Do not deliver only screenshots or a PDF if the user asked for a PPT/PPTX deck.
