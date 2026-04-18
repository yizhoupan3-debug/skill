---
name: paper-visuals
description: |
  Audit and improve paper figures, tables, captions, legends, axes, notes,
  and result presentation.
  Use for submission-grade figure/table review, self-containment,
  information density, and visual polish.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
metadata:
  version: "2.1.0"
  platforms: [codex]
  tags: [paper, visuals, figures, tables, captions, publication]
framework_roles:
  - detector
  - executor
framework_phase: 1
framework_contracts:
  emits_findings: true
  consumes_findings: true
  emits_execution_items: false
  consumes_execution_items: false
  emits_verification_results: false
risk: medium
source: local
---
- **Dual-Dimension Audit (Pre: Vector-Logic/Theme, Post: Resolution-DPI/Font-Fidelity Results)** → `$execution-audit-codex` [Overlay]

# Paper Visuals

This skill owns **paper figure/table presentation quality**.

## When to use

- The user wants figure/table review or repair
- The task involves captions, legends, axes, notes, labels, units, or layout inside a figure/table artifact
- The user wants publication-grade visual polish
- The task mentions density, readability, professionalism, or overlap

## Do not use

- The user wants whole-paper triage → use `$paper-reviewer`
- The task is page-level PDF layout or float placement → use `$pdf` first
- The task is code-first plotting-system redesign → use `$scientific-figure-plotting`

## Routing decision: `paper-visuals` vs `scientific-figure-plotting`

| Scenario | Route to |
|----------|---------|
| Review / audit an existing figure or table artifact | `paper-visuals` |
| Fix caption, legend, axis label, or layout issues on existing render | `paper-visuals` |
| Create or rewrite plotting **code** (matplotlib, seaborn, etc.) | `scientific-figure-plotting` |
| Redesign a figure from scratch using code | `scientific-figure-plotting` |
| Mixed: audit found code-level issues that need replotting | Start with `paper-visuals`, delegate code fixes to `scientific-figure-plotting` |
| Audit statistical annotations (error bars, significance markers, n) | `paper-visuals` (audit) → `scientific-figure-plotting` if code fix needed |

Rule of thumb: **already rendered** → `paper-visuals`. **Need code to (re)draw** → `scientific-figure-plotting`.

## Finding-driven framework compatibility

Visual findings produced here should be mappable to the shared
finding-driven framework without losing figure/table-specific context.

Minimum compatibility expectations:
- preserve `finding_id` for each visual issue when revisiting the same artifact
- keep `severity_native` in paper terms (`P0 / A / B / C`)
- include `evidence`, `fixability`, and `recommended_owner_skill`
- when the task is a revision rather than critique, consume findings from
  `$paper-reviewer` or reviewer comments normalized into findings

## Default quality bar

Assume these defaults unless overridden:

- default language: **English**
- visual style: **advanced / publication-grade**
- density: **high information density without clutter**
- overlap policy: **no unresolved overlap**

## Required workflow

1. Identify whether the object is:
   - figure
   - table
   - caption/legend/axis/note
   - mixed result presentation issue
2. Use rendered visuals whenever available.
3. If a render exists, call `$visual-review`.
4. Distinguish:
   - local figure/table issue
   - page-level layout issue
   - code-generated plotting issue
5. Route code-driven redraws to `$scientific-figure-plotting`.

## Review checklist

Audit or fix:
- self-containment (readable without main text; all axes labeled with units; legend complete)
- visual hierarchy (title / subtitle / axis / data layers clearly ordered)
- information density (every element earns its place; no redundant encodings)
- label/unit completeness (all variables have labels and SI or domain units)
- legend strategy (direct labeling vs separate legend; consistent and minimal)
- readability at likely paper scale (≥6 pt text, ≥0.5 pt lines after column-fit scaling)
- precision/alignment for tables (decimal alignment, consistent sig figs)
- overlap across text, annotations, ticks, legends, or marks
- color accessibility (colorblind-safe palette; grayscale fallback if required)
- format compliance (width, DPI, file format match target journal)

### Statistical visualization audit
- error bars present on all summary statistics with type (SD / SEM / CI) stated in caption
- sample size (n) reported per group (in labels, annotation, or caption)
- significance markers (\*, \*\*, \*\*\*) consistent and backed by reported statistical tests
- individual data points visible when n < 30 (not hidden behind summary bars)
- 3D plots justified (not used when 2D heatmap/contour would suffice)

## Technical quality checks

| # | Check | Pass Criteria |
|---|-------|---------------|
| T1 | Resolution | ≥300 DPI for raster, vector preferred for plots |
| T2 | Font embedding | All fonts embedded in PDF/EPS |
| T3 | Font size | ≥6pt at final print size (8pt recommended) |
| T4 | Color space | RGB for online, CMYK for print (check venue) |
| T5 | Colorblind safety | Distinguishable without color alone |
| T6 | Compression | No JPEG artifacts on line art or plots |
| T7 | Figure width | Fits within venue column width at submission scale |
| T8 | Caption | Self-contained, describes what is shown and key finding |

For journal-specific format specs and a reusable audit template, see
[`references/journal-specs.md`](references/journal-specs.md).

## Output defaults

Use `视觉问题单` or `视觉修订记录`:
- object
- issue
- evidence
- fix
- whether visual evidence was checked
- remaining risk

## Hard constraints

- Do not skip `$visual-review` when rendered evidence exists.
- Do not accept slight overlap as good enough.
- Do not mistake ornament for publication quality.
- When the user mentions a specific journal, verify format compliance against
  `references/journal-specs.md` before sign-off.
