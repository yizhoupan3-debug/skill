# doc — Detailed Guide

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

This skill is a **Phase-2 artifact gate / detector / verifier** in the shared
finding-driven framework. It should keep Word work `.docx`-native, then emit
findings or verification results that downstream owners can consume without
repeating render checks. Use the shared structures in
[`../SKILL_FRAMEWORK_PROTOCOLS.md`](../SKILL_FRAMEWORK_PROTOCOLS.md) and the
verification protocol when layout or pagination correctness matters.

## Required workflow

1. Identify whether the task is read, generate, edit, or audit.
2. Prefer structured `.docx` edits with `python-docx`.
3. Render to PDF/images when layout fidelity matters.
4. Recheck every meaningful document change visually.
5. Deliver both content/result status and layout status.

## Core workflow

### 1. Intake

- Confirm:
  - source `.docx` path or desired output path
  - whether the task is create / edit / review
  - whether tables, pagination, or layout are in scope
  - whether a render check is required

### 2. Choose the right mode

#### Read / inspect

- Use `python-docx` for text and structural inspection.
- If layout matters, convert:
  - DOCX → PDF
  - PDF → PNGs

#### Create / edit

- Use `python-docx` for:
  - headings
  - paragraphs
  - tables
  - lists
  - section structure
- Keep formatting consistent with the existing document when editing.

#### Render-aware audit

- Prefer the bundled helper:
  - `/Users/joe/Documents/skill/skills/doc/scripts/render_docx.py`
- Or use:
  - `soffice` for DOCX → PDF
  - `pdftoppm` for PDF → PNG

### 3. Common high-risk checks

- broken table widths or cell overflow
- heading hierarchy inconsistency
- numbering or list continuity issues
- clipped text after conversion
- bad page breaks or orphaned headings
- default-template styling leaking into the final document

### 4. Validation / recheck

- After meaningful changes, re-render and inspect affected pages.
- If render tooling is unavailable, call out that layout confidence is limited.

## Dependencies

Prefer `uv` when installing Python packages.

Python packages:

```bash
uv pip install python-docx pdf2image
```

Fallback:

```bash
python3 -m pip install python-docx pdf2image
```

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
