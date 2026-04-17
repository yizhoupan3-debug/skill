#!/usr/bin/env python3
"""Write the standard Phase-1 session artifact contract files."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


def write_session_summary(
    path: Path,
    *,
    task: str,
    phase: str,
    status: str,
    summary: str,
) -> None:
    """Write the canonical Markdown session summary.

    Parameters:
        path: Output Markdown path.
        task: Task title.
        phase: Current execution phase.
        status: Current task status.
        summary: High-level summary text.

    Returns:
        None.
    """

    content = "\n".join(
        [
            "# SESSION_SUMMARY",
            "",
            f"- task: {task}",
            f"- phase: {phase}",
            f"- status: {status}",
            "",
            "## Summary",
            summary.strip() or "No summary provided.",
            "",
        ]
    )
    path.write_text(content, encoding="utf-8")


def write_next_actions(path: Path, actions: list[str]) -> None:
    """Write the canonical JSON next-actions file.

    Parameters:
        path: Output JSON path.
        actions: Ordered next actions list.

    Returns:
        None.
    """

    payload = {
        "version": 1,
        "next_actions": actions,
    }
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def write_evidence_index(path: Path, entries: list[dict[str, Any]]) -> None:
    """Write the canonical JSON evidence index file.

    Parameters:
        path: Output JSON path.
        entries: Evidence entry dictionaries.

    Returns:
        None.
    """

    payload = {
        "version": 1,
        "artifacts": entries,
    }
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


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
) -> dict[str, str]:
    """Write the three standard session artifact files into a directory.

    Parameters:
        output_dir: Target directory for artifact files.
        task: Task title.
        phase: Current execution phase.
        status: Current task status.
        summary: High-level summary text.
        next_actions: Ordered next action items.
        evidence: Structured evidence entries.

    Returns:
        dict[str, str]: Mapping of artifact type to written file path.
    """

    output_dir.mkdir(parents=True, exist_ok=True)

    summary_path = output_dir / "SESSION_SUMMARY.md"
    next_actions_path = output_dir / "NEXT_ACTIONS.json"
    evidence_path = output_dir / "EVIDENCE_INDEX.json"

    write_session_summary(summary_path, task=task, phase=phase, status=status, summary=summary)
    write_next_actions(next_actions_path, next_actions)
    write_evidence_index(evidence_path, evidence)

    return {
        "summary": str(summary_path),
        "next_actions": str(next_actions_path),
        "evidence": str(evidence_path),
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
    args = parser.parse_args()

    paths = write_artifacts(
        args.output_dir,
        task=args.task,
        phase=args.phase,
        status=args.status,
        summary=args.summary,
        next_actions=args.next_action,
        evidence=parse_evidence(args.evidence),
    )
    print(json.dumps(paths, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
