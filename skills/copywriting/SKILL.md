---
name: copywriting
description: |
  Create persuasive commercial copy for landing pages, ads, product descriptions, and other conversion-focused assets.
  Use when the user asks to 写文案、广告词、产品卖点、落地页、品牌故事、CTA、小红书文案, or needs campaign copy, landing page copy, product descriptions, taglines, or other marketing text with a clear call-to-action.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 广告词
  - 产品卖点
  - 落地页
  - 品牌故事
  - CTA
  - 小红书文案
  - landing page copy
  - product descriptions
  - taglines
  - copywriting
metadata:
  version: "2.0.0"
  platforms: [codex, antigravity]
  tags:
    - copywriting
    - marketing
    - brand
    - social-media
    - ux-copy
    - seo-copy
    - slogan
    - cta
    - ad-copy
risk: low
source: local
---

# Copywriting

This skill owns **goal-driven commercial copy creation** — text designed to
persuade, convert, or build brand awareness, distinct from prose naturalization,
academic writing, or technical documentation.

For detailed guidance, use:
- [references/copy-frameworks.md](/Users/joe/Documents/skill/skills/copywriting/references/copy-frameworks.md) — AIDA, PAS, BAB, 4U, FAB, StoryBrand with EN+CN examples
- [references/quality-checklist.md](/Users/joe/Documents/skill/skills/copywriting/references/quality-checklist.md) — P0/P1/P2 copy quality audit
- [references/tone-presets.md](/Users/joe/Documents/skill/skills/copywriting/references/tone-presets.md) — 6 brand voice registers (Premium, Friendly, Professional, Bold, Playful, Empathetic)

## When to use

- The user wants to create new marketing or advertising copy from scratch
- The task involves writing product descriptions, landing page text, or ad headlines
- The user wants brand-level writing: slogans, taglines, brand stories, tone-of-voice guides
- The task involves social media copy adapted to specific platforms (小红书, 微博, Twitter, LinkedIn, Instagram)
- The user needs UX microcopy: onboarding flows, button labels, empty states, error messages, tooltips
- The user wants SEO-optimized copy (titles, meta descriptions, keyword-rich content)
- The user wants sales copy: cold emails, outreach messages, pitch deck text
- The user wants multiple copy variants for A/B testing
- Best for requests like:
  - "帮我写个落地页文案"
  - "给这个产品写个卖点"
  - "写一组小红书推广文案"
  - "Write a compelling tagline for our brand"
  - "帮我写个 CTA 按钮文案"
  - "写几个 A/B 测试的标题变体"
  - "给这个功能写个引导文案"

## Do not use

- The task is naturalizing or de-templating existing prose → use `$humanizer`
- The task is academic paper writing or polishing → use `$paper-writing`
- The task is HTML email template development (rendering, compatibility) → use `$email-template`
- The task is project documentation (README, API docs, ADR) → use `$documentation-engineering`
- The task is SEO technical implementation (structured data, meta tags, sitemap) → use `$seo-web`
- The task is Word document formatting → use `$doc`
- The task is general prose rewriting with no commercial goal → use `$humanizer`

## Task ownership and boundaries

This skill owns:
- marketing and advertising copy creation
- brand voice and messaging (slogans, taglines, brand stories, tone guides)
- social media copy with platform-specific adaptation
- product UX microcopy (onboarding, CTA, empty states, errors, tooltips)
- SEO content copy (keyword-optimized titles, descriptions, articles)
- sales copy (cold outreach, pitch text)
- A/B variant generation for copy testing

This skill does not own:
- style naturalization of existing drafts (→ `$humanizer`)
- academic or scientific prose (→ `$paper-writing`)
- HTML email rendering and client compatibility (→ `$email-template`)
- SEO technical implementation (→ `$seo-web`)
- developer documentation (→ `$documentation-engineering`)

If the task shifts to adjacent skill territory, route to:
- `$humanizer` when the user mainly wants to polish existing text, not create new copy
- `$seo-web` when the task shifts from content to technical SEO markup
- `$email-template` when the focus becomes email rendering, not email content

## Handoff to humanizer

Route to `$humanizer` when one or more become true:

- the user already has a draft and mainly wants it to sound less robotic or AI-generated
- the request is "改自然 / 去模板腔 / 降 AI 味" without any conversion or brand goal
- the text is not commercial copy but a general paragraph, email, or statement

When handing off:
- state briefly that the task is better treated as prose naturalization
- keep any brand-tone context the user gave
- let `$humanizer` own the style pass

## Handoff from humanizer

Accept handoff from `$humanizer` when:

- the rewrite reveals the text is actually commercial copy that needs strategic restructuring, not just style cleanup
- the user shifts from "改自然" to "重写一版更有卖点的"
- the goal changes from naturalization to conversion optimization

## Required workflow

1. Confirm the brief: audience, brand tone, CTA, platform/medium, word limits.
2. Choose the right copy framework for the task type.
3. Write the copy with strong hooks, clear value props, and actionable CTAs.
4. Polish: cut filler, sharpen rhythm, verify CTA clarity, remove AI patterns.
5. Deliver variants if A/B testing is in scope.

## Core workflow

### 1. Brief intake

Confirm or infer:
- **Target audience**: who is reading this? demographics, pain points, desires
- **Brand tone**: formal / casual / playful / authoritative / luxury / friendly
- **Goal**: awareness / conversion / engagement / retention
- **CTA**: what action should the reader take?
- **Platform/medium**: landing page / ad / social post / email / in-app
- **Constraints**: character limits, keyword requirements, brand guidelines

### 2. Framework selection

Choose the copy framework that fits the task:

| Framework | Best for | Structure |
|---|---|---|
| **AIDA** | Landing pages, ads | Attention → Interest → Desire → Action |
| **PAS** | Problem-aware audiences | Problem → Agitate → Solution |
| **BAB** | Aspirational products | Before → After → Bridge |
| **4U** | Headlines, subject lines | Useful, Urgent, Unique, Ultra-specific |
| **FAB** | Product descriptions | Feature → Advantage → Benefit |
| **StoryBrand** | Brand narratives | Character → Problem → Guide → Plan → Action → Success |

### 3. Copy creation

- Lead with the strongest hook — the first line must earn the second
- Focus on benefits over features; translate features into user outcomes
- Use concrete, specific language; avoid vague adjectives
- Match the platform's native voice and format conventions
- For Chinese copy: adapt idiom, rhythm, and cultural references; do not translate English copy patterns literally
- For social media: respect platform character limits and formatting norms

#### Platform-specific notes

| Platform | Key conventions |
|---|---|
| **小红书** | Emoji-rich, personal voice, list format, 标题党 hooks, hashtags |
| **微博** | 140 chars originally, visual-first, trending topic hooks |
| **Twitter/X** | 280 chars, thread format for long copy, hook-first |
| **LinkedIn** | Professional tone, insight-led, story format, line breaks for readability |
| **Landing page** | Hero headline → sub-headline → value props → social proof → CTA |
| **In-app UX** | Ultra-concise, action-oriented, context-aware, no jargon |

### 4. Polish and audit

- Remove filler words and weak intensifiers (很, 非常, really, very)
- Vary sentence length for rhythm
- Verify CTA is specific and actionable ("立即领取" > "了解更多")
- Check for AI-sounding patterns (generic praise, templated structure)
- Ensure tone consistency throughout
- For UX copy: verify clarity at a glance; test with "5-second rule"
- Run the full audit against `references/quality-checklist.md`

### 5. Self-audit loop (two-pass)

After initial draft:
1. Re-read and ask: "Would I click this? Would I stop scrolling for this?"
2. Run the 5-dimension quick audit (Hook / Clarity / Proof / CTA / Voice)
3. List 1-3 remaining weaknesses
4. Revise to address them
5. For long copy, show the audit explicitly; for short copy, do it mentally

### 6. Variant delivery (if A/B scope)

- Generate 2-4 meaningfully different variants, not cosmetic rewrites
- Vary: hook angle, emotional appeal, CTA phrasing, length
- Label each variant with its strategic angle (e.g., "pain-point lead" vs "benefit lead")

## Execution modes

- **Quick draft mode**: for short copy (headlines, CTAs, social posts) — deliver directly
- **Brief-first mode**: for landing pages, brand campaigns — confirm brief before writing
- **Variant mode**: when A/B testing is the goal — generate 2-4 strategically different versions
- **Platform-adapt mode**: when the same message needs to work across multiple platforms — write one core message, then adapt per platform
- **UX microcopy mode**: for in-product text — ultra-concise, context-aware, clarity over cleverness

## Staged pipeline (for complex copy)

For high-stakes or long-form copy, use a staged pipeline:

| Stage | Goal | Focus |
|---|---|---|
| **DRAFT** | Get the structure and message right | Framework, hook, value prop, CTA |
| **REFINE** | Improve rhythm, specificity, and punch | Word choice, sentence rhythm, proof points |
| **AUDIT** | Catch remaining issues | Quality checklist, AI patterns, tone consistency |

- Simple copy (headline, CTA): DRAFT only
- Standard copy (product page, social post): DRAFT → REFINE
- High-stakes copy (landing page, brand story, campaign): DRAFT → REFINE → AUDIT

## Output defaults

Default output should contain:
- copy brief summary (audience, tone, goal)
- final copy (or variants)
- strategic rationale for key choices

Recommended structure:

````markdown
## 文案 Brief
- 受众：...
- 调性：...
- 目标：...
- CTA：...

## 文案

[Final copy here]

## 变体（如适用）

### 变体 A — [角度说明]
...

### 变体 B — [角度说明]
...

## 策略说明
- ...
````

## Hard constraints

- Never fabricate statistics, testimonials, endorsements, or certifications.
- Never promise results the product or service cannot deliver.
- Always respect platform-specific character limits and formatting rules.
- Do not produce copy that is deceptive, misleading, or violates advertising regulations.
- For UX microcopy, prioritize clarity and helpfulness over cleverness.
- Do not default to English copy patterns when writing Chinese; adapt idiomatically.
- If brand guidelines are provided, follow them strictly.

## Trigger examples

- "帮我写一个 SaaS 产品的落地页文案，重点突出效率提升。"
- "给这个新功能写 3 个 A/B 测试标题。"
- "Write social media copy for our product launch across Twitter and LinkedIn."
- "写一组小红书种草文案，目标是 25-35 岁女性用户。"
- "帮我写一封冷邮件，目标是约 CTO 聊产品合作。"
- "给 App 的首次引导流程写一套 UX 文案。"
