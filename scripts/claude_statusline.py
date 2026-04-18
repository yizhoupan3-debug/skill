#!/usr/bin/env python3
"""Render a compact Claude Code status line from shared runtime artifacts."""

from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path


def _read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8") if path.is_file() else ""


def _read_json(path: Path) -> dict:
    if not path.is_file():
        return {}
    return json.loads(path.read_text(encoding="utf-8"))


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


def _count_open_blockers(supervisor_state: dict) -> int:
    blockers = supervisor_state.get("blockers", {}).get("open_blockers", [])
    if not isinstance(blockers, list):
        return 0
    return len(blockers)


def _short_text(value: str, limit: int) -> str:
    text = " ".join(value.split())
    return text if len(text) <= limit else f"{text[: limit - 3]}..."


def _decision_hint(supervisor_state: dict, next_actions: dict, *, git_state: str, status: str) -> str:
    blockers = supervisor_state.get("blockers", {}).get("open_blockers", [])
    if isinstance(blockers, list):
        for blocker in blockers:
            if isinstance(blocker, str) and blocker.strip():
                return f"blocked={_short_text(blocker, 36)}"

    if status != "completed":
        return "next=run verification"

    items = next_actions.get("next_actions")
    if isinstance(items, list):
        for item in items:
            if isinstance(item, str) and item.strip():
                return f"next={_short_text(item, 36)}"

    if git_state == "dirty":
        return "next=review local changes"
    return "next=pick task"


def render_statusline(repo_root: Path) -> str:
    session_summary = _parse_summary(_read_text(repo_root / "SESSION_SUMMARY.md"))
    trace_metadata = _read_json(repo_root / "TRACE_METADATA.json")
    supervisor_state = _read_json(repo_root / ".supervisor_state.json")
    next_actions = _read_json(repo_root / "NEXT_ACTIONS.json")

    task = session_summary.get("task") or trace_metadata.get("task") or supervisor_state.get("task_summary") or "none"
    phase = session_summary.get("phase") or supervisor_state.get("active_phase") or "idle"
    status = (
        session_summary.get("status")
        or trace_metadata.get("verification_status")
        or supervisor_state.get("verification", {}).get("verification_status")
        or "unknown"
    )
    route = _short_route(trace_metadata.get("matched_skills") or [])
    git_state, branch = _git_state(repo_root)
    blockers = _count_open_blockers(supervisor_state)
    next_count = _count_next_actions(next_actions)
    short_task = _short_text(task, 24)
    hint = _decision_hint(supervisor_state, next_actions, git_state=git_state, status=status)
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
