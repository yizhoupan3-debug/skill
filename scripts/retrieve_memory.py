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
from scripts.memory_support import (
    classify_runtime_continuity,
    evaluate_memory_freshness,
    get_repo_root,
    is_generic_query,
    query_matches_task,
    read_json_if_exists,
    read_memory_state,
    read_text_if_exists,
    refresh_memory_state_if_needed,
    resolve_effective_memory_dir,
    safe_slug,
    tokenize_query,
    load_runtime_snapshot,
)

SQLITE_FILENAMES = ("memory.sqlite3", "memory.db", ".memory.sqlite3")
STABLE_MEMORY_FILES = ("MEMORY.md", "preferences.md", "decisions.md", "lessons.md", "runbooks.md")
VALID_MODES = {"stable", "active", "history", "debug"}


def _normalize_for_match(text: str) -> str:
    return re.sub(r"\s+", " ", text).strip().lower()


def _topic_strong_match(topic: str, searchable: str) -> bool:
    topic_tokens = tokenize_query(topic)
    if not topic_tokens:
        return True
    token_hits = sum(token in searchable for token in topic_tokens)
    if token_hits == 0:
        return False
    exact_phrase = _normalize_for_match(topic) in searchable
    required_hits = len(topic_tokens) if len(topic_tokens) <= 2 else 2
    return exact_phrase or token_hits >= required_hits


def _extract_markdown_segments(text: str) -> list[tuple[tuple[str, ...], str]]:
    """Split markdown into small heading-aware segments."""

    segments: list[tuple[tuple[str, ...], str]] = []
    heading_stack: list[str] = []
    paragraph: list[str] = []

    def flush_paragraph() -> None:
        if paragraph:
            body = " ".join(part.strip() for part in paragraph if part.strip()).strip()
            if body:
                segments.append((tuple(heading_stack), body))
            paragraph.clear()

    for raw_line in text.splitlines():
        stripped = raw_line.strip()
        if not stripped:
            flush_paragraph()
            continue
        heading_match = re.match(r"^(#{1,6})\s+(.*)$", stripped)
        if heading_match:
            flush_paragraph()
            level = len(heading_match.group(1))
            title = heading_match.group(2).strip()
            if level == 1:
                heading_stack = []
                continue
            depth = max(0, level - 2)
            heading_stack = heading_stack[:depth]
            heading_stack.append(title)
            continue
        bullet_match = re.match(r"^(?:[-*]|\d+[.)])\s+(.*)$", stripped)
        if bullet_match:
            flush_paragraph()
            body = bullet_match.group(1).strip()
            if body:
                segments.append((tuple(heading_stack), body))
            continue
        paragraph.append(stripped)
    flush_paragraph()
    return segments


def _segment_score(headings: tuple[str, ...], body: str, topic: str) -> float:
    topic_tokens = tokenize_query(topic)
    if not topic_tokens:
        return 1.0
    searchable = _normalize_for_match(" ".join([*headings, body]))
    if not _topic_strong_match(topic, searchable):
        return 0.0
    token_hits = sum(token in searchable for token in topic_tokens)
    exact_phrase = _normalize_for_match(topic) in searchable
    coverage = token_hits / len(topic_tokens)
    heading_hits = sum(token in _normalize_for_match(" ".join(headings)) for token in topic_tokens)
    return (100.0 if exact_phrase else 0.0) + coverage * 10.0 + heading_hits * 2.0 + token_hits


def _render_segment(headings: tuple[str, ...], body: str) -> str:
    title = " / ".join(headings)
    if title:
        return f"### {title}\n- {body}".strip()
    return f"- {body}"


def _stable_fallback_sections_from_store(
    workspace: str,
    memory_workspace_root: Path,
    *,
    topic: str,
    max_items: int,
) -> list[tuple[str, str]]:
    """Use the structured memory index only as a narrow fallback."""

    if not topic.strip():
        return []
    sqlite_path = _workspace_sqlite_path(memory_workspace_root)
    if sqlite_path is None:
        return []
    store = MemoryStore(sqlite_path, workspace, ensure_schema=False)
    items = store.search_memory_items(topic, limit=max_items)
    lines: list[str] = []
    for item in items:
        summary = str(item.get("summary") or "").strip()
        if not summary:
            continue
        metadata = {}
        raw_metadata = item.get("metadata_json")
        if isinstance(raw_metadata, str):
            try:
                metadata = json.loads(raw_metadata)
            except json.JSONDecodeError:
                metadata = {}
        elif isinstance(raw_metadata, dict):
            metadata = raw_metadata
        headings = metadata.get("headings", [])
        normalized_headings = tuple(str(value).strip() for value in headings if str(value).strip())
        if _segment_score(normalized_headings, summary, topic) <= 0:
            continue
        heading_text = " / ".join(str(value).strip() for value in headings if str(value).strip())
        if heading_text:
            lines.extend([f"### {heading_text}", f"- {summary}", ""])
        else:
            lines.append(f"- {summary}")
    content = "\n".join(lines).strip()
    return [("memory/index.md", content)] if content else []


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
    items = (
        store.search_memory_items(topic, limit=max_items)
        if topic.strip()
        else store.list_memory_items(limit=max_items)
    )
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


def _stable_sections(
    workspace: str,
    memory_workspace_root: Path,
    *,
    topic: str,
    max_items: int,
) -> list[tuple[str, str]]:
    if not topic.strip():
        sections: list[tuple[str, str]] = []
        for name in STABLE_MEMORY_FILES:
            text = read_text_if_exists(memory_workspace_root / name).strip()
            if text:
                sections.append((name, text))
        return sections

    ranked: list[tuple[float, str, str]] = []
    for name in STABLE_MEMORY_FILES:
        text = read_text_if_exists(memory_workspace_root / name).strip()
        if not text:
            continue
        for headings, body in _extract_markdown_segments(text):
            score = _segment_score(headings, body, topic)
            if score <= 0:
                continue
            ranked.append((score, name, _render_segment(headings, body)))

    ranked.sort(key=lambda item: (-item[0], item[1], item[2]))
    deduped: list[tuple[str, str]] = []
    seen: set[tuple[str, str]] = set()
    for _, name, content in ranked:
        key = (name, content)
        if key in seen:
            continue
        seen.add(key)
        deduped.append((name, content))
        if len(deduped) >= max_items:
            break
    if deduped:
        return deduped
    return _stable_fallback_sections_from_store(
        workspace,
        memory_workspace_root,
        topic=topic,
        max_items=max_items,
    )


def _active_task_section(
    snapshot: Any,
    memory_workspace_root: Path,
    *,
    topic: str,
) -> tuple[tuple[str, str] | None, dict[str, Any]]:
    continuity = classify_runtime_continuity(snapshot)
    state = read_memory_state(memory_workspace_root)
    freshness = evaluate_memory_freshness(snapshot, state)
    if not continuity.get("current_execution"):
        return None, freshness
    if is_generic_query(topic):
        freshness = dict(freshness)
        freshness["state"] = "generic-query"
        freshness["active_task_allowed"] = False
        freshness["reasons"] = ["query is empty or generic"]
        return None, freshness
    task = str(continuity.get("task") or "")
    if not query_matches_task(topic, task):
        freshness = dict(freshness)
        freshness["state"] = "query-mismatch"
        freshness["active_task_allowed"] = False
        freshness["reasons"] = ["query does not target the active task"]
        return None, freshness
    if not freshness.get("active_task_allowed"):
        return None, freshness
    current = continuity["current_execution"]
    lines = [
        "# runtime:current_task",
        f"- task: {current.get('task', '')}",
        f"- phase: {current.get('phase', '')}",
        f"- status: {current.get('status', '')}",
    ]
    if current.get("route"):
        lines.append(f"- route: {' / '.join(current['route'])}")
    if current.get("next_actions"):
        lines.append(f"- next_actions: {' / '.join(current['next_actions'])}")
    if current.get("blockers"):
        lines.append(f"- blockers: {' / '.join(current['blockers'])}")
    if current.get("scope"):
        lines.append(f"- scope: {' / '.join(current['scope'])}")
    return ("runtime/current_task.md", "\n".join(lines)), freshness


def _archive_sections(memory_workspace_root: Path, *, topic: str, max_items: int) -> list[tuple[str, str]]:
    archive_root = memory_workspace_root / "archive"
    if not archive_root.is_dir():
        return []
    sections: list[tuple[str, str]] = []
    for path in sorted(archive_root.rglob("*")):
        if not path.is_file():
            continue
        rel_path = path.relative_to(memory_workspace_root)
        if path.suffix == ".json":
            text = json.dumps(read_json_if_exists(path), ensure_ascii=False, indent=2)
        else:
            text = read_text_if_exists(path)
        if topic.strip():
            matches = []
            for headings, body in _extract_markdown_segments(text):
                score = _segment_score(headings, body, topic)
                if score <= 0:
                    continue
                matches.append((score, _render_segment(headings, body)))
            matches.sort(key=lambda item: item[0], reverse=True)
            filtered = "\n\n".join(content for _, content in matches[:max_items]).strip()
        else:
            filtered = text.strip()
        if filtered.strip():
            sections.append((str(rel_path), filtered))
        if len(sections) >= max_items:
            break
    return sections


def render_context(
    *,
    workspace: str,
    topic: str = "",
    max_items: int = 8,
    memory_root: Path | None = None,
    repo_root: Path | None = None,
    artifact_root: Path | None = None,
    mode: str = "stable",
    task_id: str = "",
) -> dict[str, Any]:
    """Render memory context for prompt injection."""

    if mode not in VALID_MODES:
        raise ValueError(f"Unsupported memory recall mode: {mode}")
    repo_root = repo_root.resolve() if repo_root is not None else None
    memory_workspace_root = resolve_effective_memory_dir(
        workspace=workspace,
        memory_root=memory_root,
        repo_root=repo_root,
    )
    memory_workspace_root.mkdir(parents=True, exist_ok=True)
    sections = _stable_sections(
        workspace,
        memory_workspace_root,
        topic=topic,
        max_items=max_items,
    )
    snapshot = load_runtime_snapshot(
        repo_root or get_repo_root(),
        artifact_root=artifact_root,
        task_id=task_id or None,
    )
    refresh_memory_state_if_needed(snapshot, memory_workspace_root)
    active_section = None
    freshness = {
        "state": "not-requested",
        "active_task_allowed": False,
        "reasons": [],
    }
    if mode in {"active", "history", "debug"}:
        active_section, freshness = _active_task_section(snapshot, memory_workspace_root, topic=topic)
        if active_section is not None:
            sections.append(active_section)
    if mode in {"history", "debug"}:
        sections.extend(_archive_sections(memory_workspace_root, topic=topic, max_items=max_items))
    if mode == "debug":
        sqlite_path = _workspace_sqlite_path(memory_workspace_root)
        if sqlite_path is not None:
            sections.extend(_render_sqlite_sections(workspace, sqlite_path, topic=topic, max_items=max_items))
        state_payload = read_memory_state(memory_workspace_root)
        if state_payload:
            sections.append(("state.json", json.dumps(state_payload, ensure_ascii=False, indent=2)))
    blocks = [f"## {path}\n{content.strip()}" for path, content in sections if content.strip()]
    sqlite_path = _workspace_sqlite_path(memory_workspace_root)
    continuity = classify_runtime_continuity(snapshot)
    return {
        "workspace": workspace,
        "topic": topic,
        "mode": mode,
        "memory_root": str(memory_workspace_root),
        "sqlite_path": str(sqlite_path) if sqlite_path else "",
        "items": [{"path": path, "content": content} for path, content in sections],
        "context": "\n\n".join(blocks).strip(),
        "active_task_included": active_section is not None,
        "freshness": freshness,
        "continuity_state": continuity.get("state"),
        "active_task_id": snapshot.active_task_id,
    }


def main() -> int:
    """CLI entry point."""

    parser = argparse.ArgumentParser(description="Retrieve workspace memory context.")
    parser.add_argument("--workspace", required=True)
    parser.add_argument("--topic", default="")
    parser.add_argument("--top", type=int, default=8, dest="max_items")
    parser.add_argument("--mode", choices=sorted(VALID_MODES), default="stable")
    parser.add_argument("--json", action="store_true", dest="json_output")
    args = parser.parse_args()
    result = render_context(
        workspace=args.workspace,
        topic=args.topic,
        max_items=args.max_items,
        repo_root=get_repo_root(),
        mode=args.mode,
    )
    if args.json_output:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print(result["context"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
