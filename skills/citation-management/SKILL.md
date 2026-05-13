---
name: citation-management
description: Verify and format academic citations and bibliographies.
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
  version: "2.3.2"
  platforms: [supported]
  tags: [citation, bibliography, bibtex, reference, doi, academic]
risk: low
source: local

---

# citation-management

This skill owns reference correctness and style consistency. It makes
citations verifiable, complete, deduplicated, and aligned with the manuscript.

Manuscript workflow context: [`../paper-workbench/references/RESEARCH_PAPER_STACK.md`](../paper-workbench/references/RESEARCH_PAPER_STACK.md).

## When to Use

- The main object is references, a bibliography, `.bib`, DOI list, or citation style.
- The user wants citation verification, de-duplication, metadata completion, or formatting.
- In-text citations need to match the reference list.
- Claims need a quick source-support check at citation level.

## Do Not Use

- Searching and synthesizing a topic literature corpus -> keep this skill only for citation truth; broader source synthesis belongs to `$paper-workbench` for manuscripts or `$research-workbench` for non-manuscript research.
- Writing or polishing manuscript prose -> use `$paper-writing`.
- Checking paper logic beyond citations -> use `$paper-reviewer` logic mode.
- Formatting non-academic documents without citations.

## Truth Rules

- **诚信红线**（不可核验主张、图像诚信、自我剽窃）：[`references/integrity-redlines.md`](references/integrity-redlines.md)；与 `$paper-reviewer` **P0** 口径对齐（致命问题先报、不粉饰）。
- When manuscript context is available, keep citation keys and bibliography
  titles aligned with the frozen terminology in
  [`../paper-workbench/references/research-language-norms.md`](../paper-workbench/references/research-language-norms.md)
  (preferred names for methods/datasets/metrics); do not introduce a second
  naming system in `.bib` that conflicts with in-text terms unless the user
  asked for a rename pass.
- Never invent missing author, title, venue, year, DOI, PMID, or pages.
- **软件 / 数据 / 预印本**：在 `.bib` 或参考表里写清版本、修订、仓库 commit、数据 DOI 或访问条款；不得用模糊「某工具/某数据集」顶替可核对字段。
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
- For issue reviews: issue table with severity, entry, problem, and fix.
- For manuscript consistency: missing-in-text and missing-in-reference lists.
- For verification gaps: unresolved entries with the lookup source attempted.

## References

- [references/style-policy.md](./references/style-policy.md)
- [references/integrity-redlines.md](./references/integrity-redlines.md)
