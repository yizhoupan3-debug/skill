---
name: design-md
description: |
  Analyze real UI assets and synthesize a reusable semantic design system into `DESIGN.md`.
  Use when the user wants to extract atmosphere, palette, typography, component signatures, and
  layout rules from existing screens or code before redesign or generation starts.
routing_layer: L3
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
trigger_hints:
  - design.md
  - DESIGN.md
  - 设计系统
  - 设计语言
  - 语义设计系统
  - 先抽设计规范
  - 先沉淀 DESIGN.md
  - 提炼现有界面的设计规则
metadata:
  version: "0.1.0"
  platforms: [codex, antigravity, claude-code]
  tags:
    - design-system
    - ui-design
    - semantic-design-system
    - design-spec
    - source-of-truth
risk: low
source: local
artifact_outputs:
  - DESIGN.md
---

# design-md

This skill turns an existing interface into a reusable **design source of
truth**. It should extract the design language from real artifacts and write a
`DESIGN.md` that later design or implementation work can follow consistently.

## When to use

- The user wants to analyze an existing product, screen set, screenshot pack, or front-end codebase and turn it into a reusable design spec
- The user explicitly asks for `DESIGN.md`, `design.md`, `设计系统`, `设计语言`, or a source-of-truth design file
- The task is not “直接改漂亮一点” but “先把现有设计抽象清楚，再驱动后续页面生成或改版”
- The user wants semantic naming for colors, typography, spacing, component shapes, and layout principles rather than raw CSS values alone
- The task needs a stable handoff artifact so later `frontend-design`, `css-pro`, `tailwind-pro`, or `motion-design` work stops drifting

## Do not use

- Named-product reference grounding comes first, such as `像 Linear 一样` or `先给我参考源` -> use `$design-agent`
- The user wants to rewrite a vague UI request into a structured generation prompt -> use `$design-prompt-enhancer`
- The user already wants direct UI redesign or implementation without a system-capture step -> use `$frontend-design`
- The task is screenshot defect review, overlap/readability audit, or visible regression triage -> use `$visual-review`
- The task is CSS mechanics, Tailwind token wiring, or responsive implementation -> use `$css-pro` or `$tailwind-pro`
- The task is motion-first behavior and transition language -> use `$motion-design`

## Core workflow

1. Identify the real source surfaces: screenshots, rendered pages, HTML/CSS, component code, design tokens, or live product pages.
2. Capture the **big picture first**:
   - product posture
   - visual atmosphere
   - density and whitespace strategy
   - hierarchy style
3. Extract **semantic tokens**, not just raw values:
   - color names + exact hex/rgb role
   - typography families, weights, scale, and voice
   - geometry such as corner language, stroke language, and spacing rhythm
   - depth model such as flat / soft / layered / high-contrast elevation
4. Distill component signatures:
   - buttons
   - cards / containers
   - inputs / forms
   - navigation
   - data-display patterns
5. Add generation-facing constraints:
   - reusable design-system block
   - anti-patterns / banned drift
   - what later prompts must preserve
6. Write the result into `DESIGN.md` using natural design language backed by concrete values.
7. End with downstream handoff guidance:
   - `$design-workflow-protocol` for a durable file-backed loop around this system
   - `$design-prompt-enhancer` for generation prompts that should consume this system
   - `$frontend-design` for redesign direction
   - `$design-output-auditor` for post-generation fidelity checks against this system
   - `$css-pro` / `$tailwind-pro` for token implementation
   - `$motion-design` for interaction expression

## Output contract

Default output should create or update `DESIGN.md` with:

1. `Visual Theme & Atmosphere`
2. `Color Palette & Roles`
3. `Typography Rules`
4. `Component Stylings`
5. `Layout Principles`
6. `Prompt Block For Reuse`
7. `Generation Guardrails`
8. `Anti-Patterns`

## Rules

- Start from real surfaces, not imagined style adjectives.
- Translate technical values into plain visual language the next model can actually follow.
- Keep exact color values and other high-signal constants when they matter.
- Name colors and patterns by role and character, not only by hue.
- Explain why a token exists in the system, not just what it is.
- Prefer one coherent house style over a long unordered dump of observations.
- Treat `DESIGN.md` as future prompt context, not just a descriptive report.
- Encode both "what to do" and "what must not drift".
- If the evidence is thin or contradictory, say what is confident vs inferred.

## References

- [references/semantic-design-system.md](references/semantic-design-system.md)
- [references/DESIGN-template.md](references/DESIGN-template.md)
- [references/design-quality-rubric.md](references/design-quality-rubric.md)

## Trigger examples

- "深度分析这个后台的现有页面，把设计语言沉淀成 `DESIGN.md`。"
- "先别改 UI，先从这批截图和前端代码里提炼一套设计系统。"
- "把这个产品现有的配色、字体、圆角、阴影、组件规则抽出来，后面所有页面都按这份设计规范走。"
