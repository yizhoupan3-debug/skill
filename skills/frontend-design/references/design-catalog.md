# Design Catalog

Comprehensive reference for aesthetic directions, typography, color, motion, and spatial composition.

## Aesthetic Directions

### Minimalist

| Style | Key Traits | Best For |
|-------|-----------|----------|
| **Brutally Minimal** | Stark contrast, system fonts, zero decoration, maximum whitespace | Developer tools, documentation |
| **Swiss Modernism** | Grid-based, geometric, structured, limited color | Corporate, portfolio |
| **Japanese Zen** | Natural materials, subtle shadows, breathing room, muted tones | Wellness, luxury |
| **Scandinavian** | Clean lines, natural light, soft colors, functional beauty | Home, lifestyle |

### Maximalist

| Style | Key Traits | Best For |
|-------|-----------|----------|
| **Maximalist Chaos** | Layered elements, bold typography, explosive color | Creative agencies, music |
| **Neo-Brutalism** | Bold shapes, thick borders (3-5px), shadow extrusion, high contrast | Startups, playful apps |
| **Y2K/Cyber** | Metallic gradients, chrome effects, futuristic UI, neon accents | Gaming, tech |
| **Retro-Futuristic** | 70s/80s sci-fi, analog/digital fusion, warm gradients | Entertainment |

### Distinctive

| Style | Key Traits | Best For |
|-------|-----------|----------|
| **Editorial/Magazine** | Bold typography hierarchy, generous images, asymmetric grids | Blogs, media |
| **Art Deco/Geometric** | Symmetry, gold accents, geometric patterns, luxury feel | Finance, luxury brands |
| **Organic/Natural** | Curves, earth tones, textures, hand-drawn elements | Food, eco, crafts |
| **Soft/Pastel** | Gentle gradients, rounded corners, light colors, dreamy atmosphere | Kids, beauty, social |
| **Industrial/Utilitarian** | Raw materials, monospace fonts, technical aesthetic | Engineering, data tools |
| **Glassmorphism** | Frosted glass, transparency, layered depth, subtle border-glow | Dashboards, modern SaaS |
| **Bento UI** | Staggered grid cells, varying heights, rounded corners (16-24px), clean hierarchy | Dashboards, feature sections, Apple-style landings |
| **Mesh Gradients** | Vibrantly blurry circular shapes, low-contrast background motion, premium vibe | Hero sections, high-end marketing |
| **Neumorphism** | Soft shadows, subtle extrusion, tactile interfaces | Settings, music players |
| **Claymorphism** | 3D clay-like textures, soft shadows, playful depth | Education, casual apps |
| **Skeuomorphism** | Realistic textures, light simulation, physical metaphors | Specialized tools |
| **Flat Design** | Zero depth, solid colors, geometric icons, content-first | Utility apps, minimalist |

---

## Typography System

### Banned Fonts (avoid generic AI aesthetic)

❌ Inter · ❌ Roboto · ❌ Arial · ❌ Helvetica · ❌ Space Grotesk · ❌ System fonts (unless intentionally brutalist)

### Display/Heading Fonts

| Category | Fonts |
|----------|-------|
| Serif | Playfair Display, Crimson Pro, Spectral, Lora, Fraunces |
| Sans-serif | DM Sans, Manrope, Outfit, Sora, Cabinet Grotesk, Archivo |
| Geometric | Montserrat, Raleway, Poppins (use sparingly) |
| Condensed | Anton, Bebas Neue, Oswald, Barlow Condensed |
| Editorial | Bodoni Moda, Libre Baskerville, Cormorant |
| Experimental | Righteous, Unbounded, Rubik, Recursive |

### Body/Text Fonts

| Category | Fonts |
|----------|-------|
| Classic | Source Sans Pro, Public Sans, IBM Plex Sans, Work Sans |
| Modern | Instrument Sans, Geist, Satoshi, Plus Jakarta Sans |
| Editorial | Lora, Merriweather, Source Serif Pro, Bitter |
| Monospace | JetBrains Mono, Fira Code, IBM Plex Mono |

### Pairing Patterns

**Contrast** (Serif + Sans): `Playfair Display` + `DM Sans`
**Harmony** (Same family): `Outfit 700` + `Outfit 400`
**Editorial** (Serif + Serif): `Fraunces` + `Lora`

### Typography Scale

```css
/* GOOD — dramatic contrast */
.hero-title  { font-size: clamp(48px, 8vw, 120px); line-height: 1.1; }
.section-title { font-size: clamp(32px, 4vw, 64px); line-height: 1.2; }
.body-text   { font-size: 18px; line-height: 1.6; }
.caption     { font-size: 14px; line-height: 1.5; }

/* BAD — timid scaling */
.hero-title  { font-size: 36px; }
.section-title { font-size: 24px; }
```

---

## Color System

### Strategy: Dominant + Accent (60/30/10)

```css
:root {
  /* Using oklch for vibrant, uniform perceived lightness */
  --primary: oklch(25% 0.05 260);    /* Deep Navy */
  --accent: oklch(70% 0.18 45);      /* Vibrant Orange */
  --highlight: oklch(90% 0.15 90);   /* Soft Yellow */
  --background: oklch(98% 0.01 260); /* Off-white */
  --text: oklch(20% 0.02 260);       /* Near-black */
}
```

### Light Mode Parameters

| Element | Value | Notes |
|---------|-------|-------|
| Page background | `#FAFAFA` / `#FAF9F7` | Off-white, never pure `#FFFFFF` |
| Cards/surfaces | `#FFFFFF` | Pure white for natural hierarchy |
| Primary text | `#1A1A1A` / `#0F172A` | Near-black, never pure `#000000` |
| Secondary text | `#6B7280` / `#64748B` | Must meet WCAG AA (4.5:1) |
| Borders | `rgba(0,0,0,0.06-0.10)` | Prefer whitespace over borders |
| Card shadow | `0 1px 3px rgba(0,0,0,0.04), 0 1px 2px rgba(0,0,0,0.06)` | Soft, layered |
| Dropdown shadow | `0 4px 6px -1px rgba(0,0,0,0.05), 0 2px 4px -2px rgba(0,0,0,0.05)` | |
| Modal shadow | `0 10px 25px -5px rgba(0,0,0,0.08), 0 8px 10px -6px rgba(0,0,0,0.04)` | |

### Dark Mode Parameters

| Element | Value | Notes |
|---------|-------|-------|
| Level -1 (deepest) | `#09090B` – `#0C0C0E` | Never pure `#000000` |
| Level 0 (page) | `#111113` – `#141416` | |
| Level 1 (cards) | `#1A1A1E` – `#1C1C20` | |
| Level 2 (dropdowns) | `#222228` – `#27272A` | |
| Level 3 (modals) | `#2A2A30` – `#2C2C32` | |
| Primary text | `#ECECEF` / `#E4E4E7` | Off-white, never pure `#FFFFFF` |
| Secondary text | `#9898A0` / `#A0A0A8` | Must meet WCAG AA |
| Borders | `rgba(255,255,255,0.06-0.10)` | Semi-transparent |
| Card shadow | `0 2px 8px rgba(0,0,0,0.3), 0 1px 3px rgba(0,0,0,0.4)` | Much darker than light mode |
| Scrollbar thumb | `rgba(255,255,255,0.12)` | Track transparent, 6-8px thin |
| Accent adjustment | Reduce saturation 10-20% | Colors pop on dark backgrounds |

### Palette Archetypes

**Monochromatic + Punch:**
```css
:root {
  --color-900: #0a2540; --color-700: #1a4d7a;
  --color-500: #2a7ab0; --color-300: #5fa8d3;
  --accent: #ff6b35;
}
```

**Natural/Earthy:**
```css
:root { --clay: #d4a574; --stone: #6b7280; --moss: #4a5d23; --sand: #f4f1e8; }
```

**High Contrast/Bold:**
```css
:root { --black: #000; --white: #fff; --red: #ff0000; --yellow: #ffff00; }
```

---

## Motion Patterns

### Philosophy

**One orchestrated moment > many scattered micro-interactions.**

### Priority

1. **Page load** — staggered reveals (highest impact)
2. **Scroll triggers** — section transitions
3. **Hover states** — surprising responses
4. **State changes** — smooth transitions

### CSS Staggered Reveal

```css
.hero-content > * {
  animation: fadeInUp 0.8s cubic-bezier(0.16, 1, 0.3, 1) backwards;
}
.hero-content > *:nth-child(1) { animation-delay: 0.1s; }
.hero-content > *:nth-child(2) { animation-delay: 0.2s; }
.hero-content > *:nth-child(3) { animation-delay: 0.3s; }

@keyframes fadeInUp {
  from { opacity: 0; transform: translateY(30px); }
  to   { opacity: 1; transform: translateY(0); }
}
```

### Scroll Reveal (IntersectionObserver)

```css
.reveal {
  opacity: 0; transform: translateY(50px);
  transition: opacity 0.8s ease, transform 0.8s ease;
}
.reveal.visible { opacity: 1; transform: translateY(0); }
```

```javascript
const observer = new IntersectionObserver((entries) => {
  entries.forEach(e => { if (e.isIntersecting) e.target.classList.add('visible'); });
}, { threshold: 0.1 });
document.querySelectorAll('.reveal').forEach(el => observer.observe(el));
```

### High-Impact Hover Effects

```css
/* Lift & Shadow + Spring-like feel */
.card { transition: transform 0.5s cubic-bezier(0.175, 0.885, 0.32, 1.275), box-shadow 0.3s ease; }
.card:hover { transform: translateY(-8px) scale(1.02); box-shadow: 0 20px 60px rgba(0,0,0,0.15); }

/* Framer Motion Suggestion (React) */
/* useSpring({ stiffness: 300, damping: 20 }) is the gold standard for micro-interactions */
```

### Reduced Motion

```css
@media (prefers-reduced-motion: reduce) {
  * { animation-duration: 0.01ms !important; transition-duration: 0.01ms !important; }
}
```

---

## Visual Atmosphere

### Gradient Mesh

```css
.gradient-mesh {
  background:
    radial-gradient(circle at 20% 30%, rgba(255,107,53,0.3) 0%, transparent 50%),
    radial-gradient(circle at 80% 70%, rgba(98,0,234,0.3) 0%, transparent 50%),
    radial-gradient(circle at 50% 50%, rgba(52,211,153,0.2) 0%, transparent 50%),
    linear-gradient(135deg, #0a0a0a 0%, #1a1a1a 100%);
}
```

### Glassmorphism

```css
.glass {
  background: rgba(255,255,255,0.1);
  backdrop-filter: blur(10px) saturate(180%);
  border: 1px solid rgba(255,255,255,0.2);
  box-shadow: 0 8px 32px rgba(0,0,0,0.1);
}
```

### Layered Shadows

```css
.card-elevated {
  box-shadow:
    0 2px 4px rgba(0,0,0,0.05),
    0 8px 16px rgba(0,0,0,0.1),
    0 16px 32px rgba(0,0,0,0.1),
    0 32px 64px rgba(0,0,0,0.1);
}
```

### Colored Shadow

```css
.button-primary {
  background: #ff6b35;
  box-shadow: 0 10px 40px rgba(255, 107, 53, 0.4);
}
```

---

## Component Guidance

### Buttons
Primary (solid accent) / Secondary (subtle border) / Ghost (transparent hover). Consistent border-radius.

### Inputs
Resting (subtle border) → Focus (accent ring + `aria-describedby`) → Error (red border + message) → Disabled (muted).

### Cards
16-24px padding, consistent radius, subtle elevation. Hierarchy: title → body → actions.

### Tables
Row hover: `rgba(0,0,0,0.02)` light / `rgba(255,255,255,0.03)` dark. Mobile: scroll + sticky first column or card layout.

### Modals
`backdrop-filter: blur(8px)`. Enter: `scale(0.97) opacity(0)` → `scale(1) opacity(1)`, 150-200ms.

### Toasts
Light, unobtrusive. White/tinted background with left-edge color accent for type indicator.

---

## Scenario Cheat Sheet

| Scenario | Hero | Grid | Interaction | CTA |
|----------|------|------|-------------|-----|
| **Landing Page** | Bold type + striking visual | Asymmetric features | Scroll reveals | High-contrast animated button |
| **Dashboard** | Clean sidebar + metrics | Data cards with shadows | Smooth transitions | Subtle action buttons |
| **Portfolio** | Large type + photo | Hover-reveal project grid | Parallax / magnetic cursor | Simple contact form |
| **E-commerce** | Product hero + lifestyle | Product grid with hover zoom | Quick-view overlay | Cart glassmorphic overlay |
