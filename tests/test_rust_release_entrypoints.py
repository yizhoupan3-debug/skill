from __future__ import annotations

import json
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]


def test_python_wrapper_scripts_stay_retired() -> None:
    retired_paths = (
        PROJECT_ROOT / "scripts" / "materialize_cli_host_entrypoints.py",
        PROJECT_ROOT / "scripts" / "install_codex_native_integration.py",
        PROJECT_ROOT / "scripts" / "install_codex_framework_default.py",
        PROJECT_ROOT / "scripts" / "write_session_artifacts.py",
        PROJECT_ROOT / "scripts" / "runtime_background_cli.py",
        PROJECT_ROOT / "scripts" / "rust_binary_runner.py",
        PROJECT_ROOT / "scripts" / "host_integration_runner.py",
        PROJECT_ROOT / "configs" / "codex" / "model_instructions.md",
    )

    assert [path for path in retired_paths if path.exists()] == []


def test_installed_project_hooks_use_router_rs_only() -> None:
    surfaces = (
        PROJECT_ROOT / ".claude" / "settings.json",
        PROJECT_ROOT / ".codex" / "hooks.json",
    )

    for surface in surfaces:
        payload = json.loads(surface.read_text(encoding="utf-8"))
        commands = [
            hook["command"]
            for entries in payload["hooks"].values()
            for entry in entries
            for hook in entry["hooks"]
        ]
        assert commands
        assert all("router-rs" in command for command in commands)
        assert not any("python3" in command for command in commands)
        assert not any(".py" in command for command in commands)
        assert not any("host-integration-rs" in command for command in commands)


def test_repo_local_codex_framework_mcp_uses_rust_only_entrypoint() -> None:
    source = (PROJECT_ROOT / ".codex" / "config.toml").read_text(encoding="utf-8")

    assert "python3" not in source
    assert "scripts.framework_mcp" not in source
    assert "scripts/router-rs/Cargo.toml" in source
    assert "--framework-mcp-stdio" in source


def test_install_skills_uses_rust_only_entrypoints() -> None:
    source = (PROJECT_ROOT / "scripts" / "install_skills.sh").read_text(encoding="utf-8")
    assert "python3" not in source
    assert "router-rs/Cargo.toml" in source
    assert "--host-integration" in source
    assert "install-native-integration" in source
    assert "validate-default-bootstrap" in source


def test_sync_skills_uses_router_rs_directly() -> None:
    source = (PROJECT_ROOT / "scripts" / "sync_skills.py").read_text(encoding="utf-8")

    assert "materialize_cli_host_entrypoints" not in source
    assert "cargo" in source
    assert "--sync-host-entrypoints-json" in source


def test_memory_automation_lives_in_rust_host_integration() -> None:
    source = (PROJECT_ROOT / "scripts" / "router-rs" / "src" / "host_integration.rs").read_text(
        encoding="utf-8"
    )

    assert "RunMemoryAutomation" in source
    assert "run_memory_automation(" in source
