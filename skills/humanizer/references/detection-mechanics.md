# AI Text Detection — How Detectors Work

A quick-reference on the mechanics behind AI-generated text detectors.
Understanding these helps the rewriter target the **actual signals** detectors rely on.

---

## Core metrics

### Perplexity (predictability)

- Measures how "surprised" a language model is by the next token.
- **AI text → low perplexity**: token choices are statistically safe, uniform, and predictable.
- **Human text → higher perplexity**: humans pick unexpected words, use domain slang, make idiosyncratic phrasing choices.
- **Implication for rewriting**: replacing the "safest" word with a less common but equally accurate synonym raises perplexity naturally.
- **Trap**: don't use rare words that are contextually wrong — accuracy is non-negotiable.

### Burstiness (rhythm variation)

- Measures variance in sentence length and complexity across a passage.
- **AI text → low burstiness**: sentence lengths cluster around a narrow band (e.g., 15–20 words per sentence), paragraph lengths uniform.
- **Human text → high burstiness**: short punchy sentences mixed with long flowing ones; paragraph lengths vary; some ideas get one line, others get five.
- **Implication for rewriting**: let sentences breathe naturally — some very short, some longer — without forcing a mechanical alternating pattern.

### Token probability distribution

- Detectors score each token against the generating model's probability distribution.
- High-probability tokens appearing in sequence = strong AI signal.
- A few low-probability tokens break the chain and reduce the overall AI score.
- **Key insight**: it's not any single word that triggers detection — it's the *consistent stream* of safe choices that accumulates into a score.

### Sentence-level entropy gradient (2024-2025)

- Newer detectors (e.g., RADAR, SemStamp) measure **entropy variation across sentences** within a paragraph.
- AI text maintains near-uniform entropy across sentences because the model's confidence stays consistent.
- Human writing shows entropy spikes: one sentence is certain ("The deadline is Friday."), the next is hedged ("I'm not sure the timeline works.").
- **Implication**: vary the certainty level sentence by sentence — don't maintain uniform confidence throughout.

### Segment-based Context Analysis (2026)

- **Mechanic**: Modern Turnitin analysis uses a sliding window (5-10 sentences) with overlapping segments.
- **Why it matters**: Detection isn't just word-by-word; it's the *rhythmic consistency* across a window.
- **Humanizing tip**: Ensure each 5-sentence block contains at least one "low-probability" structural break (e.g., a short fragment, a citation synthesis, or a perspective shift).

---

## Detection approaches

| Approach | How it works | Weakness |
|---|---|---|
| **Statistical (perplexity + burstiness)** | Compute text-level perplexity and sentence-level variance | Struggles with formal human writing (low perplexity by nature); ESL authors are over-flagged |
| **Classifier-based (fine-tuned BERT/RoBERTa)** | Binary classifier trained on human vs. AI examples | Degrades significantly when text is paraphrased (NeurIPS 2023); not robust across domains |
| **Watermark detection** | Detects statistical watermarks embedded during generation | Only works if the generating model added watermarks; most deployed models don't |
| **Stylometry** | Analyzes function-word frequency, punctuation patterns, avg sentence length | Brittle across registers and domains; easily confused by genre switching |
| **Multiscale PU learning** (ICLR'24) | Learns AI patterns at multiple granularities with unlabeled data | Requires large training corpus; paraphrasing still degrades accuracy by ~20% |
| **Cross-attention pattern analysis** (2025) | Analyzes structural consistency of transition logic and discourse markers | Catches high-level discourse predictability even after local paraphrasing |
| **Semantic fingerprinting** (RAID benchmark, ACL'24) | Measures similarity of generated text to known AI corpora at embedding level | Affected by domain mismatch; struggles with specialized technical prose |
| **Bypasser Detection (Turnitin 2026)** | Specifically trained to identify signatures of "AI Humanizer" tool manipulation (e.g., Ryne, Phrasly) | Brittle against manual structural rewrites; relies on statistical noise patterns |

---

## Known detector weaknesses

1. **Paraphrasing defeats most classifiers** — NeurIPS 2023 showed that a single DIPPER paraphrase pass drops GPTZero/DetectGPT accuracy by 20–40%.
2. **Non-native speakers are over-flagged** — formal, low-perplexity writing by ESL authors is frequently misclassified as AI.
3. **Domain-specific text is unreliable** — legal, medical, regulatory, and academic prose has naturally low perplexity and uniform structure.
4. **Short text (<200 words) is hard to classify** — insufficient statistical signal for reliable classification.
5. **Mixed authorship confuses detectors** — human-edited AI text or AI-assisted human text produces inconsistent, unpredictable scores.
6. **Register-switching disrupts classifiers** — inserting a casual aside into formal prose breaks the expected stylistic signature.
7. **Concrete specifics defeat statistical checks** — named entities, precise numbers, and domain jargon are low-probability tokens that disrupt AI signals without harming quality.
8. **20% Noise Threshold (Turnitin)** — As of 2026, Turnitin marking scores below 20% with an asterisk (*) suggests that reaching "low-risk" (rather than zero) is the viable target for academic integrity.

---

## What detectors actually measure (summary)

```
AI signal = low perplexity
          + low burstiness
          + uniform token probabilities
          + consistent sentence-level entropy (uniform confidence)
          + predictable discourse structure (intro→body→conclusion)
          + high-frequency "safe" vocabulary
          + predictable transition patterns
          + consistent paragraph lengths
```

Effective rewriting should **naturally** disrupt these signals through better, more specific, more human prose — not through artificial tricks.

---

## The quality-first principle

The most robust counter to AI detection is **genuinely better writing**:
- Specific claims > vague generalizations (raises perplexity)
- Varied certainty levels > uniform hedging (disrupts entropy pattern)
- Concrete nouns > abstract nouns (lower token probability, higher specificity)
- Domain expertise > generic safe language (raises perplexity, adds trust)
- First-person judgment > neutral reporting (breaks discourse predictability)

No rewrite should trade quality for a lower score. If the rewrite is worse, the detection score doesn't matter.
