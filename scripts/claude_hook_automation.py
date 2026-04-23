#!/usr/bin/env python3
"""Lightweight automation hooks that nudge Claude toward better code quality."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

CODING_HINT_TOKENS = (
    "代码",
    "实现",
    "优化",
    "加速",
    "内存",
    "hook",
    "agent.md",
    "agent",
    "hook",
    "fix",
    "bug",
    "refactor",
    "optimize",
    "implement",
    "performance",
    "memory",
    "runtime",
)
QUALITY_TARGET_PREFIXES = (
    "codex_agno_runtime/src/",
    "scripts/",
    "tests/",
)
QUALITY_TARGET_SUFFIXES = (".py", ".rs", ".sh")
COMMON_CONTEXT = (
    "实现倾向：优先直接实现目标行为，不要默认叠加兼容层、补丁式分支、"
    "兜底开关或 keep-old-and-add-new。顺手检查热路径上的重复 I/O、"
    "重复序列化、无谓 clone/临时对象，以及还能删掉的过渡逻辑。"
)
RUST_CONTEXT = "Rust 额外检查：盯住热循环里的分配、clone、String/Vec 复制和 serde_json 往返。"
PYTHON_CONTEXT = "Python 额外检查：盯住重复解析/序列化、重复读文件、wrapper-on-wrapper 和兼容别名链。"
HOOK_CONTEXT = "Hook 额外检查：让 hook 做增量自动化，不要只做阻拦；优先短上下文、窄验证、自动收口。"


def _read_payload() -> dict[str, Any]:
    raw = sys.stdin.read().strip()
    if not raw:
        return {}
    try:
        payload = json.loads(raw)
    except json.JSONDecodeError:
        return {"raw": raw}
    return payload if isinstance(payload, dict) else {"payload": payload}


def _normalize_path(value: Any) -> str:
    if not isinstance(value, str) or not value:
        return ""
    return value.replace("\\", "/")


def _iter_candidate_paths(payload: dict[str, Any]) -> list[str]:
    candidates: list[str] = []
    for key in ("file_path", "path", "target_path", "changed_path"):
        normalized = _normalize_path(payload.get(key))
        if normalized:
            candidates.append(normalized)
    return candidates


def _iter_payload_paths(payload: dict[str, Any]) -> list[str]:
    candidates = list(_iter_candidate_paths(payload))
    tool_input = payload.get("tool_input")
    if isinstance(tool_input, dict):
        candidates.extend(_iter_candidate_paths(tool_input))
    return candidates


def _relative_candidate(path: str, repo_root: Path) -> str:
    candidate = Path(path)
    if candidate.is_absolute():
        try:
            return candidate.resolve().relative_to(repo_root.resolve()).as_posix()
        except ValueError:
            return candidate.as_posix()
    return candidate.as_posix()


def _extract_prompt_text(payload: dict[str, Any]) -> str:
    candidates: list[str] = []
    for key in ("prompt", "user_prompt", "text", "message"):
        value = payload.get(key)
        if isinstance(value, str):
            candidates.append(value)
    if isinstance(payload.get("input"), str):
        candidates.append(payload["input"])
    if isinstance(payload.get("payload"), dict):
        nested = payload["payload"]
        for key in ("prompt", "user_prompt", "text", "message"):
            value = nested.get(key)
            if isinstance(value, str):
                candidates.append(value)
    return "\n".join(text for text in candidates if text).strip()


def _looks_like_coding_request(prompt_text: str) -> bool:
    lowered = prompt_text.lower()
    return any(token in prompt_text or token in lowered for token in CODING_HINT_TOKENS)


def run_user_prompt_submit(_repo_root: Path, payload: dict[str, Any]) -> int:
    prompt_text = _extract_prompt_text(payload)
    if not prompt_text or not _looks_like_coding_request(prompt_text):
        return 0
    print(
        "这轮如果涉及写代码：默认直接实现，不要先上兼容层、补丁式保底或宽泛兜底；"
        "优先顺手看速度、内存、重复 I/O/重复序列化，以及还能删除的临时过渡逻辑。"
    )
    return 0


def _quality_target_context(path: str) -> str | None:
    if not any(path.startswith(prefix) for prefix in QUALITY_TARGET_PREFIXES):
        return None
    if not path.endswith(QUALITY_TARGET_SUFFIXES):
        return None
    parts = [COMMON_CONTEXT]
    if path.endswith(".rs"):
        parts.append(RUST_CONTEXT)
    if path.endswith(".py"):
        parts.append(PYTHON_CONTEXT)
    if "hook" in path or path.endswith(".sh"):
        parts.append(HOOK_CONTEXT)
    return " ".join(parts)


def run_pre_tool_use_quality(repo_root: Path, payload: dict[str, Any]) -> int:
    tool_name = payload.get("tool_name")
    if tool_name not in {"Edit", "MultiEdit", "Write"}:
        return 0
    rel_paths = {
        _relative_candidate(path, repo_root)
        for path in _iter_payload_paths(payload)
    }
    for path in sorted(rel_paths):
        context = _quality_target_context(path)
        if not context:
            continue
        print(
            json.dumps(
                {
                    "hookSpecificOutput": {
                        "hookEventName": "PreToolUse",
                        "permissionDecision": "allow",
                        "permissionDecisionReason": "Apply repo implementation-quality defaults.",
                        "additionalContext": context,
                    }
                },
                ensure_ascii=False,
            )
        )
        return 0
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Emit lightweight automation context for Claude hooks.")
    parser.add_argument("command", choices=("user-prompt-submit", "pre-tool-use-quality"))
    parser.add_argument("--repo-root", required=True)
    args = parser.parse_args()

    repo_root = Path(args.repo_root).resolve()
    payload = _read_payload()

    if args.command == "user-prompt-submit":
        return run_user_prompt_submit(repo_root, payload)
    return run_pre_tool_use_quality(repo_root, payload)


if __name__ == "__main__":
    raise SystemExit(main())
