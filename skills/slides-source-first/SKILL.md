---
name: slides-source-first
description: |
  Build or revise slide workflows where source-of-truth authoring and artifact consistency matter
  more than any one presentation format. Use when the user wants source-first slide generation,
  consistent export paths, or cross-format slide maintenance across markdown, HTML, Beamer, or PPTX.
  Use this for workflow choice; keep in-place `.pptx` editing on existing decks in `officecli` and
  source-first `.pptx` authoring in `$ppt-pptx`.
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - source-first slide generation
  - consistent export paths
  - cross-format slide maintenance across markdown
  - HTML
  - Beamer
  - PPTX
  - slides
  - source first
  - presentation
  - export
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - slides
    - source-first
    - presentation
    - export
    - workflow
risk: low
source: local
---

# slides-source-first

This skill owns source-first slide workflow decisions and maintenance.

## When to use

- The user wants one source of truth across slide formats
- The task is slide workflow consistency, not just one deck implementation

## Do not use

- Markdown-authored deck implementation -> use `$ppt-markdown`
- HTML slide authoring/export -> use `$ppt-html-export`
- Editable PowerPoint generation -> use `$ppt-pptx`
- Existing `.pptx` inspection or in-place patching -> use `officecli`
