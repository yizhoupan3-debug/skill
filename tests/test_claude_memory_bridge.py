from __future__ import annotations

import json
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.claude_memory_bridge import run_bridge, sync_claude_memory_projection


def _write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def _write_json(path: Path, payload: dict[str, object]) -> None:
    _write_text(path, json.dumps(payload, ensure_ascii=False, indent=2) + "\n")


def _seed_runtime_artifacts(repo_root: Path) -> None:
    _write_text(
        repo_root / "artifacts" / "current" / "SESSION_SUMMARY.md",
        "\n".join(
            [
                "- task: Validate Claude bridge",
                "- phase: integration",
                "- status: in_progress",
            ]
        )
        + "\n",
    )
    _write_json(repo_root / "artifacts" / "current" / "NEXT_ACTIONS.json", {"next_actions": ["Wire hooks", "Verify sync"]})
    _write_json(
        repo_root / "artifacts" / "current" / "EVIDENCE_INDEX.json",
        {"artifacts": [{"kind": "doc", "path": "CLAUDE.md", "status": "ok"}]},
    )
    _write_json(
        repo_root / "artifacts" / "current" / "TRACE_METADATA.json",
        {"matched_skills": ["skill-developer-codex", "agent-memory"]},
    )
    _write_json(
        repo_root / ".supervisor_state.json",
        {
            "task_summary": "Validate Claude bridge",
            "active_phase": "integration",
            "verification": {"verification_status": "in_progress"},
            "blockers": {"open_blockers": ["Need lifecycle hook wiring"]},
            "execution_contract": {
                "scope": ["Claude lifecycle bridge", "shared memory projection"],
                "acceptance_criteria": ["Claude imports shared memory", "hooks refresh projection"],
            },
        },
    )


def _seed_shared_memory(repo_root: Path) -> None:
    _write_text(
        repo_root / ".codex" / "memory" / "MEMORY.md",
        "\n".join(
            [
                "# 项目长期记忆",
                "",
                "## Active Patterns",
                "",
                "- AP-1: Sync skills after skill edits",
                "- AP-2: Externalize complex task state into artifacts",
                "",
                "## 稳定决策",
                "",
                "- SD-1: Shared CLI memory root lives under `./.codex/memory/`",
                "",
                "## Lessons",
                "",
                "- L-1: Do not let generated host files drift from runtime truth",
                "",
            ]
        )
        + "\n",
    )


def test_sync_claude_memory_projection_renders_shared_runtime_state(tmp_path: Path) -> None:
    _seed_runtime_artifacts(tmp_path)
    _seed_shared_memory(tmp_path)

    result = sync_claude_memory_projection(tmp_path)

    target = tmp_path / ".codex" / "memory" / "CLAUDE_MEMORY.md"
    content = target.read_text(encoding="utf-8")
    assert result["changed"] is True
    assert "Validate Claude bridge" in content
    assert "AP-1: Sync skills after skill edits" in content
    assert "SD-1: Shared CLI memory root lives under `./.codex/memory/`" in content
    assert "`artifacts/current/SESSION_SUMMARY.md`" in content


def test_run_bridge_session_end_consolidates_then_refreshes_projection(tmp_path: Path) -> None:
    _seed_runtime_artifacts(tmp_path)
    _seed_shared_memory(tmp_path)

    result = run_bridge("session-end", tmp_path)

    assert "consolidation" in result
    assert (tmp_path / ".codex" / "memory" / "MEMORY_AUTO.md").is_file()
    assert (tmp_path / ".codex" / "memory" / "CLAUDE_MEMORY.md").is_file()
    auto_memory = (tmp_path / ".codex" / "memory" / "MEMORY_AUTO.md").read_text(encoding="utf-8")
    projection = (tmp_path / ".codex" / "memory" / "CLAUDE_MEMORY.md").read_text(encoding="utf-8")
    assert "当前主线任务：Validate Claude bridge" in auto_memory
    assert "Need lifecycle hook wiring" in projection
