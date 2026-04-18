---
name: humanizer
description: |
  Naturalize existing prose into clearer, more human-sounding text.
  Use for: 精修, 文本精修, humanize, 自然化改写, 去模板腔, 降 AI 味/AIGC 率, 去 AI 感, 像人写的, anti-AI.
  Acts as a **Core Style Naturalizer** for existing text. Focuses on preserving facts, register, and authorial voice while reducing templated or machine-like structure.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - writing
  - rewrite
  - prose
  - naturalization
  - style
  - voice
metadata:
  version: "3.1.0"
  platforms: [codex, antigravity]
  tags: [writing, rewrite, prose, naturalization, style, voice, humanize, anti-aigc, aigc-reduction]
risk: medium
source: local
---

# Humanizer

**Owner of style naturalization for existing text.**

## References

- [naturalization-rules.md](references/naturalization-rules.md) — rewrite rules + 5 core speed rules + detection-aware tactics
- [de-aigc-standards.md](../writing-skills/resources/de-aigc-standards.md) — shared universal de-AIGC rules
- [ai-patterns-checklist.md](references/ai-patterns-checklist.md) — 27 AI patterns + CN/EN phrase tables
- [register-presets.md](references/register-presets.md) — 7 register presets (EN + CN)
- [soul-injection.md](references/soul-injection.md) — 11 voice injection techniques
- [detection-mechanics.md](references/detection-mechanics.md) — how detectors work (perplexity, burstiness)
- [adversarial-strategies.md](references/adversarial-strategies.md) — 9 quality-based strategies to lower detection scores
- [claim-safety.md](references/claim-safety.md) — safe wording for detector-related claims and limits
- [examples/before-after.md](examples/before-after.md) — 11 before/after examples

## Routing

| Request type | Route to |
|---|---|
| Generic prose naturalization, 降 AI 味, 降 AIGC 率 | `$humanizer` (this skill) |
| Paper-specific prose revision | `$paper-writing` |
| Scientific logic / novelty / evidence review | `$paper-logic` or `$paper-reviewer` |
| Translation or summarization only | Do not use this skill |

## When to use

- User wants existing text to sound more natural, less robotic, or less templated
- User says "humanize", "去模板腔", "降 AI 味", "降低 AIGC 率", "降 aigc", "去 AI 感", "像人写的", "不像机器", "anti-AI", or "精修" (as a style overlay)
- Draft contains chatbot artifacts, vague praise, filler, repetitive rhythm, or generic conclusions
- Target is a paragraph, email, statement, blog, post, docs snippet, or other prose slice
- User explicitly wants to lower AIGC signals through quality improvement
- End when the point is made — no mandatory wrap-up

## Output Format: Revision Record (修改意见)

When providing a "精修" or "humanizer" result, use this format to explain changes:

### 1. Polished Text
[Deliver the full rewritten text here]

### 2. Revision Record (修改意见)
| Target | Change Made | Strategy/Reason | Logic (Why this works) |
|---|---|---|---|
| [Sentence/Phrase] | [New Version] | [e.g., Rhythm variance] | [e.g., Breaks the 20-word AI repetition; raises burstiness score] |
| [Technical claim] | [Grounded narr.] | [Pragmatic Grounding] | [Replaces sterile 'success' with human 'decision' signal] |

### 3. Remaining Risks / Next Steps
- [Any remaining AI-ness or suggestions for further humanization]

## Do not use

- Main task is scientific review or claims-vs-evidence critique → `$paper-logic` / `$paper-reviewer`
- Task is manuscript prose revision in clear paper context → `$paper-writing`
- Task is only translation or summarization
- User wants entirely new content ghostwritten from zero (no existing text to naturalize)
- User wants fake authorship or fabricated personal experience

## Handoff & Orchestration

This skill acts as the **style naturalization layer**. Once the text is cleaned up, it hands off to:

- **$paper-writing**: For academic papers that need professional scientific polishing after the AI structure is broken.
- **$copywriting**: For marketing/social copy that needs commercial frameworks (AIDA/PAS) after naturalization.
- **$writing-skills**: For repository-level skill documentation consistency.

If the user asks for "精修" (polishing) alongside humanization, perform the humanizer pass first to break the AI patterns, then invoke the downstream skill.
Only consult `references/claim-safety.md` when the user explicitly asks about detector scores, Turnitin, or guarantee-style wording.

## Workflow

A single integrated pipeline. Depth scales automatically — short text does this mentally; long or heavily templated text shows each step explicitly.

### Step 1 — Lock constraints (C.A.R.E. Framework)

- **Context**: Define the specific scholarly or commercial context (e.g., "IEEE conference paper on ML").
- **Audience**: Expert peers vs. general public.
- **Role**: Active researcher vs. neutral observer.
- **Examples**: Identify 1-2 examples of the target "voice" if provided.
- Preserve: facts, citations, numbers, causal logic, domain terminology.
- Identify register: academic / professional / technical / casual / founder / storytelling / social.
- Confirm whether the goal is style naturalization, register matching, or integrity-safe polishing.
- For Chinese text, also check CN subsections in `references/register-presets.md`.

### Step 2 — Scan & diagnose

Scan against `references/ai-patterns-checklist.md` (27 patterns). For Chinese text, also scan the Appendix CN phrase tables.

Prioritize the patterns that most damage the specific text — don't enumerate all 27.
Use `references/claim-safety.md` only when the request explicitly pulls in detectors, scores, Turnitin, or AIGC percentage claims.

### Step 3 — Rewrite

Apply all of these as a **single integrated pass**, not sequential layers:

| Principle | What to do |
|---|---|
| **Specificity** | Replace vague claims with concrete details, numbers, names |
| **Attribution** | Name sources, dates, methods — not "experts say" |
| **Rhythm** | Let sentence lengths vary naturally (Burstiness); break three-item patterns |
| **Clean emphasis** | Remove gratuitous bold, emoji, em dash |
| **Register voice** | Apply the matching preset from `references/register-presets.md` |
| **Soul** | Apply techniques from `references/soul-injection.md` when register allows |
| **Perplexity** | Prefer accurate-but-less-generic word choices; avoid template phrases |
| **Structure** | Break repetitive intro→body→conclusion flow when the source text is already predictable |
| **De-sterilize** | For science: narrate decisions and failures (Adv. Strategy 7) |
| **Synthesis** | Cluster citations to show cross-document reasoning (Adv. Strategy 16) |
| **Controlled Friction**| Use small, authentic asymmetries; keep them tied to real content |
| **Idiosyncrasy** | Use field-specific, less-common collocations that show insider knowledge |

Rules:
- Rewrite long inputs paragraph-by-paragraph, not as a uniform blob.
- Never insert fake anecdotes, fake opinions, or fake sourcing.
- **Quality guard**: never reduce information density, specificity, or professional tone to lower a detector score. If a rewrite trades accuracy for "naturalness", revert.

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

Score against `references/quality-scoring.md` (100pt scale):

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

1. Treat it as a direct rewrite task: make the prose less templated, less symmetric, and more grounded.
2. Push on clarity, specificity, rhythm, register, and local idiosyncrasy instead of talking about detectors.
3. Use detector-aware heuristics only as an internal editing aid when needed.
4. Keep the output focused on the rewritten text and the concrete revision record.

## Overlay compatibility

- Stacks with `$iterative-optimizer` for multi-round deep rewrites.
- `$anti-laziness` auto-activates if rewrite repeats the same patterns across passes.
- When stacked with `$paper-writing` (handoff), this skill provides AI-pattern diagnosis; `$paper-writing` owns final wording.

## Hard constraints

- Never fabricate facts, references, data, experiments, quotes, or personal experience.
- Do not promise detector scores or guaranteed pass outcomes when the user explicitly asks about them.
- Never optimize by making the text worse, less specific, or less accurate.
- Prefer clearer, more specific prose over ornamental rewriting.
- Quality comes first — no rewrite should make the text worse to achieve a detector score.
- Information density of the output must be ≥ the input. Cutting filler is good; cutting substance is not.
