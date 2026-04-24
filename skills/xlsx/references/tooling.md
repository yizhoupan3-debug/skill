# XLSX Tooling And Engine Choices

Use this reference when you need to choose the right workbook path or explain
why a spreadsheet operation is risky.

This reference belongs to the Rust-first `$xlsx` compatibility lane. It does
not override the default `$spreadsheets` artifact-tool path for generic workbook
intake.

## Rust OOXML CLI

Best for:
- workbook-native `.xlsx` structure inspection
- sheet order, visibility, dimensions, tables, merged cells, freeze panes, filters, print areas, validations, conditional formatting, charts, images, external links, and defined names
- JSON summaries that can be compared before and after edits
- render handoff through LibreOffice through the Rust CLI

Use:

```bash
cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/ooxml_parser_rs/Cargo.toml -- xlsx report.xlsx
cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/ooxml_parser_rs/Cargo.toml -- xlsx report.xlsx --json
cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/ooxml_parser_rs/Cargo.toml -- render-xlsx report.xlsx --outdir rendered --png
```

## Before / after comparison

For edits, capture JSON before and after when workbook structure matters:

```bash
cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/ooxml_parser_rs/Cargo.toml -- xlsx before.xlsx --json > before.json
cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/ooxml_parser_rs/Cargo.toml -- xlsx after.xlsx --json > after.json
```

Compare sheet count, sheet names, dimensions, formulas, merged ranges, tables,
validations, conditional formatting, print areas, charts, images, defined names,
and external links.

## Tabular transforms

For simple data reshaping, keep the workbook as the source of truth and avoid
round-tripping the whole file through a flat table. A full tabular rewrite can
lose workbook-native structure, formula logic, widths, styling, merged cells,
tables, validation, and sheet-level metadata.

Safe pattern:
- inspect first
- patch only the touched range when possible
- reload or re-open after save to catch corruption early
- export helper CSVs only as side artifacts, not replacements for the workbook
- render to PDF/PNG when layout or print behavior matters

## LibreOffice / `soffice`

Best for:
- rendering `.xlsx` to PDF for visual QA
- checking print-view issues such as clipped headers, overflow, or awkward page breaks

## `pdftoppm`

Best for:
- converting rendered PDF pages to PNG for sheet-by-sheet visual inspection

## Format boundaries

`.xlsx` is the default supported format for this skill.

`.xlsm` needs care:
- reading is usually fine
- writing can preserve workbook content but macro-sensitive flows need explicit caution
- if the file contains VBA or macro-dependent behavior, call out risk before saving

`.xls` is a legacy binary Excel format:
- do not pretend it is identical to `.xlsx`
- if the user really needs `.xls`, call out that a conversion or different toolchain may be required first

## Formula caveats

- Formula text, cached values, and reference targets are different checks.
- A workbook modified outside Excel may not refresh cached values automatically.
- When formula correctness matters, say whether you checked formula text, cached values, reference targets, or rendered output.
