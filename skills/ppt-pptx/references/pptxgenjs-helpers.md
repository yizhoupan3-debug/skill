# PptxGenJS Helpers

## When To Read This

Read this file when you need helper API details, command examples for the bundled Rust tooling, or dependency notes for a slide-generation task.

## Helper Modules

- `autoFontSize(textOrRuns, fontFace, opts)`: Pick a font size that fits a fixed box.
- `calcTextBox(fontSizePt, opts)`: Estimate text-box geometry from font size and content.
- `calcTextBoxHeightSimple(fontSizePt, numLines, leading?, padding?)`: Quick text height estimate.
- `imageSizingCrop(pathOrData, x, y, w, h)`: Center-crop an image into a target box.
- `imageSizingContain(pathOrData, x, y, w, h)`: Fit an image fully inside a target box.
- `svgToDataUri(svgString)`: Convert an SVG string into an embeddable data URI.
- `latexToSvgDataUri(texString)`: Render LaTeX to SVG for crisp equations.
- `getImageDimensions(pathOrData)`: Read image width, height, type, and aspect ratio.
- `safeOuterShadow(...)`: Build a safe outer-shadow config for PowerPoint output.
- `codeToRuns(source, language)`: Convert source code into rich-text runs for `addText`.
- `warnIfSlideHasOverlaps(slide, pptx)`: Emit overlap warnings for diagnostics.
- `warnIfSlideElementsOutOfBounds(slide, pptx)`: Emit boundary warnings for diagnostics.
- `alignSlideElements(slide, indices, alignment)`: Align selected elements precisely.
- `distributeSlideElements(slide, indices, direction)`: Evenly space selected elements.

## Dependency Notes

JavaScript helpers expect these packages when you use the corresponding features:

- Core authoring: `pptxgenjs`
- Text measurement: `skia-canvas`, `linebreak`, `fontkit`
- Syntax highlighting: `prismjs`
- LaTeX rendering: `mathjax-full`

System tools used by Rust tooling:

- `soffice` / LibreOffice for PPTX to PDF conversion
- Poppler tools for PDF size/raster support used in render paths
- `fc-list` for font inspection
- Optional raster conversion tools for vector/unusual assets: Inkscape, ImageMagick, Ghostscript, `heif-convert`, `JxrDecApp`
- Optional deep inspection layer: `officecli` for `outline`, `issues`, `validate`, `get`, `query`, `watch`, and batch edits against existing `.pptx`

## Script Notes

- `ppt init <workdir>`: Create a ready deck workspace from the bundled templates/helpers.
- `ppt outline <outline.yaml|outline.json> --bootstrap --build`: Generate `deck.js` from an outline and build the editable `.pptx`.
- `ppt render <deck>`: Convert a deck to PNGs. Good for visual review and diffing.
- `ppt slides-test <deck>`: Check whether any content leaks outside the original canvas.
- `ppt create-montage --input-file ...`: Combine multiple rendered slide images into a single overview image.
- `ppt detect-fonts <deck>`: Distinguish between fonts that are missing entirely and fonts that are installed but substituted during rendering.
- `ppt office ...`: Use local `officecli` as an optional deep inspection / preview / patch layer for existing decks and rebuild input analysis.
- `ppt build-qa|qa|intake`: Orchestrate the default mixed lane: author in `deck.js`, run Rust QA, then fold in OfficeCLI audit / intake results.

## Practical Rules

- Default to `LAYOUT_WIDE` unless the source material says otherwise.
- Set font families explicitly before measuring text.
- Use cross-platform-safe defaults: `Arial` for general text and `Courier New` for code.
- Avoid platform-specific deck defaults such as `Helvetica Neue`, `Calibri`, or `Consolas`.
- Use `valign: "top"` for content boxes that may grow.
- Prefer native PowerPoint charts over rendered images when the chart is simple and likely to be edited later.
- Use SVG instead of PNG for diagrams whenever possible.
- When rebuilding from an existing `.pptx`, use `ppt office doctor` or `ppt office view/get/query` first instead of guessing the deck structure by hand.
