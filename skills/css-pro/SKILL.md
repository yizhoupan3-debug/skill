---
name: css-pro
description: |
  Architect maintainable CSS layout, responsive, animation, and vibrant design-token
  systems with explicit browser-support tradeoffs.
  Use when the user asks for CSS engineering, aesthetic layouts, animation
  optimization, or phrases like “CSS 布局策略”, “响应式方案”, “oklch 配色”, or “Grid
  subgrid”. Route Tailwind-specific work to `$tailwind-pro` and browser-mechanism
  explanations to `$web-platform-basics`.
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - css
    - layout
    - responsive
    - tailwind
    - animations
risk: low
source: local
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - CSS 布局策略
  - 响应式方案
  - oklch 配色
  - Grid subgrid
  - CSS engineering
  - aesthetic layouts
  - animation optimization
  - css
  - layout
  - responsive
---

# css-pro

This skill owns CSS engineering work: layout systems, responsive design, animation, architecture patterns, and CSS tooling.

## When to use

- The user wants to design or debug CSS layouts
- The task involves responsive design, grid, flexbox, subgrid, or container queries
- The task involves high-end CSS animations, View Transitions, or Scroll-driven animations
- The task involves oklch() color systems or vibrant design themes
- The task involves Tailwind CSS configuration or custom design systems
- Best for requests like:
  - "这个布局怎么用 Grid 实现"
  - "帮我做响应式适配"
  - "CSS 动画性能优化"
  - "设计一个 CSS 架构方案"
  - "配置 Tailwind 主题"

## Do not use

- The task is explaining browser-native CSS **behavior mechanics** (box model rules, cascade/specificity theory, overflow/stacking defaults, why a layout behaves a certain way) without engineering action → use `$web-platform-basics`
- The task is **Tailwind CSS** configuration, theming, plugins, or v3→v4 migration → use `$tailwind-pro`
- The task is pure HTML structure or DOM events without CSS focus → `$web-platform-basics`
- The task is JavaScript logic without styling concerns
- The task is framework component design → use framework skills

> **Decision guide: `css-pro` vs `web-platform-basics`**
> - "How should I architect this responsive layout?" → `css-pro` (engineering decision)
> - "Why is my element getting cut off by overflow hidden?" → `web-platform-basics` (mechanism explanation)
> - "Design a CSS custom properties based theme system" → `css-pro`
> - "How does the CSS cascade work?" → `web-platform-basics`

## Task ownership and boundaries

This skill owns:
- CSS layout systems (Grid, Flexbox, subgrid, container queries)
- responsive design and "Premium" layout strategies (Bento UI)
- CSS custom properties and oklch() design tokens
- animation, transitions, View Transitions, and Scroll-driven performance
- CSS architecture (BEM, CSS Modules, utility-first)
- Tailwind CSS configuration and customization

This skill does not own:
- HTML semantics and accessibility → `$web-platform-basics`
- JavaScript logic and framework state
- **Dual-Dimension Audit (Pre: Layout/Spec, Post: Cross-browser/Visual Results)** → `$execution-audit` [Overlay]

## Required workflow

1. Confirm the task shape:
   - object: layout, component styles, animation, design system, responsive breakpoints
   - action: build, debug, optimize, refactor, review
   - constraints: browser support, framework context, CSS methodology
   - deliverable: styles, architecture plan, or debug guidance
2. Check browser support requirements before using modern features.
3. Test across breakpoints and devices.
4. Validate with visual inspection and performance checks.

## Core workflow

### 1. Intake
- Identify CSS methodology (BEM, CSS Modules, Tailwind, styled-components).
- Check browser support and target devices.
- Inspect existing design tokens and variables.

### 2. Execution
- Use modern CSS features: `has()`, `container queries`, `@layer`, nesting.
- Design mobile-first responsive layouts.
- Use CSS custom properties for theming.
- Optimize animations with `will-change` and GPU-accelerated properties.
- Avoid `!important` and deep selector specificity.

### 3. Validation / recheck
- Test across major breakpoints (mobile, tablet, desktop).
- Check for layout shifts and paint issues.
- Validate animation performance (avoid layout-triggering properties).
- Check CSS bundle size if relevant.

## Capabilities

### Modern CSS
- CSS Grid and Subgrid
- Flexbox and alignment
- Container queries and `@container`
- CSS nesting (native and preprocessor)
- `@layer` cascade layers
- `:has()`, `:is()`, `:where()` selectors
- Color functions: `oklch()`, `color-mix()`, `light-dark()`
- View transitions API (cross-document and same-document)
- Scroll-driven animations (@scroll-timeline, animation-timeline)

### Layout Patterns
- Holy grail layout
- Responsive cards and masonry
- Sticky headers and footers
- Sidebar + content layouts
- Full-bleed within constrained containers

### Architecture
- BEM methodology
- CSS Modules
- Utility-first (Tailwind CSS)
- CSS-in-JS (styled-components, vanilla-extract)
- Design token systems
- Theming with custom properties

### Animation & Transitions
- CSS transitions and keyframes
- Scroll-driven animations
- View Transition API
- Performance optimization (compositor-only properties)
- `prefers-reduced-motion` accessibility

### Tooling
- PostCSS and autoprefixer
- Lightning CSS / cssnano
- Stylelint
- CSS specificity analysis
- `source-map-explorer` for CSS bundle analysis

> [!NOTE]
> For Tailwind CSS configuration, theming, and plugins, use `$tailwind-pro`.

## Output defaults

Recommended structure:

````markdown
## CSS Summary
- Methodology: ...
- Browser targets: ...
- Framework context: ...

## Changes / Guidance
- ...

## Validation / Risks
- Tested: ...
- Browser compatibility: ...
````

## Hard constraints

- Do not use `!important` without documenting why overrides are necessary.
- Do not use magic numbers for spacing; prefer design tokens or variables.
- Always include `prefers-reduced-motion` for non-trivial animations.
- Do not animate `width`, `height`, `top`, `left` when `transform` works.
- Check browser support before using cutting-edge features without fallbacks.
- Prefer logical properties (`inline-start`, `block-end`) over physical when i18n matters.
- CSS methodology decision tree: CSS Modules / BEM for large-scale apps needing strict specificity control; CSS-in-JS for tightly-coupled component logic+style. For utility-first (Tailwind), route to `$tailwind-pro`.
- When Tailwind questions arise, route to `$tailwind-pro` rather than providing in-line guidance.
- **Superior Quality Audit**: For "Premium" or layout-critical designs, trigger `$execution-audit` to verify against [Superior Quality Bar](../execution-audit/references/superior-quality-bar.md).

## Trigger examples

- "Use $css-pro to debug this Grid layout."
- "帮我设计 CSS 的响应式布局方案。"
- "这个动画在移动端卡顿怎么优化？"
- "配置 Tailwind 主题和 design tokens。"
- "强制进行 CSS 深度审计 / 检查布局逻辑与视觉复现结果。"
- "Use $execution-audit to audit this layout for pixel-perfect result idealism."
