# Claude Entrypoint Maintenance

Claude Code has several visible entrypoints in this repo, but they are not
several sources of truth. Treat them as a small projection graph:

| Surface | Role | Edit here? |
| --- | --- | --- |
| `AGENT.md` | Shared policy for Codex, Claude, and Gemini | Yes, for global behavior |
| `CLAUDE.md` | Thin Claude startup proxy | No, regenerate from `router-rs` |
| `.claude/settings.json` | Project Claude hook and MCP projection | No, regenerate from `router-rs` |
| `.claude/hooks/README.md` | Generated hook contract and validation notes | No, regenerate from `router-rs` |
| `.claude/commands/refresh.md` | Explicit continue / resume command | No, regenerate from `router-rs` |
| `.claude/agents/*.md` | Retired project subagent projection | No, keep removed |
| `.claude/settings.local.json` | Machine-local project override | Yes, but do not commit |
| `~/.claude/settings.json` | User-wide Claude preference | Yes, but keep it host-private |

## Maintenance Rule

- Put global behavior in `AGENT.md`.
- Put Claude generated output changes in `scripts/router-rs/src/claude_hooks.rs`,
  then run:

```sh
./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml --sync-host-entrypoints-json --repo-root "$PWD"
```

- Put personal shortcuts, notifications, and local approvals in
  `~/.claude/settings.json` or `.claude/settings.local.json`, not committed
  generated project settings.
- Do not edit `CLAUDE.md`, `.claude/settings.json`,
  `.claude/hooks/README.md`, `.claude/commands/*.md`,
  `.codex/hooks.json`, or `.codex/host_entrypoints_sync_manifest.json`
  as long-term truth. They will be overwritten by sync.

## Current Kept Projections

- `CLAUDE.md`: one lean Claude entrypoint.
- `.claude/settings.json`: only installs `PreToolUse` and `ConfigChange`.
- `.claude/commands/refresh.md`: the only explicit continue/resume slash command.
- `.claude/skills`: symlink into shared `skills/` for Claude-native skill
  discovery without duplicating project policy.

## Retired Or Redundant Projections

These are intentionally not steady-state entrypoints:

- `.claude/CLAUDE.md`
- `configs/claude/CLAUDE.md`
- `.claude/agents/*.md`
- `.claude/commands/autopilot.md`
- `.claude/commands/background_batch.md`
- `.claude/commands/deepinterview.md`
- `.claude/commands/latex-compile-acceleration.md`
- `.claude/commands/team.md`
- `.claude/commands/deepreview.md`
- `.claude/hooks/*.sh`
- automatic `UserPromptSubmit`, `SessionEnd`, `PostToolUse`,
  `PostToolUseFailure`, and `StopFailure` project hooks
- `scripts/materialize_cli_host_entrypoints.py`
- OMC / `oh-my-claudecode` prompt, plugin, command, and `.omc/**` runtime
  surfaces

If one of these reappears, remove it through the Rust host-entrypoint sync path
instead of wiring another compatibility layer around it.
