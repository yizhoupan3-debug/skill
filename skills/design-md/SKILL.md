---
name: design-md
description: |
  Gate for DESIGN.md-style design-system source of truth. Use at conversation
  start when the user asks to create, update, lint, diff, read, or apply
  `DESIGN.md`, capture reusable visual identity from UI/code/screenshots,
  extract design tokens, or audit UI output against a persistent design
  contract. Inspired by google-labs-code/design.md: YAML tokens are normative;
  markdown explains rationale. Not for direct UI redesign, named-product
  reference scouting, CSS mechanics, Tailwind implementation, or
  screenshot-only review. For high-bar UI or PPT work, use this gate when a
  reusable visual contract is needed before execution.
routing_layer: L3
routing_owner: gate
routing_gate: artifact
routing_priority: P1
session_start: required
trigger_hints:
  - DESIGN.md
  - design.md
  - 设计规范
  - 设计系统
  - 设计 token
  - 视觉身份
  - 风格漂移
  - 设计验收
  - 根据 DESIGN.md
  - 生成 DESIGN.md
  - extract design tokens
  - design contract
  - PPT 设计规范
  - UI 设计规范
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - design-system
    - design-md
    - design-tokens
    - ui-design
    - acceptance
risk: low
source: local
framework_roles:
  - gate
  - detector
allowed_tools:
  - shell
  - browser
approval_required_tools:
  - network install
filesystem_scope:
  - repo
  - artifacts
network_access: conditional
artifact_outputs:
  - DESIGN.md
  - design_acceptance.md
  - EVIDENCE_INDEX.json
---

# design-md

This skill is the **design-system source-of-truth gate**. It keeps visual
identity persistent across coding sessions by turning design intent into a
`DESIGN.md` contract before implementation owners start styling.

## When to use

- The primary artifact is `DESIGN.md` or a design-system spec
- The user asks to extract, create, update, lint, diff, or apply design tokens
- Existing UI/code/screenshots should be captured into a reusable visual identity
- A design output must be checked for drift against an agreed style contract
- The user wants a prompt or acceptance workflow grounded in persistent tokens
- UI or PPT work needs a reusable visual contract before implementation, deck
  authoring, or rendered QA

## Do not use

- Named-product reference grounding ("像 Linear/Stripe/Apple") without a persistent design contract -> use `$frontend-design`
- Direct UI redesign or implementation without a spec artifact -> use `$frontend-design`
- CSS layout mechanics, Tailwind config, or framework-specific theming -> use `$css-pro` or `$css-pro`
- Screenshot-only visible evidence -> use `$visual-review` first
- Motion implementation -> use `$frontend-design`

## Priority routing rule

If the task mentions `DESIGN.md`, design-system capture, design tokens, or
style-contract acceptance, check this gate before `frontend-design`. After the
contract is clear, hand off to the narrowest downstream owner.

## Core workflow

1. Decide mode: `capture`, `update`, `lint/diff`, `apply-to-implementation`, or `acceptance`.
2. Locate existing style sources: `DESIGN.md`, CSS variables, Tailwind theme,
   screenshots, component code, brand notes, or named references.
3. For named external references, record borrow/adapt/avoid decisions before
   writing tokens so the visual contract stays explicit.
4. Write or revise `DESIGN.md` with two layers:
   - YAML front matter for normative tokens
   - Markdown sections for rationale and usage rules
5. Keep token groups concrete: `colors`, `typography`, `spacing`, `rounded`,
   and `components` when useful.
6. Validate structural quality and contrast where possible; if using Google's
   CLI is acceptable in the environment, run `npx @google/design.md lint DESIGN.md`.
7. Hand off:
   - `$frontend-design` for visual direction or implementation
   - `$css-pro` / `$css-pro` for token wiring
   - `$slides`, `$ppt-pptx`, `$source-slide-formats`, or `$ppt-beamer` for deck authoring
   - `$visual-review` for rendered UI/deck proof
   - `$design-workflow` for prompt shaping or final acceptance summary

## Output contract

For `capture` / `update`, produce or patch `DESIGN.md` with:

- front matter tokens
- `## Overview`
- `## Colors`
- `## Typography`
- `## Layout`
- `## Elevation & Depth`
- `## Shapes`
- `## Components`
- `## Do's and Don'ts`

For `acceptance`, return:

- `verdict`: pass, rework, or fail
- `token drift`: concrete mismatches against the YAML tokens
- `rationale drift`: mismatches against the markdown guidance
- `handoff`: the next owner and smallest repair step

For `apply-to-implementation`, return a compact mapping before handoff:

- `source tokens`: exact `DESIGN.md` token names and values used
- `target surfaces`: CSS variables, Tailwind theme keys, component props, or deck theme fields
- `owner`: `$css-pro`, `$css-pro`, `$frontend-design`, or a slide/deck artifact owner
- `verification`: lint, rendered screenshot review, or route-specific test

## UI and PPT trigger scenarios

Use this gate before doing UI or PPT work when any of these are true:

- The user says the UI/deck must preserve a brand style, house style, or visual identity across multiple screens/slides.
- The task involves a redesign plus future reuse, not just one-off styling.
- A PPT/deck needs theme colors, typography hierarchy, callout styles, chart colors, or component-like slide blocks defined before authoring.
- The user asks to make output "less AI", more consistent, or less style-drifty and there is enough source material to codify a contract.
- The implementation owner needs exact tokens before editing Tailwind/CSS, HTML slides, Beamer theme macros, or `deck.plan.json`.

Do not block quick one-off UI/PPT tasks just to create a spec. If the user only
needs immediate visual execution, route directly to `$frontend-design` or the
presentation artifact owner and optionally backfill `DESIGN.md` later.

## Rules

- Treat YAML token values as the source of truth; prose explains intent but
  must not contradict tokens.
- Prefer stable semantic token names over one-off visual descriptions.
- Preserve unknown sections unless they conflict with core sections.
- Do not implement CSS/Tailwind/component changes inside this gate unless the
  change is only to the `DESIGN.md` contract; hand off implementation after the
  mapping is explicit.
- Do not clone external brand systems; encode portable cues and target-product
  identity.
- Keep this gate compact. Put format details in references instead of bloating
  the main skill.

## References

- [references/design-md-format.md](references/design-md-format.md)
