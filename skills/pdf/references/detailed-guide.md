# pdf — Detailed Guide

> Extracted from SKILL.md to reduce token consumption at routing time.

## Task ownership and boundaries

This skill owns:
- PDF reading with rendering awareness
- PDF generation workflows
- PDF layout defect inspection
- text/structure extraction when tied to a PDF artifact
- re-render-and-recheck loops after PDF changes

This skill does not own:
- Word `.docx` editing
- general image/UI review detached from a PDF
- paper-specific scientific logic review

If the task shifts to adjacent skill territory, route to:
- `$doc` for `.docx`
- `$visual-review` for image-grounded render review
- `$paper-reviewer` or `$paper-reviser` when PDF layout judgment is part of manuscript-wide review/revision

## Finding-driven framework role

This skill is a **Phase-2 artifact gate / detector / verifier** in the shared finding-driven framework. It should keep PDF work PDF-native, then emit findings or verification results that downstream domain owners can consume without repeating render checks. Use the shared structures in [`../SKILL_FRAMEWORK_PROTOCOLS.md`](../SKILL_FRAMEWORK_PROTOCOLS.md) and the verification protocol when layout correctness matters.

For PDF audits, preserve at least:
- `finding_id`
- `artifact_ref`
- `evidence` (rendered page or extraction mismatch)
- `fixability`
- `verification_method`
- `status`

## Required workflow

1. Identify the task mode:
   - read
   - generate
   - edit/fix
   - audit
2. Prefer render-based inspection when layout matters.
3. Use the smallest correct extraction/generation tool for the job.
4. Re-render after meaningful changes.
5. Deliver both substance and render-quality status.

## Core workflow

### 1. Intake

- Confirm:
  - input file path or generation target
  - whether the task is read / generate / edit / audit
  - whether layout fidelity matters
  - expected final deliverable

### 2. Choose the right mode

#### Read / inspect

- Use `pdfplumber` or `pypdf` for extraction.
- If visual correctness matters, render pages to PNGs first or in parallel.
- Do not trust text extraction alone for tables, spacing, or clipping.

#### Generate

- Prefer `reportlab` for programmatic PDF creation.
- Generate, then render to images for QA.

#### Edit / fix

- Modify the source or regeneration pipeline, not the already-broken PDF artifact when avoidable.
- Re-render after each meaningful update.

#### Audit

- Convert pages to PNGs with:
  - `pdftoppm -png $INPUT_PDF $OUTPUT_PREFIX`
- Review:
  - clipping
  - overlapping text
  - broken tables
  - margin inconsistency
  - unreadable glyphs
  - hierarchy/spacing issues

### 3. Validation / recheck

- After any fix or generation step:
  - re-render the affected pages
  - inspect the latest images
- If render review is blocked by missing dependencies, say so clearly and note the remaining risk.

## Dependencies

Prefer `uv` when installing Python dependencies.

Python packages:

```bash
uv pip install reportlab pdfplumber pypdf
```

Fallback:

```bash
python3 -m pip install reportlab pdfplumber pypdf
```

System rendering tools:

```bash
# macOS
brew install poppler

# Ubuntu/Debian
sudo apt-get install -y poppler-utils
```

## Output defaults

Default output should contain:
- task mode
- content/result summary
- render-quality status
- whether the PDF artifact is ready for downstream domain review or still blocked on render defects

Recommended structure:

````markdown
## PDF Summary
- Mode: read / generate / edit / audit
- Target: ...

## Findings / Result
- ...

## Render Review
- Pages checked: ...
- Defects found: ...

## Risks / Assumptions
- ...
````

## Hard constraints

- Do not treat text extraction as proof that layout is correct.
- Do not deliver a changed PDF without a render recheck when layout matters.
- Do not ignore clipped text, overlapping elements, or broken tables.
- If dependencies are missing, report exactly what is blocked.
- Use ASCII hyphens only in generated textual content.

## Trigger examples

- "Use $pdf to inspect this PDF and tell me whether the rendering looks broken."
- "Use $pdf to generate a PDF and verify the rendered pages."
- "Read this PDF, extract the content, and check whether tables or text are misrendered."
