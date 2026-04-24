---
name: humanizer
description: |
  Polish existing prose so it sounds natural, specific, and human while
  preserving facts and author voice. Use for 润色这段话, 文本精修, 改自然,
  表达优化, 去模板腔, 像人写的, humanize, 邮件/申请文书/博客/说明文字润色.
  Only run sentence-level AI味/AIGC risk audit when the user explicitly asks.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 润色这段话
  - 文本润色
  - 文本精修
  - 改自然
  - 表达优化
  - 普通写作
  - 邮件润色
  - 申请文书润色
  - 博客润色
  - 说明文字润色
  - 自然化改写
  - 去模板腔
  - 像人写的
  - humanize
  - 逐句评估
  - 哪些句子像 ai 写的
  - 降低 aigc
  - 去 ai 感
metadata:
  version: "3.6.0"
  platforms: [codex]
  tags: [writing, rewrite, prose, naturalization, style, voice, humanize]
risk: medium
source: local
---

# humanizer

This skill owns ordinary prose polishing. It makes supplied text clearer,
less templated, and more natural without changing the underlying facts.

## When to Use

- The user gives text and asks for polishing, naturalization, or expression cleanup.
- The task is emails, statements, blogs, descriptions, notes, or general prose.
- The user asks for "AI味" or AIGC reduction on supplied text.
- The user wants sentence-level risk comments before rewriting.

## Do Not Use

- Academic-paper sections with claim/evidence constraints -> use `$paper-writing`.
- Commercial conversion copy -> use `$copywriting`.
- Citation, reference, or literature work -> use the research skills.
- No source text is provided and the task is new content strategy.

## Operating Rules

- Preserve meaning, facts, names, numbers, and author intent.
- Improve specificity before adding style.
- Remove boilerplate phrases, symmetrical sentence rhythms, and generic transitions.
- Keep the requested register: casual, formal, academic, warm, concise, or persuasive.
- Do not invent lived experience, evidence, credentials, or emotions.
- If the user asks only to assess AI flavor, do not rewrite until useful.

## Workflow

1. Identify audience, purpose, language, and register from the text or request.
2. Decide whether the task is direct polish, rewrite, or sentence-level audit.
3. Fix unclear logic and unnatural phrasing while preserving substance.
4. Prefer a clean final version; include notes only when the user asks or risk is high.
5. If factual claims look unsafe, flag them rather than smoothing them into certainty.

## Sentence-Level AI Audit

Use this only when explicitly requested. Mark sentences as low, medium, or high
risk based on genericness, unsupported certainty, formulaic transitions,
over-balanced structure, and unnatural abstraction. Suggest targeted edits
instead of rewriting everything by default.

## Output Defaults

- For short text: return the polished version only.
- For longer text: return the polished version plus a very short change note if helpful.
- For audit requests: return a compact table with sentence, risk, reason, and fix.

## References

- [references/naturalization-rules.md](./references/naturalization-rules.md)
- [references/register-presets.md](./references/register-presets.md)
- [references/sentence-risk-rubric.md](./references/sentence-risk-rubric.md)
- [references/claim-safety.md](./references/claim-safety.md)
