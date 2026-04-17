# Layout Patterns

Use this reference when selecting or auto-assigning slide layouts based on content structure.

## Standard Patterns

### `cover`
**Trigger**: first slide or `type: "cover"` in outline.
**Structure**: blurred full-slide background + dark overlay + title stack (title, subtitle, metadata) on left, optional focus image on right.
**Slot needs**: title (required), subtitle, presenter, date, coverImage.

### `hero-image`
**Trigger**: `hasImage: true && bulletCount <= 2`.
**Structure**: dominant image (70%) + short text overlay or side panel (30%).
**Slot needs**: image (required), title, caption.

### `image-text-split`
**Trigger**: `hasImage: true && textLength > 100`.
**Structure**: left image panel (5.5 in) + right text panel (6.5 in), or mirrored.
**Slot needs**: image, title, bullets or body text.

### `multi-card`
**Trigger**: `bulletCount >= 3 && bulletCount <= 6 && !hasImage`.
**Structure**: 2-4 dark panels arranged in grid, each with a number/icon + title + description.
**Slot needs**: title, items array [{title, description, value?}].

### `data-panel`
**Trigger**: `dataPoints >= 3`.
**Structure**: metric chip row at top + wide evidence panel below with chart or table.
**Slot needs**: title, metrics [{value, label}], chart or table data.

### `comparison`
**Trigger**: `hasComparison: true` or exactly 2 top-level groups.
**Structure**: two side-by-side panels with matching internal structure.
**Slot needs**: title, left {title, items}, right {title, items}, optional images.

### `timeline`
**Trigger**: `hasTimeline: true` or items with date/year fields.
**Structure**: horizontal timeline bar with event cards above/below.
**Slot needs**: title, events [{date, title, description}].

### `process-flow`
**Trigger**: `hasSteps: true` or sequential numbered items.
**Structure**: connected nodes left-to-right with arrows.
**Slot needs**: title, steps [{title, description}].

### `full-text`
**Trigger**: `textLength > 300 && !hasImage && dataPoints == 0`.
**Structure**: section title + 2-3 dark text panels with structured content.
**Slot needs**: title, body text or structured bullets.

### `closing`
**Trigger**: last slide or `type: "closing"` in outline.
**Structure**: blurred background (same as cover) + thank you / Q&A text + optional contact info.
**Slot needs**: closingText, optional contact.

## Auto-Selection Algorithm

```
function selectPattern(slideContent) {
  if (slideContent.type === "cover")      return "cover";
  if (slideContent.type === "closing")    return "closing";
  if (slideContent.hasTimeline)           return "timeline";
  if (slideContent.hasSteps)              return "process-flow";
  if (slideContent.hasComparison)         return "comparison";
  if (slideContent.dataPoints >= 3)       return "data-panel";
  if (slideContent.hasImage && slideContent.bulletCount <= 2) return "hero-image";
  if (slideContent.hasImage)              return "image-text-split";
  if (slideContent.bulletCount >= 3)      return "multi-card";
  return "full-text";
}
```

## Coordinate Quick Reference (16:9, inches)

| Zone | x | y | w | h |
|------|---|---|---|---|
| Full slide | 0 | 0 | 13.333 | 7.5 |
| Content area | 0.92 | 0.96 | 11.5 | 5.62 |
| Top label | 0.9 | 0.38 | 2.0 | 0.12 |
| Section title | 0.92 | 0.96 | 5.0 | 0.24 |
| Bottom glow line | 0.86 | 6.86 | 11.6 | 0.018 |
| Source note | 0.92 | 6.98 | 8.0 | 0.16 |
| Page number | 12.2 | 7.03 | 0.4 | 0.12 |
| Left panel (split) | 0.94 | 1.9 | 5.48 | 4.4 |
| Right panel (split) | 6.72 | 1.9 | 5.48 | 4.4 |
| Metric chip row | 0.94 | 2.3 | ~2.0 each | 0.94 |
| Wide evidence band | 0.94 | 3.56 | 11.42 | 2.24 |
