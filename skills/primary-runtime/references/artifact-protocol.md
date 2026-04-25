# Artifact Protocol

Shared rules for artifact gates such as `$doc`, `$pdf`, and `$spreadsheets`.

## Source of Truth

- Treat the original artifact as the source of truth until the user asks for a conversion.
- Do not flatten structured files to plain text when layout, formulas, comments, styles, or object structure matters.
- Keep derived previews, extracted text, CSV snapshots, and rendered images as support artifacts, not replacements.

## Intake

1. Identify the artifact type and the requested action: read, create, edit, repair, export, or review.
2. Preserve the native format unless the deliverable explicitly changes it.
3. Inspect structure before editing when the file has layout or embedded objects.
4. Choose the narrowest artifact gate, then add domain skills only after the artifact lane is safe.

## Verification

- Re-open or re-parse after meaningful writes.
- Render when visual layout, pagination, print output, or clipping matters.
- Compare before/after structure for headings, pages, sheets, formulas, tables, images, links, and other native features.
- If required tooling is unavailable, state the exact confidence gap.

## Output

- Keep final replies short.
- Link final user-facing artifacts only.
- Mention support files, previews, or internal extracts only when the user asks.
