from __future__ import annotations

import json
import subprocess
from pathlib import Path

import pytest


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
        [str(_router_rs_binary()), "--host-integration", *args],
        cwd=PROJECT_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    payload = json.loads(completed.stdout)
    assert isinstance(payload, dict)
    return payload


def _runtime_registry(repo_root: Path = PROJECT_ROOT) -> dict[str, object]:
    return _run_host_integration("export-runtime-registry", "--repo-root", str(repo_root))


def test_python_runtime_package_is_retired() -> None:
    assert not (PROJECT_ROOT / "framework_runtime").exists()


def test_runtime_registry_missing_file_uses_default_registry(tmp_path: Path) -> None:
    repo_root = tmp_path / "repo"
    repo_root.mkdir()

    payload = _runtime_registry(repo_root)

    assert payload["schema_version"] == "framework-runtime-registry-v1"
    assert payload["shared_project_mcp_servers"] == ["framework-mcp"]


def test_runtime_registry_prefers_repo_local_registry_for_explicit_repo_root(tmp_path: Path) -> None:
    repo_root = tmp_path / "repo"
    registry_path = repo_root / "configs" / "framework" / "RUNTIME_REGISTRY.json"
    registry_path.parent.mkdir(parents=True)
    registry_path.write_text(
        json.dumps(
            {
                "schema_version": "framework-runtime-registry-v1",
                "default_host_peer_set": ["repo-host"],
                "shared_project_mcp_servers": [],
                "workspace_bootstrap_defaults": {"skill_bridge": {"source_rel": "repo-skills"}},
                "framework_native_aliases": {"autopilot": {"canonical_owner": "repo-owner"}},
                "omc_retirement_contract": {"runtime_authority": "repo-rust"},
                "plugins": [{"plugin_name": "repo-plugin", "source_rel": "repo-plugin"}],
                "host_adapters": [],
            },
            ensure_ascii=False,
            indent=2,
        ),
        encoding="utf-8",
    )

    payload = _runtime_registry(repo_root)

    assert payload["plugins"][0]["plugin_name"] == "repo-plugin"
    assert payload["shared_project_mcp_servers"] == []
    assert payload["framework_native_aliases"]["autopilot"]["canonical_owner"] == "repo-owner"


def test_runtime_registry_exposes_framework_native_aliases_and_omc_retirement_contract() -> None:
    payload = _runtime_registry()
    aliases = payload["framework_native_aliases"]

    assert aliases["autopilot"]["canonical_owner"] == "execution-controller-coding"
    assert aliases["autopilot"]["host_entrypoints"]["codex-cli"] == "$autopilot"
    assert aliases["autopilot"]["interaction_invariants"]["implicit_route_policy"] == "never"
    assert aliases["deepinterview"]["host_entrypoints"]["claude-code"] == "/deepinterview"
    assert aliases["team"]["host_entrypoints"]["claude-code"] == "/team"
    assert aliases["team"]["route_mode"] == "team-orchestration"
    assert aliases["latex-compile-acceleration"]["host_entrypoints"]["codex-cli"] == "$latex-compile-acceleration"

    retirement = payload["omc_retirement_contract"]
    assert retirement["runtime_authority"] == "rust-session-supervisor"
    assert ".omc" in retirement["steady_state_forbidden_roots"]
    assert "external_session_supervisor" in retirement["replacement_capabilities"]


def test_runtime_registry_exposes_shared_project_mcp_servers() -> None:
    assert _runtime_registry()["shared_project_mcp_servers"] == ["framework-mcp"]


def test_runtime_registry_host_records_expose_supervisor_capabilities() -> None:
    records = {row["adapter_id"]: row for row in _runtime_registry()["host_adapters"]}
    codex = records["codex_cli_adapter"]
    claude = records["claude_code_adapter"]

    for record, expected_driver in ((codex, "codex_driver"), (claude, "claude_driver")):
        assert "external_session_supervisor" in record["host_capabilities"]
        assert "rate_limit_auto_resume" in record["host_capabilities"]
        assert "host_resume_entrypoint" in record["host_capabilities"]
        assert "host_tmux_worker_management" in record["host_capabilities"]
        assert record["protocol_hints"]["session_supervisor_driver"] == expected_driver

    assert codex["protocol_hints"]["framework_alias_entrypoints"]["autopilot"] == "$autopilot"
    assert claude["protocol_hints"]["framework_alias_entrypoints"]["deepinterview"] == "/deepinterview"
