---
name: design-prompt-enhancer
description: |
  Transform vague UI or page-generation asks into structured, copy-ready design prompts with
  platform, atmosphere, design-system block, page structure, and targeted edit instructions. Use
  when the user wants better design-generation prompts rather than direct UI implementation.
routing_layer: L3
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
trigger_hints:
  - 设计 prompt
  - UI 提示词
  - 优化页面生成提示词
  - 把需求改成设计 prompt
  - 结构化设计提示词
  - 生成页面提示词
  - stitch prompt
  - design system block
metadata:
  version: "0.1.0"
  platforms: [codex, antigravity, claude-code]
  tags:
    - design-prompt
    - ui-design
    - prompt-engineering
    - structured-generation
    - design-system
risk: low
source: local
---

# design-prompt-enhancer

This skill turns rough design intent into a **structured generation prompt**.
It is the missing layer between a vague request and later design generation or
editing work.

## When to use

- The user wants a better UI-generation prompt instead of direct implementation
- The user has a rough page idea and wants it rewritten into a structured design prompt
- The user wants a prompt that can reliably drive Stitch, image/UI generators, or later design agents
- The task needs explicit blocks such as platform, atmosphere, design-system tokens, and page structure
- The user wants targeted edit prompts like "改这个按钮 / 改 hero / 改导航" written in a precise way

## Do not use

- Generic prompt engineering unrelated to design/UI -> use `$prompt-engineer`
- The user wants to extract a reusable `DESIGN.md` from existing product surfaces -> use `$design-md`
- The user wants named-reference source grounding first -> use `$design-agent`
- The user already wants direct redesign or implementation -> use `$frontend-design`
- The task is screenshot-grounded defect review -> use `$visual-review`

## Core workflow

1. Identify whether the prompt is for:
   - new page generation
   - targeted edit
   - multi-page consistency
2. Translate vague user language into concrete UI terms:
   - page type
   - component names
   - visual atmosphere
   - interaction expectations
3. If a `DESIGN.md` or existing design system is available, pull its high-signal constraints into a reusable `DESIGN SYSTEM` block.
4. Format the prompt into a stable structure:
   - one-line purpose and vibe
   - platform / theme / design-system block
   - numbered page structure or specific edit instructions
5. Add precision where helpful:
   - semantic color roles
   - shape language
   - spacing / density cues
   - banned drift or anti-patterns
6. Include acceptance-oriented guardrails when consistency matters:
   - what must stay fixed
   - what can vary
   - what later audit should reject
7. If the user is building a repeatable loop, place the prompt inside the design artifact protocol rather than leaving it chat-only.
8. Return a clean prompt the user can copy directly.

## Output contract

Default output should produce one of these:

1. `generation prompt`
2. `edit prompt`
3. `generation prompt + compact rationale`

## Rules

- Do not stop at adjectives like "高级感" or "modern".
- Always convert vague nouns into real UI component language.
- Prefer explicit page structure over one-paragraph prompt blobs.
- If a `DESIGN.md` exists, use it as a hard consistency block instead of paraphrasing it loosely.
- For edit prompts, keep the change narrow and local.
- Separate what is required from what is optional.

## References

- [references/prompt-enhancement-pipeline.md](references/prompt-enhancement-pipeline.md)
- [references/design-prompt-template.md](references/design-prompt-template.md)
- [references/vague-to-ui-mapping.md](references/vague-to-ui-mapping.md)

## Trigger examples

- "把这个产品需求改成一个更强的 UI 生成 prompt。"
- "这个页面生成出来太普通了，帮我重写 prompt。"
- "先别做页面，先把这段需求整理成结构化设计提示词。"
