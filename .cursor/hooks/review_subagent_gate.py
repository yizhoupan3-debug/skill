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

PARALLEL_DELEGATION_PATTERNS = [
    re.compile(r"(并行|同时|分头|分路|分三路|多路|多线).*(前端|后端|测试|API|数据库|UI|安全|性能|架构|实现|策略|验证|模块|方向)", re.I),
    re.compile(r"(前端|后端|测试|API|数据库|UI|安全|性能|架构|实现|策略|验证).*(并行|同时|分头|分路|分三路|多路|多线)", re.I),
    re.compile(r"(多个|多条|多路|多维|多方向|独立).*(假设|模块|方向|维度|lane|lanes)", re.I),
    re.compile(r"\b(parallel|concurrent|in parallel|split lanes|split work)\b.*\b(frontend|backend|test|testing|database|security|performance|architecture|implementation|verification)\b", re.I),
]

OVERRIDE_PATTERNS = [
    re.compile(r"do not use (a )?subagent", re.I),
    re.compile(r"without (a )?subagent", re.I),
    re.compile(r"不要.*subagent", re.I),
    re.compile(r"不用.*subagent", re.I),
    re.compile(r"不要.*子代理", re.I),
    re.compile(r"不用.*子代理", re.I),
]


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
    return any(p.search(text or "") for p in REVIEW_PATTERNS)


def is_parallel_delegation_prompt(text: str) -> bool:
    return any(p.search(text or "") for p in PARALLEL_DELEGATION_PATTERNS)


def session_key(event: dict[str, Any]) -> str:
    raw = str(
        event.get("session_id")
        or event.get("conversation_id")
        or event.get("thread_id")
        or event.get("cwd")
        or "default"
    )
    return hashlib.sha256(raw.encode("utf-8")).hexdigest()[:32]


def repo_root(event: dict[str, Any]) -> Path:
    cwd = event.get("cwd")
    if isinstance(cwd, str) and cwd.strip():
        return Path(cwd)
    return Path(os.getcwd())


def state_dir(event: dict[str, Any]) -> Path:
    return repo_root(event) / ".cursor" / "hook-state"


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


def print_json(payload: dict[str, Any]) -> None:
    sys.stdout.write(json.dumps(payload, ensure_ascii=False))


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
    save_state(event, state)
    # Do not block prompt submission; just record state.
    print_json({"continue": True})
    return 0


def handle_subagent_start(event: dict[str, Any]) -> int:
    state = load_state(event)
    state["review_subagent_seen"] = True
    save_state(event, state)
    print_json({})
    return 0


def handle_stop(event: dict[str, Any]) -> int:
    state = load_state(event)
    loop_count = event.get("loop_count")
    loop_count = int(loop_count) if isinstance(loop_count, int) else 0
    required = bool(state.get("review_required") or state.get("delegation_required"))
    overridden = bool(state.get("review_override") or state.get("delegation_override"))
    seen = bool(state.get("review_subagent_seen"))
    if required and not overridden and not seen and loop_count < 5:
        print_json(
            {
                "followup_message": (
                    "Broad/deep review (or independent parallel lanes) was requested, but no "
                    "independent subagent/sidecar was observed. Spawn a suitable subagent lane now, "
                    "or explicitly state why spawning is rejected."
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

