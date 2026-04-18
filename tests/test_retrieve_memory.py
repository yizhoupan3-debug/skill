"""Regression tests for prompt memory retrieval modes."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.memory_store import MemoryItem, MemoryStore
from scripts.retrieve_memory import render_context


def _prepare_memory_workspace(tmp_path: Path) -> tuple[Path, str]:
    workspace = "demo-workspace"
    memory_root = tmp_path / "memory-root"
    workspace_root = memory_root / workspace
    workspace_root.mkdir(parents=True, exist_ok=True)
    (workspace_root / "MEMORY.md").write_text("# MEMORY\n\n## Stable\nkeep this stable fact\n", encoding="utf-8")
    (workspace_root / "preferences.md").write_text("prefer concise outputs\n", encoding="utf-8")
    (workspace_root / "sessions").mkdir(parents=True, exist_ok=True)
    (workspace_root / "sessions" / "2026-04-18.md").write_text("latest session detail\n", encoding="utf-8")

    store = MemoryStore.for_workspace(workspace, resolved_dir=workspace_root)
    store.upsert_memory_item(
        MemoryItem(
            item_id="item-1",
            category="general",
            source="sqlite",
            summary="sqlite-only row",
            notes="diagnostic row",
        )
    )
    return memory_root, workspace


@pytest.mark.parametrize("mode", ["stable", "active", "history"])
def test_default_prompt_modes_do_not_include_sqlite_sections(tmp_path: Path, mode: str) -> None:
    memory_root, workspace = _prepare_memory_workspace(tmp_path)

    result = render_context(workspace=workspace, memory_root=memory_root, mode=mode)
    paths = [item["path"] for item in result["items"]]

    assert "sqlite/memory_items.md" not in paths
    assert "MEMORY.md" in paths


def test_debug_mode_exposes_sqlite_sections(tmp_path: Path) -> None:
    memory_root, workspace = _prepare_memory_workspace(tmp_path)

    result = render_context(workspace=workspace, memory_root=memory_root, mode="debug")
    paths = [item["path"] for item in result["items"]]

    assert "sqlite/memory_items.md" in paths


def test_invalid_render_mode_raises_value_error(tmp_path: Path) -> None:
    memory_root, workspace = _prepare_memory_workspace(tmp_path)

    with pytest.raises(ValueError):
        render_context(workspace=workspace, memory_root=memory_root, mode="invalid")
