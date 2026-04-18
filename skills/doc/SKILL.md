---
name: doc
description: |
  Read, create, edit, repair, and review `.docx` Word documents when layout and
  Word-native structure matter.
  Use when the user wants structured Word edits, 模板化文档生成, 表格或版式修复,
  or render-aware `.docx` verification with `python-docx`, LibreOffice, and
  `scripts/render_docx.py`. As an artifact gate, check this skill early at
  conversation start / first turn when the primary artifact is a `.docx`.
routing_layer: L3
routing_owner: gate
routing_gate: artifact
routing_priority: P1
session_start: required
runtime_requirements:
  python:
    - pdf2image
  commands:
    - soffice
    - pdftoppm
metadata:
  version: "2.0.0"
  platforms: [codex]
  tags:
    - docx
    - word
    - python-docx
    - pagination
    - document-layout
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
  - docx_review.md
  - EVIDENCE_INDEX.json
---
# doc

At conversation start or first turn, check this artifact gate early whenever the main object is a `.docx` file or the workflow should stay Word-native.


This skill owns `.docx` work where professional document structure and rendered appearance both matter.

## Priority routing rule

If the primary artifact is a `.docx` file and the task is to read, generate,
edit, repair, or review that document with structure and layout intact, check
this skill before generic writing, PDF, or visual-only workflows.

In that case:

1. this skill owns the Word-native structure-preserving workflow
2. paired skills should only layer on top after the `.docx` artifact is handled
   correctly

## When to use

- The task involves reading, creating, editing, or reviewing a `.docx` file
- The user cares about styles, headings, tables, numbering, pagination, or visual layout
- The user wants structured document edits rather than raw XML hacking
- The user wants render-aware QA after document changes
- Best for requests like:
  - "改这个 Word 文档"
  - "生成一个 docx 报告"
  - "检查这个 DOCX 的表格和分页"

## Do not use

- The file is primarily a PDF artifact → use `$pdf`
- The user wants a slide deck rather than a document
- The task is plain text editing with no Word/document structure concerns
- The user pasted text directly and only wants rewriting

## Reference

For detailed workflow, examples, and implementation guidance, see [references/detailed-guide.md](./references/detailed-guide.md).
