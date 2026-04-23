#!/usr/bin/env python3
"""Lightweight automation hooks that nudge Claude toward better code quality."""

from __future__ import annotations

import argparse
import difflib
import hashlib
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

PROMPT_PATH_RE = re.compile(r"(?<!\w)(?:[\w./-]+\.(?:py|rs|sh|json|md))(?!\w)", re.IGNORECASE)
CODE_PATH_RE = re.compile(r"(?<!\w)(?:[\w./-]+\.(?:py|rs|sh))(?!\w)", re.IGNORECASE)
STRONG_ACTION_TERMS = (
    "修复",
    "实现",
    "优化",
    "重构",
    "加速",
    "删掉",
    "去掉",
    "移除",
    "清理",
    "替换",
    "落实",
    "补齐",
    "增强",
    "改代码",
    "fix",
    "implement",
    "optimize",
    "refactor",
    "speed up",
    "remove",
    "rewrite",
)
WEAK_ACTION_TERMS = (
    "改",
    "写",
    "调",
    "修",
    "做",
    "update",
    "change",
    "improve",
)
CODE_TARGET_TERMS = (
    "代码",
    "runtime",
    "hook",
    "hooks",
    "agent.md",
    "claude.md",
    "脚本",
    "router",
    "路由",
    "内存",
    "性能",
    "热区",
    "热路径",
    "保底",
    "补丁",
    "fallback",
    "shim",
    "wrapper",
    "patch",
    "兼容层",
)
NON_CODE_EDIT_TERMS = (
    "结论",
    "措辞",
    "翻译",
    "润色",
    "摘要",
    "邮件",
    "口语化",
    "人话",
)
PERF_TERMS = ("加速", "性能", "内存", "热区", "热路径", "performance", "memory", "latency")
COMPAT_TERMS = ("保底", "补丁", "兼容", "fallback", "shim", "wrapper", "patch")
HOOK_TERMS = ("hook", "hooks", "agent.md", "claude.md", "pretooluse", "userpromptsubmit")
COMMON_CONTEXT = "实现要求：优先直接落目标行为，不要先叠兼容层、补丁分支、保底开关或 keep-old-and-add-new。"
SIMPLIFY_CONTEXT = "简化优先：先判断能不能删、合并、内联或收窄；如果两种实现都能完成需求，选层级更少、分支更少、拷贝更少的。"
PERF_CONTEXT = "顺手看热路径上的重复 I/O、重复序列化、无谓 clone、临时对象和多余包装层。"
COMPAT_CONTEXT = "如果旧兼容/过渡逻辑已经没有真实必要，优先删掉而不是继续包一层。"
RUST_CONTEXT = "Rust 额外检查：盯住热循环里的分配、clone、String/Vec 复制和 serde_json 往返。"
PYTHON_CONTEXT = "Python 额外检查：盯住重复解析、重复读文件、wrapper-on-wrapper 和兼容别名链。"
HOOK_CONTEXT = "Hook 额外检查：让 hook 增加自动化，而不是只做阻拦；优先短上下文、窄触发、低开销，并尽量用 matcher/if 避免无谓触发。"
TEST_CONTEXT = "测试额外检查：锁真实契约和回归点，不给补丁式旧行为续命。"
RUNTIME_PREFIXES = (
    "codex_agno_runtime/src/codex_agno_runtime/",
    "scripts/router-rs/src/",
)
HOOK_PREFIXES = (
    "scripts/claude_hook_",
    ".claude/hooks/",
)
QUALITY_TARGET_SUFFIXES = (".py", ".rs", ".sh")
ASYNC_AUDIT_PREFIXES = (
    "codex_agno_runtime/src/codex_agno_runtime/",
    "scripts/router-rs/src/",
    "scripts/claude_hook_",
    "scripts/materialize_cli_host_entrypoints.py",
    ".claude/hooks/",
)
COMPAT_SMELL_RE = re.compile(
    r"\b(?:compat|compatibility|legacy|fallback|shim|patch|workaround|temporary|deprecated)\b|兼容|保底|补丁"
)
SNAPSHOT_ROOT = Path(tempfile.gettempdir()) / "claude_hook_automation_snapshots"


def _join_unique_context(parts: list[str]) -> str:
    seen: set[str] = set()
    ordered: list[str] = []
    for part in parts:
        if not part or part in seen:
            continue
        seen.add(part)
        ordered.append(part)
    return " ".join(ordered)


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


def _extract_path_hints(prompt_text: str) -> set[str]:
    return {match.group(0).replace("\\", "/") for match in CODE_PATH_RE.finditer(prompt_text)}


def _looks_like_coding_request(prompt_text: str) -> bool:
    lowered = prompt_text.lower()
    strong_actions = sum(token in lowered for token in STRONG_ACTION_TERMS)
    weak_actions = sum(token in prompt_text or token in lowered for token in WEAK_ACTION_TERMS)
    code_targets = sum(token in prompt_text or token in lowered for token in CODE_TARGET_TERMS)
    path_mentions = len(PROMPT_PATH_RE.findall(prompt_text))
    non_code_edits = sum(token in prompt_text for token in NON_CODE_EDIT_TERMS)
    action_score = strong_actions * 2 + weak_actions
    target_score = code_targets + path_mentions * 2
    if action_score == 0:
        return False
    if target_score == 0:
        return False
    if non_code_edits >= 2 and strong_actions == 0 and target_score < 2:
        return False
    return action_score + target_score - non_code_edits >= 3


def _prompt_context(prompt_text: str) -> str | None:
    if not _looks_like_coding_request(prompt_text):
        return None
    lowered = prompt_text.lower()
    parts = [COMMON_CONTEXT, SIMPLIFY_CONTEXT]
    if any(token in prompt_text or token in lowered for token in PERF_TERMS):
        parts.append(PERF_CONTEXT)
    if any(token in prompt_text or token in lowered for token in COMPAT_TERMS):
        parts.append(COMPAT_CONTEXT)
    if any(token in lowered for token in HOOK_TERMS):
        parts.append(HOOK_CONTEXT)
    path_hints = _extract_path_hints(prompt_text)
    if any("hook" in hint.lower() for hint in path_hints):
        parts.append(HOOK_CONTEXT)
    return _join_unique_context(parts)


def run_user_prompt_submit(_repo_root: Path, payload: dict[str, Any]) -> int:
    prompt_text = _extract_prompt_text(payload)
    context = _prompt_context(prompt_text)
    if not context:
        return 0
    print(
        json.dumps(
            {
                "hookSpecificOutput": {
                    "hookEventName": "UserPromptSubmit",
                    "additionalContext": context,
                }
            },
            ensure_ascii=False,
        )
    )
    return 0


def _quality_target_context(path: str) -> str | None:
    if not path.endswith(QUALITY_TARGET_SUFFIXES):
        return None
    parts = [COMMON_CONTEXT, SIMPLIFY_CONTEXT]
    is_runtime = any(path.startswith(prefix) for prefix in RUNTIME_PREFIXES)
    is_hook = any(path.startswith(prefix) for prefix in HOOK_PREFIXES) or "hook" in path
    is_test = path.startswith("tests/")
    if not (is_runtime or is_hook or is_test or path == "scripts/materialize_cli_host_entrypoints.py"):
        return None
    if is_runtime:
        parts.append(PERF_CONTEXT)
    if path.endswith(".rs") and is_runtime:
        parts.append(RUST_CONTEXT)
    if path.endswith(".py"):
        parts.append(PYTHON_CONTEXT)
    if is_hook or path.endswith(".sh"):
        parts.append(HOOK_CONTEXT)
    if is_test:
        parts.append(TEST_CONTEXT)
    return _join_unique_context(parts)


def _read_text_if_small(path: Path, max_bytes: int = 200_000) -> str:
    try:
        if not path.is_file() or path.stat().st_size > max_bytes:
            return ""
        return path.read_text(encoding="utf-8")
    except Exception:
        return ""


def _snapshot_path(repo_root: Path, path: str) -> Path:
    key = hashlib.sha256(f"{repo_root.resolve()}::{path}".encode("utf-8")).hexdigest()
    return SNAPSHOT_ROOT / key[:2] / f"{key}.json"


def _store_pre_edit_snapshot(repo_root: Path, path: str) -> None:
    target = repo_root / path
    payload: dict[str, Any]
    if target.is_file():
        text = _read_text_if_small(target)
        if not text and target.stat().st_size > 200_000:
            return
        payload = {"exists": True, "text": text}
    else:
        payload = {"exists": False, "text": ""}
    snapshot = _snapshot_path(repo_root, path)
    snapshot.parent.mkdir(parents=True, exist_ok=True)
    snapshot.write_text(json.dumps(payload, ensure_ascii=False), encoding="utf-8")


def _pop_pre_edit_snapshot(repo_root: Path, path: str) -> tuple[str, str] | None:
    snapshot = _snapshot_path(repo_root, path)
    if not snapshot.is_file():
        return None
    try:
        payload = json.loads(snapshot.read_text(encoding="utf-8"))
    except Exception:
        snapshot.unlink(missing_ok=True)
        return None
    snapshot.unlink(missing_ok=True)
    if not isinstance(payload, dict):
        return None
    before = payload.get("text", "")
    if not isinstance(before, str):
        before = ""
    mode = "pre_tool_snapshot_added_lines" if payload.get("exists") else "pre_tool_snapshot_new_file"
    return before, mode


def _compat_smell_count(text: str) -> int:
    return len(COMPAT_SMELL_RE.findall(text))


def _git_tracked_text(repo_root: Path, path: str) -> str:
    try:
        result = subprocess.run(
            ["git", "show", f"HEAD:{path}"],
            cwd=repo_root,
            capture_output=True,
            check=False,
            text=True,
        )
    except Exception:
        return ""
    if result.returncode != 0:
        return ""
    return result.stdout


def _added_lines(before: str, after: str) -> str:
    if not before:
        return after
    added: list[str] = []
    for line in difflib.ndiff(before.splitlines(), after.splitlines()):
        if line.startswith("+ "):
            added.append(line[2:])
    return "\n".join(added).strip()


def _extract_multi_edit_delta(tool_input: dict[str, Any]) -> str:
    edits = tool_input.get("edits")
    if not isinstance(edits, list):
        return ""
    parts: list[str] = []
    for edit in edits:
        if not isinstance(edit, dict):
            continue
        new_string = edit.get("new_string")
        if isinstance(new_string, str) and new_string.strip():
            parts.append(new_string)
    return "\n".join(parts).strip()


def _extract_audit_delta(repo_root: Path, path: str, payload: dict[str, Any]) -> tuple[str, str]:
    tool_name = payload.get("tool_name")
    tool_input = payload.get("tool_input")
    if not isinstance(tool_input, dict):
        return "", "none"

    snapshot = _pop_pre_edit_snapshot(repo_root, path)
    if snapshot is not None:
        before_text, mode = snapshot
        after_text = _read_text_if_small(repo_root / path)
        if not after_text and (repo_root / path).exists() and (repo_root / path).stat().st_size > 200_000:
            return "", "snapshot_skip_large_after"
        added = _added_lines(before_text, after_text)
        return added, mode if added else f"{mode}_no_added_lines"

    if tool_name == "Edit":
        new_string = tool_input.get("new_string")
        if isinstance(new_string, str) and new_string.strip():
            return new_string, "edit_new_string"
        return "", "edit_empty"

    if tool_name == "MultiEdit":
        delta = _extract_multi_edit_delta(tool_input)
        return delta, "multi_edit_new_strings" if delta else "multi_edit_empty"

    if tool_name == "Write":
        content = tool_input.get("content")
        if not isinstance(content, str) or not content.strip():
            return "", "write_empty"
        tracked = _git_tracked_text(repo_root, path)
        if tracked:
            added = _added_lines(tracked, content)
            return added, "write_git_added_lines" if added else "write_no_added_lines"
        current_path = repo_root / path
        if not current_path.exists():
            return content, "write_new_file"
        return "", "write_skip_existing_without_base"

    return "", "unsupported_tool"


def _build_async_audit_context(path: str, text: str, source_mode: str) -> str | None:
    compat_hits = _compat_smell_count(text)
    parts: list[str] = []
    lowered_path = path.lower()
    source_label = f"增量来源={source_mode}"

    if path.endswith(".rs"):
        clone_hits = text.count(".clone(") + text.count(".clone()")
        serde_hits = text.count("serde_json::")
        string_hits = text.count(".to_string()") + text.count(".to_owned()")
        if compat_hits >= 1 or clone_hits >= 2 or serde_hits >= 2 or string_hits >= 3:
            parts.append(
                f"`{path}` 的新增片段有实现复查信号：{source_label}, compat={compat_hits}, clone={clone_hits}, serde={serde_hits}, string_copy={string_hits}。"
            )
            parts.append("如果这轮还在继续，优先通过删除、合并、内联或收窄解决；先删过渡逻辑，再压缩热路径里的 clone 和序列化往返。")
    elif path.endswith(".py"):
        json_hits = text.count("json.loads(") + text.count("json.dumps(")
        io_hits = text.count(".read_text(") + text.count(".read_bytes(") + text.count(".write_text(")
        wrapper_hits = text.count("def ") if "hook" in lowered_path else 0
        if compat_hits >= 1 or json_hits >= 2 or io_hits >= 2:
            parts.append(
                f"`{path}` 的新增片段有实现复查信号：{source_label}, compat={compat_hits}, json_roundtrip={json_hits}, file_io={io_hits}。"
            )
            parts.append("优先通过删除、合并、内联或收窄解决；先删兼容/补丁分支，再减少重复解析、重复读写和 wrapper-on-wrapper。")
        elif "hook" in lowered_path and compat_hits >= 1:
            parts.append(
                f"`{path}` 的新增片段仍带有明显的 hook 过渡信号：{source_label}, compat={compat_hits}, helper_defs={wrapper_hits}。"
            )
            parts.append("确认这层是在增加自动化，而不是只多加一道阻拦或中转包装；能删、能并、能收窄就别再包一层。")
    elif path.endswith(".sh"):
        deny_hits = text.count("permissionDecision")
        if "hook" in lowered_path and (compat_hits >= 1 or deny_hits >= 1):
            parts.append(
                f"`{path}` 的新增片段有 hook 复查信号：{source_label}, compat={compat_hits}, deny_rules={deny_hits}。"
            )
            parts.append("确认这层仍然是短路径、低开销、加自动化；能删规则、合并判断、收窄 matcher/if，就不要继续堆阻拦脚本。")

    if not parts:
        return None
    return "异步实现复查：" + " ".join(parts)


def _is_async_audit_target(path: str) -> bool:
    return any(path.startswith(prefix) for prefix in ASYNC_AUDIT_PREFIXES)


def run_post_tool_audit(repo_root: Path, payload: dict[str, Any]) -> int:
    tool_name = payload.get("tool_name")
    if tool_name not in {"Edit", "MultiEdit", "Write"}:
        return 0
    rel_paths = {
        _relative_candidate(path, repo_root)
        for path in _iter_payload_paths(payload)
    }
    first_context: str | None = None
    for path in sorted(rel_paths):
        if not _is_async_audit_target(path) or not path.endswith(QUALITY_TARGET_SUFFIXES):
            continue
        delta_text, source_mode = _extract_audit_delta(repo_root, path, payload)
        if not delta_text.strip():
            continue
        context = _build_async_audit_context(path, delta_text, source_mode)
        if context and first_context is None:
            first_context = context
    if not first_context:
        return 0
    print(
        json.dumps(
            {
                "hookSpecificOutput": {
                    "hookEventName": "PostToolUse",
                    "additionalContext": first_context,
                },
                "additionalContext": first_context,
            },
            ensure_ascii=False,
        )
    )
    return 0


def run_pre_tool_use_quality(repo_root: Path, payload: dict[str, Any]) -> int:
    tool_name = payload.get("tool_name")
    if tool_name not in {"Edit", "MultiEdit", "Write"}:
        return 0
    rel_paths = {
        _relative_candidate(path, repo_root)
        for path in _iter_payload_paths(payload)
    }
    for path in sorted(rel_paths):
        if _is_async_audit_target(path) and path.endswith(QUALITY_TARGET_SUFFIXES):
            _store_pre_edit_snapshot(repo_root, path)
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
    parser.add_argument("command", choices=("user-prompt-submit", "pre-tool-use-quality", "post-tool-audit"))
    parser.add_argument("--repo-root", required=True)
    args = parser.parse_args()

    repo_root = Path(args.repo_root).resolve()
    payload = _read_payload()

    if args.command == "user-prompt-submit":
        return run_user_prompt_submit(repo_root, payload)
    if args.command == "pre-tool-use-quality":
        return run_pre_tool_use_quality(repo_root, payload)
    return run_post_tool_audit(repo_root, payload)


if __name__ == "__main__":
    raise SystemExit(main())
