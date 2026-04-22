---
description: Run the repo's durable background parallel-batch CLI and answer from its JSON result.
allowed-tools: Bash(python3 scripts/runtime_background_cli.py *)
---

Use `python3 scripts/runtime_background_cli.py` as the only host-level entrypoint
for this repository's durable background batch control.

Supported actions:

- Enqueue and wait:
  `python3 scripts/runtime_background_cli.py enqueue-batch --input-file <path>`
  or
  `python3 scripts/runtime_background_cli.py enqueue-batch --input-json '<json>'`
- Read one group:
  `python3 scripts/runtime_background_cli.py group-summary --parallel-group-id <id>`
- List all groups:
  `python3 scripts/runtime_background_cli.py list-groups`

Always relay the command's JSON result and then summarize it briefly in plain Chinese.
Do not invent batch state that the command did not return.
