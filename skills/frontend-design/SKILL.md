---
name: frontend-design
description: |
  Guide distinctive, high-end UI design: aesthetic direction, typography, color, motion, and delivery quality.
  Use when the user wants a page or interface to look "Premium", "Stunning", "WOW", or needs help
  implementing Bento UI, Glassmorphism, Mesh Gradients, or sophisticated micro-interactions.
  Not for `DESIGN.md` synthesis from existing UI assets, CSS mechanics, Tailwind config, accessibility
  audit, or performance work.
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 改UI
  - UI 改版
  - 品牌感
  - 视觉层级
  - 高级感
  - Bento UI
  - dashboard
  - Premium UI
  - Glassmorphism
  - Mesh Gradients
metadata:
  version: "1.1.0"
  platforms: [codex]
  tags:
    - ui-design
    - ux
    - typography
    - color-palette
    - frontend-aesthetics
risk: low
source: local
---

- **Dual-Dimension Audit (Pre: Layout-Spec/Logic, Post: Visual-Fidelity/Responsive Results)** → `$execution-audit` [Overlay]

# frontend-design

This skill owns **visual design decisions**: what the interface should
look and feel like, not the low-level CSS or framework mechanics.

## When to use

- The user wants to beautify or redesign a web interface with a "Premium" or "High-end" feel
- The user asks for aesthetic direction, Bento UI, Glassmorphism, or Mesh Gradient patterns
- The task is landing-page, dashboard, portfolio, or product-UI visual quality with a "WOW" factor
- The deliverable is a clearer design direction or a polished visual system for a modern web app
- The user already knows they want implementation-facing redesign rather than a reference-mapping pass

## Do not use

- Named-product reference grounding, `参考源`, `verified tokens`, or brand-plus-motion decomposition before implementation -> use `$design-agent`
- The user wants a stronger UI-generation prompt, DESIGN.md capture, or design acceptance workflow rather than implementation -> use `$design-workflow`
- CSS engineering or layout mechanics → use `$css-pro`
- Tailwind theming/config → use `$tailwind-pro`
- Accessibility review → use `$accessibility-auditor`
- High-end animations or micro-interaction implementation → use `$motion-design`
- Frontend performance / CWV optimization → use `$performance-expert`
- Frontend runtime bug diagnosis → use `$frontend-debugging`

## Core workflow

1. If the user starts from a named product/style reference and wants source grounding first, route to `$design-agent` before redesign.
2. If the user starts from existing product surfaces and wants a reusable house style, prompt, or acceptance protocol captured first, route to `$design-workflow` before redesign.
4. Identify the interface goal, audience, and brand tone (e.g., Luxury, Professional, Playful).
5. Choose one clear premium aesthetic direction (Bento, Glass, Minimal, etc.).
6. Define typography, oklch-based color, and motion (Framer Motion) system.
7. Ensure key interactive states and micro-animations (staggered reveals) are covered.
8. Deliver a concise design rationale plus production-ready guidance or code.

## Design rules

- Pick one coherent direction instead of mixing styles.
- Favor memorable hierarchy over generic “AI-looking” defaults.
- Respect accessibility and implementation constraints even when visual quality is primary.
- Push style catalogs and long checklists into references.
- **Superior Quality Audit**: For premium UI development, trigger `$execution-audit` to verify against [Superior Quality Bar](../execution-audit/references/superior-quality-bar.md).

## References

- [references/design-catalog.md](references/design-catalog.md)
- [references/delivery-checklist.md](references/delivery-checklist.md)

## Routing note

- Use `$design-agent` first when the request is "make it feel like X product" and the user wants reference sources, verified tokens, or borrow/adapt decisions before any UI rewrite starts.
- Use `$design-workflow` first when the request is "先把现有设计抽出来 / 先沉淀成 DESIGN.md / 设计 prompt / 页面生成提示词 / 设计验收".

## Trigger examples
- "这个设计规范里的配色和阴影怎么在 CSS 里精确还原？"
- "强制进行前端设计深度审计 / 检查布局规范与多端渲染结果。"
- "Use $execution-audit to audit this UI for visual-fidelity idealism."
