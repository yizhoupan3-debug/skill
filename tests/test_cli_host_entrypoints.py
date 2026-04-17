from __future__ import annotations

import json
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.materialize_cli_host_entrypoints import (
    materialize_repo_host_entrypoints,
    sync_repo_host_entrypoints,
)
from scripts.sync_skills import write_generated_files


def test_materialize_repo_host_entrypoints_creates_shared_policy_and_host_proxies(
    tmp_path: Path,
) -> None:
    result = materialize_repo_host_entrypoints(tmp_path)

    assert "AGENT.md" in result["written"]
    assert "AGENTS.md" in result["written"]
    assert "CLAUDE.md" in result["written"]
    assert "GEMINI.md" in result["written"]
    assert ".claude/settings.json" in result["written"]
    assert ".gemini/settings.json" in result["written"]
    assert ".mcp.json" in result["written"]

    assert (tmp_path / "AGENT.md").is_file()
    assert "Shared Agent Policy" in (tmp_path / "AGENT.md").read_text(encoding="utf-8")
    assert "AGENT.md" in (tmp_path / "AGENTS.md").read_text(encoding="utf-8")
    assert "@.claude/CLAUDE.md" in (tmp_path / "CLAUDE.md").read_text(encoding="utf-8")
    assert "AGENT.md" in (tmp_path / "GEMINI.md").read_text(encoding="utf-8")
    assert "@../AGENT.md" in (tmp_path / ".claude" / "CLAUDE.md").read_text(encoding="utf-8")
    assert "@../.codex/memory/CLAUDE_MEMORY.md" in (
        tmp_path / ".claude" / "CLAUDE.md"
    ).read_text(encoding="utf-8")
    settings = json.loads((tmp_path / ".claude" / "settings.json").read_text(encoding="utf-8"))
    assert settings["$schema"] == "https://json.schemastore.org/claude-code-settings.json"
    assert set(settings["hooks"]) == {"SessionStart", "Stop", "SessionEnd"}
    assert json.loads((tmp_path / ".gemini" / "settings.json").read_text(encoding="utf-8")) == {}
    assert json.loads((tmp_path / ".mcp.json").read_text(encoding="utf-8")) == {
        "mcpServers": {
            "browser-mcp": {
                "command": "bash",
                "args": ["./tools/browser-mcp/scripts/start_browser_mcp.sh"],
            }
        }
    }
    assert (tmp_path / ".claude" / "agents" / "README.md").is_file()
    assert (tmp_path / ".claude" / "hooks" / "README.md").is_file()
    assert (tmp_path / ".claude" / "hooks" / "session_start.sh").is_file()
    assert (tmp_path / ".claude" / "hooks" / "stop.sh").is_file()
    assert (tmp_path / ".claude" / "hooks" / "session_end.sh").is_file()
    assert (tmp_path / "configs" / "claude" / "CLAUDE.md").is_file()
    assert (tmp_path / "configs" / "gemini" / "GEMINI.md").is_file()


def test_materialize_repo_host_entrypoints_is_idempotent(tmp_path: Path) -> None:
    materialize_repo_host_entrypoints(tmp_path)
    result = materialize_repo_host_entrypoints(tmp_path)

    assert result["written"] == []
    assert "AGENT.md" in result["unchanged"]
    assert ".claude/settings.json" in result["unchanged"]


def test_sync_repo_host_entrypoints_reports_drift_without_writing(tmp_path: Path) -> None:
    result = sync_repo_host_entrypoints(tmp_path, apply=False)

    assert "AGENT.md" in result["written"]
    assert not (tmp_path / "AGENT.md").exists()


def test_write_generated_files_includes_shared_cli_entrypoints_when_repo_is_dirty(tmp_path: Path) -> None:
    root = PROJECT_ROOT
    managed_paths = [
        root / "AGENT.md",
        root / "AGENTS.md",
        root / "CLAUDE.md",
        root / "GEMINI.md",
        root / ".claude" / "CLAUDE.md",
        root / ".claude" / "settings.json",
        root / ".claude" / "agents" / "README.md",
        root / ".claude" / "hooks" / "README.md",
        root / ".claude" / "hooks" / "session_start.sh",
        root / ".claude" / "hooks" / "stop.sh",
        root / ".claude" / "hooks" / "session_end.sh",
        root / ".gemini" / "settings.json",
        root / ".mcp.json",
        root / "configs" / "claude" / "CLAUDE.md",
        root / "configs" / "codex" / "AGENTS.md",
        root / "configs" / "gemini" / "GEMINI.md",
    ]
    backups = {
        path: path.read_text(encoding="utf-8") if path.exists() else None
        for path in managed_paths
    }

    target = root / "CLAUDE.md"
    original = target.read_text(encoding="utf-8")
    target.write_text(original + "\nDRIFT\n", encoding="utf-8")
    try:
        changed = write_generated_files(apply=False)
        assert "CLAUDE.md" in changed
    finally:
        for path, content in backups.items():
            if content is None:
                if path.exists():
                    path.unlink()
                continue
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")
