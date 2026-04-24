# doc - Detailed Guide

> Extracted from SKILL.md to reduce token consumption at routing time.

## Task ownership and boundaries

This skill owns:
- `.docx` generation and editing
- style-preserving structured edits
- table, heading, numbering, and pagination repair
- render-aware document QA

This skill does not own:
- PDF-native workflows
- slide generation
- general prose rewriting detached from a document artifact

If the task shifts to adjacent skill territory, route to:
- `$pdf` for PDF artifacts
- `$visual-review` for image-grounded render inspection when page screenshots already exist

## Finding-driven framework role

This skill is a Phase-2 artifact gate / detector / verifier in the shared
finding-driven framework. It should keep Word work `.docx`-native, then emit
findings or verification results that downstream owners can consume without
repeating render checks. Use the shared structures in
[`../SKILL_FRAMEWORK_PROTOCOLS.md`](../SKILL_FRAMEWORK_PROTOCOLS.md) and the
verification protocol when layout or pagination correctness matters.

## Required workflow

1. Identify whether the task is read, generate, edit, or audit.
2. Prefer structured `.docx` edits that preserve existing styles, headings, tables, numbering, and sections.
3. Use the Rust OOXML CLI for structure and render-aware checks:
   - `cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/ooxml_parser_rs/Cargo.toml -- docx <docx>`
   - `cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/ooxml_parser_rs/Cargo.toml -- docx <docx> --json`
   - `cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/ooxml_parser_rs/Cargo.toml -- render-docx <docx> --output-dir <dir>`
   - `cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/ooxml_parser_rs/Cargo.toml -- render-docx <docx> --width 1600 --height 2000`
   - `cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/ooxml_parser_rs/Cargo.toml -- render-docx <docx> --dpi 180`
4. Recheck every meaningful document change visually when layout matters.
5. Deliver both content/result status and layout status.

## Core workflow

### 1. Intake

Confirm:
- source `.docx` path or desired output path
- whether the task is create / edit / review
- whether tables, pagination, or layout are in scope
- whether a render check is required

### 2. Choose the right mode

#### Read / inspect

- Inspect document text and structure while preserving the original `.docx`.
- Use `docx` from the Rust OOXML CLI to summarize paragraphs, headings, tables, sections, page size, images, links, notes, and comments.
- If layout matters, render DOCX to PNG pages through the Rust CLI.

#### Create / edit

- Preserve headings, paragraphs, tables, lists, section structure, and existing style names.
- Keep formatting consistent with the existing document when editing.
- Avoid raw XML changes unless the user needs a feature the higher-level document tools cannot express safely.
- After editing, compare the Rust `docx --json` summary against the expected structure.

#### Render-aware audit

- Use `render-docx` from the Rust OOXML CLI.
- The command converts DOCX to PDF with LibreOffice, then converts PDF pages to PNG with Poppler.

### 3. Common high-risk checks

- broken table widths or cell overflow
- heading hierarchy inconsistency
- numbering or list continuity issues
- clipped text after conversion
- bad page breaks or orphaned headings
- default-template styling leaking into the final document

### 4. Validation / recheck

- After meaningful changes, re-render and inspect affected pages.
- Compare paragraph, heading, table, section, image, hyperlink, note, and comment counts when structure preservation matters.
- If render tooling is unavailable, call out that layout confidence is limited.

## Dependencies

System tools:

```bash
# macOS
brew install libreoffice poppler

# Ubuntu/Debian
sudo apt-get install -y libreoffice poppler-utils
```

## Output defaults

Default output should contain:
- document task summary
- modifications/findings
- layout recheck status

Recommended structure:

````markdown
## DOCX Summary
- Mode: read / create / edit / audit
- Target: ...

## Changes / Findings
- ...

## Layout Recheck
- Rendered pages: ...
- Issues found/fixed: ...

## Risks / Assumptions
- ...
````

## Hard constraints

- Do not rely on text extraction alone when layout is in scope.
- Do not deliver a modified `.docx` without visual recheck when formatting matters.
- Do not silently change heading hierarchy, numbering, or tables without checking downstream layout effects.
- If rendering tools are missing, say exactly what confidence is limited.
- Use ASCII hyphens only in generated textual content.

## Trigger examples

- "Use $doc to update this Word file and verify the formatting."
- "Use $doc to create a `.docx` report with clean headings and tables."
- "Inspect this DOCX for broken pagination and table layout."
