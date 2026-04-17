---
name: tailwind-pro
description: |
  Produce Tailwind CSS configurations with design tokens, plugin hooks, and framework-ready integration.
  Use when the user asks about Tailwind configuration, custom themes, plugin authoring, design tokens, dark mode, responsive utilities, Tailwind 配置 / Tailwind 主题 / Tailwind 插件, or Tailwind v3 (`tailwind.config.js`) / v4 (`@theme`).
metadata:
  version: "1.0.0"
  platforms: [codex, antigravity, claude-code]
  tags:
    - tailwind
    - design-system
    - css
    - theme
    - utility-first
risk: low
source: local
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
---
# tailwind-pro

This skill owns Tailwind CSS-specific engineering: configuration, theming, plugin authoring, and integration patterns.

## When to use

- Configuring or customizing Tailwind CSS (v3 or v4)
- Designing a design system or token architecture with Tailwind
- Writing custom Tailwind plugins
- Integrating Tailwind with frameworks (React, Vue, Svelte, Next.js)
- Debugging Tailwind class conflicts, purge issues, or dark mode
- Best for requests like:
  - "帮我配置 Tailwind 主题"
  - "写一个 Tailwind 自定义插件"
  - "Tailwind v4 怎么迁移"
  - "设计一套 design tokens"

## Do not use

- The task is general CSS layout/animation without Tailwind focus → use `$css-pro`
- The task is pure component architecture → use the framework skill
- The task is backend code styling

## Task ownership and boundaries

This skill owns:
- Tailwind CSS v3/v4 configuration and migration
- design token architecture and oklch() color scales with Tailwind
- custom plugin and utility authoring for premium layouts (Bento, Glass)
- dark mode and responsive strategy (container queries) with Tailwind
- Tailwind integration with shadcn/ui and framer-motion

This skill does not own:
- general CSS layout, Grid, Flexbox without Tailwind context → `$css-pro`
- framework-specific component patterns
- **Dual-Dimension Audit (Pre: Config/Tokens, Post: Purge/Conflict Results)** → `$execution-audit-codex` [Overlay]

## Capabilities

### Tailwind v3
- `tailwind.config.js` / `tailwind.config.ts` configuration
- `theme.extend` for customization
- Custom colors, spacing, typography scales
- `@apply` directive usage and anti-patterns
- Plugins: `@tailwindcss/typography`, `@tailwindcss/forms`, `@tailwindcss/container-queries`
- JIT mode and arbitrary values (`[color:var(--custom)]`)
- Purge / content configuration for tree-shaking

### Tailwind v4
- CSS-first configuration with `@theme` directive
- No `tailwind.config.js` required
- Native CSS nesting support
- Container queries built-in
- `@variant` for custom variants
- Automatic content detection
- Lightning CSS engine
- Migration from v3 to v4

### Design System Integration
- Design tokens (colors, spacing, typography, shadows, radii)
- Consistent color palette with `oklch()` or HSL
- Responsive breakpoint strategy
- Dark mode patterns (`class` strategy vs `media` strategy)
- Component-level variant systems (`cva`, `class-variance-authority`)
- Tailwind Merge (`twMerge`) for class conflict resolution

### Plugin Authoring
- `plugin()` function API
- Adding utilities, components, and base styles
- Dynamic utilities with `matchUtilities`
- Accessing theme values in plugins
- Publishing reusable plugins

### Framework Integration
- React: `clsx` / `cn()` utility pattern with **shadcn/ui** components
- Animation: Integration with **framer-motion** using custom Tailwind utilities
- Vue: class binding with `:class` + Tailwind
- Svelte: class directive with Tailwind
- Next.js: `globals.css` setup, font integration

## Hard constraints

- Do not use `@apply` extensively; prefer composing Tailwind classes directly.
- Do not hardcode colors or spacing; use theme tokens.
- Do not fight Tailwind's utility-first philosophy with excessive custom CSS.
- When class lists become unreadable, extract into `cva` variants or component abstractions.
- Always configure content/purge paths to avoid bloated production CSS.
- Prefer `twMerge` when dynamically combining class strings to avoid conflicts.
- **Superior Quality Audit**: For design-system-level changes, trigger `$execution-audit-codex` to verify against [Superior Quality Bar](../execution-audit-codex/references/superior-quality-bar.md).

## Trigger examples

- "Use $tailwind-pro to set up a Tailwind v4 design system."
- "帮我配置 Tailwind 的 design tokens 和暗色模式。"
- "写一个自定义 Tailwind 插件。"
- "Tailwind v3 怎么迁移到 v4？"
- "强制进行 Tailwind 深度审计 / 检查 Config 逻辑与样式复现结果。"
- "Use $execution-audit-codex to audit this design system for superior theme integrity."
