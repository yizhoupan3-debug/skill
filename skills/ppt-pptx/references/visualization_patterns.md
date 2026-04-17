# Visualization Patterns

Use this reference when choosing how to visualize data, processes, or relationships in a slide deck.

## Pattern Catalog

### 1. Timeline

**When**: chronological events, project milestones, historical progression.

**Structure**: horizontal or vertical axis with event markers stacked along it.

**Layout guide**:
- 4-8 events per slide max
- Use `addDarkPanel()` for event cards along a horizontal glow line
- Date/year labels at top; event description below
- Color-code phases with palette accent

**Helpers**: `addMetricChip()` for date labels, `addDarkPanel()` for event cards

---

### 2. Process Flow

**When**: step-by-step procedure, workflow, pipeline.

**Structure**: left-to-right or top-to-bottom connected nodes.

**Layout guide**:
- 3-6 steps per slide
- Use arrow shapes between nodes or a Mermaid flowchart
- Each node: icon/number + short label + 1-line description
- Keep spacing even with `distributeSlideElements()`

**Helpers**: `addMermaidDiagram()` for auto flowcharts, `addDarkPanel()` for manual nodes

---

### 3. Comparison (A vs B)

**When**: two alternatives, before/after, pros/cons.

**Structure**: two side-by-side panels with matching vertical structure.

**Layout guide**:
- Left panel (6.2 in wide) vs right panel (6.2 in wide) with 0.2 in gap
- Shared title row at top
- Matching bullet count and visual weight on both sides
- Use contrasting accent colors for each side

**Helpers**: `addDarkPanel()` for each side, `addImageCard()` for visual evidence

---

### 4. Data Dashboard

**When**: multiple KPIs, metrics, summary statistics.

**Structure**: metric chips across top + one wide evidence surface below.

**Layout guide**:
- 3-5 metric chips in a row (use `addMetricChip()`)
- One wide panel below with chart or detailed breakdown
- Use `addStyledChart()` for the main chart
- Title + one-sentence insight above the chart

**Helpers**: `addMetricChip()`, `addStyledChart()`, `addDarkPanel()`

---

### 5. Hierarchy / Pyramid

**When**: importance ranking, organizational structure, abstraction levels.

**Structure**: stacked trapezoids or tiered blocks, widening from top to bottom.

**Layout guide**:
- 3-4 levels max
- Top level: smallest + brightest accent
- Bottom level: widest + most muted
- Label each level with a one-word category + short description

**Helpers**: `addDarkPanel()` with varying widths, manual coordinate calculation

---

### 6. Matrix / Grid

**When**: 2×2 analysis, feature comparison table, multi-dimensional categorization.

**Structure**: equal quadrants or structured grid cells.

**Layout guide**:
- For 2×2: four equal `addDarkPanel()` with axis labels
- For tables: native PowerPoint table (restyle from defaults)
- Highlight the decision-relevant cell with accent color
- Keep text minimal; move detail to backup slides

**Helpers**: `slide.addTable()` with custom styles

---

### 7. Image + Insight

**When**: visual evidence paired with a key takeaway.

**Structure**: dominant image (60%) + text panel (40%), or image band + overlay text.

**Layout guide**:
- Image side: `addImageCard()` with overlay protection
- Text side: title + 2-3 bullet insights + optional source note
- Image should be intentionally cropped to focal point
- Text must be readable without squinting

**Helpers**: `addImageCard()`, `imageSizingCrop()`, `addDarkPanel()`

---

### 8. Evidence Board

**When**: multiple pieces of supporting evidence, mixed media types.

**Structure**: dark evidence surface with embedded cards, images, and stats.

**Layout guide**:
- One wide `addDarkPanel()` as background surface
- 2-3 embedded elements: image card + stat chips + text conclusion
- Layered composition, not a flat grid
- Use Section title + English subtitle at top

**Helpers**: `addDarkPanel()`, `addImageCard()`, `addMetricChip()`

---

## Auto-Selection Rules

When choosing a pattern automatically, use these heuristics:

| Signal | Recommended Pattern |
|--------|-------------------|
| `hasTimeline: true` | Timeline |
| `hasSteps: true` | Process Flow |
| `hasComparison: true` | Comparison |
| `dataPoints >= 3` | Data Dashboard |
| `hasHierarchy: true` | Hierarchy |
| `hasMatrix: true` | Matrix |
| `hasImage && textLength < 300` | Image + Insight |
| `hasImage && dataPoints >= 2` | Evidence Board |
| `textLength > 500 && !hasImage` | split into multiple slides |

## Anti-Patterns

- Equal-weight 6-card grid where nothing stands out
- Chart pasted with default office colors
- Timeline with 15 events crammed into one slide
- Comparison where both sides have different structures
- Dashboard where all numbers are the same size
