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

from framework_runtime.rust_router import get_cached_route_adapter


def _read_json(path: Path) -> dict:
    if not path.is_file():
        return {}
    return json.loads(path.read_text(encoding="utf-8"))


def _adapter(repo_root: Path):
    framework_root = Path(__file__).resolve().parents[1]
    return get_cached_route_adapter(framework_root)


def _route_from_snapshot_payload(snapshot: dict) -> list[str]:
    continuity = snapshot.get("continuity", {})
    if isinstance(continuity, dict):
        route = continuity.get("route")
        if isinstance(route, list) and route:
            return [str(item).strip() for item in route if str(item).strip()]
    current_root = Path(str(snapshot.get("current_root") or ""))
    if current_root:
        trace_payload = _read_json(current_root / "TRACE_METADATA.json")
        skills = trace_payload.get("matched_skills")
        if isinstance(skills, list):
            return [str(item).strip() for item in skills if str(item).strip()]
    return []


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

    if status == "completed":
        for item in next_actions:
            if isinstance(item, str) and item.strip():
                return f"next={_short_text(item, 36)}"
        if git_state == "dirty":
            return "next=review local changes"
        return "next=pick task"

    for item in next_actions:
        if isinstance(item, str) and item.strip():
            return "next=/refresh"

    return "next=run verification"


def render_statusline(repo_root: Path) -> str:
    snapshot = _adapter(repo_root).framework_runtime_snapshot(repo_root=repo_root)
    continuity = snapshot.get("continuity", {}) if isinstance(snapshot.get("continuity"), dict) else {}
    supervisor_state = (
        snapshot.get("supervisor_state", {})
        if isinstance(snapshot.get("supervisor_state"), dict)
        else {}
    )

    task = str(continuity.get("task") or supervisor_state.get("task_summary") or "none")
    phase = str(continuity.get("phase") or supervisor_state.get("active_phase") or "idle")
    status = str(continuity.get("status") or "unknown")
    route = _short_route(_route_from_snapshot_payload(snapshot))
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

    focus_task_id = str(snapshot.get("focus_task_id") or "")
    known_task_ids = [str(item).strip() for item in snapshot.get("known_task_ids", []) if str(item).strip()]
    recoverable_task_ids = [
        str(item).strip() for item in snapshot.get("recoverable_task_ids", []) if str(item).strip()
    ]
    other_known_count = max(len(known_task_ids) - (1 if focus_task_id else 0), 0)
    other_recoverable_count = sum(1 for item in recoverable_task_ids if item != focus_task_id)

    short_task = _short_text(task, 24)
    hint = _decision_hint(blockers_list, next_actions_list, git_state=git_state, status=status)
    return (
        f"{branch} | {hint} | {phase}/{status} | "
        f"task={short_task} | route={route} | nexts={next_count} | blockers={blockers} | "
        f"others={other_known_count} | resumable={other_recoverable_count} | git={git_state}"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Render the Claude Code status line for this repo.")
    parser.add_argument("--repo-root", required=True)
    args = parser.parse_args()

    print(render_statusline(Path(args.repo_root).resolve()))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
