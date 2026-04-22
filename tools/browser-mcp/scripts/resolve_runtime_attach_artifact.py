#!/usr/bin/env python3
"""Resolve the newest replay-capable browser-mcp attach artifact."""

from __future__ import annotations

import argparse
import json
import sqlite3
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Iterable

TRACE_RESUME_MANIFEST_SCHEMA_VERSION = "runtime-resume-manifest-v1"
RUNTIME_EVENT_TRANSPORT_SCHEMA_VERSION = "runtime-event-transport-v1"
DEFAULT_SEARCH_ROOT = (
    Path(__file__).resolve().parents[3] / "codex_agno_runtime" / "artifacts" / "scratch"
)


@dataclass(frozen=True)
class AttachCandidate:
    """One resolved attach artifact candidate."""

    attach_path: str
    source_kind: str
    source_path: str
    updated_at_epoch: float
    recency_hint: int


def _parse_iso_epoch(value: object) -> float:
    if not isinstance(value, str) or not value.strip():
        return 0.0
    normalized = value.strip().replace("Z", "+00:00")
    try:
        return datetime.fromisoformat(normalized).timestamp()
    except ValueError:
        return 0.0


def _json_object(raw: str) -> dict[str, object] | None:
    try:
        parsed = json.loads(raw)
    except json.JSONDecodeError:
        return None
    if not isinstance(parsed, dict):
        return None
    return parsed


def _manifest_candidate(
    payload: dict[str, object],
    *,
    source_path: str,
    recency_hint: int,
) -> AttachCandidate | None:
    if payload.get("schema_version") != TRACE_RESUME_MANIFEST_SCHEMA_VERSION:
        return None
    attach_path = payload.get("event_transport_path")
    if not isinstance(attach_path, str) or not attach_path.strip():
        return None
    return AttachCandidate(
        attach_path=attach_path.strip(),
        source_kind="resume_manifest",
        source_path=source_path,
        updated_at_epoch=_parse_iso_epoch(payload.get("updated_at")),
        recency_hint=recency_hint,
    )


def _binding_candidate(
    payload: dict[str, object],
    *,
    source_path: str,
    fallback_attach_path: str | None,
    recency_hint: int,
) -> AttachCandidate | None:
    if payload.get("schema_version") != RUNTIME_EVENT_TRANSPORT_SCHEMA_VERSION:
        return None
    attach_path = payload.get("binding_artifact_path")
    if not isinstance(attach_path, str) or not attach_path.strip():
        attach_path = fallback_attach_path
    if not isinstance(attach_path, str) or not attach_path.strip():
        return None
    return AttachCandidate(
        attach_path=attach_path.strip(),
        source_kind="binding_artifact",
        source_path=source_path,
        updated_at_epoch=0.0,
        recency_hint=recency_hint,
    )


def _iter_filesystem_candidates(search_root: Path) -> Iterable[AttachCandidate]:
    for manifest_path in search_root.glob("**/TRACE_RESUME_MANIFEST.json"):
        if not manifest_path.is_file():
            continue
        payload = _json_object(manifest_path.read_text(encoding="utf-8"))
        if payload is None:
            continue
        candidate = _manifest_candidate(
            payload,
            source_path=str(manifest_path.resolve()),
            recency_hint=manifest_path.stat().st_mtime_ns,
        )
        if candidate is not None:
            yield candidate

    for binding_path in search_root.glob("**/runtime_event_transports/*.json"):
        if not binding_path.is_file():
            continue
        payload = _json_object(binding_path.read_text(encoding="utf-8"))
        if payload is None:
            continue
        candidate = _binding_candidate(
            payload,
            source_path=str(binding_path.resolve()),
            fallback_attach_path=str(binding_path.resolve()),
            recency_hint=binding_path.stat().st_mtime_ns,
        )
        if candidate is not None:
            yield candidate


def _iter_sqlite_candidates(search_root: Path) -> Iterable[AttachCandidate]:
    for db_path in search_root.glob("**/runtime_checkpoint_store.sqlite3"):
        if not db_path.is_file():
            continue
        try:
            connection = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
        except sqlite3.Error:
            continue
        try:
            rows = connection.execute(
                """
                SELECT rowid, payload_key, payload_text
                FROM runtime_storage_payloads
                WHERE payload_key LIKE '%TRACE_RESUME_MANIFEST.json'
                   OR payload_key LIKE '%runtime_event_transports/%.json'
                """
            ).fetchall()
        except sqlite3.Error:
            connection.close()
            continue
        connection.close()
        for rowid, payload_key, payload_text in rows:
            if not isinstance(payload_text, str):
                continue
            payload = _json_object(payload_text)
            if payload is None:
                continue
            source_path = f"{db_path.resolve()}::{payload_key}"
            recency_hint = int(db_path.stat().st_mtime_ns) + int(rowid)
            candidate = _manifest_candidate(
                payload,
                source_path=source_path,
                recency_hint=recency_hint,
            )
            if candidate is not None:
                yield candidate
                continue
            candidate = _binding_candidate(
                payload,
                source_path=source_path,
                fallback_attach_path=payload_key if isinstance(payload_key, str) else None,
                recency_hint=recency_hint,
            )
            if candidate is not None:
                yield candidate


def resolve_runtime_attach_artifact(search_root: Path) -> str | None:
    """Return the newest attach artifact path across filesystem and sqlite surfaces."""

    candidates = list(_iter_filesystem_candidates(search_root))
    candidates.extend(_iter_sqlite_candidates(search_root))
    if not candidates:
        return None

    deduped: dict[str, AttachCandidate] = {}
    for candidate in candidates:
        current = deduped.get(candidate.attach_path)
        if current is None or (
            candidate.updated_at_epoch,
            candidate.recency_hint,
            1 if candidate.source_kind == "resume_manifest" else 0,
        ) > (
            current.updated_at_epoch,
            current.recency_hint,
            1 if current.source_kind == "resume_manifest" else 0,
        ):
            deduped[candidate.attach_path] = candidate

    selected = max(
        deduped.values(),
        key=lambda item: (
            item.updated_at_epoch,
            item.recency_hint,
            1 if item.source_kind == "resume_manifest" else 0,
            item.attach_path,
        ),
    )
    return selected.attach_path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--search-root",
        type=Path,
        default=DEFAULT_SEARCH_ROOT,
        help="Root directory that contains runtime scratch artifacts.",
    )
    args = parser.parse_args()

    attach_path = resolve_runtime_attach_artifact(args.search_root.resolve())
    if attach_path is None:
        return 1
    print(attach_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
