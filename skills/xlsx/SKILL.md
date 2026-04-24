---
name: xlsx
description: |
  Read, create, edit, repair, render, and review Excel `.xlsx` workbooks when
  formulas, formatting, workbook structure, or print layout must be preserved.
  Use after the `$spreadsheets` gate when the task needs a workbook-native
  compatibility lane backed by the Rust OOXML CLI and LibreOffice render checks.
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - xlsx
  - excel
  - workbook structure audit
  - formula format repair
  - keep formulas and formatting
  - render workbook
runtime_requirements:
  commands:
    - cargo
    - soffice
    - pdftoppm
metadata:
  version: "2.0.0"
  platforms: [codex]
  tags:
    - xlsx
    - excel
    - spreadsheet
    - rust
    - libreoffice
framework_roles:
  - executor
  - verifier
framework_phase: 2
framework_contracts:
  emits_findings: true
  consumes_findings: false
  emits_execution_items: false
  consumes_execution_items: false
  emits_verification_results: true
risk: low
source: local
allowed_tools:
  - shell
  - rust
approval_required_tools:
  - file overwrite
filesystem_scope:
  - repo
  - artifacts
network_access: conditional
artifact_outputs:
  - workbook_review.md
  - EVIDENCE_INDEX.json
---

# xlsx

This skill owns the Rust-first compatibility lane for `.xlsx` work. Use it when
the workbook path should stay workbook-native and needs formula, style, table,
validation, sheet-state, or rendered print-layout awareness.

## Routing note

Generic Excel / spreadsheet intake should hit `$spreadsheets` first. This skill
takes over when workbook preservation, compatibility repair, or render-aware QA
matters more than a generic spreadsheet artifact path.

## Rust CLI quick path

Use these as the default inspection and verification commands:

```bash
cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/ooxml_parser_rs/Cargo.toml -- xlsx <workbook>
cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/ooxml_parser_rs/Cargo.toml -- xlsx <workbook> --json
cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/ooxml_parser_rs/Cargo.toml -- render-xlsx <workbook> --outdir <dir> --png
```

`xlsx` reports sheet order, visibility, dimensions, formulas, merged ranges, freeze panes, filters, tables, validations, conditional formatting, charts, images, print areas, defined names, and external links. `render-xlsx` produces PDF/PNG output for print-layout review.

## When to use

- The task involves an `.xlsx` workbook as the main artifact
- The user wants to create, read, edit, review, repair, or audit Excel files
- The task depends on workbook-native features such as formulas, styles, merged cells, filters, tables, named ranges, data validation, conditional formatting, hidden sheets, or print settings
- The user wants to preserve Excel behavior rather than converting straight to CSV
- Layout-sensitive review matters, such as print ranges, clipped headers, or visual sheet QA
- Best for requests like "检查这个工作簿结构", "修公式但保留格式", or "渲染这个 Excel 看分页"

## Do not use

- Generic Excel / spreadsheet requests with no explicit engine choice yet - use `$spreadsheets` first
- The real task is plain CSV or dataframe analysis with no workbook features in scope
- The artifact is `.docx` or PDF rather than Excel
- The user only pasted plain text and wants rewriting
- The file is legacy `.xls` and the task explicitly requires BIFF-era tooling; call out the format mismatch first
- The task is a slide deck or Word-document workflow

## Task ownership and boundaries

This skill owns:
- `.xlsx` workbook reading, generation, and repair
- structure-preserving sheet edits
- formulas, styles, tables, named ranges, validations, and workbook-feature-aware review
- render-aware spreadsheet QA when sheet appearance matters
- safe export of helper views such as CSV snapshots without losing the workbook source

This skill does not own:
- generic workbook intake when the implementation lane is still undecided
- pure dataframe/statistical analysis detached from the workbook artifact
- legacy `.xls` restoration as a primary workflow
- database modeling or SQL query design
- presentation/document authoring

If the task shifts to adjacent skill territory, route to:
- `$sql-pro` for database-side query work
- `$visual-review` for image-grounded review of already-rendered sheet screenshots
- `$doc` for Word artifacts
- `$pdf` for PDF-first artifacts

## Required workflow

1. Confirm this really should not stay on the default `$spreadsheets` artifact-tool path.
2. Identify whether the task is inspect, create, edit, repair, export, or audit.
3. Preserve the workbook as the source of truth; do not flatten to CSV unless that is explicitly the deliverable.
4. Use the Rust CLI for structure inspection and render handoff.
5. Recheck formulas, styles, and sheet-level features after meaningful changes.
6. If layout matters, render and visually inspect the affected sheets.

## Integrity checklist

- Compare before/after sheet names, order, visibility, and used ranges.
- Confirm formulas still exist where expected.
- Confirm merged ranges, tables, filters, validations, and conditional formatting were not dropped.
- Confirm print areas and page rendering are plausible when layout matters.
- Treat `.xlsm` as macro-sensitive and call out macro preservation risk before writing.

## Common high-risk spreadsheet defects

- formulas replaced by static values unintentionally
- broken references after row/column insertion or sheet renaming
- hidden sheets accidentally exposed or removed
- merged cells split by naive writes
- styles or number formats lost after a tabular rewrite
- filters, tables, or validations disappearing after a rewrite
- print ranges and page breaks producing clipped exports
- workbook dimensions becoming huge because of accidental writes far below/right of the real data region

## Validation and recheck

After meaningful changes:
- reload or re-open the workbook once to catch write-time corruption
- recheck targeted formulas and sheet features with the Rust inspector
- if layout matters, render to PDF/PNG and inspect affected sheets
- if LibreOffice or Poppler is unavailable, say exactly what layout confidence is limited

## Tooling defaults

Read these only when needed:
- [references/tooling.md](./references/tooling.md) for engine choice, dependency setup, and `.xlsm` / CSV / formula caveats
- [references/review-checklist.md](./references/review-checklist.md) for audit and sign-off checks

System tools for render-aware QA:

```bash
# macOS
brew install libreoffice poppler

# Ubuntu/Debian
sudo apt-get install -y libreoffice poppler-utils
```

## Output defaults

Default output should contain:
- workbook task summary
- modifications or findings
- workbook-feature integrity status
- render/layout recheck status when applicable

## Hard constraints

- Do not silently flatten an `.xlsx` workbook into CSV when workbook-native features matter.
- Do not casually rewrite whole sheets if a targeted patch preserves formulas and styles better.
- Do not claim formula correctness without checking whether formulas, cached values, or references changed.
- If macros or `.xlsm` files are involved, call out macro-preservation risk before writing.
- If rendering tools are missing, say exactly what layout confidence is limited.
- Use ASCII hyphens only in generated textual content.
