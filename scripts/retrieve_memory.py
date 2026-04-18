#!/usr/bin/env python3
"""Retrieve workspace memory and render prompt-injectable context."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.memory_store import MemoryStore
from scripts.memory_support import get_repo_root, read_text_if_exists, resolve_effective_memory_dir

SQLITE_FILENAMES = ("memory.sqlite3", "memory.db", ".memory.sqlite3")
STABLE_MEMORY_FILES = ("MEMORY.md", "preferences.md", "decisions.md", "lessons.md", "runbooks.md")
VALID_MODES = {"stable", "active", "history", "debug"}


def _tokenizer(topic: str) -> list[str]:
    return [part.lower() for part in re.split(r"[\s,/|]+", topic) if part.strip()]


def _section_score(text: str, topic_tokens: list[str]) -> int:
    lowered = text.lower()
    return sum(token in lowered for token in topic_tokens)


def _filter_text(text: str, topic: str, max_items: int) -> str:
    if not text.strip():
        return ""
    topic_tokens = _tokenizer(topic)
    if not topic_tokens:
        return text.strip()
    blocks: list[tuple[int, str]] = []
    current_lines: list[str] = []
    for line in text.splitlines():
        if line.strip().startswith("## "):
            if current_lines:
                block = "\n".join(current_lines).strip()
                blocks.append((_section_score(block, topic_tokens), block))
            current_lines = [line]
            continue
        if not current_lines:
            current_lines = [line]
        else:
            current_lines.append(line)
    if current_lines:
        block = "\n".join(current_lines).strip()
        blocks.append((_section_score(block, topic_tokens), block))
    kept = [block for score, block in sorted(blocks, key=lambda item: item[0], reverse=True) if score > 0]
    deduped: list[str] = []
    seen: set[str] = set()
    for block in kept[:max_items]:
        if block in seen:
            continue
        seen.add(block)
        deduped.append(block)
    return "\n\n".join(deduped).strip()


def _workspace_sqlite_path(memory_workspace_root: Path) -> Path | None:
    for candidate in (memory_workspace_root / name for name in SQLITE_FILENAMES):
        if candidate.is_file():
            return candidate
    return None


def _render_sqlite_sections(
    workspace: str,
    db_path: Path,
    *,
    topic: str,
    max_items: int,
) -> list[tuple[str, str]]:
    store = MemoryStore(db_path, workspace, ensure_schema=False)
    items = store.search_memory_items(topic, limit=max_items) if topic.strip() else store.list_memory_items(limit=max_items)
    sections: list[tuple[str, str]] = []
    if items:
        lines = ["# sqlite:memory_items"]
        for index, item in enumerate(items, start=1):
            lines.extend(
                [
                    f"### {item.get('summary') or f'item-{index}'}",
                    f"- source: {item.get('source', '')}",
                    f"- category: {item.get('category', '')}",
                    f"- status: {item.get('status', '')}",
                    f"- summary: {item.get('summary', '')}",
                    f"- notes: {item.get('notes', '')}",
                    "",
                ]
            )
        sections.append(("sqlite/memory_items.md", "\n".join(lines).strip()))
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
        sections.append(("sqlite/session_notes.md", "\n".join(lines).strip()))
    evidence = store.list_evidence(limit=max_items)
    if evidence:
        lines = ["# sqlite:evidence_records"]
        for artifact in evidence:
            lines.extend(
                [
                    f"### {artifact.get('path', '') or artifact.get('kind', '') or 'evidence'}",
                    f"- kind: {artifact.get('kind', '')}",
                    f"- path: {artifact.get('path', '')}",
                    f"- content: {artifact.get('content', '')}",
                    "",
                ]
            )
        sections.append(("sqlite/evidence_records.md", "\n".join(lines).strip()))
    return sections


def _latest_session_section(memory_workspace_root: Path, *, topic: str, max_items: int) -> tuple[str, str] | None:
    sessions_root = memory_workspace_root / "sessions"
    session_paths = sorted(sessions_root.glob("*.md")) if sessions_root.exists() else []
    if not session_paths:
        return None
    latest_path = session_paths[-1]
    latest_text = read_text_if_exists(latest_path).strip()
    if not latest_text:
        return None
    return (
        f"sessions/{latest_path.name}",
        _filter_text(latest_text, topic, max_items) if topic else latest_text,
    )


def render_context(
    *,
    workspace: str,
    topic: str = "",
    max_items: int = 8,
    memory_root: Path | None = None,
    repo_root: Path | None = None,
    mode: str = "stable",
) -> dict[str, Any]:
    """Render memory context for prompt injection."""

    if mode not in VALID_MODES:
        raise ValueError(f"Unsupported memory recall mode: {mode}")

    memory_workspace_root = resolve_effective_memory_dir(
        workspace=workspace,
        memory_root=memory_root,
        repo_root=repo_root,
    )
    memory_workspace_root.mkdir(parents=True, exist_ok=True)
    sqlite_path = _workspace_sqlite_path(memory_workspace_root)
    sections: list[tuple[str, str]] = []

    for name in STABLE_MEMORY_FILES:
        text = read_text_if_exists(memory_workspace_root / name).strip()
        if not text:
            continue
        filtered = _filter_text(text, topic, max_items) if topic else text
        if filtered:
            sections.append((name, filtered))

    if mode in {"active", "history", "debug"}:
        latest_session = _latest_session_section(memory_workspace_root, topic=topic, max_items=max_items)
        if latest_session is not None:
            sections.append(latest_session)

    if mode == "debug" and sqlite_path is not None:
        sections.extend(_render_sqlite_sections(workspace, sqlite_path, topic=topic, max_items=max_items))

    blocks = [f"## {path}\n{content.strip()}" for path, content in sections if content.strip()]
    return {
        "workspace": workspace,
        "topic": topic,
        "mode": mode,
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
    result = render_context(
        workspace=args.workspace,
        topic=args.topic,
        max_items=args.max_items,
        repo_root=get_repo_root(),
    )
    if args.json_output:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print(result["context"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
