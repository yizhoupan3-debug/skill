---
name: copywriting
description: |
  Create persuasive commercial copy for landing pages, ads, product descriptions, and other conversion-focused assets.
  Use when the user asks to 写文案、广告词、产品卖点、落地页、品牌故事、CTA、小红书文案,
  销售信, 转化率文案, UX 微文案, or needs campaign copy, landing page copy,
  product descriptions, taglines, or other marketing text with a clear call-to-action.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
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
  - 销售信
  - 转化率文案
  - UX 微文案
  - in-app microcopy
  - cold email copy
metadata:
  version: "2.1.0"
  platforms: [codex]
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

- The task is naturalizing or de-templating existing prose with no commercial goal → use the owning domain skill, or `$documentation-engineering` for project docs
- The task is academic paper writing or polishing → use `$paper-writing`
- The task is HTML email template development (rendering, compatibility) → use `$email-template`
- The task is project documentation (README, API docs, ADR) → use `$documentation-engineering`
- The task is SEO technical implementation (structured data, meta tags, sitemap) -> use the current web implementation context
- The task is Word document formatting → use `$doc`
- The task is general prose rewriting with no commercial goal → use the owning domain skill, or `$documentation-engineering` for project docs

## Required workflow

1. Infer the brief when it is safe: audience, brand tone, CTA, platform/medium, word limits.
2. Choose the right copy framework for the task type.
3. Write the copy with strong hooks, clear value props, and actionable CTAs.
4. Polish: cut filler, sharpen rhythm, verify CTA clarity, remove AI patterns.
5. Deliver variants if A/B testing is in scope.

Do not block on brief questions unless the missing detail would materially change
the claim, compliance risk, audience, or platform constraints.

## Execution modes

- **Quick draft mode**: for short copy (headlines, CTAs, social posts) — deliver directly
- **Brief-first mode**: for landing pages, brand campaigns — confirm brief before writing
- **Variant mode**: when A/B testing is the goal — generate 2-4 strategically different versions
- **Platform-adapt mode**: when the same message needs to work across multiple platforms — write one core message, then adapt per platform
- **UX microcopy mode**: for in-product text — ultra-concise, context-aware, clarity over cleverness

## Output defaults

Default output should start with the final copy. Add brief notes only when they
help the user choose or reduce risk.

For one-liners, CTAs, titles, subject lines, and UX microcopy, output only the
best options with short labels. Do not include a full Brief section by default.

## Anti-bad-output rules

- Do not make every answer look like an AIDA/PAS template.
- Do not explain the framework unless the user asks.
- Do not write glossy but unverifiable promises; use concrete product facts or placeholders.
- Do not produce ten near-identical variants; each variant must use a different angle.
- Do not use generic AI marketing words such as "赋能", "革新", "一站式", or "极致体验" unless the brand voice requires them.

## Hard constraints

- Never fabricate statistics, testimonials, endorsements, or certifications.
- Never promise results the product or service cannot deliver.
- Always respect platform-specific character limits and formatting rules.
- Do not produce copy that is deceptive, misleading, or violates advertising regulations.
- For UX microcopy, prioritize clarity and helpfulness over cleverness.
- Do not default to English copy patterns when writing Chinese; adapt idiomatically.
- If brand guidelines are provided, follow them strictly.
