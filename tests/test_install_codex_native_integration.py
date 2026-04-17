"""Regression tests for the unified Codex native integration installer."""

from __future__ import annotations

import json
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.install_codex_native_integration import (
    build_personal_marketplace_payload,
    build_framework_server_block,
    ensure_config_file,
    install_native_integration,
    sync_directory,
)


def test_build_framework_server_block_uses_python_module_and_repo_cwd(tmp_path: Path) -> None:
    repo_root = tmp_path / "repo"
    block = build_framework_server_block(repo_root)
    assert "[mcp_servers.framework-mcp]" in block
    assert 'command = "python3"' in block
    assert 'args = ["-m", "scripts.framework_mcp"]' in block
    assert f'cwd = "{repo_root}"' in block


def test_ensure_config_file_bootstraps_schema_header(tmp_path: Path) -> None:
    config_path = tmp_path / ".codex" / "config.toml"
    changed = ensure_config_file(config_path)
    assert changed is True
    assert config_path.read_text(encoding="utf-8").startswith("#:schema https://developers.openai.com/codex/config-schema.json")


def test_install_native_integration_is_idempotent(tmp_path: Path) -> None:
    repo_root = tmp_path / "repo"
    (repo_root / ".codex").mkdir(parents=True)
    plugin_root = repo_root / "plugins" / "skill-framework-native" / ".codex-plugin"
    plugin_root.mkdir(parents=True)
    (plugin_root / "plugin.json").write_text('{"name":"skill-framework-native"}\n', encoding="utf-8")
    home_config_path = tmp_path / "home" / ".codex" / "config.toml"
    home_plugin_root = tmp_path / "home" / ".codex" / "plugins" / "skill-framework-native"
    home_marketplace_path = tmp_path / "home" / ".agents" / "plugins" / "marketplace.json"
    project_instructions_path = Path(".codex") / "model_instructions.md"

    first = install_native_integration(
        home_config_path=home_config_path,
        repo_root=repo_root,
        home_plugin_root=home_plugin_root,
        home_marketplace_path=home_marketplace_path,
        project_instructions_path=project_instructions_path,
    )
    second = install_native_integration(
        home_config_path=home_config_path,
        repo_root=repo_root,
        home_plugin_root=home_plugin_root,
        home_marketplace_path=home_marketplace_path,
        project_instructions_path=project_instructions_path,
    )

    content = home_config_path.read_text(encoding="utf-8")
    instructions = (repo_root / project_instructions_path).read_text(encoding="utf-8")
    marketplace = json.loads(home_marketplace_path.read_text(encoding="utf-8"))
    assert first["success"] is True
    assert second["success"] is True
    assert content.count("[mcp_servers.browser-mcp]") == 1
    assert content.count("[mcp_servers.framework-mcp]") == 1
    assert (home_plugin_root / ".codex-plugin" / "plugin.json").is_file()
    assert [plugin["name"] for plugin in marketplace["plugins"]].count("skill-framework-native") == 1
    assert "HERMES_DEFAULT_RUNTIME_START" in instructions


def test_sync_directory_removes_stale_files_and_copies_updates(tmp_path: Path) -> None:
    source = tmp_path / "source"
    destination = tmp_path / "destination"
    (source / "nested").mkdir(parents=True)
    destination.mkdir(parents=True)
    (source / "nested" / "keep.txt").write_text("fresh\n", encoding="utf-8")
    (destination / "stale.txt").write_text("old\n", encoding="utf-8")

    changed = sync_directory(source, destination)

    assert changed is True
    assert not (destination / "stale.txt").exists()
    assert (destination / "nested" / "keep.txt").read_text(encoding="utf-8") == "fresh\n"


def test_build_personal_marketplace_payload_preserves_existing_plugins() -> None:
    payload = build_personal_marketplace_payload(
        Path.home() / ".codex" / "plugins" / "skill-framework-native",
        existing_marketplace={
            "name": "custom-marketplace",
            "interface": {"displayName": "Custom"},
            "plugins": [
                {
                    "name": "existing-plugin",
                    "source": {"source": "local", "path": "./.codex/plugins/existing-plugin"},
                    "policy": {"installation": "AVAILABLE", "authentication": "ON_INSTALL"},
                    "category": "Developer Tools",
                }
            ],
        },
    )

    names = [plugin["name"] for plugin in payload["plugins"]]
    assert payload["name"] == "custom-marketplace"
    assert payload["interface"]["displayName"] == "Custom"
    assert "existing-plugin" in names
    assert "skill-framework-native" in names
