# Claude Hooks Directory

Claude Code project hooks live here.

Generated-first maintenance:

- Edit `scripts/materialize_cli_host_entrypoints.py` first.
- Treat `.claude/settings.json`, this README, and `.claude/hooks/*.sh` as
  materialized outputs.
- Manual Claude host guidance belongs in `.claude/agents/*.md` unless noted.

Lifecycle matrix:

| Event | Status | Script | Bridge command | Write boundary | Notes |
| --- | --- | --- | --- | --- | --- |
| `SessionStart` | disabled | `session_start.sh` | `session-start` | host projection only | Keep startup lean; do not auto-refresh projection at session start. Use manually only if needed. |
| `Stop` | disabled | `stop.sh` | `session-stop` | host projection only | Avoid per-turn background refresh; keep this script only for manual recovery. |
| `PreCompact` | disabled | `pre_compact.sh` | `pre-compact` | host projection only | Keep compaction cheap; do not auto-refresh before compaction. |
| `SubagentStop` | disabled | `subagent_stop.sh` | `subagent-stop` | host projection only | Avoid sidecar-completion refresh churn; keep this script only for manual recovery. |
| `SessionEnd` | enabled | `session_end.sh` | `session-end` | project-local memory bundle plus host projection | Consolidates shared memory bundle, refreshes projection, and may repair stale terminal resume state in `.supervisor_state.json`. |
| `ConfigChange` | enabled | `config_change.sh` | n/a | host-private audit only | Audit project-level generated-surface drift and remind maintainers to regenerate from source. Never auto-repairs or rewrites shared continuity. |
| `StopFailure` | enabled | `stop_failure.sh` | n/a | host-private alert only | Classify Claude stop failures and point maintainers back to host projection drift or hook inspection. Never rewrites shared continuity. |
| `InstructionsLoaded` | document-disable | n/a | n/a | none | Keep startup lean; the Claude projection stays on disk for `/refresh` or manual recovery instead of default auto-import. |
| `PostToolUse` | document-disable | n/a | n/a | none | High-frequency tool hook would require payload-aware hidden side effects, which violates the thin projection goal. |
| `UserPromptSubmit` | disabled | n/a | n/a | none | Avoid hidden prompt mutation; this repo prefers artifact-driven context. |
| `Notification` | disabled | n/a | n/a | none | Informational only; not part of projection or continuity refresh. |

Hook responsibilities:

- `session_end.sh`: consolidate shared memory, then refresh the Claude memory projection.
- `config_change.sh`: audit project settings changes on generated Claude surfaces without blocking or auto-repair.
- `stop_failure.sh`: emit a host-private failure hint for selected Claude stop failure classes.

Manual-only maintenance scripts:

- `session_start.sh`: one-off projection refresh when you explicitly want to rebuild recovery context.
- `stop.sh`: one-off projection refresh after a turn if you are debugging projection drift.
- `pre_compact.sh`: one-off projection refresh before compaction if you are testing that lane.
- `subagent_stop.sh`: one-off projection refresh after sidecar completion if you are debugging that lane.

Validation commands:

- `CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/session_start.sh`
  Expected: `.codex/memory/CLAUDE_MEMORY.md` is refreshed and the command exits 0.
- `CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/stop.sh`
  Expected: lightweight projection refresh only; no consolidation side effects.
- `CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/pre_compact.sh`
  Expected: projection refresh only before compaction; no consolidation side effects.
- `CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/subagent_stop.sh`
  Expected: projection refresh only after subagent completion; no supervisor-state takeover.
- `CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/session_end.sh`
  Expected: project-local memory bundle refresh plus projection refresh; may repair stale terminal resume state in `.supervisor_state.json`.
- `printf '{"hook_event_name":"ConfigChange","source":"project_settings","file_path":".claude/settings.json"}
' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/config_change.sh`
  Expected: audit-only stderr guidance about regenerating generated Claude host files; exit 0.
- `printf '{"hook_event_name":"StopFailure","error":"server_error"}
' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/stop_failure.sh`
  Expected: host-private failure classification hint on stderr; exit 0.
- `cargo run --quiet --manifest-path scripts/router-rs/Cargo.toml -- --claude-hook-command session-start --repo-root "$PWD"`
  Expected: JSON result with `canonical_command`, `contract`, and `projection`.
- `cargo run --quiet --manifest-path scripts/router-rs/Cargo.toml -- --claude-hook-command session-end --repo-root "$PWD"`
  Expected: compatibility alias for `session-end`; same consolidation and projection contract.
- `printf '{"hook_event_name":"ConfigChange","source":"project_settings","file_path":".claude/settings.json"}
' | cargo run --quiet --manifest-path scripts/router-rs/Cargo.toml -- --claude-hook-audit-command config-change --repo-root "$PWD"`
  Expected: JSON on stdout plus audit-only stderr guidance; exit 0.

Shared routing policy still comes from `../../AGENT.md`.
