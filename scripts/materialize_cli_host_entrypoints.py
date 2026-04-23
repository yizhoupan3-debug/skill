#!/usr/bin/env python3
"""Materialize shared Codex/Claude/Gemini entrypoint files for this repo."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from subprocess import CalledProcessError
from tempfile import TemporaryDirectory
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from framework_runtime.runtime_registry import (
    framework_native_aliases as load_framework_native_aliases,
)
from framework_runtime.runtime_registry import load_runtime_registry
from framework_runtime.runtime_registry import (
    shared_project_mcp_servers as load_shared_project_mcp_servers,
)
from scripts.host_integration_runner import run_host_integration as _shared_run_host_integration
from scripts.rust_binary_runner import ensure_rust_binary


PROJECT_ROOT = Path(__file__).resolve().parents[1]

CLAUDE_ROUTER_RS_RELEASE_BINARY = "./scripts/router-rs/target/release/router-rs"
CLAUDE_ROUTER_RS_DEBUG_BINARY = "./scripts/router-rs/target/debug/router-rs"
CLAUDE_ROUTER_RS_MANIFEST_PATH = "./scripts/router-rs/Cargo.toml"

ROUTER_RS_PROJECTION_SOURCES = (
    PROJECT_ROOT / "AGENT.md",
    PROJECT_ROOT / "AGENTS.md",
    PROJECT_ROOT / "CLAUDE.md",
    PROJECT_ROOT / "GEMINI.md",
    PROJECT_ROOT / ".claude" / "hooks" / "README.md",
    PROJECT_ROOT / ".claude" / "hooks" / "run.sh",
)


def _ensure_router_rs_binary() -> Path:
    project_root = Path(__file__).resolve().parents[1]
    crate_root = project_root / "scripts" / "router-rs"
    return ensure_rust_binary(
        crate_root=crate_root,
        binary_name="router-rs",
        release=True,
        allow_stale_fallback=False,
        allow_cross_profile_fallback=False,
        cwd=project_root,
        extra_source_paths=ROUTER_RS_PROJECTION_SOURCES,
    )


def _load_router_rs_json_output(*args: str) -> dict[str, Any]:
    binary_path = _ensure_router_rs_binary()
    project_root = Path(__file__).resolve().parents[1]
    command = [str(binary_path), *args]
    try:
        completed = subprocess.run(
            command,
            cwd=project_root,
            check=True,
            text=True,
            capture_output=True,
        )
    except CalledProcessError as exc:
        # Host entrypoint regeneration is a bootstrap path. If the existing
        # release binary is behaviorally stale despite mtimes, rebuild and run
        # through cargo once instead of failing the entire sync chain.
        retry = subprocess.run(
            [
                "cargo",
                "run",
                "--quiet",
                "--manifest-path",
                str(project_root / "scripts" / "router-rs" / "Cargo.toml"),
                "--release",
                "--",
                *args,
            ],
            cwd=project_root,
            check=True,
            text=True,
            capture_output=True,
        )
        completed = retry
    payload = json.loads(completed.stdout)
    if not isinstance(payload, dict):
        raise ValueError("router-rs output must be a JSON object")
    return payload


def _load_claude_hook_manifest() -> dict[str, Any]:
    return _load_router_rs_json_output("--claude-hook-manifest-json")


def _load_claude_project_settings(repo_root: Path) -> dict[str, Any]:
    return _load_router_rs_json_output(
        "--claude-project-settings-json",
        "--repo-root",
        str(repo_root),
    )


def _load_claude_hook_projection() -> dict[str, Any]:
    return _load_router_rs_json_output("--claude-hook-projection-json")


CLAUDE_PROJECT_DIR_SNIPPET = 'PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"'
CLAUDE_ROUTER_RS_ALLOWED_TOOLS = """allowed-tools:
  - Bash(git rev-parse *)
  - Bash(./scripts/router-rs/target/release/router-rs *)
  - Bash(./scripts/router-rs/target/debug/router-rs *)
  - Bash(*scripts/router-rs/target/release/router-rs *)
  - Bash(*scripts/router-rs/target/debug/router-rs *)
  - Bash(cargo run --manifest-path ./scripts/router-rs/Cargo.toml --release -- *)
  - Bash(cargo run --manifest-path *scripts/router-rs/Cargo.toml --release -- *)
"""

CLAUDE_REFRESH_COMMAND = """---
description: 使用 Rust refresh 命令继续当前活跃任务，并复制下一轮执行提示。
{allowed_tools}---

把 `/refresh` 当作当前仓库唯一显式的 continue / next 入口。
它会读取现有 continuity 真源，为当前活跃任务生成下一轮执行提示。

运行：

`{project_dir_snippet}; "$PROJECT_DIR"/scripts/router-rs/target/release/router-rs --framework-refresh-json --claude-hook-max-lines 4 --repo-root "$PROJECT_DIR"`

如果 release 二进制不存在，用下面的命令重试：

`{project_dir_snippet}; "$PROJECT_DIR"/scripts/router-rs/target/debug/router-rs --framework-refresh-json --claude-hook-max-lines 4 --repo-root "$PROJECT_DIR"`

如果两个常驻二进制都不存在，用下面的命令自修复：

`{project_dir_snippet}; cargo run --manifest-path "$PROJECT_DIR"/scripts/router-rs/Cargo.toml --release -- --framework-refresh-json --claude-hook-max-lines 4 --repo-root "$PROJECT_DIR"`

然后严格回复：
`下一轮执行提示已准备好，并且已经复制到剪贴板。`
""".format(
    allowed_tools=CLAUDE_ROUTER_RS_ALLOWED_TOOLS,
    project_dir_snippet=CLAUDE_PROJECT_DIR_SNIPPET,
)

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

def _runtime_registry_payload() -> dict[str, Any]:
    return load_runtime_registry(repo_root=PROJECT_ROOT)


def _run_host_integration_command(*args: str) -> dict[str, Any]:
    return _shared_run_host_integration(*args, cwd=PROJECT_ROOT)


def _framework_native_aliases() -> dict[str, Any]:
    return load_framework_native_aliases(repo_root=PROJECT_ROOT)


def _shared_project_mcp_servers() -> tuple[str, ...]:
    return load_shared_project_mcp_servers(repo_root=PROJECT_ROOT)


def _framework_alias_payload(alias_name: str) -> dict[str, Any]:
    payload = _framework_native_aliases().get(alias_name)
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


def _framework_alias_upstream_source(alias_name: str) -> dict[str, str]:
    payload = _framework_alias_payload(alias_name)
    raw = payload.get("upstream_source")
    if not isinstance(raw, dict):
        return {}
    return {str(key): str(value) for key, value in raw.items() if isinstance(value, str) and value}


def _framework_alias_interaction_invariants(alias_name: str) -> dict[str, Any]:
    payload = _framework_alias_payload(alias_name)
    invariants = payload.get("interaction_invariants")
    if isinstance(invariants, dict):
        explicit_entrypoints = invariants.get("explicit_entrypoints")
        if isinstance(explicit_entrypoints, list) and any(
            isinstance(item, str) and item for item in explicit_entrypoints
        ):
            return invariants
    lane_contract = payload.get("lane_contract")
    explicit_entrypoints: list[str] = []
    if isinstance(lane_contract, dict):
        raw = lane_contract.get("explicit_entrypoints")
        if isinstance(raw, list):
            explicit_entrypoints = [str(item) for item in raw if isinstance(item, str) and item]
    if not explicit_entrypoints:
        host_entrypoints = payload.get("host_entrypoints")
        if isinstance(host_entrypoints, dict):
            explicit_entrypoints = [
                str(item)
                for item in host_entrypoints.values()
                if isinstance(item, str) and item
            ]
    implicit_route_policy = "manual-only"
    if isinstance(invariants, dict):
        raw_policy = invariants.get("implicit_route_policy")
        if isinstance(raw_policy, str) and raw_policy:
            implicit_route_policy = raw_policy
    return {
        "explicit_entrypoints": explicit_entrypoints,
        "implicit_route_policy": implicit_route_policy,
    }


def _build_claude_framework_alias_command(alias_name: str) -> str:
    entrypoint = _framework_alias_claude_entrypoint(alias_name)
    interaction_invariants = _framework_alias_interaction_invariants(alias_name)
    explicit_entrypoints = interaction_invariants.get("explicit_entrypoints")
    if not isinstance(explicit_entrypoints, list):
        explicit_entrypoints = []
    explicit_entrypoint_text = ", ".join(
        f"`{item}`" for item in explicit_entrypoints if isinstance(item, str) and item
    )
    implicit_route_policy = interaction_invariants.get("implicit_route_policy")
    if not isinstance(implicit_route_policy, str):
        implicit_route_policy = "unknown"
    skill_path = _framework_alias_upstream_source(alias_name).get(
        "official_skill_path",
        f"skills/{alias_name}/SKILL.md",
    )
    return """---
description: Enter the repo's Rust-owned {alias_name} lane.
{allowed_tools}---

Treat `{entrypoint}` as a thin Rust-first alias.
This command now enters the repo through the resident Rust binary directly.

Run:

`{project_dir_snippet}; "$PROJECT_DIR"/scripts/router-rs/target/release/router-rs --framework-alias-json --framework-alias {alias_name} --framework-host-id claude-code --compact-output --claude-hook-max-lines 3 --repo-root "$PROJECT_DIR"`

If the release binary is missing, rerun the same command with:

`{project_dir_snippet}; "$PROJECT_DIR"/scripts/router-rs/target/debug/router-rs --framework-alias-json --framework-alias {alias_name} --framework-host-id claude-code --compact-output --claude-hook-max-lines 3 --repo-root "$PROJECT_DIR"`

If both resident binaries are missing, self-heal with:

`{project_dir_snippet}; cargo run --manifest-path "$PROJECT_DIR"/scripts/router-rs/Cargo.toml --release -- --framework-alias-json --framework-alias {alias_name} --framework-host-id claude-code --compact-output --claude-hook-max-lines 3 --repo-root "$PROJECT_DIR"`

Use `alias.state_machine` and `alias.entry_contract` as the working contract for this turn.
This alias only enters through explicit entrypoints: {explicit_entrypoint_text}.
Implicit routing policy: `{implicit_route_policy}`.
Prefer the Rust alias payload over opening long docs or restating OMC background.
Only open `{skill_path}` if the alias payload is missing something you still need.
Keep execution inside the repo's native Rust/continuity lane.
    """.format(
        alias_name=alias_name,
        allowed_tools=CLAUDE_ROUTER_RS_ALLOWED_TOOLS,
        entrypoint=entrypoint,
        project_dir_snippet=CLAUDE_PROJECT_DIR_SNIPPET,
        skill_path=skill_path,
        explicit_entrypoint_text=explicit_entrypoint_text,
        implicit_route_policy=implicit_route_policy,
    )


def _build_claude_autopilot_command() -> str:
    return _build_claude_framework_alias_command("autopilot")


def _build_claude_deepinterview_command() -> str:
    return _build_claude_framework_alias_command("deepinterview")


def _build_claude_team_command() -> str:
    return _build_claude_framework_alias_command("team")


def _build_claude_latex_compile_acceleration_command() -> str:
    return _build_claude_framework_alias_command("latex-compile-acceleration")


CLAUDE_AUTOPILOT_COMMAND = _build_claude_autopilot_command()
CLAUDE_DEEPINTERVIEW_COMMAND = _build_claude_deepinterview_command()
CLAUDE_TEAM_COMMAND = _build_claude_team_command()
CLAUDE_LATEX_COMPILE_ACCELERATION_COMMAND = _build_claude_latex_compile_acceleration_command()


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
ROUTER_RS_RELEASE_BIN="$PROJECT_DIR/scripts/router-rs/target/release/router-rs"
ROUTER_RS_DEBUG_BIN="$PROJECT_DIR/scripts/router-rs/target/debug/router-rs"
ROUTER_RS_CRATE_ROOT="$PROJECT_DIR/scripts/router-rs"

router_rs_is_fresh() {
  bin_path="$1"
  [ -x "$bin_path" ] || return 1
  [ "$ROUTER_RS_CRATE_ROOT/Cargo.toml" -nt "$bin_path" ] && return 1
  find "$ROUTER_RS_CRATE_ROOT/src" -type f -newer "$bin_path" | grep -q . && return 1
  return 0
}

run_router_rs() {
  if router_rs_is_fresh "$ROUTER_RS_RELEASE_BIN"; then
    "$ROUTER_RS_RELEASE_BIN" "$@"
    return
  fi
  if router_rs_is_fresh "$ROUTER_RS_DEBUG_BIN"; then
    "$ROUTER_RS_DEBUG_BIN" "$@"
    return
  fi
  if [ -x "$ROUTER_RS_RELEASE_BIN" ]; then
    "$ROUTER_RS_RELEASE_BIN" "$@"
    return
  fi
  if [ -x "$ROUTER_RS_DEBUG_BIN" ]; then
    "$ROUTER_RS_DEBUG_BIN" "$@"
    return
  fi
  echo "Missing required router-rs binary: $ROUTER_RS_RELEASE_BIN or $ROUTER_RS_DEBUG_BIN" >&2
  exit 1
}
"""

CLAUDE_HOOKS_README = """# Claude Hooks Directory

Claude Code project hooks live here.

Generated-first maintenance:

- Edit `scripts/materialize_cli_host_entrypoints.py` for host-entrypoint rendering, and update `scripts/router-rs/` first for Claude hook rules and contracts.
- Treat `.claude/settings.json`, this README, and `.claude/hooks/*.sh` as
  materialized outputs.
- Manual Claude host guidance belongs in `.claude/agents/*.md` unless noted.
- Codex uses `.codex/hooks.json` for a separate silent preflight guardrail
  layer on `Edit`/`MultiEdit`/`Write`/`Bash`; do not mirror Claude prompt hooks onto Codex.

Active hooks:

| Event | Runner | Purpose |
| --- | --- | --- |
| `UserPromptSubmit` | `run.sh user-prompt-submit` | Inject the repo-local shared memory and continuity truth on every real prompt, plus narrow execution-time hints when the current prompt clearly needs them. |
| `PreToolUse` | `run.sh pre-tool-use-quality` | Add a short path-aware implementation reminder before editing runtime, materializer, hook, or contract-test code that is already inside the narrow quality lane, and capture a lightweight pre-edit baseline for later delta-aware review. |
| `PreToolUse` | `run.sh pre-tool-use` | Deny direct edits to generated host outputs and the imported Claude projection before `Edit`, `MultiEdit`, `Write`, or targeted `Bash` writes run. |
| `PostToolUse` | `run.sh post-tool-audit` | Run a background implementation audit after real code edits and inspect the new delta first, so only newly introduced compatibility-heavy or wasteful patterns get fed back. |
| `SessionEnd` | `run.sh session-end` | Consolidate project-local memory, refresh the Claude projection, and repair stale terminal resume state when needed. |
| `ConfigChange` | `run.sh config-change` | Warn when generated Claude host files were edited directly instead of regenerated from source. |
| `StopFailure` | `run.sh stop-failure` | Emit a host-private hint for selected Claude stop failures without mutating shared continuity. |

Everything else stays intentionally uninstalled here so startup and tool turns remain lean.
`UserPromptSubmit` is installed here on purpose: this repo keeps memory truth under
`./.codex/memory/` plus continuity artifacts, so prompt-time injection is the
lowest-friction way to keep Claude aligned with repo-local state instead of stale
host-global recall.
Reply tone, "讲人话" rules, closeout shape, and broad implementation philosophy
still live in `AGENT.md`, not in hooks.
Static behavior rules belong in `AGENT.md` or `CLAUDE.md`; these hooks exist
for deterministic guardrails, lightweight execution-time context, and lifecycle
maintenance.

Project hook principles:

- Keep project hooks for repo-specific invariants only.
- Keep hooks fast, especially `PreToolUse`, because it runs inside the agent
  loop.
- Use `matcher` first and `if` to narrow further, so hook handlers do not spawn
  on unrelated tool calls and normal edits stay fast.
- Automation hooks should be additive and short: inject narrow repo context or
  launch cheap follow-up work, not essay-length prompt rewrites.
- Keep durable implementation philosophy in `AGENT.md`; hook-time nudges should
  stay concrete, local to the current path, and local to the current delta.
- Prefer async `PostToolUse` for cheap quality follow-up that should not block
  the main turn.
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

- `printf '{"tool_name":"Edit","tool_input":{"file_path":"scripts/router-rs/src/claude_hooks.rs"}}\n' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/run.sh pre-tool-use-quality`
  Expected: stdout returns a JSON `permissionDecision: allow` payload with `additionalContext`.
- `printf '{"tool_name":"Edit","tool_input":{"file_path":"scripts/router-rs/src/claude_hooks.rs"}}\n' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/run.sh post-tool-audit`
  Expected: stdout is empty for clean edits, or JSON with top-level `additionalContext` when the new delta still looks patchy, compatibility-heavy, or wasteful.
- `printf '{"tool_name":"Edit","tool_input":{"file_path":"scripts/materialize_cli_host_entrypoints.py"}}\n' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/run.sh pre-tool-use-quality`
  Expected: stdout returns a JSON `permissionDecision: allow` payload with Python-oriented `additionalContext`.
- `printf '{"tool_name":"Edit","tool_input":{"file_path":".claude/hooks/run.sh"}}\n' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/run.sh pre-tool-use-quality`
  Expected: stdout returns a JSON `permissionDecision: allow` payload with hook-oriented `additionalContext`.
- `printf '{"tool_name":"MultiEdit","tool_input":{"file_path":".claude/settings.json"}}\n' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/run.sh pre-tool-use`
  Expected: stdout returns a JSON `permissionDecision: deny` payload.
- `printf '{"tool_name":"Bash","tool_input":{"command":"cp tmp .claude/settings.json"}}\n' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/run.sh pre-tool-use`
  Expected: stdout returns a JSON `permissionDecision: deny` payload for the targeted write.
- `printf '{"tool_name":"Bash","tool_input":{"command":"printf x > .claude/settings.json"}}\n' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/run.sh pre-tool-use`
  Expected: stdout returns a JSON `permissionDecision: deny` payload for shell redirection into a protected generated file.
- `printf '{"hook_event_name":"UserPromptSubmit","prompt":"继续修复这个仓库的共享记忆和 runtime"}\n' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/run.sh user-prompt-submit`
  Expected: stdout returns JSON with `hookSpecificOutput.additionalContext` containing repo-local memory and continuity reminders.
- `CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/run.sh session-end`
  Expected: project-local memory bundle refresh plus projection refresh; may repair stale terminal resume state in `.supervisor_state.json`.
- `printf '{"hook_event_name":"ConfigChange","source":"project_settings","file_path":".claude/settings.json"}\n' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/run.sh config-change`
  Expected: audit-only stderr guidance about regenerating generated Claude host files; exit 0.
- `printf '{"hook_event_name":"StopFailure","error":"server_error"}\n' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/run.sh stop-failure`
  Expected: host-private failure classification hint on stderr; exit 0.
- `./scripts/router-rs/target/debug/router-rs --claude-hook-command session-end --repo-root "$PWD" --claude-hook-max-lines 4`
  Expected: compatibility alias for `session-end`; same consolidation and projection contract.
- `printf '{"hook_event_name":"ConfigChange","source":"project_settings","file_path":".claude/settings.json"}\n' | ./scripts/router-rs/target/debug/router-rs --claude-hook-audit-command config-change --repo-root "$PWD"`
  Expected: JSON on stdout plus audit-only stderr guidance; exit 0.
- In Claude Code, run `/hooks`
  Expected: the project shows `PreToolUse`, `PostToolUse`, `UserPromptSubmit`,
  `SessionEnd`, `ConfigChange`, and `StopFailure` from `.claude/settings.json`.

Shared routing policy still comes from `../../AGENT.md`.
"""

CLAUDE_HOOK_RUNNER = CLAUDE_ROUTER_RS_HOOK_RUNNER + """
command_name="${1:-}"
if [ -z "$command_name" ]; then
  echo "Missing Claude hook command name" >&2
  exit 1
fi

case "$command_name" in
  session-end)
    run_router_rs --claude-hook-command session-end --repo-root "$PROJECT_DIR" >/dev/null
    ;;
  config-change|stop-failure)
    run_router_rs --claude-hook-audit-command "$command_name" --repo-root "$PROJECT_DIR" >/dev/null
    ;;
  pre-tool-use)
    response="$(run_router_rs --claude-hook-audit-command pre-tool-use --repo-root "$PROJECT_DIR")"
    if printf '%s' "$response" | grep -Eq '"permissionDecision"[[:space:]]*:[[:space:]]*"deny"'; then
      printf '%s\\n' "$response"
    fi
    ;;
  user-prompt-submit)
    response="$(run_router_rs --claude-hook-audit-command "$command_name" --repo-root "$PROJECT_DIR")"
    if [ -n "$response" ]; then
      if printf '%s' "$response" | grep -Eq '"hookSpecificOutput"[[:space:]]*:'; then
        printf '%s\\n' "$response"
      else
        printf '[claude-user-prompt-submit] shared hook returned no hookSpecificOutput; continuing with degraded context.\\n' >&2
      fi
    fi
    ;;
  pre-tool-use-quality|post-tool-audit)
    response="$(run_router_rs --claude-hook-audit-command "$command_name" --repo-root "$PROJECT_DIR")"
    if printf '%s' "$response" | grep -Eq '"hookSpecificOutput"[[:space:]]*:'; then
      printf '%s\\n' "$response"
    fi
    ;;
  *)
    echo "Unsupported Claude hook command: $command_name" >&2
    exit 1
    ;;
esac
"""
HOST_ENTRYPOINT_SYNC_MANIFEST_PATH = ".codex/host_entrypoints_sync_manifest.json"


def _host_entrypoint_text_files() -> dict[str, str]:
    hook_projection = _load_claude_hook_projection()
    agent_policy = hook_projection.get("agent_policy")
    root_agents_proxy = hook_projection.get("root_agents_proxy")
    root_claude_proxy = hook_projection.get("root_claude_proxy")
    root_gemini_proxy = hook_projection.get("root_gemini_proxy")
    hooks_readme = hook_projection.get("hooks_readme")
    hook_runner = hook_projection.get("hook_runner")
    if not all(
        isinstance(value, str)
        for value in (
            agent_policy,
            root_agents_proxy,
            root_claude_proxy,
            root_gemini_proxy,
            hooks_readme,
            hook_runner,
        )
    ):
        raise ValueError(
            "Rust hook projection must include AGENT/proxy text plus hooks_readme and hook_runner strings"
        )
    return {
        "AGENT.md": agent_policy,
        "AGENTS.md": root_agents_proxy,
        "CLAUDE.md": root_claude_proxy,
        "GEMINI.md": root_gemini_proxy,
        ".claude/agents/README.md": CLAUDE_AGENTS_README,
        ".claude/commands/refresh.md": CLAUDE_REFRESH_COMMAND,
        ".claude/commands/background_batch.md": CLAUDE_BACKGROUND_BATCH_COMMAND,
        ".claude/commands/autopilot.md": CLAUDE_AUTOPILOT_COMMAND,
        ".claude/commands/deepinterview.md": CLAUDE_DEEPINTERVIEW_COMMAND,
        ".claude/commands/team.md": CLAUDE_TEAM_COMMAND,
        ".claude/commands/latex-compile-acceleration.md": CLAUDE_LATEX_COMPILE_ACCELERATION_COMMAND,
        ".claude/hooks/README.md": hooks_readme,
        ".claude/hooks/run.sh": hook_runner,
    }


HOST_ENTRYPOINT_JSON_RELATIVE_PATHS = (
    ".codex/hooks.json",
    ".claude/settings.json",
    ".gemini/settings.json",
)


def _host_entrypoint_json_files(repo_root: Path) -> dict[str, dict[str, Any]]:
    hook_projection = _load_claude_hook_projection()
    codex_hooks = hook_projection.get("codex_hooks")
    if not isinstance(codex_hooks, dict):
        raise ValueError("Rust hook projection must include codex_hooks object")
    return {
        ".codex/hooks.json": codex_hooks,
        ".gemini/settings.json": {},
        ".claude/settings.json": _load_claude_project_settings(repo_root),
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
    "AGENT.md",
    "AGENTS.md",
    "CLAUDE.md",
    "GEMINI.md",
    ".claude/agents/README.md",
    ".claude/commands/autopilot.md",
    ".claude/commands/background_batch.md",
    ".claude/commands/deepinterview.md",
    ".claude/commands/latex-compile-acceleration.md",
    ".claude/commands/refresh.md",
    ".claude/commands/team.md",
    ".claude/hooks/README.md",
    ".claude/hooks/run.sh",
)

PARTIAL_SYNC_MANAGED_DIRECTORIES = FULL_SYNC_MANAGED_DIRECTORIES

RETIRED_HOST_ENTRYPOINT_PATHS = (
    ".claude/CLAUDE.md",
    ".codex/model_instructions.md",
    ".mcp.json",
    "configs/codex/AGENTS.md",
    "configs/claude/CLAUDE.md",
    "configs/gemini/GEMINI.md",
    ".claude/commands/deepreview.md",
    ".claude/hooks/session_start.sh",
    ".claude/hooks/stop.sh",
    ".claude/hooks/pre_compact.sh",
    ".claude/hooks/subagent_stop.sh",
    ".claude/hooks/user_prompt_submit.sh",
    ".claude/hooks/pre_tool_use_quality.sh",
    ".claude/hooks/post_tool_use_audit.sh",
    ".claude/hooks/pre_tool_use.sh",
    ".claude/hooks/session_end.sh",
    ".claude/hooks/config_change.sh",
    ".claude/hooks/stop_failure.sh",
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
            "text_files": sorted(_host_entrypoint_text_files()),
            "json_files": sorted(HOST_ENTRYPOINT_JSON_RELATIVE_PATHS),
            "managed_directories": list(FULL_SYNC_MANAGED_DIRECTORIES),
            "retired_paths": list(RETIRED_HOST_ENTRYPOINT_PATHS),
        },
        "partial_sync": {
            "text_files": list(PARTIAL_SYNC_TEXT_FILES),
            "json_files": sorted(HOST_ENTRYPOINT_JSON_RELATIVE_PATHS),
            "managed_directories": list(PARTIAL_SYNC_MANAGED_DIRECTORIES),
            "retired_paths": list(RETIRED_HOST_ENTRYPOINT_PATHS),
        },
    }


def _write_host_entrypoint_template(template_root: Path, *, repo_root: Path) -> None:
    for relative_path, content in _host_entrypoint_text_files().items():
        _write_text(template_root / relative_path, content)
    for relative_path, payload in _host_entrypoint_json_files(repo_root).items():
        _write_json(template_root / relative_path, payload)
    _write_json(
        template_root / HOST_ENTRYPOINT_SYNC_MANIFEST_PATH,
        _build_host_entrypoint_sync_manifest(),
    )


def write_host_entrypoint_template(template_root: Path, *, repo_root: Path | None = None) -> None:
    """Materialize one temporary template tree for host-entrypoint consumers."""

    _write_host_entrypoint_template(
        template_root,
        repo_root=(repo_root or Path(__file__).resolve().parents[1]).resolve(),
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
        write_host_entrypoint_template(template_root, repo_root=root)
        return _run_host_integration_command(
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
