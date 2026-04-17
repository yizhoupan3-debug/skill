# Visual Review — Detailed Reference

## Chain-of-Thought Template

Use this reasoning sequence for every review. Externalizing these steps reduces hallucination and improves calibration.

```
1. OBSERVE  — What artifact type is this? What are its dimensions/resolution?
2. DESCRIBE — What major elements are visible? What text is legible?
3. SCAN     — Run the 5-pass inspection (Global → Text → Structure → Anomaly → Task-specific)
4. ASSESS   — For each found issue: what is the evidence? what is the impact?
5. JUDGE    — Assign verdict labels. Separate defects from polish suggestions.
6. REPORT   — Lead with the most impactful findings. Attach to locations.
```

## Targeted Audit Mode

Use this mode when another skill wants visual verification of a specific problem type.

### Embedding guidance

When another skill calls this one, keep the handoff compact. Prefer passing only:

- `target issue`
- `artifact scope`
- `decision lens`
- `output need`
- one short line of domain context only if it changes the threshold for judgment

Do not dump the full upstream task if the visual check is local and specific. Keep the visual audit focused on what must be decided from the artifact itself.

When a paired `paper-visuals`, `paper-reviser`, or `scientific-figure-plotting` skill calls this one for recheck, prefer comparing the post-fix artifact against the original issue target and say explicitly whether the issue appears resolved, partially resolved, unchanged, or replaced by a new regression.

### Input contract

Try to identify these four inputs from the caller:

- target issue: the exact problem class to check
- artifact scope: which screen, panel, page, chart region, or comparison pair matters
- decision lens: presence check, severity check, regression check, or compliance check
- output need: binary answer, ranked findings, evidence list, or pass/fail gate

If the caller only provides a vague target like "看看有没有问题", stay in general review mode. If the caller names a concrete issue family, switch to targeted audit mode.

### Issue operationalization

Before inspecting, rewrite the target issue into a visual test:

- what visible cues would support the issue
- what visible cues would argue against it
- what confounders could produce a false positive

Examples:

- `text truncation`: cut-off labels, clipped buttons, ellipsis where full copy seems required
- `misalignment`: edges that should align but do not, uneven spacing in repeated components
- `weak hierarchy`: primary action not visually dominant, headings not clearly separated from body text
- `chart readability`: labels overlap, axes missing units, legend-color mapping unclear
- `chart professionalism`: palette, line weight, spacing, annotation density, data-ink ratio, and overall polish do not look publication-ready
- `caption self-containment`: caption, legend, units, or abbreviations are too incomplete for the artifact to be understood in isolation
- `professional polish`: the artifact is readable but still looks rough, inconsistent, cluttered, or below submission quality
- `table readability`: headers, units, emphasis, or density make the table hard to scan
- `layout defect`: spacing, alignment, placement, or scaling visibly harms readability or composition
- `tiny text`: key content is technically present but too small to read at normal review scale
- `page professionalism`: the rendered page looks crowded, unbalanced, or below camera-ready quality
- `export/render bug`: clipped content, shifted elements, broken pagination, missing assets
- `accessibility violation`: contrast too low, interactive elements too small, color-only encoding

### Audit sequence

For targeted checks, inspect in this order:

1. Restate the target issue as a testable claim
2. Find the most relevant regions first instead of scanning the whole artifact evenly
3. Collect visible evidence for and against the claim
4. Decide the verdict: `confirmed`, `likely`, `not found`, or `indeterminate`
5. Only after the verdict, add secondary findings if they materially affect the same problem

If the caller is rechecking a fix, also state whether the new artifact is improved relative to the old one and whether any regression is visible nearby.

### Evidence standard

Every targeted finding should include:

- verdict
- issue label
- visible evidence
- location
- impact
- confidence

Preferred finding template:

- `Verdict:` confirmed / likely / not found / indeterminate
- `Issue:` one short problem label
- `Evidence:` what is visibly present
- `Location:` where it appears
- `Impact:` why it matters
- `Confidence:` high / medium / low

### False-positive control

To keep downstream skills reliable:

- do not upgrade to `confirmed` if the key region is blurry or cut off
- do not return `not found` when the relevant area is not visible
- do not generalize one local defect into a global claim unless multiple regions support it
- if two interpretations are plausible, prefer `likely` or `indeterminate`
- if the caller defined a strict gate, answer that gate first and keep optional commentary separate

## Multi-Image Comparison Protocol

When the user provides multiple images (before/after, version A/B, sequential states):

### Step 1 — Independent inspection

Inspect each image separately. Note key elements, text, layout, and any issues.

### Step 2 — Difference mapping

Identify:
- **Added**: elements present in the new image but not the old
- **Removed**: elements present in the old image but not the new
- **Moved**: same element in a different position or size
- **Restyled**: same element with different color, font, spacing, or visual treatment
- **Unchanged**: elements that appear identical

### Step 3 — Change classification

For each difference, classify as:
- `intentional improvement` — clearly better
- `intentional neutral` — different but neither better nor worse
- `possible regression` — potentially worse, needs confirmation
- `definite regression` — clearly worse than before

### Step 4 — Summary

Lead with the most impactful changes. State overall direction (improved / mixed / regressed).

## Resolution & Scale Awareness

Different target media demand different quality thresholds:

| Medium | Typical PPI | Min text | Min line weight | Notes |
|--------|-------------|----------|-----------------|-------|
| Screen (1×) | 72–96 | 12px | 1px | Most forgiving |
| Retina/HiDPI | 144–288 | 12px logical | 0.5px logical | Renders sharply but beware logical vs physical px |
| Print (academic) | 300+ | 6pt (~8px) | 0.5pt | Unforgiving; test at actual column width |
| Projection | 72–96 | 24pt+ | 2pt+ | Low contrast room; needs large, bold elements |

When reviewing, always ask: **at what scale will this artifact be consumed?**

- If the target scale is known, evaluate at that scale
- If unknown, assume the most demanding plausible scenario
- Flag elements that pass at screen zoom but may fail at final output scale

## Accessibility Audit Checklist

When the review lens includes accessibility, check these WCAG 2.1 AA criteria that are visually verifiable:

| # | Check | Pass criteria | Visible signal |
|---|-------|---------------|----------------|
| A1 | Text contrast |4.5:1 ratio (normal), 3:1 (large ≥18pt or bold ≥14pt) | Light text on light bg, dark on dark |
| A2 | Non-text contrast | 3:1 for UI components and graphics | Low-contrast borders, icons, charts |
| A3 | Touch targets | ≥44×44 CSS px (mobile), ≥24×24 px (desktop) | Closely packed buttons, tiny icons |
| A4 | Focus indicators | Visible outline/ring on focused elements | Missing or transparent focus rings |
| A5 | Color-only encoding | Info must have a non-color channel (shape, label, pattern) | Legend using only color swatches |
| A6 | Text reflow | Content readable without horizontal scroll at 320px | Horizontal overflow, overlapping text |
| A7 | Spacing | ≥1.5× line height, ≥2× paragraph spacing | Dense text blocks, cramped UI |

## Review Mode Checklists

### UI / screenshot review

Check:

- visual hierarchy and whether the primary action is obvious
- spacing, alignment, contrast, and consistency
- component states, disabled/loading/error feedback, and copy clarity
- accidental clutter, duplicated controls, or hidden affordances
- accessibility risks visible from the screenshot (contrast, target size, focus, color-only encoding)
- responsive layout issues (overflow, cramped elements, broken grids)

### Error screenshot debugging

Check:

- exact error text, stack traces, status codes, and timestamps visible on screen
- which app, environment, or page the error appears in
- preceding UI state that hints at what action triggered it
- whether the issue looks like network, auth, validation, runtime, config, or permission failure

Prefer output shaped as: visible evidence, likely cause, next debugging step.

### Chart / figure audit

Check:

- chart type fit for the claim being made
- axis titles, units, legend clarity, label collisions, and unreadable text
- whether color, annotation, and ordering distort interpretation
- whether a viewer could extract the intended takeaway quickly
- whether the figure looks aesthetically disciplined and professionally made
- whether palette, line weights, spacing, marker size, annotation density, and composition look publication-ready rather than improvised
- **data-ink ratio** — whether non-data elements (gridlines, borders, backgrounds) dominate over actual data
- **annotation density** — whether annotations are helpful or cluttering
- whether the chart still reads cleanly at likely document scale, not only when zoomed in

### Table audit

Check:

- whether headers, units, notes, and abbreviations are complete enough to read the table in isolation
- whether alignment, emphasis, and row/column organization make the table easy to scan
- whether the table is too dense, too small, or visually cluttered at likely document scale
- whether the table looks polished and publication-ready rather than like a raw spreadsheet export

### Document / slide / PDF render review

Check:

- clipping, overflow, widows, inconsistent margins, and misalignment
- font hierarchy, section rhythm, and whether dense blocks should be split
- image sharpness, caption placement, and table readability
- whether the page looks exported correctly rather than merely text-complete
- whether the rendered page looks professionally composed rather than mechanically assembled
- whether page balance, whitespace, and float placement look camera-ready

### Image comparison

Check:

- content added, removed, moved, or restyled
- whether the change affects behavior, readability, or emphasis
- whether the differences are intentional improvements or regressions
- use the Multi-Image Comparison Protocol above for structured analysis

### Targeted problem audit

Check:

- whether the named issue is visually present, absent, or undecidable
- what exact evidence supports that verdict
- whether the issue is isolated or repeated
- whether the issue blocks usability, harms readability, or looks cosmetic
- whether nearby conditions weaken the confidence of the claim
- whether the problem is one of correctness, readability, or professional polish

## Default Output Shapes

### A. Quick describe

- What is visible
- Key text or numbers
- Anything ambiguous

### B. Review findings

- Findings: issue + where + why it matters
- Open questions or ambiguity
- Recommended fixes

### C. Screenshot debugging

- Visible evidence
- Most likely causes
- Next checks to run

### D. Visual diff

- What changed
- What improved
- What regressed or needs confirmation

### E. Targeted audit

- Audit target
- Verdict
- Evidence for the verdict
- Location and impact
- Resolution status when rechecking a fix
- Confidence and ambiguity
- Follow-up action if the verdict is `likely` or `indeterminate`

## Trigger Examples

- "用 visual-review 看一下这张截图哪里有问题。"
- "帮我分析这个报错截图，先说你看到了什么。"
- "比较这两张图，找出 UI 改动和可能的回归。"
- "检查这个 PDF 导出页的排版。"
- "看这张图表，判断标题、标注和视觉表达有没有问题。"
- "判断这张论文图表是否足够美观、专业、适合投稿。"
- "检查这张论文表格是否太密、太小、是否达到投稿级表达。"
- "检查这个 caption 是否足够自洽，图表能不能脱离正文被看懂。"
- "判断这页 PDF 有没有 tiny text 或 layout defect。"
- "判断这个 PDF 页面是否足够专业、像正式成稿。"
- "审核这张图里有没有文字截断问题。"
- "检查这个页面是否存在对齐不一致。"
- "判断导出的 PDF 有没有明显的渲染错误，只给结论和证据。"
- "Check this screenshot for accessibility issues."
- "Compare these two versions and list all visual regressions."
- "Describe exactly what is visible in this screenshot and point out likely issues."
