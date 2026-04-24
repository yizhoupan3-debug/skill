# Target-Journal Reference Corpus

Use this workflow when the user wants to download or prepare refs before writing, especially for a target journal. The goal is not "more papers"; it is a compact corpus that teaches what the target venue accepts as a convincing story.

## Default Corpus Shape

Build about 20 close references:

| Bucket | Count | Purpose |
| --- | ---: | --- |
| Target-journal near neighbors | 12 | Learn venue-specific story shape, section balance, evidence style, title/abstract patterns |
| Closest competitors | 4 | Identify required baselines, novelty risks, and reviewer expectations |
| Canonical anchors | 4 | Ground the field history and prevent shallow positioning |

Adjust counts only when the target journal is too sparse, the field is very new, or the user already has a strong corpus.

The number 20 is a working default, not a quota. Stop at fewer papers if the nearest set is already saturated; expand beyond 20 only when the target journal has multiple subgenres that would otherwise be mixed together.

## Search Inputs

Extract or ask for:

- target journal or venue
- manuscript topic, method, data domain, and claimed contribution
- nearest keywords and synonyms
- expected article type: original research, review, methods paper, case study, short communication
- time window, defaulting to the last 3-5 years for near-neighbor papers
- whether preprints are acceptable in the field

## Retrieval Rules

- Prefer official journal pages, DOI pages, PubMed/PMC, arXiv, institutional repositories, or publisher-provided PDFs.
- Do not bypass paywalls or use unauthorized downloads.
- Keep source URLs and DOI metadata with each paper.
- Deduplicate preprint/journal versions and mark which version is used.
- If PDFs cannot be obtained, keep metadata and abstract, then mark `[NEEDS PDF]`.
- If full text is inaccessible, do not invent section details from abstract-only evidence.

## Candidate Funnel

Use a three-stage funnel:

1. **Collect 50-80 candidates** from target journal search, citation graph expansion, and keyword searches.
2. **Shortlist 25-30 candidates** by title/abstract/venue closeness.
3. **Retain about 20 papers** after checking contribution type, evidence style, and story usefulness.

Do not let citation count dominate the funnel. A recent target-journal paper with the same article type can teach more about writing than a highly cited but stylistically distant paper.

## Keep / Reject Criteria

Keep papers that match at least two of:

- same target journal or sister journal
- same problem or use case
- same method family
- same data type, population, benchmark, organism, material, or application setting
- same evidence style: experiments, simulations, case studies, theory, clinical validation, field trial
- directly comparable contribution claim

Reject or downgrade papers that are:

- only keyword-overlapping but not problem-overlapping
- too old unless canonical
- review-only when the manuscript is original research and story norms differ
- weakly sourced, unverifiable, or non-primary

## Reference Inventory Table

Use this table as the working artifact:

| # | Paper | Year | Venue | Bucket | Why kept | Story role | Evidence style | Must cite? | PDF/source |
| --- | --- | ---: | --- | --- | --- | --- | --- | --- | --- |
| 1 | ... | ... | ... | Target-neighbor | ... | gap model / abstract template / baseline | ... | yes/no | DOI/PDF |

## Story Extraction Pass

For each retained paper, extract only reusable writing intelligence:

- title pattern: object + method + outcome, mechanism + setting, or problem + solution
- abstract moves: context, gap, method, result, implication
- introduction funnel: broad problem -> specific bottleneck -> why existing work fails -> proposed move
- novelty posture: first/novel vs extension/application/resource/validation
- evidence promise: what result type the paper needs to be believable
- figure/table order: what appears first and what reviewers are trained to expect
- limitation style: where and how scope is narrowed
- key phrases worth emulating at the structural level, not copying sentence wording

Use this extraction table when the corpus is meant to guide writing:

| Paper | Opening move | Gap type | Contribution posture | Evidence order | Limitation placement | Reusable lesson |
| --- | --- | --- | --- | --- | --- | --- |
| ... | field pain / method bottleneck / application need | capability / setting / evidence / mechanism | first / extension / validation / resource | result -> ablation -> failure case | discussion / end of intro / limitations section | ... |

## Pattern Synthesis

After reading individual papers, synthesize patterns across the corpus:

- **Common opening move**: what problem frame appears repeatedly?
- **Accepted novelty posture**: does the venue reward new methods, careful validation, resources, mechanisms, or translation?
- **Evidence sequence**: what evidence usually appears before the strongest claim?
- **Baseline expectation**: which comparators or controls are treated as obvious?
- **Language ceiling**: how aggressively do accepted papers claim novelty and impact?
- **Limitation norm**: do papers state limitations early, in discussion, or in a dedicated section?

This pattern layer is the part `$paper-writing` should use most.

## Output: Ref-Learning Brief

After building the corpus, produce a short brief that `$paper-writing` can use:

```markdown
## Target-Journal Ref Corpus

Target venue: ...
Corpus: 20 papers retained from ... candidates

## Venue Story Norm

- Common opening move:
- Typical contribution posture:
- Expected evidence sequence:
- Common abstract shape:
- Usual limitation placement:

## Must-Match Expectations

- Baselines/comparators:
- Metrics/evidence:
- Terminology:
- Citation anchors:

## Writing Implications for Our Paper

- Best story angle:
- Claims to avoid:
- Sections needing strongest alignment:
- Phrases/templates to emulate structurally, not copy:
```

## Handoff to Paper Writing

Send `$paper-writing` the ref-learning brief plus the user's manuscript facts. The writing lane should use the corpus for story architecture, section balance, claim tone, and venue fit, but must not copy sentences, invent citations, or make the user's evidence stronger than it is.

## Local Artifact Layout

When the user is doing a real manuscript project in the filesystem, prefer a small, repeatable folder shape:

```text
refs/
  candidates.tsv
  retained.tsv
  pdf/
  notes/
  ref_learning_brief.md
```

Minimum file contracts:

- `candidates.tsv`: all found candidates with title, year, venue, DOI/source, bucket guess, and keep/reject status.
- `retained.tsv`: the final about-20 paper set with why-kept, story role, evidence style, must-cite flag, and local PDF path if available.
- `notes/<slug>.md`: one short note per retained paper, focused on story and evidence norms, not a full summary.
- `ref_learning_brief.md`: the only artifact that should usually be handed to `$paper-writing`.

For paper notes, use this compact template:

```markdown
# [Short Paper Title]

- Bucket:
- Why kept:
- Opening move:
- Gap type:
- Contribution posture:
- Evidence order:
- Baselines/controls:
- Limitation placement:
- Reusable lesson:
- Do not copy:
```

Keep filenames stable and boring: `year-first-author-keyword.md` and `year-first-author-keyword.pdf`.

## Failure Modes

- **Ref hoarding**: collecting many PDFs but not extracting the venue story norm.
- **Keyword drift**: retaining papers that share words but not the same problem or evidence style.
- **Citation padding**: adding papers to look comprehensive without mapping them to claims.
- **Style mimicry**: copying phrases instead of learning architecture.
- **Overfitting to one paper**: imitating a single article's structure when the corpus shows multiple subgenres.
