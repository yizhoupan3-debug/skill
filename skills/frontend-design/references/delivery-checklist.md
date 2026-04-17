# Pre-Delivery Checklist

Run this checklist before delivering any UI code. Every ❌ is a potential quality issue.

## 1. Visual Quality

- [ ] No emojis used as icons (use SVG: Heroicons, Lucide, Phosphor, Simple Icons)
- [ ] All icons from ONE consistent icon set (never mix libraries)
- [ ] Brand logos are correct (verified from official sources / Simple Icons)
- [ ] Bento Grid: Proper spacing (1.5rem to 2rem) and consistent rounding (16px+)
- [ ] Hover states don't cause layout shift (avoid `scale` that moves siblings)
- [ ] No Inter/Roboto/Arial for headings (use distinctive fonts)
- [ ] Typography has dramatic scale contrast (hero ≠ section ≠ body)
- [ ] Colors: Using `oklch()` for vibrance and consistent perceived lightness
- [ ] No purple-gradient-on-white (AI slop) or generic cookie-cutter look

## 2. Interaction

- [ ] All clickable elements have `cursor-pointer`
- [ ] Hover states provide clear visual feedback (color, shadow, or border shift)
- [ ] Transitions: Fluid and responsive (150-300ms) or spring-based (Framer Motion)
- [ ] Spring Physics: Stiffness (300) and damping (20) for standard premium feel
- [ ] Focus states visible for keyboard navigation (`:focus-visible`)
- [ ] Buttons disabled during async operations (loading state)
- [ ] Error feedback is clear and near the problem element
- [ ] No hover-only interactions without mobile fallback

## 3. Light / Dark Mode

### Light Mode
- [ ] Page background is off-white (`#FAFAFA`), not pure `#FFFFFF`
- [ ] Primary text is near-black (`#1A1A1A`), not pure `#000000`
- [ ] Secondary text meets WCAG AA 4.5:1 contrast ratio
- [ ] Glass/transparent elements have sufficient opacity (≥ `bg-white/80`)
- [ ] Borders use `rgba(0,0,0,0.06-0.10)`, not hard grey values
- [ ] Shadows are soft and layered

### Dark Mode
- [ ] Base is NOT pure `#000000` (use `#111113` - `#141416`)
- [ ] 4-5 surface levels with subtle color tint (cool blue or warm neutral)
- [ ] Primary text is off-white (`#ECECEF`), not pure `#FFFFFF`
- [ ] Accent colors have reduced saturation vs light mode
- [ ] Borders use `rgba(255,255,255,0.06-0.10)`
- [ ] Scrollbars are styled to match (track transparent, thumb subtle)
- [ ] Glow effects used sparingly (max 1-2 per viewport)

### Both Modes
- [ ] Tested in both themes before delivery
- [ ] CSS variables defined for all theme colors
- [ ] Borders visible in both modes
- [ ] Icons readable in both modes

## 4. Layout & Responsive

- [ ] Floating elements (navbar, FAB) have spacing from edges
- [ ] No content hidden behind fixed navbars (account for height)
- [ ] Consistent container max-width (`max-w-6xl` or `max-w-7xl`)
- [ ] Responsive at 375px (mobile), 768px (tablet), 1024px (laptop), 1440px (desktop)
- [ ] No horizontal scroll on mobile
- [ ] Touch targets ≥ 44×44px with 8px gaps
- [ ] Body text ≥ 16px on mobile
- [ ] Images responsive with `srcset` and lazy loading

## 5. Component States

For every interactive element, verify:
- [ ] Resting state: clear default appearance
- [ ] Hover state: subtle feedback
- [ ] Focus state: visible ring (accent color)
- [ ] Active/Pressed state: visual confirmation
- [ ] Disabled state: reduced opacity + muted colors
- [ ] Loading state: spinner, skeleton, or pulse
- [ ] Error state: destructive color + icon + message
- [ ] Empty state: helpful illustration + what-to-do message

## 6. Accessibility Basics

- [ ] All meaningful images have descriptive `alt` text
- [ ] Form inputs have associated `<label>` elements
- [ ] Color is not the only indicator (supplement with icons/text)
- [ ] `prefers-reduced-motion` respected (disable/simplify animations)
- [ ] Semantic HTML used (`<nav>`, `<main>`, `<button>`, not `<div onClick>`)
- [ ] Tab order matches visual order

## 7. Performance Basics

- [ ] Images optimized (WebP, lazy loading, proper dimensions)
- [ ] Fonts subsetted with `font-display: swap`
- [ ] Animations use `transform` / `opacity` only (GPU-accelerated)
- [ ] No layout-triggering animations (`width`, `height`, `top`, `left`)
- [ ] Skeleton screens for async content (no blank voids)

---

## Severity Guide

| Symbol | Meaning |
|--------|---------|
| 🔴 | Critical: causes user confusion or inaccessibility |
| 🟡 | Important: degrades perceived quality |
| 🟢 | Nice-to-have: polish item |

Items in sections 1-3 are mostly 🔴/🟡. Items in sections 6-7 overlap with `$accessibility-auditor` and `$performance-expert` — this checklist only covers basics.
