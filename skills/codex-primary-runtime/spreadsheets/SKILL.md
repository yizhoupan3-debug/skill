---
name: spreadsheets
description: |
  Route workbook-native spreadsheet artifact work before choosing a narrower implementation lane.
  Use this artifact gate at 每轮对话开始 / first-turn / conversation start when the main artifact is an Excel workbook or spreadsheet-like file and formulas, formatting, charts, recalculation, or workbook fidelity matter.
routing_layer: L3
routing_owner: gate
routing_gate: artifact
routing_priority: P1
session_start: required
trigger_hints:
  - Excel
  - spreadsheet
  - xlsx
  - csv
  - sheet review
  - xls
  - tsv
metadata:
  version: "2.0.0"
  platforms: [codex]
  tags:
    - excel
    - spreadsheet
    - xlsx
    - xls
    - csv
    - tsv
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
  - final_workbook.xlsx
  - EVIDENCE_INDEX.json
---

# spreadsheets

At every-conversation-start / first turn, check this artifact gate early whenever the primary artifact is an Excel workbook or spreadsheet-like file and the workflow should stay spreadsheet-native.

This skill owns the canonical workbook entry gate. Use it to absorb generic Excel / spreadsheet asks first, then either keep the default `@oai/artifact-tool` path or reroute to a narrower lane when the user explicitly wants Python/openpyxl/pandas/LibreOffice-style handling.

## When to use

- The primary artifact is `.xlsx`, `.xls`, `.csv`, or `.tsv`
- The user says Excel / spreadsheet / xlsx and the implementation lane is not chosen yet
- The user wants a real workbook, not just flattened tabular output
- The task needs formulas, formatting, charts, tables, dashboards, or workbook QA
- The request is about creating, editing, analyzing, or reviewing spreadsheet artifacts

## Do not use

- The task is plain data wrangling with no workbook artifact requirement
- The user explicitly wants an `openpyxl` / `pandas` / LibreOffice-driven compatibility workflow; hand off to `$xlsx`
- The user only wants narrative analysis without a spreadsheet deliverable
- The artifact is primarily a document, slide deck, or PDF

## Core contract

- Treat this as the canonical first check for generic Excel / spreadsheet requests before any narrower workbook owner claims the task.
- If the user explicitly asks for `openpyxl`, `pandas`, workbook-structure audit scripts, or LibreOffice render-based repair, reroute to `$xlsx`.
- Use the installed `@oai/artifact-tool` JS workflow for workbook authoring, editing, rendering, and `.xlsx` export by default.
- Keep calculations auditable: prefer spreadsheet formulas for workbook logic that users may edit later.
- Use real spreadsheet structures when they add value: tables, filters, freeze panes, validation, conditional formats, and charts.
- Keep the user-facing answer focused on the workbook result and final `.xlsx` link(s), not the internal builder path.
- Treat rendered inspection and formula scanning as part of the default finish line, not optional polish.

## Required workflow

1. Confirm the workbook goal, target audience, and whether the implementation lane is still undecided.
2. If the user explicitly wants Python/openpyxl/pandas/LibreOffice handling, reroute to `$xlsx` and stop this gate there.
3. Otherwise create or import the workbook through the artifact-tool path.
4. Build inputs, structure, formulas, and formatting in that order.
5. Add charts or KPI visuals when the prompt implies summary analysis or presentation-ready output.
6. Inspect key ranges, scan for formula errors, and render the important sheets once before export.
7. Export the final `.xlsx` and stop once the workbook is correct and legible.

## Quality rules

- Avoid clipped numbers, broken formulas, unreadable dashboard areas, and default blank sheets.
- Keep layouts bounded; do not let autofit or wrapping explode row heights or column widths.
- Use concise, professional worksheet organization: summary first when appropriate, then inputs/assumptions, then detail tabs.
- For editable templates, keep blank states neutral and non-misleading.

## Completion criteria

- Workbook content is populated and formulas compute
- No obvious formula errors in key scanned ranges
- Important sheets render legibly
- Final `.xlsx` exported successfully
- Final response contains only a concise result summary and final workbook link(s)

## References

- [references/workflow.md](./references/workflow.md) for the compact build/verify loop
- [references/api-surface.md](./references/api-surface.md) for the high-value artifact-tool workbook surface
- [style_guidelines.md](./style_guidelines.md) for spreadsheet presentation conventions
- [templates/financial_models.md](./templates/financial_models.md) for finance/model-specific guidance
