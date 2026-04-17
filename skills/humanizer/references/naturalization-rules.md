# Naturalization Rewrite Rules

Use this file when the user wants to rewrite existing text so it sounds more natural, concrete, and author-driven.

## Goal

Produce prose that reads like a careful person wrote it: specific, grounded, readable, and appropriately voiced for the context.

## Non-negotiable rules

- preserve facts, references, quantities, and causal logic unless the user asked for substantive changes
- keep domain terms precise and stable
- rewrite beyond synonym swapping; fix sentence shape and information flow
- prefer evidence-linked claims over broad praise
- do not invent anecdotes, feelings, citations, or examples

## Common patterns to reduce

- inflated importance claims without nearby evidence
- vague attributions such as “experts believe” or “observers note”
- filler transitions that add no content
- repeated sentence lengths across a paragraph
- rigid three-item rhetorical patterns used mechanically
- decorative bullets, emojis, or bold labels that interrupt reading
- generic conclusions that say little beyond “the future looks bright”
- chatbot residue such as “great question” or “I hope this helps”

## Rewrite tactics

### 1. Re-anchor claims

Prefer:
- the mechanism
- the dataset
- the comparison target
- the concrete outcome

Over:
- abstract praise
- vague importance
- broad “impact” language

### 2. Vary cadence naturally

Mix shorter factual sentences with longer interpretive ones when needed.

Avoid paragraphs where every sentence has the same rhythm.

### 3. Use exact verbs

Prefer verbs that reflect actual action:
- quantify
- isolate
- characterize
- validate
- compare
- constrain
- fail
- degrade

Over generic verbs like:
- improve
- optimize
- enhance
- leverage

### 4. Remove scaffolding

Cut phrases that mostly manage conversation rather than carry information:
- “It is worth noting that”
- “In order to”
- “At its core”
- “It can be seen that”

### 5. Keep authentic restraint

Good prose often:
- acknowledges limits
- avoids certainty inflation
- names tradeoffs explicitly
- says less when less is warranted

## 3-Pass Self-Audit Protocol (三段式自检协议)

Before delivering, run this 3-pass self-audit mentally (or explicitly for long texts).

### Pass 1: The Micro-Level (Words & Sentences)
- [ ] Scan for repeated sentence stems.
- [ ] Cut empty intensifiers and "AI vocabulary" (e.g. 显著提升, 具有重要意义, empower, robust).
- [ ] Replace vague praise with specifics.

### Pass 2: The Macro-Level (Structure & Rhythm)
- [ ] Check paragraph transitions. If >2 paragraphs start with transitional adverbs (Furthermore, 此外), delete them.
- [ ] Ensure sentence lengths vary naturally (high burstiness).
- [ ] Remove generic opening paragraphs and wrap-up conclusions that add zero factual value.

### Pass 3: The Context-Level (Turnitin/Register)
- [ ] **Academic**: Did I un-cluster the citations? Did I make the methodology sound like a real lab/field report instead of a textbook? 
- [ ] **Commercial**: Are there fake-enthusiastic emojis or over-promising claims?
- [ ] **General**: Confirm nothing was fabricated for style.

---

## 5 core rules (quick reference)

When short on time, remember these 5 rules (adapted from Humanizer-zh):

1. **Delete filler phrases** — remove openers and emphasis crutches
2. **Break formulaic structure** — avoid binary contrasts, dramatic segmenting, rhetorical setups
3. **Vary rhythm** — mix sentence lengths; two items beats three; vary paragraph endings
4. **Trust the reader** — state facts directly, skip softening, justification, and hand-holding
5. **Delete quotable lines** — if it sounds like an inspirational poster, rewrite it

---

## Detection-aware rewriting

These tactics address the specific signals AI detectors measure.
For full detector mechanics, see [detection-mechanics.md](detection-mechanics.md).
For the full strategy list, see [adversarial-strategies.md](adversarial-strategies.md).

### Raise perplexity naturally

- Replace the "safest" synonym with an equally accurate but less common one
- Use domain-specific terminology that a generic LLM would not select
- Prefer concrete nouns over abstract ones (concrete nouns have lower token probability)

### Let rhythm emerge organically

- Don't force a pattern — just avoid the AI default of 15-20 word sentences throughout
- Let some sentences be very short when the content warrants it
- Let complex ideas stretch across longer sentences when clarity requires it
- Paragraph lengths should vary too — not every paragraph needs to be 3-5 sentences

### Disrupt structural predictability

- Don't always open with context → body → conclusion
- Some paragraphs can start with the conclusion and then explain why
- Subheadings are optional — flowing prose is fine for short passages
- End when the point is made — no mandatory wrap-up

