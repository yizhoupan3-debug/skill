---
name: xlsx
description: |
  Use after the `$spreadsheets` gate when the user explicitly wants an
  `openpyxl` / `pandas` / LibreOffice-driven `.xlsx` workflow. Best for
  workbook-structure inspection, formula/format repair, compatibility-oriented
  edits, or render-aware QA that should stay Python/tooling-first rather than
  defaulting to `@oai/artifact-tool`.
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - openpyxl
  - pandas
  - libreoffice
  - workbook structure audit
  - formula format repair
  - keep formulas and formatting
runtime_requirements:
  python:
    - openpyxl
    - pandas
  commands:
    - soffice
    - pdftoppm
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - xlsx
    - excel
    - spreadsheet
    - openpyxl
    - pandas
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
  - python
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

This skill owns the explicit Python/tooling lane for `.xlsx` work. Use it when
the workbook path should be driven by `openpyxl`, `pandas`, bundled audit
scripts, or LibreOffice render checks rather than the default `$spreadsheets`
artifact-tool path.

## Routing note

Generic Excel / spreadsheet intake should hit `$spreadsheets` first. This skill
should take over only when the user explicitly wants the Python/openpyxl lane or
when compatibility / targeted preservation work makes that lane the better fit.

## When to use

- The user explicitly asks for `openpyxl`, `pandas`, LibreOffice, or a Python-based workbook workflow
- The task involves an `.xlsx` workbook as the main artifact
- The user wants to create, read, edit, review, repair, or audit Excel files
- The task depends on workbook-native features such as:
  - formulas
  - styles and number formats
  - merged cells
  - filters and frozen panes
  - tables and named ranges
  - data validation or conditional formatting
  - hidden sheets / workbook structure
- The user wants to preserve Excel behavior rather than converting straight to CSV
- Layout-sensitive review matters, such as print ranges, clipped headers, or visual sheet QA
- Best for requests like:
  - "用 openpyxl 改这个 Excel / xlsx 文件"
  - "生成一个带公式和格式的表格"
  - "检查这个工作簿是不是有坏公式/坏引用"
  - "看一下这个报表 xlsx 的结构和排版"

## Do not use

- Generic Excel / spreadsheet requests with no explicit engine choice yet → use `$spreadsheets` first
- The real task is plain CSV or dataframe analysis with no workbook features in scope
- The artifact is `.docx` or PDF rather than Excel
- The user only pasted plain text and wants rewriting
- The file is legacy `.xls` and the task explicitly requires BIFF-era tooling; call out the format mismatch first
- The task is a slide deck or Word-document workflow

## Task ownership and boundaries

This skill owns:
- explicit `openpyxl` / `pandas` / LibreOffice-driven `.xlsx` workflows
- `.xlsx` workbook reading and generation
- structure-preserving sheet edits
- formulas, styles, and workbook-feature-aware repair
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

## Finding-driven framework role

This skill is a **Phase-2 implementation / verification lane** in the shared
finding-driven framework. It should keep workbook work `.xlsx`-native after
`$spreadsheets` has already decided that the Python/tooling path is the right
one. Use the shared structures in
[`../SKILL_FRAMEWORK_PROTOCOLS.md`](../SKILL_FRAMEWORK_PROTOCOLS.md) and the
verification protocol when formulas, formatting, or rendered layout must be
rechecked explicitly.

## Required workflow

1. Confirm this really should not stay on the default `$spreadsheets` artifact-tool path.
2. Identify whether the task is inspect, create, edit, repair, export, or audit.
3. Preserve the workbook as the source of truth; do not flatten to CSV unless that is explicitly the deliverable.
4. Choose the right engine:
   - `openpyxl` for workbook-native structure and edits
   - `pandas` for data transforms feeding workbook output
   - LibreOffice / `soffice` for render-oriented conversion to PDF
5. Recheck formulas, styles, and sheet-level features after meaningful changes.
6. If layout matters, render and visually inspect the affected sheets.

## Core workflow

### 1. Intake

Confirm:
- source `.xlsx` path or desired output path
- whether the task is create / inspect / edit / audit
- whether formulas, styles, validation, tables, or merged cells are in scope
- whether layout or print-view QA is required
- whether macros are involved (`.xlsm` should usually be preserved carefully rather than rewritten casually)

### 2. Choose the right mode

#### Inspect / audit

- Use `openpyxl` to inspect workbook structure and formula presence.
- Prefer the bundled helper:
  - `/Users/joe/Documents/skill/skills/xlsx/scripts/inspect_xlsx.py`
- Summarize:
  - sheet names and visibility
  - used ranges
  - formulas and tables
  - merged cells
  - freeze panes / filters
  - data validation / conditional formatting
  - named ranges and obvious risks

#### Create / edit

- Use `openpyxl` for workbook-native creation and edits.
- Use `pandas` only as an input/output helper when tabular transforms are needed.
- Preserve whenever possible:
  - existing sheet names and order
  - formulas
  - number formats
  - column widths / row heights
  - merged regions
  - tables and filters
- Avoid destructive sheet rewrites when a targeted edit is enough.

#### Render-aware review

- Use LibreOffice to convert workbook to PDF.
- Prefer the bundled helper:
  - `/Users/joe/Documents/skill/skills/xlsx/scripts/render_xlsx.py`
- If requested, rasterize the PDF to PNGs for page-by-page inspection.
- If rendered screenshots already exist, pair with `$visual-review`.

### 3. Common high-risk spreadsheet defects

Watch for:
- formulas replaced by static values unintentionally
- broken references after row/column insertion or sheet renaming
- hidden sheets accidentally exposed or removed
- merged cells split by naive writes
- styles or number formats lost after dataframe round-trip
- filters, tables, or validations disappearing after a rewrite
- print ranges and page breaks producing clipped exports
- workbook dimensions becoming huge because of accidental writes far below/right of the real data region

### 4. Validation and recheck

After meaningful changes:
- reload the workbook once to catch write-time corruption
- recheck targeted formulas and sheet features
- if layout matters, render to PDF/PNG and inspect affected sheets
- if confidence is limited because LibreOffice or Poppler is unavailable, say so explicitly

## Tooling defaults

Read these only when needed:
- [references/tooling.md](./references/tooling.md) for engine choice, dependency setup, and `.xlsm` / CSV / formula caveats
- [references/review-checklist.md](./references/review-checklist.md) for audit and sign-off checks

Prefer `uv` when installing Python packages.

Python packages:

```bash
uv pip install openpyxl pandas
```

Fallback:

```bash
python3 -m pip install openpyxl pandas
```

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

Recommended structure:

````markdown
## XLSX Summary
- Mode: inspect / create / edit / audit
- Target: ...

## Changes / Findings
- ...

## Integrity Check
- Formulas/styles/tables/validation: ...
- Risks: ...

## Layout Recheck
- Rendered: yes/no
- Issues found/fixed: ...
````

## Hard constraints

- Do not silently flatten an `.xlsx` workbook into CSV when workbook-native features matter.
- Do not casually rewrite whole sheets if a targeted patch preserves formulas and styles better.
- Do not assume `pandas.to_excel()` preserves advanced workbook features.
- Do not claim formula correctness without checking whether formulas, cached values, or references changed.
- If macros or `.xlsm` files are involved, call out macro-preservation risk before writing.
- If rendering tools are missing, say exactly what layout confidence is limited.
- Use ASCII hyphens only in generated textual content.

## Trigger examples

- "Use $xlsx to inspect this workbook and summarize sheet structure plus formula risks."
- "Use $xlsx to update the report template but keep formulas and formatting intact."
- "Create a polished `.xlsx` budget workbook with formulas, filters, and frozen headers."
- "Audit this Excel file for broken references, formatting loss, and print-layout issues."
