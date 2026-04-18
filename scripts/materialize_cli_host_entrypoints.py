#!/usr/bin/env python3
"""Materialize shared Codex/Claude/Gemini entrypoint files for this repo."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any


SHARED_AGENT_POLICY = """# Shared Agent Policy

This repository is designed to be entered from `AGENTS.md` (Codex), `CLAUDE.md`
(Claude Code), or `GEMINI.md` (Gemini CLI). These files must project one shared
framework policy instead of forking per-host routing or memory rules.

## Default Behavior

- Reply in Chinese unless the user asks for another language.
- Keep answers direct and concise.
- Execute safe read/search/test/build commands directly when the runtime allows.
- Ask before destructive actions, external publishing, or account-impacting work.

## Turn-Start Routing

1. Extract `object / action / constraints / deliverable`.
2. Check gates before owners.
3. Use the narrowest matching skill.
4. Read the chosen `SKILL.md` before analysis, search, coding, or edits.
5. If no skill matches, consult `skills/SKILL_ROUTING_RUNTIME.json`, then
   `skills/SKILL_ROUTING_INDEX.md`.
6. Keep exactly one primary owner and at most one overlay.
7. For high-load, cross-file, or long-running tasks, invoke
   `execution-controller-coding` and maintain `.supervisor_state.json`.
8. For complex tasks, check `subagent-delegation` before deciding whether to
   split bounded sidecars.

## Shared Runtime Contract

- The shared runtime truth lives in `skills/`, task artifacts, and
  `.supervisor_state.json`.
- Host-specific entry files are thin projections only. They must not fork the
  routing truth, memory schema, or artifact contract.
- Complex tasks should externalize state into:
  `SESSION_SUMMARY.md`, `NEXT_ACTIONS.json`, `EVIDENCE_INDEX.json`,
  `TRACE_METADATA.json`, and `.supervisor_state.json`.
- Task-scoped continuity lives under `artifacts/current/<task_id>/`.
  Root-level continuity files and `artifacts/current/` are current-task mirrors
  or pointer surfaces only; they must not act as cross-task global truth.
  `artifacts/current/` may contain only `active_task.json`, the four mirror
  files, and task-scoped continuity directories. Bootstrap payloads belong in
  `artifacts/bootstrap/`, memory-automation diagnostics belong in
  `artifacts/ops/memory_automation/`, evidence belongs in `artifacts/evidence/`,
  and scratch or demo outputs belong in `artifacts/scratch/`.
- Shared continuity artifacts are a **single-writer surface**. Only the active
  supervisor / integrator may write `SESSION_SUMMARY.md`, `NEXT_ACTIONS.json`,
  `EVIDENCE_INDEX.json`, `TRACE_METADATA.json`, and `.supervisor_state.json`.
  Parallel lanes must emit lane-local summaries or delta artifacts and leave
  global continuity flushes to the integration step.
- Claude host hooks may refresh imported host projections, and `SessionEnd`
  may consolidate the project-local memory bundle, but they must not rewrite
  root continuity artifacts or take over supervisor integration.

## Memory Contract

- Long-term framework memory remains project-local at `./.codex/memory/`
  unless tooling explicitly switches roots.
- In this repository, `./.codex/memory/` is the logical framework path and
  currently resolves via symlink to `./memory/`; treat that as one shared root,
  not two independent memory trees.
- Default recall reads only the stable layer: `MEMORY.md`, `preferences.md`,
  `decisions.md`, `lessons.md`, `runbooks.md`, plus a freshness-gated active
  task summary only when the query clearly targets the current task.
- Historical/debug snapshots such as old session notes, legacy SQLite rows, and
  previous automation snapshots must live under `memory/archive/` or
  `artifacts/ops/memory_automation/`; they are not part of the normal prompt
  path.
- This path is shared framework state, not a Codex-only policy claim.
- Host entry files may reference framework memory, but must not redefine its
  schema or ownership.

## Workspace Binding

- When the user says `绑定xx目录` and the path is relative, resolve it under
  `/Users/joe/Documents`.
- Example: `绑定research/made` means `/Users/joe/Documents/research/made`.
- If the relative path does not exist there, ask for clarification instead of
  guessing across other roots.

## Runtime Sources Of Truth

- `skills/SKILL_ROUTING_RUNTIME.json`: machine-readable routing truth
- `skills/SKILL_ROUTING_INDEX.md`: human quick reference
- `skills/SKILL_ROUTING_LAYERS.md`: owner and reroute map
- `skills/SKILL_SOURCE_MANIFEST.json`: source precedence
- `skills/SKILL_SHADOW_MAP.json`: shadow audit
- `skills/SKILL_LOADOUTS.json`: loadout definitions
- `skills/SKILL_APPROVAL_POLICY.json`: approval policy registry

## Host Entry Files

- Codex: `AGENTS.md`
- Claude Code: `CLAUDE.md`, `.claude/CLAUDE.md`, `.claude/settings.json`
- Gemini CLI: `GEMINI.md`, `.gemini/settings.json`

These entry files must stay thin and point back to this shared policy.
"""

ROOT_AGENTS_PROXY = """# Codex Entry Proxy

This file exists because Codex discovers `AGENTS.md`.

- Shared framework policy source of truth: [AGENT.md](AGENT.md)

Do not fork routing, memory, or artifact policy in this file.
"""

ROOT_CLAUDE_PROXY = """# Claude Code Entry Proxy

This file exists because Claude Code discovers `CLAUDE.md`.

@.claude/CLAUDE.md
"""

CLAUDE_LOCAL_PROXY = """# Claude Local Overlay

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
"""

ROOT_GEMINI_PROXY = """# Gemini CLI Entry Proxy

This file exists because Gemini CLI discovers `GEMINI.md`.

- Shared framework policy source of truth: [AGENT.md](AGENT.md)
- Gemini local settings root: [.gemini/settings.json](.gemini/settings.json)

Gemini-specific config belongs in `.gemini/`, but the shared routing, memory,
and artifact rules still come from `AGENT.md`.
"""

CONFIG_CODEX_PROXY = """# Codex Entry Proxy

This file is a thin proxy only.

- Repository policy source of truth: `/Users/joe/Documents/skill/AGENT.md`
- Codex project entrypoint: `AGENTS.md`

Do not duplicate or diverge from the shared policy here.
"""

CONFIG_CLAUDE_PROXY = """# Claude Config Proxy

This file is a thin proxy only.

- Repository policy source of truth: `/Users/joe/Documents/skill/AGENT.md`
- Repository Claude entrypoint: `/Users/joe/Documents/skill/CLAUDE.md`

Do not duplicate or diverge from the shared policy here.
"""

CONFIG_GEMINI_PROXY = """# Gemini Config Proxy

This file is a thin proxy only.

- Repository policy source of truth: `/Users/joe/Documents/skill/AGENT.md`
- Repository Gemini entrypoint: `/Users/joe/Documents/skill/GEMINI.md`

Do not duplicate or diverge from the shared policy here.
"""

CLAUDE_AGENTS_README = """# Claude Agents Directory

These project-scoped Claude Code subagents help Claude use this repository's
shared routing, execution, and host-projection system without duplicating it.
The policy source of truth is still `../../AGENT.md`.

Available agents:

- `framework-router.md`: read-only router for choosing the right repo skill,
  gate, and next files to inspect
- `skill-maintainer.md`: bounded editor for `skills/**` and nearby framework
  surfaces when the task already has a clear write scope
- `state-artifact-keeper.md`: bounded maintainer for `.supervisor_state.json`
  and the shared task-artifact contract
- `claude-host-maintainer.md`: bounded maintainer for `.claude/**`,
  `CLAUDE.md`, and Claude-host compatibility docs without forking shared policy

Design rules for these subagents:

- They must read `../../AGENT.md` first and treat it as authoritative.
- They should stay thin: route into existing repo skills and artifacts instead
  of restating the framework.
- They should keep outputs concise and integration-friendly for the parent
  agent.
- They should not widen scope beyond the surfaces named in their prompt.
"""

CLAUDE_HOOKS_README = """# Claude Hooks Directory

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
- `printf '{"hook_event_name":"ConfigChange","scope":"project_settings","changed_path":".claude/settings.json"}\n' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/config_change.sh`
  Expected: audit-only stderr guidance about regenerating generated Claude host files; exit 0.
- `printf '{"hook_event_name":"StopFailure","failure_type":"server_error"}\n' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/stop_failure.sh`
  Expected: host-private failure classification hint on stderr; exit 0.
- `python3 scripts/claude_memory_bridge.py session-start --repo-root "$PWD" --json`
  Expected: JSON result with `canonical_command`, `contract`, and `projection`.

Shared routing policy still comes from `../../AGENT.md`.
"""

CLAUDE_SESSION_START_HOOK = """#!/bin/sh
set -eu

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"

python3 "$PROJECT_DIR/scripts/claude_memory_bridge.py" session-start \
  --repo-root "$PROJECT_DIR" >/dev/null
"""

CLAUDE_STOP_HOOK = """#!/bin/sh
set -eu

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"

python3 "$PROJECT_DIR/scripts/claude_memory_bridge.py" session-stop \
  --repo-root "$PROJECT_DIR" >/dev/null
"""

CLAUDE_PRE_COMPACT_HOOK = """#!/bin/sh
set -eu

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"

python3 "$PROJECT_DIR/scripts/claude_memory_bridge.py" pre-compact \
  --repo-root "$PROJECT_DIR" >/dev/null
"""

CLAUDE_SUBAGENT_STOP_HOOK = """#!/bin/sh
set -eu

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"

python3 "$PROJECT_DIR/scripts/claude_memory_bridge.py" subagent-stop \
  --repo-root "$PROJECT_DIR" >/dev/null
"""

CLAUDE_SESSION_END_HOOK = """#!/bin/sh
set -eu

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"

python3 "$PROJECT_DIR/scripts/claude_memory_bridge.py" session-end \
  --repo-root "$PROJECT_DIR" >/dev/null
"""

CLAUDE_CONFIG_CHANGE_HOOK = """#!/bin/sh
set -eu

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"

python3 "$PROJECT_DIR/scripts/claude_hook_audit.py" config-change \
  --repo-root "$PROJECT_DIR"
"""

CLAUDE_STOP_FAILURE_HOOK = """#!/bin/sh
set -eu

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"

python3 "$PROJECT_DIR/scripts/claude_hook_audit.py" stop-failure \
  --repo-root "$PROJECT_DIR"
"""

CLAUDE_PROJECT_SETTINGS = {
    "$schema": "https://json.schemastore.org/claude-code-settings.json",
    "permissions": {
        "allow": [
            "Bash(ls)",
            "Bash(pwd)",
            "Bash(git status)",
            "Bash(git diff)",
            "Bash(python3 scripts/check_skills.py --verify-sync)",
            "Bash(python3 scripts/check_skills.py --verify-codex-link)",
            "Bash(python3 scripts/claude_memory_bridge.py *)",
            "Bash(python3 scripts/claude_statusline.py --repo-root *)",
            "Bash(cmp -s TRACE_METADATA.json artifacts/current/TRACE_METADATA.json)",
            "Bash(./tools/browser-mcp/scripts/start_browser_mcp.sh *)",
            "Bash(bash ./tools/browser-mcp/scripts/start_browser_mcp.sh *)",
        ]
    },
    "allowedMcpServers": [{"serverName": "browser-mcp"}],
    "statusLine": {
        "type": "command",
        "command": "python3 \"$CLAUDE_PROJECT_DIR\"/scripts/claude_statusline.py --repo-root \"$CLAUDE_PROJECT_DIR\"",
        "padding": 1,
        "refreshInterval": 30,
    },
    "hooks": {
        "SessionStart": [
            {
                "matcher": "*",
                "hooks": [
                    {
                        "type": "command",
                        "command": "sh \"$CLAUDE_PROJECT_DIR\"/.claude/hooks/session_start.sh",
                    }
                ],
            }
        ],
        "Stop": [
            {
                "hooks": [
                    {
                        "type": "command",
                        "command": "sh \"$CLAUDE_PROJECT_DIR\"/.claude/hooks/stop.sh",
                    }
                ],
            }
        ],
        "PreCompact": [
            {
                "matcher": "*",
                "hooks": [
                    {
                        "type": "command",
                        "command": "sh \"$CLAUDE_PROJECT_DIR\"/.claude/hooks/pre_compact.sh",
                    }
                ],
            }
        ],
        "SubagentStop": [
            {
                "hooks": [
                    {
                        "type": "command",
                        "command": "sh \"$CLAUDE_PROJECT_DIR\"/.claude/hooks/subagent_stop.sh",
                    }
                ],
            }
        ],
        "SessionEnd": [
            {
                "hooks": [
                    {
                        "type": "command",
                        "command": "sh \"$CLAUDE_PROJECT_DIR\"/.claude/hooks/session_end.sh",
                    }
                ],
            }
        ],
        "ConfigChange": [
            {
                "matcher": "project_settings",
                "hooks": [
                    {
                        "type": "command",
                        "command": "sh \"$CLAUDE_PROJECT_DIR\"/.claude/hooks/config_change.sh",
                    }
                ],
            }
        ],
        "StopFailure": [
            {
                "matcher": "invalid_request|server_error|max_output_tokens|unknown",
                "hooks": [
                    {
                        "type": "command",
                        "command": "sh \"$CLAUDE_PROJECT_DIR\"/.claude/hooks/stop_failure.sh",
                    }
                ],
            }
        ],
    },
}


def _write_text(path: Path, content: str) -> bool:
    existing = path.read_text(encoding="utf-8") if path.is_file() else None
    if existing == content:
        return False
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    return True


def _write_json(path: Path, payload: dict[str, Any]) -> bool:
    content = json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    return _write_text(path, content)


def sync_repo_host_entrypoints(
    repo_root: Path | None = None,
    *,
    apply: bool,
) -> dict[str, list[str]]:
    """Check or write the shared and host-specific entrypoint files for this repository."""

    root = (repo_root or Path(__file__).resolve().parents[1]).resolve()
    written: list[str] = []
    unchanged: list[str] = []
    created_dirs: list[str] = []

    text_files: dict[Path, str] = {
        root / "AGENT.md": SHARED_AGENT_POLICY,
        root / "AGENTS.md": ROOT_AGENTS_PROXY,
        root / "CLAUDE.md": ROOT_CLAUDE_PROXY,
        root / "GEMINI.md": ROOT_GEMINI_PROXY,
        root / ".claude" / "CLAUDE.md": CLAUDE_LOCAL_PROXY,
        root / ".claude" / "agents" / "README.md": CLAUDE_AGENTS_README,
        root / ".claude" / "hooks" / "README.md": CLAUDE_HOOKS_README,
        root / ".claude" / "hooks" / "session_start.sh": CLAUDE_SESSION_START_HOOK,
        root / ".claude" / "hooks" / "stop.sh": CLAUDE_STOP_HOOK,
        root / ".claude" / "hooks" / "pre_compact.sh": CLAUDE_PRE_COMPACT_HOOK,
        root / ".claude" / "hooks" / "subagent_stop.sh": CLAUDE_SUBAGENT_STOP_HOOK,
        root / ".claude" / "hooks" / "session_end.sh": CLAUDE_SESSION_END_HOOK,
        root / ".claude" / "hooks" / "config_change.sh": CLAUDE_CONFIG_CHANGE_HOOK,
        root / ".claude" / "hooks" / "stop_failure.sh": CLAUDE_STOP_FAILURE_HOOK,
        root / "configs" / "codex" / "AGENTS.md": CONFIG_CODEX_PROXY,
        root / "configs" / "claude" / "CLAUDE.md": CONFIG_CLAUDE_PROXY,
        root / "configs" / "gemini" / "GEMINI.md": CONFIG_GEMINI_PROXY,
    }
    json_files: dict[Path, dict[str, Any]] = {
        root / ".claude" / "settings.json": CLAUDE_PROJECT_SETTINGS,
        root / ".gemini" / "settings.json": {},
    }
    retired_paths = [
        root / ".codex" / "model_instructions.md",
        root / ".mcp.json",
    ]

    for directory in (
        root / ".claude",
        root / ".claude" / "agents",
        root / ".claude" / "hooks",
        root / ".gemini",
        root / "configs" / "claude",
        root / "configs" / "gemini",
        root / "configs" / "codex",
        root / ".codex",
    ):
        if not directory.exists():
            directory.mkdir(parents=True, exist_ok=True)
            created_dirs.append(str(directory.relative_to(root)))

    for path, content in text_files.items():
        relative = str(path.relative_to(root))
        existing = path.read_text(encoding="utf-8") if path.is_file() else None
        is_changed = existing != content
        if is_changed and apply:
            _write_text(path, content)
        (written if is_changed else unchanged).append(relative)

    for path, payload in json_files.items():
        relative = str(path.relative_to(root))
        content = json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
        existing = path.read_text(encoding="utf-8") if path.is_file() else None
        is_changed = existing != content
        if is_changed and apply:
            _write_json(path, payload)
        (written if is_changed else unchanged).append(relative)

    for path in retired_paths:
        relative = str(path.relative_to(root))
        exists = path.exists()
        if exists and apply:
            path.unlink()
        (written if exists else unchanged).append(relative)

    return {
        "written": sorted(written),
        "unchanged": sorted(unchanged),
        "created_dirs": sorted(created_dirs),
    }


def materialize_repo_host_entrypoints(repo_root: Path | None = None) -> dict[str, list[str]]:
    """Write the shared and host-specific entrypoint files for this repository."""

    return sync_repo_host_entrypoints(repo_root, apply=True)


def main() -> int:
    result = materialize_repo_host_entrypoints()
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
