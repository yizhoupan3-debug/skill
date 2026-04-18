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
| `SessionStart` | enabled | `session_start.sh` | `session-start` | host projection only | Refresh imported Claude projection at session start. |
| `Stop` | enabled | `stop.sh` | `session-stop` | host projection only | Lightweight per-turn projection refresh only. |
| `PreCompact` | enabled | `pre_compact.sh` | `pre-compact` | host projection only | Preserve minimal continuity before compaction without consolidation. |
| `SubagentStop` | enabled | `subagent_stop.sh` | `subagent-stop` | host projection only | Refresh projection after sidecar completion without taking over subagent orchestration. |
| `SessionEnd` | enabled | `session_end.sh` | `session-end` | project-local memory bundle plus host projection | Consolidates shared memory bundle, then refreshes projection. Never rewrites root continuity artifacts. |
| `ConfigChange` | enabled | `config_change.sh` | n/a | host-private audit only | Audit project-level generated-surface drift and remind maintainers to regenerate from source. Never auto-repairs or rewrites shared continuity. |
| `StopFailure` | enabled | `stop_failure.sh` | n/a | host-private alert only | Classify Claude stop failures and point maintainers back to host projection drift or hook inspection. Never rewrites shared continuity. |
| `InstructionsLoaded` | document-disable | n/a | n/a | none | Redundant with imported `../.codex/memory/CLAUDE_MEMORY.md` and `SessionStart` refresh; no extra repo-specific action is needed. |
| `PostToolUse` | document-disable | n/a | n/a | none | High-frequency tool hook would require payload-aware hidden side effects, which violates the thin projection goal. |
| `UserPromptSubmit` | disabled | n/a | n/a | none | Avoid hidden prompt mutation; this repo prefers artifact-driven context. |
| `Notification` | disabled | n/a | n/a | none | Informational only; not part of projection or continuity refresh. |

Hook responsibilities:

- `session_start.sh`: refresh the Claude memory projection.
- `stop.sh`: refresh the Claude memory projection after a completed turn.
- `pre_compact.sh`: refresh the Claude memory projection before compaction.
- `subagent_stop.sh`: refresh the Claude memory projection after subagent completion.
- `session_end.sh`: consolidate shared memory, then refresh the Claude memory projection.
- `config_change.sh`: audit project settings changes on generated Claude surfaces without blocking or auto-repair.
- `stop_failure.sh`: emit a host-private failure hint for selected Claude stop failure classes.

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
  Expected: project-local memory bundle refresh plus projection refresh; no root continuity rewrite.
- `printf '{"hook_event_name":"ConfigChange","scope":"project_settings","changed_path":".claude/settings.json"}
' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/config_change.sh`
  Expected: audit-only stderr guidance about regenerating generated Claude host files; exit 0.
- `printf '{"hook_event_name":"StopFailure","failure_type":"server_error"}
' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/stop_failure.sh`
  Expected: host-private failure classification hint on stderr; exit 0.
- `python3 scripts/claude_memory_bridge.py session-start --repo-root "$PWD" --json`
  Expected: JSON result with `canonical_command`, `contract`, and `projection`.

Shared routing policy still comes from `../../AGENT.md`.
