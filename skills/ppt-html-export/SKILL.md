---
name: ppt-html-export
description: |
  Use when the user wants to create, revise, or export a slide deck / PPT /
  presentation from an outline, notes, markdown, or research materials and the
  final deliverable should be HTML slides plus a browser-matched PDF. Best for
  requests like "做个PPT", "生成演示文稿", "根据大纲做汇报 slides", or when
  HTML/CSS layout control, page-by-page browser QA, and high-fidelity PDF export
  matter more than editable `.pptx`.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
runtime_requirements:
  commands:
    - node
metadata:
  version: "1.0.0"
  platforms: [codex, antigravity]
  tags:
    - ppt
    - html
    - export
---

# PPT HTML Export

Build presentations as fixed-size HTML slides first, audit them page by page in the browser, then export them to PDF with the browser's native PDF renderer.

Default to this skill when visual fidelity matters more than editable `.pptx` output. If the user explicitly needs a native PowerPoint file, say that this skill is still useful for high-fidelity layout prototyping, but the output chain is HTML -> PDF by default.

## When to use

- The user wants HTML slides with pixel-perfect CSS layout and browser-matched PDF
- The user wants full control over HTML/CSS styling for presentations
- Visual fidelity and browser-based QA matter more than PowerPoint editability

## Do not use

- Do not use when the user wants a native editable `.pptx`; use `$ppt-pptx`
- Do not use when the user wants LaTeX Beamer source plus PDF; use `$ppt-beamer`
- Do not use when HTML output is only a temporary scaffold for a native PPTX deliverable
- Do not use when the user wants a fast Markdown-to-slides workflow with Slidev or Marp; use `$ppt-markdown`

## Workflow

1. Create or reuse a project folder that contains:
   - `presentation.html`
   - `print_pdf.js`
   - `assets/`
   - `sources.md` or an equivalent source log
2. Turn the outline into a slide plan:
   - cover
   - 4-8 content slides
   - summary / closing slide if needed
3. Collect images before writing the final HTML.
   - Use user-provided images first.
   - Otherwise search online, prefer official / institutional / map / media sources, then download every chosen image into local `assets/`.
   - Do not leave remote image URLs in the final HTML.
4. Build `presentation.html` on a fixed `1920x1080` canvas with local assets only.
5. Audit the browser view before export.
   - Open the HTML in a browser and inspect every slide for overflow, missing images, font fallback, broken citations, awkward empty space, oversized empty cards, weak hierarchy, and small unreadable body text.
   - Capture slide screenshots for QA before final export.
   - Screenshot each `.slide` or at least the suspicious slides.
   - Call `$visual-review` on those screenshots instead of relying on ad hoc visual judgment. Pass:
     - `target issue`: clipping, weak hierarchy, unreadable text, dead space, bad box proportions, or another concrete defect
     - `artifact scope`: slide numbers or slide ids
     - `decision lens`: presence check or sign-off gate
     - `output need`: `E. Targeted audit`
6. Export with the bundled browser-native PDF pattern.
   - Wait for fonts and images.
   - Disable animations.
   - Emulate print.
   - Use `page.pdf()` with `printBackground: true` and `preferCSSPageSize: true`.
7. Audit browser/PDF parity after export.
   - Confirm PDF page count matches the number of `.slide` elements.
   - Compare the exported PDF pages against the audited browser slides.
   - Use `$visual-review` again when checking for print-only regressions such as shifted blocks, cropped content, missing backgrounds, or font reflow.
8. Deliver the HTML, PDF, and source list together.

## Non-Negotiables

- Treat each slide as a separate `.slide` block sized to exactly `1920x1080`.
- Use local relative paths for images in final output.
- Do not build the final deck by stitching slide screenshots into a PDF unless the user explicitly needs image-only pages.
- Keep one layout only; print CSS may handle page size, page breaks, and color preservation, but must not create a second layout.
- Verify the browser view before exporting.
- If the browser view is wrong, fix HTML/CSS first instead of trying to patch the PDF stage.
- Do not skip screenshot-based visual QA just because the browser page "looks roughly fine." Small text, dead space, and export regressions are easy to miss without a structured audit.
- Use `$visual-review` in targeted audit mode for both browser screenshots and post-export parity checks.
- Avoid content grids that force equal-height empty boxes. If a slide is content-driven, prefer `grid-auto-rows: auto` or explicit `auto` rows over `1fr`.
- On a `1920x1080` slide, body copy should usually not drop below `18px`; dense explanatory text should usually be `18-20px`, and small labels / notes should usually stay at `14-16px`.
- If a card looks empty after content is placed, change the structure rather than leaving a large decorative box.
- Default toward fuller slides: most content slides should feel visually occupied rather than sparse, but still readable. If a slide has large dead zones, add another evidence block, comparison, takeaway strip, or secondary chart before accepting it.
- Raise information density by structuring content, not by shrinking text: prefer 2-tier layouts, summary strips, side notes, mini-metrics, and compact evidence cards.
- If the PDF differs from the browser view, fix HTML/CSS, asset loading, or print CSS first instead of treating PDF export as a separate layout target.

## Resource Guide

- Read [references/workflow.md](./references/workflow.md) when building a deck from scratch or when you need the detailed design / export checklist.
- Read [references/design-system.md](./references/design-system.md) when the deck needs stronger aesthetics, clearer hierarchy, or a more distinctive visual direction.
- Read [references/visual-design-principles.md](./references/visual-design-principles.md) for color theory (60/30/10 rule), typography hierarchy at 1920×1080, composition patterns, and pre-built slide palettes.
- Copy [assets/presentation.template.html](./assets/presentation.template.html) as the starting point for new decks.
- Copy [assets/print_pdf.template.js](./assets/print_pdf.template.js) as the starting point for the exporter.
- Copy or invoke [scripts/export_pdf.js](./scripts/export_pdf.js) to export HTML slides to PDF via Puppeteer with font/image waiting and animation disabling.
- Copy or invoke [scripts/screenshot_slides.js](./scripts/screenshot_slides.js) to capture per-slide PNG screenshots for QA and `$visual-review`.

## Practical Defaults

- Default output: polished HTML presentation plus matching PDF.
- Default visual direction: dark, editorial, data-friendly layout unless the repo already has an established style.
- Default asset policy: user images first, then downloaded local images, then charts / diagrams if no suitable photo exists.
- Default citation policy: cite every quantitative claim and every external image source.
- Default typography bias: make the deck readable from a screenshot first; prefer fewer words and larger text over cramming content into undersized cards.
- Default layout bias: if a slide feels sparse, first add one more meaningful layer of information, then merge or resize boxes; if a slide feels cramped, split content into fewer cards or another slide.
- Default density target: content slides should usually contain 2-4 distinct information zones and avoid leaving more than roughly a quarter of the slide as purposeless empty space.
- Default QA policy: audit browser screenshots with `$visual-review`, export to PDF, then audit browser/PDF parity with `$visual-review` before sign-off.

## Final Checks

- Confirm slide count matches the plan.
- Confirm every slide fits without clipping.
- Confirm the PDF page count matches the number of `.slide` elements.
- Confirm the browser screenshots were audited through `$visual-review`.
- Confirm the PDF visually matches the browser slide view.
- Confirm post-export parity was checked through `$visual-review`, not only by casual eyeballing.
- Confirm no key slide relies on tiny paragraph text to fit.
- Confirm no major card is mostly decorative empty space.
- Confirm each content slide has enough substance to justify the page and does not read like a headline floating over empty containers.
