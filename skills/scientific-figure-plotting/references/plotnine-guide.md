# plotnine (ggplot2-like) Guide

## Overview

[plotnine](https://plotnine.org/) brings R's ggplot2 grammar of graphics to
Python. It is well-suited for users coming from R who want the same
declarative, layered approach to building publication-quality figures.

## Installation

```bash
pip install plotnine
# or
uv pip install plotnine
```

## Basic Usage

```python
from plotnine import *
import pandas as pd

df = pd.DataFrame({
    'x': range(10),
    'y': [2, 3, 5, 7, 11, 13, 17, 19, 23, 29],
    'group': ['A'] * 5 + ['B'] * 5
})

# Basic scatter plot
p = (
    ggplot(df, aes('x', 'y', color='group'))
    + geom_point(size=3)
    + geom_line()
    + labs(title='Prime Numbers', x='Index', y='Value')
    + theme_minimal()
)
p.save('scatter.pdf', dpi=300, width=3.5, height=2.5)
```

## Publication-Grade Theming

```python
from plotnine import *

# Clean publication theme
pub_theme = (
    theme_minimal()
    + theme(
        text=element_text(family='serif', size=10),
        axis_title=element_text(size=10),
        axis_text=element_text(size=8),
        legend_title=element_text(size=9),
        legend_text=element_text(size=8),
        legend_position='bottom',
        panel_grid_minor=element_blank(),
        panel_border=element_rect(color='black', size=0.5),
        strip_text=element_text(size=9, face='bold'),
        figure_size=(3.5, 2.5),  # single column
    )
)

p = (
    ggplot(df, aes('x', 'y'))
    + geom_point()
    + pub_theme
)
```

## Multi-Panel Figures (facets)

```python
# facet_wrap for one variable
p = (
    ggplot(df, aes('x', 'y'))
    + geom_point()
    + facet_wrap('group', ncol=2)
    + theme_minimal()
)

# facet_grid for two variables
p = (
    ggplot(df, aes('x', 'y'))
    + geom_point()
    + facet_grid('group ~ .')
    + theme_minimal()
)
```

## Colorblind-Safe Palettes

```python
from plotnine import *

# Use ColorBrewer palettes
p = (
    ggplot(df, aes('x', 'y', color='group'))
    + geom_point()
    + scale_color_brewer(type='qual', palette='Set2')
)

# Manual colorblind-safe colors
cb_colors = ['#0072B2', '#D55E00', '#009E73', '#CC79A7', '#F0E442', '#56B4E9']
p = (
    ggplot(df, aes('x', 'y', color='group'))
    + geom_point()
    + scale_color_manual(values=cb_colors)
)
```

## Export Settings

```python
# PDF for vector (preferred for papers)
p.save('figure.pdf', dpi=300, width=3.5, height=2.5)

# PNG for raster
p.save('figure.png', dpi=600, width=3.5, height=2.5)

# SVG for editing
p.save('figure.svg', width=3.5, height=2.5)
```

## When to Use plotnine vs matplotlib

| Scenario | Preferred |
|----------|-----------|
| R user migrating to Python | plotnine |
| Quick exploratory faceted plots | plotnine |
| Complex custom annotations | matplotlib |
| Fine-grained layout control | matplotlib |
| Existing codebase uses matplotlib | matplotlib |
| ggplot2 grammar preferred | plotnine |
