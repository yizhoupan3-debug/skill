#!/usr/bin/env python3
"""Retrieve workspace memory and render prompt-injectable context."""

from __future__ import annotations

import argparse
import json
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
    resolve_effective_memory_dir,
    safe_slug,
    tokenize_query,
    load_runtime_snapshot,
)

SQLITE_FILENAMES = ("memory.sqlite3", "memory.db", ".memory.sqlite3")
STABLE_MEMORY_FILES = ("MEMORY.md", "preferences.md", "decisions.md", "lessons.md", "runbooks.md")
VALID_MODES = {"stable", "active", "history", "debug"}


def _section_score(text: str, topic_tokens: list[str]) -> int:
    line_lowers = text.lower()
    return sum(token in line_lowers for token in topic_tokens)


def _filter_text(text: str, topic: str, max_items: int) -> str:
    if not text.strip():
        return ""
    topic_tokens = tokenize_query(topic)
    if not topic_tokens:
        return text.strip()
    blocks: list[tuple[int, str]] = []
    current_lines: list[str] = []
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("## "):
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
    kept_blocks = [block for score, block in sorted(blocks, key=lambda x: x[0], reverse=True) if score > 0]
    deduped: list[str] = []
    seen: set[str] = set()
    for item in kept_blocks[:max_items]:
        if item not in seen:
            seen.add(item)
            deduped.append(item)
    return "\n\n".join(deduped).strip()


def _workspace_sqlite_path(memory_workspace_root: Path) -> Path | None:
    for candidate in (memory_workspace_root / name for name in SQLITE_FILENAMES):
        if candidate.is_file():
            return candidate
    return None


def _render_sqlite_memory_items(
    workspace: str,
    db_path: Path,
    *,
    topic: str,
    max_items: int,
) -> tuple[str, str] | None:
    store = MemoryStore(db_path, workspace, ensure_schema=False)
    items = (
        store.search_memory_items(topic, limit=max_items)
        if topic.strip()
        else store.list_memory_items(limit=max_items)
    )
    if not items:
        return None
    lines = ["# sqlite:memory_items"]
    for index, item in enumerate(items, start=1):
        lines.extend(
            [
                f"### {item.get('summary') or f'item-{index}'}",
                f"- source: {item.get('source', '')}",
                f"- category: {item.get('category', '')}",
                f"- status: {item.get('status', '')}",
                f"- summary: {item.get('summary', '')}",
                "",
            ]
        )
    return "sqlite/memory_items.md", "\n".join(lines).strip()


def _stable_sections(
    workspace: str,
    memory_workspace_root: Path,
    *,
    topic: str,
    max_items: int,
) -> list[tuple[str, str]]:
    sections: list[tuple[str, str]] = []
    sqlite_path = _workspace_sqlite_path(memory_workspace_root)
    if sqlite_path:
        section = _render_sqlite_memory_items(workspace, sqlite_path, topic=topic, max_items=max_items)
        if section is not None:
            sections.append(section)
    for name in STABLE_MEMORY_FILES:
        text = read_text_if_exists(memory_workspace_root / name).strip()
        if not text:
            continue
        filtered = _filter_text(text, topic, max_items) if topic.strip() else text
        if filtered.strip():
            sections.append((name, filtered))
    return sections


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
        filtered = _filter_text(text, topic, max_items) if topic.strip() else text.strip()
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
    snapshot = load_runtime_snapshot(repo_root or get_repo_root(), artifact_root=artifact_root)
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
