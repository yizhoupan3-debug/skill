# Statistical Annotations for Scientific Figures

## Error bar selection guide

| Statistic | Shows | When to use | Caption text |
|-----------|-------|-------------|-------------|
| **SD** (Standard Deviation) | Data spread | Describing variability of measurements | "Error bars represent ±1 SD" |
| **SEM** (Standard Error of Mean) | Precision of mean estimate | Comparing group means | "Error bars represent ±1 SEM" |
| **95% CI** (Confidence Interval) | Range of plausible mean values | Formal inference on means | "Error bars represent 95% CI" |

> [!IMPORTANT]
> Always state which error bar type is used in the figure caption. Never leave error bars unexplained.

### Implementation

```python
import numpy as np
import matplotlib.pyplot as plt

means = [group.mean() for group in groups]
sds   = [group.std() for group in groups]
sems  = [group.std() / np.sqrt(len(group)) for group in groups]

# 95% CI (assuming normal distribution)
from scipy import stats
cis = [stats.sem(group) * stats.t.ppf(0.975, len(group)-1) for group in groups]

# Plot with error bars
fig, ax = plt.subplots()
ax.bar(x, means, yerr=sems, capsize=4, error_kw={'linewidth': 1})
```

---

## Significance markers

### Convention

| Symbol | Meaning |
|--------|---------|
| ns | p > 0.05 |
| \* | p ≤ 0.05 |
| \*\* | p ≤ 0.01 |
| \*\*\* | p ≤ 0.001 |
| \*\*\*\* | p ≤ 0.0001 |

### Using `statannotations` library

```bash
pip install statannotations
# or
uv pip install statannotations
```

```python
import seaborn as sns
from statannotations.Annotator import Annotator

# Create base plot
ax = sns.barplot(data=df, x='group', y='value')

# Define comparison pairs
pairs = [('Control', 'Treatment1'), ('Control', 'Treatment2')]

# Add statistical annotations
annotator = Annotator(ax, pairs, data=df, x='group', y='value')
annotator.configure(test='Mann-Whitney', text_format='star', loc='inside')
annotator.apply_and_annotate()
```

### Manual significance brackets

```python
def add_significance_bracket(ax, x1, x2, y, p_value, dh=0.02, barh=0.02):
    """
    Draw a significance bracket between two x positions.

    Args:
        ax: matplotlib Axes
        x1, x2: x positions of the two groups
        y: y position of the bracket (data coordinates)
        p_value: p-value for star annotation
        dh: offset above data for the bracket
        barh: height of the bracket ends
    """
    stars = 'ns' if p_value > 0.05 else '*' * min(4, sum([
        p_value <= 0.05, p_value <= 0.01, p_value <= 0.001, p_value <= 0.0001
    ]))

    y_bracket = y + dh
    ax.plot([x1, x1, x2, x2], [y_bracket - barh, y_bracket, y_bracket, y_bracket - barh],
            lw=1, c='black')
    ax.text((x1 + x2) / 2, y_bracket, stars, ha='center', va='bottom', fontsize=8)
```

---

## Individual data points overlay

Always show individual data points when **n < 30** to avoid hiding the data distribution behind summary statistics.

```python
# Scatter + bar (classic approach)
ax.bar(x, means, yerr=sems, capsize=4, alpha=0.6, color='lightgray', edgecolor='black')
ax.scatter(x_jittered, individual_values, s=15, alpha=0.7, zorder=3, color='#0072B2')

# Seaborn: strip + bar overlay
sns.barplot(data=df, x='group', y='value', ci='sd', alpha=0.4, ax=ax)
sns.stripplot(data=df, x='group', y='value', size=4, alpha=0.6, jitter=0.2, ax=ax)

# Better alternative: violin + strip (no bar)
sns.violinplot(data=df, x='group', y='value', inner=None, alpha=0.3, ax=ax)
sns.stripplot(data=df, x='group', y='value', size=3, alpha=0.7, jitter=0.15, ax=ax)
```

---

---

## Sample size (n) annotation

Always report n. Options:

### Option 1: In x-axis labels
```python
ax.set_xticklabels([f'{name}\n(n={n})' for name, n in zip(group_names, sample_sizes)])
```

### Option 2: Below bars
```python
for i, (x_pos, n) in enumerate(zip(x, sample_sizes)):
    ax.text(x_pos, 0, f'n={n}', ha='center', va='top', fontsize=7, color='gray')
```

### Option 3: In the caption (preferred for clean figures)
Example text: "Data are presented as mean ± SEM. Control (n=24), Treatment (n=18)."

---

## 4. Advanced Statistical Automation with Pingouin

[Pingouin](https://pingouin-stats.org/) is a statistical package that returns familiar Pandas DataFrames, making it ideal for automated plotting.

### 1. Automated T-test with Bracket

```python
import pingouin as pg

# Perform T-test
res = pg.ttest(df[df['group'] == 'A']['value'], 
               df[df['group'] == 'B']['value'])
p_val = res['p-val'].values[0]

# Plot and annotate
ax = sns.barplot(data=df, x='group', y='value')
add_significance_bracket(ax, 0, 1, y=df['value'].max(), p_value=p_val)
```

### 2. ANOVA + Post-hoc with Multi-brackets

```python
# 1-way ANOVA
aov = pg.anova(data=df, dv='value', between='group')
print(aov) # Check F and p-val

# Post-hoc tests (Tukey)
posthoc = pg.pairwise_tukey(data=df, dv='value', between='group')

# Filter significant pairs (p < 0.05)
sig_pairs = posthoc[posthoc['p-tukey'] < 0.05]

# Plot and auto-annotate
ax = sns.boxplot(data=df, x='group', y='value')
y_max = df['value'].max()
for i, row in sig_pairs.iterrows():
    # Get indices for groups (assuming groups_list is defined)
    idx1 = groups_list.index(row['A'])
    idx2 = groups_list.index(row['B'])
    # Increment y to avoid bracket overlap
    y_bracket = y_max * (1.1 + i * 0.08)
    add_significance_bracket(ax, idx1, idx2, y=y_bracket, p_value=row['p-tukey'])
```

---

## Best Practices for Statistical Figures

1.  **Exact P-values**: For 0.001 < p < 0.05, consider reporting the exact p-value instead of just stars (`p = 0.024` > `*`).
2.  **Effect Size**: Report effect sizes (Cohen's d, Hedge's g, or η²) in the caption or as annotations for key findings.
3.  **Assumptions Check**: Briefly mention that normality and variance homogeneity were checked (e.g., via Shapiro-Wilk and Levene's tests in Pingouin).
4.  **Transparency**: State the exact test used (e.g., "Two-sided Welch’s t-test") in the figure caption.

---

## 5. Complete example: publication-grade comparison figure

```python
import matplotlib.pyplot as plt
import seaborn as sns
import numpy as np

fig, ax = plt.subplots(figsize=(3.5, 3))

# Violin + strip + significance
sns.violinplot(data=df, x='group', y='value', inner=None,
               palette='colorblind', alpha=0.3, ax=ax)
sns.stripplot(data=df, x='group', y='value', size=3,
              palette='colorblind', alpha=0.7, jitter=0.15, ax=ax)

# Add mean + SEM
for i, group in enumerate(df['group'].unique()):
    vals = df[df['group'] == group]['value']
    ax.errorbar(i, vals.mean(), yerr=vals.sem(), fmt='_', color='black',
                markersize=10, linewidth=1.5, capsize=4)

# Significance bracket
add_significance_bracket(ax, 0, 1, y=df['value'].max(), p_value=0.003)

ax.set_ylabel('Measurement (units)')
ax.set_xlabel('')
fig.savefig('comparison.pdf', dpi=300, bbox_inches='tight')
```
