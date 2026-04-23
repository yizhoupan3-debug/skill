---
name: design-agent
description: |
  Route named-product design references and brand-plus-motion source grounding before implementation.
  Use when the user asks for a product to feel like Linear, Stripe, Apple, Vercel, or another named
  reference and explicitly wants reference sources, verified tokens, brand mapping, or borrowable
  component/motion cues before any UI rewrite starts. Not for direct page redesign, CSS mechanics,
  motion implementation, or screenshot review.
routing_layer: L3
routing_owner: gate
routing_gate: none
routing_priority: P2
session_start: n/a
trigger_hints:
  - 像 Linear 一样
  - 参考源
  - verified tokens
  - 品牌 token
  - Stripe 的品牌 token
  - liquid glass motion
  - 产品风格映射
metadata:
  version: "0.1.0"
  platforms: [codex, antigravity, claude-code]
  tags:
    - design-routing
    - design-reference
    - brand-tokens
    - source-grounding
    - ui-design
risk: low
source: local
framework_roles:
  - gate
allowed_tools:
  - shell
  - browser
approval_required_tools:
  - browser
filesystem_scope:
  - repo
  - artifacts
network_access: conditional
artifact_outputs:
  - design_reference_map.md
---

# design-agent

This skill is a **design source-grounding gate**. It decides which external
product cues are safe to borrow, what should stay distinct, and which
downstream design owner should execute the work after the reference frame is
clear.

## When to use

- The user asks for a product or interface to feel like a named reference
- The user explicitly wants `参考源`, `verified tokens`, brand tokens, or a style map first
- The request mixes brand cues with motion or component language and needs decomposition before implementation
- The user says not to start changing the page yet

## Do not use

- The user wants a structured generation prompt for their own UI request -> use `$design-prompt-enhancer`
- The user already wants direct UI redesign or implementation -> use `$frontend-design`
- The user wants to extract a reusable `DESIGN.md` from their own existing product surfaces -> use `$design-md`
- The request is specifically about Framer Motion, transitions, or micro-interactions -> use `$motion-design`
- The task is CSS layout, tokens, responsive rules, or Tailwind mechanics -> use `$css-pro` or `$tailwind-pro`
- The task depends on screenshots or visible artifact evidence -> use `$visual-review`

## Core workflow

1. Extract the requested references, constraints, and "must feel like / must not feel like" boundaries.
2. Split each reference into reusable surfaces:
   - brand tokens
   - layout grammar
   - component signatures
   - motion cues
   - tone / product posture
3. Mark each cue as:
   - `verified`: directly grounded in the named reference pattern
   - `portable`: safe to borrow across products
   - `risky`: likely to become imitation or conflict with the target product
4. Produce a borrow / avoid / adapt decision map.
5. Hand off to the narrowest downstream owner:
   - `$design-prompt-enhancer` for structured generation prompts after the reference frame is settled
   - `$design-md` for synthesizing the target product's own design source of truth
   - `$frontend-design` for visual direction
   - `$motion-design` for motion expression
   - `$css-pro` or `$tailwind-pro` for tokenization / layout implementation

## Output contract

Default output should be compact and source-first:

1. `reference frame`
2. `verified tokens`
3. `borrowable components / motion cues`
4. `conflict / imitation risks`
5. `recommended next owner`

## Rules

- Do not jump straight into implementation when the user asked for source grounding first.
- Do not collapse all named references into a vague "premium" style bucket.
- Separate visual language from motion language; mixed-source requests should stay decomposed until handoff.
- Prefer portable design principles over literal cloning.

## References

- [references/source-routing.md](references/source-routing.md)
