#!/usr/bin/env python3
"""Write the standard Phase-1 session artifact contract files."""

from __future__ import annotations

import argparse
import json
import sys
from functools import lru_cache
from pathlib import Path
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from framework_runtime.rust_router import RustRouteAdapter


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


@lru_cache(maxsize=1)
def _rust_adapter() -> RustRouteAdapter:
    return RustRouteAdapter(_repo_root())


def parse_evidence(raw_items: list[str]) -> list[dict[str, Any]]:
    """Parse CLI evidence items using `kind=path` syntax.

    Parameters:
        raw_items: Raw CLI evidence items.

    Returns:
        list[dict[str, Any]]: Structured evidence entries.
    """

    parsed: list[dict[str, Any]] = []
    for item in raw_items:
        if "=" not in item:
            raise SystemExit(f"Invalid evidence item (expected kind=path): {item}")
        kind, path = item.split("=", 1)
        parsed.append({"kind": kind.strip(), "path": path.strip()})
    return parsed

def write_artifacts(
    output_dir: Path,
    *,
    task: str,
    phase: str,
    status: str,
    summary: str,
    next_actions: list[str],
    evidence: list[dict[str, Any]],
    task_id: str | None = None,
    mirror_output_dir: Path | None = None,
    repo_root: Path | None = None,
    focus: bool = False,
) -> dict[str, str]:
    payload = {
        "output_dir": str(output_dir),
        "task": task,
        "phase": phase,
        "status": status,
        "summary": summary,
        "next_actions": next_actions,
        "evidence": evidence,
        "task_id": task_id,
        "mirror_output_dir": str(mirror_output_dir) if mirror_output_dir is not None else None,
        "repo_root": str(repo_root) if repo_root is not None else None,
        "focus": focus,
    }
    resolved = _rust_adapter().write_framework_session_artifacts(payload)
    return {
        "summary": resolved["summary"],
        "next_actions": resolved["next_actions"],
        "evidence": resolved["evidence"],
        "task_id": resolved["task_id"],
    }


def main() -> int:
    """CLI entry point for writing standard session artifacts.

    Parameters:
        None.

    Returns:
        int: Process exit code.
    """

    parser = argparse.ArgumentParser(description="Write standard session artifacts.")
    parser.add_argument("--output-dir", type=Path, required=True, help="Target output directory.")
    parser.add_argument("--task", required=True, help="Task title.")
    parser.add_argument("--phase", default="implementation", help="Current phase label.")
    parser.add_argument("--status", default="in_progress", help="Current status label.")
    parser.add_argument("--summary", default="", help="Summary text.")
    parser.add_argument("--next-action", action="append", default=[], help="Repeatable next-action item.")
    parser.add_argument("--evidence", action="append", default=[], help="Repeatable evidence item using kind=path.")
    parser.add_argument("--task-id", default="", help="Optional task id for task-scoped artifact writes.")
    parser.add_argument("--mirror-output-dir", type=Path, default=None, help="Optional compatibility mirror output directory.")
    parser.add_argument("--repo-root", type=Path, default=None, help="Optional repo root used to refresh the focus-task projection.")
    parser.add_argument("--focus", action="store_true", help="Project this task into root and artifacts/current compatibility mirrors.")
    args = parser.parse_args()

    paths = write_artifacts(
        args.output_dir,
        task=args.task,
        phase=args.phase,
        status=args.status,
        summary=args.summary,
        next_actions=args.next_action,
        evidence=parse_evidence(args.evidence),
        task_id=args.task_id or None,
        mirror_output_dir=args.mirror_output_dir,
        repo_root=args.repo_root,
        focus=args.focus,
    )
    print(json.dumps(paths, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
