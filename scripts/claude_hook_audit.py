#!/usr/bin/env python3
"""Host-private Claude hook helpers for audit and generated-surface guards."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

GENERATED_PATHS = {
    ".claude/settings.json",
    ".claude/hooks/README.md",
    ".claude/hooks/pre_tool_use.sh",
    ".claude/hooks/session_end.sh",
    ".claude/hooks/config_change.sh",
    ".claude/hooks/stop_failure.sh",
}
PROTECTED_GENERATED_PATHS = {
    "AGENT.md",
    "AGENTS.md",
    "CLAUDE.md",
    "GEMINI.md",
    ".gemini/settings.json",
    ".claude/settings.json",
    ".claude/agents/README.md",
    ".codex/host_entrypoints_sync_manifest.json",
    ".codex/memory/CLAUDE_MEMORY.md",
}
PROTECTED_GENERATED_PREFIXES = (
    ".claude/hooks/",
    ".claude/commands/",
)
PROTECTED_BASH_PATH_HINTS = (
    "AGENT.md",
    "AGENTS.md",
    "CLAUDE.md",
    "GEMINI.md",
    ".gemini/settings.json",
    ".claude/settings.json",
    ".claude/agents/README.md",
    ".claude/hooks/",
    ".claude/commands/",
    ".claude/",
    ".codex/host_entrypoints_sync_manifest.json",
    ".codex/memory/CLAUDE_MEMORY.md",
)
BASH_SEGMENT_SPLIT_RE = re.compile(r"\s*(?:&&|\|\||;|\|)\s*")
BASH_MUTATION_PATTERNS = (
    re.compile(r"^\s*(mv|cp|install|touch|rm|unlink|truncate)\b"),
    re.compile(r"^\s*ln\b[^\n]*\s-[^\n]*[fs][^\n]*\b"),
    re.compile(r"^\s*git\s+(checkout\s+--|restore\b)"),
    re.compile(r"\bsed\s+-i\b"),
    re.compile(r"\bperl\s+-pi\b"),
    re.compile(r"\bpython3?\s+-c\b"),
    re.compile(r"\bnode\s+-e\b"),
    re.compile(r"\bruby\s+-e\b"),
    re.compile(r"\btee\b"),
    re.compile(r"\bdd\b"),
)
SHARED_CONTINUITY_PATHS = (
    "SESSION_SUMMARY.md",
    "NEXT_ACTIONS.json",
    "EVIDENCE_INDEX.json",
    "TRACE_METADATA.json",
    ".supervisor_state.json",
)


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
    for key in (
        "file_path",
        "changed_path",
        "path",
        "config_path",
        "target_path",
    ):
        normalized = _normalize_path(payload.get(key))
        if normalized:
            candidates.append(normalized)
    changed_files = payload.get("changed_files")
    if isinstance(changed_files, list):
        for item in changed_files:
            normalized = _normalize_path(item)
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


def _payload_mentions_continuity(payload: dict[str, Any]) -> bool:
    serialized = json.dumps(payload, ensure_ascii=False)
    return any(forbidden in serialized for forbidden in SHARED_CONTINUITY_PATHS)


def _classify_protected_generated_path(path: str) -> str | None:
    if path in PROTECTED_GENERATED_PATHS:
        return "generated_file"
    if any(path.startswith(prefix) for prefix in PROTECTED_GENERATED_PREFIXES):
        return "generated_file"
    return None


def _pre_tool_use_message(path: str) -> str:
    if path == ".codex/memory/CLAUDE_MEMORY.md":
        return (
            "[claude-pre-tool-use] blocked direct edits to imported Claude projection "
            f"{path}; edit the memory source files or rerun the projection refresh instead."
        )
    return (
        "[claude-pre-tool-use] blocked direct edits to generated host surface "
        f"{path}; edit scripts/materialize_cli_host_entrypoints.py and regenerate outputs instead."
    )


def _find_bash_generated_write(payload: dict[str, Any]) -> str | None:
    tool_name = payload.get("tool_name")
    if tool_name != "Bash":
        return None
    tool_input = payload.get("tool_input")
    command = ""
    if isinstance(tool_input, dict):
        value = tool_input.get("command")
        if isinstance(value, str):
            command = value
    elif isinstance(payload.get("command"), str):
        command = payload["command"]
    if not command:
        return None
    for segment in _iter_bash_segments(command):
        looks_mutating = any(pattern.search(segment) for pattern in BASH_MUTATION_PATTERNS)
        for hint in PROTECTED_BASH_PATH_HINTS:
            if hint not in segment:
                continue
            if looks_mutating or _segment_redirects_to_protected_path(segment, hint):
                if hint == ".claude/":
                    return ".claude/**"
                return hint
    return None


def _iter_bash_segments(command: str) -> list[str]:
    return [segment.strip() for segment in BASH_SEGMENT_SPLIT_RE.split(command) if segment.strip()]


def _segment_redirects_to_protected_path(segment: str, hint: str) -> bool:
    escaped_hint = re.escape(hint)
    patterns = (
        rf"(>>?|>\|)\s*['\"]?[^'\"\n;&|]*{escaped_hint}[^'\"\n;&|]*['\"]?",
        rf"\btee\b(?:\s+-a)?\s+['\"]?[^'\"\n;&|]*{escaped_hint}[^'\"\n;&|]*['\"]?",
        rf"\bdd\b[^\n;&|]*\bof=['\"]?[^'\"\n;&|]*{escaped_hint}[^'\"\n;&|]*['\"]?",
    )
    return any(re.search(pattern, segment) for pattern in patterns)


def run_config_change(repo_root: Path, payload: dict[str, Any]) -> int:
    scope = payload.get("source") or payload.get("scope") or payload.get("matcher") or "unknown"
    rel_paths = {
        _relative_candidate(path, repo_root)
        for path in _iter_candidate_paths(payload)
    }
    if _payload_mentions_continuity(payload):
        print(
            "[claude-config-change] payload referenced shared continuity artifacts; leaving them untouched and keeping audit host-private.",
            file=sys.stderr,
        )
    hit_generated = sorted(path for path in rel_paths if path in GENERATED_PATHS)
    if scope != "project_settings":
        return 0
    if not hit_generated:
        print(
            "[claude-config-change] project settings changed outside generated Claude host surfaces; no action taken.",
            file=sys.stderr,
        )
        return 0
    print(
        "[claude-config-change] detected edits on generated Claude host surfaces: "
        + ", ".join(hit_generated)
        + "; regenerate via scripts/materialize_cli_host_entrypoints.py instead of hand-editing outputs.",
        file=sys.stderr,
    )
    return 0


def run_stop_failure(_repo_root: Path, payload: dict[str, Any]) -> int:
    failure_type = payload.get("error") or payload.get("failure_type") or payload.get("matcher") or "unknown"
    continuity_note = " Shared continuity remains untouched." if _payload_mentions_continuity(payload) else ""
    print(
        "[claude-stop-failure] Claude stop failure classified as "
        f"{failure_type}; inspect /hooks, generated host files, and host-private projection drift before retrying."
        + continuity_note,
        file=sys.stderr,
    )
    return 0


def run_pre_tool_use(repo_root: Path, payload: dict[str, Any]) -> int:
    rel_paths = {
        _relative_candidate(path, repo_root)
        for path in _iter_payload_paths(payload)
    }
    for path in sorted(rel_paths):
        if _classify_protected_generated_path(path):
            print(
                json.dumps(
                    {
                        "hookSpecificOutput": {
                            "hookEventName": "PreToolUse",
                            "permissionDecision": "deny",
                            "permissionDecisionReason": _pre_tool_use_message(path),
                        }
                    },
                    ensure_ascii=False,
                )
            )
            return 0
    bash_path = _find_bash_generated_write(payload)
    if bash_path:
        print(
            json.dumps(
                {
                    "hookSpecificOutput": {
                        "hookEventName": "PreToolUse",
                        "permissionDecision": "deny",
                        "permissionDecisionReason": _pre_tool_use_message(bash_path),
                    }
                },
                ensure_ascii=False,
            )
        )
        return 0
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Audit Claude hook payloads without mutating shared continuity.")
    parser.add_argument("command", choices=("config-change", "stop-failure", "pre-tool-use"))
    parser.add_argument("--repo-root", required=True)
    args = parser.parse_args()

    repo_root = Path(args.repo_root).resolve()
    payload = _read_payload()

    if args.command == "config-change":
        return run_config_change(repo_root, payload)
    if args.command == "pre-tool-use":
        return run_pre_tool_use(repo_root, payload)
    return run_stop_failure(repo_root, payload)


if __name__ == "__main__":
    raise SystemExit(main())
