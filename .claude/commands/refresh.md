---
description: Build the next-turn execution prompt, copy it to the clipboard, and reply with one fixed sentence.
allowed-tools: Bash(python3 scripts/claude_memory_bridge.py *)
---

If `scripts/claude_memory_bridge.py` exists in the current repository, run:

`python3 scripts/claude_memory_bridge.py refresh-workflow --json`

If the bridge copied the prompt successfully, reply with exactly:
`下一轮执行 prompt 已准备好，并且已经复制到剪贴板。`

If the bridge did not copy it successfully, copy `workflow_prompt` to the macOS clipboard yourself, then reply with exactly:
`下一轮执行 prompt 已准备好，并且已经复制到剪贴板。`
