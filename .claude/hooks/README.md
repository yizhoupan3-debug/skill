# Claude Hooks Directory

Claude Code project hooks live here.

Generated-first maintenance:

- Update `scripts/router-rs/` first for Claude hook rules and host-entrypoint projections, then regenerate via `./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml --sync-host-entrypoints-json --repo-root "$PWD"`.
- Host entrypoint sync runs directly through `router-rs`; do not put a Python wrapper back in front of it.
- Treat `.claude/settings.json` and this README as materialized outputs.
- Long-term entrypoint map: `../../docs/claude_entrypoint_maintenance.md`.
- Project Claude subagents are retired here; use shared `skills/` and `AGENT.md` instead of `.claude/agents/*.md` projections.
- Codex uses `.codex/hooks.json` for a separate silent preflight guardrail
  layer on `Bash`; do not mirror Claude prompt or lifecycle hooks onto Codex.

Active hooks:

| Event | Runner | Purpose |
| --- | --- | --- |
| `PreToolUse` | `router-rs --claude-host-hook-command pre-tool-use` | Deny direct edits to generated host outputs and the imported Claude projection before `Edit`, `MultiEdit`, `Write`, or targeted `Bash` writes run. |
| `ConfigChange` | `router-rs --claude-host-hook-command config-change` | Warn when generated Claude host files were edited directly instead of regenerated from source. |
| `UserPromptSubmit` | command available, not installed | Use `/refresh` instead of automatic prompt injection for explicit continuity. |
| `SessionEnd` | command available, not installed | Memory projection refresh stays explicit to avoid hidden lifecycle writes. |
| `PostToolUse` | command available, not installed | Delta audits remain callable for tests/debugging but do not auto-run after every edit. |
| `PostToolUseFailure` | command available, not installed | Generated-surface protection is handled before edits, not by failure cleanup. |
| `PostToolBatch` | documented surface, not installed | Available for batch-level follow-up after parallel tool calls; avoid using it for single-tool checks already covered by `PostToolUse`. |
| `UserPromptExpansion` | documented surface, not installed | Available for slash/command expansion validation; repo slash command bodies stay static unless a concrete risk appears. |
| `StopFailure` | command available, not installed | Failure classification remains host-private and explicit. |

Only `PreToolUse` and `ConfigChange` are installed here so startup, prompt, edit,
and lifecycle paths stay lean. Use `/refresh` when continuity context is needed;
do not reintroduce automatic prompt or session-end injection without a concrete
repo invariant.
Reply tone, "讲人话" rules, closeout shape, and broad implementation philosophy
still live in `AGENT.md`, not in hooks.
Static behavior rules belong in `AGENT.md` or `CLAUDE.md`; these hooks exist
for deterministic guardrails and config-drift warnings.

Project hook principles:

- Keep project hooks for repo-specific invariants only.
- Keep hooks fast, especially `PreToolUse`, because it runs inside the agent
  loop.
- Use `matcher` first and `if` to narrow further, so hook handlers do not spawn
  on unrelated tool calls and normal edits stay fast.
- Automation hooks should be additive and short: guard a concrete repo surface
  or emit a concise warning, not essay-length prompt rewrites.
- Prefer `command` hooks for deterministic repo guardrails. `http`,
  `mcp_tool`, `prompt`, and `agent` hook handlers are supported by Claude Code,
  but should only be introduced for a real repo invariant that cannot be handled
  locally and cheaply.
- Use `asyncRewake` only for background checks that may discover a real problem
  after Claude has moved on; ordinary async audits should stay quiet unless they
  have actionable feedback.
- Keep durable implementation philosophy in `AGENT.md`; hook-time nudges should
  stay concrete, local to the current path, and local to the current delta.
- Put personal notifications and local approval shortcuts in `~/.claude/settings.json`
  or `.claude/settings.local.json`, not in committed project settings.
- Use `"$CLAUDE_PROJECT_DIR"`-anchored paths in hook commands and treat hook
  stdin JSON as untrusted input.
- Prefer `PreToolUse` deny over `PostToolUse` cleanup for protected files.
- Keep the generated-surface guard intentionally narrow so normal edits stay fast.
- Keep project hooks non-writing by default; use explicit commands for refresh
  and maintenance work.
- When debugging config drift, verify the installed hook set from Claude
  Code's `/hooks` menu before changing generated files.

Validation commands:

- `./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml --sync-host-entrypoints-json --repo-root "$PWD"`
  Expected: regenerate `AGENT.md`, `AGENTS.md`, `CLAUDE.md`, `.claude/settings.json`, `.codex/hooks.json`, and matching worktree projections directly from Rust.
- `printf '{"tool_name":"MultiEdit","tool_input":{"file_path":".claude/settings.json"}}
' | CLAUDE_PROJECT_DIR="$PWD" ./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml --claude-host-hook-command pre-tool-use --repo-root "$PWD"`
  Expected: stdout returns a JSON `permissionDecision: deny` payload.
- `printf '{"tool_name":"Bash","tool_input":{"command":"cp tmp .claude/settings.json"}}
' | CLAUDE_PROJECT_DIR="$PWD" ./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml --claude-host-hook-command pre-tool-use --repo-root "$PWD"`
  Expected: stdout returns a JSON `permissionDecision: deny` payload for the targeted write.
- `printf '{"tool_name":"Bash","tool_input":{"command":"printf x > .claude/settings.json"}}
' | CLAUDE_PROJECT_DIR="$PWD" ./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml --claude-host-hook-command pre-tool-use --repo-root "$PWD"`
  Expected: stdout returns a JSON `permissionDecision: deny` payload for shell redirection into a protected generated file.
- `printf '{"hook_event_name":"ConfigChange","source":"project_settings","file_path":".claude/settings.json"}
' | CLAUDE_PROJECT_DIR="$PWD" ./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml --claude-host-hook-command config-change --repo-root "$PWD"`
  Expected: audit-only stderr guidance about regenerating generated Claude host files; exit 0.
- `printf '{"hook_event_name":"ConfigChange","source":"project_settings","file_path":".claude/settings.json"}
' | ./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml --claude-hook-audit-command config-change --repo-root "$PWD"`
  Expected: JSON on stdout plus audit-only stderr guidance; exit 0.
- In Claude Code, run `/hooks`
  Expected: the project shows only `PreToolUse` and `ConfigChange` from `.claude/settings.json`.

Shared routing policy still comes from `../../AGENT.md`.
