# Claude Code Entry Proxy

This file exists because Claude Code discovers `CLAUDE.md`.

Keep startup lean. Do not add `@...` imports here.

Treat `.claude/**` as host-shell glue, not repository truth.
The recovery projection lives at `.codex/memory/CLAUDE_MEMORY.md` for `/refresh`
or manual resume, not default startup injection.

Generated-first maintenance rule:

- Edit `scripts/materialize_cli_host_entrypoints.py` first for host-entrypoint rendering, and update `scripts/router-rs/` first for Claude hook rules and contracts.
- Treat those files as materialized outputs, not hand-authored truth.
- `.claude/agents/*.md` stays manually maintained unless a file says otherwise.
- Event-level lifecycle decisions live in `.claude/hooks/README.md`.
