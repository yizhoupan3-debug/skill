# Cursor Hooks — Legacy (Python)

This directory holds the **retired** Python implementation of the Cursor
review-subagent gate. It is preserved as a break-glass fallback and for
git history; **the active implementation is Rust**.

## Active path

`.cursor/hooks.json` now invokes `router-rs cursor hook --event=<name>` for
all 8 events (beforeSubmitPrompt, subagentStart, subagentStop, postToolUse,
afterAgentResponse, preCompact, sessionEnd, stop).

The Rust implementation lives at:

- `scripts/router-rs/src/cursor_hooks.rs` (state machine + 12 unit tests)
- `scripts/router-rs/src/main.rs` (`RouterCommand::Cursor { hook }` subcommand)

## Equivalence evidence

A 14-case semantic-equality smoke (Cursor + parallel + override + reject_reason
+ narrow_path + en review + stop + sessionEnd + preCompact + subagent
start/stop + postToolUse + afterAgentResponse) was run with identical input
payloads against both implementations on 2026-05-08; all 14 outputs were
JSON-equal under `python3 json.loads`. The cosmetic difference (Python
`json.dumps` adds spaces after separators while Rust `serde_json` does not)
does not affect downstream consumers.

## How to fall back manually

If the Rust binary cannot be built or behaves unexpectedly:

```sh
echo '{"session_id":"...","cwd":"<repo>","prompt":"..."}' \
  | /usr/bin/env python3 .cursor/hooks/legacy/review_subagent_gate.py --event beforeSubmitPrompt
```

To restore Python as the active gate, edit `.cursor/hooks.json` and replace
each event command with the original `python3` invocation. The old commands
are preserved in git history at the previous revision of this file.

## Legacy tests

`.cursor/hook-tests/legacy/test_cursor_hooks.py` still drives this Python
implementation (paths updated to `.cursor/hooks/legacy/`). It is **not** part
of the default test surface; run it manually only when verifying legacy
parity.
