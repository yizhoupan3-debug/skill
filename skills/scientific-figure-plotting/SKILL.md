---
name: scientific-figure-plotting
description: |
  Create, refactor, and review code-generated scientific figures for papers
  using matplotlib, seaborn, plotnine, or related Python plotting tools. Use for
  科研出图, publication charts, journal style figures, statistical annotations,
  colorblind-safe palettes, Raincloud/Ridge plots, or CJK font troubleshooting.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - scientific figures
  - plotting
  - matplotlib
  - seaborn
  - publication
  - charts
  - 科研出图
  - statistical annotations
  - colorblind-safe palettes
  - CJK font
runtime_requirements:
  python:
    - matplotlib
    - numpy
    - scienceplots
metadata:
  version: "1.2.0"
  platforms: [codex]
  tags: [scientific-figures, plotting, matplotlib, seaborn, publication, charts]
risk: medium
source: local

---

# scientific-figure-plotting

This skill owns paper-grade scientific figure code and review. It should
produce reproducible scripts, not one-off screenshots.

## When to Use

- The user needs figures for a paper, thesis, poster, or scientific report.
- The work involves matplotlib, seaborn, plotnine, statistical annotations, or journal styling.
- The user wants chart-type choice, figure polish, or reproducible plotting code.
- CJK fonts, colorblind palettes, export DPI, vector output, or panel layout matter.

## Do Not Use

- General data cleaning before plotting -> answer in the current data/implementation context.
- Statistical test choice without figure work -> use `$statistical-analysis`.
- Visual review of an already-rendered figure only -> use `$visual-review`.
- Infographics or marketing visuals -> use `$infographic` or design skills.

## Figure Rules

- Keep data transformation separate from plotting code.
- Choose chart type from the scientific claim, not from aesthetics alone.
- Label axes, units, groups, sample sizes, and uncertainty clearly.
- Prefer colorblind-safe palettes and readable typography.
- Export publication assets with explicit size, DPI, and vector/raster choice.
- Do not imply statistical significance without validated test outputs.

## Workflow

1. Identify claim, variables, audience, journal/context, and output format.
2. Inspect data shape and any required statistical summaries.
3. Select chart type and layout.
4. Write reproducible plotting code with deterministic export paths.
5. Render and inspect for clipping, readability, legend clarity, and style consistency.
6. Pair with `$visual-review` when image-grounded critique is needed.

## Output Defaults

- Plotting script or patch.
- Exported figure path when generated.
- Short notes on statistical or visual caveats.
- Re-render status when verification ran.

## References

- [references/chart-type-decision-tree.md](./references/chart-type-decision-tree.md)
- [references/chart-recipes.md](./references/chart-recipes.md)
- [references/stat-annotations.md](./references/stat-annotations.md)
- [references/cjk-font-guide.md](./references/cjk-font-guide.md)
- [references/auto-review-workflow.md](./references/auto-review-workflow.md)
