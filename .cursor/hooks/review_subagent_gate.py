#!/usr/bin/env python3
"""Cursor hook: require sidecar/subagent when broad review is requested.

This is Cursor-specific glue:
- Cursor Agent hooks use events like beforeSubmitPrompt, subagentStart, stop.
- stop hooks cannot hard-block completion; they can auto-submit followup_message.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
from pathlib import Path
from typing import Any


REVIEW_PATTERNS = [
    re.compile(r"\b(code|security|architecture|architect)\s+review\b", re.I),
    re.compile(r"\breview\s+this\s+(pr|pull request)\b", re.I),
    re.compile(r"\breview\s+(my\s+)?(pr|pull request)\b", re.I),
    re.compile(r"\b(pr|pull request)\s+review\b", re.I),
    re.compile(r"\breview\s+(code|security|architecture)\b", re.I),
    re.compile(r"^\s*review\b.*\bagain\b", re.I),
    re.compile(r"\bfocus on finding\b.*\bproblems\b", re.I),
    re.compile(r"(深度|全面|全仓|仓库级|跨模块|多模块|多维)\s*review", re.I),
    re.compile(
        r"review.*(仓库|全仓|跨模块|多模块|严重程度|findings|severity|repo|repository|cross[- ]module|回归风险|架构风险|实现质量|路由系统|skill\s*边界)",
        re.I,
    ),
    re.compile(r"(深度|全面|全仓|仓库级|跨模块|多模块|多维).*(审查|审核|审计|评审)", re.I),
    re.compile(
        r"(审查|审核|审计|评审).*(仓库|全仓|跨模块|多模块|严重程度|回归风险|架构风险|实现质量|路由系统|skill\s*边界)",
        re.I,
    ),
    re.compile(r"(代码审查|安全审查|架构审查|审查这个\s*PR|审查这段代码)", re.I),
    re.compile(r"(审查|评审|审核).*(PR|pull request|合并请求)", re.I),
]

PARALLEL_DELEGATION_PATTERNS = [
    re.compile(r"(并行|同时|分头|分路|分三路|多路|多线).*(前端|后端|测试|API|数据库|UI|安全|性能|架构|实现|策略|验证|模块|方向)", re.I),
    re.compile(r"(前端|后端|测试|API|数据库|UI|安全|性能|架构|实现|策略|验证).*(并行|同时|分头|分路|分三路|多路|多线)", re.I),
    re.compile(r"(多个|多条|多路|多维|多方向|独立).*(假设|模块|方向|维度|lane|lanes)", re.I),
    re.compile(r"\b(parallel|concurrent|in parallel|split lanes|split work)\b.*\b(frontend|backend|test|testing|database|security|performance|architecture|implementation|verification)\b", re.I),
    re.compile(r"\b(parallel|concurrent|in parallel|split lanes?|independent lanes?)\b", re.I),
    re.compile(r"(并行|分路|分头|独立).*(lane|路线|路)", re.I),
]

OVERRIDE_PATTERNS = [
    re.compile(r"do not use (a )?subagent", re.I),
    re.compile(r"without (a )?subagent", re.I),
    re.compile(r"handle (this|it) locally", re.I),
    re.compile(r"do it yourself", re.I),
    re.compile(r"no (parallel|delegation|delegating|split)", re.I),
    re.compile(r"不要.*subagent", re.I),
    re.compile(r"不用.*subagent", re.I),
    re.compile(r"不要.*子代理", re.I),
    re.compile(r"不用.*子代理", re.I),
    re.compile(r"(你|你自己).*(本地处理|直接处理|自己做)", re.I),
    re.compile(r"(不要|不用).*(分工|并行|分路|分头)", re.I),
]

REJECT_REASONS = {
    "small_task",
    "shared_context_heavy",
    "write_scope_overlap",
    "next_step_blocked",
    "verification_missing",
    "token_overhead_dominates",
}
REJECT_REASON_PATTERNS = [
    re.compile(rf"(?<![a-z0-9_]){re.escape(reason)}(?![a-z0-9_])", re.I)
    for reason in REJECT_REASONS
]
SUBAGENT_TYPES = {
    "generalpurpose",
    "explore",
    "shell",
    "browser-use",
    "cursor-guide",
    "ci-investigator",
    "best-of-n-runner",
    "explorer",  # legacy alias
}


def read_event() -> dict[str, Any]:
    raw = sys.stdin.read()
    if not raw.strip():
        return {}
    try:
        payload = json.loads(raw)
    except json.JSONDecodeError:
        return {}
    return payload if isinstance(payload, dict) else {}


def prompt_text(event: dict[str, Any]) -> str:
    for key in ("prompt", "user_prompt", "message", "input", "text"):
        value = event.get(key)
        if isinstance(value, str):
            return value
    return ""


def has_override(text: str) -> bool:
    return any(p.search(text or "") for p in OVERRIDE_PATTERNS)


def is_review_prompt(text: str) -> bool:
    value = text or ""
    if is_narrow_review_prompt(value):
        return False
    return any(p.search(value) for p in REVIEW_PATTERNS)


def is_parallel_delegation_prompt(text: str) -> bool:
    return any(p.search(text or "") for p in PARALLEL_DELEGATION_PATTERNS)


def is_narrow_review_prompt(text: str) -> bool:
    value = text or ""
    if not re.search(r"\breview\b", value, re.I):
        return False
    if re.search(r"\b(pr|pull request)\b", value, re.I):
        return False
    if re.search(r"(深度|全面|全仓|跨模块|多模块|多维|架构|安全|回归风险|严重程度|findings)", value, re.I):
        return False
    return bool(re.search(r"^\s*review\s+(/|\.|[A-Za-z0-9_-].*\.(md|rs|tsx?|jsx?|py|json|toml))", value, re.I))


def session_key(event: dict[str, Any]) -> str:
    raw = str(
        event.get("session_id")
        or event.get("conversation_id")
        or event.get("thread_id")
        or event.get("cwd")
        or "default"
    )
    return hashlib.sha256(raw.encode("utf-8")).hexdigest()[:32]


def _candidate_paths(event: dict[str, Any]) -> list[Path]:
    candidates: list[Path] = []
    cwd = event.get("cwd")
    if isinstance(cwd, str) and cwd.strip():
        candidates.append(Path(cwd).resolve())
    candidates.append(Path(os.getcwd()).resolve())
    candidates.append(Path(__file__).resolve().parents[2])
    return candidates


def repo_root(event: dict[str, Any]) -> Path:
    for candidate in _candidate_paths(event):
        for probe in [candidate, *candidate.parents]:
            if (probe / ".git").exists() and (probe / ".cursor").is_dir():
                return probe
            if (probe / ".cursor" / "hooks" / "review_subagent_gate.py").exists():
                return probe
    return _candidate_paths(event)[0]


def state_dir(event: dict[str, Any]) -> Path:
    return repo_root(event) / ".cursor" / "hook-state"


def state_path(event: dict[str, Any]) -> Path:
    return state_dir(event) / f"review-subagent-{session_key(event)}.json"


def load_state(event: dict[str, Any]) -> dict[str, Any]:
    path = state_path(event)
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return {}
    except json.JSONDecodeError:
        return {"__state_io_error": "state_json_invalid"}
    except OSError:
        return {"__state_io_error": "state_read_failed"}
    return data if isinstance(data, dict) else {}


def save_state(event: dict[str, Any], state: dict[str, Any]) -> bool:
    directory = state_dir(event)
    try:
        directory.mkdir(parents=True, exist_ok=True)
        state_path(event).write_text(
            json.dumps(state, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        return True
    except OSError:
        return False


def print_json(payload: dict[str, Any]) -> None:
    sys.stdout.write(json.dumps(payload, ensure_ascii=False))


def saw_reject_reason(text: str) -> bool:
    return any(pattern.search(text or "") for pattern in REJECT_REASON_PATTERNS)


def normalize_subagent_type(value: Any) -> str:
    if not isinstance(value, str):
        return ""
    return value.strip().lower()


def handle_before_submit(event: dict[str, Any]) -> int:
    text = prompt_text(event)
    state: dict[str, Any] = {"seq": 0}
    if is_review_prompt(text):
        state["review_required"] = True
        state["prompt"] = text[:500]
    if is_parallel_delegation_prompt(text):
        state["delegation_required"] = True
        state["prompt"] = text[:500]
    if has_override(text):
        state["review_override"] = True
        state["delegation_override"] = True
    if saw_reject_reason(text):
        state["reject_reason_seen"] = True
    persisted = save_state(event, state)
    # Do not block prompt submission; just record state.
    output: dict[str, Any] = {"continue": True}
    if state.get("review_required") and not state.get("review_override") and not state.get("reject_reason_seen"):
        output["followup_message"] = (
            "Broad/deep review detected. Spawn an independent reviewer subagent lane now; "
            "if you will not spawn, provide one explicit reject reason before finalizing."
        )
    elif state.get("delegation_required") and not state.get("delegation_override") and not state.get("reject_reason_seen"):
        output["followup_message"] = (
            "Parallel lane request detected. Spawn bounded subagent lanes now; "
            "if you will not spawn, provide one explicit reject reason before finalizing."
        )
    if not persisted:
        state_warning = (
            "Cursor review gate state could not be persisted under .cursor/hook-state. "
            "Review/delegation enforcement may be degraded for this turn."
        )
        output["followup_message"] = (
            f"{output.get('followup_message')} {state_warning}".strip()
            if output.get("followup_message")
            else state_warning
        )
    print_json(output)
    return 0


def handle_subagent_start(event: dict[str, Any]) -> int:
    state = load_state(event)
    if state.get("review_required") or state.get("delegation_required"):
        subagent_type = event.get("subagent_type")
        agent_type = event.get("agent_type")
        if normalize_subagent_type(subagent_type) in SUBAGENT_TYPES:
            state["review_subagent_seen"] = True
        elif normalize_subagent_type(agent_type) in SUBAGENT_TYPES:
            state["review_subagent_seen"] = True
    save_state(event, state)
    print_json({})
    return 0


def handle_stop(event: dict[str, Any]) -> int:
    state = load_state(event)
    text = prompt_text(event)
    inferred_required = is_review_prompt(text) or is_parallel_delegation_prompt(text)
    inferred_overridden = has_override(text) or saw_reject_reason(text)
    loop_count = event.get("loop_count")
    try:
        loop_count = int(loop_count)
    except (TypeError, ValueError):
        loop_count = 0
    required = bool(state.get("review_required") or state.get("delegation_required"))
    overridden = bool(state.get("review_override") or state.get("delegation_override"))
    rejected = bool(state.get("reject_reason_seen"))
    seen = bool(state.get("review_subagent_seen"))
    state_io_error = state.get("__state_io_error")
    if not state:
        if inferred_required and not inferred_overridden:
            print_json(
                {
                    "followup_message": (
                        "Cursor review gate state is missing under .cursor/hook-state, so enforcement cannot "
                        "be verified for this turn. Re-run with subagent lanes or an explicit reject reason "
                        "before finalizing."
                    )
                }
            )
        else:
            print_json({})
        return 0
    if state_io_error:
        print_json(
            {
                "followup_message": (
                    "Cursor review gate state is unreadable or unavailable under .cursor/hook-state "
                    f"({state_io_error}). Enforcement may be degraded; please verify hook-state "
                    "permissions and JSON integrity."
                )
            }
        )
        return 0
    if required and not overridden and not rejected and not seen:
        state["followup_count"] = int(state.get("followup_count", 0)) + 1
        save_state(event, state)
        escalation = (
            "This has already looped multiple times; do not silently continue. "
            if loop_count >= 5
            else ""
        )
        print_json(
            {
                "followup_message": (
                    "Broad/deep review (or independent parallel lanes) was requested, but no "
                    "independent subagent/sidecar was observed. Spawn a suitable subagent lane now, "
                    "or explicitly state why spawning is rejected. "
                    + escalation
                )
            }
        )
        return 0
    if state:
        save_state(event, {"seq": 0})
    print_json({})
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--event", default="", help="Cursor hook event name")
    args = parser.parse_args()

    event = read_event()
    name = (args.event or "").strip().lower()

    if name == "beforesubmitprompt":
        return handle_before_submit(event)
    if name == "subagentstart":
        return handle_subagent_start(event)
    if name == "stop":
        return handle_stop(event)
    print_json({})
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

