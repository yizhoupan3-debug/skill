---
name: pdf
description: |
  Read, create, edit, repair, and review PDFs when rendering and page layout
  matter.
  Use when the user needs PDF inspection, generation, layout-aware extraction, or rendered-page
  verification with tools such as `pdftoppm`, `pdfplumber`, `pypdf`, and
  `reportlab`. As an artifact gate, check this skill early at 每轮对话开始 / first-turn / conversation start
  whenever the main object is a PDF.
routing_layer: L3
routing_owner: gate
routing_gate: artifact
routing_priority: P1
session_start: required
metadata:
  version: "2.1.0"
  platforms: [codex]
  tags:
    - pdf
    - rendering
    - layout
    - typography
    - reportlab
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
  - python
approval_required_tools:
  - file overwrite
filesystem_scope:
  - repo
  - artifacts
network_access: conditional
artifact_outputs:
  - pdf_review.md
  - EVIDENCE_INDEX.json
---

# pdf

This skill owns PDF work where final rendered appearance matters more than raw text alone.

## Priority routing rule

If the primary artifact is a PDF and the task is to inspect, generate, repair,
extract, or visually verify that PDF, check this skill before generic document,
visual-review, or domain advice.

In that case:

1. this skill owns the PDF-native workflow and render-aware handling
2. paired skills should only layer on top after the PDF artifact has been
   handled correctly

## When to use

- The user wants to read, inspect, generate, edit, or repair a PDF
- The user cares about page layout, typography, clipping, overlap, or render quality
- The user wants render-based checking rather than plain text extraction only
- The user asks to extract PDF content but layout or structure still matters
- Best for requests like:
  - "检查这个 PDF 排版"
  - "生成一个 PDF"
  - "把这个 PDF 读出来并看看有没有渲染问题"

## Do not use

- The task is really about `.docx` Word editing → use `$doc`
- The task is specifically a visual screenshot/UI review rather than PDF artifact work → use `$visual-review`
- The user only wants plain-text summarization of text they already pasted into chat
- The file is not actually a PDF

## Reference

For detailed workflow, examples, and implementation guidance, see [references/detailed-guide.md](./references/detailed-guide.md).
