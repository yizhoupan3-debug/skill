"""Regression tests for browser MCP Codex installer."""

from __future__ import annotations

import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.install_browser_mcp_codex import build_server_block, install_server


def test_build_server_block_uses_repo_relative_script_path(tmp_path: Path) -> None:
    """Verify the generated TOML block points at the repo-local start script.

    Parameters:
        tmp_path: Temporary pytest directory fixture.

    Returns:
        None.
    """

    repo_root = tmp_path / "repo"
    block = build_server_block(repo_root)
    assert "[mcp_servers.browser-mcp]" in block
    assert str(repo_root / "tools" / "browser-mcp" / "scripts" / "start_browser_mcp.sh") in block


def test_install_server_is_idempotent(tmp_path: Path) -> None:
    """Verify installer writes once and skips duplicate entries.

    Parameters:
        tmp_path: Temporary pytest directory fixture.

    Returns:
        None.
    """

    config_path = tmp_path / "config.toml"
    config_path.write_text("[model]\nname = \"gpt-5\"\n", encoding="utf-8")

    first = install_server(config_path=config_path, repo_root=tmp_path / "repo")
    second = install_server(config_path=config_path, repo_root=tmp_path / "repo")

    content = config_path.read_text(encoding="utf-8")
    assert first is True
    assert second is False
    assert content.count("[mcp_servers.browser-mcp]") == 1
