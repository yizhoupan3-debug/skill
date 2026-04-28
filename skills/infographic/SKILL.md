---
name: infographic
description: |
  Generate HTML/CSS/JS infographics — single-page long-form visuals, knowledge cards, and data summary posters.
  Use when the user asks to create "信息图", "infographic", "一图读懂", "知识卡片",
  "数据长图", "summary poster", or needs a structured visual data summary rendered
  as a self-contained HTML file that can be screenshotted or exported to PDF/PNG.
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 信息图
  - infographic
  - 一图读懂
  - 知识卡片
  - 数据长图
  - summary poster
  - exported to PDF
  - PNG
  - html
  - data visualization
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - infographic
    - html
    - data-visualization
    - knowledge-card
    - visual-summary
risk: low
source: local
---

# Infographic

Generate self-contained HTML infographics — single-page long-form data visuals,
knowledge cards, "一图读懂" posters, and structured summary graphics. Output is
an HTML file rendered in-browser and optionally exported to PNG/PDF via screenshot.

## When to use

- The user wants a visual data summary as an infographic
- The task involves creating "一图读懂", knowledge cards, or data long-form images
- The user needs a structured visual poster with sections, icons, and data points
- The deliverable is an HTML file that renders a single-page infographic
- The user wants a visual summary of an article, report, or dataset
- Best for requests like:
  - "帮我做个信息图"
  - "把这篇文章做成一图读懂"
  - "Create an infographic summarizing this data"
  - "做个知识卡片"
  - "把这个报告做成数据长图"

## Do not use

- The user wants AI-generated raster images (photos, illustrations) → use `$image-generated`
- The task is a multi-page HTML slide presentation → use `$source-slide-formats`
- The task is a Mermaid flow/sequence diagram → use `$diagramming`
- The task is a Graphviz/DOT precise-layout diagram → use `$diagramming`
- The task is a code-driven scientific plot (matplotlib, seaborn) → use `$scientific-figure-plotting`
- The task is existing manuscript figure/table review -> use `$paper-reviewer` figure-table mode or `$visual-review`

## Boundary with `image-generated`

| Scenario | Route |
|----------|-------|
| "做个信息图" (structured, text-rich, data-driven) | `infographic` (HTML) |
| "生成一张 infographic 风格的图片" (AI-rendered raster) | `image-generated` (slug: `infographic-diagram`) |
| "做个封面图" | `image-generated` (cover workflow) |
| "把数据做成可视化长图" | `infographic` (HTML) |

Rule of thumb: if the output should be **editable, text-selectable, and structured** → `infographic`. If the output should be an **AI-rendered raster image** → `image-generated`.

## Workflow

### 1. Content Analysis

- Extract key data points, sections, and hierarchy from the source material
- Identify the narrative arc: what story does this data tell?
- Determine the information density: how much content needs to fit?
- Count sections to plan vertical layout

### 2. Structure Design

Choose a layout pattern (see [references/layout-patterns.md](references/layout-patterns.md)):

| Pattern | Best For |
|---------|----------|
| **Vertical flow** | Article summaries, timelines, step-by-step |
| **Card grid** | Multi-topic overviews, comparison |
| **Split panel** | Before/after, pros/cons, dual perspectives |
| **Dashboard** | Metrics-heavy, KPI summaries |
| **Timeline** | Historical, process, chronological |

### 3. HTML Construction

Build a single `infographic.html` file:

- Fixed width (typically 800–1200px), variable height
- Self-contained: all CSS inline or in `<style>`, no external dependencies
- Use CSS Grid / Flexbox for layout
- Use Google Fonts via `@import` for typography
- Use CSS custom properties for theming

#### Design Defaults

- **Background**: gradient or solid with subtle texture
- **Sections**: clear visual separation (color blocks, dividers, spacing)
- **Typography**: display font for headers + body font for content
- **Icons**: Unicode emoji, SVG inline, or CSS shapes (no external icon libraries required)
- **Colors**: follow 60/30/10 rule; use a cohesive palette
- **Spacing**: generous padding (32–48px between sections)
- **Width**: 900px default; 800px for phone-oriented; 1200px for desktop-oriented

#### Must-haves

- Title section with clear hierarchy
- Visual section dividers (not just whitespace)
- At least one data visualization (chart, metric, or comparison)
- Source attribution at bottom
- Readable at both full-size and 50% zoom

#### Must-avoid

- ❌ External image URLs (download and embed as data URI or use CSS/SVG)
- ❌ JavaScript frameworks (keep it vanilla)
- ❌ More than 3 font families
- ❌ Wall of text without visual breaks
- ❌ Inconsistent spacing or alignment

### 4. Browser Preview & QA

- Open in browser and verify rendering
- Check: text readability, color contrast, section alignment, responsive at target width
- Use `$visual-review` for structured QA when available

### 5. Export (optional)

If the user needs PNG/PDF output:
- Use Puppeteer or browser screenshot to capture the full-page render
- For PNG: `page.screenshot({ fullPage: true })`
- For PDF: `page.pdf({ printBackground: true })`

## Style Presets

| Preset | Background | Palette | Font | Vibe |
|--------|-----------|---------|------|------|
| `modern-dark` | `#0F172A` → `#1E293B` | Cyan/Purple accents | Space Grotesk + Inter | Professional tech |
| `warm-light` | `#FDF6E3` → `#FEF3C7` | Amber/Coral accents | Playfair + Lato | Editorial elegant |
| `minimal` | `#FFFFFF` | Black + one accent | DM Sans | Clean corporate |
| `vivid` | `#1A1A2E` → `#16213E` | Neon green/pink/blue | Outfit + DM Sans | Bold energetic |
| `earth` | `#F5F0EB` → `#E8DDD3` | Forest/sage/brown | Cormorant + Source Sans | Natural organic |

## Output Conventions

- Default output directory: project root or `output/infographic/`
- Filename: `infographic.html` (or `{topic-slug}-infographic.html`)
- If exporting images: `infographic.png` / `infographic.pdf` alongside

## Hard Constraints

- All content must be self-contained in the HTML file
- No external runtime dependencies (no React, no Tailwind CDN — vanilla only)
- Every section must be visually distinct
- Text must be selectable (not rendered as images)
- Must render correctly in Chrome/Safari/Firefox
- Source attribution must be present when data comes from external sources

## Reference Map

- [references/layout-patterns.md](references/layout-patterns.md) — layout patterns, CSS snippets, and section templates
