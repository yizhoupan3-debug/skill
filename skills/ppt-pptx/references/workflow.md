# PPTX Workflow

Use this reference when building a PowerPoint deck from scratch, refactoring an HTML-first habit into a `.pptx` workflow, or checking whether a deck is both editable and presentation-grade.

## Lane Decision

Use this quick split before touching files:

- Generic PPT ask or existing deck artifact with workflow still unclear -> start at `slides`
- New deck from outline / notes / YAML / structured content where `deck.js` should be the source of truth -> use `ppt-pptx`
- Existing deck needs substantial redesign, new visual system, or source-of-truth rebuild -> use `ppt-pptx`
- Existing `.pptx` needs in-place text edits, table/chart tweaks, inspection, or batch patches while the file stays the source of truth -> use `slides`

Practical rule:

- If the desired source of truth is `deck.js`, stay in `ppt-pptx`
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
├── deck.js
├── deck.pptx
├── assets/
├── rendered/
└── sources.md
```

Notes:

- `assets/` can start empty; the bundled templates and `outline_to_deck.js` now fall back to placeholder panels when sample images are missing.
- Add real local images later for final polish rather than blocking the first successful build.

Optional:

```text
project/
└── scripts/
    ├── render_slides.py
    ├── slides_test.py
    ├── create_montage.py
    ├── detect_font.py
    └── ensure_raster_image.py
```

## Engine Choice

- Prefer PptxGenJS for deck generation.
- Do not use `python-pptx` for full authoring unless the task is inspection-oriented or very small.
- Keep editable output in JavaScript so layout logic stays visible and reproducible.

## Existing Decks

When the input is an existing `.pptx`, choose one of two modes:

1. In-place edit mode
   - Best for copy fixes, slide reordering, shape/table/chart property changes, and repeated patch operations.
   - Start with `slides`, because the file itself stays the editable source of truth.
2. Rebuild mode
   - Best for major redesign, consistent visual system, repeated generation, or a deck that should become reproducible from code.
   - Extract structure/assets if helpful, then rebuild in PptxGenJS and keep `deck.js` as the source of truth.

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
- Use helper functions for image crop/contain and text-box sizing instead of hand-tuning every box.
- Use `valign: "top"` for growing content boxes.
- Prefer native charts for simple visuals, then restyle them to match the deck palette and hierarchy.
- Use SVG for diagrams when possible.
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
2. Run `slides_test.py` when the slide is dense or edge-tight.
3. Render the deck to PNGs with `render_slides.py`.
4. Build a montage with `create_montage.py` when the deck is long.
5. Run `detect_font.py` when typography is part of the design.
6. Call `$visual-review` on suspicious slides or the montage.
7. Fix the source `.js` and repeat.

If the deck is using fallback placeholder panels because images are missing, treat the build as structurally valid but not visually final.

For a full regression pass from the skill root, run:

```bash
python3 scripts/smoke_test.py
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
- `deck.js`
- `assets/`
- `sources.md`

Do not deliver only screenshots or a PDF if the user asked for a PPT/PPTX deck.
