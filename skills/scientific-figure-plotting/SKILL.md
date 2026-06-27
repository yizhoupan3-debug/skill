---

description: Create, refactor, and review code-generated scientific figures for papers using matplotlib, seaborn, plotnine, or related Python plotting tools. Use for 科研出图, publication charts, journal style figures
metadata:
  platforms:
  - supported
  tags:
  - scientific-figures
  - plotting
  - matplotlib
  - seaborn
  - publication
  - charts
  version: '1.2.0'
name: scientific-figure-plotting
scene: visual
risk: medium
routing_gate: none
routing_layer: L4
routing_owner: owner
routing_priority: P2
runtime_requirements:
  python:
  - matplotlib
  - numpy
  - scienceplots
session_start: preferred
source: project
trigger_hints:
- CJK font
- charts
- colorblind-safe palettes
- matplotlib
- plotting
- publication
- scientific figures
- seaborn
- statistical annotations
- 科研出图
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
- Infographics or marketing visuals -> answer in the current context, or use design skills（参见 `infographic/SKILL.md`）.

## Figure Rules

- Keep data transformation separate from plotting code.
- Choose chart type from the scientific claim, not from aesthetics alone.
- Label axes, units, groups, sample sizes, and uncertainty clearly.
- Prefer colorblind-safe palettes and readable typography.
- Export publication assets with explicit size, DPI, and vector/raster choice.
- Do not imply statistical significance without validated test outputs.

## Hard constraints

- 出版级图表最低 DPI 为 300（print）或 150（screen），不得低于此值
- 每张图必须可独立理解——不依赖正文才能解读坐标轴和图例
- 统计显著性标注必须附带检验方法和 p 值来源，不得仅标星号无依据
- 颜色方案必须通过色盲友好检查（deuteranopia/protanopia），不得仅用红绿区分
- 导出路径必须是确定性的（非临时文件），脚本可重复生成同一输出

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
