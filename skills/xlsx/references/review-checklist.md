# XLSX review checklist

Use this checklist for audits, sign-off, or after nontrivial workbook edits.

## Structural checks

- correct sheet names and order
- expected hidden vs visible sheets
- no accidental extra sheets
- used range is plausible; no stray writes thousands of rows down/right

## Formula checks

- formulas still exist where expected
- references were not broken by insert/delete/rename operations
- summary sheets still point at the intended tabs
- no accidental formula-to-value replacement in edited regions

## Formatting checks

- number formats remain correct
- column widths and row heights remain usable
- merged cells remain intact where intended
- header styling and banding remain consistent

## Feature checks

- freeze panes still match the intended header/identifier area
- filters still exist where expected
- tables still cover the intended ranges
- data validation remains attached to edited cells
- conditional formatting rules still apply to the intended ranges

## Render / print checks

- long headers do not clip in PDF export
- print ranges and page breaks are sensible
- repeated header rows or key context are not lost in multi-page exports
- dense sheets remain readable when rendered
