---
name: doc
description: Handle layout-aware Word .docx creation, edits, and review.
routing_layer: L3
routing_owner: gate
routing_gate: artifact
routing_priority: P1
session_start: required
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - structured Word edits
  - 模板化文档生成
  - 表格或版式修复
  - render-aware
  - docx
  - word
  - pagination
  - document layout
runtime_requirements:
  commands:
    - cargo
    - soffice
    - pdftoppm
metadata:
  version: "2.0.0"
  platforms: [supported]
  tags:
    - docx
    - word
    - rust
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
  - rust
approval_required_tools:
  - file overwrite
filesystem_scope:
  - repo
  - artifacts
network_access: conditional
artifact_outputs:
  - docx_review.md
  - EVIDENCE_INDEX.json
  - ooxml-batch/catalog.json
  - ooxml-batch/results.jsonl
  - ooxml-batch/index.md
  - ooxml-batch/text/*.txt

---

# doc

At conversation start or first turn, check this artifact gate early whenever the main object is a `.docx` file or the workflow should stay Word-native.

This skill owns `.docx` work where professional document structure and rendered appearance both matter. The operational lane is Rust-first: inspect with `docx`, render with `render-docx`, and only use other editors when the actual document mutation requires it.

## Priority routing rule

If the primary artifact is a `.docx` file and the task is to read, generate,
edit, repair, or review that document with structure and layout intact, check
this skill before generic writing, PDF, or visual-only workflows.

In that case:

1. this skill owns the Word-native structure-preserving workflow
2. paired skills should only layer on top after the `.docx` artifact is handled
   correctly

## Rust CLI quick path

安装（一次性，对齐 `pdf` / `router-rs self install` 模式）：

```bash
bash ${SKILL_FRAMEWORK_ROOT}/scripts/install-ooxml-tool.sh
# 或：just install-ooxml
```

单文件阅读与结构 QA：

```bash
ooxml read-docx <docx>
ooxml read-docx <docx> --json --compact
ooxml docx <docx> --json
ooxml render-docx <docx> --output-dir <dir>
```

开发探测仍可用 `cargo run --manifest-path ${SKILL_FRAMEWORK_ROOT}/rust_tools/ooxml_parser_rs/Cargo.toml --bin ooxml -- read-docx <docx>`。

多文件批量阅读（**必须用已安装的 `ooxml` 二进制，禁止 `cargo run` batch**）：

```bash
ooxml batch --manifest <paths.json> --out-dir artifacts/current/<task_id>/ooxml-batch
ooxml batch --stdin-paths --out-dir artifacts/current/<task_id>/ooxml-batch <<'EOF'
/path/to/a.docx
/path/to/b.xlsx
EOF
```

`batch` 常用选项：`--jobs auto|N`（默认 `auto`；可设 `OOXML_BATCH_JOBS`）、`--resume`、`--fail-fast`、`--max-chars`、`--max-rows`（xlsx 默认 10000）。

`read-docx` emits linear text: headings, paragraphs, markdown tables, images, and footnote/comment text when present. `docx` reports structure counts and metadata. `render-docx` converts the document to PNG pages for layout review. Batch 产物：`catalog.json`、`results.jsonl`、`checkpoint.json`、`index.md`、`text/<sha256>.txt`。

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

- The file is primarily a PDF artifact - use `$pdf`
- The user wants a slide deck rather than a document
- The task is plain text editing with no Word/document structure concerns
- The user pasted text directly and only wants rewriting

## Shared artifact protocol

Follow the shared artifact rules in
[`primary-runtime/references/artifact-protocol.md`](../primary-runtime/references/artifact-protocol.md).

## Integrity checklist

- Confirm the heading outline did not drift.
- Confirm table count and table layout are still plausible.
- Confirm sections and page size are still expected.
- Confirm images, links, notes, and comments were not dropped accidentally.
- Re-render when pagination, tables, or visual layout matters.

## Hard constraints

- Do not treat extracted text as enough when layout is in scope.
- Do not silently flatten a Word document into plain text.
- Do not rewrite raw OOXML unless a structured edit path cannot preserve the needed feature.
- If render tooling is missing, say exactly what confidence is limited.

## Reference

For detailed workflow, examples, and implementation guidance, see [references/detailed-guide.md](./references/detailed-guide.md).
