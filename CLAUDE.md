# CLAUDE.md

This is the generated Claude Desktop / Claude Code policy entrypoint for the shared skill system.

This repository uses a single-source skill system shared by Codex CLI, Codex Desktop, Claude Code CLI, and Claude Desktop. The repository root is the policy root.

- `skills/` is the only live skill source. Do not treat `.claude/skills`, `.codex/skills`, or `artifacts/*-skill-surface/skills` as source of truth.
- Before loading skill bodies, consult `skills/SKILL_ROUTING_RUNTIME.json` and then read only the matched `skills/<name>/SKILL.md`.
- Host-facing skill surfaces are Rust-generated thin projections for discovery only; regenerate them through router-rs host entrypoint sync or install-skills commands instead of editing them directly.
- Claude Code CLI uses `.claude/CLAUDE.md` and `.claude/settings.json` as the minimal generated entrypoint and hook settings.
- Claude Desktop should use repo-managed stdio MCP servers from `router-rs` and the same `skills/` runtime source; do not add copied skill mirrors or host-private policy forks.
- Keep Claude Desktop and Claude Code integration minimal: prefer generated pointers, symlinks, MCP stdio, and aliases over copied mirrors.
