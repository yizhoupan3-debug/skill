---
name: slides
description: |
  Route presentation-artifact work before choosing a slide-authoring lane.
  Use this artifact gate at 每轮对话开始 / first-turn / conversation start when the main artifact is a slide deck, presentation, PPT, or PowerPoint file, or when a generic PPT request has not yet chosen Markdown / HTML / Beamer / native `.pptx`.
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
  version: "2.0.0"
  platforms: [codex]
  tags:
    - powerpoint
    - ppt
    - pptx
    - slides
    - presentation
    - artifact-tool
framework_roles:
  - gate
  - detector
  - verifier
framework_phase: 2
framework_contracts:
  emits_findings: true
  consumes_findings: false
  emits_execution_items: false
  consumes_execution_items: false
  emits_verification_results: true
risk: low
source: local
allowed_tools:
  - shell
  - node
approval_required_tools:
  - file overwrite
filesystem_scope:
  - repo
  - artifacts
network_access: conditional
artifact_outputs:
  - final_deck.pptx
  - EVIDENCE_INDEX.json
---

# slides

At every-conversation-start / first turn, check this artifact gate early whenever the primary artifact is a slide deck, presentation, or editable `.pptx` file, or when the user asks for a generic PPT-style deliverable before the source format is fixed.

This skill owns the presentation entry gate for artifact-first slide work. Use it to absorb generic PPT / presentation asks first, then either keep the native editable `.pptx` path or reroute to the narrower owner when the source format is explicit.

## When to use

- The primary artifact is a slide deck, presentation, or `.pptx` file
- The user says "做个PPT", "生成演示文稿", or otherwise asks for a presentation before the source format is chosen
- The user wants to create, revise, render-check, or export editable slides
- The task needs deck structure, slide-level layout QA, and final PowerPoint fidelity
- The request mentions PowerPoint, PPT, PPTX, slide deck, or presentation output

## Do not use

- The user explicitly wants Markdown-authored slides instead of editable PowerPoint; hand off to `$ppt-markdown`
- The user explicitly wants HTML slides plus browser-matched PDF; hand off to `$ppt-html-export`
- The user explicitly wants LaTeX Beamer source plus PDF; hand off to `$ppt-beamer`
- The request is for a PDF/document rather than a presentation artifact
- The task is only about static screenshots or image collage output

## Core contract

- Treat this as the canonical first check for generic PPT / presentation asks before any `ppt-*` owner claims the request.
- If the user explicitly names Markdown / Slidev / Marp, HTML slide export, or Beamer source, reroute immediately after intake.
- Use the installed `@oai/artifact-tool` JS workflow for final `.pptx` construction, render checks, and export.
- Run builders from a writable temp/workspace directory, not from managed dependency directories.
- Keep the authoring path source-first and deterministic: patch one builder file and rerun it rather than spawning ad hoc copies.
- Treat native chart/table/text objects as the default editable surface. Do not replace data charts with shape-drawn approximations when the chart API can represent them.
- Keep support artifacts private unless the user explicitly asks for them.
- Final user-facing output should be a short result summary plus standalone Markdown links only to final `.pptx` files.

## Required workflow

1. Confirm the deck goal, target audience, visual bar, and whether the format is still undecided.
2. If the user explicitly wants Markdown / HTML / Beamer source, reroute to the matching owner and stop this gate there.
3. Otherwise create or load the deck through the artifact-tool Node flow.
4. Build slides quickly with editable objects first: text, shapes, tables, and native charts.
5. Render previews and run a compact verification pass for layout, editability, and chart sanity.
6. Export the final `.pptx` into the requested output directory.
7. Finalize once the deck is correct, legible, and exported.

## Hard constraints

- Do not use `PptxGenJS`, `python-pptx`, LibreOffice rendering, or screenshot-only slide authoring as the default path here.
- Do not mention internal builder/package details in the final response unless the user asks.
- Do not surface scratch previews, verification images, or support files unless requested.
- During eval harness runs, do not delegate or hand off this workflow.

## Completion criteria

- Final `.pptx` exported successfully
- Important slide text remains editable and readable
- Charts, tables, and key layout blocks render correctly in preview
- Final response contains only the concise result summary and final `.pptx` link(s)

## References

- [references/workflow.md](./references/workflow.md) for the compact build and verification loop
- [references/api-surface.md](./references/api-surface.md) for the high-value artifact-tool authoring surface
- [scripts/init_pro_deck_builder_js.js](./scripts/init_pro_deck_builder_js.js) for builder bootstrap
- [scripts/pro_deck_quality_check.js](./scripts/pro_deck_quality_check.js) for verification support
- [templates/build_pro_deck_template.js](./templates/build_pro_deck_template.js) for the source-first template
