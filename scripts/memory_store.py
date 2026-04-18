#!/usr/bin/env python3
"""Workspace-scoped SQLite memory store for Codex long-memory workflows."""

from __future__ import annotations

import json
import sqlite3
import sys
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Any, Mapping

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.memory_support import (
    DEFAULT_MEMORY_ROOT,
    current_local_timestamp,
    ensure_workspace_memory_dir,
    safe_slug,
    workspace_sqlite_path,
)

SCHEMA_VERSION = "1"
DEFAULT_DB_FILENAME = "memory.sqlite3"
DEFAULT_STATUS = "active"
ALLOWED_STATUSES = {"active", "superseded", "deprecated"}
CATEGORY_PRIORITY = {
    "task_state": 100,
    "blocker": 90,
    "constraint": 80,
    "decision": 70,
    "invariant": 60,
    "preference": 50,
    "runbook": 40,
    "lesson": 30,
    "general": 20,
}


@dataclass(slots=True)
class MemoryItem:
    """Normalized memory item payload."""

    item_id: str
    category: str
    source: str
    summary: str
    confidence: float = 0.5
    status: str = DEFAULT_STATUS
    notes: str = ""
    evidence: list[Any] = field(default_factory=list)
    metadata: dict[str, Any] = field(default_factory=dict)
    keywords: list[str] = field(default_factory=list)


@dataclass(slots=True)
class EvidenceRecord:
    """Normalized evidence payload."""

    kind: str
    path: str
    content: str = ""
    artifact_id: str = ""


@dataclass(slots=True)
class SessionNote:
    """Normalized session note payload."""

    session_key: str
    note: str
    position: int
    note_type: str = "append"
    metadata: dict[str, Any] = field(default_factory=dict)


def _json_default(value: Any) -> Any:
    if isinstance(value, Path):
        return str(value)
    raise TypeError(f"Unsupported JSON value: {type(value)!r}")


def _json_loads(text: str, default: Any) -> Any:
    try:
        return json.loads(text)
    except json.JSONDecodeError:
        return default


def _normalize_text(text: Any) -> str:
    return str(text or "").strip()


def _normalize_float(value: Any) -> float:
    try:
        return max(0.0, min(1.0, float(value)))
    except (TypeError, ValueError):
        return 0.5


def _normalize_list(values: Any) -> list[Any]:
    if isinstance(values, list):
        return values
    if isinstance(values, tuple):
        return list(values)
    return []


def _normalize_mapping(value: Any) -> dict[str, Any]:
    if isinstance(value, Mapping):
        return dict(value)
    return {}


def _now() -> str:
    return current_local_timestamp()


def _tokenize(query: str) -> list[str]:
    return [part.lower() for part in query.replace(",", " ").split() if part.strip()]


def _parse_timestamp(value: str) -> datetime | None:
    try:
        return datetime.fromisoformat(value)
    except Exception:
        return None


def _memory_item_search_blob(item: Mapping[str, Any]) -> str:
    parts: list[str] = []
    for key in ("summary", "notes", "category", "source", "status"):
        parts.append(_normalize_text(item.get(key)))
    parts.extend(str(x) for x in _normalize_list(item.get("keywords")))
    return " ".join(part for part in parts if part)


def _memory_item_rank(item: Mapping[str, Any], query: str) -> tuple[float, str]:
    tokens = _tokenize(query)
    searchable = _memory_item_search_blob(item).lower()
    token_hits = sum(token in searchable for token in tokens) if tokens else 0
    coverage = token_hits / len(tokens) if tokens else 0.0
    category = _normalize_text(item.get("category")) or "general"
    priority = CATEGORY_PRIORITY.get(category, 0)
    freshness_bonus = 0.0
    if updated_at := _parse_timestamp(_normalize_text(item.get("updated_at"))):
        freshness_bonus = max(0.0, 72 - (datetime.now(updated_at.tzinfo) - updated_at).total_seconds() / 3600.0) / 72.0
    score = priority + coverage * 10 + freshness_bonus
    return score, searchable


class MemoryStore:
    """Workspace-scoped SQLite store for structured memory."""

    def __init__(self, db_path: Path, workspace: str, *, ensure_schema: bool = True) -> None:
        self.db_path = db_path.expanduser().resolve()
        self.workspace = workspace
        self.db_path.parent.mkdir(parents=True, exist_ok=True)
        if ensure_schema:
            self.ensure_schema()

    @classmethod
    def for_workspace(
        cls,
        workspace: str,
        memory_root: Path | None = None,
        db_filename: str = DEFAULT_DB_FILENAME,
        resolved_dir: Path | None = None,
    ) -> "MemoryStore":
        """Open the default SQLite store for one workspace."""

        if resolved_dir is not None:
            ws_dir = resolved_dir.expanduser().resolve()
            ws_dir.mkdir(parents=True, exist_ok=True)
            return cls(ws_dir / db_filename, workspace)
        ensure_workspace_memory_dir(workspace, memory_root)
        return cls(workspace_sqlite_path(workspace, memory_root), workspace)

    def connect(self) -> sqlite3.Connection:
        """Create a configured SQLite connection."""

        conn = sqlite3.connect(self.db_path, timeout=5.0)
        conn.row_factory = sqlite3.Row
        conn.execute("PRAGMA foreign_keys = ON")
        conn.execute("PRAGMA busy_timeout = 5000")
        conn.execute("PRAGMA journal_mode = WAL")
        conn.execute("PRAGMA synchronous = NORMAL")
        return conn

    def ensure_schema(self) -> None:
        """Create the store schema if it does not exist."""

        schema = """
        CREATE TABLE IF NOT EXISTS schema_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS memory_items (
            item_id TEXT PRIMARY KEY,
            workspace TEXT NOT NULL,
            category TEXT NOT NULL,
            source TEXT NOT NULL,
            confidence REAL NOT NULL DEFAULT 0.5,
            status TEXT NOT NULL DEFAULT 'active',
            summary TEXT NOT NULL,
            notes TEXT NOT NULL DEFAULT '',
            evidence_json TEXT NOT NULL DEFAULT '[]',
            metadata_json TEXT NOT NULL DEFAULT '{}',
            keywords_json TEXT NOT NULL DEFAULT '[]',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            CHECK (confidence >= 0.0 AND confidence <= 1.0),
            CHECK (status IN ('active', 'superseded', 'deprecated'))
        );
        CREATE INDEX IF NOT EXISTS idx_memory_items_workspace_updated
        ON memory_items(workspace, updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_memory_items_workspace_category_status
        ON memory_items(workspace, category, status, updated_at DESC);
        CREATE TABLE IF NOT EXISTS session_notes (
            note_id INTEGER PRIMARY KEY AUTOINCREMENT,
            workspace TEXT NOT NULL,
            session_key TEXT NOT NULL,
            position INTEGER NOT NULL,
            note TEXT NOT NULL,
            note_type TEXT NOT NULL DEFAULT 'append',
            metadata_json TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE (workspace, session_key, position)
        );
        CREATE INDEX IF NOT EXISTS idx_session_notes_workspace_session_position
        ON session_notes(workspace, session_key, position);
        CREATE TABLE IF NOT EXISTS evidence_records (
            evidence_id INTEGER PRIMARY KEY AUTOINCREMENT,
            workspace TEXT NOT NULL,
            kind TEXT NOT NULL,
            path TEXT NOT NULL,
            content TEXT NOT NULL DEFAULT '',
            artifact_id TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_evidence_records_workspace_updated
        ON evidence_records(workspace, updated_at DESC);
        """
        with self.connect() as conn:
            conn.executescript(schema)
            columns = {row["name"] for row in conn.execute("PRAGMA table_info(evidence_records)").fetchall()}
            if columns and "artifact_id" not in columns:
                conn.execute("ALTER TABLE evidence_records ADD COLUMN artifact_id TEXT NOT NULL DEFAULT ''")
            conn.execute(
                """
                INSERT INTO schema_meta(key, value, updated_at)
                VALUES ('schema_version', ?, ?)
                ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=excluded.updated_at
                """,
                (SCHEMA_VERSION, _now()),
            )

    def upsert_memory_item(self, item: MemoryItem) -> None:
        """Insert or update one memory item."""

        created_at = _now()
        with self.connect() as conn:
            row = conn.execute(
                "SELECT created_at FROM memory_items WHERE item_id = ?",
                (item.item_id,),
            ).fetchone()
            if row:
                created_at = row["created_at"]
            conn.execute(
                """
                INSERT INTO memory_items (
                    item_id, workspace, category, source, confidence, status, summary, notes,
                    evidence_json, metadata_json, keywords_json, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(item_id) DO UPDATE SET
                    workspace=excluded.workspace,
                    category=excluded.category,
                    source=excluded.source,
                    confidence=excluded.confidence,
                    status=excluded.status,
                    summary=excluded.summary,
                    notes=excluded.notes,
                    evidence_json=excluded.evidence_json,
                    metadata_json=excluded.metadata_json,
                    keywords_json=excluded.keywords_json,
                    updated_at=excluded.updated_at
                """,
                (
                    item.item_id,
                    self.workspace,
                    item.category,
                    item.source,
                    _normalize_float(item.confidence),
                    item.status if item.status in ALLOWED_STATUSES else DEFAULT_STATUS,
                    _normalize_text(item.summary),
                    _normalize_text(item.notes),
                    json.dumps(_normalize_list(item.evidence), ensure_ascii=False, default=_json_default),
                    json.dumps(_normalize_mapping(item.metadata), ensure_ascii=False, default=_json_default),
                    json.dumps(_normalize_list(item.keywords), ensure_ascii=False, default=_json_default),
                    created_at,
                    _now(),
                ),
            )

    def delete_memory_items_by_sources(self, sources: list[str]) -> None:
        """Delete memory items for one workspace/source list before a full resync."""

        cleaned = [str(source).strip() for source in sources if str(source).strip()]
        if not cleaned:
            return
        placeholders = ", ".join("?" for _ in cleaned)
        with self.connect() as conn:
            conn.execute(
                f"DELETE FROM memory_items WHERE workspace = ? AND source IN ({placeholders})",
                [self.workspace, *cleaned],
            )

    def sync_session_notes(self, session_key: str, notes: list[str]) -> None:
        """Replace one session note stream."""

        with self.connect() as conn:
            conn.execute(
                "DELETE FROM session_notes WHERE workspace = ? AND session_key = ?",
                (self.workspace, session_key),
            )
            for idx, note in enumerate(notes):
                conn.execute(
                    """
                    INSERT INTO session_notes (
                        workspace, session_key, position, note, note_type, metadata_json, created_at, updated_at
                    ) VALUES (?, ?, ?, ?, 'append', '{}', ?, ?)
                    """,
                    (self.workspace, session_key, idx, note, _now(), _now()),
                )

    def write_evidence(self, rows: list[EvidenceRecord]) -> None:
        """Replace evidence rows for the workspace."""

        with self.connect() as conn:
            conn.execute("DELETE FROM evidence_records WHERE workspace = ?", (self.workspace,))
            for row in rows:
                conn.execute(
                    """
                    INSERT INTO evidence_records (
                        workspace, kind, path, content, artifact_id, created_at, updated_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?)
                    """,
                    (self.workspace, row.kind, row.path, row.content, row.artifact_id, _now(), _now()),
                )

    def list_memory_items(self, *, limit: int = 20, active: bool = True) -> list[dict[str, Any]]:
        """Return memory items for the workspace."""

        query = "SELECT * FROM memory_items WHERE workspace = ?"
        params: list[Any] = [self.workspace]
        if active:
            query += " AND status = 'active'"
        query += " ORDER BY updated_at DESC LIMIT ?"
        params.append(limit)
        with self.connect() as conn:
            rows = conn.execute(query, params).fetchall()
        return [dict(row) for row in rows]

    def search_memory_items(self, query: str, *, limit: int = 20) -> list[dict[str, Any]]:
        """Search memory items using lightweight ranking."""

        items = self.list_memory_items(limit=500, active=True)
        ranked = []
        for item in items:
            score, blob = _memory_item_rank(item, query)
            if query and all(token not in blob for token in _tokenize(query)):
                continue
            ranked.append((score, item))
        ranked.sort(key=lambda pair: pair[0], reverse=True)
        return [item for _, item in ranked[:limit]]

    def list_recent_session_notes(self, *, limit: int = 50) -> list[dict[str, Any]]:
        """Return recent session notes."""

        with self.connect() as conn:
            rows = conn.execute(
                """
                SELECT * FROM session_notes
                WHERE workspace = ?
                ORDER BY updated_at DESC, session_key DESC, position ASC
                LIMIT ?
                """,
                (self.workspace, limit),
            ).fetchall()
        return [dict(row) for row in rows]

    def list_evidence(self, *, limit: int = 50) -> list[dict[str, Any]]:
        """Return evidence rows."""

        with self.connect() as conn:
            rows = conn.execute(
                """
                SELECT * FROM evidence_records
                WHERE workspace = ?
                ORDER BY updated_at DESC
                LIMIT ?
                """,
                (self.workspace, limit),
            ).fetchall()
        return [dict(row) for row in rows]

    def export_legacy_rows(self) -> dict[str, list[dict[str, Any]]]:
        """Export the legacy non-authoritative tables for archival before cutover."""

        with self.connect() as conn:
            session_rows = conn.execute(
                """
                SELECT * FROM session_notes
                WHERE workspace = ?
                ORDER BY updated_at DESC, session_key DESC, position ASC
                """,
                (self.workspace,),
            ).fetchall()
            evidence_rows = conn.execute(
                """
                SELECT * FROM evidence_records
                WHERE workspace = ?
                ORDER BY updated_at DESC
                """,
                (self.workspace,),
            ).fetchall()
        return {
            "session_notes": [dict(row) for row in session_rows],
            "evidence_records": [dict(row) for row in evidence_rows],
        }

    def clear_legacy_rows(self) -> None:
        """Clear the legacy non-authoritative tables after archival."""

        with self.connect() as conn:
            conn.execute("DELETE FROM session_notes WHERE workspace = ?", (self.workspace,))
            conn.execute("DELETE FROM evidence_records WHERE workspace = ?", (self.workspace,))


def open_workspace_store(
    workspace: str,
    memory_root: Path | None = None,
    resolved_dir: Path | None = None,
) -> MemoryStore:
    """Open a store for one workspace."""

    return MemoryStore.for_workspace(workspace, memory_root=memory_root, resolved_dir=resolved_dir)


__all__ = [
    "ALLOWED_STATUSES",
    "CATEGORY_PRIORITY",
    "DEFAULT_DB_FILENAME",
    "DEFAULT_MEMORY_ROOT",
    "DEFAULT_STATUS",
    "EvidenceRecord",
    "MemoryItem",
    "MemoryStore",
    "SCHEMA_VERSION",
    "SessionNote",
    "open_workspace_store",
]
