# Chart Recipes: Publication-Ready Code Templates

Minimal, copy-paste-ready templates for common scientific figure types.
All recipes use colorblind-safe palettes and publication-grade defaults.

## Setup (shared across all recipes)

```python
import matplotlib.pyplot as plt
import numpy as np

plt.rcParams.update({
    "font.family": "sans-serif",
    "font.size": 8,
    "axes.linewidth": 0.8,
    "axes.unicode_minus": False,
    "figure.dpi": 150,
    "savefig.dpi": 300,
})
```

---

## 1. Line plot with error band

Best for: **trends, time series, model predictions with uncertainty**.

```python
fig, ax = plt.subplots(figsize=(3.5, 2.5))

x = np.linspace(0, 10, 50)
y_mean = np.sin(x)
y_std = 0.2 + 0.1 * np.abs(np.cos(x))

ax.plot(x, y_mean, linewidth=1.5, label='Model')
ax.fill_between(x, y_mean - y_std, y_mean + y_std, alpha=0.2)

ax.set_xlabel('Time (s)')
ax.set_ylabel('Amplitude (V)')
ax.legend(frameon=False)
fig.savefig('line_errorband.pdf', dpi=300, bbox_inches='tight')
```

---

## 2. Grouped bar chart with individual points

Best for: **categorical group comparisons with small n**.

```python
import seaborn as sns

fig, ax = plt.subplots(figsize=(3.5, 2.8))

sns.barplot(data=df, x='condition', y='score', hue='method',
            ci='sd', capsize=0.05, alpha=0.6, ax=ax)
sns.stripplot(data=df, x='condition', y='score', hue='method',
              dodge=True, size=3, alpha=0.7, jitter=0.1, ax=ax,
              legend=False)

ax.set_xlabel('')
ax.set_ylabel('Accuracy (%)')
ax.legend(title='', frameon=False, loc='upper right')
fig.savefig('grouped_bar.pdf', dpi=300, bbox_inches='tight')
```

---

## 3. Violin + strip (distribution comparison)

Best for: **showing full distribution shape across groups**.

```python
import seaborn as sns

fig, ax = plt.subplots(figsize=(3.5, 2.8))

sns.violinplot(data=df, x='group', y='value', inner=None,
               palette='colorblind', alpha=0.3, cut=0, ax=ax)
sns.stripplot(data=df, x='group', y='value', size=2.5,
              palette='colorblind', alpha=0.7, jitter=0.15, ax=ax)

# Overlay mean ± SEM
for i, g in enumerate(df['group'].unique()):
    vals = df.loc[df['group'] == g, 'value']
    ax.errorbar(i, vals.mean(), yerr=vals.sem(), fmt='_k',
                markersize=10, linewidth=1.5, capsize=4)

ax.set_ylabel('Expression level (RPKM)')
ax.set_xlabel('')
fig.savefig('violin_strip.pdf', dpi=300, bbox_inches='tight')
```

---

## 4. Annotated heatmap

Best for: **correlation matrices, confusion matrices, gene expression**.

```python
import seaborn as sns

fig, ax = plt.subplots(figsize=(4, 3.5))

sns.heatmap(corr_matrix, annot=True, fmt='.2f', cmap='RdBu_r',
            center=0, vmin=-1, vmax=1, square=True,
            linewidths=0.5, linecolor='white',
            cbar_kws={'shrink': 0.8, 'label': "Pearson r"},
            ax=ax)

ax.set_xticklabels(ax.get_xticklabels(), rotation=45, ha='right')
fig.savefig('heatmap.pdf', dpi=300, bbox_inches='tight')
```

---

## 5. Multi-panel comparison (2×2)

Best for: **results figures comparing methods/conditions/ablations**.

```python
from string import ascii_lowercase

fig, axes = plt.subplots(2, 2, figsize=(7, 5))
axes = axes.flatten()

for i, (ax, data) in enumerate(zip(axes, datasets)):
    ax.plot(data['x'], data['y'], linewidth=1.2)
    ax.set_xlabel('X label')
    ax.set_ylabel('Y label')
    ax.set_title(f'Condition {i+1}', fontsize=9)
    # Panel label: (a), (b), (c), (d)
    ax.text(-0.15, 1.05, f'({ascii_lowercase[i]})',
            transform=ax.transAxes, fontsize=10, fontweight='bold', va='top')

fig.tight_layout()
fig.savefig('multi_panel.pdf', dpi=300, bbox_inches='tight')
```

---

## 6. 3D surface plot

Best for: **showing landscape/function over 2D domain (use sparingly)**.

> [!WARNING]
> 3D plots lose precision due to perspective projection. Use only when the 3D
> shape itself is the message. For quantitative comparison, prefer 2D heatmaps
> or contour plots.

```python
from mpl_toolkits.mplot3d import Axes3D

fig = plt.figure(figsize=(4, 3.5))
ax = fig.add_subplot(111, projection='3d')

X, Y = np.meshgrid(np.linspace(-3, 3, 50), np.linspace(-3, 3, 50))
Z = np.sin(X) * np.cos(Y)

surf = ax.plot_surface(X, Y, Z, cmap='viridis', alpha=0.9,
                       edgecolor='none', antialiased=True)
fig.colorbar(surf, shrink=0.6, pad=0.1, label='Z value')

ax.set_xlabel('X')
ax.set_ylabel('Y')
ax.set_zlabel('Z')
ax.view_init(elev=25, azim=-60)

fig.savefig('surface_3d.pdf', dpi=300, bbox_inches='tight')
```

### 3D constraints

- Always provide a 2D alternative (contour or heatmap) alongside
- Set `view_init` explicitly for reproducible angle
- Avoid 3D bar charts — they distort magnitude perception
- Keep surfaces smooth; avoid wireframe when colors encode value
- Label all three axes with units

---

## 7. Raincloud Plot (Box + Violin + Raw Data)

Best for: **showing data distribution, summary stats, and raw points simultaneously**.

```python
import seaborn as sns
import matplotlib.pyplot as plt

fig, ax = plt.subplots(figsize=(5, 4))

# Use ptitprince or manual half-violin
# For manual:
sns.violinplot(data=df, x='group', y='value', split=True, inner=None, 
               alpha=0.3, ax=ax)
# Offset boxplot
sns.boxplot(data=df, x='group', y='value', width=0.15, 
            boxprops={'zorder': 2}, ax=ax)
# Rain: jittered points below
sns.stripplot(data=df, x='group', y='value', alpha=0.5, size=3, 
              jitter=0.05, ax=ax)

ax.set_title("Raincloud Plot")
fig.savefig('raincloud.pdf', bbox_inches='tight')
```

---

## 8. Ridge Plot (Joyplot)

Best for: **comparing many distributions over a continuous scale (e.g., time, temperature)**.

```python
import seaborn as sns

# Initialize the FacetGrid object
pal = sns.cubehelix_palette(10, rot=-.25, light=.7)
g = sns.FacetGrid(df, row="group", hue="group", aspect=15, height=.5, palette=pal)

# Draw the densities
g.map(sns.kdeplot, "value", bw_adjust=.5, clip_on=False, fill=True, alpha=1, linewidth=1.5)
g.map(sns.kdeplot, "value", clip_on=False, color="w", lw=2, bw_adjust=.5)

# Add a horizontal line
g.map(plt.axhline, y=0, lw=2, clip_on=False)

# Better overlap
g.fig.subplots_adjust(hspace=-.25)
g.set_titles("")
g.set(yticks=[], ylabel="")
g.despine(bottom=True, left=True)
g.savefig('ridge_plot.pdf', bbox_inches='tight')
```

---

## 9. Hierarchical Heatmap (Clustermap)

Best for: **showing clusters in large matrices (e.g., gene expression)**.

```python
import seaborn as sns

# Standardize/Normalize data if needed before clustering
g = sns.clustermap(df_matrix, 
                   cmap="RdBu_r", 
                   center=0,
                   metric="euclidean", 
                   method="ward",
                   linewidths=.5, 
                   figsize=(7, 7),
                   cbar_kws={'label': 'Z-score'})

# Rotate labels
plt.setp(g.ax_heatmap.get_xticklabels(), rotation=45, ha='right')
g.savefig('clustermap.pdf', bbox_inches='tight')
```
