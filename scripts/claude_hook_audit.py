#!/usr/bin/env python3
"""Host-private audit helpers for Claude config change and stop failure hooks."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

GENERATED_PATHS = {
    ".claude/settings.json",
    ".claude/hooks/README.md",
    ".claude/hooks/session_start.sh",
    ".claude/hooks/stop.sh",
    ".claude/hooks/pre_compact.sh",
    ".claude/hooks/subagent_stop.sh",
    ".claude/hooks/session_end.sh",
    ".claude/hooks/config_change.sh",
    ".claude/hooks/stop_failure.sh",
}
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


def main() -> int:
    parser = argparse.ArgumentParser(description="Audit Claude hook payloads without mutating shared continuity.")
    parser.add_argument("command", choices=("config-change", "stop-failure"))
    parser.add_argument("--repo-root", required=True)
    args = parser.parse_args()

    repo_root = Path(args.repo_root).resolve()
    payload = _read_payload()

    if args.command == "config-change":
        return run_config_change(repo_root, payload)
    return run_stop_failure(repo_root, payload)


if __name__ == "__main__":
    raise SystemExit(main())
