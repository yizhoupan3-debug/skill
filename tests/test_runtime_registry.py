from __future__ import annotations

import json
import sys
from pathlib import Path

import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = PROJECT_ROOT / "framework_runtime" / "src"
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

from framework_runtime.host_adapters import DEFAULT_HOST_PEER_SET, get_host_adapter
from framework_runtime import runtime_registry
from framework_runtime.runtime_registry import (
    default_host_peer_set,
    framework_native_aliases,
    host_adapter_records,
    host_adapter_record,
    omc_retirement_contract,
    plugin_records,
    shared_project_mcp_servers,
    workspace_bootstrap_defaults,
)


@pytest.fixture(autouse=True)
def _clear_runtime_registry_cache() -> None:
    runtime_registry._load_runtime_registry_cached.cache_clear()
    yield
    runtime_registry._load_runtime_registry_cached.cache_clear()


def test_host_adapter_specs_are_materialized_from_runtime_registry() -> None:
    assert tuple(DEFAULT_HOST_PEER_SET) == default_host_peer_set()

    claude_record = host_adapter_record("claude_code_adapter")
    claude_spec = get_host_adapter("claude_code_adapter")
    assert claude_spec.host_id == claude_record["host_id"]
    assert claude_spec.transport == claude_record["transport"]
    assert list(claude_spec.host_capabilities) == claude_record["host_capabilities"]
    assert list(claude_spec.protocol_hints["plugin_hook_manifest_paths"]) == claude_record["protocol_hints"][
        "plugin_hook_manifest_paths"
    ]

    legacy_record = host_adapter_record("codex_desktop_host_adapter", include_legacy_aliases=True)
    legacy_spec = get_host_adapter("codex_desktop_host_adapter", include_legacy_aliases=True)
    assert legacy_spec.protocol_hints["canonical_adapter_id"] == legacy_record["protocol_hints"][
        "canonical_adapter_id"
    ]


def test_runtime_registry_falls_back_when_generated_file_is_missing(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    baseline_payload = runtime_registry.load_runtime_registry()
    baseline_host_adapters = host_adapter_records(include_legacy_aliases=True)
    baseline_peer_set = default_host_peer_set()
    baseline_project_mcp_servers = shared_project_mcp_servers()
    baseline_plugins = plugin_records()
    baseline_bootstrap_defaults = workspace_bootstrap_defaults()

    missing_registry = tmp_path / "configs" / "framework" / "RUNTIME_REGISTRY.json"
    monkeypatch.setattr(runtime_registry, "_DEFAULT_REGISTRY_PATH", missing_registry)

    assert tuple(DEFAULT_HOST_PEER_SET) == default_host_peer_set()
    assert default_host_peer_set() == baseline_peer_set
    assert shared_project_mcp_servers() == baseline_project_mcp_servers
    assert host_adapter_records(include_legacy_aliases=True) == baseline_host_adapters

    assert plugin_records() == baseline_plugins
    assert plugin_records() == tuple(baseline_payload["plugins"])

    assert workspace_bootstrap_defaults() == baseline_bootstrap_defaults
    assert workspace_bootstrap_defaults() == baseline_payload["workspace_bootstrap_defaults"]
    assert shared_project_mcp_servers() == tuple(baseline_payload["shared_project_mcp_servers"])

    claude_record = host_adapter_record("claude_code_adapter")
    claude_spec = get_host_adapter("claude_code_adapter")
    assert claude_record["host_id"] == claude_spec.host_id
    assert claude_record["transport"] == claude_spec.transport
    assert claude_record["protocol_hints"]["plugin_hook_manifest_paths"] == list(
        claude_spec.protocol_hints["plugin_hook_manifest_paths"]
    )


def test_runtime_registry_missing_file_uses_embedded_snapshot_before_host_adapter_bridge(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    missing_registry = tmp_path / "configs" / "framework" / "RUNTIME_REGISTRY.json"
    monkeypatch.setattr(runtime_registry, "_DEFAULT_REGISTRY_PATH", missing_registry)

    def _unexpected_last_resort() -> tuple[dict[str, object], ...]:
        raise AssertionError("runtime registry fallback should not re-materialize host adapters")

    monkeypatch.setattr(
        runtime_registry,
        "_last_resort_fallback_host_adapter_rows",
        _unexpected_last_resort,
    )

    legacy_ids = [row["adapter_id"] for row in host_adapter_records(include_legacy_aliases=True)]

    assert "codex_desktop_host_adapter" in legacy_ids
    assert "claude_code_adapter" in legacy_ids


def test_runtime_registry_fallback_preserves_default_visibility_boundary(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    baseline_default_ids = [
        row["adapter_id"] for row in host_adapter_records() if row["registry_lane"] == "default"
    ]
    missing_registry = tmp_path / "configs" / "framework" / "RUNTIME_REGISTRY.json"
    monkeypatch.setattr(runtime_registry, "_DEFAULT_REGISTRY_PATH", missing_registry)

    default_ids = [row["adapter_id"] for row in host_adapter_records()]
    legacy_ids = [row["adapter_id"] for row in host_adapter_records(include_legacy_aliases=True)]

    assert "codex_desktop_host_adapter" not in default_ids
    assert "codex_desktop_host_adapter" in legacy_ids
    assert default_ids == baseline_default_ids


def test_runtime_registry_prefers_rust_export_for_explicit_repo_root(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    repo_root = tmp_path / "repo"
    (repo_root / "configs" / "framework").mkdir(parents=True)
    registry_path = repo_root / "configs" / "framework" / "RUNTIME_REGISTRY.json"
    registry_path.write_text(
        json.dumps(
            {
                "schema_version": "framework-runtime-registry-v1",
                "default_host_peer_set": ["repo-host"],
                "shared_project_mcp_servers": ["framework-mcp"],
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

    captured: list[str] = []

    def _fake_run_host_integration_rs(*args: str) -> dict[str, object]:
        captured.extend(args)
        return json.loads(registry_path.read_text(encoding="utf-8"))

    monkeypatch.setattr(runtime_registry, "run_host_integration_rs", _fake_run_host_integration_rs)

    payload = runtime_registry.load_runtime_registry(repo_root=repo_root)

    assert payload["plugins"][0]["plugin_name"] == "repo-plugin"
    assert captured == ["export-runtime-registry", "--repo-root", str(repo_root.resolve())]


def test_runtime_registry_exposes_framework_native_aliases_and_omc_retirement_contract() -> None:
    aliases = framework_native_aliases()
    assert aliases["autopilot"]["canonical_owner"] == "execution-controller-coding"
    assert aliases["autopilot"]["host_entrypoints"]["codex-cli"] == "$autopilot"
    assert aliases["autopilot"]["omc_lineage"]["inherits_core_capabilities"] is True
    assert aliases["autopilot"]["upstream_source"]["tag"] == "v4.13.2"
    assert "qa" in aliases["autopilot"]["official_workflow"]["phases"]
    assert "resume-and-recovery-required" in aliases["autopilot"]["implementation_bar"]
    assert aliases["deepinterview"]["canonical_owner"] == "code-review"
    assert aliases["deepinterview"]["host_entrypoints"]["claude-code"] == "/deepinterview"
    assert aliases["deepinterview"]["omc_lineage"]["inherits_core_capabilities"] is True
    assert aliases["deepinterview"]["upstream_source"]["official_skill_path"] == "skills/deep-interview/SKILL.md"
    assert "one-question-at-a-time" in aliases["deepinterview"]["official_workflow"]["loop_rules"]
    assert "verification-evidence-required" in aliases["deepinterview"]["implementation_bar"]
    assert aliases["team"]["canonical_owner"] == "execution-controller-coding"
    assert aliases["team"]["host_entrypoints"]["codex-cli"] == "$team"
    assert aliases["team"]["host_entrypoints"]["claude-code"] == "/team"
    assert aliases["team"]["omc_lineage"]["inherits_core_capabilities"] is True
    assert aliases["team"]["upstream_source"]["official_skill_path"] == "skills/team/SKILL.md"
    assert "delegation" in aliases["team"]["official_workflow"]["phases"]
    assert "supervisor-owned-continuity" in aliases["team"]["implementation_bar"]

    retirement = omc_retirement_contract()
    assert retirement["runtime_authority"] == "rust-session-supervisor"
    assert ".omc" in retirement["steady_state_forbidden_roots"]
    assert "external_session_supervisor" in retirement["replacement_capabilities"]
    assert retirement["framework_native_alias_guarantees"]["autopilot"]["inherits_omc_core_capabilities"] is True
    assert (
        "fix-verify-loop-until-bounded-scope-clean"
        in retirement["framework_native_alias_guarantees"]["deepinterview"]["implementation_bar"]
    )
    assert retirement["framework_native_alias_guarantees"]["team"]["inherits_omc_core_capabilities"] is True
    assert (
        "supervisor-owned-continuity"
        in retirement["framework_native_alias_guarantees"]["team"]["implementation_bar"]
    )


def test_runtime_registry_exposes_shared_project_mcp_servers() -> None:
    assert shared_project_mcp_servers() == ("browser-mcp", "framework-mcp", "openaiDeveloperDocs")


def test_runtime_registry_host_records_expose_supervisor_capabilities() -> None:
    codex = host_adapter_record("codex_cli_adapter")
    claude = host_adapter_record("claude_code_adapter")

    for record, expected_driver in ((codex, "codex_driver"), (claude, "claude_driver")):
        assert "external_session_supervisor" in record["host_capabilities"]
        assert "rate_limit_auto_resume" in record["host_capabilities"]
        assert "host_resume_entrypoint" in record["host_capabilities"]
        assert "host_tmux_worker_management" in record["host_capabilities"]
        assert record["protocol_hints"]["session_supervisor_driver"] == expected_driver

    assert codex["protocol_hints"]["framework_alias_entrypoints"]["autopilot"] == "$autopilot"
    assert codex["protocol_hints"]["framework_alias_entrypoints"]["team"] == "$team"
    assert claude["protocol_hints"]["framework_alias_entrypoints"]["deepinterview"] == "/deepinterview"
    assert claude["protocol_hints"]["framework_alias_entrypoints"]["team"] == "/team"
