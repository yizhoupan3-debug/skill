# Layout Patterns

Use this reference when selecting or auto-assigning slide layouts based on content structure.

## Standard Patterns

### `cover`
**Trigger**: first slide or `type: "cover"` in outline.
**Structure**: blurred full-slide background + dark overlay + title stack (title, subtitle, metadata) on left, optional focus image on right.
**Slot needs**: title (required), subtitle, presenter, date, coverImage.
**Design intent**: create one memorable entry point, with title as the clear hero.
**Quality gate**: focal image, protection layer, and title hierarchy are obvious; no busy image under unprotected text.

### `hero-image`
**Trigger**: `hasImage: true && bulletCount <= 2`.
**Structure**: dominant image (70%) + short text overlay or side panel (30%).
**Slot needs**: image (required), title, caption.
**Design intent**: let the image carry the emotional or evidentiary load.
**Quality gate**: crop has a focal point; caption is attached to the image, not floating as decoration.

### `image-text-split`
**Trigger**: `hasImage: true && textLength > 100`.
**Structure**: left image panel (5.5 in) + right text panel (6.5 in), or mirrored.
**Slot needs**: image, title, bullets or body text.
**Design intent**: pair one visual anchor with a short explanation path.
**Quality gate**: image and text have unequal but intentional weight; body copy remains readable.

### `multi-card`
**Trigger**: `bulletCount >= 3 && bulletCount <= 6 && !hasImage`.
**Structure**: 2-4 dark panels arranged in grid, each with a number/icon + title + description.
**Slot needs**: title, items array [{title, description, value?}].
**Design intent**: break a concept into a few comparable parts without flattening hierarchy.
**Quality gate**: avoid equal-weight card farms; one card, number, or takeaway should lead the eye.

### `data-panel`
**Trigger**: `dataPoints >= 3`.
**Structure**: metric chip row at top + wide evidence panel below with chart or table.
**Slot needs**: title, metrics [{value, label}], chart or table data.
**Design intent**: make the comparison clear before showing detail.
**Quality gate**: chart/table is restyled; no default Office palette, clutter, or unreadable axis labels.

### `comparison`
**Trigger**: `hasComparison: true` or exactly 2 top-level groups.
**Structure**: two side-by-side panels with matching internal structure.
**Slot needs**: title, left {title, items}, right {title, items}, optional images.
**Design intent**: make one tradeoff visible at a glance.
**Quality gate**: both sides align, but the conclusion or preferred option is visually signaled.

### `timeline`
**Trigger**: `hasTimeline: true` or items with date/year fields.
**Structure**: horizontal timeline bar with event cards above/below.
**Slot needs**: title, events [{date, title, description}].
**Design intent**: show change over time, not just a decorated list.
**Quality gate**: dates are legible, event density is controlled, and the visual rhythm follows chronology.

### `process-flow`
**Trigger**: `hasSteps: true` or sequential numbered items.
**Structure**: connected nodes left-to-right with arrows.
**Slot needs**: title, steps [{title, description}].
**Design intent**: describe action order and handoffs.
**Quality gate**: each step starts with an action; arrows clarify sequence rather than adding decoration.

### `full-text`
**Trigger**: `textLength > 300 && !hasImage && dataPoints == 0`.
**Structure**: section title + 2-3 dark text panels with structured content.
**Slot needs**: title, body text or structured bullets.
**Design intent**: preserve a dense argument while still creating scan points.
**Quality gate**: copy is pruned before font size is reduced; no wall-of-text panel.

### `closing`
**Trigger**: last slide or `type: "closing"` in outline.
**Structure**: blurred background (same as cover) + thank you / Q&A text + optional contact info.
**Slot needs**: closingText, optional contact.
**Design intent**: echo the cover and leave one final thought.
**Quality gate**: closing feels intentional, not a leftover title slide or unrelated template.

## Auto-Selection Rules

- `type: "cover"` -> `cover`
- `type: "closing"` -> `closing`
- `timeline` items present -> `timeline`
- `steps` items present -> `process-flow`
- `comparison` present -> `comparison`
- `metrics` or chart-like data present -> `data-panel`
- `image` present with 1-2 bullets -> `hero-image`
- `image` present with longer text -> `image-text-split`
- 3-6 bullets with no dominant image -> `multi-card`
- dense prose with no data/image -> `full-text`

When two rules match, choose the pattern that creates the clearest reading path,
not the most decorative page.

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
