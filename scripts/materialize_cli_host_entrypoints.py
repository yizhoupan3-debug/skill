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
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "codex_agno_runtime" / "src"))

from codex_agno_runtime.runtime_registry import framework_native_aliases, shared_project_mcp_servers
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

Keep startup lean. Do not add `@...` imports here.

Treat `.claude/**` as host-shell glue, not repository truth.
The recovery projection lives at `.codex/memory/CLAUDE_MEMORY.md` for `/refresh`
or manual resume, not default startup injection.

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
description: Generate and copy the next-turn execution prompt with the Rust refresh command.
allowed-tools: Bash(cargo run --quiet --manifest-path */scripts/router-rs/Cargo.toml -- *)
---

Run:

`cargo run --quiet --manifest-path scripts/router-rs/Cargo.toml -- --framework-refresh-json`

Then reply with exactly:
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

def _framework_alias_payload(alias_name: str) -> dict[str, Any]:
    payload = framework_native_aliases().get(alias_name)
    if not isinstance(payload, dict):
        raise ValueError(f"framework_native_aliases missing alias payload for {alias_name!r}")
    return payload


def _framework_alias_claude_entrypoint(alias_name: str) -> str:
    payload = _framework_alias_payload(alias_name)
    host_entrypoints = payload.get("host_entrypoints")
    if not isinstance(host_entrypoints, dict):
        raise ValueError(f"framework_native_aliases[{alias_name!r}] missing host_entrypoints")
    entrypoint = host_entrypoints.get("claude-code")
    if not isinstance(entrypoint, str) or not entrypoint:
        raise ValueError(
            f"framework_native_aliases[{alias_name!r}] missing claude-code host_entrypoint"
        )
    return entrypoint


def _framework_alias_implementation_bar(alias_name: str) -> list[str]:
    payload = _framework_alias_payload(alias_name)
    raw = payload.get("implementation_bar")
    if not isinstance(raw, list):
        return []
    return [str(item) for item in raw if isinstance(item, str) and item]


def _framework_alias_upstream_source(alias_name: str) -> dict[str, str]:
    payload = _framework_alias_payload(alias_name)
    raw = payload.get("upstream_source")
    if not isinstance(raw, dict):
        return {}
    return {str(key): str(value) for key, value in raw.items() if isinstance(value, str) and value}


def _framework_alias_local_adaptations(alias_name: str) -> list[str]:
    payload = _framework_alias_payload(alias_name)
    raw = payload.get("local_adaptations")
    if not isinstance(raw, list):
        return []
    return [str(item) for item in raw if isinstance(item, str) and item]


def _framework_alias_workflow_items(alias_name: str, field_name: str) -> list[str]:
    payload = _framework_alias_payload(alias_name)
    workflow = payload.get("official_workflow")
    if not isinstance(workflow, dict):
        return []
    raw = workflow.get(field_name)
    if not isinstance(raw, list):
        return []
    return [str(item) for item in raw if isinstance(item, str) and item]


def _render_alias_upgrade_requirements(alias_name: str) -> str:
    requirements = _framework_alias_implementation_bar(alias_name)
    if not requirements:
        return ""
    rendered = "\n".join(f"   - `{item}`" for item in requirements)
    return (
        "This alias inherits the original OMC core capability, but this repo must exceed OMC by enforcing:\n\n"
        f"{rendered}\n"
    )


def _render_alias_upstream_baseline(alias_name: str) -> str:
    upstream = _framework_alias_upstream_source(alias_name)
    if not upstream:
        return ""
    tag = upstream.get("tag", "unknown")
    commit = upstream.get("commit", "unknown")
    skill_path = upstream.get("official_skill_path", "unknown")
    return (
        "Official upstream baseline:\n\n"
        f"- repo: `{upstream.get('repo', 'unknown')}`\n"
        f"- tag: `{tag}`\n"
        f"- commit: `{commit}`\n"
        f"- skill: `{skill_path}`\n"
    )


def _render_alias_list_section(title: str, items: list[str]) -> str:
    if not items:
        return ""
    rendered = "\n".join(f"- `{item}`" for item in items)
    return f"{title}\n\n{rendered}\n"


def _build_claude_autopilot_command() -> str:
    payload = _framework_alias_payload("autopilot")
    entrypoint = _framework_alias_claude_entrypoint("autopilot")
    stronger_requirements = _render_alias_upgrade_requirements("autopilot")
    upstream_baseline = _render_alias_upstream_baseline("autopilot")
    official_phases = _render_alias_list_section(
        "Official OMC phases to preserve:",
        _framework_alias_workflow_items("autopilot", "phases"),
    )
    local_adaptations = _render_alias_list_section(
        "Local Rust/localization adaptations:",
        _framework_alias_local_adaptations("autopilot"),
    )
    ambiguous_owner = str(payload.get("reroute_when_ambiguous", "idea-to-plan"))
    unknown_root_cause_owner = str(payload.get("reroute_when_root_cause_unknown", "systematic-debugging"))
    canonical_owner = str(payload.get("canonical_owner", "execution-controller-coding"))
    return f"""---
description: Enter the repo's shared autopilot execution lane.
---

Treat `{entrypoint}` as a thin alias for the repository's native execution lane.

{upstream_baseline}

{stronger_requirements}

{official_phases}

{local_adaptations}

Follow this routing:

1. If the task is still ambiguous, first structure it the way `{ambiguous_owner}` would.
2. If the root cause is still unknown, switch into `{unknown_root_cause_owner}`.
3. Otherwise take the `{canonical_owner}` posture:
   - define the minimum success criteria
   - define the verification path
   - make the smallest complete change
   - keep going until the repo has real verification evidence or a real blocker

Use `skills/autopilot/SKILL.md`, `AGENT.md`, and the live continuity artifacts as the truth.
Keep user-facing wording centered on the repository's own capability, not host quirks or external compatibility history.
"""


def _build_claude_deepinterview_command() -> str:
    payload = _framework_alias_payload("deepinterview")
    entrypoint = _framework_alias_claude_entrypoint("deepinterview")
    stronger_requirements = _render_alias_upgrade_requirements("deepinterview")
    upstream_baseline = _render_alias_upstream_baseline("deepinterview")
    official_loop_rules = _render_alias_list_section(
        "Official OMC loop rules to preserve:",
        _framework_alias_workflow_items("deepinterview", "loop_rules"),
    )
    local_adaptations = _render_alias_list_section(
        "Local Rust/localization adaptations:",
        _framework_alias_local_adaptations("deepinterview"),
    )
    canonical_owner = str(payload.get("canonical_owner", "code-review"))
    review_lanes = payload.get("review_lanes")
    if not isinstance(review_lanes, list):
        raise ValueError("framework_native_aliases['deepinterview'] missing review_lanes")
    rendered_review_lanes = "\n".join(f"   - `{lane}`" for lane in review_lanes)
    return f"""---
description: Enter the repo's shared deepinterview lane.
---

Treat `{entrypoint}` as a thin alias for the repository's native review lane.

{upstream_baseline}

{stronger_requirements}

{official_loop_rules}

{local_adaptations}

Follow this routing:

1. Primary owner: `{canonical_owner}`
2. Add review lanes as needed:
{rendered_review_lanes}
3. If the root cause is still unknown, investigate it before summarizing findings.
4. Lead with findings, rank by severity, and cite concrete file or behavior evidence.
5. If the user wants fixes too, keep iterating review -> fix -> verify until the bounded scope converges.

Use `skills/deepinterview/SKILL.md`, `AGENT.md`, and the live repo state as the truth.
Keep user-facing wording centered on the repository's own review capability, not host quirks or external compatibility history.
"""


CLAUDE_AUTOPILOT_COMMAND = _build_claude_autopilot_command()
CLAUDE_DEEPINTERVIEW_COMMAND = _build_claude_deepinterview_command()


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

CLAUDE_ROUTER_RS_HOOK_RUNNER = """#!/bin/sh
set -eu

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"

run_router_rs() {
  cargo run --quiet --manifest-path "$PROJECT_DIR/scripts/router-rs/Cargo.toml" -- "$@"
}
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
- `printf '{"hook_event_name":"ConfigChange","source":"project_settings","file_path":".claude/settings.json"}\n' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/config_change.sh`
  Expected: audit-only stderr guidance about regenerating generated Claude host files; exit 0.
- `printf '{"hook_event_name":"StopFailure","error":"server_error"}\n' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/stop_failure.sh`
  Expected: host-private failure classification hint on stderr; exit 0.
- `cargo run --quiet --manifest-path scripts/router-rs/Cargo.toml -- --claude-hook-command session-start --repo-root "$PWD"`
  Expected: JSON result with `canonical_command`, `contract`, and `projection`.
- `cargo run --quiet --manifest-path scripts/router-rs/Cargo.toml -- --claude-hook-command session-end --repo-root "$PWD"`
  Expected: compatibility alias for `session-end`; same consolidation and projection contract.
- `printf '{"hook_event_name":"ConfigChange","source":"project_settings","file_path":".claude/settings.json"}\n' | cargo run --quiet --manifest-path scripts/router-rs/Cargo.toml -- --claude-hook-audit-command config-change --repo-root "$PWD"`
  Expected: JSON on stdout plus audit-only stderr guidance; exit 0.

Shared routing policy still comes from `../../AGENT.md`.
"""

CLAUDE_SESSION_START_HOOK = CLAUDE_ROUTER_RS_HOOK_RUNNER + """
run_router_rs --claude-hook-command session-start --repo-root "$PROJECT_DIR" >/dev/null
"""

CLAUDE_STOP_HOOK = CLAUDE_ROUTER_RS_HOOK_RUNNER + """
run_router_rs --claude-hook-command session-stop --repo-root "$PROJECT_DIR" >/dev/null
"""

CLAUDE_PRE_COMPACT_HOOK = CLAUDE_ROUTER_RS_HOOK_RUNNER + """
run_router_rs --claude-hook-command pre-compact --repo-root "$PROJECT_DIR" >/dev/null
"""

CLAUDE_SUBAGENT_STOP_HOOK = CLAUDE_ROUTER_RS_HOOK_RUNNER + """
run_router_rs --claude-hook-command subagent-stop --repo-root "$PROJECT_DIR" >/dev/null
"""

CLAUDE_SESSION_END_HOOK = CLAUDE_ROUTER_RS_HOOK_RUNNER + """
run_router_rs --claude-hook-command session-end --repo-root "$PROJECT_DIR" >/dev/null
"""

CLAUDE_CONFIG_CHANGE_HOOK = CLAUDE_ROUTER_RS_HOOK_RUNNER + """
run_router_rs --claude-hook-audit-command config-change --repo-root "$PROJECT_DIR" >/dev/null
"""

CLAUDE_STOP_FAILURE_HOOK = CLAUDE_ROUTER_RS_HOOK_RUNNER + """
run_router_rs --claude-hook-audit-command stop-failure --repo-root "$PROJECT_DIR" >/dev/null
"""

CLAUDE_PROJECT_SETTINGS = {
    "$schema": "https://json.schemastore.org/claude-code-settings.json",
    "permissions": {
        "allow": [
            "Bash(ls)",
            "Bash(pwd)",
            "Bash(rg *)",
            "Bash(cat *)",
            "Bash(sed -n *)",
            "Bash(git status)",
            "Bash(git diff)",
            "Bash(git show *)",
            "Bash(git rev-parse *)",
            "Bash(git ls-files *)",
            "Bash(python3 scripts/check_skills.py --verify-sync)",
            "Bash(python3 scripts/materialize_cli_host_entrypoints.py)",
            "Bash(python3 -m pytest *)",
            "Bash(python3 -m compileall *)",
            "Bash(cargo test *)",
            "Bash(cargo run --quiet --manifest-path */scripts/router-rs/Cargo.toml -- *)",
            "Bash(python3 scripts/runtime_background_cli.py *)",
            "Bash(cmp -s TRACE_METADATA.json artifacts/current/TRACE_METADATA.json)",
            "Bash(./tools/browser-mcp/scripts/start_browser_mcp.sh *)",
            "Bash(bash ./tools/browser-mcp/scripts/start_browser_mcp.sh *)",
        ]
    },
    "allowedMcpServers": [
        {"serverName": server_name} for server_name in shared_project_mcp_servers()
    ],
    "hooks": {
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
                "matcher": "invalid_request|server_error|max_output_tokens|rate_limit|authentication_failed|billing_error|unknown",
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
    ".claude/commands/autopilot.md": CLAUDE_AUTOPILOT_COMMAND,
    ".claude/commands/deepinterview.md": CLAUDE_DEEPINTERVIEW_COMMAND,
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
    ".claude/commands/autopilot.md",
    ".claude/commands/deepinterview.md",
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
    ".claude/commands/deepreview.md",
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
