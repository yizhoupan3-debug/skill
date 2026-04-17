#!/usr/bin/env python3
"""Project-local Claude memory bridge backed by shared runtime artifacts."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.consolidate_memory import build_memory_documents, persist_memory_bundle, write_documents
from scripts.memory_support import (
    current_local_timestamp,
    get_repo_root,
    load_runtime_snapshot,
    normalize_next_actions,
    normalize_trace_skills,
    parse_session_summary,
    read_text_if_exists,
    resolve_effective_memory_dir,
    stable_line_items,
    supervisor_contract,
    write_text_if_changed,
)

CLAUDE_MEMORY_PATH = Path(".codex") / "memory" / "CLAUDE_MEMORY.md"
DEFAULT_MAX_LINES = 6


def _extract_section(text: str, headings: tuple[str, ...]) -> str:
    for heading in headings:
        pattern = re.compile(rf"^##\s+{re.escape(heading)}\s*\n(.*?)(?=^##\s|\Z)", re.MULTILINE | re.DOTALL)
        match = pattern.search(text)
        if match:
            return match.group(1).strip()
    return ""


def _extract_bullets(text: str, *, limit: int) -> list[str]:
    items: list[str] = []
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped.startswith("- "):
            continue
        value = stripped[2:].strip()
        if value:
            items.append(value)
    return stable_line_items(items)[:limit]


def _markdown_block(title: str, items: list[str]) -> str:
    if not items:
        items = ["暂无"]
    return "\n".join([f"## {title}", "", *[f"- {item}" for item in items], ""])


def _current_state_lines(repo_root: Path) -> list[str]:
    snapshot = load_runtime_snapshot(repo_root)
    summary = parse_session_summary(snapshot.session_summary_text)
    route = ", ".join(normalize_trace_skills(snapshot.trace_metadata))
    contract = supervisor_contract(snapshot.supervisor_state)
    blockers = snapshot.supervisor_state.get("blockers", {}).get("open_blockers", [])
    lines = [
        f"task: {summary.get('task') or snapshot.supervisor_state.get('task_summary', '未记录')}",
        f"phase: {summary.get('phase') or snapshot.supervisor_state.get('active_phase', '未记录')}",
        "status: "
        + (
            snapshot.supervisor_state.get("verification", {}).get("verification_status")
            or summary.get("status")
            or "in_progress"
        ),
        f"route: {route}" if route else "",
        f"next_actions: {' / '.join(normalize_next_actions(snapshot.next_actions))}"
        if normalize_next_actions(snapshot.next_actions)
        else "",
        f"blockers: {' / '.join(str(item) for item in blockers)}" if blockers else "",
        f"scope: {' / '.join(str(item) for item in contract.get('scope', []))}" if contract.get("scope") else "",
        f"acceptance: {' / '.join(str(item) for item in contract.get('acceptance_criteria', []))}"
        if contract.get("acceptance_criteria")
        else "",
    ]
    return [line for line in lines if line]


def build_claude_memory_projection(
    repo_root: Path,
    *,
    max_lines: int = DEFAULT_MAX_LINES,
) -> str:
    """Render a concise Claude-friendly projection from shared memory and artifacts."""

    memory_dir = resolve_effective_memory_dir(repo_root=repo_root)
    memory_md = read_text_if_exists(memory_dir / "MEMORY.md")
    stable_patterns = _extract_bullets(
        _extract_section(memory_md, ("Active Patterns", "项目约定", "稳定事实")),
        limit=max_lines,
    )
    stable_decisions = _extract_bullets(
        _extract_section(memory_md, ("稳定决策", "Decisions")),
        limit=max_lines,
    )
    lessons = _extract_bullets(
        _extract_section(memory_md, ("Lessons", "经验教训")),
        limit=max_lines,
    )
    artifact_paths = [
        "`artifacts/current/SESSION_SUMMARY.md`",
        "`artifacts/current/NEXT_ACTIONS.json`",
        "`artifacts/current/EVIDENCE_INDEX.json`",
        "`artifacts/current/TRACE_METADATA.json`",
        "`.supervisor_state.json`",
        "`./.codex/memory/`",
    ]
    lines = [
        "# Claude Shared Memory Projection",
        "",
        "_Generated from shared runtime artifacts and `./.codex/memory/`. Do not edit manually._",
        "",
        f"- generated_at: {current_local_timestamp()}",
        f"- repo_root: `{repo_root}`",
        "",
        _markdown_block("Current Execution State", _current_state_lines(repo_root)).rstrip(),
        "",
        _markdown_block("Stable Project Patterns", stable_patterns).rstrip(),
        "",
        _markdown_block("Stable Decisions", stable_decisions).rstrip(),
        "",
        _markdown_block("Recent Lessons", lessons).rstrip(),
        "",
        _markdown_block("Artifact Anchors", artifact_paths).rstrip(),
    ]
    return "\n".join(lines).strip() + "\n"


def sync_claude_memory_projection(
    repo_root: Path,
    *,
    max_lines: int = DEFAULT_MAX_LINES,
) -> dict[str, Any]:
    """Write the Claude memory projection into the shared project memory directory."""

    target = repo_root / CLAUDE_MEMORY_PATH
    content = build_claude_memory_projection(repo_root, max_lines=max_lines)
    changed = write_text_if_changed(target, content)
    return {
        "status": "updated" if changed else "unchanged",
        "target_path": str(target),
        "changed": changed,
    }


def consolidate_shared_memory(repo_root: Path) -> dict[str, Any]:
    """Refresh the shared project memory bundle from current runtime artifacts."""

    snapshot = load_runtime_snapshot(repo_root)
    workspace = repo_root.name
    resolved_dir = resolve_effective_memory_dir(workspace=workspace, repo_root=repo_root)
    documents = build_memory_documents(workspace=workspace, snapshot=snapshot)
    changed_files = write_documents(documents, resolved_dir)
    sqlite_result = persist_memory_bundle(workspace, documents, resolved_dir=resolved_dir)
    return {
        "memory_root": str(resolved_dir),
        "changed_files": changed_files,
        "sqlite_result": sqlite_result,
    }


def run_bridge(command: str, repo_root: Path, *, max_lines: int = DEFAULT_MAX_LINES) -> dict[str, Any]:
    """Run one lifecycle bridge command."""

    result: dict[str, Any] = {
        "command": command,
        "repo_root": str(repo_root),
    }
    if command == "session-end":
        result["consolidation"] = consolidate_shared_memory(repo_root)
    result["projection"] = sync_claude_memory_projection(repo_root, max_lines=max_lines)
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description="Bridge shared project memory into Claude-readable context.")
    parser.add_argument(
        "command",
        choices=("sync", "session-start", "session-stop", "session-end"),
        help="Lifecycle action to run.",
    )
    parser.add_argument("--repo-root", type=Path, default=None, help="Repository root. Defaults to the detected git root.")
    parser.add_argument("--max-lines", type=int, default=DEFAULT_MAX_LINES, help="Maximum bullets per section.")
    parser.add_argument("--json", action="store_true", dest="json_output", help="Emit JSON.")
    args = parser.parse_args()

    repo_root = (args.repo_root or get_repo_root()).resolve()
    command = "sync" if args.command == "sync" else args.command
    result = run_bridge(command, repo_root, max_lines=args.max_lines)
    if args.json_output:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print(result["projection"]["target_path"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
