#!/usr/bin/env python3
"""Require independent subagent review for broad/deep review prompts."""

from __future__ import annotations

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
    re.compile(r"^\s*review\s+(/|\.|[A-Za-z0-9_-].*\.(md|rs|tsx?|jsx?|py|json|toml))", re.I),
    re.compile(r"^\s*review\b.*\bagain\b", re.I),
    re.compile(r"\bfocus on finding\b.*\bproblems\b", re.I),
    re.compile(r"(深度|全面|全仓|仓库级|跨模块|多模块|多维)\s*review", re.I),
    re.compile(
        r"review.*(仓库|全仓|跨模块|多模块|严重程度|findings|回归风险|架构风险|实现质量|路由系统|skill\s*边界)",
        re.I,
    ),
    re.compile(r"(深度|全面|全仓|仓库级|跨模块|多模块|多维).*(审查|审核|审计|评审)", re.I),
    re.compile(
        r"(审查|审核|审计|评审).*(仓库|全仓|跨模块|多模块|严重程度|回归风险|架构风险|实现质量|路由系统|skill\s*边界)",
        re.I,
    ),
    re.compile(r"(代码审查|安全审查|架构审查|审查这个\s*PR|审查这段代码)", re.I),
]

OVERRIDE_PATTERNS = [
    re.compile(r"do not use (a )?subagent", re.I),
    re.compile(r"without (a )?subagent", re.I),
    re.compile(r"不要.*subagent", re.I),
    re.compile(r"不用.*subagent", re.I),
    re.compile(r"不要.*子代理", re.I),
    re.compile(r"不用.*子代理", re.I),
]

SUBAGENT_EVIDENCE_TERMS = (
    "spawn_agent",
    "agent",
    "task",
    "subagent",
    "code-reviewer",
    "reviewer",
    "security",
    "plan",
    "general-purpose",
)


def read_event() -> dict[str, Any]:
    try:
        payload = json.load(sys.stdin)
    except json.JSONDecodeError:
        return {}
    return payload if isinstance(payload, dict) else {}


def event_name(event: dict[str, Any]) -> str:
    return str(event.get("hook_event_name") or event.get("event") or "").lower()


def prompt_text(event: dict[str, Any]) -> str:
    for key in ("prompt", "user_prompt", "message", "input"):
        value = event.get(key)
        if isinstance(value, str):
            return value
    return ""


def session_key(event: dict[str, Any]) -> str:
    raw = str(event.get("session_id") or event.get("conversation_id") or event.get("cwd") or "default")
    return hashlib.sha256(raw.encode("utf-8")).hexdigest()[:32]


def state_dir(event: dict[str, Any]) -> Path:
    repo = Path(str(event.get("cwd") or os.getcwd()))
    return repo / ".codex" / "hook-state"


def state_path(event: dict[str, Any]) -> Path:
    return state_dir(event) / f"review-subagent-{session_key(event)}.json"


def load_state(event: dict[str, Any]) -> dict[str, Any]:
    path = state_path(event)
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (FileNotFoundError, json.JSONDecodeError):
        return {}
    return data if isinstance(data, dict) else {}


def save_state(event: dict[str, Any], state: dict[str, Any]) -> None:
    directory = state_dir(event)
    directory.mkdir(parents=True, exist_ok=True)
    state_path(event).write_text(json.dumps(state, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def is_review_prompt(text: str) -> bool:
    return any(pattern.search(text or "") for pattern in REVIEW_PATTERNS)


def has_override(text: str) -> bool:
    return any(pattern.search(text or "") for pattern in OVERRIDE_PATTERNS)


def tool_name(event: dict[str, Any]) -> str:
    return str(event.get("tool_name") or event.get("tool") or event.get("name") or "")


def tool_input(event: dict[str, Any]) -> dict[str, Any]:
    value = event.get("tool_input") or event.get("input") or event.get("arguments") or {}
    return value if isinstance(value, dict) else {}


def saw_subagent(event: dict[str, Any]) -> bool:
    data = tool_input(event)
    joined = " ".join(
        str(data.get(key, ""))
        for key in ("subagent_type", "agent_type", "agentType", "description", "prompt", "message", "name")
    )
    evidence = f"{tool_name(event)} {joined}".lower()
    return any(term in evidence for term in SUBAGENT_EVIDENCE_TERMS)


def print_json(payload: dict[str, Any]) -> None:
    print(json.dumps(payload, ensure_ascii=False))


def handle_prompt(event: dict[str, Any]) -> int:
    text = prompt_text(event)
    state: dict[str, Any] = {"seq": 0}
    if is_review_prompt(text):
        state["review_required"] = True
        state["prompt"] = text[:500]
    if has_override(text):
        state["review_override"] = True
    save_state(event, state)
    if state.get("review_required") and not state.get("review_override"):
        print_json(
            {
                "hookSpecificOutput": {
                    "hookEventName": "UserPromptSubmit",
                    "additionalContext": (
                        "Broad/deep review detected. Before finalizing, run independent reviewer "
                        "subagent lanes when the scope can be split; if spawning is not appropriate, "
                        "state the explicit reject reason."
                    ),
                }
            }
        )
    return 0


def handle_post_tool(event: dict[str, Any]) -> int:
    if saw_subagent(event):
        state = load_state(event)
        state["review_subagent_seen"] = True
        state["review_subagent_tool"] = tool_name(event)
        save_state(event, state)
    return 0


def handle_stop(event: dict[str, Any]) -> int:
    if event.get("stop_hook_active") or event.get("stopHookActive"):
        return 0
    state = load_state(event)
    if state.get("review_required") and not state.get("review_override") and not state.get("review_subagent_seen"):
        print_json(
            {
                "decision": "block",
                "reason": (
                    "Broad/deep review was requested, but no independent subagent review was observed. "
                    "Spawn suitable reviewer sidecars now, or explicitly record why spawning is rejected."
                ),
            }
        )
        return 0
    if state:
        save_state(event, {"seq": 0})
    return 0


def main() -> int:
    event = read_event()
    name = event_name(event)
    if name == "userpromptsubmit":
        return handle_prompt(event)
    if name == "posttooluse":
        return handle_post_tool(event)
    if name == "stop":
        return handle_stop(event)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
