---
name: citation-management
description: |
  Verify, normalize, de-duplicate, complete, and format academic citations and
  references with a truth-first workflow. Use for 参考文献核查, BibTeX/DOI/PMID
  cleanup, Zotero-style normalization, in-text/reference consistency, or APA /
  IEEE / ACM / GB/T 7714 formatting. Check early when the artifact is a bibliography or `.bib`.
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
  - GB/T 7714
metadata:
  version: "2.2.0"
  platforms: [codex]
  tags: [citation, bibliography, bibtex, reference, doi, academic]
risk: low
source: local
---

# citation-management

This skill owns reference correctness and style consistency. It makes
citations verifiable, complete, deduplicated, and aligned with the manuscript.

## When to Use

- The main object is references, a bibliography, `.bib`, DOI list, or citation style.
- The user wants citation verification, de-duplication, metadata completion, or formatting.
- In-text citations need to match the reference list.
- Claims need a quick source-support check at citation level.

## Do Not Use

- Searching and synthesizing a topic literature corpus -> use `$literature-synthesis`.
- Writing or polishing manuscript prose -> use `$paper-writing`.
- Checking paper logic beyond citations -> use `$paper-logic` or `$paper-reviewer`.
- Formatting non-academic documents without citations.

## Truth Rules

- Never invent missing author, title, venue, year, DOI, PMID, or pages.
- Mark unverifiable fields instead of guessing.
- Preserve citation keys unless the user asks to rename them.
- Keep style formatting separate from factual metadata.
- Use current external lookup when citation metadata may be incomplete or stale.
- Treat publisher metadata and DOI records as stronger than copied reference text.
- Keep unresolved ambiguity visible in the output.

## Workflow

1. Identify the citation style and input format.
2. Parse all entries and detect duplicates, missing fields, and malformed records.
3. Verify high-risk records through DOI, PMID, Crossref, publisher pages, or trusted indexes.
4. Normalize names, titles, venues, years, pages, identifiers, and capitalization.
5. Check in-text citations against the reference list when manuscript text is available.
6. Return cleaned entries plus unresolved items.

## Output Defaults

- For `.bib`: corrected BibTeX and a short unresolved list.
- For reference lists: formatted references in the requested style.
- For audits: issue table with severity, entry, problem, and fix.
- For manuscript consistency: missing-in-text and missing-in-reference lists.
- For verification gaps: unresolved entries with the lookup source attempted.

## References

- [references/style-policy.md](./references/style-policy.md)
