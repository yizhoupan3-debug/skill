# Beamer Deck Checklist

## Non-Negotiables

### Frame Structure
- Treat each frame as a distinct communication unit. Split frames with two unrelated messages.
- Use local relative paths for all figures.
- Default to `aspectratio=169` unless explicitly requested otherwise.
- Compile after meaningful layout changes, not just at the end.
- Keep deliverables flat: `.tex` and `.pdf` side by side; use `build/` only for intermediates.

### Visual System
- Implement deliberate cover, footline, semantic colors, emphasis boxes, and closing slide.
- Default house style: deep navy title band, white background, pale panels, muted blue footline, gold emphasis only for selected callouts.
- Readable type outranks content packing. Prefer `12pt` base, `\normalsize` to `\large` body.
- Preserve safe margins and footer clearance. Keep plain-frame footers visible after export.
- No footer on the cover slide unless explicitly requested.
- Center page number inside its own footline block.
- Keep semantic colors role-based: one primary, one accent, one alert, one neutral.
- Reuse a small set of box styles (info, insight, warning). No one-off decorative boxes.
- Avoid `Madrid`, `Warsaw`, or dated stock themes. Use custom source-level styling.
- Keep cover and closing frames in the same visual family.

### Content Integrity
- Never fabricate experimental results, metrics, significance claims, or qualitative conclusions.
- If results are unavailable, label as planned experiment, expected format, or hypothesis.
- Cite every quantitative claim and every external image source.

### Text Quality
- No paragraphs/bullets/titles with line breaks leaving only 1–2 Chinese characters.
- Keep mixed-language tokens intact (English terms, years, percentages, citations).
- Keep titles and subtitles visually balanced; no tiny trailing second lines.
- Reserve bottom safe area for source lines, captions, and footlines.

### Layout Quality
- Do not solve overflow with `\tiny`, `\scriptsize`, `\resizebox`, or screenshots.
- Fix layout problems in TeX source, not by patching the PDF.
- Balance two-column frames in visual weight and whitespace.
- For tables: shorten headers, prune columns, highlight decision-relevant slices first.
- For equation-heavy frames: split derivations or move detail to backup slides.
- Raise density by adding structured zones, not by shrinking text.
- Avoid title-only frames or frames with visually weak, overly-small content.

### QA
- Do not skip visual QA because the log is clean. Beamer can clip without warnings.
- Use `$visual-review` in targeted audit mode for overlap detection.
- Prefer Mermaid-to-SVG/PDF for flowcharts before hand-written TikZ.
- Use `[plain]` frames intentionally; ensure suppressed footlines look deliberate.

## Final Checks

### Compilation
- [ ] Frame count matches PDF page count
- [ ] `latexmk` exits cleanly; remaining warnings understood
- [ ] Final `.pdf` sits beside main `.tex`; `build/` has only intermediates

### Visual QA
- [ ] Rendered slides inspected through `$visual-review`, not just compile logs
- [ ] Overlap explicitly checked on rendered pages
- [ ] Body text readable in rendered PNGs without zooming
- [ ] No tiny-text workarounds on important frames
- [ ] PDF matches intended hierarchy and spacing from source

### Text
- [ ] No 1–2 character Chinese orphan lines
- [ ] Titles/subtitles visually balanced, no tiny trailing lines
- [ ] Mixed-language tokens not split awkwardly
- [ ] Bottom safe area clear for captions/citations/footlines

### Structure
- [ ] Cover has deliberate hierarchy, no raw default title dump
- [ ] Cover has no accidental footer or page number
- [ ] Cover has no unnecessary third-line explanatory text
- [ ] Footline consistent on all non-plain frames
- [ ] Page number centered in its footline block
- [ ] Plain-frame footers fully visible, not clipped
- [ ] Two-column frames balanced in weight and whitespace
- [ ] Deck reads as one restrained template, not mixed styles
- [ ] Enough visual mass per frame; no floating-title-over-nothing

### Semantic Consistency
- [ ] Palette semantically consistent across frames
- [ ] Emphasis boxes use reusable system with matched visual weight
- [ ] Closing/Q&A frame present and visually resolved
- [ ] All external assets stored locally
- [ ] All claims traceable in `sources.md` or `refs.bib`
- [ ] Experimental claims match real completed work
