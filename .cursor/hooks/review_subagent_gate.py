#!/usr/bin/env python3
"""Cursor hook: enforce independent subagent for broad/deep review prompts.

Cursor-specific glue. Compared to the Codex counterpart this hook:
- runs as a single script dispatched via `--event <name>` CLI arg
- handles 8 events: beforeSubmitPrompt, subagentStart, subagentStop,
  postToolUse, afterAgentResponse, stop, preCompact, sessionEnd
- soft-enforces via `followup_message` because Cursor `stop` cannot hard-block
- persists a per-session phase state machine under `.cursor/hook-state/`

Phase state machine
-------------------
phase 0 = idle (default)
phase 1 = armed (review or delegation required, no override/reject yet)
phase 2 = subagent_seen (subagentStart fired OR Task tool used)
phase 3 = subagent_completed (subagentStop fired — strongest evidence)

`stop` is satisfied when:
- phase >= 2, OR
- review_override / delegation_override is set, OR
- reject_reason_seen is set (in user prompt or agent response), OR
- nothing was required (idle phase 0)

Policy: gate is OPT-IN. It activates when the prompt matches
`is_review_prompt`, `is_parallel_delegation_prompt`, or explicit framework
entrypoints (`/autopilot`, `$autopilot`, `/team`, `$team`, `/gitx`, `$gitx`). Plain prompts do NOT
arm the gate. See `.cursor/rules/review-subagent-gate.mdc` for the canonical
trigger spec.
"""

from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
import sys
import time
import uuid
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

_HERE = Path(__file__).resolve().parent
if str(_HERE) not in sys.path:
    sys.path.insert(0, str(_HERE))

from _patterns import (  # noqa: E402
    SUBAGENT_TYPES,
    SUBAGENT_TOOL_NAMES,
    has_goal_blocker_signal,
    has_goal_contract_signal,
    has_goal_progress_signal,
    has_goal_verify_signal,
    has_delegation_override,
    has_override,
    has_review_override,
    is_autopilot_entrypoint_prompt,
    is_framework_entrypoint_prompt,
    is_parallel_delegation_prompt,
    is_review_prompt,
    normalize_subagent_type,
    normalize_tool_name,
    saw_reject_reason,
)

STATE_VERSION = 2
MAX_GOAL_NO_PROGRESS_LOOPS = 2

# ---------- IO ----------


def read_event() -> dict[str, Any]:
    raw = sys.stdin.read()
    if not raw.strip():
        return {}
    try:
        payload = json.loads(raw)
    except json.JSONDecodeError:
        return {"__stdin_io_error": "stdin_json_invalid"}
    return payload if isinstance(payload, dict) else {}


def print_json(payload: dict[str, Any]) -> None:
    sys.stdout.write(json.dumps(payload, ensure_ascii=False) + "\n")


# ---------- Event accessors ----------


def prompt_text(event: dict[str, Any]) -> str:
    for key in ("prompt", "user_prompt", "message", "input", "text"):
        value = event.get(key)
        if isinstance(value, str):
            return value
    return ""


def agent_response_text(event: dict[str, Any]) -> str:
    for key in ("response", "agent_response", "content", "text", "message", "output"):
        value = event.get(key)
        if isinstance(value, str):
            return value
    return ""


def tool_name_of(event: dict[str, Any]) -> str:
    return str(event.get("tool_name") or event.get("tool") or event.get("name") or "")


def tool_input_of(event: dict[str, Any]) -> dict[str, Any]:
    value = event.get("tool_input") or event.get("input") or event.get("arguments")
    return value if isinstance(value, dict) else {}


def loop_count_of(event: dict[str, Any]) -> int:
    raw = event.get("loop_count") or event.get("loopCount") or event.get("loop")
    try:
        return int(raw)
    except (TypeError, ValueError):
        return 0


# ---------- Session / state path ----------


def session_key(event: dict[str, Any]) -> str:
    """Stable per-session hash. Falls back to a per-run uuid when *no* identifier
    is supplied, so unrelated sessions never share the same state file.
    """
    for key in ("session_id", "conversation_id", "thread_id", "agent_id"):
        value = event.get(key)
        if isinstance(value, str) and value.strip():
            return hashlib.sha256(value.encode("utf-8")).hexdigest()[:32]
    cwd = event.get("cwd")
    if isinstance(cwd, str) and cwd.strip():
        return hashlib.sha256(("cwd::" + cwd).encode("utf-8")).hexdigest()[:32]
    fallback = f"ephemeral::{uuid.uuid4()}"
    return hashlib.sha256(fallback.encode("utf-8")).hexdigest()[:32]


def _candidate_paths(event: dict[str, Any]) -> list[Path]:
    candidates: list[Path] = []
    cwd = event.get("cwd")
    if isinstance(cwd, str) and cwd.strip():
        try:
            candidates.append(Path(cwd).resolve())
        except OSError:
            pass
    try:
        candidates.append(Path(os.getcwd()).resolve())
    except OSError:
        pass
    candidates.append(Path(__file__).resolve().parents[2])
    seen: set[Path] = set()
    deduped: list[Path] = []
    for p in candidates:
        if p not in seen:
            deduped.append(p)
            seen.add(p)
    return deduped


def repo_root(event: dict[str, Any]) -> Path:
    """Locate the Cursor workspace root by probing for `.cursor/hooks/`."""
    for candidate in _candidate_paths(event):
        for probe in [candidate, *candidate.parents]:
            if (probe / ".cursor" / "hooks" / "review_subagent_gate.py").exists():
                return probe
            if (probe / ".git").exists() and (probe / ".cursor").is_dir():
                return probe
    return _candidate_paths(event)[0]


def state_dir(event: dict[str, Any]) -> Path:
    return repo_root(event) / ".cursor" / "hook-state"


def state_path(event: dict[str, Any]) -> Path:
    return state_dir(event) / f"review-subagent-{session_key(event)}.json"


def state_lock_path(event: dict[str, Any]) -> Path:
    return state_dir(event) / f"review-subagent-{session_key(event)}.lock"


def acquire_state_lock(event: dict[str, Any]) -> Any | None:
    directory = state_dir(event)
    try:
        directory.mkdir(parents=True, exist_ok=True)
        handle = state_lock_path(event).open("a+", encoding="utf-8")
        fcntl.flock(handle.fileno(), fcntl.LOCK_EX)
        return handle
    except OSError:
        return None


def release_state_lock(handle: Any | None) -> None:
    if handle is None:
        return
    try:
        fcntl.flock(handle.fileno(), fcntl.LOCK_UN)
    except OSError:
        pass
    try:
        handle.close()
    except OSError:
        pass


def empty_state() -> dict[str, Any]:
    return {
        "version": STATE_VERSION,
        "phase": 0,
        "review_required": False,
        "delegation_required": False,
        "review_override": False,
        "delegation_override": False,
        "reject_reason_seen": False,
        "subagent_start_count": 0,
        "subagent_stop_count": 0,
        "followup_count": 0,
        "goal_required": False,
        "goal_contract_seen": False,
        "goal_progress_seen": False,
        "goal_verify_seen": False,
        "goal_blocker_seen": False,
        "goal_no_progress_count": 0,
    }


def _migrate_v1(raw: dict[str, Any]) -> dict[str, Any]:
    state = empty_state()
    state["review_required"] = bool(raw.get("review_required"))
    state["delegation_required"] = bool(raw.get("delegation_required"))
    state["review_override"] = bool(raw.get("review_override"))
    state["delegation_override"] = bool(raw.get("delegation_override"))
    state["reject_reason_seen"] = bool(raw.get("reject_reason_seen"))
    if raw.get("review_subagent_seen"):
        state["phase"] = 2
    elif state["review_required"] or state["delegation_required"]:
        state["phase"] = 1
    state["followup_count"] = int(raw.get("followup_count") or 0)
    return state


def load_state(event: dict[str, Any]) -> dict[str, Any]:
    path = state_path(event)
    try:
        text = path.read_text(encoding="utf-8")
    except FileNotFoundError:
        return {}
    except OSError:
        return {"__state_io_error": "state_read_failed"}
    try:
        raw = json.loads(text)
    except json.JSONDecodeError:
        return {"__state_io_error": "state_json_invalid"}
    if not isinstance(raw, dict):
        return {"__state_io_error": "state_not_object"}
    if int(raw.get("version") or 0) < STATE_VERSION:
        return _migrate_v1(raw)
    base = empty_state()
    for k, v in raw.items():
        if k.startswith("__"):
            continue
        base[k] = v
    base["version"] = STATE_VERSION
    return base


def save_state(event: dict[str, Any], state: dict[str, Any]) -> bool:
    """Atomic write via tmp + rename; returns True on success."""
    directory = state_dir(event)
    target = state_path(event)
    state.setdefault("version", STATE_VERSION)
    state["updated_at"] = datetime.now(timezone.utc).isoformat()
    payload = json.dumps(state, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    tmp: Path | None = None
    try:
        directory.mkdir(parents=True, exist_ok=True)
        tmp = directory / f".tmp-{os.getpid()}-{int(time.time() * 1e6)}-{target.name}"
        tmp.write_text(payload, encoding="utf-8")
        os.replace(tmp, target)
        return True
    except OSError:
        if tmp is not None:
            try:
                tmp.unlink()
            except OSError:
                pass
        return False


# ---------- Phase helpers ----------


def is_armed(state: dict[str, Any]) -> bool:
    return bool(state.get("review_required") or state.get("delegation_required"))


def is_overridden(state: dict[str, Any]) -> bool:
    return bool(state.get("review_override") or state.get("delegation_override"))


def is_satisfied(state: dict[str, Any]) -> bool:
    if not is_armed(state):
        return True
    if is_overridden(state):
        return True
    if state.get("reject_reason_seen"):
        return True
    return int(state.get("phase") or 0) >= 2


def goal_is_satisfied(state: dict[str, Any]) -> bool:
    if not state.get("goal_required"):
        return True
    if is_overridden(state) or state.get("reject_reason_seen"):
        return True
    return bool(
        state.get("goal_contract_seen")
        and state.get("goal_progress_seen")
        and (state.get("goal_verify_seen") or state.get("goal_blocker_seen"))
    )


def bump_phase(state: dict[str, Any], target: int) -> None:
    state["phase"] = max(int(state.get("phase") or 0), target)


# ---------- Event handlers ----------


def handle_before_submit(event: dict[str, Any]) -> int:
    lock_handle = acquire_state_lock(event)
    try:
        state = load_state(event)
        if not state or "__state_io_error" in state:
            state = empty_state()

        text = prompt_text(event)
        review = is_review_prompt(text)
        framework_entrypoint = is_framework_entrypoint_prompt(text)
        autopilot_entrypoint = is_autopilot_entrypoint_prompt(text)
        delegation = is_parallel_delegation_prompt(text) or framework_entrypoint
        review_override = has_review_override(text) or has_override(text)
        delegation_override = has_delegation_override(text) or has_override(text)
        rejected = saw_reject_reason(text)

        state["review_required"] = bool(state.get("review_required")) or review
        state["delegation_required"] = bool(state.get("delegation_required")) or delegation
        state["review_override"] = bool(state.get("review_override")) or review_override
        state["delegation_override"] = bool(state.get("delegation_override")) or delegation_override
        state["reject_reason_seen"] = bool(state.get("reject_reason_seen")) or rejected
        state["goal_required"] = bool(state.get("goal_required")) or autopilot_entrypoint
        state["goal_contract_seen"] = bool(state.get("goal_contract_seen")) or has_goal_contract_signal(
            text
        )
        state["goal_progress_seen"] = bool(state.get("goal_progress_seen")) or has_goal_progress_signal(
            text
        )
        state["goal_verify_seen"] = bool(state.get("goal_verify_seen")) or has_goal_verify_signal(text)
        state["goal_blocker_seen"] = bool(state.get("goal_blocker_seen")) or (
            has_goal_blocker_signal(text) or rejected
        )
        if review or delegation:
            state["last_prompt"] = text[:500]
        if is_armed(state) and not is_overridden(state) and not state.get("reject_reason_seen"):
            bump_phase(state, 1)

        persisted = save_state(event, state)
    finally:
        release_state_lock(lock_handle)

    output: dict[str, Any] = {"continue": True}
    needs_followup = (
        is_armed(state)
        and not is_overridden(state)
        and not state.get("reject_reason_seen")
        and int(state.get("phase") or 0) < 2
    )
    if needs_followup:
        if state.get("review_required"):
            output["followup_message"] = (
                "Broad/deep review detected. Spawn an independent reviewer subagent lane now; "
                "if you will not spawn, provide one explicit reject reason before finalizing."
            )
        else:
            output["followup_message"] = (
                "Parallel lane request detected. Spawn bounded subagent lanes now; "
                "if you will not spawn, provide one explicit reject reason before finalizing."
            )

    if not persisted:
        warning = (
            "Cursor review gate state could not be persisted under .cursor/hook-state. "
            "Review/delegation enforcement may be degraded for this turn."
        )
        output["followup_message"] = (
            f"{output.get('followup_message', '')} {warning}".strip()
        )

    print_json(output)
    return 0


def handle_subagent_start(event: dict[str, Any]) -> int:
    lock_handle = acquire_state_lock(event)
    try:
        state = load_state(event)
        if not state or "__state_io_error" in state:
            state = empty_state()
            armed = False
        else:
            armed = is_armed(state)
        mutated = False
        if armed:
            bump_phase(state, 2)
            state["subagent_start_count"] = int(state.get("subagent_start_count") or 0) + 1
            # Presence of real subagent evidence is sufficient; do not gate on
            # optional lane-intent metadata from model text heuristics.
            state["lane_intent_matches"] = True
            mutated = True
        sub_type = normalize_subagent_type(event.get("subagent_type"))
        agent_type = normalize_subagent_type(event.get("agent_type"))
        if armed and (sub_type or agent_type):
            state["last_subagent_type"] = sub_type or agent_type
            mutated = True
        if mutated:
            save_state(event, state)
    finally:
        release_state_lock(lock_handle)
    print_json({})
    return 0


def handle_subagent_stop(event: dict[str, Any]) -> int:
    lock_handle = acquire_state_lock(event)
    try:
        state = load_state(event)
        if not state or "__state_io_error" in state:
            state = empty_state()
            armed = False
        else:
            armed = is_armed(state)
        if armed:
            bump_phase(state, 3)
            state["subagent_stop_count"] = int(state.get("subagent_stop_count") or 0) + 1
            state["lane_intent_matches"] = True
            save_state(event, state)
    finally:
        release_state_lock(lock_handle)
    print_json({})
    return 0


def handle_post_tool_use(event: dict[str, Any]) -> int:
    lock_handle = acquire_state_lock(event)
    try:
        state = load_state(event)
        if not state or "__state_io_error" in state:
            state = empty_state()
            armed = False
        else:
            armed = is_armed(state)
        name = normalize_tool_name(tool_name_of(event))
        tool_input = tool_input_of(event)
        sub_type = normalize_subagent_type(
            tool_input.get("subagent_type")
            or tool_input.get("subagentType")
            or event.get("subagent_type")
            or event.get("subagentType")
        )
        agent_type = normalize_subagent_type(
            tool_input.get("agent_type")
            or tool_input.get("agentType")
            or event.get("agent_type")
            or event.get("agentType")
        )
        typed_subagent = bool(
            (sub_type and sub_type in SUBAGENT_TYPES) or (agent_type and agent_type in SUBAGENT_TYPES)
        )
        if name in SUBAGENT_TOOL_NAMES and typed_subagent and armed:
            bump_phase(state, 2)
            state["subagent_start_count"] = int(state.get("subagent_start_count") or 0) + 1
            state["last_subagent_tool"] = name
            if sub_type or agent_type:
                state["last_subagent_type"] = sub_type or agent_type
            state["lane_intent_matches"] = True
            save_state(event, state)
    finally:
        release_state_lock(lock_handle)
    print_json({})
    return 0


def handle_after_agent_response(event: dict[str, Any]) -> int:
    lock_handle = acquire_state_lock(event)
    try:
        state = load_state(event)
        if not state or "__state_io_error" in state:
            state = empty_state()
            armed = False
        else:
            armed = is_armed(state)
        text = agent_response_text(event)
        if armed and saw_reject_reason(text):
            state["reject_reason_seen"] = True
            state["goal_blocker_seen"] = True
        if armed and has_goal_contract_signal(text):
            state["goal_contract_seen"] = True
        if armed and has_goal_progress_signal(text):
            state["goal_progress_seen"] = True
        if armed and has_goal_verify_signal(text):
            state["goal_verify_seen"] = True
        if armed and has_goal_blocker_signal(text):
            state["goal_blocker_seen"] = True
        if armed and (
            state.get("reject_reason_seen")
            or state.get("goal_contract_seen")
            or state.get("goal_progress_seen")
            or state.get("goal_verify_seen")
            or state.get("goal_blocker_seen")
        ):
            save_state(event, state)
    finally:
        release_state_lock(lock_handle)
    print_json({})
    return 0


def _goal_followup_message() -> str:
    return (
        "Autopilot goal mode requires completion evidence before closeout. Provide: "
        "1) goal contract (Goal/Done when/Validation commands), "
        "2) checkpoint progress + next step, and "
        "3) verification result or explicit blocker."
    )


def _goal_has_advance_signal(state: dict[str, Any]) -> bool:
    return bool(state.get("goal_verify_seen") or state.get("goal_blocker_seen"))


def _goal_execution_template(state: dict[str, Any]) -> str:
    missing = _goal_missing_parts(state)
    next_line = (
        "Next action template:\n"
        "- Goal: <one objective>\n"
        "- Done when: <measurable condition>\n"
        "- Validation commands: <exact command list>\n"
        "- Checkpoint progress: <what changed> / Next step: <immediate action>\n"
        "- Verification: <passed output> OR blocker: <typed blocker>\n"
    )
    if missing:
        return f"{next_line}- Fill missing now: {', '.join(missing)}"
    return next_line


def _goal_missing_parts(state: dict[str, Any]) -> list[str]:
    missing: list[str] = []
    if not state.get("goal_contract_seen"):
        missing.append("goal_contract")
    if not state.get("goal_progress_seen"):
        missing.append("checkpoint_progress")
    if not state.get("goal_verify_seen") and not state.get("goal_blocker_seen"):
        missing.append("verification_or_blocker")
    return missing


def handle_stop(event: dict[str, Any]) -> int:
    lock_handle = acquire_state_lock(event)
    try:
        state = load_state(event)
        text = prompt_text(event)
        inferred_required = (
            is_review_prompt(text)
            or is_parallel_delegation_prompt(text)
            or is_framework_entrypoint_prompt(text)
        )
        inferred_overridden = (
            has_review_override(text)
            or has_delegation_override(text)
            or has_override(text)
            or saw_reject_reason(text)
        )
        loop_count = loop_count_of(event)

        if not state:
            if inferred_required and not inferred_overridden:
                print_json(
                    {
                        "followup_message": (
                            "Cursor review gate state is missing under .cursor/hook-state, so "
                            "enforcement cannot be verified for this turn. Re-run with subagent "
                            "lanes or an explicit reject reason before finalizing."
                        )
                    }
                )
            else:
                print_json({})
            return 0

        if "__state_io_error" in state:
            print_json(
                {
                    "followup_message": (
                        "Cursor review gate state is unreadable under .cursor/hook-state "
                        f"({state['__state_io_error']}). Enforcement degraded; check "
                        "hook-state permissions and JSON integrity."
                    )
                }
            )
            return 0

        if has_review_override(text) or has_override(text):
            state["review_override"] = True
        if has_delegation_override(text) or has_override(text):
            state["delegation_override"] = True
        if saw_reject_reason(text):
            state["reject_reason_seen"] = True
            state["goal_blocker_seen"] = True
        if has_goal_contract_signal(text):
            state["goal_contract_seen"] = True
        if has_goal_progress_signal(text):
            state["goal_progress_seen"] = True
        if has_goal_verify_signal(text):
            state["goal_verify_seen"] = True
        if has_goal_blocker_signal(text):
            state["goal_blocker_seen"] = True
        if _goal_has_advance_signal(state):
            state["goal_no_progress_count"] = 0

        if not is_satisfied(state):
            state["followup_count"] = int(state.get("followup_count") or 0) + 1
            save_state(event, state)
            escalation = (
                "This has already looped multiple times; do not silently continue. "
                if loop_count >= 3 or int(state.get("followup_count") or 0) >= 3
                else ""
            )
            print_json(
                {
                    "followup_message": (
                        "Broad/deep review (or independent parallel lanes) was requested, but no "
                        "independent subagent/sidecar was observed. Spawn a suitable subagent lane "
                        "now, or explicitly state why spawning is rejected. " + escalation
                    ).strip()
                }
            )
            return 0

        if not goal_is_satisfied(state):
            missing = ", ".join(_goal_missing_parts(state))
            state["goal_no_progress_count"] = int(state.get("goal_no_progress_count") or 0) + 1
            state["followup_count"] = int(state.get("followup_count") or 0) + 1
            save_state(event, state)
            no_progress = int(state.get("goal_no_progress_count") or 0)
            escalation = (
                " Autopilot appears stalled; stop summarizing and execute the next checkpoint now."
                if no_progress >= MAX_GOAL_NO_PROGRESS_LOOPS
                else ""
            )
            print_json(
                {
                    "followup_message": (
                        f"{_goal_followup_message()} Missing: {missing}. "
                        f"No-progress loops: {no_progress}. "
                        "If no blocker exists, continue execution now and report the next verified checkpoint."
                        + escalation
                        + "\n"
                        + _goal_execution_template(state)
                    ),
                }
            )
            return 0

        save_state(event, empty_state())
        print_json({})
        return 0
    finally:
        release_state_lock(lock_handle)


def handle_pre_compact(event: dict[str, Any]) -> int:
    state = load_state(event)
    if not state or "__state_io_error" in state:
        print_json({})
        return 0
    summary = (
        f"Cursor review gate state (preserved across compaction): "
        f"phase={int(state.get('phase') or 0)} "
        f"review_required={bool(state.get('review_required'))} "
        f"delegation_required={bool(state.get('delegation_required'))} "
        f"override={is_overridden(state)} "
        f"rejected={bool(state.get('reject_reason_seen'))} "
        f"subagent_starts={int(state.get('subagent_start_count') or 0)} "
        f"subagent_stops={int(state.get('subagent_stop_count') or 0)}"
    )
    print_json({"additional_context": summary})
    return 0


def handle_session_end(event: dict[str, Any]) -> int:
    try:
        state_path(event).unlink(missing_ok=True)
    except OSError:
        pass
    print_json({})
    return 0


# ---------- Main ----------


_DISPATCH = {
    "beforesubmitprompt": handle_before_submit,
    "userpromptsubmit": handle_before_submit,
    "subagentstart": handle_subagent_start,
    "subagentstop": handle_subagent_stop,
    "posttooluse": handle_post_tool_use,
    "afteragentresponse": handle_after_agent_response,
    "stop": handle_stop,
    "precompact": handle_pre_compact,
    "sessionend": handle_session_end,
}


def main() -> int:
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--event", default="", help="Cursor hook event name")
    parser.add_argument("--help", action="help")
    args, _unknown = parser.parse_known_args()

    event = read_event()
    if "__stdin_io_error" in event:
        print_json({})
        return 0

    name = (args.event or event.get("hook_event_name") or "").strip().lower()
    handler = _DISPATCH.get(name)
    if handler is None:
        print_json({})
        return 0
    try:
        return handler(event)
    except Exception:
        print_json({})
        return 0


if __name__ == "__main__":
    raise SystemExit(main())
