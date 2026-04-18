#!/usr/bin/env python3
"""Compatibility wrapper for repository-local session lifecycle memory hooks."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.claude_memory_bridge import DEFAULT_MAX_LINES, run_bridge
from scripts.memory_support import get_repo_root


COMMAND_ALIASES = {
    "start-session": "session-start",
    "stop-session": "session-stop",
    "end-session": "session-end",
}

SUPPORTED_COMMANDS = (
    "session-start",
    "session-stop",
    "pre-compact",
    "subagent-stop",
    "session-end",
)


def _canonical_command(command: str) -> str:
    resolved = COMMAND_ALIASES.get(command, command)
    if resolved not in SUPPORTED_COMMANDS:
        raise ValueError(f"Unsupported lifecycle command: {command}")
    return resolved


def run_lifecycle_hook(command: str, repo_root: Path, *, max_lines: int = DEFAULT_MAX_LINES) -> dict[str, Any]:
    """Run one lifecycle event through the shared memory bridge."""

    canonical = _canonical_command(command)
    result = run_bridge(canonical, repo_root, max_lines=max_lines)
    return {
        "wrapper_command": command,
        "canonical_command": canonical,
        **result,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Run one repository-local session lifecycle hook.")
    parser.add_argument("command", choices=tuple((*COMMAND_ALIASES.keys(), *SUPPORTED_COMMANDS)))
    parser.add_argument("--repo-root", type=Path, default=None, help="Repository root. Defaults to the detected git root.")
    parser.add_argument("--max-lines", type=int, default=DEFAULT_MAX_LINES, help="Maximum bullets per section.")
    parser.add_argument("--json", action="store_true", dest="json_output", help="Emit JSON.")
    args = parser.parse_args()

    repo_root = (args.repo_root or get_repo_root()).resolve()
    result = run_lifecycle_hook(args.command, repo_root, max_lines=args.max_lines)
    if args.json_output:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print(result["projection"]["target_path"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
