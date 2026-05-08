# TikZ Paper Rules

Use these rules before writing non-trivial TikZ for a paper figure.

## Drawing Brief First

Before coding, write a short brief:

- Claim: what the figure proves or explains.
- Canvas: target width, aspect ratio, and final paper placement.
- Zones: logical regions and their relative positions.
- Nodes: every visible module, grouped by hierarchy.
- Edges: direction, type, and label of every flow.
- Omissions: details from the raster/AI draft that should not survive into the
  paper figure.

## Layout

- Design at the final aspect ratio, not on an infinite canvas.
- Prefer clear left-to-right pipelines for method flow and top-to-bottom layers
  for stack/architecture figures.
- Keep core modules larger than support modules.
- Use background zones sparingly; their labels must not overlap content.
- Same-level boxes should have consistent widths/heights.
- Avoid large empty regions; add meaningful annotations only when they clarify
  the figure claim.

## Text

- Keep labels short enough to read at paper width.
- Do not include `Figure 1`, captions, prompt text, watermarks, or AI metadata
  inside the TikZ canvas.
- Put secondary details inside the box as a smaller second line rather than as
  floating labels that arrows can cross.
- Prefer English labels for international paper figures unless the manuscript is
  Chinese.

## Arrows And Paths

- Arrow direction must match semantic flow at first glance.
- If source and target align horizontally or vertically, use a straight line.
- Use `|-` or `-|` paths for orthogonal routing when needed.
- Avoid diagonal connector segments in architecture/pipeline diagrams unless
  they encode geometry.
- Do not let lines cross labels, pierce boxes, or terminate inside nodes.
- Dashed feedback rails need enough separation from dashed zone borders.
- Multiple arrows entering the same node should use separated anchors.

## Standalone Safety

- Keep the preamble minimal and figure-local.
- Use named styles for repeated nodes and lines.
- Use `\usetikzlibrary{arrows.meta,positioning,calc,fit,backgrounds}` by
  default; add other libraries only when needed.
- Use `xelatex` for CJK labels. Never rotate CJK labels with `rotate=90`.
- Keep the original raster as a temporary reference only; final output should be
  editable vector TikZ.

## Conversion From AI/Raster Images

- Measure or estimate original aspect ratio.
- Identify visual hierarchy: title-like labels, main blocks, support blocks,
  callouts, and legends.
- Rebuild with paper-appropriate simplification. Do not trace decorative noise.
- Replace fuzzy raster text with clean LaTeX text.
- Replace imprecise icons with simple TikZ symbols unless the icon carries
  essential meaning.

