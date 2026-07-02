---

allowed_tools:
- shell
- rust
- python
approval_required_tools:
- file overwrite
description: Handle layout-aware PDF reading, editing, repair, and review.
metadata:
  platforms:
  - supported
  tags:
  - pdf
  - rendering
  - layout
  - typography
  - reportlab
  version: '2.1.0'
name: pdf
scene: general
network_access: conditional
risk: low
routing_gate: artifact
routing_layer: L3
routing_owner: owner
routing_priority: P1
runtime_requirements:
  commands:
  - pdf
session_start: required
source: project
trigger_hints:
- PDF 转文字
- pdf layout
- pdf
- rendering
- reportlab
- typography
- 合并 PDF
- 编辑 PDF
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

## Rust batch path

- Batch catalog: `pdf-batch/catalog.json` under `${SKILL_FRAMEWORK_ROOT}` artifacts.
- 禁止在 skill 正文默认写 `cargo run` 作为操作员主路径；已安装 `mcp-pdf` 二进制优先。

## Shared artifact protocol

Follow the shared artifact rules in
[`primary-runtime/references/artifact-protocol.md`](../primary-runtime/references/artifact-protocol.md).

## Reference

For detailed workflow, examples, and implementation guidance, see [references/detailed-guide.md](./references/detailed-guide.md).

## Hard constraints

- PDF 修复前必须先做 render-aware 检查（非仅文本提取），确认渲染层问题
- 使用 Rust `pdf` CLI 优先于 Python 工具，除非 Rust 工具不支持特定操作
- 覆盖写入 PDF 前必须确认用户授权（`approval_required_tools: file overwrite`）
- PDF 生成必须指定显式输出路径，不得写入临时目录后遗忘
- 损坏 PDF 的恢复必须保留原始文件副本，不得原地修复
