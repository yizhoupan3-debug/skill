# PPT HTML/PDF Workflow

## Use this reference when

- starting a new presentation project
- turning a markdown outline into slides
- checking whether HTML -> PDF export will stay visually identical

## Project shape

Create or reuse:

```text
project/
├── presentation.html
├── print_pdf.js
├── assets/
└── sources.md
```

Install:

```bash
npm init -y
npm install puppeteer
```

## Asset workflow

1. Map each slide to its image need before writing the final HTML.
2. Prefer:
   - user-provided photos
   - official or institutional sources
   - maps / satellite imagery
   - major news or public reference images
3. Download chosen images into local `assets/`.
4. Rename consistently:
   - `slide1_cover.jpg`
   - `slide3_siteplan.png`
   - `slide5_heatmap.jpg`
5. Record source URL, organization / author, and access date in `sources.md`.

## Layout rules

- Fixed slide size: `1920x1080`
- Absolute pixel sizing, not responsive scaling
- One `.slide` per page
- Use local images only in final HTML
- Prefer content-sized rows for content-heavy slides; avoid `grid-auto-rows: 1fr` unless every card is intentionally equal height
- Keep content density controlled:
  - 4-8 cards per slide
  - 3-5 list items per list
  - no long paragraphs when a stat card, comparison strip, or split layout will do
  - if a slide is visually sparse, add another evidence layer before accepting the empty space
- Typography floor on `1920x1080`:
  - dense body copy: usually `18-20px`
  - normal body copy: usually `20px+`
  - small labels / notes: usually `14-16px`
- Density target:
  - most content slides should have 2-4 clear information zones
  - avoid leaving more than roughly 25% of the slide as purposeless empty space
- Prefer concise labels over explanatory blocks
- Put citations in a bottom-left citation bar and page numbers bottom-right

## HTML rules

- Use one screen layout only
- `@media print` is allowed only for page size, page breaks, and color preservation; do not create a second layout
- Do not use `transform: scale(...)` on the whole slide
- Avoid dependencies that can reflow unexpectedly during export
- If online fonts are used, wait for them before export
- If font loading is unstable, switch to local or system fonts and re-check layout
- If a box becomes mostly empty after content placement, merge it, shrink it, or rewrite the slide structure
- If a slide still feels empty after layout cleanup, add a compact takeaway band, supporting metric row, comparison card, or source-backed annotation instead of leaving decorative blank area

## Preview checklist

Check every slide in the browser:

- no overflow
- no missing images
- no distorted image crops
- no font fallback
- citation bar visible
- page number visible
- no giant dead space
- no oversized empty cards
- no text that feels too small when viewed as a slide screenshot
- no remote-only resources that break offline
- no content slide that feels under-filled relative to its headline

Recommended QA:

1. take screenshots of every slide or at least the suspicious ones
2. call `$visual-review` on the screenshots instead of doing an unstructured visual pass
3. pass:
   - `target issue`: clipping, unreadable text, weak hierarchy, dead space, bad box proportions, or render bug
   - `artifact scope`: slide numbers or slide ids
   - `decision lens`: sign-off gate
   - `output need`: `E. Targeted audit`
4. only export after the screenshots look presentation-ready

## Export rules

The final PDF should normally be generated with the browser's native PDF renderer, not by stitching screenshots.

Required behavior:

1. open `presentation.html`
2. wait for `document.fonts.ready`
3. wait for all images to finish loading
4. disable animations and transitions
5. emulate print
6. use `page.pdf()` with:
   - `printBackground: true`
   - `preferCSSPageSize: true`
   - zero margins
7. use screenshots as QA artifacts, not as the final PDF pages

This keeps text and vector elements sharper while still matching the audited browser layout.

## Post-export parity check

Do not stop after `page.pdf()`.

1. confirm PDF page count equals the number of `.slide` elements
2. capture or render PDF pages for comparison when needed
3. call `$visual-review` on browser screenshots and exported PDF pages when checking:
   - shifted blocks
   - clipped content
   - missing backgrounds or colors
   - font reflow
   - print-only spacing regressions
4. if the PDF differs materially from the browser view, fix the HTML/CSS, asset loading, or print rules first

## Failure handling

If the PDF differs from the browser view, check in this order:

1. remote resources still referenced in HTML
2. font reflow after load
3. animations / transitions still running
4. content overflow or empty-box imbalance in browser
5. print CSS accidentally changing layout
6. element-level cropping or bad image aspect ratio

Do not "fix it in PDF" if the HTML is wrong.
