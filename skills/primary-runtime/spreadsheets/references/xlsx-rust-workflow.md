# XLSX Rust Workflow

This reference is the compatibility lane for workbook-native `.xlsx` inspection
and rendering under the canonical `spreadsheets` gate.

Use it when the user explicitly asks for workbook structure audit, formula/style
inspection, LibreOffice-style rendering checks, or Rust OOXML tooling.

Typical commands:

```bash
cargo run --manifest-path rust_tools/ooxml_parser_rs/Cargo.toml -- xlsx report.xlsx
cargo run --manifest-path rust_tools/ooxml_parser_rs/Cargo.toml -- xlsx report.xlsx --json
cargo run --manifest-path rust_tools/ooxml_parser_rs/Cargo.toml -- render-xlsx report.xlsx --outdir rendered --png
```

Completion checks:

- workbook structure is readable
- formulas and key styles are preserved
- rendered sheets are legible
- final `.xlsx` exists and opens
