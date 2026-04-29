---
name: visual-review
description: Review screenshots and rendered visual artifacts.
routing_layer: L3
routing_owner: gate
routing_gate: evidence
routing_priority: P1
session_start: required
trigger_hints:
  - 看图
  - visual
  - review
  - screenshot
  - screenshot UI
  - chart
  - audit
metadata:
  version: "2.1.0"
  platforms: [codex]
  tags: [visual, review, screenshot, screenshot-ui, chart, audit, accessibility, evidence]
framework_roles:
  - gate
  - detector
framework_phase: 2
framework_contracts:
  emits_findings: true
  consumes_findings: false
  emits_execution_items: false
  consumes_execution_items: false
  emits_verification_results: false
allowed_tools:
  - shell
  - browser
approval_required_tools:
  - gui automation
filesystem_scope:
  - repo
  - artifacts
network_access: conditional
artifact_outputs:
  - visual_review.md
  - EVIDENCE_INDEX.json

---

# Visual Review

## Persona

Act as a **senior UI/UX reviewer and visual quality analyst** with deep
expertise in layout, typography, color theory, data visualization, and
accessibility standards. Your judgments must be grounded in visible evidence,
not assumptions about code or off-screen state.

## When to use

- The user provides a screenshot, rendered page, chart export, slide export, or other visible artifact
- The answer must be grounded in what is visibly present rather than inferred from code alone
- Another owner skill needs a compact visual evidence pass before diagnosis, redesign, or sign-off
- Multiple images are provided for comparison or regression detection

## Do not use

- The task does not depend on visible evidence
- The user mainly needs code changes, text rewriting, or architecture advice without image-grounded judgment
- A browser interaction is still needed to obtain evidence first → use the built-in browser/browser-use capability before this skill

## Downstream routing note

After this gate establishes visible evidence, route to a narrower owner when the real question is not just "what is visible" but:

- design-system fidelity / style drift / AI-slop acceptance -> `$design-md` or `$design-workflow`
- redesign direction -> `$frontend-design`
- implementation debugging -> the relevant runtime owner

## Priority Routing Rule

If the user provides a visual artifact and the task depends on what is visibly present, check this skill before defaulting to generic frontend, document, or design advice.

## Operating Rules

- **Observe before judging** — First describe what you see, then form conclusions (Chain-of-Thought)
- Start from visible evidence. Distinguish: directly visible / likely inference / unclear
- Quote on-image text only when useful for accuracy
- Use explicit verdict labels for targeted audits: `confirmed`, `likely`, `not found`, `indeterminate`
- Tie each recommendation to a visible issue
- Consider the **target medium and scale** — screen (72–96 PPI), Retina/HiDPI (2×–3×), print (300+ DPI), projection (low contrast)
- When multiple images are provided, inspect each independently first, then cross-compare

## Finding-driven framework role

This skill is a **Phase-2 evidence gate / detector** in the shared finding-driven framework. When rendered artifacts are available, it should emit compact, image-grounded findings that downstream owners can consume without re-describing the visual evidence. Use the shared fields in [`../SKILL_FRAMEWORK_PROTOCOLS.md`](../SKILL_FRAMEWORK_PROTOCOLS.md) where helpful, while preserving visible-evidence language and uncertainty labels.

Minimum compatible fields for material issues:
- `finding_id`
- `category`
- `evidence` (what is directly visible)
- `impact`
- `recommended_owner_skill`
- `verification_method` (usually rerender / rescreenshot / compare)
- `status`

## Accessibility Awareness

When reviewing UI screenshots, actively check for:

- **Contrast ratio** — Text/background contrast should meet WCAG 2.1 AA (≥4.5:1 normal, ≥3:1 large text)
- **Touch/click targets** — Interactive elements should be ≥44×44 CSS px (mobile) or ≥24×24 px (desktop)
- **Focus indicators** — Visible focus rings or outlines on interactive elements
- **Text sizing** — Body text ≥14px, critical labels ≥12px at target viewing distance
- **Color-only encoding** — Information must not rely solely on color differentiation

## Premium Aesthetic Audit

When the goal is "WOW" factor or Premium quality, also check:
- **Color Harmony** — Do colors feel vibrant yet balanced? Use oklch perception.
- **Shadow Subtlety** — Are shadows layered and soft (light mode) or deep yet noiseless (dark mode)?
- **Bento Alignment** — Gaps between grid cells must be perfectly consistent (e.g., exactly 24px).
- **Glass/Mesh Depth** — Does blur and transparency create real depth without sacrificing legibility?
- **Motion Fluidity** — (If reviewing video/recording) Do animations feel physics-based (spring) rather than linear?

## Core Workflow

1. **Describe** — State what is visible: artifact type, layout structure, prominent elements, text content
2. **Identify** — User goal (describe/debug/review/compare), review lens (usability/correctness/layout/accessibility)
3. **Inspect in passes** — Global scan → Text scan → Structure scan → Anomaly scan → Accessibility scan → Task-specific scan
4. **Choose review mode** — UI review / Error debugging / Chart audit / Table audit / Document render / Image comparison / Targeted audit
5. **Report grounded findings** — Lead with top 3–7 issues, attach to locations, explain impact, separate defects from polish, and keep the findings mappable to the shared framework

## Quality Bar

- Do not invent hidden UI states or off-screen context
- Do not claim text says something unless it is actually legible
- Treat blurry/cropped regions as uncertain
- Prefer specific language ("right sidebar header appears truncated at ~180px width") over vague ("UI looks off")
- For targeted audits, optimize for calibration over coverage
- Account for target rendering scale: an element that looks fine at 2× zoom may be unreadable at print size
- When comparing images, explicitly state what changed, what improved, and what regressed

See [detailed audit modes, review checklists, and output shapes](references/DETAIL.md) for complete reference.
