---

allowed_tools:
- shell
- browser
description: Convert AI diagrams, screenshots, sketches into publication-ready TikZ/LaTeX standalone figures.
metadata:
  platforms:
  - supported
  tags:
  - tikz
  - latex
  - standalone
  - paper-figure
  - figure-conversion
  version: '1.0.0'
name: tikz-paper-figure
scene: slides
risk: low
routing_gate: none
routing_layer: L3
routing_owner: owner
routing_priority: P2
session_start: n/a
short_description: Convert AI/raster drafts into paper-ready TikZ standalone figures
source: project
trigger_hints:
- AI image to TikZ
- AI 图转 TikZ
- AI 图转论文图
- LaTeX standalone
- LaTeX 配图
- TikZ
- method figure
- paper figure
- protocol diagram
- recreate this figure in TikZ
- standalone figure
- system architecture figure
- thesis figure
- tikz-paper-figure
- 复刻图片
- 截图转 TikZ
- 论文 TikZ
- 论文配图
---
# tikz-paper-figure

Owns one narrow job: turn an AI/raster/rough diagram or paper-figure request
into a **compiled TikZ standalone asset** that is ready to include in a paper.

The default output is not a full paper, not a Mermaid diagram, and not a
one-off screenshot. It is:

- `figure.tex`: `standalone` class with a self-contained `tikzpicture`
- `figure.pdf`: cropped vector output for LaTeX papers
- `figure.png`: 300 DPI preview for visual review
- `include-snippet.tex`: a minimal `figure` environment that includes the PDF

## When To Use

- The user wants to convert an AI-generated image, screenshot, sketch, or rough
  visual into TikZ.
- The user wants a paper method figure, architecture diagram, pipeline,
  protocol flow, sequence-style interaction, geometric explanation, or compact
  algorithm schematic in LaTeX.
- The user explicitly says `TikZ`, `standalone`, `LaTeX figure`, `paper figure`,
  `AI image to TikZ`, `screenshot to TikZ`, `replicate this figure`, or
  `论文配图`.
- The output must be editable source plus compiled paper assets.

## Do Not Use

- For Mermaid or Graphviz/DOT source diagrams: use `diagramming`.
- For matplotlib/seaborn/plotnine data charts: use `scientific-figure-plotting`.
- For generic LaTeX build speed or externalization tuning: use
  `latex-compile-acceleration`.
- For raster illustration generation where TikZ source is not required: use the
  image generation path instead.

## Required Workflow

1. Extract the figure claim: what the reader should understand in five seconds.
2. If an image is provided, measure or estimate the canvas ratio, major zones,
   node bounding boxes, arrow directions, and label hierarchy before coding.
3. Choose paper target width: single column `89mm`, double column `183mm`, or a
   user-specified width. Design at final aspect ratio.
4. Write an explicit drawing brief before code: layout, zones, nodes, edges,
   labels, style, and what to omit from the raster draft.
5. Generate a self-contained `standalone` `.tex` using
   `skills/tikz-paper-figure/assets/standalone-figure.tex` as the default
   template (paths are repo-root relative; if `cwd` is the skill directory, the
   leading `skills/tikz-paper-figure/` may be dropped).
6. Run `skills/tikz-paper-figure/scripts/check_tikz_figure.sh figure.tex`
   before compiling.
7. Compile with `skills/tikz-paper-figure/scripts/compile_standalone.sh
   figure.tex`.
8. Inspect the rendered PNG. Do not claim completion from code alone.
9. Iterate until there are no clipped labels, unreadable text, arrow-direction
   errors, overlap, excessive whitespace, or paper-inclusion issues.
10. Deliver paths to `.tex`, `.pdf`, `.png`, plus the include snippet.

## Standalone Rules

- Use `\documentclass[tikz,border=2pt]{standalone}` unless a larger border is
  needed for external loops.
- Keep captions, figure numbers, and paper labels outside the standalone file.
- Prefer English labels for paper figures unless the target paper is Chinese.
- Use vector-native TikZ primitives for boxes, arrows, braces, paths, matrices,
  and lightweight embedded mini-charts.
- Do not embed the original AI image as the final figure unless the user asks
  for a traced-underlay workflow. The final asset should be editable TikZ.
- Use `xelatex` when CJK labels or system fonts are required; otherwise
  `pdflatex` is acceptable if the figure compiles cleanly.

## Quality Bar

The figure is complete only when:

- The standalone file compiles without fatal errors.
- The PDF is cropped and can be included with `\includegraphics`.
- The PNG preview has readable text at the final intended width.
- Arrows visually flow in the same direction as the paper semantics.
- Lines do not cross labels, pierce boxes, or share confusing dashed rails.
- The figure has a clear hierarchy: primary module, supporting modules, flows,
  and annotations are visually distinct.
- No caption, figure number, prompt text, or AI-generation metadata is drawn
  inside the TikZ canvas.

## References

- Read `references/standalone-workflow.md` when creating or compiling assets.
- Read `references/tikz-paper-rules.md` before writing non-trivial TikZ.
- Read `references/visual-review-checklist.md` before final delivery.
