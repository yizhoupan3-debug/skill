---
description: Enter the repo's Rust-owned deepinterview lane.
allowed-tools: Bash(python3 scripts/router_rs_runner.py *)
---

Treat `/deepinterview` as a thin Rust-first alias.
This command prefers the repo's resident router-rs stdio hot path.

Run:

`python3 scripts/router_rs_runner.py --framework-alias-json --framework-alias deepinterview --compact-output --claude-hook-max-lines 3 --repo-root "$PWD"`

Use `alias.state_machine` and `alias.entry_contract` as the working contract for this turn.
Only fall back to `alias.entry_prompt` if you need the compact prose form.
Prefer the Rust alias payload over opening long docs or restating OMC background.
Only open `skills/deep-interview/SKILL.md` if the alias payload is missing something you still need.
Keep execution inside the repo's native Rust/continuity lane.
