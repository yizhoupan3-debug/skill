# Style Libraries for Publication-Grade Figures

## Recommended Libraries

### SciencePlots (garrettj403/SciencePlots)

Journal-specific matplotlib styles. Install and use:

```bash
pip install SciencePlots
# or
uv pip install SciencePlots
```

```python
import scienceplots

# IEEE style
with plt.style.context(['science', 'ieee']):
    fig, ax = plt.subplots()
    ax.plot(x, y)

# Nature style
with plt.style.context(['science', 'nature']):
    fig, ax = plt.subplots()
    ax.plot(x, y)

# High-contrast for presentations
with plt.style.context(['science', 'high-contrast']):
    fig, ax = plt.subplots()
    ax.plot(x, y)
```

Available style combos: `science`, `ieee`, `nature`, `scatter`, `notebook`,
`high-vis`, `high-contrast`, `light`, `grid`, `retro`, `muted`.

### LovelyPlots (killiansheriff/LovelyPlots)

Produces Illustrator-editable SVG exports. Best when the figure needs
post-generation touch-up in a vector editor.

```bash
pip install LovelyPlots
```

```python
import LovelyPlots

plt.style.use('ipynb')  # for notebooks
plt.style.use('paper')  # for papers

# Save as editable SVG
fig.savefig('figure.svg', format='svg')
```

### ExtensysPlots (mcekwonu/ExtensysPlots)

Alternative scientific style with clean defaults.

```bash
pip install ExtensysPlots
```

```python
import ExtensysPlots

plt.style.use('extensys')
```

---

## Colorblind-Safe Palettes

### Quick recommendations

| Palette | Source | Best for |
|---------|--------|----------|
| `tableau-colorblind10` | matplotlib built-in | ≤ 10 categories |
| `colorblind` | seaborn | ≤ 6 categories |
| `Set2` | ColorBrewer | ≤ 8 qualitative categories |
| `viridis` / `cividis` | matplotlib built-in | continuous/sequential data |
| `RdYlBu` | ColorBrewer | diverging data |

### Usage examples

```python
import matplotlib.pyplot as plt
import seaborn as sns

# Option 1: matplotlib colorblind-safe cycle
plt.rcParams['axes.prop_cycle'] = plt.cycler(
    color=plt.cm.tab10.colors[:6]
)

# Option 2: seaborn colorblind palette
sns.set_palette("colorblind")

# Option 3: explicit tableau-colorblind10
from matplotlib import colormaps
cmap = colormaps['tab10']

# Option 4: for continuous data, always prefer perceptually uniform
plt.imshow(data, cmap='viridis')   # sequential
plt.imshow(data, cmap='cividis')   # colorblind-safe sequential
plt.imshow(data, cmap='RdYlBu')   # diverging
```

### Verification

Use the `colorspacious` package or online simulators (e.g., Coblis) to check
how your figure looks under deuteranopia, protanopia, and tritanopia.

```bash
pip install colorspacious
```

---

## Publication Output Settings

### Recommended save defaults

```python
fig.savefig(
    'figure.pdf',
    dpi=300,
    bbox_inches='tight',
    pad_inches=0.05,
    transparent=False,
)

# For raster (when vector is not accepted)
fig.savefig(
    'figure.png',
    dpi=600,           # high DPI for print
    bbox_inches='tight',
    pad_inches=0.05,
)

# For Illustrator editing
fig.savefig(
    'figure.svg',
    format='svg',
    bbox_inches='tight',
)
```

### Figure sizing for papers

```python
# Single column (typical ~3.5 inches wide)
fig, ax = plt.subplots(figsize=(3.5, 2.5))

# Double column (typical ~7 inches wide)
fig, ax = plt.subplots(figsize=(7, 4))

# Nature single column (~89mm = 3.5in)
fig, ax = plt.subplots(figsize=(3.5, 2.625))

# IEEE single column (~3.5in)
fig, ax = plt.subplots(figsize=(3.5, 2.5))
```
