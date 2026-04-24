---
name: motion-design
description: |
  Design and implement high-end web animations, micro-interactions, and staggered reveals.
  Use when the user wants "alive" interfaces, "smooth" transitions, Framer Motion integration,
  GSAP effects, or spring-physics based interactions to achieve a "WOW" factor.
  Not for basic CSS transitions or low-level animation debugging.
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - motion design
  - Framer Motion
  - GSAP
  - smooth transitions
  - micro-interactions
  - alive
  - smooth
  - WOW
  - alive" interfaces
  - smooth" transitions
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - motion-design
    - framer-motion
    - gsap
    - animations
    - micro-interactions
risk: low
source: local
---

- **Dual-Dimension Audit (Pre: Easing-Logic/Curve, Post: Frame-Rate/Interaction Results)** → `$execution-audit` [Overlay]

# motion-design

This skill owns **motion strategy and high-level implementation**: how the interface moves and breathes, using modern libraries and physics-based principles.

## When to use

- The user wants to add "WOW" factor via animations
- Implementing staggered reveals, scroll-driven entrance, or page transitions
- Using **Framer Motion**, **GSAP**, or **Motion** (formerly Framer Motion)
- Designing micro-interactions (magnetic buttons, springy hovers, smooth modals)
- The task is about "fluidity", "smoothness", or "alive" UI

## Do not use

- Basic CSS layouts or simple `:hover` transitions → use `$css-pro`
- Performance optimization of existing animations → use `$performance-expert`
- Visual review of animation timing → `$visual-review`
- **Dual-Dimension Audit (Pre: Physics/Logic, Post: Smoothness/Timing Results)** → `$execution-audit` [Overlay]

## Core workflow

1. **Identify Timing & Pacing**: Choose between "Snappy" (Professional) or "Soft" (Playful).
2. **Apply Physics**: Use Spring physics (Stiffness/Damping) instead of linear durations.
3. **Orchestrate Reveals**: Use staggered variants for entrance animations.
4. **Link to State**: Ensure motion reflects data/interaction state changes.
5. **Iterate with Visuals**: Use `$visual-review` or screen recordings to tune easing.

## Principles for Premium Motion

- **Spring over Duration**: Use `{ stiffness: 300, damping: 20 }` for a standard premium feel.
- **Stagger Everything**: Children should reveal with a 0.05s-0.1s delay between them.
- **Directionality**: Elements should enter from the direction they were triggered or from a consistent "logical" direction (e.g., bottom-up).
- **Subtlety**: Motion should be felt but not distract from the content. Use small distances (10px-20px) for translates.

## Capabilities

- Framer Motion (Variants, Gesture, Layout, AnimatePresence)
- Scroll-driven animations (IntersectionObserver integration)
- View Transitions API usage for seamless page changes
- GSAP for complex timeline-based orchestration

## References

- [references/spring-presets.md](references/spring-presets.md)
- [references/stagger-patterns.md](references/stagger-patterns.md)
- [Superior Quality Bar](../execution-audit/references/superior-quality-bar.md)
