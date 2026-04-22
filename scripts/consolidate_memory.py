#!/usr/bin/env python3
"""Consolidate short-term execution artifacts into stable long-term memory files."""

from __future__ import annotations

import argparse
import json
import re
import shutil
import sys
from pathlib import Path
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.memory_store import MemoryItem, open_workspace_store
from scripts.memory_support import (
    build_memory_state,
    current_local_date,
    current_local_timestamp,
    load_runtime_snapshot,
    memory_state_path,
    read_text_if_exists,
    resolve_effective_memory_dir,
    safe_slug,
    write_json_if_changed,
    write_text_if_changed,
)

STABLE_DOCUMENTS = ("MEMORY.md", "preferences.md", "decisions.md", "lessons.md", "runbooks.md")


def _extract_bullet_lines(raw: str) -> list[str]:
    items: list[str] = []
    for line in raw.splitlines():
        if line.startswith("- "):
            items.append(line[2:].strip())
    return items


def _extract_memory_segments(raw: str) -> list[tuple[tuple[str, ...], str]]:
    """Split markdown into heading-scoped memory segments."""

    segments: list[tuple[tuple[str, ...], str]] = []
    heading_stack: list[str] = []
    paragraph: list[str] = []

    def flush_paragraph() -> None:
        if not paragraph:
            return
        body = " ".join(part.strip() for part in paragraph if part.strip()).strip()
        paragraph.clear()
        if not body or (body.startswith("_") and body.endswith("_")):
            return
        segments.append((tuple(heading_stack), body))

    for raw_line in raw.splitlines():
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


def _memory_category_for_file(file_name: str) -> str:
    return {
        "MEMORY.md": "invariant",
        "preferences.md": "preference",
        "decisions.md": "decision",
        "lessons.md": "lesson",
        "runbooks.md": "runbook",
    }.get(file_name, "general")


def _memory_item_id(workspace: str, category: str, index: int, summary: str, fallback: str) -> str:
    summary_slug = safe_slug(summary[:80], fallback=fallback)
    return f"{safe_slug(workspace)}:{category}:{index}:{summary_slug}"


def _default_memory_md(repo_root: Path) -> str:
    return "\n".join(
        [
            "# 项目长期记忆",
            "",
            "_本文件沉淀跨会话稳定的项目事实、决策与约定。当前任务态以 continuity artifacts 为准；历史/debug 归档到 `memory/archive/`。_",
            "",
            "## 项目身份",
            "",
            f"- **仓库**: `{repo_root}`",
            "- **闭环事实源**: `artifacts/current/<task_id>/` + `artifacts/current/active_task.json` + `.supervisor_state.json`",
            "- **默认召回策略**: 稳定层优先，仅在 query 明确命中 active task 且 freshness gate 通过时追加当前任务态",
            "- **Artifact 分层**: `artifacts/bootstrap/` / `artifacts/ops/memory_automation/` / `artifacts/evidence/` / `artifacts/scratch/`",
            "",
        ]
    ) + "\n"


def _default_runbooks() -> str:
    return "\n".join(
        [
            "# runbooks",
            "",
            "## 标准操作",
            "",
            "- 统一维护入口：python3 scripts/run_memory_automation.py --workspace <workspace>",
            "- 需要迁移旧 artifact 布局时显式执行：python3 scripts/run_memory_automation.py --workspace <workspace> --apply-artifact-migrations",
            "- 合并稳定记忆：python3 scripts/consolidate_memory.py --workspace <workspace>",
            "- 召回上下文：python3 scripts/retrieve_memory.py --workspace <workspace> --mode stable|active|history|debug --topic <关键词>",
            "- 生命周期收口：python3 scripts/router_rs_runner.py --claude-hook-command session-end --repo-root <repo_root> --claude-hook-max-lines 4",
            "- 诊断快照与存储审计查看 `artifacts/ops/memory_automation/<run_id>/`，不再从 MEMORY_AUTO 或 sessions 读取。",
            "",
        ]
    ) + "\n"


def _load_stable_documents(repo_root: Path, resolved_dir: Path) -> dict[str, str]:
    documents: dict[str, str] = {}
    documents["MEMORY.md"] = read_text_if_exists(resolved_dir / "MEMORY.md") or _default_memory_md(repo_root)
    documents["preferences.md"] = read_text_if_exists(resolved_dir / "preferences.md") or "# preferences\n"
    documents["decisions.md"] = read_text_if_exists(resolved_dir / "decisions.md") or "# decisions\n"
    documents["lessons.md"] = read_text_if_exists(resolved_dir / "lessons.md") or "# lessons\n"
    documents["runbooks.md"] = read_text_if_exists(resolved_dir / "runbooks.md") or _default_runbooks()
    return documents


def _move_to_archive(source: Path, destination: Path) -> str:
    destination.parent.mkdir(parents=True, exist_ok=True)
    if destination.exists():
        suffix = current_local_timestamp().replace(":", "").replace("+", "_")
        destination = destination.with_name(f"{destination.stem}-{suffix}{destination.suffix}")
    shutil.move(str(source), str(destination))
    return str(destination)


def archive_legacy_memory_bundle(
    workspace: str,
    resolved_dir: Path,
    *,
    memory_root: Path | None = None,
) -> dict[str, Any]:
    """Archive legacy prompt-visible memory surfaces before the strong cutover."""

    store = open_workspace_store(workspace, memory_root=memory_root, resolved_dir=resolved_dir)
    archive_root = resolved_dir / "archive" / f"pre-cutover-{current_local_date()}"
    archived_paths: list[str] = []
    for legacy_name in ("MEMORY_AUTO.md",):
        legacy_path = resolved_dir / legacy_name
        if legacy_path.exists():
            archived_paths.append(_move_to_archive(legacy_path, archive_root / legacy_name))
    sessions_dir = resolved_dir / "sessions"
    if sessions_dir.exists():
        archived_paths.append(_move_to_archive(sessions_dir, archive_root / "sessions"))
    legacy_rows = store.export_legacy_rows()
    legacy_memory_items = store.export_memory_items_excluding_sources(list(STABLE_DOCUMENTS))
    legacy_row_count = sum(len(rows) for rows in legacy_rows.values())
    legacy_memory_item_count = len(legacy_memory_items)
    dump_path = archive_root / "sqlite_legacy_dump.json"
    if legacy_row_count or legacy_memory_item_count:
        write_json_if_changed(
            dump_path,
            {
                "schema_version": "memory-legacy-dump-v1",
                "exported_at": current_local_timestamp(),
                "workspace": workspace,
                "memory_items": legacy_memory_items,
                **legacy_rows,
            },
        )
        store.clear_legacy_rows()
        store.delete_memory_items_not_in_sources(list(STABLE_DOCUMENTS))
        archived_paths.append(str(dump_path))
    return {
        "archive_root": str(archive_root),
        "archived_paths": archived_paths,
        "legacy_row_count": legacy_row_count,
        "legacy_memory_item_count": legacy_memory_item_count,
    }


def persist_memory_bundle(
    workspace: str,
    documents: dict[str, str],
    *,
    memory_root: Path | None = None,
    resolved_dir: Path | None = None,
) -> dict[str, Any]:
    """Persist only stable memory documents into the SQLite store."""

    store = open_workspace_store(workspace, memory_root=memory_root, resolved_dir=resolved_dir)
    store.delete_memory_items_not_in_sources(list(documents))
    store.delete_memory_items_by_sources(list(documents))
    persisted_items = 0
    for file_name, text in documents.items():
        category = _memory_category_for_file(file_name)
        segments = _extract_memory_segments(text)
        for index, (headings, summary) in enumerate(segments, start=1):
            heading_context = " / ".join(headings)
            keywords = [summary, file_name, *headings]
            store.upsert_memory_item(
                MemoryItem(
                    item_id=_memory_item_id(workspace, category, index, summary, file_name),
                    category=category,
                    source=file_name,
                    summary=summary,
                    notes=heading_context,
                    confidence=0.8,
                    metadata={"document": file_name, "headings": list(headings)},
                    keywords=[keyword for keyword in keywords if keyword],
                )
            )
            persisted_items += 1
    return {
        "db_path": str(store.db_path),
        "memory_items": len(store.list_memory_items(limit=1000, active=False)),
        "persisted_items": persisted_items,
        "legacy_tables_authoritative": False,
    }


def build_memory_documents(
    *,
    workspace: str,
    snapshot: Any,
) -> dict[str, str]:
    """Build the stable memory markdown documents."""

    repo_root = snapshot.artifact_base.parent
    resolved_dir = resolve_effective_memory_dir(workspace=workspace, repo_root=repo_root)
    return _load_stable_documents(repo_root, resolved_dir)


def write_documents(documents: dict[str, str], resolved_dir: Path) -> list[str]:
    """Write only the stable markdown documents into the memory directory."""

    changed_files: list[str] = []
    resolved_dir.mkdir(parents=True, exist_ok=True)
    for file_name in STABLE_DOCUMENTS:
        text = documents.get(file_name, "")
        if text and write_text_if_changed(resolved_dir / file_name, text):
            changed_files.append(str((resolved_dir / file_name).resolve()))
    return changed_files


def write_memory_state(snapshot: Any, resolved_dir: Path) -> str | None:
    """Write the freshness gate state file."""

    path = memory_state_path(resolved_dir)
    if write_json_if_changed(path, build_memory_state(snapshot)):
        return str(path.resolve())
    return None


def main() -> int:
    """CLI entry point."""

    parser = argparse.ArgumentParser(description="Consolidate short-term artifacts into long-term stable memory.")
    parser.add_argument("--workspace", required=True)
    parser.add_argument("--source-root", type=Path, default=None)
    parser.add_argument("--memory-root", type=Path, default=None)
    parser.add_argument("--json", action="store_true", dest="json_output")
    args = parser.parse_args()
    source_root = (args.source_root or Path(__file__).resolve().parents[1]).resolve()
    snapshot = load_runtime_snapshot(source_root)
    resolved_dir = resolve_effective_memory_dir(
        workspace=args.workspace,
        memory_root=args.memory_root,
        repo_root=source_root,
    )
    archive_result = archive_legacy_memory_bundle(
        args.workspace,
        resolved_dir,
        memory_root=args.memory_root,
    )
    documents = build_memory_documents(workspace=args.workspace, snapshot=snapshot)
    changed_files = write_documents(documents, resolved_dir)
    state_path = write_memory_state(snapshot, resolved_dir)
    if state_path:
        changed_files.append(state_path)
    sqlite_result = persist_memory_bundle(
        args.workspace,
        documents,
        memory_root=args.memory_root,
        resolved_dir=resolved_dir,
    )
    payload = {
        "workspace": args.workspace,
        "memory_root": str(resolved_dir),
        "changed_files": changed_files,
        "archive": archive_result,
        "sqlite_result": sqlite_result,
    }
    if args.json_output:
        print(json.dumps(payload, ensure_ascii=False, indent=2))
    else:
        print(payload)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
