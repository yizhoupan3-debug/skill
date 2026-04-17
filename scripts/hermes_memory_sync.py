#!/usr/bin/env python3
"""One-way sync: Codex project memory to Hermes memories directory."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.memory_support import (
    current_local_timestamp,
    get_repo_root,
    read_text_if_exists,
    resolve_effective_memory_dir,
    safe_slug,
    workspace_name_from_root,
    write_text_if_changed,
)

HERMES_HOME = Path.home() / ".hermes"
HERMES_MEMORIES_DIR = HERMES_HOME / "memories"
SYNC_HEADER_TEMPLATE = """---
source: codex-project-memory
synced_at: {timestamp}
---

"""


def _extract_section(text: str, heading: str) -> str:
    """Extract content under a ## heading."""

    pattern = re.compile(rf"^##\s+{re.escape(heading)}\s*\n(.*?)(?=^##\s|\Z)", re.MULTILINE | re.DOTALL)
    match = pattern.search(text)
    return match.group(1).strip() if match else ""


def _extract_lessons(memory_md: str) -> str:
    """Extract the Lessons section from MEMORY.md."""

    return _extract_section(memory_md, "Lessons")


def _extract_decisions(memory_md: str) -> str:
    """Extract stable decisions from MEMORY.md."""

    return _extract_section(memory_md, "稳定决策") or _extract_section(memory_md, "Decisions")


def _extract_facts(memory_md: str) -> str:
    """Extract project identity and active patterns from MEMORY.md."""

    lines: list[str] = []
    skip_sections = {"Lessons", "Decisions", "稳定决策"}
    in_skip = False
    for line in memory_md.splitlines():
        if line.startswith("## "):
            title = line[3:].strip()
            in_skip = title in skip_sections
        if not in_skip:
            lines.append(line)
    return "\n".join(lines).strip()


def _collect_recent_logs(memory_dir: Path, days: int = 7) -> str:
    """Collect and concatenate the last N days of daily logs."""

    sessions = memory_dir / "sessions"
    if not sessions.exists():
        return ""
    sections: list[str] = []
    for path in sorted(sessions.glob("*.md"))[-days:]:
        contents = read_text_if_exists(path).strip()
        if contents:
            sections.append(f"## {path.stem}\n{contents}")
    return "\n\n".join(sections).strip()


def _read_auto_state(source_root: Path) -> str:
    """Read MEMORY_AUTO.md if it exists in the project workspace."""

    auto_paths = [
        source_root / ".codex" / "memory" / "MEMORY_AUTO.md",
        source_root / "MEMORY_AUTO.md",
    ]
    for path in auto_paths:
        if path.is_file():
            return read_text_if_exists(path).strip()
    return ""


def sync_project_to_hermes(source_root: Path, *, dry_run: bool = False) -> dict[str, Any]:
    """One-way sync: project memory to Hermes memories directory."""

    repo_root = source_root.resolve()
    workspace = workspace_name_from_root(repo_root)
    memory_dir = resolve_effective_memory_dir(workspace=workspace, repo_root=repo_root)
    memory_md_path = memory_dir / "MEMORY.md"
    if not memory_md_path.is_file():
        return {
            "status": "no_source",
            "message": f"{memory_md_path} not found",
            "files_written": [],
            "files_planned": [],
            "dry_run": dry_run,
        }
    memory_md = read_text_if_exists(memory_md_path)
    timestamp = current_local_timestamp()
    target_dir = HERMES_MEMORIES_DIR / safe_slug(workspace)
    output_files = {
        "project-memory.md": _extract_facts(memory_md),
        "project-lessons.md": _extract_lessons(memory_md),
        "project-decisions.md": _extract_decisions(memory_md),
        "daily-log.md": _collect_recent_logs(memory_dir),
        "auto-state.md": _read_auto_state(repo_root),
    }
    files_planned = [str((target_dir / name).resolve()) for name, content in output_files.items() if content.strip()]
    if dry_run:
        return {
            "status": "dry_run",
            "message": "Dry run - no files written",
            "files_written": [],
            "files_planned": files_planned,
            "dry_run": True,
            "files_total": len(files_planned),
            "source_memory_dir": str(memory_dir),
            "synced_at": timestamp,
        }
    target_dir.mkdir(parents=True, exist_ok=True)
    written: list[str] = []
    for name, content in output_files.items():
        if not content.strip():
            continue
        final = SYNC_HEADER_TEMPLATE.format(timestamp=timestamp) + content.strip() + "\n"
        path = target_dir / name
        if write_text_if_changed(path, final):
            written.append(str(path.resolve()))
    return {
        "status": "synced",
        "files_written": written,
        "files_planned": files_planned,
        "dry_run": False,
        "files_total": len(files_planned),
        "source_memory_dir": str(memory_dir),
        "synced_at": timestamp,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="One-way sync: project memory to Hermes memories directory.")
    parser.add_argument("--source-root", type=Path, default=None, help="Repository root (defaults to git root).")
    parser.add_argument("--dry-run", action="store_true", help="Print what would be synced without writing.")
    parser.add_argument("--json", action="store_true", dest="json_output", help="Output result as JSON.")
    args = parser.parse_args()
    result = sync_project_to_hermes((args.source_root or get_repo_root()).resolve(), dry_run=args.dry_run)
    if args.json_output:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print(result.get("message") or ", ".join(result.get("files_written", [])))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
