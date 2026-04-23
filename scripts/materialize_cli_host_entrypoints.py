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

from framework_runtime.runtime_registry import framework_native_aliases, shared_project_mcp_servers
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

## Simplify First

- Prefer simplification before expansion: delete, merge, inline, or narrow an
  existing path before adding a new layer, branch, helper, or adapter.
- If two approaches both solve the task, prefer the one with fewer moving
  parts, fewer branching paths, and fewer files or surfaces to keep in sync.
- Prefer removing obsolete compatibility, transition, or fallback logic over
  wrapping it again, unless a real caller or rollout constraint still needs it.
- Keep hot paths simple: avoid repeated parse/serialize loops, repeated file
  I/O, copy-heavy data flow, and wrapper-on-wrapper structure when a direct path
  will do.
- Keep `AGENT.md` short and high-signal; if a rule only matters for one
  workflow, move it into the task-specific doc instead of bloating shared
  policy.

## Repo Landmarks

- `skills/` holds the shared routing and workflow bodies; read the selected
  `SKILL.md` before acting.
- `scripts/materialize_cli_host_entrypoints.py` is the source of truth for
  `AGENT.md`, `CLAUDE.md`, `.claude/settings.json`, and `.claude/hooks/*.sh`.
- `scripts/router-rs/` owns the Rust hook bridge, lifecycle commands, and
  generated-surface audits.
- `artifacts/current/` plus `.supervisor_state.json` are the durable task-state
  surfaces; do not treat chat text as the only recovery source.

## Communication Style

- Lead with the answer or result, not status reports, greetings, or self-talk.
- Use plain Chinese and everyday words by default.
- Explain things in plain language first; give internal terms only if they help.
- Avoid internal runtime, routing, framework, or tool jargon unless the user
  explicitly asks for it.
- If a technical term is necessary, explain it in simple words the first time.
- Keep the default reply to one short paragraph; use lists only when the content
  is genuinely list-shaped.
- Do not force personality, performative style, jokes, or deliberate roughness
  by default; sounding natural matters more than sounding theatrical.
- Keep the tone calm, friendly, and practical.

## Output Compaction

- For high-output local commands where exact raw output is not required, follow
  `RTK.md` and prefer the corresponding `rtk ...` wrapper.
- Treat `RTK.md` as repo-local operator guidance only; shared routing and
  policy truth still lives in this file plus the generated routing artifacts.

## Verification Defaults

- Verify the narrowest meaningful slice before handoff.
- For shared policy, host entrypoint, routing, or hook changes, prefer this
  order unless the task says otherwise: `python3 scripts/check_skills.py
  --verify-sync`, targeted `python3 -m pytest ...`, `python3 -m compileall
  ...`, and `cargo test --manifest-path ./scripts/router-rs/Cargo.toml` when
  Rust hook or runtime code changed.
- If you skip a verification step that would normally matter, say so plainly in
  the closeout.

## Task Closeout

- Keep end-of-task user-facing closeouts in plain Chinese by default.
- Default the closeout to one short paragraph that says what now works or what
  effect was achieved, and what still needs to happen next.
- Prefer user-visible effect over implementation narration in the default
  closeout.
- If no further work is needed, say that directly instead of inventing follow-up
  tasks.
- Do not default to changed-file inventories, changelog-style recaps, or
  step-by-step implementation retellings in the final user-facing closeout.
- Machine continuity artifacts such as `NEXT_ACTIONS.json`,
  `.supervisor_state.json`, and verification or blocker fields remain the
  recovery truth; do not mirror them verbatim into the user-facing closeout
  unless they materially affect the user's next decision.

## Policy Placement

- Put durable response policy in this file: answer-first phrasing, plain-language
  explanation, tone, closeout shape, and routing posture.
- Put deterministic runtime safeguards and narrow execution-time coding nudges
  in hooks: generated-surface protection, lifecycle refresh, environment
  reloads, failure alerts, and cheap repo-specific implementation reminders.
- Keep user-specific notifications, personal approvals, and machine-local
  preferences in `~/.claude/settings.json` or `.claude/settings.local.json`,
  not in committed project hooks.
- Do not use hooks to inject personality, carry general writing policy, or
  rewrite broad prompts when `AGENT.md` or `CLAUDE.md` can express the rule
  directly.
- Keep this file compact and factual. If a rule turns into a long workflow,
  move the procedure into `skills/`, `code_review.md`, or another task-specific
  doc and reference it instead of bloating this file.
- Add or tighten durable rules only after repeated real mistakes or verified
  friction.

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

CLAUDE_ROUTER_RS_RELEASE_BINARY = "./scripts/router-rs/target/release/router-rs"
CLAUDE_ROUTER_RS_DEBUG_BINARY = "./scripts/router-rs/target/debug/router-rs"
CLAUDE_ROUTER_RS_MANIFEST_PATH = "./scripts/router-rs/Cargo.toml"
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
description: Generate and copy the next-turn execution prompt with the Rust refresh command.
{allowed_tools}---

Run:

`{project_dir_snippet}; "$PROJECT_DIR"/scripts/router-rs/target/release/router-rs --framework-refresh-json --claude-hook-max-lines 4 --repo-root "$PROJECT_DIR"`

If the release binary is missing, rerun the same command with:

`{project_dir_snippet}; "$PROJECT_DIR"/scripts/router-rs/target/debug/router-rs --framework-refresh-json --claude-hook-max-lines 4 --repo-root "$PROJECT_DIR"`

If both resident binaries are missing, self-heal with:

`{project_dir_snippet}; cargo run --manifest-path "$PROJECT_DIR"/scripts/router-rs/Cargo.toml --release -- --framework-refresh-json --claude-hook-max-lines 4 --repo-root "$PROJECT_DIR"`

Then reply with exactly:
`下一轮执行 prompt 已准备好，并且已经复制到剪贴板。`
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


def _framework_alias_upstream_source(alias_name: str) -> dict[str, str]:
    payload = _framework_alias_payload(alias_name)
    raw = payload.get("upstream_source")
    if not isinstance(raw, dict):
        return {}
    return {str(key): str(value) for key, value in raw.items() if isinstance(value, str) and value}


def _build_claude_framework_alias_command(alias_name: str) -> str:
    entrypoint = _framework_alias_claude_entrypoint(alias_name)
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

`{project_dir_snippet}; "$PROJECT_DIR"/scripts/router-rs/target/release/router-rs --framework-alias-json --framework-alias {alias_name} --compact-output --claude-hook-max-lines 3 --repo-root "$PROJECT_DIR"`

If the release binary is missing, rerun the same command with:

`{project_dir_snippet}; "$PROJECT_DIR"/scripts/router-rs/target/debug/router-rs --framework-alias-json --framework-alias {alias_name} --compact-output --claude-hook-max-lines 3 --repo-root "$PROJECT_DIR"`

If both resident binaries are missing, self-heal with:

`{project_dir_snippet}; cargo run --manifest-path "$PROJECT_DIR"/scripts/router-rs/Cargo.toml --release -- --framework-alias-json --framework-alias {alias_name} --compact-output --claude-hook-max-lines 3 --repo-root "$PROJECT_DIR"`

Use `alias.state_machine` and `alias.entry_contract` as the working contract for this turn.
Prefer the Rust alias payload over opening long docs or restating OMC background.
Only open `{skill_path}` if the alias payload is missing something you still need.
Keep execution inside the repo's native Rust/continuity lane.
    """.format(
        alias_name=alias_name,
        allowed_tools=CLAUDE_ROUTER_RS_ALLOWED_TOOLS,
        entrypoint=entrypoint,
        project_dir_snippet=CLAUDE_PROJECT_DIR_SNIPPET,
        skill_path=skill_path,
    )


def _build_claude_autopilot_command() -> str:
    return _build_claude_framework_alias_command("autopilot")


def _build_claude_deepinterview_command() -> str:
    return _build_claude_framework_alias_command("deepinterview")


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
  if command -v cargo >/dev/null 2>&1; then
    cargo build --manifest-path "$ROUTER_RS_CRATE_ROOT/Cargo.toml" >/dev/null
    if [ -x "$ROUTER_RS_DEBUG_BIN" ]; then
      "$ROUTER_RS_DEBUG_BIN" "$@"
      return
    fi
    if [ -x "$ROUTER_RS_RELEASE_BIN" ]; then
      "$ROUTER_RS_RELEASE_BIN" "$@"
      return
    fi
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

- Edit `scripts/materialize_cli_host_entrypoints.py` first.
- Treat `.claude/settings.json`, this README, and `.claude/hooks/*.sh` as
  materialized outputs.
- Manual Claude host guidance belongs in `.claude/agents/*.md` unless noted.
- Codex repo hooks stay disabled here by default; keep shared hook logic scoped
  to Claude unless the project explicitly re-enables a Codex-specific layer.

Active hooks:

| Event | Script | Purpose |
| --- | --- | --- |
| `UserPromptSubmit` | `user_prompt_submit.sh` | Inject the repo-local shared memory and continuity truth on every real prompt so Claude starts from the project’s own memory root instead of stale host-global recall. |
| `PreToolUse` | `pre_tool_use_quality.sh` | Add a short path-aware implementation reminder before editing runtime, hook, or contract-test code that is already inside the narrow quality lane, and capture a lightweight pre-edit baseline for later delta-aware review. |
| `PreToolUse` | `pre_tool_use.sh` | Deny direct edits to generated host outputs and the imported Claude projection before `Edit`, `MultiEdit`, `Write`, or targeted `Bash` writes run. |
| `PostToolUse` | `post_tool_use_audit.sh` | Run a background implementation audit after real code edits and inspect the new delta first, so only newly introduced compatibility-heavy or wasteful patterns get fed back. |
| `SessionEnd` | `session_end.sh` | Consolidate project-local memory, refresh the Claude projection, and repair stale terminal resume state when needed. |
| `ConfigChange` | `config_change.sh` | Warn when generated Claude host files were edited directly instead of regenerated from source. |
| `StopFailure` | `stop_failure.sh` | Emit a host-private hint for selected Claude stop failures without mutating shared continuity. |

Everything else stays intentionally uninstalled here so startup and tool turns remain lean.
`UserPromptSubmit` is installed here on purpose: this repo keeps memory truth under
`./.codex/memory/` plus continuity artifacts, so prompt-time injection is the
lowest-friction way to keep Claude aligned with repo-local state instead of stale
host-global recall.
Reply tone, "讲人话" rules, and closeout style live in `AGENT.md`, not in hooks.
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

- `printf '{"tool_name":"Edit","tool_input":{"file_path":"scripts/router-rs/src/claude_hooks.rs"}}\n' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/pre_tool_use_quality.sh`
  Expected: stdout returns a JSON `permissionDecision: allow` payload with `additionalContext`.
- `printf '{"tool_name":"Edit","tool_input":{"file_path":"scripts/router-rs/src/claude_hooks.rs"}}\n' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/post_tool_use_audit.sh`
  Expected: stdout is empty for clean edits, or JSON with top-level `additionalContext` when the new delta still looks patchy, compatibility-heavy, or wasteful.
- `printf '{"tool_name":"MultiEdit","tool_input":{"file_path":".claude/settings.json"}}\n' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/pre_tool_use.sh`
  Expected: stdout returns a JSON `permissionDecision: deny` payload.
- `printf '{"tool_name":"Bash","tool_input":{"command":"cp tmp .claude/settings.json"}}\n' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/pre_tool_use.sh`
  Expected: stdout returns a JSON `permissionDecision: deny` payload for the targeted write.
- `printf '{"tool_name":"Bash","tool_input":{"command":"printf x > .claude/settings.json"}}\n' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/pre_tool_use.sh`
  Expected: stdout returns a JSON `permissionDecision: deny` payload for shell redirection into a protected generated file.
- `printf '{"hook_event_name":"UserPromptSubmit","prompt":"继续修复这个仓库的共享记忆和 runtime"}\n' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/user_prompt_submit.sh`
  Expected: stdout returns JSON with `hookSpecificOutput.additionalContext` containing repo-local memory and continuity reminders.
- `CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/session_end.sh`
  Expected: project-local memory bundle refresh plus projection refresh; may repair stale terminal resume state in `.supervisor_state.json`.
- `printf '{"hook_event_name":"ConfigChange","source":"project_settings","file_path":".claude/settings.json"}\n' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/config_change.sh`
  Expected: audit-only stderr guidance about regenerating generated Claude host files; exit 0.
- `printf '{"hook_event_name":"StopFailure","error":"server_error"}\n' | CLAUDE_PROJECT_DIR="$PWD" sh .claude/hooks/stop_failure.sh`
  Expected: host-private failure classification hint on stderr; exit 0.
- `./scripts/router-rs/target/debug/router-rs --claude-hook-command session-end --repo-root "$PWD" --claude-hook-max-lines 4`
  Expected: compatibility alias for `session-end`; same consolidation and projection contract.
- `printf '{"hook_event_name":"ConfigChange","source":"project_settings","file_path":".claude/settings.json"}\n' | ./scripts/router-rs/target/debug/router-rs --claude-hook-audit-command config-change --repo-root "$PWD"`
  Expected: JSON on stdout plus audit-only stderr guidance; exit 0.
- In Claude Code, run `/hooks`
  Expected: the project shows `PreToolUse`, `PostToolUse`, `SessionEnd`,
  `ConfigChange`, and `StopFailure` from `.claude/settings.json`.

Shared routing policy still comes from `../../AGENT.md`.
"""

CLAUDE_SESSION_END_HOOK = CLAUDE_ROUTER_RS_HOOK_RUNNER + """
run_router_rs --claude-hook-command session-end --repo-root "$PROJECT_DIR" >/dev/null
"""

CLAUDE_CONFIG_CHANGE_HOOK = CLAUDE_ROUTER_RS_HOOK_RUNNER + """
run_router_rs --claude-hook-audit-command config-change --repo-root "$PROJECT_DIR" >/dev/null
"""

CLAUDE_PRE_TOOL_USE_HOOK = CLAUDE_ROUTER_RS_HOOK_RUNNER + """
response="$(run_router_rs --claude-hook-audit-command pre-tool-use --repo-root "$PROJECT_DIR")"
if printf '%s' "$response" | grep -Eq '"permissionDecision"[[:space:]]*:[[:space:]]*"deny"'; then
  printf '%s\\n' "$response"
fi
"""

CLAUDE_USER_PROMPT_SUBMIT_HOOK = CLAUDE_ROUTER_RS_HOOK_RUNNER + """
response="$(run_router_rs --claude-hook-audit-command user-prompt-submit --repo-root "$PROJECT_DIR")"
if printf '%s' "$response" | grep -Eq '"hookSpecificOutput"[[:space:]]*:'; then
  printf '%s\\n' "$response"
fi
"""

CLAUDE_PRE_TOOL_USE_QUALITY_HOOK = CLAUDE_ROUTER_RS_HOOK_RUNNER + """
response="$(run_router_rs --claude-hook-audit-command pre-tool-use-quality --repo-root "$PROJECT_DIR")"
if printf '%s' "$response" | grep -Eq '"hookSpecificOutput"[[:space:]]*:'; then
  printf '%s\\n' "$response"
fi
"""

CLAUDE_POST_TOOL_USE_AUDIT_HOOK = CLAUDE_ROUTER_RS_HOOK_RUNNER + """
response="$(run_router_rs --claude-hook-audit-command post-tool-audit --repo-root "$PROJECT_DIR")"
if printf '%s' "$response" | grep -Eq '"hookSpecificOutput"[[:space:]]*:'; then
  printf '%s\\n' "$response"
fi
"""

CLAUDE_STOP_FAILURE_HOOK = CLAUDE_ROUTER_RS_HOOK_RUNNER + """
run_router_rs --claude-hook-audit-command stop-failure --repo-root "$PROJECT_DIR" >/dev/null
"""

CLAUDE_PRE_TOOL_USE_RULES = [
    "/AGENT.md",
    "/AGENTS.md",
    "/CLAUDE.md",
    "/GEMINI.md",
    "/.gemini/settings.json",
    "/.claude/settings.json",
    "/.claude/agents/README.md",
    "/.claude/hooks/README.md",
    "/.claude/hooks/*.sh",
    "/.claude/commands/**",
    "/.codex/hooks.json",
    "/.codex/host_entrypoints_sync_manifest.json",
    "/.codex/memory/CLAUDE_MEMORY.md",
]


def _claude_pre_tool_use_hooks() -> list[dict[str, Any]]:
    command = 'sh "$CLAUDE_PROJECT_DIR"/.claude/hooks/pre_tool_use.sh'
    return _build_claude_tool_path_hooks(command, CLAUDE_PRE_TOOL_USE_RULES)


def _claude_pre_tool_use_bash_hooks() -> list[dict[str, str]]:
    command = 'sh "$CLAUDE_PROJECT_DIR"/.claude/hooks/pre_tool_use.sh'
    hooks: list[dict[str, str]] = []
    for rule in (
        "*AGENT.md*",
        "*AGENTS.md*",
        "*CLAUDE.md*",
        "*GEMINI.md*",
        "*.gemini/settings.json*",
        "*.claude/settings.json*",
        "*.claude/agents/README.md*",
        "*.claude/hooks/*",
        "*.claude/commands/*",
        "*.codex/hooks.json*",
        "*.codex/host_entrypoints_sync_manifest.json*",
        "*.codex/memory/CLAUDE_MEMORY.md*",
    ):
        hooks.append({"type": "command", "if": f"Bash({rule})", "command": command})
    return hooks


def _build_claude_tool_path_hooks(
    command: str,
    rules: tuple[str, ...] | list[str],
    *,
    extras: dict[str, Any] | None = None,
) -> list[dict[str, Any]]:
    hooks: list[dict[str, Any]] = []
    for rule in rules:
        for tool_name in ("Edit", "MultiEdit", "Write"):
            hook = {"type": "command", "if": f"{tool_name}({rule})", "command": command}
            if extras:
                hook.update(extras)
            hooks.append(hook)
    return hooks


CLAUDE_QUALITY_PRE_TOOL_USE_RULES = (
    "/framework_runtime/src/**",
    "/scripts/router-rs/src/**",
    "/tests/test_cli_host_entrypoints.py",
    "/tests/test_codex_omx_hook_bridge.py",
)


def _claude_quality_pre_tool_use_hooks() -> list[dict[str, Any]]:
    command = 'sh "$CLAUDE_PROJECT_DIR"/.claude/hooks/pre_tool_use_quality.sh'
    return _build_claude_tool_path_hooks(command, CLAUDE_QUALITY_PRE_TOOL_USE_RULES)


def _claude_quality_post_tool_use_hooks() -> list[dict[str, Any]]:
    command = 'sh "$CLAUDE_PROJECT_DIR"/.claude/hooks/post_tool_use_audit.sh'
    return _build_claude_tool_path_hooks(
        command,
        CLAUDE_QUALITY_PRE_TOOL_USE_RULES,
        extras={"async": True, "timeout": 8},
    )

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
            f"Bash(cargo run --manifest-path {CLAUDE_ROUTER_RS_MANIFEST_PATH} --release -- *)",
            "Bash(./scripts/router-rs/target/release/router-rs *)",
            "Bash(./scripts/router-rs/target/debug/router-rs *)",
            "Bash(*scripts/router-rs/target/release/router-rs *)",
            "Bash(*scripts/router-rs/target/debug/router-rs *)",
            "Bash(cargo run --manifest-path *scripts/router-rs/Cargo.toml --release -- *)",
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
        "PreToolUse": [
            {
                "matcher": "Edit|MultiEdit|Write",
                "hooks": _claude_pre_tool_use_hooks(),
            },
            {
                "matcher": "Bash",
                "hooks": _claude_pre_tool_use_bash_hooks(),
            },
            {
                "matcher": "Edit|MultiEdit|Write",
                "hooks": _claude_quality_pre_tool_use_hooks(),
            },
        ],
        "PostToolUse": [
            {
                "matcher": "Edit|MultiEdit|Write",
                "hooks": _claude_quality_post_tool_use_hooks(),
            }
        ],
        "UserPromptSubmit": [
            {
                "hooks": [
                    {
                        "type": "command",
                        "command": "sh \"$CLAUDE_PROJECT_DIR\"/.claude/hooks/user_prompt_submit.sh",
                    }
                ]
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


CODEX_PROJECT_HOOKS = {"hooks": {}}

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
    ".claude/hooks/user_prompt_submit.sh": CLAUDE_USER_PROMPT_SUBMIT_HOOK,
    ".claude/hooks/pre_tool_use_quality.sh": CLAUDE_PRE_TOOL_USE_QUALITY_HOOK,
    ".claude/hooks/post_tool_use_audit.sh": CLAUDE_POST_TOOL_USE_AUDIT_HOOK,
    ".claude/hooks/pre_tool_use.sh": CLAUDE_PRE_TOOL_USE_HOOK,
    ".claude/hooks/session_end.sh": CLAUDE_SESSION_END_HOOK,
    ".claude/hooks/config_change.sh": CLAUDE_CONFIG_CHANGE_HOOK,
    ".claude/hooks/stop_failure.sh": CLAUDE_STOP_FAILURE_HOOK,
}

HOST_ENTRYPOINT_JSON_FILES = {
    ".codex/hooks.json": CODEX_PROJECT_HOOKS,
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

PARTIAL_SYNC_TEXT_FILES = tuple(sorted(HOST_ENTRYPOINT_TEXT_FILES))

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
            "json_files": sorted(HOST_ENTRYPOINT_JSON_FILES),
            "managed_directories": list(PARTIAL_SYNC_MANAGED_DIRECTORIES),
            "retired_paths": list(RETIRED_HOST_ENTRYPOINT_PATHS),
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


def write_host_entrypoint_template(template_root: Path) -> None:
    """Materialize one temporary template tree for host-entrypoint consumers."""

    _write_host_entrypoint_template(template_root)


def sync_repo_host_entrypoints(
    repo_root: Path | None = None,
    *,
    apply: bool,
) -> dict[str, list[str]]:
    """Check or write the shared and host-specific entrypoint files for this repository."""

    root = (repo_root or Path(__file__).resolve().parents[1]).resolve()
    with TemporaryDirectory() as temp_dir:
        template_root = Path(temp_dir)
        write_host_entrypoint_template(template_root)
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
