# XLSX tooling and engine choices

Use this reference when you need to choose the right toolchain or explain why a
certain spreadsheet operation is risky.

## Engine choice

### `openpyxl`

Best for:
- workbook-native `.xlsx` reads and writes
- formulas, styles, number formats, merged cells
- sheet order, visibility, freeze panes, filters
- tables, named ranges, data validation, conditional formatting

Prefer this when the workbook artifact itself matters.

### `pandas`

Best for:
- data cleaning and transforms before writing to a workbook
- quick tabular summaries
- exporting helper CSV snapshots

Risk:
- a dataframe round-trip can lose workbook-native structure, formula logic,
  widths, styling, merged cells, tables, validation, and sheet-level metadata.

Use `pandas` as a helper layer, not the default source of truth for rich Excel files.

### LibreOffice / `soffice`

Best for:
- rendering `.xlsx` to PDF for visual QA
- checking print-view issues such as clipped headers, overflow, or awkward page breaks

### `pdftoppm`

Best for:
- converting rendered PDF pages to PNG for sheet-by-sheet visual inspection

## Format boundaries

### `.xlsx`

Default supported format for this skill.

### `.xlsm`

Treat carefully.
- Reading is usually fine.
- Writing can preserve workbook content but macro-sensitive flows need explicit caution.
- If the file contains VBA or macro-dependent behavior, call out risk before saving.

### `.xls`

Legacy binary Excel format.
- Do not pretend it is identical to `.xlsx`.
- If the user really needs `.xls`, call out that a conversion or different toolchain may be required first.

## Formula caveats

- `openpyxl` reads formulas as expressions unless `data_only=True` is used.
- `data_only=True` reads cached values, not the live formula text.
- A workbook modified outside Excel may not refresh cached values automatically.
- When formula correctness matters, say whether you checked:
  - formula text
  - cached values
  - reference targets

## Common safe patterns

- inspect first, edit second
- patch only the touched range when possible
- reload after save to catch corruption early
- export helper CSVs only as side artifacts, not replacements for the workbook
- render to PDF when layout or print behavior matters
