#!/usr/bin/env python3
"""Materialize shared Codex/Claude/Gemini entrypoint files for this repo."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.host_integration_rs import run_host_integration_rs


SHARED_AGENT_POLICY = """# Shared Agent Policy

This repository is designed to be entered from `AGENTS.md` (Codex), `CLAUDE.md`
(Claude Code), or `GEMINI.md` (Gemini CLI). These files must project one shared
framework policy instead of forking per-host routing or memory rules.

## Default Behavior

- Reply in Chinese unless the user asks for another language.
- Keep answers direct, concise, and easy to scan.
- Execute safe read/search/test/build commands directly when the runtime allows.
- Default to a get-shit-done posture for clear local tasks: auto-continue safe,
  reversible work, keep ownership local, and verify before handoff.
- Do not silently choose an ambiguous interpretation when it would materially
  change the code, output, or risk surface; surface the assumption or ask.
- Prefer the smallest solution that fully solves the stated problem; do not add
  speculative abstractions, options, or future-proofing that was not requested.
- Keep edits surgical: touch only what the task requires, match local style,
  and clean up only the mess created by the current change.
- For non-trivial work, define success in a verifiable way before
  implementation and use that definition to drive execution.
- Ask before destructive actions, external publishing, or account-impacting work.

## Communication Style

- Lead with the answer or result, not status reports, greetings, or self-talk.
- Use plain Chinese and everyday words by default.
- Avoid internal runtime, routing, framework, or tool jargon unless the user
  explicitly asks for it.
- If a technical term is necessary, explain it in simple words the first time.
- Keep the default reply to one short paragraph; use lists only when the content
  is genuinely list-shaped.
- Keep the tone calm, friendly, and practical.

## Output Compaction

- For high-output local commands where exact raw output is not required, follow
  `RTK.md` and prefer the corresponding `rtk ...` wrapper.
- Treat `RTK.md` as repo-local operator guidance only; shared routing and
  policy truth still lives in this file plus the generated routing artifacts.

## Task Closeout

- Keep end-of-task user-facing closeouts in plain Chinese by default.
- Default the closeout to one short paragraph that says what now works or what
  effect was achieved, and what still needs to happen next.
- If no further work is needed, say that directly instead of inventing follow-up
  tasks.
- Do not default to changed-file inventories, changelog-style recaps, or
  step-by-step implementation retellings in the final user-facing closeout.
- Machine continuity artifacts such as `NEXT_ACTIONS.json`,
  `.supervisor_state.json`, and verification or blocker fields remain the
  recovery truth; do not mirror them verbatim into the user-facing closeout
  unless they materially affect the user's next decision.

## Turn-Start Routing

1. Extract `object / action / constraints / deliverable`.
2. Surface any ambiguity that would materially change the route or result.
3. Check gates before owners.
4. Use the narrowest matching skill and read its `SKILL.md` before acting.
5. For non-trivial execution, state the minimum success criteria and intended
   verification path before coding.
6. If no skill matches, consult `skills/SKILL_ROUTING_RUNTIME.json` first, then
   `skills/SKILL_ROUTING_INDEX.md`.
7. Keep one primary owner and at most one overlay.
8. Use `execution-controller-coding` for high-load or long-running work, and
   check `subagent-delegation` before splitting bounded sidecars.
9. Treat explicit `gsd` / `get shit done` / “推进到底” requests as a posture
   boost for `execution-controller-coding` plus `anti-laziness`, not as an
   external workflow.

## Shared Runtime Contract

- Runtime truth lives in `skills/`, task artifacts, and
  `.supervisor_state.json`.
- Host-specific entry files are thin projections only; they must not fork
  routing, memory schema, or artifact rules.
- Complex tasks externalize state into `SESSION_SUMMARY.md`,
  `NEXT_ACTIONS.json`, `EVIDENCE_INDEX.json`, `TRACE_METADATA.json`, and
  `.supervisor_state.json`.
- `artifacts/current/<task_id>/` is task-local continuity. Keep bootstrap,
  ops, evidence, and scratch outputs in their own roots.
- Shared continuity files are a single-writer surface: only the active
  integrator writes them; parallel lanes emit local deltas.

## Memory Contract

- Framework memory lives at `./.codex/memory/`; in this repo it resolves to
  `./memory/`, and both paths are one shared root.
- Default recall reads only the stable layer: `MEMORY.md`, `preferences.md`,
  `decisions.md`, `lessons.md`, `runbooks.md`, plus an active task summary only
  when clearly needed.
- Historical or debug snapshots belong under `memory/archive/` or
  `artifacts/ops/memory_automation/`, not the normal prompt path.
- Host entry files may reference framework memory, but must not redefine its
  schema or ownership.

## Workspace Binding

- When the user says `绑定xx目录` and the path is relative, resolve it under
  `/Users/joe/Documents`.
- Example: `绑定research/made` means `/Users/joe/Documents/research/made`.
- If the relative path does not exist there, ask for clarification instead of
  guessing across other roots.

## Runtime Sources Of Truth

- Default routing truth:
  `skills/SKILL_ROUTING_RUNTIME.json` and `skills/SKILL_ROUTING_INDEX.md`.
- Open the extended generated references only when you need ambiguity or audit
  detail: `skills/SKILL_ROUTING_LAYERS.md`,
  `skills/SKILL_SOURCE_MANIFEST.json`, `skills/SKILL_SHADOW_MAP.json`,
  `skills/SKILL_LOADOUTS.json`,
  `configs/framework/FRAMEWORK_SURFACE_POLICY.json`, and
  `skills/SKILL_APPROVAL_POLICY.json`.

## Host Entry Files

- Default host entrypoints: `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`.
- Host-private overlays: `.claude/settings.json`, `.gemini/settings.json`.
- These files stay thin and point back to this shared policy.
"""

ROOT_AGENTS_PROXY = """# Codex Entry Proxy

This file exists because Codex discovers `AGENTS.md`.

- Shared framework policy source of truth: [AGENT.md](AGENT.md)

Do not fork routing, memory, or artifact policy in this file.
"""

ROOT_CLAUDE_PROXY = """# Claude Code Entry Proxy

This file exists because Claude Code discovers `CLAUDE.md`.

@AGENT.md
@.codex/memory/CLAUDE_MEMORY.md

## Claude Project Entry

Use `.claude/` only for Claude host-private files such as:

- `.claude/settings.json`
- `.claude/agents/`
- `.claude/commands/`
- `.claude/hooks/`

Claude-specific hooks may refresh the imported memory projection, but must not
fork the shared framework policy or memory ownership.

Generated-first maintenance rule:

- Edit `scripts/materialize_cli_host_entrypoints.py` first for
  `.claude/settings.json`, `.claude/commands/*.md`, `.claude/hooks/README.md`,
  and `.claude/hooks/*.sh`.
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

CLAUDE_REFRESH_COMMAND = """---
description: Build the next-turn execution prompt, copy it to the clipboard, and reply with one fixed sentence.
allowed-tools: Bash(python3 scripts/claude_memory_bridge.py *)
---

If `scripts/claude_memory_bridge.py` exists in the current repository, run:

`python3 scripts/claude_memory_bridge.py refresh-workflow --json`

If the bridge copied the prompt successfully, reply with exactly:
`下一轮执行 prompt 已准备好，并且已经复制到剪贴板。`

If the bridge did not copy it successfully, copy `workflow_prompt` to the macOS clipboard yourself, then reply with exactly:
`下一轮执行 prompt 已准备好，并且已经复制到剪贴板。`
"""

CLAUDE_BACKGROUND_BATCH_COMMAND = """---
description: Run the repo's durable background parallel-batch CLI and answer from its JSON result.
allowed-tools: Bash(python3 scripts/runtime_background_cli.py *)
---

Use `python3 scripts/runtime_background_cli.py` as the only host-level entrypoint
for this repository's durable background batch control.

Supported actions:

- Enqueue and wait:
  `python3 scripts/runtime_background_cli.py enqueue-batch --input-file <path>`
  or
  `python3 scripts/runtime_background_cli.py enqueue-batch --input-json '<json>'`
- Read one group:
  `python3 scripts/runtime_background_cli.py group-summary --parallel-group-id <id>`
- List all groups:
  `python3 scripts/runtime_background_cli.py list-groups`

Always relay the command's JSON result and then summarize it briefly in plain Chinese.
Do not invent batch state that the command did not return.
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
- `python3 scripts/session_lifecycle_hook.py session-start --repo-root "$PWD" --json`
  Expected: JSON result with `canonical_command`, `contract`, and `projection`.
- `python3 scripts/session_lifecycle_hook.py end-session --repo-root "$PWD" --json`
  Expected: compatibility alias for `session-end`; same consolidation and projection contract.

Shared routing policy still comes from `../../AGENT.md`.
"""

CLAUDE_SESSION_START_HOOK = """#!/bin/sh
set -eu

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"

python3 "$PROJECT_DIR/scripts/session_lifecycle_hook.py" session-start \
  --repo-root "$PROJECT_DIR" >/dev/null
"""

CLAUDE_STOP_HOOK = """#!/bin/sh
set -eu

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"

python3 "$PROJECT_DIR/scripts/session_lifecycle_hook.py" session-stop \
  --repo-root "$PROJECT_DIR" >/dev/null
"""

CLAUDE_PRE_COMPACT_HOOK = """#!/bin/sh
set -eu

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"

python3 "$PROJECT_DIR/scripts/session_lifecycle_hook.py" pre-compact \
  --repo-root "$PROJECT_DIR" >/dev/null
"""

CLAUDE_SUBAGENT_STOP_HOOK = """#!/bin/sh
set -eu

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"

python3 "$PROJECT_DIR/scripts/session_lifecycle_hook.py" subagent-stop \
  --repo-root "$PROJECT_DIR" >/dev/null
"""

CLAUDE_SESSION_END_HOOK = """#!/bin/sh
set -eu

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"

python3 "$PROJECT_DIR/scripts/session_lifecycle_hook.py" session-end \
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
            "Bash(python3 scripts/session_lifecycle_hook.py *)",
            "Bash(python3 scripts/claude_memory_bridge.py *)",
            "Bash(python3 scripts/runtime_background_cli.py *)",
            "Bash(python3 scripts/claude_statusline.py --repo-root *)",
            "Bash(cmp -s TRACE_METADATA.json artifacts/current/TRACE_METADATA.json)",
            "Bash(./tools/browser-mcp/scripts/start_browser_mcp.sh *)",
            "Bash(bash ./tools/browser-mcp/scripts/start_browser_mcp.sh *)",
        ]
    },
    "allowedMcpServers": [{"serverName": "browser-mcp"}],
    "statusLine": {
        "type": "command",
        "command": 'python3 "$CLAUDE_PROJECT_DIR"/scripts/claude_statusline.py --repo-root "$CLAUDE_PROJECT_DIR"',
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

HOST_ENTRYPOINT_SYNC_MANIFEST_PATH = ".codex/host_entrypoints_sync_manifest.json"

HOST_ENTRYPOINT_TEXT_FILES = {
    "AGENT.md": SHARED_AGENT_POLICY,
    "AGENTS.md": ROOT_AGENTS_PROXY,
    "CLAUDE.md": ROOT_CLAUDE_PROXY,
    "GEMINI.md": ROOT_GEMINI_PROXY,
    ".claude/agents/README.md": CLAUDE_AGENTS_README,
    ".claude/commands/refresh.md": CLAUDE_REFRESH_COMMAND,
    ".claude/commands/background_batch.md": CLAUDE_BACKGROUND_BATCH_COMMAND,
    ".claude/hooks/README.md": CLAUDE_HOOKS_README,
    ".claude/hooks/session_start.sh": CLAUDE_SESSION_START_HOOK,
    ".claude/hooks/stop.sh": CLAUDE_STOP_HOOK,
    ".claude/hooks/pre_compact.sh": CLAUDE_PRE_COMPACT_HOOK,
    ".claude/hooks/subagent_stop.sh": CLAUDE_SUBAGENT_STOP_HOOK,
    ".claude/hooks/session_end.sh": CLAUDE_SESSION_END_HOOK,
    ".claude/hooks/config_change.sh": CLAUDE_CONFIG_CHANGE_HOOK,
    ".claude/hooks/stop_failure.sh": CLAUDE_STOP_FAILURE_HOOK,
}

HOST_ENTRYPOINT_JSON_FILES = {
    ".claude/settings.json": CLAUDE_PROJECT_SETTINGS,
    ".gemini/settings.json": {},
}

FULL_SYNC_MANAGED_DIRECTORIES = (
    ".claude",
    ".claude/agents",
    ".claude/commands",
    ".claude/hooks",
    ".gemini",
    ".codex",
)

PARTIAL_SYNC_TEXT_FILES = (
    ".claude/commands/refresh.md",
    ".claude/commands/background_batch.md",
)

PARTIAL_SYNC_MANAGED_DIRECTORIES = (
    ".claude",
    ".claude/commands",
)

RETIRED_HOST_ENTRYPOINT_PATHS = (
    ".claude/CLAUDE.md",
    ".codex/model_instructions.md",
    ".mcp.json",
    "configs/codex/AGENTS.md",
    "configs/claude/CLAUDE.md",
    "configs/gemini/GEMINI.md",
)

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


def _build_host_entrypoint_sync_manifest() -> dict[str, Any]:
    return {
        "schema_version": "host-entrypoints-sync-manifest-v1",
        "full_sync": {
            "text_files": sorted(HOST_ENTRYPOINT_TEXT_FILES),
            "json_files": sorted(HOST_ENTRYPOINT_JSON_FILES),
            "managed_directories": list(FULL_SYNC_MANAGED_DIRECTORIES),
            "retired_paths": list(RETIRED_HOST_ENTRYPOINT_PATHS),
        },
        "partial_sync": {
            "text_files": list(PARTIAL_SYNC_TEXT_FILES),
            "json_files": [],
            "managed_directories": list(PARTIAL_SYNC_MANAGED_DIRECTORIES),
            "retired_paths": [],
        },
    }


def _write_host_entrypoint_template(template_root: Path) -> None:
    for relative_path, content in HOST_ENTRYPOINT_TEXT_FILES.items():
        _write_text(template_root / relative_path, content)
    for relative_path, payload in HOST_ENTRYPOINT_JSON_FILES.items():
        _write_json(template_root / relative_path, payload)
    _write_json(
        template_root / HOST_ENTRYPOINT_SYNC_MANIFEST_PATH,
        _build_host_entrypoint_sync_manifest(),
    )


def sync_repo_host_entrypoints(
    repo_root: Path | None = None,
    *,
    apply: bool,
) -> dict[str, list[str]]:
    """Check or write the shared and host-specific entrypoint files for this repository."""

    root = (repo_root or Path(__file__).resolve().parents[1]).resolve()
    with TemporaryDirectory() as temp_dir:
        template_root = Path(temp_dir)
        _write_host_entrypoint_template(template_root)
        return run_host_integration_rs(
            "sync-host-entrypoints",
            "--template-root",
            str(template_root),
            "--repo-root",
            str(root),
            "--apply" if apply else "--check",
        )


def materialize_repo_host_entrypoints(repo_root: Path | None = None) -> dict[str, list[str]]:
    """Write the shared and host-specific entrypoint files for this repository."""

    return sync_repo_host_entrypoints(repo_root, apply=True)


def main() -> int:
    result = materialize_repo_host_entrypoints()
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
