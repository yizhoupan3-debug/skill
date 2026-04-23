# Sentence-level AIGC risk rubric

Use this note when the user wants **逐句评估**, **逐句判断要不要改**, or wants to see where the text feels most machine-like before any rewrite.

## Research boundary

Sentence-level judgment is a **local heuristic**, not ground truth.

- Turnitin's official model guidance describes AI writing detection as a document/report aid with minimum-length and review constraints, not a sentence-by-sentence truth machine.
- Turnitin also notes that low percentages and short spans are more error-prone, and its public guidance warns about sentence-level false positives.
- Recent papers show paraphrasing and small rewrites can materially shift detector outputs, so local sentence judgments should be treated as review priorities, not proof.

Use sentence judgments to answer:

1. Which sentences deserve human attention first.
2. Whether the text problem is concentrated or distributed.
3. Whether partial editing is enough, or the whole paragraph structure is too uniform.

## Judgment scale

### 1. 不需要改

This sentence is broadly fine and should usually be kept.

Typical signs:
- Specific actor, object, or mechanism
- Natural sentence rhythm
- No stacked transition filler
- No inflated "importance" wording
- Register fits the context

### 2. 需要自然话改写

This sentence has clear machine-like signals, but the core meaning is usable. Usually it needs a lighter, more natural rewrite rather than a full rebuild.

Typical signs:
- One generic transition or one safe-but-vague verb
- Mild symmetry with surrounding sentences
- Slightly abstract wording, but still clear
- Core information is present, but the phrasing is too neat, too safe, or too generic

Default action:
- Rewrite in more natural language while keeping the same information

### 3. 需要完全重写

This sentence is so templated, inflated, or empty that patching word-by-word is usually not worth it.

Typical signs:
- Significance inflation: "具有重要意义", "plays a vital role", "marks a pivotal moment"
- Generic academic boilerplate: "In recent years...", "This study aims to..."
- Low-information abstraction: many abstract nouns, few concrete details
- Predictable clause-comma-clause rhythm
- Safe vocabulary chains: leverage, robust, comprehensive, facilitate, underscores
- Obvious three-item pattern or transition stacking
- Multiple high-risk signals stacked in one sentence
- Heavy meta-discourse about what the text "will discuss" or "seeks to highlight"
- Promotional or motivational filler replacing real content
- Symmetric, over-complete structure that sounds generated rather than written
- Says almost nothing concrete after many words

Default action:
- Rewrite immediately, then re-check neighboring sentences because the paragraph structure is usually also the problem

## Signal checklist

Use 2-4 concrete signals per sentence instead of vague judgments.

### Lexical signals
- Safe abstract verbs: leverage, facilitate, enhance, optimize
- Meaning inflation: crucial, pivotal, transformative, significant
- Empty academic wrappers: aims to, seeks to, is intended to

### Structural signals
- Similar sentence openings repeated 3+ times
- Similar sentence lengths across a paragraph
- Mechanical "first, second, third" or "X, Y, and Z" packing
- Intro-body-conclusion logic compressed into one sentence

### Pragmatic signals
- No real actor or decision-maker
- No concrete detail, evidence, or mechanism
- Over-explains obvious transitions
- Sounds universally safe, not context-aware

### Human signals
- Specific detail, date, number, or named entity
- Honest limit, contrast, or decision rationale
- Natural asymmetry in length or certainty
- Field-specific phrasing that feels used, not generated

## Practical judgment rule

Do not fake percentages. Use a simple rule:

- `不需要改`: no strong signal, or only one weak signal
- `需要自然话改写`: one strong signal or several weak signals, but the sentence still carries usable content
- `需要完全重写`: multiple strong signals stacked together, or the sentence is mostly empty formula

If a sentence feels borderline, judge one level lighter unless the surrounding paragraph shows the same pattern repeatedly.

## Suggested output

| # | Sentence | Judgment | Signals | Why | Action |
|---|---|---|---|---|---|
| 1 | [sentence] | 需要自然话改写 | boilerplate opener; vague abstraction | says little in many words | patch |
| 2 | [sentence] | 不需要改 | concrete data; natural rhythm | specific and grounded | keep |

## Source anchors

- Turnitin AI writing detection model guide: https://guides.turnitin.com/hc/en-us/articles/28294949544717-AI-writing-detection-model
- Turnitin AI Writing Report guide: https://guides.turnitin.com/hc/en-us/articles/22774058814093-Using-the-AI-Writing-Report
- Turnitin false-positive note for sentences: https://www.turnitin.com/blog/understanding-the-false-positive-rate-for-sentences-of-our-ai-writing-detection-capability
- "Paraphrase Me If You Can" benchmark: https://aclanthology.org/2024.findings-emnlp.883/
- SeqXGPT (sentence/sequence-level detection framing): https://aclanthology.org/2024.acl-long.738/
