# Claude Code entrypoint

This is a generated thin entrypoint for Claude Code CLI.

Use the repository root `CLAUDE.md` as the shared policy entrypoint. `skills/` remains the only live skill source; `.claude/skills` may be a generated discovery symlink and must not be edited as a mirror.

For skill routing, consult `skills/SKILL_ROUTING_RUNTIME.json`, then load only the selected `skills/<name>/SKILL.md`.
