# Ref-First Paper Writing Workflow

Use this when a manuscript should be shaped by target-journal examples before review or rewriting. The workflow is meant to be light enough for interactive use and stable enough for multi-turn manuscript work.

## Purpose

Turn a target venue and rough manuscript facts into a venue-calibrated story:

```text
target journal -> close ref corpus -> venue story norm -> our story card -> section rewrite plan -> prose
```

This is not a citation-hunting workflow. The corpus teaches story architecture, evidence expectations, contribution posture, and claim tone.

## When To Start Here

- The user says "先下载/整理 ref 再写".
- The user wants target-journal style or "科研讲故事".
- The paper has facts/results, but abstract or introduction feels generic.
- The user asks to rewrite a manuscript for a specific journal.
- A review found that the paper is hard to position even if the experiments may be usable.

## Lane Order

1. `$literature-synthesis`: build the target-journal reference corpus and `ref_learning_brief.md`.
2. `$paper-logic`: only if the corpus reveals novelty, baseline, or claim/evidence risk.
3. `$paper-writing`: build the story card, section rewrite plan, and revised prose.
4. `$citation-management`: final citation truth, `.bib`, and claim-to-citation cleanup.

Do not start with `$citation-management` unless the active problem is bibliography hygiene. Do not start with `$paper-writing` if the venue story norm is unknown and the user explicitly asked to learn refs first.

## Minimal Artifact Layout

```text
refs/
  candidates.tsv
  retained.tsv
  notes/
  pdf/
  ref_learning_brief.md
paper_story/
  STORY_CARD.md
  SECTION_REWRITE_PLAN.md
  VERIFY.md
```

If the task is quick and chat-only, keep these as headings in the response instead of files.

## Ref Learning Brief Contract

`refs/ref_learning_brief.md` should answer:

- What target venue and article type are being imitated?
- Which about-20 papers were retained, and why?
- What opening moves recur?
- What gap types does the venue accept?
- What contribution postures are common: introduce, extend, validate, explain, operationalize?
- What evidence order appears before strong claims?
- What baselines, metrics, or limitations are treated as mandatory?
- What wording ceiling should the user's paper not exceed?

## Story Card Contract

`paper_story/STORY_CARD.md` should be short:

```markdown
# Story Card

- Target venue:
- Reader:
- Core problem:
- Specific bottleneck:
- Why now:
- Prior-work boundary:
- Our move:
- Evidence ceiling:
- Main comparator:
- Contribution posture:
- One-sentence spine:
- Forbidden claims:
```

This card is the single source of truth for abstract, introduction, discussion, and conclusion rewriting.

## Section Rewrite Plan

`paper_story/SECTION_REWRITE_PLAN.md` should map the story into sections:

| Section | Job | Current problem | Rewrite move | Evidence needed | Risk |
| --- | --- | --- | --- | --- | --- |
| Abstract | Miniature paper | ... | ... | ... | ... |
| Introduction | Gap + contribution | ... | ... | ... | ... |
| Related work | Positioning | ... | ... | ... | ... |
| Results | Evidence path | ... | ... | ... | ... |
| Discussion | Meaning + limits | ... | ... | ... | ... |

## Stop Conditions

Stop and route to `$paper-logic` when:

- the desired story requires a claim stronger than the results support
- the ref corpus shows a missing obvious baseline or competitor
- the paper's novelty becomes unclear after reading close refs
- the chosen contribution posture depends on evidence not yet present

Stop and route to `$literature-synthesis` when:

- the retained corpus is too weak, off-topic, or not target-venue-like
- the target journal has multiple subgenres and the current corpus mixes them
- the user asks for more references, novelty check, or related-work synthesis

Stop and route to `$citation-management` when:

- the active problem is fake, stale, duplicate, or imprecise citations
- the section is already written but claim-level citation support is uncertain

## Hard Rules

- Learn architecture, not sentences.
- Keep the user's evidence ceiling fixed.
- Do not cite all 20 papers just because they were collected.
- Do not imitate a review article when the user is writing original research.
- Do not hide missing evidence with more polished prose.
