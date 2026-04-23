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
PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"; "$PROJECT_DIR"/scripts/router-rs/target/release/router-rs --framework-refresh-json --claude-hook-max-lines 4 --repo-root "$PROJECT_DIR"
```

If the release binary is missing, rerun the same command with:

```bash
PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"; "$PROJECT_DIR"/scripts/router-rs/target/debug/router-rs --framework-refresh-json --claude-hook-max-lines 4 --repo-root "$PROJECT_DIR"
```

If both resident binaries are missing, self-heal with:

```bash
PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"; cargo run --manifest-path "$PROJECT_DIR"/scripts/router-rs/Cargo.toml --release -- --framework-refresh-json --claude-hook-max-lines 4 --repo-root "$PROJECT_DIR"
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
