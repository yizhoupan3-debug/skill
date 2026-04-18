#!/usr/bin/env python3
"""Retire the repo-local Codex model instructions overlay when present."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

FRAMEWORK_START_MARKER = "<!-- FRAMEWORK_DEFAULT_RUNTIME_START -->"
FRAMEWORK_END_MARKER = "<!-- FRAMEWORK_DEFAULT_RUNTIME_END -->"
DEFAULT_INSTRUCTIONS_PATH = Path(".codex") / "model_instructions.md"
def _strip_marker_pair(text: str, start_marker: str, end_marker: str) -> str:
    start = text.find(start_marker)
    end = text.find(end_marker)
    if start == -1 or end == -1:
        return text
    after = text[end + len(end_marker):]
    return (text[:start] + after).strip() + ("\n" if text.strip() else "")


def strip_managed_block(text: str) -> str:
    return _strip_marker_pair(text, FRAMEWORK_START_MARKER, FRAMEWORK_END_MARKER)


def retire_overlay(path: Path) -> dict[str, Any]:
    """Retire the repo-local overlay file or managed block when it still exists."""

    if not path.exists():
        return {
            "success": True,
            "path": str(path),
            "changed": False,
            "status": "already-retired",
            "retirement_mode": "missing",
        }

    original = path.read_text(encoding="utf-8")
    stripped = strip_managed_block(original).strip()
    if stripped:
        updated = stripped + "\n"
        changed = updated != original
        if changed:
            path.write_text(updated, encoding="utf-8")
        return {
            "success": True,
            "path": str(path),
            "changed": changed,
            "status": "retired-managed-block",
            "retirement_mode": "preserved-user-content",
        }

    path.unlink()
    return {
        "success": True,
        "path": str(path),
        "changed": True,
        "status": "retired-file",
        "retirement_mode": "deleted-empty-overlay",
    }


def install_block(path: Path) -> dict[str, Any]:
    """Compatibility alias for old callers; now retires the dead overlay."""

    return retire_overlay(path)


def remove_block(path: Path) -> dict[str, Any]:
    """Remove the managed block."""

    if not path.exists():
        return {"success": True, "path": str(path), "changed": False, "status": "missing"}
    original = path.read_text(encoding="utf-8")
    updated = strip_managed_block(original)
    changed = updated != original
    if changed:
        path.write_text(updated, encoding="utf-8")
    return {"success": True, "path": str(path), "changed": changed, "status": "removed"}


def main() -> int:
    parser = argparse.ArgumentParser(description="Retire the repo-local Codex overlay when it still exists.")
    sub = parser.add_subparsers(dest="cmd", required=True)
    for name in ("install", "remove", "retire"):
        child = sub.add_parser(name)
        child.add_argument("--path", type=Path, default=DEFAULT_INSTRUCTIONS_PATH)
        child.add_argument("--json", action="store_true", dest="json_output")
    args = parser.parse_args()
    if args.cmd in {"install", "retire"}:
        payload = retire_overlay(args.path)
    else:
        payload = remove_block(args.path)
    if args.json_output:
        print(json.dumps(payload, ensure_ascii=False, indent=2))
    else:
        print(payload)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
