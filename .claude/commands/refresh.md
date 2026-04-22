---
description: Generate and copy the next-turn execution prompt with the Rust refresh command.
allowed-tools: Bash(python3 scripts/router_rs_runner.py *)
---

Run:

`python3 scripts/router_rs_runner.py --framework-refresh-json --claude-hook-max-lines 4`

Then reply with exactly:
`下一轮执行 prompt 已准备好，并且已经复制到剪贴板。`
