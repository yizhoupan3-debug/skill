# Visual Design Principles for Slides

Design system foundations tailored for 1920×1080 HTML slide decks.

## Color System

### 60/30/10 Rule

- **60%** — Background / dominant color (slide background, large panels)
- **30%** — Secondary (cards, sidebars, section headers)
- **10%** — Accent (CTAs, highlights, key data, icons)

### Pre-built Slide Palettes

| Name | Dominant | Secondary | Accent | Vibe |
|------|----------|-----------|--------|------|
| **Midnight** | `#0F172A` | `#1E293B` | `#38BDF8` | Dark professional |
| **Warm Editorial** | `#FDF6E3` | `#EDE0C8` | `#D97706` | Light elegant |
| **Deep Ocean** | `#0C1222` | `#162032` | `#06B6D4` | Dark technical |
| **Forest** | `#1A2E1A` | `#2D4A2D` | `#4ADE80` | Dark organic |
| **Coral Studio** | `#1C1917` | `#292524` | `#F97316` | Dark creative |
| **Arctic** | `#F8FAFC` | `#E2E8F0` | `#6366F1` | Light minimal |

### Color Relationships

- **Analogous**: Adjacent hues → harmonious, low contrast
- **Complementary**: Opposite hues → high energy, use sparingly
- **Monochromatic**: One hue, varying lightness → sophisticated, cohesive

## Typography Hierarchy

### Scale for 1920×1080

| Role | Size Range | Weight | Purpose |
|------|-----------|--------|---------|
| Slide title | 48–72px | Bold/Black | One per slide, top or left |
| Section header | 36–48px | Semibold | Major content divisions |
| Card heading | 24–32px | Medium/Semibold | Within content blocks |
| Body copy | 18–22px | Regular | Explanatory text |
| Caption/note | 14–16px | Regular/Light | Sources, footnotes |
| Metric / KPI | 56–96px | Bold | Standalone data points |

### Font Pairing

| Display | Body | Vibe |
|---------|------|------|
| Playfair Display | Source Sans 3 | Classic editorial |
| Space Grotesk | Inter | Modern technical |
| Bebas Neue | DM Sans | Bold striking |
| Cormorant Garamond | Lato | Elegant |

### Blacklist (avoid as primary display)

Arial, Calibri, Times New Roman, Comic Sans, system defaults

## Composition on 1920×1080

### Layout Patterns

- **Hero**: One large visual (60%+ of slide) + overlaid title
- **Split**: Left content (50-60%) + right visual (40-50%), or vice versa
- **Grid**: 2×2 or 3×2 cards for multi-item content
- **KPI Strip**: 3-4 large metrics across the top + supporting detail below
- **Timeline**: Horizontal or vertical progression with milestones
- **Comparison**: Side-by-side panels with contrasting backgrounds

### Spacing System

- Slide padding: 80–120px from edges
- Card gap: 24–40px
- Content internal padding: 32–48px
- Consistent spacing is more important than exact values

### Hierarchy Rules

- One dominant element per slide (hero image, key metric, or headline)
- Maximum 3 levels of visual hierarchy
- If everything is emphasized, nothing is emphasized
- Use size, weight, and color contrast to create emphasis — not underlines or ALL CAPS

## Anti-Patterns

- ❌ Wall of text (>6 lines of body copy per card)
- ❌ Equal-sized everything (kills hierarchy)
- ❌ Decorative empty boxes (dead space with borders)
- ❌ Generic stock imagery (undermines credibility)
- ❌ More than 4 colors in one slide
- ❌ Centered everything (weakens visual flow)
- ❌ Tiny text to fit more content (split into more slides instead)
