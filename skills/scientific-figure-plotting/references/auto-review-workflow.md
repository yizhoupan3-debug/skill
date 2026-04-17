# Auto-Review Workflow: Plot → Export → Visual QA

Standard workflow for closing the loop between code-generated figures and
visual quality assurance.

## Workflow

```
1. Generate figure via plotting code
       ↓
2. Export as PNG (≥300 DPI) to tmp/
       ↓
3. View exported image (view_file)
       ↓
4. Run visual-review checklist
       ↓
5a. Issues found → fix code → go to 1
5b. All clear → save final artifact
```

## Step-by-step

### 1. Generate and export

```python
import matplotlib.pyplot as plt

# ... your plotting code ...

# Export for review (always PNG, high enough DPI to see details)
fig.savefig('tmp/figure_review.png', dpi=300, bbox_inches='tight')
plt.close(fig)
```

### 2. Visual review checklist

After viewing the exported image, check every item:

#### Layout & readability
- [ ] All text legible at target paper size (≥6 pt after scaling)
- [ ] All lines visible (≥0.5 pt after scaling)
- [ ] Axis labels present with units
- [ ] Title/panel labels if needed

#### Overlap scan
- [ ] Tick labels not colliding
- [ ] Axis titles not crowding ticks
- [ ] Legend not covering data
- [ ] Annotations not crossing important marks
- [ ] Panel titles/captions not colliding

#### Information density
- [ ] Every visual element earns its place
- [ ] No redundant encodings
- [ ] White space is intentional

#### Color & accessibility
- [ ] Palette is colorblind-safe
- [ ] Grayscale fallback works (if required)
- [ ] Colors are distinguishable in context

#### Export quality
- [ ] DPI meets target (≥300 for raster, vector preferred)
- [ ] Dimensions match journal column width
- [ ] Fonts are embedded (PDF/EPS)
- [ ] No compression artifacts

### 3. Verdict

After review, assign one of:

| Verdict | Meaning | Action |
|---------|---------|--------|
| `PASS` | Meets publication bar | Save final artifact |
| `MINOR` | Small fixable issues | Fix and re-export (no re-review needed) |
| `MAJOR` | Structural problems | Fix code, full re-review |
| `REDESIGN` | Wrong chart type or encoding | Rethink approach |

### 4. Final export

```python
# Final artifact — use vector for papers
fig.savefig('output/figure_1.pdf', dpi=300, bbox_inches='tight')
fig.savefig('output/figure_1.png', dpi=600, bbox_inches='tight')  # raster backup
```

## Integration with $visual-review

When `$visual-review` is available as a cross-cutting skill:

1. Export the figure to a viewable format (PNG)
2. Invoke `$visual-review` with the exported image
3. The visual-review skill will perform its structured inspection passes:
   - Global scan → Text scan → Structure scan → Anomaly scan
4. Use its verdict labels (`confirmed`, `likely`, `not found`, `indeterminate`)
5. Feed findings back into plotting code fixes

## Multi-figure batch workflow

For papers with multiple figures:

```python
figures = {
    'fig1_results': create_results_figure,
    'fig2_ablation': create_ablation_figure,
    'fig3_comparison': create_comparison_figure,
}

for name, create_fn in figures.items():
    fig = create_fn()
    fig.savefig(f'tmp/{name}_review.png', dpi=300, bbox_inches='tight')
    plt.close(fig)

# Review all exported images sequentially
# Fix any issues, then final export
for name, create_fn in figures.items():
    fig = create_fn()
    fig.savefig(f'output/{name}.pdf', dpi=300, bbox_inches='tight')
    fig.savefig(f'output/{name}.png', dpi=600, bbox_inches='tight')
    plt.close(fig)
```
