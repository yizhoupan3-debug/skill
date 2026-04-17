---
name: xlsx
description: |
  Read, create, edit, repair, and review Excel `.xlsx` workbooks when spreadsheet-native behavior matters.
  Use for `.xlsx` inspection, formula/format fixes, workbook reports, sheet audits, or Excel-native workflows that should not be flattened to CSV. As an artifact gate, check this skill early at 每轮对话开始 / first-turn / conversation start whenever the main artifact is Excel or spreadsheet-like.
routing_layer: L3
routing_owner: gate
routing_gate: artifact
session_start: required
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
  - gate
  - detector
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
---

# xlsx

This skill owns `.xlsx` workbook work where spreadsheet-native structure,
formulas, formatting, and rendered sheet behavior matter more than plain tabular
text alone.

## Priority routing rule

If the primary artifact is an Excel workbook and the task is to inspect,
generate, edit, repair, validate, or visually recheck that workbook, check this
skill before generic data-analysis, CSV-only, or prose-only workflows.

In that case:

1. this skill owns the workbook-native workflow
2. paired skills should build on the workbook artifact rather than flattening it
   too early
3. CSV export is only a helper view, not the default source of truth

## When to use

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
  - "改这个 Excel / xlsx 文件"
  - "生成一个带公式和格式的表格"
  - "检查这个工作簿是不是有坏公式/坏引用"
  - "看一下这个报表 xlsx 的结构和排版"

## Do not use

- The real task is plain CSV or dataframe analysis with no workbook features in scope
- The artifact is `.docx` or PDF rather than Excel
- The user only pasted plain text and wants rewriting
- The file is legacy `.xls` and the task explicitly requires BIFF-era tooling; call out the format mismatch first
- The task is a slide deck or Word-document workflow

## Task ownership and boundaries

This skill owns:
- `.xlsx` workbook reading and generation
- structure-preserving sheet edits
- formulas, styles, and workbook-feature-aware repair
- render-aware spreadsheet QA when sheet appearance matters
- safe export of helper views such as CSV snapshots without losing the workbook source

This skill does not own:
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

This skill is a **Phase-2 artifact gate / detector / verifier** in the shared
finding-driven framework. It should keep workbook work `.xlsx`-native, then
emit findings or verification results that downstream owners can consume
without flattening the workbook to CSV first. Use the shared structures in
[`../SKILL_FRAMEWORK_PROTOCOLS.md`](../SKILL_FRAMEWORK_PROTOCOLS.md) and the
verification protocol when formulas, formatting, or rendered layout must be
rechecked explicitly.

## Required workflow

1. Identify whether the task is inspect, create, edit, repair, export, or audit.
2. Preserve the workbook as the source of truth; do not flatten to CSV unless that is explicitly the deliverable.
3. Choose the right engine:
   - `openpyxl` for workbook-native structure and edits
   - `pandas` for data transforms feeding workbook output
   - LibreOffice / `soffice` for render-oriented conversion to PDF
4. Recheck formulas, styles, and sheet-level features after meaningful changes.
5. If layout matters, render and visually inspect the affected sheets.

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
