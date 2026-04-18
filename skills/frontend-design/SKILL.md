---
name: frontend-design
description: |
  Guide distinctive, high-end UI design: aesthetic direction, typography, color, motion, and delivery quality.
  Use when the user wants a page or interface to look "Premium", "Stunning", "WOW", or needs help
  implementing Bento UI, Glassmorphism, Mesh Gradients, or sophisticated micro-interactions.
  Not for CSS mechanics, Tailwind config, accessibility audit, or performance work.
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - Premium UI
  - Stunning
  - WOW
  - Glassmorphism
  - Mesh Gradients
  - Premium
  - a page
  - interface to look "Premium
  - sophisticated micro-interactions
  - ui design
metadata:
  version: "1.1.0"
  platforms: [codex, antigravity, claude-code]
  tags:
    - ui-design
    - ux
    - typography
    - color-palette
    - frontend-aesthetics
risk: low
source: local
---

- **Dual-Dimension Audit (Pre: Layout-Spec/Logic, Post: Visual-Fidelity/Responsive Results)** → `$execution-audit-codex` [Overlay]

# frontend-design

This skill owns **visual design decisions**: what the interface should
look and feel like, not the low-level CSS or framework mechanics.

## When to use

- The user wants to beautify or redesign a web interface with a "Premium" or "High-end" feel
- The user asks for aesthetic direction, Bento UI, Glassmorphism, or Mesh Gradient patterns
- The task is landing-page, dashboard, portfolio, or product-UI visual quality with a "WOW" factor
- The deliverable is a clearer design direction or a polished visual system for a modern web app

## Do not use

- CSS engineering or layout mechanics → use `$css-pro`
- Tailwind theming/config → use `$tailwind-pro`
- Accessibility review → use `$accessibility-auditor`
- High-end animations or micro-interaction implementation → use `$motion-design`
- Frontend performance / CWV optimization → use `$performance-expert`
- Frontend runtime bug diagnosis → use `$frontend-debugging`

## Core workflow

1. Identify the interface goal, audience, and brand tone (e.g., Luxury, Professional, Playful).
2. Choose one clear premium aesthetic direction (Bento, Glass, Minimal, etc.).
3. Define typography, oklch-based color, and motion (Framer Motion) system.
4. Ensure key interactive states and micro-animations (staggered reveals) are covered.
5. Deliver a concise design rationale plus production-ready guidance or code.

## Design rules

- Pick one coherent direction instead of mixing styles.
- Favor memorable hierarchy over generic “AI-looking” defaults.
- Respect accessibility and implementation constraints even when visual quality is primary.
- Push style catalogs and long checklists into references.
- **Superior Quality Audit**: For premium UI development, trigger `$execution-audit-codex` to verify against [Superior Quality Bar](../execution-audit-codex/references/superior-quality-bar.md).

## References

- [references/design-catalog.md](references/design-catalog.md)
- [references/delivery-checklist.md](references/delivery-checklist.md)

## Trigger examples
- "这个设计规范里的配色和阴影怎么在 CSS 里精确还原？"
- "强制进行前端设计深度审计 / 检查布局规范与多端渲染结果。"
- "Use $execution-audit-codex to audit this UI for visual-fidelity idealism."
