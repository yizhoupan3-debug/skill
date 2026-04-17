#!/usr/bin/env python3
"""Retrieve workspace memory and render prompt-injectable context."""

from __future__ import annotations

import argparse
import json
import re
import sqlite3
import sys
from pathlib import Path
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.memory_store import MemoryStore
from scripts.memory_support import (
    DEFAULT_MEMORY_ROOT,
    get_repo_root,
    read_text_if_exists,
    resolve_effective_memory_dir,
    safe_slug,
    workspace_dir,
)

SQLITE_FILENAMES = ("memory.sqlite3", "memory.db", ".memory.sqlite3")
CURRENT_STATE_CATEGORIES = {"task_state", "blocker", "constraint"}


def _tokenizer(topic: str) -> list[str]:
    return [part.lower() for part in re.split(r"[\s,/|]+", topic) if part.strip()]


def _section_score(text: str, topic_tokens: list[str]) -> int:
    line_lowers = text.lower()
    return sum(token in line_lowers for token in topic_tokens)


def _filter_text(text: str, topic: str, max_items: int) -> str:
    if not text.strip():
        return ""
    topic_tokens = _tokenizer(topic)
    if not topic_tokens:
        return text.strip()
    blocks: list[tuple[int, str]] = []
    current_heading = ""
    current_lines: list[str] = []
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("## "):
            if current_lines:
                block = "\n".join(current_lines).strip()
                blocks.append((_section_score(block, topic_tokens), block))
            current_heading = line
            current_lines = [current_heading]
            continue
        if not current_lines:
            current_lines = [line]
        else:
            current_lines.append(line)
    if current_lines:
        block = "\n".join(current_lines).strip()
        blocks.append((_section_score(block, topic_tokens), block))
    kept_blocks = [block for score, block in sorted(blocks, key=lambda x: x[0], reverse=True) if score > 0]
    deduped: list[str] = []
    seen: set[str] = set()
    for item in kept_blocks[:max_items]:
        if item not in seen:
            seen.add(item)
            deduped.append(item)
    return "\n\n".join(deduped).strip()


def _workspace_sqlite_candidates(memory_workspace_root: Path) -> list[Path]:
    """Return likely SQLite database paths for a workspace."""

    return [memory_workspace_root / name for name in SQLITE_FILENAMES]


def _workspace_sqlite_path(memory_workspace_root: Path) -> Path | None:
    """Return the first existing SQLite database path for a workspace."""

    for candidate in _workspace_sqlite_candidates(memory_workspace_root):
        if candidate.is_file():
            return candidate
    return None


def _textify(value: Any) -> str:
    """Convert SQLite values to readable text."""

    if isinstance(value, bytes):
        return value.decode("utf-8", errors="replace")
    if isinstance(value, (dict, list, tuple)):
        try:
            return json.dumps(value, ensure_ascii=False, sort_keys=True)
        except TypeError:
            return str(value)
    return str(value)


def _is_workspace_scoped(row: dict[str, Any], workspace: str) -> bool:
    """Check whether a row is scoped to the current workspace."""

    workspace_slug = safe_slug(workspace)
    workspace_basename = Path(workspace).name
    scope_fields = (
        row.get("workspace"),
        row.get("namespace"),
        row.get("project"),
        row.get("project_name"),
    )
    values = {str(value).lower() for value in scope_fields if value}
    if not values:
        return True
    return workspace.lower() in values or workspace_slug.lower() in values or workspace_basename.lower() in values


def _sqlite_columns(connection: sqlite3.Connection, table_name: str) -> list[str]:
    cursor = connection.execute(f'PRAGMA table_info("{table_name}")')
    return [row[1] for row in cursor.fetchall()]


def _sqlite_user_tables(connection: sqlite3.Connection) -> list[str]:
    rows = connection.execute(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
    ).fetchall()
    return [row[0] for row in rows]


def _sqlite_row_text(row: dict[str, Any]) -> str:
    preferred = ("title", "summary", "content", "body", "notes", "prompt", "kind", "category", "source", "status")
    parts = [_textify(row.get(key)) for key in preferred if row.get(key) not in (None, "")]
    for key, value in row.items():
        if key not in preferred and value not in (None, ""):
            parts.append(f"{key}: {_textify(value)}")
    return "\n".join(parts)


def _sqlite_row_title(row: dict[str, Any], fallback_index: int) -> str:
    for key in ("title", "summary", "kind", "category", "source", "subject"):
        value = str(row.get(key) or "").strip()
        if value:
            return value.splitlines()[0][:120]
    return f"row-{fallback_index}"


def _sqlite_row_score(row: dict[str, Any], topic_tokens: list[str]) -> int:
    lowered = _sqlite_row_text(row).lower()
    return sum(token in lowered for token in topic_tokens)


def _split_current_state_items(
    items: list[dict[str, Any]],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], list[dict[str, Any]]]:
    current_state: list[dict[str, Any]] = []
    state_history: list[dict[str, Any]] = []
    general: list[dict[str, Any]] = []
    for item in items:
        category = str(item.get("category") or "").strip()
        source = str(item.get("source") or "").strip()
        if category in CURRENT_STATE_CATEGORIES:
            current_state.append(item)
        elif "current_state_history" in source or "task_state_history" in source:
            state_history.append(item)
        else:
            general.append(item)
    return current_state, state_history, general


def _load_sqlite_sections(
    workspace: str,
    db_path: Path,
    *,
    topic: str = "",
    max_items: int = 8,
) -> list[dict[str, str]]:
    """Read structured memory data from the workspace SQLite database."""

    # Retrieval is read-only here; the database already exists if we reached this path.
    store = MemoryStore(db_path, workspace, ensure_schema=False)
    items = store.search_memory_items(topic, limit=max_items * 4) if topic else store.list_memory_items(limit=max_items * 4)
    current_state_items, fallback_state_items, general_items = _split_current_state_items(items)
    remaining_slots = max_items
    sections: list[dict[str, str]] = []
    if current_state_items:
        lines = ["# sqlite:current_state"]
        for item in current_state_items[:remaining_slots]:
            lines.extend(
                [
                    f"### {_sqlite_row_title(item, 0)}",
                    f"- category: {item.get('category', '')}",
                    f"- confidence: {item.get('confidence', '')}",
                    f"- summary: {item.get('summary', '')}",
                    f"- notes: {item.get('notes', '')}",
                    "",
                ]
            )
        sections.append({"path": "sqlite/current_state.md", "content": "\n".join(lines).strip()})
        remaining_slots = max(0, remaining_slots - len(current_state_items[:remaining_slots]))
    if remaining_slots > 0 and fallback_state_items:
        current_state_items.extend(fallback_state_items[:remaining_slots])
    if general_items:
        lines = ["# sqlite:memory_items"]
        topic_tokens = _tokenizer(topic)
        ranked = sorted(general_items, key=lambda row: _sqlite_row_score(row, topic_tokens), reverse=True)
        for index, item in enumerate(ranked[:max_items], start=1):
            lines.extend(
                [
                    f"### {_sqlite_row_title(item, index)}",
                    f"- source: {item.get('source', '')}",
                    f"- status: {item.get('status', '')}",
                    f"- summary: {item.get('summary', '')}",
                    f"- notes: {item.get('notes', '')}",
                    "",
                ]
            )
        sections.append({"path": "sqlite/memory_items.md", "content": "\n".join(lines).strip()})
    notes = store.list_recent_session_notes(limit=max_items)
    if notes:
        lines = ["# sqlite:session_notes"]
        for note in notes:
            lines.extend(
                [
                    f"### {note.get('session_key', '')}#{note.get('position', '')}",
                    f"- note_type: {note.get('note_type', '')}",
                    f"- updated_at: {note.get('updated_at', '')}",
                    f"- note: {note.get('note', '')}",
                    "",
                ]
            )
        sections.append({"path": "sqlite/session_notes.md", "content": "\n".join(lines).strip()})
    evidence = store.list_evidence(limit=max_items)
    if evidence:
        lines = ["# sqlite:evidence_records"]
        for artifact in evidence:
            lines.extend(
                [
                    f"### {_sqlite_row_title(artifact, 0)}",
                    f"- kind: {artifact.get('kind', '')}",
                    f"- path: {artifact.get('path', '')}",
                    f"- content: {artifact.get('content', '')}",
                    "",
                ]
            )
        sections.append({"path": "sqlite/evidence_records.md", "content": "\n".join(lines).strip()})
    return sections


def render_context(
    *,
    workspace: str,
    topic: str = "",
    max_items: int = 8,
    memory_root: Path | None = None,
    repo_root: Path | None = None,
) -> dict[str, Any]:
    """Render memory context for prompt injection."""

    memory_workspace_root = resolve_effective_memory_dir(workspace=workspace, memory_root=memory_root, repo_root=repo_root)
    memory_workspace_root.mkdir(parents=True, exist_ok=True)
    sections: list[tuple[str, str]] = []
    sqlite_path = _workspace_sqlite_path(memory_workspace_root)
    if sqlite_path:
        for section in _load_sqlite_sections(workspace, sqlite_path, topic=topic, max_items=max_items):
            sections.append((section["path"], section["content"]))
    memory_md = read_text_if_exists(memory_workspace_root / "MEMORY.md")
    if memory_md:
        filtered = _filter_text(memory_md, topic, max_items) if topic else memory_md.strip()
        if filtered:
            sections.append(("MEMORY.md", filtered))
    for name in ("preferences.md", "decisions.md", "lessons.md", "runbooks.md"):
        text = read_text_if_exists(memory_workspace_root / name).strip()
        if text:
            sections.append((name, _filter_text(text, topic, max_items) if topic else text))
    sessions_root = memory_workspace_root / "sessions"
    latest_path = sorted(sessions_root.glob("*.md"))[-1] if sessions_root.exists() and list(sessions_root.glob("*.md")) else None
    if latest_path:
        latest_text = read_text_if_exists(latest_path).strip()
        if latest_text:
            sections.append((f"sessions/{latest_path.name}", _filter_text(latest_text, topic, max_items) if topic else latest_text))
    blocks = [f"## {path}\n{content.strip()}" for path, content in sections if content.strip()]
    return {
        "workspace": workspace,
        "topic": topic,
        "memory_root": str(memory_workspace_root),
        "sqlite_path": str(sqlite_path) if sqlite_path else "",
        "items": [{"path": path, "content": content} for path, content in sections],
        "context": "\n\n".join(blocks).strip(),
    }


def main() -> int:
    """CLI entry point."""

    parser = argparse.ArgumentParser(description="Retrieve workspace memory context.")
    parser.add_argument("--workspace", required=True)
    parser.add_argument("--topic", default="")
    parser.add_argument("--top", type=int, default=8, dest="max_items")
    parser.add_argument("--json", action="store_true", dest="json_output")
    args = parser.parse_args()
    result = render_context(workspace=args.workspace, topic=args.topic, max_items=args.max_items, repo_root=get_repo_root())
    if args.json_output:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print(result["context"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
