# Claude Local Overlay

@../AGENT.md
@../.codex/memory/CLAUDE_MEMORY.md

## Claude Local Overlay

Use this directory only for Claude host-private files such as:

- `.claude/settings.json`
- `.claude/agents/`
- `.claude/hooks/`
- `../.codex/memory/CLAUDE_MEMORY.md`

Claude-specific hooks may refresh the imported memory projection, but must not
fork the shared framework policy or memory ownership.

Generated-first maintenance rule:

- Edit `scripts/materialize_cli_host_entrypoints.py` first for
  `.claude/settings.json`, `.claude/hooks/README.md`, and `.claude/hooks/*.sh`.
- Treat those files as materialized outputs, not hand-authored truth.
- `.claude/agents/*.md` stays manually maintained unless a file says otherwise.
- Event-level lifecycle decisions live in `.claude/hooks/README.md`.
