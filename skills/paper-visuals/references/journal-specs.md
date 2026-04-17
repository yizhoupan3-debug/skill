# Journal Figure Specifications Reference

## Common Publication Requirements

### Resolution & DPI

| Venue Type | Minimum DPI | Recommended |
|-----------|-------------|-------------|
| Print journals (Nature, Science, IEEE) | 300 DPI | 600 DPI for line art |
| Conference proceedings (NeurIPS, ICML) | 300 DPI | 300-600 DPI |
| Online-only journals | 150 DPI | 300 DPI |

### File Formats

| Format | Best for | Notes |
|--------|----------|-------|
| **PDF** | Vector graphics, plots | Preferred for most submissions |
| **EPS** | Legacy vector format | Some older journals still require |
| **TIFF** | High-quality raster | Lossless, large file size |
| **PNG** | Screen-quality raster | Good for screenshots, diagrams |
| **SVG** | Web publications | Scalable, editable |

> [!CAUTION]
> Never submit JPEG for line art or plots — compression artifacts are visible and unprofessional.

### Color Spaces

| Venue Type | Color Space | Notes |
|-----------|------------|-------|
| Print journals | **CMYK** required | Convert from RGB before submission |
| Online-only | **RGB** preferred | sRGB is safest |
| Mixed (print + online) | Submit both | Check venue guidelines |

### Font Embedding

- **Always embed fonts** in PDF/EPS figures
- Use standard fonts: Helvetica, Arial, Times New Roman, Computer Modern
- Minimum font size: **6pt** at final print size (8pt recommended)
- matplotlib: `plt.rcParams['pdf.fonttype'] = 42` to embed TrueType

---

## Venue-Specific Guidelines

### Nature / Nature family
- Max width: **89mm** (single column), **183mm** (double column)
- Resolution: 300 DPI min, 600 DPI for line art
- Color: RGB for online, CMYK for print
- Font: Arial or Helvetica, 5-7pt
- File: EPS, PDF, or TIFF

### IEEE Transactions
- Max width: **3.5in** (single column), **7.16in** (double column)
- Resolution: 300 DPI for color, 600 DPI for line art
- Font: Times New Roman or Helvetica, 8pt minimum
- File: EPS or PDF preferred

### ACM (SIGCHI, SIGMOD, etc.)
- Max width: follows template column width
- Resolution: 300 DPI minimum
- Color: RGB
- Vector format preferred (PDF)
- Must be accessible (alt text in final version)

### NeurIPS / ICML / ICLR
- Width follows template (usually 5.5in for main text)
- PDF vector strongly preferred
- Color: RGB
- No strict DPI requirement for vector, 300 DPI for raster
- **Colorblind accessibility strongly encouraged**

### Science / Science family
- Max width: **55mm** (single column), **175mm** (double column)
- Resolution: 300 DPI min, 600 DPI for line art
- Color: RGB for online, CMYK for print
- Font: Helvetica or Arial, 6-8pt
- File: EPS, PDF, or TIFF (no JPEG)
- Panel labels: **uppercase bold** (A, B, C)
- rcparams preset: `"science"`

### Cell / Cell Press
- Max width: **85mm** (single column), **178mm** (double column)
- Resolution: 300 DPI min for halftones, 1000 DPI for line art
- Color: RGB
- Font: Helvetica or Arial, 5-7pt at final print size
- File: PDF, EPS, or TIFF
- Panel labels: **uppercase bold** (A, B, C)
- rcparams preset: `"cell"`

### PLOS ONE / PLOS family
- Max width: **132mm** (single column), **173mm** (double column)
- Resolution: 300 DPI minimum
- Color: RGB
- Font: Arial, 8-12pt
- File: TIFF or EPS (vector preferred for plots)
- **Strict TIFF requirement** for final submission raster figures
- rcparams preset: `"plos"`

### Elsevier journals (generic)
- Max width: **90mm** (single column), **190mm** (double column)
- Resolution: 300 DPI for color, 500 DPI for line art, 1000 DPI for combinations
- Color: RGB for online; some journals require CMYK for print
- Font: Arial, Helvetica, or Times New Roman, 6-8pt
- File: TIFF, EPS, or PDF
- Figures must be submitted as **separate files** (not embedded in manuscript)

## Universal Quality Checklist

| # | Check | Pass Criteria |
|---|-------|---------------|
| V1 | Resolution | Meets venue minimum DPI |
| V2 | Font embedding | All fonts embedded, no substitution |
| V3 | Font size | ≥6pt at final print size |
| V4 | Color space | Matches venue requirement (RGB/CMYK) |
| V5 | File format | Matches venue accepted formats |
| V6 | Figure width | Fits within column / page width at venue scale |
| V7 | Text readability | All labels, legends, axes readable at reduced size |
| V8 | Colorblind safety | Distinguishable without color (pattern, marker shape) |
| V9 | White space | No excessive margins inside figure bounding box |
| V10 | Compression | No JPEG artifacts on line art |
| V11 | Consistency | Same style across all figures in the paper |
| V12 | Caption | Self-contained, describes what is shown and key finding |

---

## Colorblind-Safe Palettes

### Recommended palettes
- **okabe-ito**: 8-color universal palette (default recommendation)
- **viridis / plasma / inferno**: Perceptually uniform sequential colormaps
- **tab10**: Good default but check with colorblind simulator
- **SciencePlots ieee/nature styles**: Include accessible defaults

### Verification tools
- [Coblis Color Blindness Simulator](https://www.color-blindness.com/coblis-color-blindness-simulator/)
- matplotlib: `from matplotlib import colormaps; colormaps['viridis']`
- Check with deuteranopia (most common) and protanopia simulations
