---
name: refresh
description: "Build the next-turn execution prompt, copy it to the clipboard, and reply with one fixed sentence"
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

Build the next-turn execution prompt from the current repository context, copy it
to the macOS clipboard, and return one fixed confirmation sentence.

## Instructions

1. First capture the current workspace root with:

```bash
pwd
```

2. If `scripts/claude_memory_bridge.py` exists in the current repository, run:

```bash
python3 scripts/claude_memory_bridge.py refresh-workflow --json
```

3. If the bridge returns a successful clipboard copy, reply with exactly:

```text
下一轮执行 prompt 已准备好，并且已经复制到剪贴板。
```

4. If the bridge returns `workflow_prompt` but did not copy it successfully,
   copy `workflow_prompt` to the macOS clipboard yourself with a short `python3`
   snippet that calls `pbcopy`, then reply with exactly:

```text
下一轮执行 prompt 已准备好，并且已经复制到剪贴板。
```

5. If the repository does not contain `scripts/claude_memory_bridge.py`, build a
   manual next-turn execution prompt from the current conversation plus local
   anchors such as `.supervisor_state.json`,
   `artifacts/current/SESSION_SUMMARY.md`,
   `artifacts/current/NEXT_ACTIONS.json`, and
   `artifacts/current/TRACE_METADATA.json` when they exist.

6. The manual prompt should tell the next conversation to resume work
   immediately and include:
   - current workspace path
   - current objective
   - remaining work
   - next concrete action
   - the exact execution line `参考prompt设置的串并行分工，直接开始执行！`

7. Copy the manual prompt to the macOS clipboard with a short `python3` snippet
   that calls `pbcopy`, then reply with exactly:

```text
下一轮执行 prompt 已准备好，并且已经复制到剪贴板。
```

## Constraints

- Do not rewrite root continuity artifacts
- Do not refresh `.codex/memory/CLAUDE_MEMORY.md`
- Do not mention memory refresh, summary, `/clear`, missing bridge support, or internal diagnostics in the user-visible reply
- Keep behavior aligned with the global Claude `refresh` command

## Usage

```text
$refresh
```
