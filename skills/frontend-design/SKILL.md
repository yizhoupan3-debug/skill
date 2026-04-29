---
name: frontend-design
description: |
  Guide distinctive, high-end UI design: aesthetic direction, typography, color, motion, and delivery quality.
  Use when the user wants a page or interface to look "Premium", "Stunning", "WOW", or needs help
  implementing Bento UI, Glassmorphism, Mesh Gradients, or sophisticated micro-interactions.
  Use with `$design-md` when a page/app needs a reusable design contract before
  implementation. Not for `DESIGN.md` synthesis/lint/diff/read/application,
  design-system capture, CSS mechanics, Tailwind config, accessibility audit,
  motion implementation, or performance work.
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
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
  - redesign UI
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

- **Dual-Dimension Audit (Pre: Layout-Spec/Logic, Post: Visual-Fidelity/Responsive Results)** → runtime verification gate

# frontend-design

This skill owns **visual design decisions**: what the interface should
look and feel like, not the low-level CSS or framework mechanics.

## When to use

- The user wants to beautify or redesign a web interface with a "Premium" or "High-end" feel
- The user asks for aesthetic direction, Bento UI, Glassmorphism, or Mesh Gradient patterns
- The task is landing-page, dashboard, portfolio, or product-UI visual quality with a "WOW" factor
- The deliverable is a clearer design direction or a polished visual execution for a modern web app
- The user already knows they want implementation-facing redesign rather than a reference-mapping pass
- A UI task has an existing `DESIGN.md` or visual contract and needs it applied
  to page layout, component hierarchy, color, type, and motion

## Do not use

- The user mentions `DESIGN.md`, design-system capture, design tokens, style-contract lint/diff/read/apply, or spec acceptance -> use `$design-md`
- CSS engineering or layout mechanics → use `$css-pro`
- Tailwind theming/config → use `$css-pro`
- Accessibility review → use `$accessibility-auditor`
- Frontend performance / CWV optimization → use `$performance-expert`
- Frontend runtime bug diagnosis → use `$frontend-debugging`

## Core workflow

1. If the user starts from existing product surfaces and wants a reusable house style, design tokens, or `DESIGN.md` captured first, route to `$design-md` before redesign.
2. If a `DESIGN.md` or visual contract exists, read it first and map tokens to the actual UI surfaces before inventing new style.
3. Identify the interface goal, audience, brand tone, and any named product/style references.
4. Choose one clear premium aesthetic direction (Bento, Glass, Minimal, etc.).
5. Define typography, oklch-based color, and motion system.
6. Ensure key interactive states and micro-animations are covered.
7. Deliver a concise design rationale plus production-ready guidance or code.

## Design rules

- Pick one coherent direction instead of mixing styles.
- If a `DESIGN.md` exists, preserve its tokens unless there is a concrete reason
  to patch the contract through `$design-md`.
- Favor memorable hierarchy over generic “AI-looking” defaults.
- Respect accessibility and implementation constraints even when visual quality is primary.
- Push style catalogs and long checklists into references.
- **Superior Quality Audit**: For premium UI development, apply the runtime verification gate to verify against [Superior Quality Bar](runtime verification criteria).

## References

- [references/design-catalog.md](references/design-catalog.md)
- [references/delivery-checklist.md](references/delivery-checklist.md)

## Routing note

- Use `$design-md` first when the request is "先把现有设计抽出来 / 先沉淀成 DESIGN.md / 设计规范 / 设计 token / 设计验收".
- Keep named-product references and UI-generation prompt shaping inside this skill unless the user needs a persistent `DESIGN.md` contract.

## Trigger examples
- "这个设计规范里的配色和阴影怎么在 CSS 里精确还原？"
- "强制进行前端设计深度审计 / 检查布局规范与多端渲染结果。"
- "Use the runtime verification gate to audit this UI for visual-fidelity idealism."
