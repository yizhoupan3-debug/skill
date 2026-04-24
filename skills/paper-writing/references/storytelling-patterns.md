# Research Storytelling Patterns

Use this when the user asks for 科研讲故事, 论文故事线, target-journal style, or ref-guided rewriting. The goal is to make the reader understand why this paper had to exist, not to add drama or oversell the evidence.

## Story Spine

Every manuscript needs a spine the reader can repeat:

```text
Field wants [goal], but [specific bottleneck] prevents it under [scope]. Existing work solves [nearby thing] but not [this gap] because [reason]. We address this by [paper move]. The evidence shows [bounded result], which matters because [implication].
```

If any slot is missing, do not hide it with fluent prose. Mark it with `[VERIFY: ...]` or hand back to `$paper-logic`.

## Storyline Diagnosis

Before rewriting, classify the current manuscript problem:

| Problem | Symptom | Repair |
| --- | --- | --- |
| No tension | The text says the topic is important but not why current work fails | Name the bottleneck and consequence |
| Generic gap | "Few studies..." without a reason | Explain why the missing study matters |
| Claim/evidence mismatch | Introduction promises more than experiments prove | Narrow claim or route to `$paper-logic` |
| Methods-first story | Paper starts with technique before reader sees the need | Move problem and constraint before method |
| Literature dump | Related work lists papers without contrast | Cluster by gap and end with positioning |
| Result list | Results report tables without a takeaway path | Order evidence by contribution claims |

## Ref-Guided Writing Workflow

When `$literature-synthesis` provides a target-journal ref-learning brief:

1. Extract the venue's common story norm: opening move, gap type, contribution posture, evidence sequence, limitation placement.
2. Map the user's manuscript facts onto that norm.
3. Choose one story angle; do not mix several weak angles.
4. Draft the section using the user's facts only.
5. Match architecture and claim tone, not sentence wording.
6. Check that every borrowed rhetorical pattern is supported by the user's actual evidence.

## What to Learn From Refs

Learn:

- how abstracts allocate words across context, gap, method, result, implication
- how introductions narrow from field pain to a precise missing capability
- how related work clusters competitors and creates the "why us" contrast
- how methods papers separate novelty from implementation detail
- how results sections order evidence to support contribution claims
- how discussion sections admit scope without weakening the core contribution
- which baselines, metrics, and limitations are normal for the target venue
- how much space accepted papers spend before stating the contribution
- whether accepted papers foreground mechanism, application value, methodological novelty, or validation depth

Do not learn by copying:

- sentence templates with distinctive wording
- unsupported novelty claims
- citation clusters without reading support
- hype level that the user's evidence cannot defend

## Story Angles

Pick one dominant angle:

| Angle | Use when | Risk |
| --- | --- | --- |
| Bottleneck removal | Field has a clear blocker | Must prove the blocker is real |
| Method transfer | Known method works in a new setting | Novelty may look incremental |
| Mechanism clarification | Paper explains why something works | Needs convincing analysis |
| Resource or benchmark | Paper contributes data/tool/evaluation | Must justify reuse value |
| Robustness/generalization | Paper tests under harder conditions | Needs fair comparison |
| Practical deployment | Paper improves real-world usability | Needs cost, reliability, or workflow evidence |

If two angles seem equally attractive, choose the one best supported by the current evidence, not the one that sounds more impressive.

## Output Shapes

For a story-only pass, return:

```markdown
## Story Spine
[One-sentence spine]

## Main Repair
[The single biggest story problem and the fix]

## Section Moves
- Abstract: ...
- Introduction: ...
- Related work: ...
- Results/Discussion: ...
```

For a ref-guided rewrite, return:

```markdown
## Ref-Learned Pattern
[Target journal story norm in 3-5 bullets]

## Our Paper's Story Choice
[Chosen angle + why the evidence supports it]

## Revised Text
[Publication-ready draft]

## Guardrails
- [VERIFY: missing fact/citation/result if any]
```

For simple paragraph rewriting, return only the revised paragraph unless a claim/evidence risk remains.

## Story Memory Card

For long manuscript work, keep a tiny story card and reuse it across sections:

```markdown
## Story Card

- Target venue:
- Reader:
- Core problem:
- Specific bottleneck:
- Why now:
- Our move:
- Evidence ceiling:
- Main comparator:
- Contribution posture:
- One-sentence spine:
- Forbidden claims:
```

Every rewritten section should agree with this card. If a section needs a stronger claim than the card allows, stop and route to `$paper-logic` instead of silently upgrading the prose.

## Section Story Jobs

- Abstract: one-screen version of the whole story.
- Introduction: make the reader care about the gap and believe the paper's move is necessary.
- Related work: show that the gap survives the closest literature, not that the author read many papers.
- Method: make the proposed move understandable and reproducible enough to trust.
- Results: prove the promised claims in the order readers need evidence.
- Discussion: explain meaning, scope, and failure modes without adding new results.
- Conclusion: leave one bounded takeaway, not a grand claim.

## Gap Types

Name the gap precisely:

- capability gap: existing methods cannot do X
- setting gap: X is untested under condition Y
- evidence gap: prior claims lack controlled evidence
- mechanism gap: outcomes are known but reasons are unclear
- integration gap: components exist but have not been combined under the needed constraint
- translation gap: lab method exists but deployment/reproducibility/usability blocks adoption

Vague gaps such as "few studies have investigated" are weak unless paired with a reason the absence matters.

## Contribution Posture

Choose posture carefully:

- **Introduce** only when the paper truly proposes a new method, dataset, framework, or theory.
- **Extend** when the work adapts an existing idea to a harder or underexplored setting.
- **Validate** when the value is stronger evidence, broader testing, or independent confirmation.
- **Explain** when the contribution is mechanism, interpretation, or theory.
- **Operationalize** when the contribution is making an idea usable, reproducible, scalable, or deployable.

Do not use "novel", "first", or "state-of-the-art" unless the ref corpus and evidence support that posture.

## Abstract Story Ratios

For ref-guided abstract work, learn the venue's ratio first. If no better venue signal exists, use this default:

| Move | Share | Job |
| --- | ---: | --- |
| Context | 10-15% | Tell the reader what field problem matters |
| Gap/tension | 20-25% | Name the precise missing capability or evidence |
| Method/move | 20-25% | State what this paper does |
| Results | 25-35% | Give the strongest bounded evidence |
| Implication | 10-15% | Explain why the result matters within scope |

Do not let context consume the abstract. If the target journal's accepted papers use a different ratio, follow the corpus.

## Introduction Story Ladder

A strong introduction usually climbs down this ladder:

1. Field need: what community goal matters?
2. Operational bottleneck: what blocks that goal in practice?
3. Prior-work boundary: what nearby work solves, and where it stops?
4. Consequence: why the remaining gap matters.
5. Paper move: what this paper changes.
6. Evidence preview: what proof the reader will see.
7. Contribution list: what the paper adds, each mapped to evidence.

If the introduction jumps from 1 to 5, the story feels like a sales pitch. If it stays at 2-3 too long, it becomes a literature review.

## Before / After Repair

Weak story:

```text
Many methods have been proposed for X. However, they still have limitations. Therefore, we propose Y.
```

Stronger story:

```text
X is useful only when [condition]. Existing methods handle [nearby condition], but they fail under [target condition] because [specific mechanism]. This leaves [consequence]. We introduce Y to [specific move], and evaluate it on [evidence scope].
```

## Final Checks

- Can the story be summarized in one sentence without losing the contribution?
- Does the intro gap match the actual experiments?
- Do contribution bullets map to figures, tables, theorems, or analyses?
- Does the wording explain why the paper is needed, not merely what was done?
- Does the target-journal ref corpus support this story shape?
- Is the chosen story angle visible in abstract, introduction, results, and conclusion without contradiction?
