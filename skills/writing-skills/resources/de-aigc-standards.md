# Core De-AIGC Standards

Universal rules for reducing AI structural signals and naturalizing prose.

## 1. Meta-Discourse Ban (消除元话语)
Do not describe the document's structure or the AI's actions.
- **Avoid**: "This section presents...", "In the following paragraphs...", "Next, we will show...", "It is important to note that".
- **Prefer**: Direct claims. Instead of "This section presents the results," use "Results demonstrate that...".

## 2. Transition Adverb Budget (连接词管控)
AI tends to over-signpost logical flow with transitional adverbs.
- **Quota**: Max ONE instance of `Notably`, `Moreover`, `Furthermore`, `Overall`, `此外`, `更重要的是` per 200 words.
- **Cure**: Let the logical order of sentences carry the flow. If a connection is needed, use causal verbs or subject-based transitions.

## 3. The 3-Sentence Variance Rule (节奏多样性)
Avoid the "AI drone" (monotonous sentence lengths). In every paragraph:
- **Short**: At least one sentence <10 words for impact.
- **Complex**: At least one sentence >25 words (or with sub-clauses) for nuance.
- **Start**: Vary sentence openings (don't start every sentence with the same subject).

## 4. Specificity over Significance (去意义膨胀)
AI inflates importance with vague adjectives.
- **AI Words**: pivotal, testament, underscores, tapestry, multifaceted, 具有重要意义, 标志着...的转折.
- **Human Fix**: Replace with the actual mechanism, data point, or concrete outcome.

## 5. Active Voice & Agency (主体感)
AI often sounds like a neutral observer. Inject agency by identifying who did what.
- **Instead of**: "A study was conducted..."
- **Use**: "We measured..." or "Researchers isolated..."

## 7. Pragmatic Grounding (经验锚定)
AI writing follows a "textbook" logic. Human experts narrate the *experience* of research.
- **Decisions & Rationales**: Explain *why* a choice was made (e.g., "To avoid the OOM error seen in preliminary tests, we...").
- **Exclusions & Failures**: Mention what didn't work. AI is always successful; humans describe data cleanup and rejected trials.
- **Narrative Flow**: Use chronological or logical friction ("Initially, we expected X, but the reality was Y").

## 8. Adaptive Imperfection (非对称精确)
AI prose is "asymptotically perfect." Human writing has "precise friction."
- **Avoid Over-Polishing**: Sometimes a slightly clunky but highly specific technical term is more "human" than a smooth, generic one.
- **Parenthetical Nuance**: Use asides to show real-time thinking: "(though this only applies to X)" or "(notably, the baseline here is debatable)".
- **Sentence-level Asymmetry**: Break the "Clause-Comma-Clause" pattern. Mix a 40-word technical explanation with a 4-word definitive claim.

## 6. Vocabulary replacement (CN/EN)

| AI Pattern | Suggested Replacement |
|---|---|
| leverage / utilize | use / apply / employ |
| enhance / optimize | improve / sharpen / strengthen |
| comprehensive / robust | detailed / thorough / stable |
| 打造 / 赋能 | 做 / 构建 / 支撑 |
| 显著提升 / 有效解决 | [具体提升百分比] / [具体解决方式] |
