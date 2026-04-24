from __future__ import annotations

import json
import subprocess
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]
ROUTER_RS_ROOT = PROJECT_ROOT / "scripts" / "router-rs"
ROUTER_RS_DEBUG_BIN = ROUTER_RS_ROOT / "target" / "debug" / "router-rs"
ROUTER_RS_RELEASE_BIN = ROUTER_RS_ROOT / "target" / "release" / "router-rs"


def _router_rs_binary() -> Path:
    candidates = [path for path in (ROUTER_RS_RELEASE_BIN, ROUTER_RS_DEBUG_BIN) if path.is_file()]
    assert candidates
    return max(candidates, key=lambda path: (path.stat().st_mtime, path.name))


def _run_host_integration(*args: str) -> dict[str, object]:
    completed = subprocess.run(
        [
            str(_router_rs_binary()),
            "--host-integration",
            *args,
        ],
        cwd=PROJECT_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    payload = json.loads(completed.stdout)
    assert isinstance(payload, dict)
    return payload


def test_install_native_integration_is_idempotent(tmp_path: Path) -> None:
    repo_root = tmp_path / "repo"
    (repo_root / ".codex").mkdir(parents=True)
    (repo_root / "skills").mkdir(parents=True)
    plugin_root = repo_root / "plugins" / "skill-framework-native" / ".codex-plugin"
    plugin_root.mkdir(parents=True)
    (plugin_root / "plugin.json").write_text('{"name":"skill-framework-native"}\n', encoding="utf-8")

    home_config_path = tmp_path / "home" / ".codex" / "config.toml"
    home_codex_skills_path = tmp_path / "home" / ".codex" / "skills"
    home_claude_skills_path = tmp_path / "home" / ".claude" / "skills"
    home_claude_refresh_path = tmp_path / "home" / ".claude" / "commands" / "refresh.md"
    home_claude_mcp_config_path = tmp_path / "home" / ".claude.json"
    home_plugin_root = tmp_path / "home" / ".codex" / "plugins" / "skill-framework-native"
    home_marketplace_path = tmp_path / "home" / ".agents" / "plugins" / "marketplace.json"
    bootstrap_output_dir = tmp_path / "bootstrap"

    command = [
        "install-native-integration",
        "--repo-root",
        str(repo_root),
        "--home-config-path",
        str(home_config_path),
        "--home-plugin-root",
        str(home_plugin_root),
        "--home-marketplace-path",
        str(home_marketplace_path),
        "--home-codex-skills-path",
        str(home_codex_skills_path),
        "--home-claude-skills-path",
        str(home_claude_skills_path),
        "--home-claude-refresh-path",
        str(home_claude_refresh_path),
        "--home-claude-mcp-config-path",
        str(home_claude_mcp_config_path),
        "--bootstrap-output-dir",
        str(bootstrap_output_dir),
        "--skip-home-claude-skills-link",
        "--skip-home-claude-refresh",
    ]

    first = _run_host_integration(*command)
    second = _run_host_integration(*command)

    content = home_config_path.read_text(encoding="utf-8")
    claude_mcp_payload = json.loads(home_claude_mcp_config_path.read_text(encoding="utf-8"))
    plugin_mcp_payload = json.loads((home_plugin_root / ".mcp.json").read_text(encoding="utf-8"))
    marketplace = json.loads(home_marketplace_path.read_text(encoding="utf-8"))

    assert first["success"] is True
    assert second["success"] is True
    assert content.count("[features]") == 1
    assert content.count("codex_hooks = true") == 1
    assert content.count("[mcp_servers.browser-mcp]") == 0
    assert content.count("[mcp_servers.framework-mcp]") == 1
    assert content.count("[mcp_servers.openaiDeveloperDocs]") == 0
    assert content.count("[tui]") == 1
    assert home_codex_skills_path.is_symlink()
    assert home_codex_skills_path.resolve() == (repo_root / "skills").resolve()
    assert not home_claude_skills_path.exists()
    assert not home_claude_refresh_path.exists()
    assert claude_mcp_payload["mcpServers"]["framework-mcp"]["args"] == [
        "--framework-mcp-stdio",
        "--repo-root",
        str(repo_root.resolve()),
    ]
    assert set(claude_mcp_payload["mcpServers"]) == {"framework-mcp"}
    assert plugin_mcp_payload["mcpServers"]["framework-mcp"]["cwd"] == str(repo_root.resolve())
    assert set(plugin_mcp_payload["mcpServers"]) == {"framework-mcp"}
    assert [plugin["name"] for plugin in marketplace["plugins"]].count("skill-framework-native") == 1
    assert first["default_bootstrap"]["status"] == "materialized"
    assert second["default_bootstrap"]["status"] == "already-present"
    assert first["home_codex_skills_link_changed"] is True
    assert second["home_codex_skills_link_changed"] is False


def test_ensure_default_bootstrap_is_idempotent(tmp_path: Path) -> None:
    repo_root = tmp_path / "repo"
    output_dir = tmp_path / "bootstrap"
    repo_root.mkdir(parents=True)

    first = _run_host_integration(
        "ensure-default-bootstrap",
        "--repo-root",
        str(repo_root),
        "--output-dir",
        str(output_dir),
    )
    second = _run_host_integration(
        "ensure-default-bootstrap",
        "--repo-root",
        str(repo_root),
        "--output-dir",
        str(output_dir),
    )

    assert first["status"] == "materialized"
    assert second["status"] == "already-present"


def test_install_native_integration_can_opt_into_rust_browser_mcp(tmp_path: Path) -> None:
    repo_root = tmp_path / "repo"
    (repo_root / ".codex").mkdir(parents=True)
    (repo_root / "skills").mkdir(parents=True)
    plugin_root = repo_root / "plugins" / "skill-framework-native" / ".codex-plugin"
    plugin_root.mkdir(parents=True)
    (plugin_root / "plugin.json").write_text('{"name":"skill-framework-native"}\n', encoding="utf-8")

    home_config_path = tmp_path / "home" / ".codex" / "config.toml"
    command = [
        "install-native-integration",
        "--repo-root",
        str(repo_root),
        "--home-config-path",
        str(home_config_path),
        "--home-plugin-root",
        str(tmp_path / "home" / ".codex" / "plugins" / "skill-framework-native"),
        "--home-marketplace-path",
        str(tmp_path / "home" / ".agents" / "plugins" / "marketplace.json"),
        "--home-codex-skills-path",
        str(tmp_path / "home" / ".codex" / "skills"),
        "--home-claude-skills-path",
        str(tmp_path / "home" / ".claude" / "skills"),
        "--home-claude-refresh-path",
        str(tmp_path / "home" / ".claude" / "commands" / "refresh.md"),
        "--home-claude-mcp-config-path",
        str(tmp_path / "home" / ".claude.json"),
        "--with-browser-mcp",
        "--skip-personal-plugin",
        "--skip-personal-marketplace",
        "--skip-home-codex-skills-link",
        "--skip-home-claude-skills-link",
        "--skip-home-claude-refresh",
        "--skip-home-claude-mcp-sync",
        "--skip-default-bootstrap",
    ]

    result = _run_host_integration(*command)
    content = home_config_path.read_text(encoding="utf-8")

    assert result["browser_mcp_changed"] is True
    assert "[mcp_servers.browser-mcp]" in content
    assert (
        'command = "'
        + str(repo_root / "scripts" / "router-rs" / "target" / "release" / "router-rs")
        + '"'
    ) in content
    assert "--browser-mcp-stdio" in content
    assert "tools/browser-mcp/dist/index.js" not in content
    assert 'command = "node"' not in content


def test_install_skills_rust_entrypoint_links_supported_tools(tmp_path: Path) -> None:
    repo_root = tmp_path / "repo"
    home = tmp_path / "home"
    (repo_root / ".codex").mkdir(parents=True)
    (repo_root / "skills" / "demo").mkdir(parents=True)
    plugin_root = repo_root / "plugins" / "skill-framework-native" / ".codex-plugin"
    plugin_root.mkdir(parents=True)
    (plugin_root / "plugin.json").write_text('{"name":"skill-framework-native"}\n', encoding="utf-8")

    first = _run_host_integration(
        "install-skills",
        "--repo-root",
        str(repo_root),
        "--home",
        str(home),
        "--bootstrap-output-dir",
        str(tmp_path / "bootstrap"),
        "all",
    )
    second = _run_host_integration(
        "install-skills",
        "--repo-root",
        str(repo_root),
        "--home",
        str(home),
        "--bootstrap-output-dir",
        str(tmp_path / "bootstrap"),
        "status",
    )

    assert first["success"] is True
    assert first["results"]["codex"]["status"] == "installed"
    assert first["results"]["agents"]["status"] == "linked"
    assert first["results"]["gemini"]["status"] == "linked"
    assert (home / ".agents" / "skills").resolve() == (repo_root / "skills").resolve()
    assert (home / ".gemini" / "skills").resolve() == (repo_root / "skills").resolve()
    assert second["results"]["codex"]["ready"] is True
    assert second["results"]["agents"]["ready"] is True
    assert second["results"]["gemini"]["ready"] is True


def test_validation_subcommands_cover_install_skills_contract(tmp_path: Path) -> None:
    repo_root = tmp_path / "repo"
    repo_root.mkdir(parents=True)
    bootstrap_path = tmp_path / "framework_default_bootstrap.json"
    bootstrap_path.write_text(
        json.dumps(
            {
                "bootstrap": {"repo_root": str(repo_root.resolve())},
                "memory-bootstrap": {},
                "skills-export": {"source": "skills/SKILL_ROUTING_RUNTIME.json"},
                "evolution-proposals": {},
            },
            ensure_ascii=False,
        ),
        encoding="utf-8",
    )
    marketplace_path = tmp_path / "marketplace.json"
    marketplace_path.write_text(
        json.dumps({"plugins": [{"name": "skill-framework-native"}]}, ensure_ascii=False),
        encoding="utf-8",
    )

    bootstrap_ok = _run_host_integration(
        "validate-default-bootstrap",
        "--bootstrap-path",
        str(bootstrap_path),
        "--repo-root",
        str(repo_root),
    )
    marketplace_ok = _run_host_integration(
        "validate-marketplace-plugin",
        "--marketplace-path",
        str(marketplace_path),
        "--plugin-name",
        "skill-framework-native",
    )
    source_path = _run_host_integration(
        "resolve-skill-bridge-source",
        "--repo-root",
        str(repo_root),
    )

    assert bootstrap_ok["ok"] is True
    assert marketplace_ok["ok"] is True
    assert source_path["path"] == str((repo_root / "skills").resolve())
