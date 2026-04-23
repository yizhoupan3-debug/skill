# Claude Hooks Directory

Claude Code project hooks live here.

Generated-first maintenance:

- Edit `scripts/materialize_cli_host_entrypoints.py` first.
- Treat `.claude/settings.json`, this README, and `.claude/hooks/*.sh` as
  materialized outputs.
- Manual Claude host guidance belongs in `.claude/agents/*.md` unless noted.

Active hooks:

| Event | Script | Purpose |
| --- | --- | --- |
| `UserPromptSubmit` | `user_prompt_submit.sh` | Inject a short coding-only implementation bias before Claude starts planning: direct implementation first, then performance/memory/fallback cleanup. |
| `PreToolUse` | `pre_tool_use_quality.sh` | Add a short implementation-quality reminder before editing runtime, hook, or test code so code is written with direct implementation and hot-path hygiene in mind. |
| `PreToolUse` | `pre_tool_use.sh` | Deny direct edits to generated host outputs and the imported Claude projection before `Edit`, `MultiEdit`, `Write`, or targeted `Bash` writes run. |
| `SessionEnd` | `session_end.sh` | Consolidate project-local memory, refresh the Claude projection, and repair stale terminal resume state when needed. |
| `ConfigChange` | `config_change.sh` | Warn when generated Claude host files were edited directly instead of regenerated from source. |
| `StopFailure` | `stop_failure.sh` | Emit a host-private hint for selected Claude stop failures without mutating shared continuity. |

Everything else stays intentionally uninstalled here so startup and tool turns remain lean.
Reply tone, "讲人话" rules, and closeout style live in `AGENT.md`, not in hooks.
Static behavior rules belong in `AGENT.md` or `CLAUDE.md`; these hooks exist
for deterministic guardrails, lightweight execution-time context, and lifecycle
maintenance.

Project hook principles:

- Keep project hooks for repo-specific invariants only.
- Keep hooks fast, especially `PreToolUse`, because it runs inside the agent
  loop.
- Automation hooks should be additive and short: inject narrow repo context or
  launch cheap follow-up work, not essay-length prompt rewrites.
- Put personal notifications and local approval shortcuts in `~/.claude/settings.json`
  or `.claude/settings.local.json`, not in committed project settings.
- Use `"$CLAUDE_PROJECT_DIR"`-anchored paths in hook commands and treat hook
  stdin JSON as untrusted input.
- Prefer `PreToolUse` deny over `PostToolUse` cleanup for protected files.
- Keep the generated-surface guard intentionally narrow so normal edits stay fast.
- Keep `SessionEnd` as the only writer hook here; the others are guards or alerts.
- When debugging config drift, verify the installed hook set from Claude
  Code's `/hooks` menu before changing generated files.

Validation commands:

- `printf '{"hook_event_name":"UserPromptSubmit","prompt":"继续优化这个 runtime，去掉补丁式保底并顺手看下内存和速度"}
' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/user_prompt_submit.sh`
  Expected: stdout emits a short coding-only context paragraph.
- `printf '{"tool_name":"Edit","tool_input":{"file_path":"scripts/router-rs/src/claude_hooks.rs"}}
' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/pre_tool_use_quality.sh`
  Expected: stdout returns a JSON `permissionDecision: allow` payload with `additionalContext`.
- `printf '{"tool_name":"MultiEdit","tool_input":{"file_path":".claude/settings.json"}}
' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/pre_tool_use.sh`
  Expected: stdout returns a JSON `permissionDecision: deny` payload.
- `printf '{"tool_name":"Bash","tool_input":{"command":"cp tmp .claude/settings.json"}}
' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/pre_tool_use.sh`
  Expected: stdout returns a JSON `permissionDecision: deny` payload for the targeted write.
- `printf '{"tool_name":"Bash","tool_input":{"command":"printf x > .claude/settings.json"}}
' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/pre_tool_use.sh`
  Expected: stdout returns a JSON `permissionDecision: deny` payload for shell redirection into a protected generated file.
- `CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/session_end.sh`
  Expected: project-local memory bundle refresh plus projection refresh; may repair stale terminal resume state in `.supervisor_state.json`.
- `printf '{"hook_event_name":"ConfigChange","source":"project_settings","file_path":".claude/settings.json"}
' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/config_change.sh`
  Expected: audit-only stderr guidance about regenerating generated Claude host files; exit 0.
- `printf '{"hook_event_name":"StopFailure","error":"server_error"}
' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/stop_failure.sh`
  Expected: host-private failure classification hint on stderr; exit 0.
- `./scripts/router-rs/target/debug/router-rs --claude-hook-command session-end --repo-root "$PWD" --claude-hook-max-lines 4`
  Expected: compatibility alias for `session-end`; same consolidation and projection contract.
- `printf '{"hook_event_name":"ConfigChange","source":"project_settings","file_path":".claude/settings.json"}
' | ./scripts/router-rs/target/debug/router-rs --claude-hook-audit-command config-change --repo-root "$PWD"`
  Expected: JSON on stdout plus audit-only stderr guidance; exit 0.
- In Claude Code, run `/hooks`
  Expected: the project shows only `PreToolUse`, `SessionEnd`, `ConfigChange`,
  and `StopFailure` from `.claude/settings.json`.

Shared routing policy still comes from `../../AGENT.md`.
