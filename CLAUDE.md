# Claude Code Entry Proxy

This file exists because Claude Code discovers `CLAUDE.md`.

@AGENT.md
@.codex/memory/CLAUDE_MEMORY.md

## Claude Project Entry

Use `.claude/` only for Claude host-private files such as:

- `.claude/settings.json`
- `.claude/agents/`
- `.claude/commands/`
- `.claude/hooks/`

Claude-specific hooks may refresh the imported memory projection, but must not
fork the shared framework policy or memory ownership.

Generated-first maintenance rule:

- Edit `scripts/materialize_cli_host_entrypoints.py` first for
  `.claude/settings.json`, `.claude/commands/*.md`, `.claude/hooks/README.md`,
  and `.claude/hooks/*.sh`.
- Treat those files as materialized outputs, not hand-authored truth.
- `.claude/agents/*.md` stays manually maintained unless a file says otherwise.
- Event-level lifecycle decisions live in `.claude/hooks/README.md`.
