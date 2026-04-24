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
        PROJECT_ROOT / "skills" / "autoresearch" / "scripts" / "research_ctl.py",
        PROJECT_ROOT / "skills" / "autoresearch" / "scripts" / "init_research.py",
    )

    assert [path for path in retired_paths if path.exists()] == []


def test_autoresearch_uses_rust_only_controller() -> None:
    skill_dir = PROJECT_ROOT / "skills" / "autoresearch"
    skill_doc = (skill_dir / "SKILL.md").read_text(encoding="utf-8")
    rust_source = PROJECT_ROOT / "scripts" / "autoresearch-rs" / "src" / "main.rs"

    assert rust_source.exists()
    assert not (skill_dir / "scripts").exists()
    assert "scripts/autoresearch-rs" in skill_doc
    assert "research_ctl.py" not in skill_doc
    assert "init_research.py" not in skill_doc


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
    assert 'command = "/Users/joe/Documents/skill/scripts/router-rs/target/release/router-rs"' in source
    assert "scripts/router-rs/Cargo.toml" not in source
    assert 'command = "cargo"' not in source
    assert "--framework-mcp-stdio" in source


def test_install_skills_uses_rust_only_entrypoints() -> None:
    assert not (PROJECT_ROOT / "scripts" / "install_skills.sh").exists()
    source = (PROJECT_ROOT / "scripts" / "router-rs" / "src" / "host_integration.rs").read_text(
        encoding="utf-8"
    )
    assert "InstallSkills" in source
    assert "InstallNativeIntegration" in source
    assert "validate_default_bootstrap" in source


def test_sync_skills_uses_router_rs_directly() -> None:
    assert not (PROJECT_ROOT / "scripts" / "sync_skills.py").exists()
    source = (PROJECT_ROOT / "scripts" / "router-rs" / "src" / "claude_hooks.rs").read_text(
        encoding="utf-8"
    )
    assert "sync_host_entrypoints" in source


def test_memory_automation_lives_in_rust_host_integration() -> None:
    source = (PROJECT_ROOT / "scripts" / "router-rs" / "src" / "host_integration.rs").read_text(
        encoding="utf-8"
    )

    assert "RunMemoryAutomation" in source
    assert "run_memory_automation(" in source


def test_screenshot_skill_uses_workspace_rust_binary_entrypoint() -> None:
    skill_doc = (PROJECT_ROOT / "skills" / "screenshot" / "SKILL.md").read_text(encoding="utf-8")
    reference_doc = (
        PROJECT_ROOT / "skills" / "screenshot" / "references" / "os_commands.md"
    ).read_text(encoding="utf-8")
    manifest = (PROJECT_ROOT / "rust_tools" / "screenshot_rs" / "Cargo.toml").read_text(
        encoding="utf-8"
    )

    assert '[[bin]]\nname = "screenshot"' in manifest
    assert '[[bin]]\nname = "screenshot_rs"' not in manifest
    assert "rust_tools/Cargo.toml --release --bin screenshot" in skill_doc
    assert "rust_tools/Cargo.toml --release --bin screenshot" in reference_doc
    assert "rust_tools/screenshot_rs/Cargo.toml --release" not in skill_doc
    assert "rust_tools/screenshot_rs/Cargo.toml --release" not in reference_doc
