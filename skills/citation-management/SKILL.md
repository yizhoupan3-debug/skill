---
name: citation-management
description: |
  Verify, normalize, de-duplicate, complete, and format academic citations and references
  with a truth-first workflow. Use when the user asks for 文献引用管理、参考文献核查、BibTeX / DOI /
  PMID / Zotero-style cleanup、文中引用与参考文献表一致性检查、APA / IEEE / ACM / GB/T 7714 格式转换, or wants
  citations tightened so claims are supported. At conversation start / first turn, check
  this skill when the main artifact is a bibliography or `.bib` file.
routing_layer: L1
routing_owner: owner
routing_gate: none
session_start: preferred
trigger_hints:
  - 文献引用管理
  - 参考文献核查
  - BibTeX
  - DOI
  - PMID
  - Zotero-style cleanup
  - 文中引用与参考文献表一致性检查
  - APA
  - IEEE
  - ACM
metadata:
  version: "2.1.0"
  platforms: [codex]
  tags:
    - citation
    - bibliography
    - bibtex
    - reference
    - doi
    - academic
---

# Citation Management

## When to use
- The user wants a dedicated workflow for citation truthfulness and reference hygiene.
- The task involves `.bib`, BibTeX, DOI lists, Zotero exports, Word/LaTeX reference lists, manuscript citations, or mixed citation formats.
- The user asks to verify whether cited papers are real, current enough, authoritative enough, journal-first, or consistent with the manuscript.
- The user wants in-text citations checked against the reference list, duplicates removed, metadata completed, or styles converted.
- The user wants claim-to-citation precision improved so references support the exact sentence instead of being dumped in batches.

## Do not use
- The user wants one front door for a research-project task rather than reference hygiene only -> use `$research-workbench`
- The main task is literature review, topic clustering, novelty checking, or related-work synthesis; use `$literature-synthesis` first.
- The main task is manuscript-level review or reviewer-comment execution; use `$paper-workbench` first.
- The main task is rewriting paper prose or reviewer response text; use `$paper-writing` or `$paper-reviser` first.
- The user only wants a generic bibliography explanation with no real citation-management work.

## Cross-references

- `$research-workbench` may route here when the active blocker is citation truth or reference hygiene
- `$paper-reviewer` uses this skill as the primary owner for `G5 Reference Support & Venue Calibration`
- `$paper-reviser` uses this skill when a gate decision changes citation support, appendix routing, or venue-facing reference calibration
- `$paper-writing` may co-invoke for results sections that need claim-to-citation precision
- `$literature-synthesis` handles literature review and synthesis; this skill handles citation formatting and verification

## Core operating promises
1. **Truth over completeness.** Never invent authors, titles, venues, years, pages, DOIs, PMIDs, URLs, or indexing status.
2. **Authority over convenience.** Prefer publisher pages, DOI registries, PubMed, Crossref, official journal pages, and other primary metadata sources before secondary summaries.
3. **Venue-aware source choice.** Prefer the strongest authoritative source for verification, but allow the newest citable public version when the active task is frontier positioning or target-venue calibration.
4. **Exact support over citation dumping.** Citations should support the precise clause, result, method, or claim they are attached to.
5. **Fail closed.** If a reference cannot be verified, flag it as unresolved instead of guessing.

## Paper gate override: G5 Reference Support & Venue Calibration

When this skill is invoked by `$paper-reviewer` or `$paper-reviser` for the
paper gate chain, use these stricter defaults:

- `cluster_limit = 3`
- `recency_window = 3 years` by default; keep older references only when they are canonical and explicitly justified
- `target_venue_proximity = required`
- `citation_precision = claim_or_clause_level`
- `truthfulness = fail_closed`
- `prefer_preprint = true` unless the target venue or field norm clearly prefers the final peer-reviewed version

In this override mode:

- do not allow more than 3 consecutive references in one citation cluster
- verify that each citation cluster supports one narrow claim, not a vague field summary
- calibrate the reference mix to the target venue's recent conversation, not just to the broad topic
- if a claim is narrowed, hidden, moved to appendix, or abandoned in another gate, remove or reroute any citation support that no longer belongs to the surviving main-text claim

## Source priority for verification
Use the strongest available source in this order:
1. Publisher or society page with DOI and full bibliographic metadata
2. DOI resolver / Crossref record
3. PubMed / PMC for biomedical literature
4. Official journal indexing pages or library records
5. Author institutional page or accepted-manuscript page
6. arXiv / bioRxiv / SSRN only when no peer-reviewed version is available or the user explicitly wants preprints

When a preprint and journal version both exist, choose according to the active
mode:

- general verification mode → prefer the peer-reviewed version
- `G5` gate-calibration mode → prefer the newest legitimate citable version that
  matches the target venue norm, often a preprint for fast-moving fields

For style-specific field order and normalization checks, read [references/style-policy.md](references/style-policy.md) when needed.

## Bundled Rust CLI

Prefer the bundled Rust CLI before ad hoc manual checking when the input is already in `.bib`, LaTeX, Markdown, or plain-text manuscript form.

### 1. Reference audit + consistency check

Use `rust_tools/citation_tool_rs` `audit` for:
- BibTeX de-duplication signals
- missing required metadata fields
- likely preprint detection
- missing DOI warnings
- in-text citation key vs bibliography consistency
- dense citation-cluster detection in manuscript text

Example:

```bash
cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/citation_tool_rs/Cargo.toml --bin citation -- audit \
  --bib refs.bib \
  --manuscript draft.tex \
  --fail-on blocking
```

Use `--fail-on blocking` when broken bibliography links or incomplete required metadata should fail the run. Use `--fail-on warnings` for stricter CI-style checks that also fail on preprints, missing DOI, uncited references, or dense citation clusters.

### 2. Claim-to-citation lint

Use `rust_tools/citation_tool_rs` `claim-lint` when the user mainly wants prose-side tightening:
- sentence-ending citation stacks
- dense citation clusters
- places where one sentence likely needs claim-level remapping

Example:

```bash
cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/citation_tool_rs/Cargo.toml --bin citation -- claim-lint \
  --manuscript draft.tex \
  --fail-on-findings
```

Use `--fail-on-findings` when dense or sentence-ending citation clusters should block handoff instead of only being reported.

### 3. Base style rendering

Use `rust_tools/citation_tool_rs` `render` only **after** metadata is normalized. It provides a base formatter for:
- APA
- IEEE
- ACM
- GB/T 7714

This is a **base conversion helper**, not a substitute for final venue-template checking, especially for ACM variants and GB/T 7714 mode selection.

Example:

```bash
cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/citation_tool_rs/Cargo.toml --bin citation -- render \
  --bib refs.bib \
  --style ieee
```

## Practical scope limits

- The bundled automation currently works best for `.bib` plus LaTeX / Markdown / plain-text manuscripts.
- Word-native `.docx` citation fields, Zotero library internals, and EndNote databases may still require export or manual inspection first.
- Numeric citation manuscripts without stable keys can be linted for cluster density, but key-level consistency checking is strongest in key-based workflows such as BibTeX.

## Default workflow

### 1. Build a citation inventory
Normalize the input into one working table with, at minimum:
- reference key or label
- authors
- title
- year
- venue / journal / conference / book
- volume / issue / pages
- DOI / PMID / URL
- document type: journal, conference, preprint, book chapter, thesis, standard, website
- where it appears in the manuscript

If the input is mixed across manuscript text, `.bib`, Word references, screenshots, and notes, consolidate first before editing.

### 2. Verify existence and authority
For each reference:
- confirm the paper is real
- confirm the canonical title, author order, venue, year, and persistent identifier
- check whether a later journal version supersedes a preprint or workshop version
- prefer the authoritative final version unless the `G5` gate-calibration override is active
- mark unverifiable entries explicitly

Never silently keep suspicious entries.

### 3. De-duplicate and merge
Treat entries as duplicates when they resolve to the same DOI, PMID, or clearly the same canonical publication with formatting drift.

Common duplicate patterns:
- preprint + journal version
- abbreviated title + full title
- different author truncation styles
- inconsistent venue names
- duplicate BibTeX keys pointing to the same paper

Merge into one canonical entry and preserve aliases only if the manuscript still needs remapping.

### 4. Complete and normalize metadata
Fill only fields that can be verified. Normalize:
- author names and initials
- title capitalization rules appropriate to target style
- journal and conference names
- year, volume, issue, page range, article number
- DOI format, URL cleanup, PMID/PMCID when relevant
- reference type classification

Do not backfill missing fields by inference alone.

### 5. Check manuscript consistency
Run both directions:

**In-text → reference list**
- every citation callout must resolve to one reference-list entry
- grouped citations must point to distinct intended works
- no broken keys, dangling numbers, or missing entries

**Reference list → in-text**
- flag uncited references unless the user wants a background bibliography
- flag duplicate entries that are cited under different labels

Report mismatches explicitly.

### 6. Tighten citation placement
Default rule: avoid ending one sentence with a stack of loosely related citations unless the sentence is genuinely a broad field-level summary.

Prefer these patterns:
- attach the citation right after the concrete claim it supports
- split one overloaded sentence into two when different claims need different evidence
- use one strong citation instead of many weak adjacent ones when one source fully supports the statement
- group citations only when they support the same narrowly defined claim
- avoid using a citation cluster to hide uncertainty or lack of reading

When revising prose, map citations at the **claim / clause** level, not just the sentence level.

### 6A. Gate-chain calibration checks

When operating in the paper gate chain:

- flag any citation cluster longer than 3 as a gate failure unless a very rare venue-specific convention justifies it
- prefer references from the last 3 years for frontier positioning and claim support
- check whether the selected references are actually close to the target venue's standards, topics, and comparator set
- treat fake, unverifiable, or clause-misaligned citations as decision-level failures, not polish

### 7. Convert to target style
Supported default styles:
- APA
- IEEE
- ACM
- GB/T 7714

When converting styles:
- first normalize metadata canonically
- then render into the target style
- keep identifiers and punctuation consistent with the selected standard
- if some target-style-required fields are missing and cannot be verified, flag the gap instead of fabricating it

## Precision rules for in-text citation quality
Use these as a hard quality bar:
- One citation should answer “why is this exact statement true?”
- If two citations are kept together, they should contribute distinct but tightly aligned support.
- Do not place 3–5 citations at the end of a sentence unless summarizing a well-defined consensus.
- Do not cite a paper for claims outside its actual contribution, dataset, task, or result scope.
- Method papers should support method claims; benchmark papers should support benchmark facts; surveys should support overview claims.
- If a claim is controversial, recent, or non-consensus, prefer the most direct primary source rather than a secondary citation.

## Recommended output shapes
Choose the smallest useful output.

### A. Citation audit report
Use when the user wants verification and cleanup.

Include:
- verified references
- unresolved / suspicious references
- duplicates merged
- missing metadata fields
- uncited or missing-in-list items
- preprint-to-journal replacements

### B. Consistency check report
Use when the user wants manuscript cross-checking.

Include:
- in-text citations with no matching reference entry
- reference entries never cited in text
- numbering or key mismatches
- places where one sentence cites too many sources loosely

### C. Normalized reference list
Use when the user wants a clean bibliography.

Include:
- one canonical reference per work
- normalized metadata
- consistent identifiers
- target style rendering

### D. Citation-placement revision notes
Use when the user wants prose-side improvement.

For each flagged sentence:
- quote or paraphrase the claim briefly
- say which citation is too broad, redundant, or misplaced
- propose a tighter claim-to-citation mapping

## Response discipline
- Be explicit about what was verified versus inferred.
- Prefer “not verified yet” over a polished but possibly false citation.
- If the user asks for new references, prefer newer authoritative journal papers and minimize conference-only or preprint-only suggestions unless the field truly requires them.
- If SCI/SCIE status matters and cannot be confirmed from trusted sources, say so directly instead of assuming based on venue reputation.
- If the user provides a manuscript, inspect citation behavior before rewriting the bibliography in bulk.
