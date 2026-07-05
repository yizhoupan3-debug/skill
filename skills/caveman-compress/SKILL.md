---
allowed_tools:
- Bash
- Read
- Write
name: caveman-compress
description: >
  压缩自然语言文件（.md/.txt/.tex）为 caveman 格式节省 input token。
  纯 Rust CLI。备份 .original.md。保留 code block/URL/path 不变。
  触发：/caveman-compress <filepath>
scene: file_compression
risk: low
routing_gate: none
routing_layer: L3
routing_owner: owner
routing_priority: P3
session_start: on_demand
trigger_hints:
- /caveman-compress
- 压缩文件
- 压缩 memory
- 压缩文档
- compress file
- compress memory
---

# Caveman Compress

## Purpose

Compress natural language files (.md, .txt, .tex) into caveman-speak to reduce input tokens.
Compressed version overwrites original. Backup saved as `<filename>.original.md`.

## Trigger

`/caveman-compress <filepath>` or when user asks to compress a memory/doc file.

## Process

From project root, run:
```
cargo run --manifest-path core/caveman/Cargo.toml -- compress <absolute_filepath>
```

The CLI will:
1. Detect file type (extension check: .md/.txt/.tex/.typ, or no extension)
2. Compress prose sections
3. Validate output (code blocks, URLs, structure preserved)
4. Backup original to `<path>.original.md`
5. Overwrite original with compressed version

On validation failure: targeted patch only (no recompression). Retry up to 2 times.
If still failing after 2 retries: report error, leave original untouched.

## Compression Rules

### Remove
- Articles: a, an, the
- Filler: just, really, basically, actually, simply, essentially, generally
- Pleasantries: "sure", "certainly", "of course", "happy to"
- Hedging: "it might be worth", "you could consider", "the reason is because"
- Redundant: "in order to" → "to", "make sure to" → "ensure"
- Connective fluff: "however", "furthermore", "additionally"

### Preserve EXACTLY (never modify)
- Code blocks (fenced ``` and indented)
- Inline code (`backtick content`)
- URLs and links
- File paths
- Commands
- Technical terms (library names, API names, protocols, algorithms)
- Proper nouns, dates, version numbers, numeric values
- Environment variables ($HOME, NODE_ENV)

### Preserve Structure
- All markdown headings (keep exact heading text, compress body below)
- Bullet point hierarchy
- Numbered lists
- Tables (compress cell text, keep structure)
- Frontmatter/YAML headers

### Compress Style
- Short synonyms: "big" not "extensive", "fix" not "implement a solution"
- Fragments OK: "Run tests before commit" not "You should always run tests before committing"
- Drop "you should", "make sure to", "remember to" — just state action
- Merge redundant bullets

## Boundaries

- ONLY compress: .md, .txt, .tex, .typ, .typst, extensionless
- NEVER modify: .py, .js, .ts, .json, .yaml, .yml, .toml, .env, .lock, .css, .html, .xml, .sql, .sh
- Mixed content (prose + code): compress ONLY prose sections
- If unsure whether something is code or prose: leave unchanged
- Never compress FILE.original.md (skip it)
