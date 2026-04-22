"""Regression tests for the unified Codex native integration installer."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.materialize_cli_host_entrypoints import CLAUDE_REFRESH_COMMAND
from scripts.install_codex_native_integration import (
    DEFAULT_TUI_STATUS_ITEMS,
    build_personal_marketplace_payload,
    build_framework_server_block,
    ensure_config_file,
    ensure_default_bootstrap_bundle,
    ensure_home_claude_refresh_command,
    ensure_home_codex_skills_link,
    ensure_tui_status_line,
    install_native_integration,
    sync_directory,
)


def _install_env(tmp_path: Path) -> dict[str, str]:
    env = os.environ.copy()
    env["HOME"] = str(tmp_path / "home")
    env["CODEX_NATIVE_BOOTSTRAP_OUTPUT_DIR"] = str(tmp_path / "bootstrap")
    if "RUSTUP_HOME" not in env:
        env["RUSTUP_HOME"] = str(Path.home() / ".rustup")
    if "CARGO_HOME" not in env:
        env["CARGO_HOME"] = str(Path.home() / ".cargo")
    return env


def _run_install_skills(*args: str, env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["bash", "scripts/install_skills.sh", *args],
        cwd=PROJECT_ROOT,
        env=env,
        text=True,
        capture_output=True,
        check=True,
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


def test_ensure_tui_status_line_bootstraps_table_when_missing(tmp_path: Path) -> None:
    config_path = tmp_path / ".codex" / "config.toml"
    config_path.parent.mkdir(parents=True)
    config_path.write_text("#:schema https://developers.openai.com/codex/config-schema.json\n", encoding="utf-8")

    changed = ensure_tui_status_line(config_path)

    content = config_path.read_text(encoding="utf-8")
    assert changed is True
    assert "[tui]" in content
    assert 'status_line = ["model-with-reasoning", "git-branch", "context-used", "fast-mode"]' in content
    for item in DEFAULT_TUI_STATUS_ITEMS:
        assert f'"{item}"' in content


def test_ensure_tui_status_line_preserves_existing_tui_keys(tmp_path: Path) -> None:
    config_path = tmp_path / ".codex" / "config.toml"
    config_path.parent.mkdir(parents=True)
    config_path.write_text(
        "\n".join(
            [
                "#:schema https://developers.openai.com/codex/config-schema.json",
                "",
                "[tui]",
                'theme = "monokai-extended-bright"',
                "",
                "[mcp_servers.example]",
                'command = "python3"',
                "",
            ]
        ),
        encoding="utf-8",
    )

    changed = ensure_tui_status_line(config_path)

    content = config_path.read_text(encoding="utf-8")
    assert changed is True
    assert 'theme = "monokai-extended-bright"' in content
    assert content.count("[tui]") == 1
    assert 'status_line = ["model-with-reasoning", "git-branch", "context-used", "fast-mode"]' in content


def test_install_native_integration_is_idempotent(tmp_path: Path) -> None:
    repo_root = tmp_path / "repo"
    (repo_root / ".codex").mkdir(parents=True)
    (repo_root / ".codex" / "model_instructions.md").write_text(
        "<!-- FRAMEWORK_DEFAULT_RUNTIME_START -->\nlegacy\n<!-- FRAMEWORK_DEFAULT_RUNTIME_END -->\n",
        encoding="utf-8",
    )
    plugin_root = repo_root / "plugins" / "skill-framework-native" / ".codex-plugin"
    plugin_root.mkdir(parents=True)
    (plugin_root / "plugin.json").write_text('{"name":"skill-framework-native"}\n', encoding="utf-8")
    home_config_path = tmp_path / "home" / ".codex" / "config.toml"
    home_codex_skills_path = tmp_path / "home" / ".codex" / "skills"
    home_claude_refresh_path = tmp_path / "home" / ".claude" / "commands" / "refresh.md"
    home_plugin_root = tmp_path / "home" / ".codex" / "plugins" / "skill-framework-native"
    home_marketplace_path = tmp_path / "home" / ".agents" / "plugins" / "marketplace.json"
    bootstrap_output_dir = tmp_path / "bootstrap"
    project_instructions_path = Path(".codex") / "model_instructions.md"

    first = install_native_integration(
        home_config_path=home_config_path,
        home_codex_skills_path=home_codex_skills_path,
        home_claude_refresh_path=home_claude_refresh_path,
        repo_root=repo_root,
        home_plugin_root=home_plugin_root,
        home_marketplace_path=home_marketplace_path,
        project_instructions_path=project_instructions_path,
        bootstrap_output_dir=bootstrap_output_dir,
    )
    second = install_native_integration(
        home_config_path=home_config_path,
        home_codex_skills_path=home_codex_skills_path,
        home_claude_refresh_path=home_claude_refresh_path,
        repo_root=repo_root,
        home_plugin_root=home_plugin_root,
        home_marketplace_path=home_marketplace_path,
        project_instructions_path=project_instructions_path,
        bootstrap_output_dir=bootstrap_output_dir,
    )

    content = home_config_path.read_text(encoding="utf-8")
    instructions_path = repo_root / project_instructions_path
    instructions = instructions_path.read_text(encoding="utf-8") if instructions_path.exists() else ""
    marketplace = json.loads(home_marketplace_path.read_text(encoding="utf-8"))
    assert first["success"] is True
    assert second["success"] is True
    assert content.count("[mcp_servers.browser-mcp]") == 1
    assert content.count("[mcp_servers.framework-mcp]") == 1
    assert content.count("[tui]") == 1
    assert content.count("status_line = [") == 1
    assert home_codex_skills_path.is_symlink()
    assert home_codex_skills_path.resolve() == (repo_root / "skills").resolve()
    assert home_claude_refresh_path.read_text(encoding="utf-8") == CLAUDE_REFRESH_COMMAND
    assert (home_plugin_root / ".codex-plugin" / "plugin.json").is_file()
    assert [plugin["name"] for plugin in marketplace["plugins"]].count("skill-framework-native") == 1
    assert instructions == ""
    assert first["framework_overlay_retirement"]["status"] in {"retired-file", "retired-managed-block"}
    assert second["framework_overlay_retirement"]["status"] == "already-retired"
    assert first["default_bootstrap"]["status"] == "materialized"
    assert second["default_bootstrap"]["status"] == "already-present"
    assert Path(first["default_bootstrap"]["mirror_bootstrap_path"]).is_file()
    assert first["tui_status_line_changed"] is True
    assert second["tui_status_line_changed"] is False
    assert first["home_codex_skills_link_changed"] is True
    assert second["home_codex_skills_link_changed"] is False
    assert first["home_claude_refresh_changed"] is True
    assert second["home_claude_refresh_changed"] is False


def test_ensure_default_bootstrap_bundle_is_idempotent(tmp_path: Path) -> None:
    repo_root = tmp_path / "repo"
    output_dir = tmp_path / "bootstrap"
    repo_root.mkdir(parents=True)

    first = ensure_default_bootstrap_bundle(repo_root, output_dir=output_dir)
    second = ensure_default_bootstrap_bundle(repo_root, output_dir=output_dir)

    assert first["status"] == "materialized"
    assert second["status"] == "already-present"
    assert Path(first["mirror_bootstrap_path"]).is_file()
    assert Path(second["mirror_bootstrap_path"]).is_file()


def test_ensure_home_codex_skills_link_repoints_stale_target(tmp_path: Path) -> None:
    repo_root = tmp_path / "repo"
    (repo_root / "skills").mkdir(parents=True)
    stale_root = tmp_path / "stale"
    stale_root.mkdir(parents=True)
    target_path = tmp_path / "home" / ".codex" / "skills"
    target_path.parent.mkdir(parents=True, exist_ok=True)
    target_path.symlink_to(stale_root, target_is_directory=True)

    changed = ensure_home_codex_skills_link(repo_root, target_path=target_path)

    assert changed is True
    assert target_path.is_symlink()
    assert target_path.resolve() == (repo_root / "skills").resolve()


def test_ensure_home_claude_refresh_command_is_idempotent(tmp_path: Path) -> None:
    command_path = tmp_path / "home" / ".claude" / "commands" / "refresh.md"

    first = ensure_home_claude_refresh_command(command_path)
    second = ensure_home_claude_refresh_command(command_path)

    assert first is True
    assert second is False
    assert command_path.read_text(encoding="utf-8") == CLAUDE_REFRESH_COMMAND


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


def test_install_skills_codex_command_routes_through_native_installer(tmp_path: Path) -> None:
    env = _install_env(tmp_path)
    completed = _run_install_skills("codex", env=env)

    config_path = Path(env["HOME"]) / ".codex" / "config.toml"
    content = config_path.read_text(encoding="utf-8")
    assert "native integration installed" in completed.stdout
    assert "[mcp_servers.browser-mcp]" in content
    assert "[mcp_servers.framework-mcp]" in content
    assert (Path(env["HOME"]) / ".codex" / "skills").is_symlink()
    assert (tmp_path / "bootstrap" / "framework_default_bootstrap.json").is_file()


def test_install_skills_all_command_reports_codex_ready_and_links_other_hosts(tmp_path: Path) -> None:
    env = _install_env(tmp_path)

    completed = _run_install_skills("all", env=env)
    status = _run_install_skills("status", env=env)

    home_root = Path(env["HOME"])
    assert "Done!" in completed.stdout
    assert "native integration ready" in status.stdout
    assert (home_root / ".claude" / "skills").is_symlink()
    assert (home_root / ".agents" / "skills").is_symlink()
    assert (home_root / ".gemini" / "skills").is_symlink()


def test_install_skills_status_rejects_invalid_bootstrap_contract(tmp_path: Path) -> None:
    env = _install_env(tmp_path)
    _run_install_skills("codex", env=env)

    bootstrap_path = tmp_path / "bootstrap" / "framework_default_bootstrap.json"
    bootstrap_path.write_text("{}", encoding="utf-8")

    status = _run_install_skills("ls", env=env)

    assert "native integration incomplete" in status.stdout
    assert "bootstrap:false" in status.stdout
