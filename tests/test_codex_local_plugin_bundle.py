"""Regression tests for the local Codex plugin bundle."""

from __future__ import annotations

import json
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]
PLUGIN_ROOT = PROJECT_ROOT / "plugins" / "skill-framework-native"


def test_plugin_manifest_exposes_skills_and_mcp_bundle() -> None:
    manifest = json.loads((PLUGIN_ROOT / ".codex-plugin" / "plugin.json").read_text(encoding="utf-8"))
    assert manifest["name"] == "skill-framework-native"
    assert manifest["skills"] == "./skills/"
    assert manifest["mcpServers"] == "./.mcp.json"
    assert manifest["interface"]["displayName"] == "Skill Framework Native"


def test_plugin_mcp_bundle_points_back_to_repo_root() -> None:
    payload = json.loads((PLUGIN_ROOT / ".mcp.json").read_text(encoding="utf-8"))
    framework = payload["mcpServers"]["framework-mcp"]
    browser = payload["mcpServers"]["browser-mcp"]
    assert framework["command"] == "python3"
    assert framework["args"] == ["-m", "scripts.framework_mcp"]
    assert framework["cwd"] == "../.."
    assert browser["command"] == "bash"
    assert browser["cwd"] == "../.."


def test_marketplace_registers_local_plugin() -> None:
    marketplace = json.loads((PROJECT_ROOT / ".agents" / "plugins" / "marketplace.json").read_text(encoding="utf-8"))
    assert marketplace["interface"]["displayName"] == "Skill Local Marketplace"
    plugin = marketplace["plugins"][0]
    assert plugin["name"] == "skill-framework-native"
    assert plugin["source"]["path"] == "./plugins/skill-framework-native"
    assert plugin["policy"]["installation"] == "AVAILABLE"
