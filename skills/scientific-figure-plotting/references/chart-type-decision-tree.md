# Chart Type Decision Tree

Select the right chart family based on your **data shape** and **communication goal**.

## Quick decision matrix

| Data Shape | Goal | Recommended Chart | Library |
|-----------|------|-------------------|---------|
| 1 continuous Y vs continuous X | Trend / trajectory | Line plot with error band | matplotlib / seaborn |
| 1 continuous Y vs categorical X | Group comparison | Bar chart + individual points | matplotlib / seaborn |
| Distribution of 1 variable | Shape / spread | Violin + strip / beeswarm | seaborn |
| Distribution of 1 variable (large n) | Shape | Histogram / KDE | matplotlib / seaborn |
| 2 continuous variables | Correlation | Scatter + marginal distributions | seaborn `jointplot` |
| 3+ continuous variables | Multivariate relation | Pair plot / correlation heatmap | seaborn / plotnine |
| Matrix of values | Pattern / magnitude | Heatmap with annotations | seaborn / matplotlib |
| Proportions of a whole | Composition | Stacked bar (not pie) | matplotlib |
| Time series | Evolution over time | Line plot with shaded CI | matplotlib |
| Ranked values | Magnitude comparison | Horizontal bar chart | matplotlib |
| Spatial / grid data | Field visualization | Contour / imshow + colorbar | matplotlib |
| 3D surface or volumetric | Geometry / topology | 3D surface / scatter | matplotlib `mplot3d` |
| Network / graph | Connections / topology | → use `$diagramming` or `$diagramming` | — |
| Survival / Kaplan-Meier | Time-to-event | Step plot + CI bands | lifelines / matplotlib |

## Decision flowchart

```
Start
  ├─ Comparing groups?
  │   ├─ 2–5 groups → bar + points / violin + strip
  │   ├─ Many groups → heatmap / small multiples
  │   └─ Paired data → paired line / slope chart
  ├─ Showing trend over continuous X?
  │   ├─ Few series → line + error band
  │   └─ Many series → small multiples / heatmap
  ├─ Showing distribution?
  │   ├─ 1 group → histogram / KDE
  │   ├─ 2–5 groups → violin + strip / ridgeplot
  │   └─ Many groups → ridgeplot / small multiples
  ├─ Showing correlation?
  │   ├─ 2 variables → scatter + marginals
  │   └─ Many variables → correlation heatmap / pair plot
  ├─ Showing spatial / matrix data?
  │   ├─ Regular grid → imshow / contourf
  │   └─ Irregular → scatter with colormap
  └─ 3D data?
      ├─ Surface → plot_surface
      ├─ Point cloud → scatter3D
      └─ Consider: can a 2D projection (heatmap / contour) convey the same info?
```

## Anti-patterns to avoid

| Bad choice | Why | Use instead |
|-----------|-----|-------------|
| Pie chart | Hard to compare angles; breaks with >5 categories | Stacked / grouped bar |
| 3D bar chart | Perspective distorts magnitude | 2D grouped bar |
| Dual Y-axis without justification | Misleading scale correlation | Separate panels |
| Rainbow/jet colormap | Perceptually non-uniform; colorblind-unfriendly | viridis / cividis |
| Box plot alone (small n) | Hides individual data points | Violin + strip |
| Stacked area (many categories) | Hard to read middle layers | Small multiples |

## When to use small multiples (facets)

Prefer small multiples (faceted panels) over a single crowded plot when:
- You have **>4 series** that would overlap on a single axis
- You want to compare **shape** across groups (not just level)
- Each panel can use **the same scale** for fair comparison
- The audience needs to see **individual patterns**, not just aggregate

Implementation: `plt.subplots()` grid, seaborn `FacetGrid`, or plotnine `facet_wrap` / `facet_grid`.
