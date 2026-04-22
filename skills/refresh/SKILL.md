---
name: refresh
description: "Use the Rust refresh command to generate and copy the next-turn execution prompt"
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - /refresh
  - $refresh
  - refresh
  - 下一轮执行 prompt
  - 复制到剪贴板
---

# Refresh

Codex-side equivalent of Claude Code `/refresh`.

## When to use

- The user wants to prepare the next-turn execution prompt for the current repository
- The prompt should be copied to the macOS clipboard
- The user expects one fixed confirmation sentence

## Do not use

- The task is to refresh shared memory artifacts or rewrite continuity state
- The task is generic note-taking, summary-only output, or unrelated skill maintenance

## Objective

用 Rust refresh 命令生成下一轮执行 prompt，复制到剪贴板，并返回固定确认句。

## Instructions

1. Run:

```bash
python3 scripts/router_rs_runner.py --framework-refresh-json --claude-hook-max-lines 4
```

2. Then reply with exactly:

```text
下一轮执行 prompt 已准备好，并且已经复制到剪贴板。
```

## Constraints

- Do not rewrite root continuity artifacts
- Do not refresh `.codex/memory/CLAUDE_MEMORY.md`
- Do not mention memory refresh, summary, `/clear`, or internal diagnostics in the user-visible reply
- Keep behavior aligned with the global Claude `refresh` command

## Usage

```text
$refresh
```
