#!/usr/bin/env python3
"""Render a compact Claude Code status line from shared runtime artifacts."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.memory_support import classify_runtime_continuity, load_runtime_snapshot


def _read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8") if path.is_file() else ""


def _read_json(path: Path) -> dict:
    if not path.is_file():
        return {}
    return json.loads(path.read_text(encoding="utf-8"))


def _task_scoped_runtime_roots(repo_root: Path) -> list[Path]:
    """Return task-scoped and compatibility-mirror roots in preferred order."""

    current_root = repo_root / "artifacts" / "current"
    roots: list[Path] = []
    pointer = _read_json(current_root / "active_task.json")
    task_id = str(pointer.get("task_id") or "").strip()
    if task_id:
        task_root = current_root / task_id
        if task_root.is_dir():
            roots.append(task_root)
    if current_root.is_dir():
        roots.append(current_root)
    return roots


def _first_runtime_text(paths: list[Path]) -> str:
    """Return the first non-empty runtime text payload."""

    for path in paths:
        text = _read_text(path).strip()
        if text:
            return text
    return ""


def _first_runtime_json(paths: list[Path]) -> dict:
    """Return the first non-empty runtime JSON payload."""

    for path in paths:
        payload = _read_json(path)
        if payload:
            return payload
    return {}


def _parse_summary(summary_text: str) -> dict[str, str]:
    result: dict[str, str] = {}
    for line in summary_text.splitlines():
        stripped = line.strip()
        if not stripped.startswith("- "):
            continue
        key, _, value = stripped[2:].partition(":")
        key = key.strip()
        value = value.strip()
        if key and value:
            result[key] = value
    return result


def _short_route(skills: list[str]) -> str:
    if not skills:
        return "none"
    if len(skills) == 1:
        return skills[0]
    return f"{skills[0]}+{len(skills) - 1}"


def _git_state(repo_root: Path) -> tuple[str, str]:
    result = subprocess.run(
        ["git", "status", "--porcelain", "--branch", "--untracked-files=no"],
        cwd=repo_root,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return "nogit", "nogit"
    lines = result.stdout.splitlines()
    branch = "unknown"
    if lines and lines[0].startswith("## "):
        branch = lines[0][3:].split("...", 1)[0].strip() or "unknown"
    changed = any(line.strip() for line in lines[1:])
    return ("dirty" if changed else "clean"), branch


def _count_next_actions(next_actions: dict) -> int:
    items = next_actions.get("next_actions")
    if not isinstance(items, list):
        return 0
    return sum(1 for item in items if isinstance(item, str) and item.strip())


def _short_text(value: str, limit: int) -> str:
    text = " ".join(value.split())
    return text if len(text) <= limit else f"{text[: limit - 3]}..."


def _decision_hint(blockers: list[str], next_actions: list[str], *, git_state: str, status: str) -> str:
    for blocker in blockers:
        if isinstance(blocker, str) and blocker.strip():
            return f"blocked={_short_text(blocker, 36)}"

    if status != "completed":
        return "next=run verification"

    for item in next_actions:
        if isinstance(item, str) and item.strip():
            return f"next={_short_text(item, 36)}"

    if git_state == "dirty":
        return "next=review local changes"
    return "next=pick task"


def render_statusline(repo_root: Path) -> str:
    runtime_roots = _task_scoped_runtime_roots(repo_root)
    snapshot = load_runtime_snapshot(
        repo_root,
        repair=False,
        include_contract_snapshots=False,
    )
    continuity = classify_runtime_continuity(snapshot)
    supervisor_state = snapshot.supervisor_state if isinstance(snapshot.supervisor_state, dict) else {}

    fallback_summary = _parse_summary(_read_text(repo_root / "SESSION_SUMMARY.md"))
    fallback_trace = _read_json(repo_root / "TRACE_METADATA.json")
    fallback_next_actions = _read_json(repo_root / "NEXT_ACTIONS.json")
    use_root_fallback = not runtime_roots and (
        bool(fallback_summary) or bool(fallback_trace) or bool(fallback_next_actions)
    )

    task = str(continuity.get("task") or supervisor_state.get("task_summary") or "none")
    phase = str(continuity.get("phase") or supervisor_state.get("active_phase") or "idle")
    status = str(continuity.get("status") or "unknown")
    route = _short_route(continuity.get("route") or [])
    git_state, branch = _git_state(repo_root)
    blockers_list = [
        str(item).strip()
        for item in continuity.get("blockers", [])
        if str(item).strip()
    ]
    next_actions_list = [
        str(item).strip()
        for item in continuity.get("next_actions", [])
        if str(item).strip()
    ]
    blockers = len(blockers_list)
    next_count = len(next_actions_list)
    if use_root_fallback:
        task = str(
            fallback_summary.get("task")
            or fallback_trace.get("task")
            or supervisor_state.get("task_summary")
            or "none"
        )
        phase = str(fallback_summary.get("phase") or supervisor_state.get("active_phase") or "idle")
        status = str(
            fallback_summary.get("status")
            or fallback_trace.get("verification_status")
            or supervisor_state.get("verification", {}).get("verification_status")
            or "unknown"
        )
        route = _short_route(fallback_trace.get("matched_skills") or [])
        next_actions_list = [
            str(item).strip()
            for item in fallback_next_actions.get("next_actions", [])
            if str(item).strip()
        ]
        next_count = len(next_actions_list)
    short_task = _short_text(task, 24)
    hint = _decision_hint(blockers_list, next_actions_list, git_state=git_state, status=status)
    return (
        f"{branch} | {hint} | {phase}/{status} | "
        f"task={short_task} | route={route} | nexts={next_count} | blockers={blockers} | git={git_state}"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Render the Claude Code status line for this repo.")
    parser.add_argument("--repo-root", required=True)
    args = parser.parse_args()

    print(render_statusline(Path(args.repo_root).resolve()))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
