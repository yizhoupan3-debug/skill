---
name: scientific-figure-plotting
description: |
  Create, refactor, and review code-generated scientific figures for papers
  using matplotlib, seaborn, plotnine, and pingouin.
  Supports 7+ journal styles (Nature, Science, Cell, IEEE, Lancet, BMJ, Neurips).
  Use for 科研出图, Raincloud Plots, Ridge Plots, statistical annotations,
  colorblind-safe palettes, or CJK font troubleshooting.
  Pair with $visual-review when a rendered artifact exists.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - scientific figures
  - plotting
  - matplotlib
  - seaborn
  - publication
  - charts
runtime_requirements:
  python:
    - matplotlib
    - numpy
    - scienceplots
metadata:
  version: "1.1.0"
  platforms: [codex]
  tags: [scientific-figures, plotting, matplotlib, seaborn, publication, charts]
risk: medium
source: local
---

- **Dual-Dimension Audit (Pre: Plotting-Logic, Post: Visual-Fidelity/Overlap Results)** → `$execution-audit` [Overlay]

# Scientific Figure Plotting

This skill owns **code-generated scientific figure quality** for paper charts and multi-panel results figures.

## When to use

- The user wants to create or improve publication-grade charts from code
- The user asks for high-end paper figure defaults rather than only audit comments
- The task mentions:
  - code plotting
  - matplotlib / seaborn / plotnine
  - scientific charts
  - submission-ready figures
  - top-tier / journal / conference quality
  - dense but readable information
  - English-first labeling
  - chart type selection / what chart to use
  - statistical annotations / error bars / significance brackets
  - 3D surface / scatter for scientific data
  - overlap removal
  - multi-panel figure layout

## Do not use

- The user wants Mermaid or text diagrams → use `$mermaid-expert`
- The user only wants review or repair of an existing paper figure/table artifact without a code-plotting workflow → use `$paper-visuals`
- The user wants whole-paper review or revision rather than plotting-system work → use `$paper-reviewer` or `$paper-reviser`

## Task ownership and boundaries

This skill owns:
- chart family selection and encoding choice
- plotting defaults and style systems
- publication-grade typography, spacing, palette, and line-weight choices
- information-density planning
- label, legend, annotation, and panel-overlap prevention
- multi-panel figure composition for paper results
- export defaults for paper-ready raster/vector output
- visual QA handoff to `$visual-review`

This skill does not own:
- generic paper logic review
- table formatting
- Mermaid diagrams
- non-code illustration art

## Default quality bar

Assume these defaults unless the user overrides them:

- language: **English** (`locale="en"`, default)
- locale option: set `locale="zh"` for Chinese labels/titles (auto-detects macOS CJK fonts)
- tone: **advanced / publication-grade**
- density: **high information density without clutter**
- overlap policy: **no unresolved overlap across text, ticks, legends, markers, panels, or annotations**
- color accessibility: **colorblind-safe palettes by default** (see `references/style-libraries.md`)
- output goal: **top-tier paper figure, not notebook demo style**

## Required workflow

1. Identify:
   - figure purpose
   - target claim
   - audience and venue bar
   - chart family
   - expected export artifact
2. Decide whether the current figure problem is:
   - encoding problem
   - layout problem
   - typography problem
   - density problem
   - overlap problem
   - multi-panel composition problem
3. Improve the plotting code and export a render.
4. If a render exists or can be exported, call `$visual-review` before finalizing.
5. Do not stop at "looks okay"; clear the publication bar explicitly.

## Figure design rules

### 1. Information density

- Prefer **dense but structured** figures over sparse demo-style plots
- Put detail where it supports comparison, not decoration
- Use small multiples, inset summaries, aligned panels, or compact annotations instead of dumping all detail into one legend-heavy chart
- Every added element must earn its place by improving comparison, context, or interpretation

### 2. No-overlap policy

Treat all of these as defects unless proven harmless:
- tick labels colliding
- axis titles crowding ticks
- legends covering data
- annotations crossing important marks
- panel titles, captions, or shared labels colliding
- too many markers at the same position without aggregation or jitter strategy
- crowded colorbars / shared legends / panel spacings

If overlap risk remains after code changes, export and verify with `$visual-review`.

### 3. English-first publication defaults

- Default all visible text to English unless the user requests otherwise
- Prefer concise scientific noun phrases over conversational labels
- Use consistent terminology across axes, legends, panel titles, and captions
- Expand ambiguous abbreviations unless the domain strongly expects them

### 4. Publication-grade styling

- Favor restrained palettes, consistent stroke hierarchy, and disciplined whitespace
- Avoid default notebook styling
- Use typography and line weights that survive likely paper scale reduction
- Prefer direct labeling when it reduces legend search cost
- Use legends only when they are cleaner than direct labels

### 5. Multi-panel composition

- Align scales when comparison is intended
- Share legends and axis labels when it reduces repetition without harming readability
- Keep panel order semantically meaningful
- Use panel lettering only when the artifact actually needs it
- Prevent repeated information across adjacent panels unless repetition materially helps interpretation

### 6. Statistical annotations

- Always include error bars on summary statistics; specify type (SD / SEM / 95% CI) in caption
- Show individual data points when n < 30; overlay on bars or violins
- Use significance brackets with star convention (\*, \*\*, \*\*\*) only when statistical tests are reported
- Report sample size (n) per group in axis labels, annotations, or caption
- Prefer `statannotations` library for automated bracket placement
- See [`references/stat-annotations.md`](references/stat-annotations.md) for implementation patterns

### 7. 3D figure constraints

- Use 3D plots only when the 3D shape itself is the core message
- Always provide a 2D alternative (contour / heatmap) alongside for quantitative reading
- Set `view_init(elev, azim)` explicitly for reproducible viewing angle
- Never use 3D bar charts — perspective distorts magnitude comparison
- Label all three axes with units
- See [`references/chart-recipes.md`](references/chart-recipes.md) recipe #6 for implementation

### 8. CJK / Chinese mode

- Activate with `apply_publication_style(locale="zh")`
- The helper auto-detects the best available CJK font (PingFang SC → Hiragino Sans GB → STHeiti → Songti SC → fallback chain)
- `axes.unicode_minus` is set to `False` to prevent minus sign rendering as a box
- Prefer PingFang SC or Hiragino Sans GB for mixed CJK + Latin text — they have balanced Latin glyph widths
- Avoid Songti SC for mixed text (narrow Latin glyphs look cramped)

**Handling missing glyphs in user-written labels:**

CJK fonts are missing some Unicode chars (U+2212 minus, U+00B2/B3 superscripts, etc.). Two solutions:

```python
# Option A: sanitize individual strings
from publication_rcparams import sanitize_cjk_text
ax.set_xlabel(sanitize_cjk_text("范围 −3 to 3"))  # → "范围 -3 to 3"

# Option B: auto-fix all text in figure before saving
from publication_rcparams import patch_figure_cjk
patch_figure_cjk(fig)  # walks all Text objects, returns count of fixes
fig.savefig("out.png")
```

- See [`references/cjk-font-guide.md`](references/cjk-font-guide.md) for troubleshooting and manual overrides

## Output defaults

Use `出图改进建议` or `出图实现记录` depending on the request:

- figure goal
- current quality risks
- code-level changes to make
- style/default changes
- overlap risks removed
- exported artifact checked or not
- `$visual-review` result when available
- remaining risks

## Hard constraints

- Do not confuse "fancier" with "better"
- Do not increase density by adding redundant encodings
- Do not leave overlap unresolved just because it is slight
- Do not accept default plotting-library aesthetics as publication-ready without review.
- **Superior Quality Audit**: For publication-grade figures, trigger `$execution-audit` to verify against [Superior Quality Bar](../execution-audit/references/superior-quality-bar.md).
- When rendered evidence is available, do not skip `$visual-review`

## References

For SciencePlots, LovelyPlots, ExtensysPlots, colorblind palettes, and
publication export settings, see
[`references/style-libraries.md`](references/style-libraries.md).

For plotnine (ggplot2-style grammar of graphics in Python), see
[`references/plotnine-guide.md`](references/plotnine-guide.md).

For the plot → export → visual QA closed-loop workflow, see
[`references/auto-review-workflow.md`](references/auto-review-workflow.md).

For chart type selection guidance (data shape → chart family → library), see
[`references/chart-type-decision-tree.md`](references/chart-type-decision-tree.md).

For error bars, significance brackets, and data point overlay patterns, see
[`references/stat-annotations.md`](references/stat-annotations.md).

For copy-paste-ready code templates (line, bar, violin, heatmap, multi-panel,
3D surface), see [`references/chart-recipes.md`](references/chart-recipes.md).

For CJK font detection, macOS Chinese font troubleshooting, and manual
overrides, see [`references/cjk-font-guide.md`](references/cjk-font-guide.md).

To preview all available matplotlib + SciencePlots styles side by side, run
`scripts/preview_styles.py` (generates comparison images to `tmp/style_previews/`).

To test CJK font rendering on this machine, run
`scripts/test_cjk_fonts.py` (generates test images to `/tmp/cjk_font_test/`).

## Trigger examples

- "Use $scientific-figure-plotting to make these matplotlib charts submission-ready."
- "Use $scientific-figure-plotting to redesign this results figure for top-tier paper quality."
- "Use $scientific-figure-plotting to improve information density, keep everything in English, and remove overlap."
- "Apply SciencePlots IEEE style to these figures."
- "Make these charts colorblind-friendly."
- "What chart type should I use for this data?"
- "Add error bars and significance markers to this bar chart."
- "Help me choose between 3D surface and contour plot."
- "中文出图"
- "图表用中文标题和轴标签"
- "Use Chinese labels for this matplotlib figure."
- "强制进行科研出图深度审计 / 检查图表排版与视觉还原结果。"
- "Use $execution-audit to audit this scientific figure for visual-fidelity idealism."
