---
name: humanizer
description: |
  General prose polish and style naturalization for existing drafts or supplied notes.
  Use for ordinary writing asks such as 润色这段话, 文本精修, 改自然, 表达优化,
  邮件/申请文书/博客/说明文字润色, humanize, 去模板腔, or 像人写的.
  Use sentence-level AIGC audit only when the user explicitly asks for AIGC,
  AI 味, detector, Turnitin, or 逐句评估. Preserve facts, register, and authorial voice.
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
  - 逐句判断是否要改
  - 逐句判断 aigc
  - 哪些句子像 ai 写的
  - 句子 ai 味
  - 降低 aigc
  - 降 ai 味
  - 去 ai 感
  - 不要直接重写
  - 先评估再改
  - 文本精修不好用
  - 持续优化写作
metadata:
  version: "3.5.0"
  platforms: [codex, antigravity]
  tags: [writing, rewrite, prose, naturalization, style, voice, humanize, anti-aigc, aigc-reduction, sentence-audit, sentence-grading]
risk: medium
source: local
---

# Humanizer

**Owner of style naturalization for existing text.**

## References

- [naturalization-rules.md](references/naturalization-rules.md) — rewrite rules + 5 core speed rules + detection-aware tactics
- [sentence-risk-rubric.md](references/sentence-risk-rubric.md) — 逐句评估与动作判定标准，含官方/论文边界
- [de-aigc-standards.md](../writing-skills/resources/de-aigc-standards.md) — shared universal de-AIGC rules
- [ai-patterns-checklist.md](references/ai-patterns-checklist.md) — 27 AI patterns + CN/EN phrase tables
- [register-presets.md](references/register-presets.md) — 7 register presets (EN + CN)
- [detection-mechanics.md](references/detection-mechanics.md) — how detectors work (perplexity, burstiness)
- [adversarial-strategies.md](references/adversarial-strategies.md) — 9 quality-based strategies to lower detection scores
- [claim-safety.md](references/claim-safety.md) — safe wording for detector-related claims and limits
- [examples/before-after.md](examples/before-after.md) — 11 before/after examples

## Routing

| Request type | Route to |
|---|---|
| Ordinary prose polish, general writing from supplied notes, naturalization | `$humanizer` (this skill) |
| 降 AI 味, 降 AIGC 率, 逐句评估 AIGC | `$humanizer` in audit-first mode |
| Paper-specific prose revision | `$paper-writing` |
| Marketing, landing page, sales, UX microcopy | `$copywriting` |
| README, API docs, ADR, developer docs | `$documentation-engineering` |
| Skill documentation or `SKILL.md` writing | `$writing-skills` / `$skill-framework-developer` |
| Scientific logic / novelty / evidence review | `$paper-logic` or `$paper-reviewer` |
| Translation or summarization only | Do not use this skill |

## When to use

- User wants sentence-by-sentence AIGC assessment before rewriting
- User wants existing text to sound more natural, less robotic, or less templated
- User wants a normal paragraph, email, statement, post, or explanation polished without a commercial or manuscript context
- User provides bullet points or rough notes and wants a short prose draft from those supplied facts
- User says "humanize", "去模板腔", "降 AI 味", "降低 AIGC 率", "降 aigc", "去 AI 感", "像人写的", "不像机器", "anti-AI", or "精修" (as a style overlay)
- User says "逐句评估", "先别改写", "先判断哪些句子 AI 味重", "逐句判断 AIGC", "哪些句子不用改 / 哪些要重写"
- Draft contains chatbot artifacts, vague praise, filler, repetitive rhythm, or generic conclusions
- Target is a paragraph, email, statement, blog, post, docs snippet, or other prose slice
- User explicitly wants to lower AIGC signals through quality improvement
- User wants a practical action table such as "不需要改 / 需要自然话改写 / 需要完全重写" per sentence
- End when the point is made — no mandatory wrap-up

## Work modes

Choose the narrowest mode that matches the request:

| Mode | When to use | Default output |
|---|---|---|
| **Direct Polish** | User says 精修/润色/自然化 but does not mention AIGC, detector, or sentence audit | Revised text first + at most 3 notes |
| **Draft from Notes** | User gives facts/bullets and asks for an email, statement, post, or explanation | Draft text + `[VERIFY: ...]` placeholders for missing facts |
| **Audit-only** | User wants judgment first, says "逐句评估/分级/先别改" | Sentence table + overall judgment |
| **Audit + Patch** | User wants risky sentences fixed but does not want a full rewrite | Sentence table + only rewritten risky lines |
| **Full Rewrite** | User explicitly asks for full naturalization or full anti-AIGC rewrite | Sentence table + full revised text + revision record |

If the request is ambiguous but does not mention AIGC/detectors, start with **Direct Polish**. If it mentions AIGC/detectors, start with **Audit-only**.

## Default mode

Default to **audit first, rewrite second** only for AIGC/detector-shaped asks:

1. Split the text into sentences.
2. Judge each sentence with the rubric in `references/sentence-risk-rubric.md`.
3. Only rewrite the sentences marked `需要自然话改写` or `需要完全重写`, or when the user explicitly asks for a full rewrite.

Do **not** jump straight into a full "降 AIGC" rewrite unless the user clearly asks for rewriting rather than evaluation.
For ordinary polishing, skip the visible sentence table and deliver improved prose first.
For note-to-prose drafting, write only from supplied facts and mark missing specifics with `[VERIFY: ...]`.

## Output Defaults

- For normal polish: return the revised text first.
- For small edits: do not include a table, score, or long explanation.
- For note-to-prose drafting: return a ready-to-use draft, then at most three missing-info notes.
- For AIGC/detector asks: use the sentence audit table before rewriting unless the user says to skip it.
- For long documents: work section by section and keep notes local to the edited section.

## Output Format: Sentence Audit Record (逐句评估记录)

When the user asks to lower AIGC, evaluate sentence-by-sentence first unless they explicitly ask to skip the audit.

### 1. Sentence Audit Table
| # | Sentence | Judgment | Signals | Why | Action |
|---|---|---|---|---|---|
| 1 | [Original sentence] | 不需要改 / 需要自然话改写 / 需要完全重写 | [2-4 concrete signals] | [why it reads human/AI-like] | keep / patch / rebuild |

### 2. Overall Judgment
- [Short conclusion: where the main AIGC concentration is]
- [Whether the text should be partially revised or fully rewritten]

### 3. Revised Text
[Deliver only if the user asked for rewriting, or if high-risk sentences clearly need repair]

### 4. Revision Record (修改意见)
| Target | Change Made | Strategy/Reason | Logic (Why this works) |
|---|---|---|---|
| [Sentence/Phrase] | [New Version] | [e.g., Rhythm variance] | [e.g., Breaks the uniform clause pattern] |
| [Technical claim] | [Grounded narr.] | [Pragmatic Grounding] | [Replaces generic importance talk with a concrete mechanism] |

### 5. Remaining Risks / Next Steps
- [Any remaining AI-ness or suggestions for further humanization]

## Do not use

- Main task is scientific review or claims-vs-evidence critique → `$paper-logic` / `$paper-reviewer`
- Task is manuscript prose revision in clear paper context → `$paper-writing`
- Task is improving skill documentation or `SKILL.md` files → `$writing-skills` / `$skill-framework-developer`
- Task is commercial copy, ads, landing pages, product positioning, or CTA writing → `$copywriting`
- Task is developer documentation, README, API docs, ADR, or changelog prose → `$documentation-engineering`
- Task is only translation or summarization
- User wants entirely new content ghostwritten from zero with no facts, notes, or intended message
- User wants fake authorship or fabricated personal experience
- User asks for guaranteed detector outcomes, pass guarantees, or bypass claims

## Handoff & Orchestration

This skill acts as the **sentence-level audit and style naturalization layer**. Once the text is audited and, if needed, cleaned up, it hands off to:

- **$paper-writing**: For academic papers that need professional scientific polishing after the AI structure is broken.
- **$copywriting**: For marketing/social copy that needs commercial frameworks (AIDA/PAS) after naturalization.
- **$writing-skills**: For repository-level skill documentation consistency.

If the user asks for "精修" (polishing) alongside humanization, perform the sentence audit first, then repair only the risky sentences before invoking the downstream skill.
Only consult `references/claim-safety.md` when the user explicitly asks about detector scores, Turnitin, or guarantee-style wording.
For a worked sentence-by-sentence example, see `examples/sentence-audit-demo.md`.

## Workflow

A single integrated pipeline. Depth scales automatically — short text can be audited mentally; long or heavily templated text should show the sentence table explicitly.

### Step 1 — Lock constraints (C.A.R.E. Framework)

- **Context**: Define the specific scholarly or commercial context (e.g., "IEEE conference paper on ML").
- **Audience**: Expert peers vs. general public.
- **Role**: Active researcher vs. neutral observer.
- **Examples**: Identify 1-2 examples of the target "voice" if provided.
- Preserve: facts, citations, numbers, causal logic, domain terminology.
- Identify register: academic / professional / technical / casual / founder / storytelling / social.
- Confirm whether the goal is sentence-level audit, style naturalization, register matching, or integrity-safe polishing.
- For Chinese text, also check CN subsections in `references/register-presets.md`.

### Step 2 — Sentence audit first

Split the text sentence-by-sentence and score each sentence against `references/sentence-risk-rubric.md`.

Use `references/ai-patterns-checklist.md` as the signal bank when deciding the grade.
Prioritize the patterns that most damage the specific text — don't enumerate all 27.
Use `references/claim-safety.md` only when the request explicitly pulls in detectors, scores, Turnitin, or AIGC percentage claims.

The audit should answer:
- Which sentences are already acceptable (`不需要改`)
- Which sentences need a natural-language patch (`需要自然话改写`)
- Which sentences are so templated that they should be rebuilt (`需要完全重写`)
- Whether the text's problem is local (a few sentences) or structural (whole paragraph rhythm / discourse)

Fast decision rule:
- Mostly `不需要改` -> keep structure, patch only wording if needed
- Several `需要自然话改写` in one paragraph -> partial paragraph rewrite
- Any `需要完全重写` plus repeated rhythm across neighbors -> rebuild the paragraph, not just the sentence

### Step 3 — Rewrite

Rewrite only the flagged sentences by default. Move to a full-paragraph rewrite only when the audit shows the structure itself is the problem.

Apply all of these as a **single integrated pass**, not sequential layers:

| Principle | What to do |
|---|---|
| **Specificity** | Replace vague claims with concrete details, numbers, names |
| **Attribution** | Name sources, dates, methods — not "experts say" |
| **Rhythm** | Let sentence lengths vary naturally (Burstiness); break three-item patterns |
| **Clean emphasis** | Remove gratuitous bold, emoji, em dash |
| **Register voice** | Apply the matching preset from `references/register-presets.md` |
| **Voice** | Keep the text natural and specific, but do not force personality or theatrical subjectivity |
| **Perplexity** | Prefer accurate-but-less-generic word choices; avoid template phrases |
| **Structure** | Break repetitive intro→body→conclusion flow when the source text is already predictable |
| **De-sterilize** | For science: narrate decisions and failures (Adv. Strategy 7) |
| **Synthesis** | Cluster citations to show cross-document reasoning (Adv. Strategy 16) |
| **Controlled Friction**| Use small, authentic asymmetries; keep them tied to real content |
| **Idiosyncrasy** | Use field-specific, less-common collocations that show insider knowledge |

Rules:
- Rewrite long inputs paragraph-by-paragraph, not as a uniform blob.
- Preserve low-risk sentences when they already read naturally.
- Never insert fake anecdotes, fake opinions, or fake sourcing.
- **Quality guard**: never reduce information density, specificity, or professional tone to lower a detector score. If a rewrite trades accuracy for "naturalness", revert.
- Do not force extra personality as a default tactic. Prefer clarity, specificity, honest limits, and natural rhythm.
- If the user asked for usable writing, do not stop at critique; produce the revised or drafted text.

Anti-bad-output rules:
- Do not force an audit table when the user only asked for smoother prose.
- Do not rewrite everything if only two sentences are the problem.
- Do not make the text casual just to make it "human"; match the requested register.
- Do not replace precise technical language with vague everyday wording.
- Do not provide detector-score promises or percentage claims.
- Do not append generic "writing tips" after a usable rewrite unless risk remains.

### Step 4 — Self-audit

Ask one question: **"What still makes this sound AI-generated?"**

Check these 5 dimensions:

| Dimension | Red flag |
|---|---|
| **Vocabulary** | AI words from checklist survive (leverage, tapestry, comprehensive...) |
| **Structure** | Paragraph-opening transitions stack; every paragraph same length |
| **Rhythm** | 5+ sentences of similar word count; uniform cadence |
| **Voice** | No opinions, no personality — sterile neutral reporting |
| **Openers/Closers** | Formulaic first/last sentences; quotable sound bites |

Fix the remaining tells. One focused revision pass — not a full rewrite.

**Anti-fake-convergence**: if you find zero tells, re-read the first and last sentence of each paragraph — AI patterns concentrate there.

### Step 5 — Quality gate & Iteration

Use both gates:

1. Sentence gate: no obvious `需要完全重写` sentences remain unless the user asked for audit-only output.
2. Document gate: score against `references/quality-scoring.md` (100pt scale) when a rewrite was actually performed.

- **≥ 85/100**: Deliver.
- **70–84**: Focused revision on the specific weak dimension.
- **< 70**: Reject and restart Step 3 with a different structural angle.

## Deep Optimization Protocol (5-10 Rounds)

When the user asks for "优化5轮" or "优化10轮", execute with **Dimension Rotation** to prevent the "AI Snake" (repetitive, diminishing quality):

| Round | Focus Dimension | Strategy |
|---|---|---|
| 1 | Structural Signal Stripping | Meta-discourse ban, transition word erasure |
| 2 | Rhythm Injection | 3-sentence variance rule, burstiness audit |
| 3 | Specificity (Lexical) | Replacing "AI words" with concrete data/mechanisms |
| 4 | Pragmatic Grounding | Injecting "Narrative of Friction" and technical "Why" |
| 5 | Adaptive Imperfection | Introducing controlled friction and asides |
| 6 | Syntactic Diversification | Breaking clause-comma-clause monotonous flows |
| 7 | Citation Synthesis | Clustering refs and showing active critique |
| 8 | Tone Recalibration | Senior-expert persona vs general professional |
| 9 | Signal-to-Noise Sweep | Removing "polishing artifacts" from previous rounds |
| 10 | Final Cohesion Check | Ensuring the text feels like a single human voice |

**Strict Anti-Convergence Rule**: Each round MUST show a concrete delta in the "Revision Record". If no delta is possible, explain the technical limitation.

## Handling "降低 AIGC 率" requests

When the user explicitly asks to lower AIGC detection rate:

1. Treat it as an **audit-first task**, not an auto-rewrite task.
2. Grade sentences first, then explain where the strongest machine-like signals concentrate.
3. Push on clarity, specificity, rhythm, register, and local idiosyncrasy instead of talking about detectors.
4. Use detector-aware heuristics only as an internal editing aid when needed.
5. Keep detector claims heuristic and tool-dependent; do not present sentence judgments as ground truth.
6. Rewrite only after the grading step, unless the user explicitly skips the audit.

## Overlay compatibility

- Stacks with `$iterative-optimizer` for multi-round deep rewrites.
- `$anti-laziness` auto-activates if rewrite repeats the same patterns across passes.
- When stacked with `$paper-writing` (handoff), this skill provides AI-pattern diagnosis; `$paper-writing` owns final wording.

## Hard constraints

- Never fabricate facts, references, data, experiments, quotes, or personal experience.
- Do not promise detector scores or guaranteed pass outcomes when the user explicitly asks about them.
- Do not present sentence-level grades as objective percentages or official detector outputs.
- Never optimize by making the text worse, less specific, or less accurate.
- Prefer clearer, more specific prose over ornamental rewriting.
- Quality comes first — no rewrite should make the text worse to achieve a detector score.
- Information density of the output must be ≥ the input. Cutting filler is good; cutting substance is not.
