"""Regression tests for memory retrieval mode boundaries."""

from __future__ import annotations

import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.memory_store import MemoryItem, open_workspace_store
from scripts.retrieve_memory import render_context


def test_stable_and_active_modes_exclude_sqlite_sections(tmp_path: Path) -> None:
    workspace = "demo"
    memory_root = tmp_path
    workspace_dir = memory_root / workspace
    workspace_dir.mkdir(parents=True)
    (workspace_dir / "MEMORY.md").write_text("# 项目长期记忆\n\n## Facts\n- stable fact\n", encoding="utf-8")
    (workspace_dir / "sessions").mkdir()
    (workspace_dir / "sessions" / "2026-04-18.md").write_text("# latest session\n", encoding="utf-8")

    store = open_workspace_store(workspace, memory_root=memory_root)
    store.upsert_memory_item(
        MemoryItem(
            item_id="m1",
            category="general",
            source="test",
            summary="sqlite should stay out of normal recall",
            notes="debug only",
        )
    )

    stable = render_context(workspace=workspace, memory_root=memory_root, mode="stable", topic="sqlite")
    active = render_context(workspace=workspace, memory_root=memory_root, mode="active", topic="sqlite")
    history = render_context(workspace=workspace, memory_root=memory_root, mode="history", topic="sqlite")
    debug = render_context(workspace=workspace, memory_root=memory_root, mode="debug", topic="sqlite")

    assert stable["mode"] == "stable"
    assert all(not item["path"].startswith("sqlite/") for item in stable["items"])
    assert all(not item["path"].startswith("sqlite/") for item in active["items"])
    assert all(not item["path"].startswith("sqlite/") for item in history["items"])
    assert any(item["path"].startswith("sqlite/") for item in debug["items"])
    assert any(item["path"] == "sessions/2026-04-18.md" for item in active["items"])


def test_invalid_mode_raises_value_error(tmp_path: Path) -> None:
    workspace = "demo"
    try:
        render_context(workspace=workspace, memory_root=tmp_path, mode="invalid")
    except ValueError as exc:
        assert "Unsupported memory recall mode" in str(exc)
    else:  # pragma: no cover - defensive guard
        raise AssertionError("expected invalid mode to raise ValueError")
