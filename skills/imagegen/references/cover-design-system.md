# Cover Image Design System

A structured 5-dimension system for generating professional cover images.
Inspired by baoyu-cover-image's parametric approach.

## Five Dimensions

Every cover image is defined by exactly 5 dimensions. When the user doesn't
specify a dimension, auto-select based on content analysis.

### 1. Type — Visual Composition

| Type | Description | Best For |
|------|-------------|----------|
| `hero` | Large visual impact, title overlay | Product launch, brand promotion, major announcements |
| `conceptual` | Concept visualization, abstract core ideas | Technical articles, methodology, architecture |
| `typography` | Text-focused layout, prominent title | Opinion pieces, quotes, insights |
| `metaphor` | Visual metaphor, concrete expressing abstract | Philosophy, growth, personal development |
| `scene` | Atmospheric scene, narrative feel | Stories, travel, lifestyle |
| `minimal` | Minimalist composition, generous whitespace | Zen, focus, core concepts |

### 2. Palette — Color Scheme

| Palette | Vibe | Primary Colors |
|---------|------|----------------|
| `warm` | Friendly, approachable | Orange, golden yellow, terracotta |
| `elegant` | Sophisticated, refined | Soft coral, muted teal, dusty rose |
| `cool` | Technical, professional | Engineering blue, navy, cyan |
| `dark` | Cinematic, premium | Electric purple, cyan, magenta |
| `earth` | Natural, organic | Forest green, sage, earth brown |
| `vivid` | Energetic, bold | Bright red, neon green, electric blue |
| `pastel` | Gentle, whimsical | Soft pink, mint, lavender |
| `mono` | Clean, focused | Black, near-black, white |
| `retro` | Nostalgic, vintage | Muted orange, dusty pink, maroon |

**60/30/10 Rule**: Dominant color 60%, secondary 30%, accent 10%.

### 3. Rendering — Visual Style

| Rendering | Description | Key Characteristics |
|-----------|-------------|---------------------|
| `flat-vector` | Clean modern vector | Uniform outlines, flat fills, geometric icons |
| `hand-drawn` | Sketchy organic illustration | Imperfect strokes, paper texture, doodles |
| `painterly` | Soft watercolor/paint | Brush strokes, color bleeds, soft edges |
| `digital` | Polished modern digital | Precise edges, subtle gradients, UI components |
| `pixel` | Retro 8-bit pixel art | Pixel grid, dithering, chunky shapes |
| `chalk` | Chalk on blackboard | Chalk strokes, dust effects, board texture |

### 4. Text — Information Density

| Level | Title | Subtitle | Tags | Use Case |
|-------|:-----:|:--------:|:----:|----------|
| `none` | - | - | - | Pure visual, no text |
| `title-only` | ✓ | - | - | Simple headline (default) |
| `title-subtitle` | ✓ | ✓ | - | Title + supporting context |
| `text-rich` | ✓ | ✓ | ✓ | Information-dense |

### 5. Mood — Emotional Intensity

| Mood | Contrast | Saturation | Weight | Use Case |
|------|:--------:|:----------:|:------:|----------|
| `subtle` | Low | Muted | Light | Corporate, thought leadership |
| `balanced` | Medium | Normal | Medium | General articles (default) |
| `bold` | High | Vivid | Heavy | Announcements, promotions |

## Compatibility Quick-Reference

Highly recommended combinations (✓✓):

- `hero` + `vivid`/`dark` + `digital` + `bold`
- `conceptual` + `cool`/`elegant` + `flat-vector` + `balanced`
- `typography` + `mono`/`dark` + `digital` + any mood
- `metaphor` + `earth`/`warm` + `painterly`/`hand-drawn` + `balanced`
- `scene` + `warm`/`pastel` + `painterly` + `subtle`
- `minimal` + `mono`/`pastel` + `flat-vector` + `subtle`

Avoid (✗):

- `minimal` + `text-rich` (contradicts minimalism)
- `pixel` + `text-rich` (pixel art doesn't render readable dense text)
- `chalk` + `vivid` (chalk medium conflicts with vivid saturation)

## Cover Prompt Template

When generating a cover image, structure the prompt as:

```
Use case: cover-image
Cover type: [type]
Palette: [palette] (dominant: [hex], secondary: [hex], accent: [hex])
Rendering: [rendering]
Text level: [text]
Mood: [mood]
Aspect ratio: [ratio]
Title text (verbatim): "[exact title]"
Title language: [en/zh/ja/...]
Composition: [specific layout notes]
Constraints: [must keep / must avoid]
```

## Auto-Selection Rules

When dimensions are unspecified, infer from content:

| Content Signal | Suggested Type | Suggested Palette |
|----------------|---------------|-------------------|
| Technical / architecture | `conceptual` | `cool` |
| Personal story / reflection | `metaphor` | `warm` / `earth` |
| Product / announcement | `hero` | `vivid` / `dark` |
| Opinion / quote / insight | `typography` | `mono` / `elegant` |
| Travel / lifestyle / narrative | `scene` | `warm` / `pastel` |
| Zen / focus / philosophy | `minimal` | `mono` / `pastel` |
