from __future__ import annotations

import json
import sqlite3
import subprocess
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

ROUTER_RS_MANIFEST_PATH = PROJECT_ROOT / "scripts" / "router-rs" / "Cargo.toml"
ROUTER_RS_BINARY_PATH = PROJECT_ROOT / "scripts" / "router-rs" / "target" / "release" / "router-rs"


def _write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def _write_json(path: Path, payload: dict[str, object]) -> None:
    _write_text(path, json.dumps(payload, ensure_ascii=False, indent=2) + "\n")


def _router_rs_json(repo_root: Path, *args: str) -> dict[str, object]:
    command = (
        [str(ROUTER_RS_BINARY_PATH)]
        if ROUTER_RS_BINARY_PATH.is_file()
        else [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(ROUTER_RS_MANIFEST_PATH),
            "--release",
            "--",
        ]
    )
    completed = subprocess.run(
        [*command, *args, "--repo-root", str(repo_root)],
        cwd=PROJECT_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    payload = json.loads(completed.stdout)
    assert isinstance(payload, dict)
    return payload


def _render_context(
    repo_root: Path,
    *,
    topic: str = "",
    mode: str = "stable",
    top: int = 8,
) -> dict[str, object]:
    payload = _router_rs_json(
        repo_root,
        "--framework-memory-recall-json",
        "--framework-memory-mode",
        mode,
        "--limit",
        str(top),
        "--query",
        topic,
    )
    result = payload["memory_recall"]["retrieval"]
    assert isinstance(result, dict)
    return result


def _ensure_memory_store(repo_root: Path) -> sqlite3.Connection:
    db_path = repo_root / ".codex" / "memory" / "memory.sqlite3"
    db_path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(db_path)
    conn.execute(
        """
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
            updated_at TEXT NOT NULL
        )
        """
    )
    conn.commit()
    return conn


def _insert_memory_item(
    repo_root: Path,
    *,
    item_id: str,
    category: str,
    source: str,
    summary: str,
    notes: str = "",
    keywords: list[str] | None = None,
    updated_at: str = "2026-04-18T22:49:57+08:00",
) -> None:
    conn = _ensure_memory_store(repo_root)
    conn.execute(
        """
        INSERT OR REPLACE INTO memory_items (
            item_id, workspace, category, source, confidence, status, summary, notes,
            evidence_json, metadata_json, keywords_json, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            item_id,
            repo_root.name,
            category,
            source,
            0.8,
            "active",
            summary,
            notes,
            "[]",
            "{}",
            json.dumps(keywords or [], ensure_ascii=False),
            updated_at,
            updated_at,
        ),
    )
    conn.commit()
    conn.close()


def _seed_runtime(repo_root: Path, *, task: str = "active bootstrap repair") -> None:
    task_id = "active-bootstrap-repair-20260418210000"
    task_root = repo_root / "artifacts" / "current" / task_id
    _write_text(
        task_root / "SESSION_SUMMARY.md",
        "\n".join(
            [
                f"- task: {task}",
                "- phase: implementation",
                "- status: in_progress",
            ]
        )
        + "\n",
    )
    _write_json(
        task_root / "NEXT_ACTIONS.json",
        {"next_actions": ["Patch classifier", "Run pytest"]},
    )
    _write_json(task_root / "EVIDENCE_INDEX.json", {"artifacts": []})
    _write_json(
        task_root / "TRACE_METADATA.json",
        {"task": task, "matched_skills": ["execution-controller-coding"]},
    )
    _write_text(
        repo_root / "artifacts" / "current" / "SESSION_SUMMARY.md",
        (task_root / "SESSION_SUMMARY.md").read_text(encoding="utf-8"),
    )
    _write_json(
        repo_root / "artifacts" / "current" / "NEXT_ACTIONS.json",
        {"next_actions": ["Patch classifier", "Run pytest"]},
    )
    _write_json(repo_root / "artifacts" / "current" / "EVIDENCE_INDEX.json", {"artifacts": []})
    _write_json(
        repo_root / "artifacts" / "current" / "TRACE_METADATA.json",
        {"task": task, "matched_skills": ["execution-controller-coding"]},
    )
    _write_json(
        repo_root / "artifacts" / "current" / "active_task.json",
        {"task_id": task_id, "task": task},
    )
    _write_json(
        repo_root / ".supervisor_state.json",
        {
            "task_id": task_id,
            "task_summary": task,
            "active_phase": "implementation",
            "verification": {"verification_status": "in_progress"},
            "continuity": {
                "story_state": "active",
                "resume_allowed": True,
                "last_updated_at": "2026-04-18T22:49:57+08:00",
            },
            "blockers": {"open_blockers": ["Need regression coverage"]},
        },
    )


def _seed_stable_memory(repo_root: Path) -> None:
    memory_root = repo_root / ".codex" / "memory"
    _write_text(
        memory_root / "MEMORY.md",
        "# 项目长期记忆\n\n## Active Patterns\n\n- AP-1: Stable only by default\n",
    )
    _write_text(memory_root / "preferences.md", "# preferences\n\n- prefer compact recall\n")


def _seed_sqlite_memory(repo_root: Path) -> None:
    _insert_memory_item(
        repo_root,
        item_id="sqlite-item-1",
        category="general",
        source="sqlite",
        summary="sqlite-only row",
        notes="diagnostic row",
    )


def test_render_context_stable_mode_excludes_active_task_and_archive(tmp_path: Path) -> None:
    _seed_runtime(tmp_path)
    _seed_stable_memory(tmp_path)
    _write_text(
        tmp_path / ".codex" / "memory" / "archive" / "pre-cutover-2026-04-18" / "sessions" / "2026-04-18.md",
        "task=old\n",
    )

    result = _render_context(tmp_path, topic="active bootstrap repair", mode="stable")

    assert result["mode"] == "stable"
    assert result["active_task_included"] is False
    assert all(item["path"] != "runtime/current_task.md" for item in result["items"])
    assert all("archive/" not in item["path"] for item in result["items"])


def test_render_context_active_mode_includes_matching_current_task_when_fresh(tmp_path: Path) -> None:
    _seed_runtime(tmp_path)
    _seed_stable_memory(tmp_path)

    result = _render_context(tmp_path, topic="active bootstrap repair", mode="active")

    assert result["active_task_included"] is True
    assert result["freshness"]["state"] == "fresh"
    assert any(item["path"] == "runtime/current_task.md" for item in result["items"])


def test_render_context_active_mode_refreshes_stale_memory_state(tmp_path: Path) -> None:
    _seed_runtime(tmp_path)
    _seed_stable_memory(tmp_path)
    _write_json(
        tmp_path / ".codex" / "memory" / "state.json",
        {
            "schema_version": "memory-state-v1",
            "source_task_id": "older-task",
            "content_hash": "stale",
            "source_updated_at": "2026-04-18T20:00:00+08:00",
        },
    )

    result = _render_context(tmp_path, topic="active bootstrap repair", mode="active")

    assert result["active_task_included"] is True
    assert result["freshness"]["state"] == "fresh"
    state = json.loads((tmp_path / ".codex" / "memory" / "state.json").read_text(encoding="utf-8"))
    assert state["source_task_id"] == "active-bootstrap-repair-20260418210000"


def test_render_context_active_mode_self_heals_missing_memory_state(tmp_path: Path) -> None:
    _seed_runtime(tmp_path)
    memory_root = tmp_path / ".codex" / "memory"
    _write_text(memory_root / "MEMORY.md", "# 项目长期记忆\n")
    _write_text(memory_root / "preferences.md", "# preferences\n")

    result = _render_context(tmp_path, topic="active bootstrap repair", mode="active")

    assert result["active_task_included"] is True
    assert (memory_root / "state.json").is_file()
    assert result["freshness"]["state"] == "fresh"


def test_render_context_history_mode_can_read_archive(tmp_path: Path) -> None:
    _seed_runtime(tmp_path)
    _seed_stable_memory(tmp_path)
    _write_text(
        tmp_path / ".codex" / "memory" / "archive" / "pre-cutover-2026-04-18" / "sessions" / "2026-04-18.md",
        "task=old closeout\n",
    )

    result = _render_context(tmp_path, topic="old closeout", mode="history")

    assert any("archive/" in item["path"] for item in result["items"])


def test_default_modes_do_not_include_sqlite_sections(tmp_path: Path) -> None:
    _seed_runtime(tmp_path)
    _seed_stable_memory(tmp_path)
    _seed_sqlite_memory(tmp_path)

    for mode in ("stable", "active", "history"):
        result = _render_context(tmp_path, topic="sqlite", mode=mode)
        assert all(not item["path"].startswith("sqlite/") for item in result["items"])


def test_stable_mode_without_topic_compacts_stable_documents(tmp_path: Path) -> None:
    _seed_runtime(tmp_path)
    memory_root = tmp_path / ".codex" / "memory"
    _write_text(
        memory_root / "MEMORY.md",
        "\n".join(
            [
                "# 项目长期记忆",
                "",
                "## Active Patterns",
                "",
                "- AP-1: keep recall compact",
                "- AP-2: use stable summaries",
                "",
                "## 稳定决策",
                "",
                "- SD-1: avoid full document injection",
                "",
            ]
        )
        + "\n",
    )
    _write_text(
        memory_root / "runbooks.md",
        "\n".join(
            [
                "# runbooks",
                "",
                "## 标准操作",
                "",
                "- step 1",
                "- step 2",
                "- step 3",
                "",
            ]
        )
        + "\n",
    )

    result = _render_context(tmp_path, topic="", mode="stable")

    memory_item = next(item for item in result["items"] if item["path"] == "MEMORY.md")
    runbook_item = next(item for item in result["items"] if item["path"] == "runbooks.md")
    assert "AP-1: keep recall compact" in memory_item["content"]
    assert "avoid full document injection" not in memory_item["content"]
    assert "step 1" in runbook_item["content"]
    assert "step 3" not in runbook_item["content"]


def test_debug_mode_exposes_sqlite_sections(tmp_path: Path) -> None:
    _seed_runtime(tmp_path)
    _seed_stable_memory(tmp_path)
    _seed_sqlite_memory(tmp_path)

    result = _render_context(tmp_path, topic="sqlite", mode="debug")

    assert any(item["path"] == "sqlite/memory_items.md" for item in result["items"])


def test_stable_recall_does_not_fallback_on_partial_sqlite_token_match(tmp_path: Path) -> None:
    _seed_runtime(tmp_path)
    memory_root = tmp_path / ".codex" / "memory"
    memory_root.mkdir(parents=True, exist_ok=True)
    _write_text(
        memory_root / "MEMORY.md",
        "\n".join(
            [
                "# 项目长期记忆",
                "",
                "## 稳定决策",
                "",
                "### 执行编排",
                "",
                "- runtime contract only",
                "",
            ]
        )
        + "\n",
    )
    _insert_memory_item(
        tmp_path,
        item_id="runtime-contract",
        category="decision",
        source="sqlite",
        summary="runtime contract only",
        notes="tracks execution guarantees only",
        keywords=["runtime", "contract"],
    )

    result = _render_context(tmp_path, topic="runtime observability", mode="stable")

    assert result["items"] == []


def test_memory_store_search_requires_strong_query_match(tmp_path: Path) -> None:
    _seed_runtime(tmp_path)
    _seed_stable_memory(tmp_path)
    _insert_memory_item(
        tmp_path,
        item_id="runtime-contract",
        category="decision",
        source="sqlite",
        summary="runtime contract only",
        notes="tracks execution guarantees only",
        keywords=["runtime", "contract"],
    )
    _insert_memory_item(
        tmp_path,
        item_id="runtime-observability",
        category="decision",
        source="sqlite",
        summary="runtime observability contract",
        notes="tracks runtime observability guarantees",
        keywords=["runtime", "observability"],
    )

    result = _render_context(tmp_path, topic="runtime observability", mode="debug")
    sqlite_item = next(item for item in result["items"] if item["path"] == "sqlite/memory_items.md")

    assert "runtime observability contract" in sqlite_item["content"]
    assert "runtime contract only" not in sqlite_item["content"]
