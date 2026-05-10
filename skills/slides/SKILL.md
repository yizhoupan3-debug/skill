---
name: slides
description: Route presentation, PPT, PPTX, and slide deck tasks.
routing_layer: L3
routing_owner: gate
routing_gate: artifact
routing_priority: P1
session_start: required
trigger_hints:
  - PPT
  - pptx
  - 做个PPT
  - 生成演示文稿
  - slides
  - PowerPoint
  - presentation deck
  - presentation
  - artifact tool
metadata:
  version: "2.6.2"
  platforms: [codex]
  tags: [powerpoint, ppt, pptx, slides, presentation, rust-ppt, proxy]
risk: medium
source: local
allowed_tools:
  - shell
  - cargo
approval_required_tools:
  - file overwrite
filesystem_scope:
  - repo
  - workspace
  - temp
  - artifacts
network_access: conditional
artifact_outputs:
  - final_deck.pptx
  - deck.plan.json
  - rendered/*.png
  - montage.png
  - EVIDENCE_INDEX.json

---

# slides

This skill owns the presentation entry gate for artifact-first slide work.
Convert a generic PPT request into an executable lane with minimal questions
and verifiable output quality.

## Fast start

- Assume a broad professional audience, editable `.pptx`, and the requested output path when safe defaults exist.
- Do not stop to ask for goal, audience, visual bar, or format when a safe default exists.
- Ask follow-up questions only for overwrite risk, paid/private assets, or materially different source-format choices.
- State assumptions in plain language and continue execution unless blocked.

## Route boundary

Use this skill when:

- The primary artifact is a slide deck, presentation, or `.pptx`.
- The user asks to create, revise, render-check, or export editable slides.
- The request mentions PPT/PPTX/PowerPoint/presentation output.

Do not use this skill when:

- The user explicitly wants Markdown, Slidev, Marp, or HTML/CSS source slides.
- The user explicitly wants LaTeX Beamer source plus PDF.
- The task is a document/PDF workflow instead of slide artifact work.
- The output is static screenshots or collage only.

## Non-negotiable contracts

- Treat `slides` as the canonical first check for generic PPT asks before narrower owners claim work.
- If routing misses or the wrong owner opens, Re-run routing or consult the fallback manifest for that exact owner.
- If the user explicitly names Markdown/HTML/Beamer source, reroute through router/fallback and open only that owner skill.
- Use Rust `ppt` CLI from `rust_tools/pptx_tool_rs` for editable `.pptx` generation, inspection, render checks, and strict QA unless an in-place existing deck edit is required.
- Build from writable workspace/temp/artifact directories only; keep scratch files under task workspace or `artifacts/scratch`.
- Keep generated-deck authoring source-first and deterministic: update `deck.plan.json`, rebuild, then verify.

## Style contract: visual-first, plain-language delivery

Apply this contract when the preference is fewer words, more visuals, high
information density, minimal design, and direct language.

Priority rule:

- When "high information density" conflicts with "minimal premium clarity", prioritize minimal clarity first. Split slides or move content to appendix instead of shrinking type or overpacking layouts.

SHOULD:

- Keep content slides visual-first. Charts, diagrams, screenshots, and structured tables carry the message before body text.
- Use plain language. Short sentences, concrete claims, and direct next actions.
- Keep one to three core information units per slide (one unit is one claim, one chart takeaway, one decision, or one action block).
- Keep each content slide at 45 words or fewer (soft warning 46-60; fail above 60), excluding title-only and appendix slides.
- Keep visual-to-text area ratio at least 60:40 on content slides; target 70:30 for strategy/overview slides.
- Introduce at most two new terms per slide, and explain each in 12 words or fewer.

SHOULD NOT:

- Use paragraph-heavy slides, abstract slogans, or framework jargon without concrete meaning.
- Add decorative visuals that do not support a claim.
- Hide recommendation actions; owner/action/timeline should be explicit on recommendation slides.

## Aesthetic bar: minimal-premium

This skill should look restrained, intentional, and executive-ready by default.
Pass this bar only if all checks below are true:

- Each slide has one dominant visual anchor (headline, key number, or chart focus).
- Negative space is intentional and supports reading flow, not accidental emptiness.
- Headline language is conclusion-first and can stand alone without speaker notes.
- Visual emphasis is selective: no more than one primary highlight treatment per slide.
- The deck avoids template-like repetition that signals "AI-generated filler".
- Cheap-looking combos are avoided by default: prefer **one** evidence modality per slide when possible (diagram **or** photo **or** table **or** dense type), unify accents under the deck palette, and use typography/spacing for sequence instead of assigning a new bright color to every step or column.

## Aesthetic gate (fail-fast)

Fail-fast this lane when any hard readability or structure item below is detected in source plan or rendered QA. Fix the failing slide first, then continue.

- `theme rhythm` hard rule:
  - Never run the same theme family for 3 consecutive slides.
  - For decks with more than 8 slides, it is recommended to include at least 2 hero pages and cover both `dark-premium` and `light-editorial` hero treatments when narrative fit allows.
- `anti-ugly` blacklist hit (any single hit is a hard fail):
  - Decorative underline/rule directly under the title with no information function.
  - Low-contrast gray body text on dark or light surfaces.
  - Equal-weight card farm where all cards visually compete with no lead.
  - Default Office chart palette kept without restyling.
  - Text placed on top of imagery without a solid protection layer.
  - Whole deck uses one isomorphic layout pattern with no role variation.
  - `rainbow-accents`: three or more unrelated saturated accent colors on one slide (for example per-column outline rings, circles, or badges) without a single declared lead accent—reads as stock diagram mash-up.
  - `diagram-photo-stack`: primitive flat infographic (basic shapes, default-looking connectors) stacked above or beside a full-bleed photo band on the same slide without one shared surface, grid, or color temperature—looks like two templates glued together.
  - `template-table-slab`: large native table filling the canvas with header color-blocks plus zebra stripes but weak typographic hierarchy (all rows same weight)—reads as pasted spreadsheet, not editorial layout.
- `zh-layout-guard` violation:
  - Chinese title/subtitle may not end with 1-2 character orphan lines.
  - Mixed-language tokens must not split across lines (e.g., `A/B test`, years, `%`, citations).
  - Chinese titles should target 8-18 Han characters; when longer, rewrite or force semantic line breaks.
- Body size violation:
  - Minimum body text size is 18pt across all normal content slides.
  - For density overflow, split slides or move detail to appendix; never solve density by shrinking body text.

## Design tokens and composition rules

Use these defaults unless the user explicitly requests another style system.
These are baseline quality requirements, not optional polish.

Grid and spacing:

- Use a 12-column grid with consistent horizontal anchors across slides in the same section.
- Keep safe margins at 5-7% of slide width; avoid edge-hugging layouts.
- Use an 8-point spacing scale (`8/16/24/32/48`) for block gaps and insets.
- Keep at least 35% whitespace on content slides.

Type and hierarchy:

- Keep at most three visual hierarchy levels per slide (primary, secondary, annotation).
- Use no more than two font families and two to three font weights across the deck.
- Keep body text at 18pt or above; key numbers and claims should be 30pt or above.
- If content exceeds readable density, split pages or move detail to appendix instead of shrinking text.
- Prefer conclusion-first headline sentences over topic labels.

Color and tokens:

- Use a constrained palette: one primary color, one accent color, and neutrals.
- Keep categorical chart colors to eight or fewer by default (ten maximum when justified).
- Enforce readable contrast: target WCAG-style thresholds (4.5:1 normal text, 3:1 large text).
- Reuse visual tokens for radius, border, and shadow; avoid per-slide style drift.

Chart, imagery, and motion:

- Remove chartjunk: no 3D chart effects, decorative gradients, or heavy shadows.
- Highlight one data story per chart; de-emphasize non-primary series.
- Keep imagery style consistent (crop ratio, edge treatment, color temperature).
- Keep motion restrained: default static slides; when needed, only simple reveal/fade motions with a clear information purpose.

## Execution paths

1. Intake quickly: extract goal, source material, output path, and format.
2. If reusable visual identity or style-contract acceptance is required, hand off to `$design-md` before authoring.
3. If user explicitly wants Markdown/HTML source, reroute to `source-slide-formats`; if Beamer, reroute to `ppt-beamer`.
4. Native `.pptx` lane selection:
   - New or rebuilt deck: use `deck.plan.json` as source of truth.
   - Existing deck small edits: inspect existing `.pptx` first, then patch the smallest safe surface.
5. Build with editable objects first (text, shapes, tables, native charts), then verify and export final `.pptx`.

## Command cookbook

Use these commands from the deck workspace. If `ppt` is not already on `PATH`,
run it through Cargo:

```bash
cargo run --manifest-path rust_tools/pptx_tool_rs/Cargo.toml --bin ppt -- <command>
```

The Cargo fallback command assumes repository root as current directory. If you
run from another directory, use an absolute manifest path.

New deck or rebuild:

```bash
ppt init .
ppt outline outline.json --output deck.plan.json --bootstrap --build --json
ppt build-qa --workdir . --entry deck.plan.json --deck deck.pptx --rendered-dir rendered --quality strict --json
```

Default single-command gate is defined in `CLI-first mode` below.
Use diagnostics/audit commands only when that gate fails or when a focused audit is explicitly required.

Existing deck intake:

```bash
ppt intake input.pptx --json
ppt office doctor input.pptx --json
ppt render input.pptx --output-dir rendered
```

Diagnostics/Audit path (only after gate failure or for focused audit):

```bash
ppt office doctor deck.pptx --json
ppt slides-test deck.pptx --fail-on-any
ppt render deck.pptx --output-dir rendered
ppt detect-fonts deck.pptx --json
ppt qa deck.pptx --rendered-dir rendered --fail-on-issues --json
```

`ppt slides-test --fail-on-overflow` is scoped to out-of-bounds/overflow geometry
checks. Use rendered review and/or `ppt qa` as the separate overlap gate.

For decks longer than 8 slides, add this only in diagnostics/audit mode:

```bash
ppt create-montage --input-dir rendered --output-file montage.png --label-mode number
```

## Design skill handoff

Use `$design-md` before slide authoring when the user asks for:

- a branded PPT, visual system, theme, or "统一设计规范"
- reusable deck styling across many slides
- theme colors, typography, chart palette, callout boxes, icon style, or title/section slide grammar
- "less AI", "高级感", "像一个完整产品发布会", or acceptance against an existing `DESIGN.md`

Do not route through `$design-md` for quick utility decks where speed matters
more than a reusable visual contract.

## Existing PPTX Safety

- Before modifying an existing `.pptx`, run `ppt intake` and `ppt office doctor`
  to map slide count, shapes, notes, charts, tables, media, and validation
  issues.
- Preserve the original file by writing a new output path unless the user
  explicitly asked for overwrite.
- For minor edits, keep masters, layouts, notes, animations, images, charts, and
  unknown OOXML parts in place; only patch the targeted text/table/chart values.
- For major redesigns, treat the old deck as source material, extract structure
  and assets, then rebuild through `deck.plan.json`.
- If a requested edit depends on unsupported animation, embedded media, macros,
  or proprietary effects, report that limitation and preserve those parts rather
  than flattening them into screenshots.

## Cover discipline

- Support two approved cover families and keep the chosen family consistent for
  cover, section dividers, and closing slide:
  - `dark-premium` (`dark`/`black`): dark canvas, protected text zone, and optional
    softened background imagery.
  - `light-editorial` (`light`/`white`): light canvas, strong typographic hierarchy,
    subtle separators, and optional softened imagery with clear text protection.
- Do not force a single black blurred cover when the user requests light/white or
  when deck context fits a light editorial family better.
- Enforce one dominant focal point and one clear title stack regardless of theme.
- If readability or hierarchy fails on the selected family, repair by changing
  layout/slide count first, not by shrinking body text below 18pt.

## Verification Standard

Machine-checkable steps agents must be able to run from the deck workspace:

- `ppt slides-test --fail-on-overflow` for out-of-bounds geometry on dense slides.
- `ppt detect-fonts --json` when font substitution or embedding matters.

## Quality gates

Render every slide for decks up to 12 slides. For longer decks, render all slides
and review a montage plus all cover, section, dense chart/table, and changed slides.

### Machine gates (CLI-verifiable)

Only put machine-checkable pass/fail items here.
Default path runs only the single-command gate; additional machine gates below are diagnostics/audit checks.

- Build gate: `deck.pptx` is generated from `deck.plan.json`.
- Layout gate (diagnostics/audit): `ppt slides-test --fail-on-overflow` reports no out-of-bounds content.
- QA gate (diagnostics/audit): `ppt qa ... --json` runs successfully and reports no blocking issues.
- Font gate (diagnostics/audit): `ppt detect-fonts --json` shows no unaccepted missing/substituted important fonts.
- Evidence gate: `EVIDENCE_INDEX.json` is updated with artifacts and checks.

### Human review gates

Use rendered review for subjective quality and narrative fitness; treat these as review checks, not CLI hard gates.

- Editability and story fit are acceptable for the target audience and use case.
- At least 80% of non-title slides stay within the 45-word guideline.
- At least 80% of content slides meet the visual-to-text guideline.
- No slide exceeds three core information units.
- Recommendation slides include explicit owner/action/timeline.
- `theme rhythm` review passes: no 3 consecutive same-theme slides; for decks >8 slides, dark/light hero coverage is recommended when it improves pacing.
- `zh-layout-guard` review passes: no Chinese 1-2 char orphan endings; mixed-language tokens stay intact.
- `anti-ugly` review has no major hits.
- Grid, spacing, hierarchy, palette, chart emphasis, and imagery treatment remain consistent with the chosen style system.

## CLI-first mode (deterministic AI pipeline)

Use this mode when the user wants fully automatable, fast, and precise deck generation with pure Rust tooling.

CLI contract:

- Use Rust `ppt` subcommands only (no JS/Python generators in this mode).
- Use `deck.plan.json` as the executable source-of-truth entry.
- Accept `deck.request.json` only as an optional upstream adapter that must compile into `deck.plan.json` before build.
- Emit stable artifacts and machine-checkable gates for pass/fail decisions.
- Treat structured JSON stdout (`--json`) as the machine interface; persistent report files are orchestrator-owned.

Default single-command gate (canonical definition):

```bash
ppt build-qa --workdir . --entry deck.plan.json --deck deck.pptx --rendered-dir rendered --quality strict --json
```

Diagnostics/audit chain (purpose + minimum set; run only when default gate fails or audit is requested):

- Use `ppt qa ... --json` for structured failure reasons.
- Use `ppt slides-test ... --fail-on-any` for focused geometry/aesthetic diagnosis.
- Use `ppt detect-fonts ... --json` for font-specific diagnosis.
- Add `ppt office doctor ... --json`, `ppt render ...`, and `ppt create-montage ...` only when you need package-level or visual evidence.

Required artifacts:

- `deck.pptx` (final editable deck)
- `deck.plan.json` (source of truth)
- `rendered/*.png` (per-slide previews)
- `montage.png` (required when slide count is greater than 8)
- `EVIDENCE_INDEX.json` (artifact/check index)

Optional orchestrator artifacts (not guaranteed by `ppt` itself):

- `qa.report.json`
- `run.log.jsonl`
- `outline.json`

Hard gates:

- Default hard gate: `ppt build-qa ... --quality strict --json` passes and generates `deck.pptx`.
- Diagnostics/audit hard gates (only when default gate fails or audit is requested):
  - Layout gate (`ppt slides-test --fail-on-overflow`, out-of-bounds only)
  - Overlap gate (rendered review and/or `ppt qa` confirms no overlap regressions)
  - Font gate (`ppt detect-fonts ... --json` with acceptable result)
  - QA gate (`ppt qa ... --fail-on-issues --json`)
- Evidence gate: `EVIDENCE_INDEX.json` updated with artifacts and checks.

Pass criteria:

- Default path: the single-command gate passes.
- Diagnostics/audit path: the selected diagnostic commands pass for the specific failing domain.

Failure handling:

- On any hard-gate failure, fail fast and preserve CLI error output.
- If an orchestrator wrapper is present, it may additionally write `qa.report.json` with `failed_gate`, `reason`, `repair_hint`, and `retry_command`.

Common repair hints:

- Overflow/layout failures: split dense slides, reduce information units, and keep
  body text >=18pt, then rerun `ppt slides-test`.
- Font failures: switch to available fonts or install missing fonts, then rerun `ppt detect-fonts`.
- QA failures: inspect `ppt qa --json` output, patch only failing slides, rerun the selected profile end-to-end.

## Evidence Index

When this skill creates or materially changes a deck, write or update
`EVIDENCE_INDEX.json` in the deck workspace or artifact directory. Keep it
compact:

```json
{
  "artifacts": [
    {"path": "deck.pptx", "kind": "pptx", "status": "final"},
    {"path": "deck.plan.json", "kind": "source", "status": "source-of-truth"},
    {"path": "rendered", "kind": "rendered-preview", "status": "checked"},
    {"path": "montage.png", "kind": "review-image", "status": "checked"}
  ],
  "checks": [
    {"command": "ppt qa deck.pptx --rendered-dir rendered --fail-on-issues --json", "status": "passed"}
  ]
}
```

## Completion checklist

- Final `.pptx` exported successfully.
- Important slide text remains editable and readable.
- Charts, tables, and key layout blocks render correctly in preview.
- Quality gates pass, including style gate thresholds.
- Advanced style gate passes, including composition and token consistency.
- `EVIDENCE_INDEX.json` records final artifacts and checks when a deck was created or materially changed.
- Final response stays concise but includes the `.pptx` link and the verification evidence used, or an explicit blocker when verification could not run.
- In CLI-first mode, include `deck.plan.json` path and exact Rust commands used so the result is reproducible.

Final response template (use 3-4 bullets):

- `What changed:` one concrete sentence.
- `What matters:` one to two high-impact takeaways with data or evidence.
- `How to use:` who should present and in which scenario.
- `What next:` immediate action with owner/timeline when relevant.
- `Design compliance:` one line confirming grid/type/color/chart/imagery/motion checks.
- `Run reproducibility:` `deck.plan.json` path, commands used, and exit status (run id optional).

## Hard constraints

- Do not use legacy JS/Python deck generators or screenshot-only slide authoring as the default path here.
- Do not mention internal builder/package details in the final response unless the user asks.
- Do not dump full scratch artifacts by default; provide key preview evidence when visual validation is required.
- Do not search or preload the rest of `skills/`.

## Native PPTX References

- [references/native-pptx/workflow.md](references/native-pptx/workflow.md)
- [references/native-pptx/method.md](references/native-pptx/method.md)
- [references/native-pptx/install.md](references/native-pptx/install.md)
- [references/native-pptx/checklist.md](references/native-pptx/checklist.md)
- [references/native-pptx/design-system.md](references/native-pptx/design-system.md)
- [references/native-pptx/layout-patterns.md](references/native-pptx/layout-patterns.md)
- [references/native-pptx/visualization_patterns.md](references/native-pptx/visualization_patterns.md)
- [references/native-pptx/rust-cli.md](references/native-pptx/rust-cli.md)
