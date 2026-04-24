# HTML Slide Design System

## Default Visual Direction

The default direction for `source-slide-formats` HTML export is a dark editorial deck with clean
information hierarchy:

- **Canvas**: dark background (`#0a0a0a` to `#1a1a2e`) as the base language
- **Text**: high-contrast white (`#f5f5f5`) for headings, muted light (`#b0b0b0`) for body
- **Accent**: one primary accent color (default: electric blue `#4a9eff`) for highlights,
  links, and metric emphasis
- **Cards**: translucent dark panels (`rgba(255,255,255,0.05)` to `rgba(255,255,255,0.08)`)
  with subtle borders
- **Emphasis**: warm gold (`#f0c040`) reserved for key takeaways and important callouts only

## Typography

Use system font stacks or embed Google Fonts via `@import`:

- **Headings**: `'Inter', 'SF Pro Display', system-ui, sans-serif` — bold, tight letter-spacing
- **Body**: `'Inter', 'SF Pro Text', system-ui, sans-serif` — regular weight, generous line-height
- **Code**: `'JetBrains Mono', 'SF Mono', 'Fira Code', monospace`
- **Chinese**: `'Noto Sans SC', 'PingFang SC', 'Microsoft YaHei', sans-serif`

### Size Scale (1920×1080 canvas)

| Role | Size | Weight |
|------|------|--------|
| Slide title | 48–64px | 700 |
| Section subtitle | 28–36px | 600 |
| Body text | 20–24px | 400 |
| Card heading | 22–28px | 600 |
| Caption / note | 14–16px | 400 |
| Metric number | 48–72px | 700 |

> **Hard rule**: body copy must never drop below 18px on a 1920×1080 canvas.

## Layout Patterns

### Cover Slide
- Full-bleed background image (blurred or with dark overlay)
- Centered title card with strong contrast protection
- Compact metadata block below title (name, date, affiliation)

### Content Slide — Two Column
```css
.slide-content {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 40px;
  padding: 80px;
}
```

### Content Slide — Metric Strip
- Top: title bar
- Middle: 3–4 metric cards in a row
- Bottom: one-line takeaway or source note

### Content Slide — Image + Analysis
- Left: contained image with caption
- Right: bullet analysis or key findings

### Closing Slide
- Mirror cover slide visual language
- "Thank You" or "Q&A" with contact info

## Color Roles

Define CSS custom properties at the root:

```css
:root {
  --color-bg: #0a0a0a;
  --color-surface: rgba(255, 255, 255, 0.05);
  --color-border: rgba(255, 255, 255, 0.1);
  --color-text-primary: #f5f5f5;
  --color-text-secondary: #b0b0b0;
  --color-accent: #4a9eff;
  --color-emphasis: #f0c040;
  --color-success: #4ade80;
  --color-warning: #fbbf24;
  --color-danger: #f87171;
}
```

## Spacing System

Use an 8px grid:
- Slide padding: 80px (10 units)
- Section gap: 40px (5 units)
- Card padding: 24–32px (3–4 units)
- Element gap: 16px (2 units)

## Card / Panel Styling

```css
.card {
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  padding: 24px;
}

.card-emphasis {
  background: rgba(74, 158, 255, 0.08);
  border-color: rgba(74, 158, 255, 0.2);
}
```

## Alternative Visual Directions

When the user requests a non-dark theme:

### Light Academic
- White canvas, navy headings, pale blue panels
- Conservative typography, clear hierarchy
- Minimal decoration, content-first

### Corporate
- Brand-colored header bar, white body
- Structured grid layouts, metric dashboards
- Professional photography with overlays

### Minimal
- Maximum whitespace, large type
- Black text on white, one accent color
- No cards or boxes — pure typography hierarchy

## Anti-Patterns to Avoid

- ❌ Equal-height empty boxes that waste space
- ❌ Generic gradient backgrounds without purpose
- ❌ Text smaller than 18px for body content
- ❌ Raw unprocessed images without overlay or crop
- ❌ Inconsistent spacing between similar elements
- ❌ Multiple competing accent colors
- ❌ Card-heavy layouts where all cards shout equally
