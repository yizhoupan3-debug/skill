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
