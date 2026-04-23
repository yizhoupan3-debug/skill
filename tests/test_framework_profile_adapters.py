from __future__ import annotations

import importlib
import io
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = PROJECT_ROOT / "framework_runtime" / "src"
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

import framework_runtime.profile_artifacts as profile_artifacts_module
import framework_runtime.host_adapters as host_adapters_module
import framework_runtime.framework_artifact_contracts as codex_artifact_contracts_module
from framework_runtime.framework_artifact_contracts import (
    build_cli_family_capability_discovery,
    build_cli_family_parity_snapshot,
    build_codex_dual_entry_parity_snapshot,
    build_cli_family_capability_discovery as rust_build_cli_family_capability_discovery,
    build_codex_dual_entry_parity_snapshot as rust_build_codex_dual_entry_parity_snapshot,
)
from framework_runtime.control_plane_contracts import (
    build_control_plane_contract_descriptors,
    build_delegation_contract,
    build_execution_controller_contract,
    build_execution_kernel_live_fallback_retirement_status,
    build_execution_kernel_live_response_serialization_contract,
    build_supervisor_state_contract,
)
from framework_runtime.framework_profile import (
    CORE_CAPABILITIES,
    FRAMEWORK_SHARED_CONTRACT_FIELDS,
    FRAMEWORK_SHARED_CONTRACT_SCHEMA_VERSION,
    FrameworkProfile,
    build_framework_profile,
    ensure_capabilities,
    merge_profile_overrides,
    resolve_host_capability_requirements,
)
from framework_runtime.execution_kernel_contracts import (
    RUNTIME_TRACE_METADATA_FIELDS,
    build_execution_kernel_live_response_serialization_contract_core,
)
from framework_runtime.host_adapters import (
    AIONRS_COMPANION_ADAPTER,
    AIONUI_HOST_ADAPTER,
    CLAUDE_CODE_ADAPTER,
    CLI_COMMON_ADAPTER,
    CODEX_CLI_ADAPTER,
    CODEX_COMMON_ADAPTER,
    CODEX_DESKTOP_ADAPTER,
    CODEX_DESKTOP_HOST_ADAPTER,
    GEMINI_CLI_ADAPTER,
    GENERIC_HOST_ADAPTER,
    get_host_adapter,
    list_host_adapters,
    compile_claude_code_adapter,
    adapt_framework_profile,
    compile_codex_cli_adapter,
    compile_codex_common_adapter,
    compile_codex_desktop_adapter,
    compile_cli_common_adapter,
    compile_gemini_cli_adapter,
)
from framework_runtime.host_adapter_compatibility import (
    compatibility_snapshot,
    build_codex_desktop_alias_retirement_status,
    build_upgrade_compatibility_matrix,
    compile_aionrs_companion_adapter,
    compile_aionui_host_adapter,
    validate_adapter_compatibility,
)
from framework_runtime.rust_router import RustRouteAdapter
import framework_runtime.rust_router as rust_router_module
from framework_runtime.runtime_registry import framework_native_aliases
from framework_runtime.trace import (
    TRACE_EVENT_BRIDGE_SCHEMA_VERSION,
    TRACE_EVENT_TRANSPORT_SCHEMA_VERSION,
    TRACE_REPLAY_CURSOR_SCHEMA_VERSION,
)

ROUTER_RS_MANIFEST = PROJECT_ROOT / "scripts" / "router-rs" / "Cargo.toml"
ROUTER_RS_MAIN = PROJECT_ROOT / "scripts" / "router-rs" / "src" / "main.rs"
ROUTER_RS_PROFILE_MOD = PROJECT_ROOT / "scripts" / "router-rs" / "src" / "framework_profile.rs"
ROUTER_RS_DEBUG_BIN = PROJECT_ROOT / "scripts" / "router-rs" / "target" / "debug" / "router-rs"
ROUTER_RS_RELEASE_BIN = PROJECT_ROOT / "scripts" / "router-rs" / "target" / "release" / "router-rs"


def _router_rs_command() -> list[str]:
    return RustRouteAdapter(PROJECT_ROOT)._binary_command()


def test_run_framework_contract_artifacts_cli_emits_default_lane_without_rust_bundle(
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    profile = build_framework_profile(
        profile_id="cli-artifact-profile",
        display_name="CLI Artifact Profile",
        rules_bundle={"rules": [{"id": "outer-owned"}]},
        skill_bundle={"skills": ["router", "memory-bridge"]},
        session_policy={"mode": "bounded", "approval_mode": "manual"},
        artifact_contract={"layout": "stable-v1"},
        model_policy={"provider": "openai", "model": "gpt-5"},
        memory_mounts=("project",),
        mcp_servers=("local-memory",),
    )
    profile_path = tmp_path / "framework_profile.json"
    output_dir = tmp_path / "artifacts"
    profile_path.write_text(json.dumps(profile.to_dict(), ensure_ascii=False), encoding="utf-8")

    exit_code = rust_router_module.run_framework_contract_artifacts_cli(
        codex_home=PROJECT_ROOT,
        argv=[
            "--framework-profile",
            str(profile_path),
            "--output-dir",
            str(output_dir),
        ],
    )

    assert exit_code == 0
    payload = json.loads(capsys.readouterr().out)
    assert "framework_profile" in payload
    assert "cli_common_adapter" in payload
    assert "codex_dual_entry_parity_snapshot" in payload
    assert "rust_profile_bundle" not in payload
    assert Path(payload["framework_profile"]).is_file()
    assert Path(payload["cli_common_adapter"]).parent.name == "default"


def test_run_framework_contract_artifacts_cli_reuses_shared_route_adapter_for_rust_bundle(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    profile = build_framework_profile(
        profile_id="cli-artifact-profile-rust",
        display_name="CLI Artifact Profile Rust",
        rules_bundle={"rules": [{"id": "outer-owned"}]},
        skill_bundle={"skills": ["router"]},
        session_policy={"mode": "bounded"},
        artifact_contract={"layout": "stable-v1"},
        memory_mounts=("project",),
        mcp_servers=("local-memory",),
    )
    profile_path = tmp_path / "framework_profile.json"
    output_dir = tmp_path / "artifacts"
    profile_path.write_text(json.dumps(profile.to_dict(), ensure_ascii=False), encoding="utf-8")

    fake_adapter = object()
    captured: dict[str, object] = {}

    def _fake_route_adapter(**kwargs: object) -> object:
        captured["codex_home"] = kwargs["codex_home"]
        return fake_adapter

    def _fake_emit(
        output_dir_arg: Path,
        *,
        profile: FrameworkProfile,
        rust_adapter: object | None,
        include_fallback_artifacts: bool,
        include_compatibility_inventory: bool,
        include_legacy_alias_artifact: bool,
    ) -> dict[str, str]:
        captured["profile_id"] = profile.profile_id
        captured["rust_adapter"] = rust_adapter
        captured["include_fallback_artifacts"] = include_fallback_artifacts
        captured["include_compatibility_inventory"] = include_compatibility_inventory
        captured["include_legacy_alias_artifact"] = include_legacy_alias_artifact
        marker_path = output_dir_arg / "marker.json"
        marker_path.parent.mkdir(parents=True, exist_ok=True)
        marker_path.write_text("{}", encoding="utf-8")
        return {"marker": str(marker_path)}

    monkeypatch.setattr(rust_router_module, "route_adapter", _fake_route_adapter)
    monkeypatch.setattr(profile_artifacts_module, "emit_framework_contract_artifacts", _fake_emit)

    exit_code = rust_router_module.run_framework_contract_artifacts_cli(
        codex_home=PROJECT_ROOT,
        argv=[
            "--framework-profile",
            str(profile_path),
            "--output-dir",
            str(output_dir),
            "--include-rust-bundle",
            "--include-fallback-artifacts",
            "--include-compatibility-inventory",
            "--include-legacy-alias-artifact",
        ],
    )

    assert exit_code == 0
    payload = json.loads(capsys.readouterr().out)
    assert payload == {"marker": str(output_dir / "marker.json")}
    assert captured["codex_home"] == PROJECT_ROOT
    assert captured["profile_id"] == "cli-artifact-profile-rust"
    assert captured["rust_adapter"] is fake_adapter
    assert captured["include_fallback_artifacts"] is True
    assert captured["include_compatibility_inventory"] is True
    assert captured["include_legacy_alias_artifact"] is True


def test_rust_route_adapter_rejects_stale_binary_when_sources_are_newer() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        codex_home = Path(tmpdir)
        router_dir = codex_home / "scripts" / "router-rs"
        source_dir = router_dir / "src"
        debug_bin = router_dir / "target" / "debug" / "router-rs"
        source_dir.mkdir(parents=True)
        debug_bin.parent.mkdir(parents=True)
        (router_dir / "Cargo.toml").write_text("[package]\nname='router-rs'\nversion='0.1.0'\n", encoding="utf-8")
        (source_dir / "main.rs").write_text("fn main() {}\n", encoding="utf-8")
        debug_bin.write_text("stub", encoding="utf-8")

        os.utime(debug_bin, (1_700_000_000, 1_700_000_000))
        os.utime(source_dir / "main.rs", (1_700_000_100, 1_700_000_100))

        adapter = RustRouteAdapter(codex_home)

        with pytest.raises(RuntimeError, match="prebuilt binary is stale"):
            adapter._binary_command()
        health = adapter.health()
        assert health["resolved_binary"] == str(debug_bin)
        assert health["source_newer_than_resolved_binary"] is True


def test_rust_route_adapter_stdio_client_key_changes_after_binary_rebuild() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        codex_home = Path(tmpdir)
        router_dir = codex_home / "scripts" / "router-rs"
        source_dir = router_dir / "src"
        release_bin = router_dir / "target" / "release" / "router-rs"
        source_dir.mkdir(parents=True)
        release_bin.parent.mkdir(parents=True)
        (router_dir / "Cargo.toml").write_text("[package]\nname='router-rs'\nversion='0.1.0'\n", encoding="utf-8")
        (source_dir / "main.rs").write_text("fn main() {}\n", encoding="utf-8")
        release_bin.write_text("release", encoding="utf-8")

        os.utime(router_dir / "Cargo.toml", (1_700_000_050, 1_700_000_050))
        os.utime(source_dir / "main.rs", (1_700_000_000, 1_700_000_000))
        os.utime(release_bin, (1_700_000_100, 1_700_000_100))

        adapter = RustRouteAdapter(codex_home)
        first_key = adapter._stdio_client_key(adapter._stdio_command())

        os.utime(release_bin, (1_700_000_200, 1_700_000_200))

        second_key = adapter._stdio_client_key(adapter._stdio_command())

        assert first_key != second_key


def test_rust_route_adapter_uses_fresh_debug_binary_when_release_missing() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        codex_home = Path(tmpdir)
        router_dir = codex_home / "scripts" / "router-rs"
        source_dir = router_dir / "src"
        debug_bin = router_dir / "target" / "debug" / "router-rs"
        source_dir.mkdir(parents=True)
        debug_bin.parent.mkdir(parents=True)
        (router_dir / "Cargo.toml").write_text("[package]\nname='router-rs'\nversion='0.1.0'\n", encoding="utf-8")
        (source_dir / "main.rs").write_text("fn main() {}\n", encoding="utf-8")
        debug_bin.write_text("debug", encoding="utf-8")

        os.utime(router_dir / "Cargo.toml", (1_700_000_050, 1_700_000_050))
        os.utime(source_dir / "main.rs", (1_700_000_000, 1_700_000_000))
        os.utime(debug_bin, (1_700_000_100, 1_700_000_100))

        adapter = RustRouteAdapter(codex_home)

        assert adapter._binary_command() == [str(debug_bin)]
        assert adapter.health()["resolved_binary"] == str(debug_bin)


def test_rust_route_adapter_prefers_release_binary_across_debug_and_release() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        codex_home = Path(tmpdir)
        router_dir = codex_home / "scripts" / "router-rs"
        source_dir = router_dir / "src"
        debug_bin = router_dir / "target" / "debug" / "router-rs"
        release_bin = router_dir / "target" / "release" / "router-rs"
        source_dir.mkdir(parents=True)
        debug_bin.parent.mkdir(parents=True)
        release_bin.parent.mkdir(parents=True)
        (router_dir / "Cargo.toml").write_text("[package]\nname='router-rs'\nversion='0.1.0'\n", encoding="utf-8")
        (source_dir / "main.rs").write_text("fn main() {}\n", encoding="utf-8")
        debug_bin.write_text("debug", encoding="utf-8")
        release_bin.write_text("release", encoding="utf-8")

        os.utime(router_dir / "Cargo.toml", (1_700_000_050, 1_700_000_050))
        os.utime(source_dir / "main.rs", (1_700_000_100, 1_700_000_100))
        os.utime(debug_bin, (1_700_000_000, 1_700_000_000))
        os.utime(release_bin, (1_700_000_200, 1_700_000_200))

        adapter = RustRouteAdapter(codex_home)
        assert adapter._binary_command() == [str(release_bin)]
        assert adapter.health()["resolved_binary"] == str(release_bin)


def test_rust_route_adapter_prefers_fresher_debug_binary_when_release_is_stale() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        codex_home = Path(tmpdir)
        router_dir = codex_home / "scripts" / "router-rs"
        source_dir = router_dir / "src"
        debug_bin = router_dir / "target" / "debug" / "router-rs"
        release_bin = router_dir / "target" / "release" / "router-rs"
        source_dir.mkdir(parents=True)
        debug_bin.parent.mkdir(parents=True)
        release_bin.parent.mkdir(parents=True)
        (router_dir / "Cargo.toml").write_text("[package]\nname='router-rs'\nversion='0.1.0'\n", encoding="utf-8")
        (source_dir / "main.rs").write_text("fn main() {}\n", encoding="utf-8")
        debug_bin.write_text("debug", encoding="utf-8")
        release_bin.write_text("release", encoding="utf-8")

        os.utime(router_dir / "Cargo.toml", (1_700_000_050, 1_700_000_050))
        os.utime(source_dir / "main.rs", (1_700_000_100, 1_700_000_100))
        os.utime(release_bin, (1_700_000_000, 1_700_000_000))
        os.utime(debug_bin, (1_700_000_200, 1_700_000_200))

        adapter = RustRouteAdapter(codex_home)
        assert adapter._binary_command() == [str(debug_bin)]
        assert adapter.health()["resolved_binary"] == str(debug_bin)


def test_rust_route_adapter_health_reuses_cached_source_mtime() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        codex_home = Path(tmpdir)
        router_dir = codex_home / "scripts" / "router-rs"
        source_dir = router_dir / "src"
        debug_bin = router_dir / "target" / "debug" / "router-rs"
        source_dir.mkdir(parents=True)
        debug_bin.parent.mkdir(parents=True)
        (router_dir / "Cargo.toml").write_text("[package]\nname='router-rs'\nversion='0.1.0'\n", encoding="utf-8")
        (source_dir / "main.rs").write_text("fn main() {}\n", encoding="utf-8")
        debug_bin.write_text("debug", encoding="utf-8")

        adapter = RustRouteAdapter(codex_home)
        first = adapter.health()
        adapter._latest_source_mtime = lambda: (_ for _ in ()).throw(AssertionError("source scan should stay cached"))
        second = adapter.health()

        assert first["latest_source_mtime"] == second["latest_source_mtime"]


def test_rust_route_adapter_requires_prebuilt_binary_when_none_exists() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        codex_home = Path(tmpdir)
        router_dir = codex_home / "scripts" / "router-rs"
        source_dir = router_dir / "src"
        source_dir.mkdir(parents=True)
        (router_dir / "Cargo.toml").write_text("[package]\nname='router-rs'\nversion='0.1.0'\n", encoding="utf-8")
        (source_dir / "main.rs").write_text("fn main() {}\n", encoding="utf-8")

        adapter = RustRouteAdapter(codex_home)

        with pytest.raises(RuntimeError, match="requires a prebuilt binary"):
            adapter._binary_command()
        health = adapter.health()
        assert health["resolved_binary"] is None
        assert health["available"] is True


def test_rust_route_adapter_reuses_stdio_process_for_hot_commands(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    codex_home = tmp_path
    router_dir = codex_home / "scripts" / "router-rs"
    source_dir = router_dir / "src"
    release_bin = router_dir / "target" / "release" / "router-rs"
    source_dir.mkdir(parents=True)
    release_bin.parent.mkdir(parents=True)
    (router_dir / "Cargo.toml").write_text("[package]\nname='router-rs'\nversion='0.1.0'\n", encoding="utf-8")
    (source_dir / "main.rs").write_text("fn main() {}\n", encoding="utf-8")
    release_bin.write_text("release", encoding="utf-8")

    os.utime(router_dir / "Cargo.toml", (1_700_000_050, 1_700_000_050))
    os.utime(source_dir / "main.rs", (1_700_000_000, 1_700_000_000))
    os.utime(release_bin, (1_700_000_100, 1_700_000_100))

    rust_router_module._close_router_stdio_clients()
    adapter = RustRouteAdapter(codex_home)

    class _FakeStdout:
        def __init__(self) -> None:
            self._lines: list[str] = []

        def push(self, line: str) -> None:
            self._lines.append(line)

        def readline(self) -> str:
            if not self._lines:
                return ""
            return self._lines.pop(0)

        def close(self) -> None:
            return None

    class _FakeStdin:
        def __init__(self, owner: "_FakePopen") -> None:
            self._owner = owner

        def write(self, data: str) -> int:
            request = json.loads(data)
            self._owner.requests.append(request)
            op = request["op"]
            payload = {
                "runtime_control_plane": {
                    "schema_version": adapter.runtime_control_plane_schema_version,
                    "authority": adapter.runtime_control_plane_authority,
                },
                    "search_skills": {
                        "search_schema_version": adapter.search_schema_version,
                        "authority": adapter.route_authority,
                        "query": "迭代 优化",
                        "matches": [
                            {
                                "record": {
                                    "name": "iterative-optimizer",
                                    "routing_layer": "L2",
                                    "routing_owner": "codex",
                                    "routing_gate": "none",
                                    "description": "Iterative optimization loop",
                                },
                                "score": 9.5,
                                "matched_terms": 2,
                                "total_terms": 2,
                            }
                        ],
                },
                "execute": {
                    "execution_schema_version": adapter.execution_schema_version,
                    "authority": adapter.execution_authority,
                    "session_id": "session-1",
                    "user_id": "tester",
                    "skill": "plan-to-code",
                    "overlay": None,
                    "live_run": False,
                    "content": "dry-run content",
                    "usage": {
                        "input_tokens": 5,
                        "output_tokens": 3,
                        "total_tokens": 8,
                        "mode": "estimated",
                    },
                    "prompt_preview": "rust-owned prompt",
                    "model_id": None,
                    "metadata": {
                        "execution_kernel": "rust-execution-kernel-slice",
                        "execution_kernel_authority": "rust-execution-kernel-authority",
                    },
                },
                "describe_transport": {
                    "schema_version": adapter.trace_descriptor_schema_version,
                    "authority": adapter.trace_descriptor_authority,
                    "transport": {
                        "session_id": "session-1",
                        "stream_id": "stream::session-1",
                    },
                },
                "background_state": {
                    "schema_version": adapter.background_state_store_schema_version,
                    "authority": adapter.background_state_store_authority,
                    "status": "ok",
                },
                "trace_stream_inspect": {
                    "schema_version": adapter.trace_stream_inspect_schema_version,
                    "authority": adapter.trace_stream_io_authority,
                    "path": "/tmp/TRACE_EVENTS.jsonl",
                    "source_kind": "trace_stream",
                    "event_count": 0,
                    "latest_event_id": None,
                    "latest_event_kind": None,
                    "latest_event_timestamp": None,
                    "latest_cursor": None,
                    "recovery": None,
                },
            }[op]
            self._owner.stdout.push(
                json.dumps(
                    {
                        "id": request["id"],
                        "ok": True,
                        "payload": payload,
                    }
                )
                + "\n"
            )
            return len(data)

        def flush(self) -> None:
            return None

        def close(self) -> None:
            return None

    class _FakePopen:
        launched_commands: list[list[str]] = []
        instances: list["_FakePopen"] = []

        def __init__(self, command: list[str], **_: object) -> None:
            self.command = list(command)
            self.returncode: int | None = None
            self.requests: list[dict[str, object]] = []
            self.stdout = _FakeStdout()
            self.stdin = _FakeStdin(self)
            self.stderr = io.StringIO("")
            _FakePopen.launched_commands.append(self.command)
            _FakePopen.instances.append(self)

        def poll(self) -> int | None:
            return self.returncode

        def kill(self) -> None:
            self.returncode = -9

        def wait(self, timeout: float | None = None) -> int:
            self.returncode = 0
            return 0

    monkeypatch.setattr(rust_router_module.subprocess, "Popen", _FakePopen)
    monkeypatch.setattr(rust_router_module.select, "select", lambda read, write, exc, timeout: (read, [], []))
    monkeypatch.setattr(
        rust_router_module.subprocess,
        "run",
        lambda *args, **kwargs: (_ for _ in ()).throw(AssertionError("CLI fallback should stay unused")),
    )

    control_plane = adapter.runtime_control_plane()
    search_contract = adapter.search_skill_matches_contract(query="迭代 优化", limit=2)
    execute = adapter.execute(
        {
            "schema_version": "router-rs-execute-request-v1",
            "task": "test task",
            "session_id": "session-1",
            "user_id": "tester",
            "selected_skill": "plan-to-code",
            "overlay_skill": None,
            "layer": "L2",
            "route_engine": "rust",
            "diagnostic_route_mode": "none",
            "reasons": [],
            "prompt_preview": "test preview",
            "dry_run": True,
            "trace_event_count": 0,
            "trace_output_path": None,
            "default_output_tokens": 128,
            "model_id": "gpt-5.4",
            "aggregator_base_url": "http://127.0.0.1:20128/v1",
            "aggregator_api_key": "sk-test",
        }
    )
    transport = adapter.describe_transport({"session_id": "session-1"})
    inspect = adapter.trace_stream_inspect({"path": "/tmp/TRACE_EVENTS.jsonl"})
    background_state = adapter.background_state({"operation": "snapshot"})

    assert control_plane["authority"] == adapter.runtime_control_plane_authority
    assert search_contract.matches[0].record.name == "iterative-optimizer"
    assert execute["execution_schema_version"] == adapter.execution_schema_version
    assert transport["stream_id"] == "stream::session-1"
    assert inspect["authority"] == adapter.trace_stream_io_authority
    assert background_state["authority"] == adapter.background_state_store_authority
    assert len(_FakePopen.launched_commands) == 4
    assert all(command == [str(release_bin), "--stdio-json"] for command in _FakePopen.launched_commands)
    assert sorted(request["op"] for instance in _FakePopen.instances for request in instance.requests) == sorted([
        "runtime_control_plane",
        "search_skills",
        "execute",
        "describe_transport",
        "trace_stream_inspect",
        "background_state",
    ])
    rust_router_module._close_router_stdio_clients()


def test_rust_route_adapter_restarts_stdio_client_without_cli_fallback(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    codex_home = tmp_path
    router_dir = codex_home / "scripts" / "router-rs"
    source_dir = router_dir / "src"
    release_bin = router_dir / "target" / "release" / "router-rs"
    source_dir.mkdir(parents=True)
    release_bin.parent.mkdir(parents=True)
    (router_dir / "Cargo.toml").write_text("[package]\nname='router-rs'\nversion='0.1.0'\n", encoding="utf-8")
    (source_dir / "main.rs").write_text("fn main() {}\n", encoding="utf-8")
    release_bin.write_text("release", encoding="utf-8")

    rust_router_module._close_router_stdio_clients()
    adapter = RustRouteAdapter(codex_home)

    class _BrokenStdout:
        def readline(self) -> str:
            return ""

        def close(self) -> None:
            return None

    class _HealthyStdout:
        def __init__(self) -> None:
            self._lines: list[str] = []

        def push(self, line: str) -> None:
            self._lines.append(line)

        def readline(self) -> str:
            if not self._lines:
                return ""
            return self._lines.pop(0)

        def close(self) -> None:
            return None

    class _BrokenStdin:
        def write(self, data: str) -> int:
            return len(data)

        def flush(self) -> None:
            return None

        def close(self) -> None:
            return None

    class _HealthyStdin:
        def __init__(self, owner: "_ResilientFakePopen") -> None:
            self._owner = owner

        def write(self, data: str) -> int:
            request = json.loads(data)
            self._owner.requests.append(request)
            self._owner.stdout.push(
                json.dumps(
                    {
                        "id": request["id"],
                        "ok": True,
                        "payload": {
                            "schema_version": adapter.runtime_control_plane_schema_version,
                            "authority": adapter.runtime_control_plane_authority,
                        },
                    }
                )
                + "\n"
            )
            return len(data)

        def flush(self) -> None:
            return None

        def close(self) -> None:
            return None

    class _ResilientFakePopen:
        launched_commands: list[list[str]] = []
        launch_count = 0

        def __init__(self, command: list[str], **_: object) -> None:
            type(self).launch_count += 1
            self.command = list(command)
            self.requests: list[dict[str, object]] = []
            self.stderr = io.StringIO("")
            if type(self).launch_count == 1:
                self.returncode = 17
                self.stdout = _BrokenStdout()
                self.stdin = _BrokenStdin()
                self.stderr = io.StringIO("router stdio first process died")
            else:
                self.returncode = None
                self.stdout = _HealthyStdout()
                self.stdin = _HealthyStdin(self)
            type(self).launched_commands.append(self.command)

        def poll(self) -> int | None:
            return self.returncode

        def kill(self) -> None:
            self.returncode = -9

        def wait(self, timeout: float | None = None) -> int:
            self.returncode = 0
            return 0

    monkeypatch.setattr(rust_router_module.subprocess, "Popen", _ResilientFakePopen)
    monkeypatch.setattr(rust_router_module.select, "select", lambda read, write, exc, timeout: (read, [], []))
    monkeypatch.setattr(
        rust_router_module.subprocess,
        "run",
        lambda *args, **kwargs: (_ for _ in ()).throw(AssertionError("CLI fallback should stay unused")),
    )

    control_plane = adapter.runtime_control_plane()

    assert control_plane["authority"] == adapter.runtime_control_plane_authority
    assert _ResilientFakePopen.launched_commands == [
        [str(release_bin), "--stdio-json"],
        [str(release_bin), "--stdio-json"],
    ]
    rust_router_module._close_router_stdio_clients()


def test_framework_profile_requires_portable_core_contract() -> None:
    profile = build_framework_profile(
        profile_id="fusion-default",
        display_name="Fusion Default",
        host_family="generic",
        optional_capabilities=("ui",),
        rules_bundle={"bundle_id": "fusion-rules", "rules": [{"id": "outer-owned"}]},
        skill_bundle={"bundle_id": "fusion-skills", "skills": ["router"]},
        memory_mounts=("project", "user"),
        mcp_servers=("local-memory",),
        loadout_policy={"default": "portable"},
        host_capability_requirements={
            "default": {"required_host_capabilities": ["artifact_contract"]},
            "codex-desktop": {"required_host_capabilities": ["automation_bridge"]},
        },
    )

    assert profile.framework_profile_version == "0.1.0"
    assert profile.core_capabilities == CORE_CAPABILITIES
    assert profile.host_family == "generic"
    assert profile.loadout_policy == {"default": "portable"}
    resolved = resolve_host_capability_requirements(
        profile,
        host_id="codex-desktop",
        adapter_id="codex_desktop_adapter",
    )
    assert resolved["required_host_capabilities"] == [
        "artifact_contract",
        "automation_bridge",
    ]


def test_framework_profile_emits_host_neutral_shared_contract_surface() -> None:
    profile = build_framework_profile(
        profile_id="fusion-shared-contract",
        display_name="Fusion Shared Contract",
        session_policy={
            "mode": "bounded",
            "approval_mode": "manual",
            "history_policy": "append-only",
            "resume": "cursor",
        },
        tool_policy={"shell": "allow"},
        approval_policy={"mode": "manual"},
        loadout_policy={"default": "framework"},
        framework_surface_policy={
            "kernel": {"canonical_axes": ["routing", "memory", "continuity", "host_projection"]},
            "default_surface": {"default_loadouts": ["default_surface_loadout"]},
        },
        artifact_contract={"layout": "stable-v1"},
        memory_mounts=(
            {"mount_id": "project", "source": ".codex/memory"},
            "user",
        ),
        mcp_servers=({"server_id": "local-memory", "transport": "stdio"},),
        workspace_bootstrap={
            "skill_bridge": {"project_dir": ".codex/skills"},
        },
    )

    payload = profile.shared_contract_payload()

    assert payload["schema_version"] == FRAMEWORK_SHARED_CONTRACT_SCHEMA_VERSION
    assert payload["framework_truth"] == "framework_core"
    assert payload["shared_contract_fields"] == list(FRAMEWORK_SHARED_CONTRACT_FIELDS)
    assert payload["shared_contract"]["artifact_contract"] == {"layout": "stable-v1"}
    assert payload["shared_contract"]["memory_mounts"] == [
        {"mount_id": "project", "source": ".codex/memory"},
        {
            "mount_id": "user",
            "source": "user",
            "bridge_kind": "framework-memory-mount",
        },
    ]
    assert payload["shared_contract"]["mcp_servers"] == [
        {"server_id": "local-memory", "transport": "stdio"},
    ]
    assert payload["shared_contract"]["framework_surface_policy"] == {
        "kernel": {"canonical_axes": ["routing", "memory", "continuity", "host_projection"]},
        "default_surface": {"default_loadouts": ["default_surface_loadout"]},
    }
    assert payload["shared_contract"]["session_contract"] == {
        "mode": "bounded",
        "approval_mode": "manual",
        "history_policy": "append-only",
        "takeover": False,
        "extras": {"resume": "cursor"},
    }
    assert payload["shared_contract"]["workspace_bootstrap"]["bridges"]["skills"] == {
        "project_dir": ".codex/skills",
    }
    assert payload["shared_contract"]["workspace_bootstrap"]["bridges"]["memory"] == {
        "bridge_dir": ".aionrs-memory-bridge",
        "mounts": [
            {"mount_id": "project", "source": ".codex/memory"},
            {
                "mount_id": "user",
                "source": "user",
                "bridge_kind": "framework-memory-mount",
            },
        ],
    }


def test_cli_common_shared_contract_keeps_framework_truth_under_host_overrides() -> None:
    profile = build_framework_profile(
        profile_id="shared-contract-host-overrides",
        display_name="Shared Contract Host Overrides",
        session_policy={"mode": "bounded", "approval_mode": "manual"},
        tool_policy={"shell": "allow"},
        approval_policy={"mode": "manual"},
        loadout_policy={"default": "framework"},
        artifact_contract={"layout": "stable-v1"},
        memory_mounts=(
            {"mount_id": "project", "source": ".codex/memory"},
            "user",
        ),
        mcp_servers=({"server_id": "local-memory", "transport": "stdio"},),
        workspace_bootstrap={"skill_bridge": {"project_dir": ".codex/skills"}},
    )

    cli_common = compile_cli_common_adapter(
        profile,
        host_overrides={
            "workspace_bootstrap": {
                "bridges": {
                    "skills": {"project_dir": "host-shadow/.codex/skills"},
                    "memory": {"bridge_dir": "host-shadow-memory"},
                }
            },
            "tool_policy": {"shell": "deny"},
            "metadata": {"host_shadow": True},
        },
    ).host_payload
    canonical_shared_contract = profile.shared_contract_surface()

    assert cli_common["metadata"]["host_shadow"] is True
    assert cli_common["workspace_bootstrap"]["bridges"]["skills"]["project_dir"] == (
        "host-shadow/.codex/skills"
    )
    assert cli_common["shared_contract"]["workspace_bootstrap"] == canonical_shared_contract[
        "workspace_bootstrap"
    ]
    assert cli_common["shared_contract"]["tool_policy"] == canonical_shared_contract["tool_policy"]
    assert cli_common["shared_contract"]["session_contract"] == canonical_shared_contract[
        "session_contract"
    ]
    assert cli_common["bridge_contract"] == profile.shared_contract_bridges()
    assert cli_common["source_contract"]["bridge_contract_source"] == (
        "shared_contract.workspace_bootstrap.bridges"
    )
    assert cli_common["source_contract"]["canonical_adapter_id"] == "cli_common_adapter"

    desktop = compile_codex_desktop_adapter(
        profile,
        host_overrides={
            "workspace_bootstrap": {
                "bridges": {
                    "skills": {"project_dir": "desktop-shadow/.codex/skills"},
                }
            }
        },
    ).host_payload
    assert desktop["workspace_bootstrap"]["bridges"]["skills"]["project_dir"] == (
        "desktop-shadow/.codex/skills"
    )
    assert desktop["common_contract"]["workspace_bootstrap"] == canonical_shared_contract[
        "workspace_bootstrap"
    ]
    assert desktop["runtime_surface"]["workspace_bootstrap"] == canonical_shared_contract[
        "workspace_bootstrap"
    ]
    assert desktop["bridge_contract"] == canonical_shared_contract["workspace_bootstrap"]["bridges"]
    assert desktop["source_contract"]["contract_source_fields"]["shared_contract"] == (
        "common_contract"
    )
    assert desktop["source_contract"]["contract_source_fields"]["bridge_contract"] == (
        "bridge_contract"
    )
    assert desktop["source_contract"]["bridge_contract_source"] == (
        "common_contract.workspace_bootstrap.bridges"
    )


def test_adapted_host_payload_reuses_framework_shared_contract_surface() -> None:
    profile = build_framework_profile(
        profile_id="adapted-host-shared-contract",
        display_name="Adapted Host Shared Contract",
        session_policy={"mode": "bounded", "approval_mode": "manual", "history_policy": "append-only"},
        tool_policy={"shell": "allow"},
        approval_policy={"mode": "manual"},
        loadout_policy={"default": "framework"},
        framework_surface_policy={"default_surface": {"default_loadouts": ["framework"]}},
        artifact_contract={"layout": "stable-v1"},
        memory_mounts=(
            {"mount_id": "project", "source": ".codex/memory"},
            "user",
        ),
        mcp_servers=({"server_id": "local-memory", "transport": "stdio"},),
        workspace_bootstrap={"skill_bridge": {"project_dir": ".codex/skills"}},
    )

    adapted = adapt_framework_profile(profile, CODEX_DESKTOP_ADAPTER).host_payload
    canonical_shared_contract = profile.shared_contract_surface()

    assert adapted["artifact_contract"] == canonical_shared_contract["artifact_contract"]
    assert adapted["tool_policy"] == canonical_shared_contract["tool_policy"]
    assert adapted["approval_policy"] == canonical_shared_contract["approval_policy"]
    assert adapted["loadout_policy"] == canonical_shared_contract["loadout_policy"]
    assert adapted["framework_surface_policy"] == canonical_shared_contract[
        "framework_surface_policy"
    ]
    assert adapted["memory_mounts"] == canonical_shared_contract["memory_mounts"]
    assert adapted["mcp_servers"] == canonical_shared_contract["mcp_servers"]
    assert adapted["workspace_bootstrap"] == canonical_shared_contract["workspace_bootstrap"]


def test_adapt_framework_profile_rejects_implicit_host_private_override_fields() -> None:
    profile = build_framework_profile(
        profile_id="host-private-blocked",
        display_name="Host Private Blocked",
        session_policy={"mode": "bounded", "approval_mode": "manual"},
        tool_policy={"shell": "allow"},
        approval_policy={"mode": "manual"},
        loadout_policy={"default": "framework"},
        artifact_contract={"layout": "stable-v1"},
        memory_mounts=(
            {"mount_id": "project", "source": ".codex/memory"},
            "user",
        ),
        mcp_servers=({"server_id": "local-memory", "transport": "stdio"},),
        workspace_bootstrap={"skill_bridge": {"project_dir": ".codex/skills"}},
    )

    try:
        adapt_framework_profile(
            profile,
            CODEX_CLI_ADAPTER,
            host_overrides={"host_projection": {"context_files": ["AGENTS.md"]}},
        )
    except ValueError as exc:
        assert "host_private" in str(exc)
        assert "explicit opt-in" in str(exc)
    else:
        raise AssertionError("expected host-private override injection to require explicit opt-in")


def test_adapt_framework_profile_rejects_implicit_host_adapter_payload_override_fields() -> None:
    profile = build_framework_profile(
        profile_id="host-adapter-payload-blocked",
        display_name="Host Adapter Payload Blocked",
        session_policy={"mode": "bounded", "approval_mode": "manual"},
        tool_policy={"shell": "allow"},
        approval_policy={"mode": "manual"},
        loadout_policy={"default": "framework"},
        artifact_contract={"layout": "stable-v1"},
        memory_mounts=(
            {"mount_id": "project", "source": ".codex/memory"},
            "user",
        ),
        mcp_servers=({"server_id": "local-memory", "transport": "stdio"},),
        workspace_bootstrap={"skill_bridge": {"project_dir": ".codex/skills"}},
    )

    try:
        adapt_framework_profile(
            profile,
            CODEX_CLI_ADAPTER,
            host_overrides={"host_adapter_payload": {"context_files": ["AGENTS.md"]}},
        )
    except ValueError as exc:
        assert "host_private" in str(exc)
        assert "explicit opt-in" in str(exc)
    else:
        raise AssertionError("expected host-adapter-payload override injection to require explicit opt-in")


def test_adapt_framework_profile_rejects_legacy_host_projection_opt_in() -> None:
    profile = build_framework_profile(
        profile_id="legacy-host-projection-optin-blocked",
        display_name="Legacy Host Projection Optin Blocked",
        session_policy={"mode": "bounded", "approval_mode": "manual"},
        tool_policy={"shell": "allow"},
        approval_policy={"mode": "manual"},
        loadout_policy={"default": "framework"},
        artifact_contract={"layout": "stable-v1"},
        memory_mounts=(
            {"mount_id": "project", "source": ".codex/memory"},
            "user",
        ),
        mcp_servers=({"server_id": "local-memory", "transport": "stdio"},),
        workspace_bootstrap={"skill_bridge": {"project_dir": ".codex/skills"}},
    )
    try:
        adapt_framework_profile(
            profile,
            CODEX_CLI_ADAPTER,
            host_overrides={
                "host_private": {"host_projection": {"context_files": ["AGENTS.md"]}}
            },
        )
    except ValueError as exc:
        assert "host_projection is a legacy read surface" in str(exc)
        assert "host_adapter_payload" in str(exc)
    else:
        raise AssertionError("expected legacy host_projection opt-in to be rejected")


def test_adapt_framework_profile_allows_explicit_host_adapter_payload_opt_in() -> None:
    profile = build_framework_profile(
        profile_id="host-adapter-payload-optin",
        display_name="Host Adapter Payload Optin",
        session_policy={"mode": "bounded", "approval_mode": "manual"},
        tool_policy={"shell": "allow"},
        approval_policy={"mode": "manual"},
        loadout_policy={"default": "framework"},
        artifact_contract={"layout": "stable-v1"},
        memory_mounts=(
            {"mount_id": "project", "source": ".codex/memory"},
            "user",
        ),
        mcp_servers=({"server_id": "local-memory", "transport": "stdio"},),
        workspace_bootstrap={"skill_bridge": {"project_dir": ".codex/skills"}},
    )
    adapted = adapt_framework_profile(
        profile,
        CODEX_CLI_ADAPTER,
        host_overrides={
            "host_private": {"host_adapter_payload": {"context_files": ["AGENTS.md"]}}
        },
    ).host_payload
    assert adapted["host_adapter_payload"]["context_files"] == ["AGENTS.md"]
    assert adapted["host_projection"]["context_files"] == ["AGENTS.md"]


def test_compile_cli_common_adapter_uses_rust_artifact_and_keeps_override_surface(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    profile = build_framework_profile(
        profile_id="rust-wrapper-cli-common",
        display_name="Rust Wrapper CLI Common",
        session_policy={"mode": "bounded"},
    )

    def _fake_compile(profile_arg: FrameworkProfile, artifact_id: str) -> dict[str, Any]:
        assert profile_arg is profile
        assert artifact_id == "cli_common_adapter"
        return {
            "metadata": {
                "adapter_id": "cli_common_adapter",
                "host_id": "cli-family-shared",
                "transport": "host-neutral-contract",
                "rust_marker": True,
            },
            "shared_contract": {"rust_owned": True},
            "controller_boundary": {"shared_adapter": "cli_common_adapter", "source": "rust"},
            "parity_contract": {"source": "rust"},
            "source_contract": {"shared_contract": "shared_contract"},
        }

    monkeypatch.setattr(
        host_adapters_module,
        "_compile_rust_codex_artifact",
        _fake_compile,
    )

    adapted = compile_cli_common_adapter(
        profile,
        host_overrides={"display_name": "Override Display", "metadata": {"user_override": True}},
    )

    assert adapted.host_payload["display_name"] == "Override Display"
    assert adapted.host_payload["shared_contract"] == {"rust_owned": True}
    assert adapted.host_payload["controller_boundary"]["source"] == "rust"
    assert adapted.host_payload["metadata"]["rust_marker"] is True
    assert adapted.host_payload["metadata"]["user_override"] is True
    assert adapted.host_payload["metadata"]["adapter_id"] == "cli_common_adapter"


def test_host_adapter_artifact_wrappers_use_rust_compiler(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    profile = build_framework_profile(
        profile_id="rust-wrapper-artifacts",
        display_name="Rust Wrapper Artifacts",
        session_policy={"mode": "bounded"},
    )
    requested: list[str] = []

    def _fake_compile(profile_arg: FrameworkProfile, artifact_id: str) -> dict[str, Any]:
        assert profile_arg is profile
        requested.append(artifact_id)
        return {"artifact_id": artifact_id, "source": "rust"}

    monkeypatch.setattr(
        codex_artifact_contracts_module,
        "_compile_rust_codex_artifact",
        _fake_compile,
    )

    assert build_cli_family_capability_discovery(profile)["artifact_id"] == (
        "cli_family_capability_discovery"
    )
    assert build_cli_family_parity_snapshot(profile)["artifact_id"] == "cli_family_parity_snapshot"
    assert build_codex_dual_entry_parity_snapshot(profile)["artifact_id"] == (
        "codex_dual_entry_parity_snapshot"
    )
    assert requested == [
        "cli_family_capability_discovery",
        "cli_family_parity_snapshot",
        "codex_dual_entry_parity_snapshot",
    ]


def test_framework_profile_rejects_host_specific_metadata_in_framework_truth() -> None:
    try:
        build_framework_profile(
            profile_id="bad-metadata",
            display_name="Bad Metadata",
            metadata={"transport": "headless-exec"},
        )
    except ValueError as exc:
        assert "host-neutral" in str(exc)
        assert "transport" in str(exc)
    else:
        raise AssertionError("expected ValueError")


def test_framework_profile_rejects_host_projection_metadata_in_framework_truth() -> None:
    for bad_field, bad_value in {
        "context_files": ["AGENTS.md"],
        "hook_event_names": ["PreToolUse"],
    }.items():
        try:
            build_framework_profile(
                profile_id=f"bad-host-metadata-{bad_field}",
                display_name="Bad Host Metadata",
                metadata={bad_field: bad_value},
            )
        except ValueError as exc:
            assert "host-neutral" in str(exc)
            assert bad_field in str(exc)
        else:
            raise AssertionError("expected ValueError")


def test_framework_profile_rejects_aionrs_pinned_host_core() -> None:
    try:
        build_framework_profile(
            profile_id="bad",
            display_name="Bad",
            host_family="aionrs",
        )
    except ValueError as exc:
        assert "must not be pinned directly to aionrs" in str(exc)
    else:
        raise AssertionError("expected ValueError")


def test_merge_profile_overrides_merges_nested_policies() -> None:
    profile = build_framework_profile(
        profile_id="fusion-default",
        display_name="Fusion Default",
        session_policy={"approval_mode": "manual"},
        tool_policy={"network": "deny"},
        host_capability_requirements={
            "default": {"required_host_capabilities": ["artifact_contract"]},
        },
    )

    merged = merge_profile_overrides(
        profile,
        {
            "session_policy": {"turn_timeout_s": 120},
            "tool_policy": {"network": "allowlist"},
            "host_capability_requirements": {
                "codex-desktop": {"required_host_capabilities": ["automation_bridge"]},
            },
            "display_name": "Fusion Default + Host",
        },
    )

    assert merged.display_name == "Fusion Default + Host"
    assert merged.session_policy == {"approval_mode": "manual", "turn_timeout_s": 120}
    assert merged.tool_policy == {"network": "allowlist"}
    assert merged.host_capability_requirements == {
        "default": {"required_host_capabilities": ["artifact_contract"]},
        "codex-desktop": {"required_host_capabilities": ["automation_bridge"]},
    }


def test_aionrs_companion_adapter_compiles_host_neutral_profile() -> None:
    profile = build_framework_profile(
        profile_id="fusion-aionrs",
        display_name="Fusion Aionrs",
        rules_bundle={
            "bundle_id": "fusion-rules",
            "rules": [{"id": "deep-adaptation", "content": "outer-owned"}],
        },
        skill_bundle={
            "bundle_id": "fusion-skills",
            "skills": ["router", "memory-bridge"],
        },
        session_policy={
            "mode": "bounded",
            "approval_mode": "manual",
            "history_policy": "append-only",
        },
        tool_policy={"shell": "allow"},
        approval_policy={"mode": "manual", "surface": "host-approval"},
        loadout_policy={"default": "portable"},
        artifact_contract={"layout": "stable-v1"},
        model_policy={"provider": "openai", "model": "gpt-5", "temperature": 0.1},
        memory_mounts=(
            {
                "mount_id": "project",
                "source": ".codex/memory",
                "bridge_kind": "project-memory",
            },
            "user",
        ),
        mcp_servers=({"server_id": "local-memory", "transport": "stdio"},),
        workspace_bootstrap={
            "skill_bridge": {
                "project_dir": ".codex/skills",
                "bridge_dir": ".aionrs/skills",
            }
        },
        host_capability_requirements={
            "aionrs-companion": {
                "required_host_capabilities": ["session_mode", "tool_approval"],
            }
        },
    )

    adapted = compile_aionrs_companion_adapter(profile)
    contract = adapted.host_payload["companion_contract"]

    assert adapted.adapter == AIONRS_COMPANION_ADAPTER
    assert contract["presetRules"][0]["id"] == "deep-adaptation"
    assert contract["enabledSkills"][1]["skill_id"] == "memory-bridge"
    assert contract["sessionMode"]["mode"] == "bounded"
    assert contract["aionrsConfig"]["config"]["provider"] == "openai"
    assert contract["fallbackSemantics"]["fallback_adapter"] == "codex_desktop_adapter"
    assert contract["fallbackSemantics"]["default_host_peer_set"] == [
        "codex_desktop_adapter",
        "codex_cli_adapter",
        "claude_code_adapter",
        "gemini_cli_adapter",
    ]
    assert adapted.host_payload["metadata"]["legacy_surface"] is True
    assert adapted.host_payload["legacy_boundary"]["adapter_lifecycle"] == "legacy-compatibility"
    assert adapted.host_payload["legacy_boundary"]["exposure_lane"] == "fallback-only-explicit"
    assert adapted.host_payload["legacy_boundary"]["default_host_peer_set_member"] is False
    assert adapted.host_payload["host_capability_requirements"] == {
        "required_host_capabilities": ["session_mode", "tool_approval"],
    }


def test_cli_family_and_desktop_adapters_share_one_outer_contract() -> None:
    profile = build_framework_profile(
        profile_id="fusion-hosts",
        display_name="Fusion Hosts",
        rules_bundle={"rules": ["stay outer owned"]},
        skill_bundle={"skills": ["router"]},
        session_policy={"mode": "team", "approval_mode": "manual"},
        artifact_contract={"layout": "stable-v1"},
        memory_mounts=("project", "user", "reference"),
        mcp_servers=("local-memory",),
        host_capability_requirements={
            "codex-desktop": {
                "required_host_capabilities": ["local_runtime", "automation_bridge"],
            }
        },
    )

    cli_common = compile_cli_common_adapter(profile)
    common = compile_codex_common_adapter(profile)
    aionui = compile_aionui_host_adapter(profile)
    desktop = compile_codex_desktop_adapter(profile)
    cli = compile_codex_cli_adapter(profile)
    claude = compile_claude_code_adapter(profile)
    gemini = compile_gemini_cli_adapter(profile)

    assert cli_common.adapter == CLI_COMMON_ADAPTER
    assert cli_common.host_payload["controller_boundary"]["framework_truth"] == "framework_core"
    assert cli_common.host_payload["controller_boundary"]["shared_adapter"] == "cli_common_adapter"
    assert cli_common.host_payload["controller_boundary"]["cli_family_entrypoints"] == [
        "codex_cli_adapter",
        "claude_code_adapter",
        "gemini_cli_adapter",
    ]

    assert common.adapter == CODEX_COMMON_ADAPTER
    assert common.host_payload["metadata"]["adapter_alias_of"] == "cli_common_adapter"
    assert common.host_payload["metadata"]["canonical_adapter_id"] == "cli_common_adapter"
    assert common.host_payload["controller_boundary"]["shared_adapter"] == "cli_common_adapter"
    assert common.host_payload["parity_contract"]["cli_adapters"] == [
        "codex_cli_adapter",
        "claude_code_adapter",
        "gemini_cli_adapter",
    ]

    assert aionui.adapter == AIONUI_HOST_ADAPTER
    assert aionui.host_payload["host_session_create"]["sessionMode"]["mode"] == "team"
    assert aionui.host_payload["host_runtime_contract"]["preferred_backend"] == "aionrs_companion_adapter"
    assert aionui.host_payload["metadata"]["legacy_surface"] is True
    assert aionui.host_payload["legacy_boundary"]["adapter_lifecycle"] == "legacy-compatibility"
    assert aionui.host_payload["legacy_boundary"]["exposure_lane"] == "fallback-only-explicit"
    assert aionui.host_payload["legacy_boundary"]["default_host_peer_set_member"] is False
    event_transport = aionui.host_payload["host_runtime_contract"]["event_transport"]
    assert event_transport["schema_version"] == TRACE_EVENT_TRANSPORT_SCHEMA_VERSION
    assert event_transport["bridge_kind"] == "runtime_event_bridge"
    assert event_transport["transport_family"] == "host-facing-bridge"
    assert event_transport["transport_kind"] == "poll"
    assert event_transport["endpoint_kind"] == "runtime_method"
    assert event_transport["remote_capable"] is True
    assert event_transport["handoff_supported"] is True
    assert event_transport["handoff_method"] == "describe_runtime_event_handoff"
    assert event_transport["subscribe_method"] == "subscribe_runtime_events"
    assert event_transport["cleanup_method"] == "cleanup_runtime_events"
    assert event_transport["describe_method"] == "describe_runtime_event_transport"
    assert event_transport["handoff_kind"] == "artifact_handoff"
    assert event_transport["binding_refresh_mode"] == "describe_or_checkpoint"
    assert event_transport["binding_artifact_format"] == "json"
    assert event_transport["resume_mode"] == "after_event_id"
    assert event_transport["heartbeat_supported"] is True
    assert event_transport["cleanup_semantics"] == "bridge_cache_only"
    assert event_transport["cleanup_preserves_replay"] is True
    assert event_transport["replay_reseed_supported"] is True
    assert event_transport["chunk_schema_version"] == TRACE_EVENT_BRIDGE_SCHEMA_VERSION
    assert event_transport["cursor_schema_version"] == TRACE_REPLAY_CURSOR_SCHEMA_VERSION
    assert event_transport["replay_supported"] is True
    assert aionui.host_payload["host_runtime_contract"]["event_stream_binding"] == event_transport
    assert aionui.adapter.protocol_hints["deep_adaptation_not_fork"] is True

    assert desktop.adapter == CODEX_DESKTOP_ADAPTER
    assert desktop.host_payload["fallback_semantics"]["requires_aionrs"] is False
    assert desktop.host_payload["runtime_surface"]["artifact_contract"] == {"layout": "stable-v1"}
    assert desktop.host_payload["memory_mounts"][0]["mount_id"] == "project"
    assert desktop.host_payload["controller_boundary"]["single_source_of_truth"] is True
    assert desktop.host_payload["entrypoint_contract"]["entrypoint_kind"] == "interactive"
    assert desktop.host_payload["entrypoint_contract"]["shared_adapter"] == "cli_common_adapter"
    assert desktop.adapter.protocol_hints["works_without_aionrs"] is True

    assert cli.adapter == CODEX_CLI_ADAPTER
    assert cli.host_payload["runtime_surface"]["artifact_contract"] == {"layout": "stable-v1"}
    assert cli.host_payload["runtime_surface"]["execution_controller_contract"]["status_contract"] == (
        "execution_controller_contract_v1"
    )
    assert cli.host_payload["runtime_surface"]["delegation_contract"]["gate"]["gate_skill"] == (
        "subagent-delegation"
    )
    assert cli.host_payload["runtime_surface"]["supervisor_state_contract"]["state_artifact_path"] == (
        ".supervisor_state.json"
    )
    assert cli.host_payload["execution_surface"]["entrypoint_kind"] == "headless"
    assert cli.host_payload["execution_surface"]["controller_is_cli"] is False
    assert cli.host_payload["common_contract"] == desktop.host_payload["common_contract"]
    assert cli.host_payload["host_adapter_payload"] == cli.host_payload["host_projection"]
    assert cli.host_payload["execution_surface"]["shared_adapter"] == "cli_common_adapter"
    assert cli.host_payload["host_adapter_payload"]["context_files"] == ["AGENTS.md"]
    assert cli.host_payload["host_adapter_payload"]["settings_paths"] == [
        "~/.codex/config.toml",
        ".codex/config.toml",
    ]
    assert cli.host_payload["execution_surface"]["supports_cron"] is True
    assert cli.host_payload["fallback_semantics"]["desktop_peer"] == "codex_desktop_adapter"
    assert set(cli.host_payload["fallback_semantics"]["cli_family_peers"]) == {
        "claude_code_adapter",
        "gemini_cli_adapter",
    }

    assert claude.adapter == CLAUDE_CODE_ADAPTER
    assert claude.host_payload["execution_surface"]["shared_adapter"] == "cli_common_adapter"
    assert claude.host_payload["host_adapter_payload"] == claude.host_payload["host_projection"]
    assert claude.host_payload["host_adapter_payload"]["context_files"] == [
        "CLAUDE.md",
        "CLAUDE.local.md",
    ]
    assert claude.host_payload["host_adapter_payload"]["settings_paths"] == [
        "~/.claude/settings.json",
        ".claude/settings.json",
        ".claude/settings.local.json",
    ]
    assert claude.host_payload["host_adapter_payload"]["config_root_env_var"] == "CLAUDE_CONFIG_DIR"
    assert claude.host_payload["execution_surface"]["supports_cron"] is False
    assert claude.host_payload["host_adapter_payload"]["mcp_config_paths"] == ["~/.claude.json"]
    assert claude.host_payload["host_adapter_payload"]["settings_scope_order"] == [
        "managed",
        "command_line",
        "local",
        "project",
        "user",
    ]
    assert claude.host_payload["host_adapter_payload"]["settings_scopes"][0]["scope"] == "managed"
    assert claude.host_payload["host_adapter_payload"]["settings_scopes"][2]["locations"] == [
        ".claude/settings.json",
        "CLAUDE.md",
        ".claude/agents/",
    ]
    assert claude.host_payload["host_adapter_payload"]["subagent_paths"] == [
        "~/.claude/agents/",
        ".claude/agents/",
    ]
    assert ".claude/hooks/" in claude.host_payload["host_adapter_payload"]["claude_directory_features"]
    assert ".claude/rules/" in claude.host_payload["host_adapter_payload"]["claude_directory_features"]
    assert claude.host_payload["host_adapter_payload"]["hook_event_names"] == [
        "PreToolUse",
        "PostToolUse",
        "Notification",
        "Stop",
        "SubagentStart",
        "SubagentStop",
        "PreCompact",
        "PostCompact",
        "SessionStart",
        "SessionEnd",
        "UserPromptSubmit",
        "PostToolUseFailure",
        "StopFailure",
        "PermissionRequest",
        "PermissionDenied",
        "InstructionsLoaded",
        "ConfigChange",
        "CwdChanged",
        "FileChanged",
        "TaskCreated",
        "TaskCompleted",
        "WorktreeCreate",
        "WorktreeRemove",
        "TeammateIdle",
        "Elicitation",
        "ElicitationResult",
    ]
    assert claude.host_payload["host_adapter_payload"]["hook_handler_types"] == [
        "command",
        "prompt",
        "agent",
        "http",
    ]
    assert claude.host_payload["host_adapter_payload"]["hook_control_settings"] == [
        "disableAllHooks",
        "allowManagedHooksOnly",
        "allowedHttpHookUrls",
        "httpHookAllowedEnvVars",
    ]
    assert claude.host_payload["host_adapter_payload"]["hook_inspection_commands"] == ["/hooks"]
    assert claude.host_payload["host_adapter_payload"]["plugin_hook_manifest_paths"] == [
        "hooks/hooks.json"
    ]
    assert [item["source"] for item in claude.host_payload["host_adapter_payload"]["hook_definition_sources"]] == [
        "managed_settings",
        "user_settings",
        "project_settings",
        "local_settings",
        "plugin_manifest",
        "agent_frontmatter",
        "skill_frontmatter",
        "session",
        "built_in",
        "sdk",
    ]
    assert claude.host_payload["host_adapter_payload"]["hook_environment_markers"] == [
        "CLAUDE_ENV_FILE",
        "CLAUDE_PROJECT_DIR",
        "CLAUDE_PLUGIN_ROOT",
        "CLAUDE_PLUGIN_DATA",
        "CLAUDE_CODE_REMOTE",
    ]
    assert claude.host_payload["host_adapter_payload"]["checkpointing_supported"] is True
    assert claude.host_payload["host_adapter_payload"]["managed_settings_paths"][0] == (
        "/Library/Application Support/ClaudeCode/managed-settings.json"
    )
    assert claude.host_payload["host_adapter_payload"]["managed_mcp_paths"][-1] == (
        "C:/Program Files/ClaudeCode/managed-mcp.json"
    )
    assert "hook_registry" in claude.host_payload["capabilities"]["host"]
    assert "hook_browser" in claude.host_payload["capabilities"]["host"]
    assert "checkpoint_restore" in claude.host_payload["capabilities"]["host"]

    assert gemini.adapter == GEMINI_CLI_ADAPTER
    assert gemini.host_payload["execution_surface"]["shared_adapter"] == "cli_common_adapter"
    assert gemini.host_payload["host_adapter_payload"] == gemini.host_payload["host_projection"]
    assert gemini.host_payload["host_adapter_payload"]["context_files"] == ["GEMINI.md"]
    assert gemini.host_payload["host_adapter_payload"]["settings_paths"] == ["~/.gemini/settings.json"]
    assert gemini.host_payload["host_adapter_payload"]["structured_output_modes"] == [
        "json",
        "stream-json",
    ]
    assert gemini.host_payload["execution_surface"]["supports_cron"] is False
    assert gemini.host_payload["host_adapter_payload"]["checkpointing_supported"] is True


def test_host_adapter_payload_resolves_host_capability_requirements_per_adapter() -> None:
    profile = build_framework_profile(
        profile_id="resolved-host-requirements",
        display_name="Resolved Host Requirements",
        host_capability_requirements={
            "default": {"required_host_capabilities": ["artifact_contract"]},
            "codex-desktop": {"required_host_capabilities": ["automation_bridge"]},
            "codex_desktop_adapter": {"required_host_capabilities": ["local_runtime"]},
            "codex-cli": {"required_host_capabilities": ["batch_execution"]},
        },
    )

    cli_common = compile_cli_common_adapter(profile)
    desktop = compile_codex_desktop_adapter(profile)
    cli = compile_codex_cli_adapter(profile)

    assert cli_common.host_payload["host_capability_requirements"] == {
        "required_host_capabilities": ["artifact_contract"],
    }
    assert desktop.host_payload["host_capability_requirements"] == {
        "required_host_capabilities": [
            "artifact_contract",
            "automation_bridge",
            "local_runtime",
        ],
    }
    assert cli.host_payload["host_capability_requirements"] == {
        "required_host_capabilities": [
            "artifact_contract",
            "batch_execution",
        ],
    }


def test_adapter_compatibility_snapshot_validation_and_cli_family_parity_snapshot() -> None:
    profile = build_framework_profile(
        profile_id="portable",
        display_name="Portable",
        host_family="generic",
        host_capability_requirements={
            "generic": {"required_host_capabilities": ["automation_bridge"]},
            "codex-desktop": {"required_host_capabilities": ["automation_bridge"]},
        },
    )

    snapshot = compatibility_snapshot()
    assert snapshot["generic_host_adapter"]["works_without_aionrs"] is True
    assert snapshot["cli_common_adapter"]["host_id"] == "cli-family-shared"
    assert snapshot["codex_common_adapter"]["host_id"] == "codex-shared"
    assert snapshot["codex_desktop_adapter"]["host_id"] == "codex-desktop"
    assert snapshot["claude_code_adapter"]["host_id"] == "claude-code"
    assert snapshot["gemini_cli_adapter"]["host_id"] == "gemini-cli"
    assert "aionrs_companion_adapter" not in snapshot
    assert "aionui_host_adapter" not in snapshot
    assert "codex_desktop_host_adapter" not in snapshot
    assert "compatibility_lane" not in snapshot["codex_desktop_adapter"]
    assert snapshot["codex_cli_adapter"]["host_id"] == "codex-cli"
    compatibility_snapshot_with_alias = compatibility_snapshot(include_legacy_aliases=True)
    assert "codex_desktop_host_adapter" not in compatibility_snapshot_with_alias
    legacy_alias = compatibility_snapshot_with_alias["codex_desktop_adapter"]["compatibility_lane"][
        "legacy_aliases"
    ]["codex_desktop_host_adapter"]
    assert legacy_alias["host_id"] == "codex-desktop"
    assert legacy_alias["transport"] == "local-bridge"
    assert legacy_alias["legacy_surface"] is True
    fallback_lane = compatibility_snapshot_with_alias["fallback_lane"]
    assert fallback_lane["default_host_peer_set"] == [
        "codex_desktop_adapter",
        "codex_cli_adapter",
        "claude_code_adapter",
        "gemini_cli_adapter",
    ]
    assert fallback_lane["explicit_opt_in_required"] is True
    assert fallback_lane["legacy_adapters"]["aionrs_companion_adapter"]["upgrade_zone"] == (
        "upstream-safe-zone"
    )
    assert fallback_lane["legacy_adapters"]["aionrs_companion_adapter"]["legacy_surface"] is True
    assert fallback_lane["legacy_adapters"]["aionui_host_adapter"]["legacy_surface"] is True

    validation = validate_adapter_compatibility(
        profile,
        [
            CLI_COMMON_ADAPTER,
            CODEX_COMMON_ADAPTER,
            GENERIC_HOST_ADAPTER,
            CODEX_DESKTOP_ADAPTER,
            CODEX_DESKTOP_HOST_ADAPTER,
            CODEX_CLI_ADAPTER,
            CLAUDE_CODE_ADAPTER,
            GEMINI_CLI_ADAPTER,
            AIONUI_HOST_ADAPTER,
        ],
    )
    assert validation == {
        "cli_common_adapter": True,
        "codex_common_adapter": True,
        "generic_host_adapter": False,
        "codex_desktop_adapter": True,
        "codex_desktop_host_adapter": True,
        "codex_cli_adapter": True,
        "claude_code_adapter": True,
        "gemini_cli_adapter": True,
        "aionui_host_adapter": True,
    }

    cli_family = build_cli_family_parity_snapshot(profile)
    cli_discovery = build_cli_family_capability_discovery(profile)
    assert cli_family["framework_truth"] == "framework_core"
    assert cli_family["shared_adapter"] == "cli_common_adapter"
    assert cli_family["parity_checks"]["artifact_contract"] is True
    assert cli_discovery["cli_hosts"]["codex_cli_adapter"]["context_files"] == ["AGENTS.md"]
    assert cli_discovery["cli_hosts"]["codex_cli_adapter"]["session_supervisor_driver"] == (
        "codex_driver"
    )
    assert cli_discovery["cli_hosts"]["codex_cli_adapter"]["framework_alias_entrypoints"] == {
        alias_name: alias_payload["host_entrypoints"]["codex-cli"]
        for alias_name, alias_payload in framework_native_aliases().items()
    }
    assert cli_discovery["cli_hosts"]["codex_cli_adapter"]["supervisor_capabilities"] == {
        "external_session_supervisor": True,
        "rate_limit_auto_resume": True,
        "host_resume_entrypoint": True,
        "host_tmux_worker_management": True,
    }
    assert cli_discovery["cli_hosts"]["claude_code_adapter"]["settings_paths"] == [
        "~/.claude/settings.json",
        ".claude/settings.json",
        ".claude/settings.local.json",
    ]
    assert cli_discovery["cli_hosts"]["claude_code_adapter"]["settings_scope_order"] == [
        "managed",
        "command_line",
        "local",
        "project",
        "user",
    ]
    assert cli_discovery["cli_hosts"]["claude_code_adapter"]["config_root_env_var"] == (
        "CLAUDE_CONFIG_DIR"
    )
    assert cli_discovery["cli_hosts"]["claude_code_adapter"]["hook_event_names"] == [
        "PreToolUse",
        "PostToolUse",
        "Notification",
        "Stop",
        "SubagentStart",
        "SubagentStop",
        "PreCompact",
        "PostCompact",
        "SessionStart",
        "SessionEnd",
        "UserPromptSubmit",
        "PostToolUseFailure",
        "StopFailure",
        "PermissionRequest",
        "PermissionDenied",
        "InstructionsLoaded",
        "ConfigChange",
        "CwdChanged",
        "FileChanged",
        "TaskCreated",
        "TaskCompleted",
        "WorktreeCreate",
        "WorktreeRemove",
        "TeammateIdle",
        "Elicitation",
        "ElicitationResult",
    ]
    assert cli_family["cli_hosts"]["claude_code_adapter"]["hook_control_settings"] == [
        "disableAllHooks",
        "allowManagedHooksOnly",
        "allowedHttpHookUrls",
        "httpHookAllowedEnvVars",
    ]
    assert cli_family["cli_hosts"]["claude_code_adapter"]["hook_inspection_commands"] == [
        "/hooks"
    ]
    assert cli_family["cli_hosts"]["claude_code_adapter"]["plugin_hook_manifest_paths"] == [
        "hooks/hooks.json"
    ]
    assert cli_family["cli_hosts"]["claude_code_adapter"]["hook_environment_markers"] == [
        "CLAUDE_ENV_FILE",
        "CLAUDE_PROJECT_DIR",
        "CLAUDE_PLUGIN_ROOT",
        "CLAUDE_PLUGIN_DATA",
        "CLAUDE_CODE_REMOTE",
    ]
    assert cli_family["cli_hosts"]["claude_code_adapter"]["checkpointing_supported"] is True
    assert cli_discovery["framework_truth"] == "framework_core"
    assert cli_discovery["shared_adapter"] == "cli_common_adapter"
    assert cli_discovery["discovery_contract"] == "cli_family_host_capability_contract_v1"
    assert set(cli_discovery["cli_hosts"]) == {
        "codex_cli_adapter",
        "claude_code_adapter",
        "gemini_cli_adapter",
    }
    assert cli_discovery["controller_boundary"]["host_entrypoints"] == [
        "codex_desktop_adapter",
        "codex_cli_adapter",
        "claude_code_adapter",
        "gemini_cli_adapter",
    ]
    assert cli_discovery["controller_boundary"]["cli_family_entrypoints"] == [
        "codex_cli_adapter",
        "claude_code_adapter",
        "gemini_cli_adapter",
    ]
    assert cli_discovery["controller_boundary"]["codexcli_is_controller"] is False
    assert cli_discovery["cli_hosts"]["codex_cli_adapter"]["transport"] == "headless-exec"
    assert cli_discovery["cli_hosts"]["codex_cli_adapter"]["supports_cron"] is True
    assert cli_discovery["cli_hosts"]["claude_code_adapter"]["transport"] == "headless-exec"
    assert cli_discovery["cli_hosts"]["claude_code_adapter"]["settings_paths"] == [
        "~/.claude/settings.json",
        ".claude/settings.json",
        ".claude/settings.local.json",
    ]
    assert cli_discovery["cli_hosts"]["claude_code_adapter"]["hook_event_names"] == [
        "PreToolUse",
        "PostToolUse",
        "Notification",
        "Stop",
        "SubagentStart",
        "SubagentStop",
        "PreCompact",
        "PostCompact",
        "SessionStart",
        "SessionEnd",
        "UserPromptSubmit",
        "PostToolUseFailure",
        "StopFailure",
        "PermissionRequest",
        "PermissionDenied",
        "InstructionsLoaded",
        "ConfigChange",
        "CwdChanged",
        "FileChanged",
        "TaskCreated",
        "TaskCompleted",
        "WorktreeCreate",
        "WorktreeRemove",
        "TeammateIdle",
        "Elicitation",
        "ElicitationResult",
    ]
    assert cli_discovery["cli_hosts"]["claude_code_adapter"]["checkpointing_supported"] is True
    assert cli_discovery["all_cli_hosts_compatible"] is True
    assert cli_family["cli_hosts"]["gemini_cli_adapter"]["mcp_config_paths"] == [
        "~/.gemini/settings.json"
    ]
    assert cli_family["all_shared_contract_checks_pass"] is True

    parity = build_codex_dual_entry_parity_snapshot(profile)
    assert parity["framework_truth"] == "framework_core"
    assert parity["shared_adapter"] == "cli_common_adapter"
    assert parity["shared_adapter_aliases"] == ["codex_common_adapter"]
    assert parity["compatibility_view_of"] == "cli_family_parity_snapshot"
    assert parity["codexcli_is_framework_controller"] is False
    assert parity["shared_contract_fields"] == [
        "artifact_contract",
        "memory_mounts",
        "mcp_servers",
        "tool_policy",
        "approval_policy",
        "loadout_policy",
        "framework_surface_policy",
        "workspace_bootstrap",
        "session_contract",
        "execution_controller_contract",
        "delegation_contract",
        "supervisor_state_contract",
    ]
    assert parity["parity_checks"]["artifact_contract"] is True
    assert parity["parity_checks"]["memory_mounts"] is True
    assert parity["desktop"]["adapter_id"] == "codex_desktop_adapter"
    assert parity["desktop"]["entrypoint_kind"] == "interactive"
    assert parity["desktop"]["shared_adapter"] == "cli_common_adapter"
    assert parity["desktop"]["legacy_aliases"] == ["codex_desktop_host_adapter"]
    assert parity["cli"]["adapter_id"] == "codex_cli_adapter"
    assert parity["cli"]["entrypoint_kind"] == "headless"
    assert parity["cli"]["shared_adapter"] == "cli_common_adapter"
    assert parity["controller_boundary"]["single_source_of_truth"] is True
    assert parity["all_shared_contract_checks_pass"] is True

    matrix = build_upgrade_compatibility_matrix(profile)
    assert matrix["cli_common_adapter"]["compatible"] is True
    assert matrix["codex_desktop_adapter"]["compatible"] is True
    assert matrix["codex_cli_adapter"]["compatible"] is True
    assert matrix["claude_code_adapter"]["compatible"] is True
    assert matrix["gemini_cli_adapter"]["compatible"] is True
    assert "aionrs_companion_adapter" not in matrix
    assert "aionui_host_adapter" not in matrix
    assert "codex_desktop_host_adapter" not in matrix

    compatibility_matrix_with_alias = build_upgrade_compatibility_matrix(
        profile,
        include_legacy_aliases=True,
    )
    assert compatibility_matrix_with_alias["codex_desktop_host_adapter"]["compatible"] is True
    assert compatibility_matrix_with_alias["codex_desktop_host_adapter"]["legacy_surface"] is True
    assert compatibility_matrix_with_alias["aionrs_companion_adapter"]["compatible"] is True
    assert compatibility_matrix_with_alias["aionrs_companion_adapter"]["exposure_lane"] == (
        "fallback-only-explicit"
    )
    assert compatibility_matrix_with_alias["aionrs_companion_adapter"][
        "default_host_peer_set_member"
    ] is False
    assert "aionrs_session_protocol" in compatibility_matrix_with_alias["aionrs_companion_adapter"][
        "fork_danger_zone"
    ]
    assert compatibility_matrix_with_alias["aionui_host_adapter"]["legacy_surface"] is True


def test_host_adapter_lookup_defaults_to_canonical_registry() -> None:
    adapter_ids = {spec.adapter_id for spec in list_host_adapters()}
    assert "cli_common_adapter" in adapter_ids
    assert "claude_code_adapter" in adapter_ids
    assert "gemini_cli_adapter" in adapter_ids
    assert "aionrs_companion_adapter" not in adapter_ids
    assert "aionui_host_adapter" not in adapter_ids
    assert "codex_desktop_host_adapter" not in adapter_ids

    for adapter_id in (
        "aionrs_companion_adapter",
        "aionui_host_adapter",
        "codex_desktop_host_adapter",
    ):
        try:
            get_host_adapter(adapter_id)
        except KeyError as exc:
            assert "include_legacy_aliases=True" in str(exc)
        else:
            raise AssertionError("expected legacy surface lookup to require explicit opt-in")

    legacy_alias = get_host_adapter("codex_desktop_host_adapter", include_legacy_aliases=True)
    assert legacy_alias is CODEX_DESKTOP_HOST_ADAPTER
    legacy_companion = get_host_adapter("aionrs_companion_adapter", include_legacy_aliases=True)
    assert legacy_companion is AIONRS_COMPANION_ADAPTER

    expanded_adapter_ids = {
        spec.adapter_id for spec in list_host_adapters(include_legacy_aliases=True)
    }
    assert "aionrs_companion_adapter" in expanded_adapter_ids
    assert "aionui_host_adapter" in expanded_adapter_ids
    assert "codex_desktop_host_adapter" in expanded_adapter_ids


def test_validate_adapter_compatibility_requires_opt_in_for_legacy_alias_strings() -> None:
    profile = build_framework_profile(
        profile_id="portable-validation",
        display_name="Portable Validation",
        host_family="generic",
        host_capability_requirements={
            "codex-desktop": {"required_host_capabilities": ["automation_bridge"]},
        },
    )

    try:
        validate_adapter_compatibility(
            profile,
            ["aionrs_companion_adapter", "codex_desktop_host_adapter"],
        )
    except KeyError as exc:
        assert "include_legacy_aliases=True" in str(exc)
    else:
        raise AssertionError("expected legacy surface validation to require explicit opt-in")

    validation = validate_adapter_compatibility(
        profile,
        ["aionrs_companion_adapter", "codex_desktop_host_adapter"],
        include_legacy_aliases=True,
    )
    assert validation == {
        "aionrs_companion_adapter": True,
        "codex_desktop_host_adapter": True,
    }


def test_codex_desktop_alias_retirement_status_tracks_parity_first_exit_gate() -> None:
    status = build_codex_desktop_alias_retirement_status(
        alias_inventory_summary={
            "inventory_complete": True,
            "primary_identity_risk_occurrences": 0,
            "legacy_alias_shim_required": False,
        }
    )

    assert status["canonical_adapter_id"] == "codex_desktop_adapter"
    assert status["legacy_alias_id"] == "codex_desktop_host_adapter"
    assert status["alias_lifecycle"] == "retired-alias-only"
    assert status["alias_mode"] == "mirror-only"
    assert status["primary_regression_artifact"] == "cli_family_parity_snapshot"
    assert status["codex_dual_entry_parity_artifact"] == "codex_dual_entry_parity_snapshot"
    assert status["secondary_inventory_artifact"] == "upgrade_compatibility_matrix"
    assert status["emitter_contract"]["python_emits_alias_artifact"] is False
    assert status["emitter_contract"]["rust_emits_alias_artifact"] is False
    assert status["emitter_contract"]["legacy_alias_artifact_opt_in"] is True
    assert status["retirement_gates"]["runtime_primary_identity_consumers_cleared"] is True
    assert status["retirement_gates"]["legacy_alias_inventory_is_secondary"] is True
    assert status["retirement_gates"]["legacy_alias_shim_ready_if_needed"] is True


def test_legacy_codex_desktop_alias_compiler_drops_the_old_compatibility_escape_hatch() -> None:
    root_package = importlib.import_module("framework_runtime")
    cli_family_surface = importlib.import_module("framework_runtime.cli_family_contracts")
    compatibility_module_path = (
        PROJECT_ROOT / "framework_runtime" / "src" / "framework_runtime" / "compatibility.py"
    )

    assert (
        root_package.build_cli_family_capability_discovery.__module__
        == rust_build_cli_family_capability_discovery.__module__
    )
    assert (
        root_package.build_codex_dual_entry_parity_snapshot.__module__
        == rust_build_codex_dual_entry_parity_snapshot.__module__
    )
    assert (
        cli_family_surface.build_cli_family_capability_discovery.__module__
        == rust_build_cli_family_capability_discovery.__module__
    )
    assert (
        cli_family_surface.build_codex_dual_entry_parity_snapshot.__module__
        == rust_build_codex_dual_entry_parity_snapshot.__module__
    )
    assert not hasattr(root_package, "compile_codex_desktop_host_adapter")
    assert not hasattr(root_package, "compile_aionrs_companion_adapter")
    assert not hasattr(root_package, "compile_aionui_host_adapter")
    assert not hasattr(root_package, "build_upgrade_compatibility_matrix")
    assert not hasattr(root_package, "compile_codex_common_adapter")
    assert not hasattr(root_package, "build_codex_desktop_alias_retirement_status")
    assert not compatibility_module_path.exists()


def test_execution_and_supervisor_contract_artifacts_stay_contract_only() -> None:
    execution = build_execution_controller_contract()
    delegation = build_delegation_contract()
    supervisor = build_supervisor_state_contract()

    assert execution["framework_truth"] == "framework_core"
    assert execution["status_contract"] == "execution_controller_contract_v1"
    assert execution["controller"]["primary_owner"] == "execution-controller-coding"
    assert execution["controller"]["state_artifact"] == ".supervisor_state.json"
    assert execution["controller"]["user_facing_aliases"] == ["gsd", "get shit done"]
    assert execution["gsd_execution_posture"]["label"] == "get-shit-done"
    assert execution["gsd_execution_posture"]["verify_before_done"] is True
    assert execution["gsd_execution_posture"]["runtime_dependency"] == "none"
    assert execution["boundaries"]["runtime_branching_changes_required"] is False
    assert execution["required_execution_contract_fields"] == [
        "goal",
        "scope",
        "forbidden_scope",
        "acceptance_criteria",
        "evidence_required",
    ]

    assert delegation["status_contract"] == "delegation_contract_v4"
    assert delegation["gate"]["gate_skill"] == "subagent-delegation"
    assert delegation["gate"]["gate_type"] == "multi_agent_routing"
    assert delegation["gate"]["decision_before_spawn"] is True
    assert delegation["gate"]["route_outcomes"] == ["local", "subagent", "team"]
    assert delegation["gate"]["team_route_skill"] == "team"
    assert delegation["local_supervisor_mode"]["preserves_sidecar_boundaries"] is True
    assert delegation["selection_matrix"]["local_when"][0] == (
        "immediate blocker is faster to solve on the main thread"
    )
    assert delegation["selection_matrix"]["subagent_when"][0] == (
        "bounded sidecars exist with non-overlapping write scopes"
    )
    assert delegation["selection_matrix"]["team_when"][0] == (
        "supervisor-led worker lifecycle management is part of the task"
    )
    assert delegation["delegation_state_fields"] == [
        "routing_decision",
        "orchestration_mode",
        "delegation_plan_created",
        "spawn_attempted",
        "spawn_block_reason",
        "fallback_mode",
        "delegated_sidecars",
        "delegated_lanes",
    ]
    assert delegation["lane_contract_fields"] == [
        "lane_id",
        "lane_owner",
        "bounded_write_scope",
        "expected_output",
        "integration_status",
        "verification_status",
        "recovery_anchor",
    ]
    assert delegation["retry_resume_fields"] == [
        "retry_policy",
        "resume_policy",
        "escalation_path",
        "integration_preconditions",
    ]
    assert delegation["team_contract"] == {
        "supervisor_owned_continuity": True,
        "integration_and_qa_stay_supervisor_led": True,
        "resume_and_recovery_are_first_class": True,
    }

    assert supervisor["status_contract"] == "supervisor_state_contract_v3"
    assert supervisor["state_artifact_path"] == ".supervisor_state.json"
    assert supervisor["schema_expectations"]["top_level_fields"] == [
        "schema_version",
        "task_id",
        "task_summary",
        "controller",
        "primary_owner",
        "active_phase",
        "execution_contract",
        "delegation",
        "workers",
        "progress",
        "verification",
        "open_blockers",
        "next_actions",
    ]
    assert supervisor["schema_expectations"]["team_state_fields"] == [
        "delegation_planned",
        "spawn_pending",
        "spawn_blocked",
        "integration_pending",
        "resume_required",
        "cleanup_pending",
    ]
    assert supervisor["schema_expectations"]["lane_fields"] == [
        "lane_id",
        "lane_owner",
        "goal",
        "bounded_scope",
        "forbidden_scope",
        "expected_output",
        "integration_status",
        "verification_status",
        "recovery_anchor",
    ]
    assert supervisor["cross_artifact_alignment"]["lane_outputs_must_remain_lane_local_until_integrated"] is True
    assert supervisor["compatibility_rules"]["rust_may_validate_or_emit"] is True
    assert supervisor["compatibility_rules"]["no_shadow_replacement_artifact"] is True


    status = build_execution_kernel_live_fallback_retirement_status()

    assert status["framework_truth"] == "framework_core"
    assert status["status_contract"] == "execution_kernel_live_fallback_retirement_status_v1"
    assert status["live_primary"]["contract_mode"] == "rust-live-primary"
    assert status["compatibility_fallback"]["runtime_path_available"] is False
    assert status["compatibility_fallback"]["retired_mode"] == "retired"
    assert status["compatibility_fallback"]["request_behavior"] == "surface-removed"
    assert status["control_surfaces"]["former_env_var"] == "CODEX_AGNO_RUST_EXECUTE_FALLBACK_TO_PYTHON"
    assert status["control_surfaces"]["accepted_after_retirement"] is False
    assert status["control_surfaces"]["request_behavior"] == "surface-removed"
    assert status["control_surfaces"]["surface_role"] == "removed-retired-request-surface"
    assert status["retirement_exit_contract"]["surface_status"] == "removed"
    assert status["retirement_exit_contract"]["current_decision"] == "completed"
    assert status["retirement_exit_contract"]["removal_owner"] == "runtime-integrator"
    assert status["retirement_exit_contract"]["observation_sources"]["local_runtime_health"] == [
        "runtime_control_plane.services.execution.kernel_contract",
        "ExecutionEnvironmentService.health().kernel_live_backend_impl",
    ]
    assert status["public_runtime_contract_fields"] == [
        "execution_kernel",
        "execution_kernel_authority",
        "execution_kernel_contract_mode",
        "execution_kernel_in_process_replacement_complete",
        "execution_kernel_delegate",
        "execution_kernel_delegate_authority",
        "execution_kernel_live_primary",
        "execution_kernel_live_primary_authority",
    ]
    assert status["public_runtime_response_metadata_fields"] == [
        "execution_kernel_delegate_family",
        "execution_kernel_delegate_impl",
    ]
    assert status["current_contract_truth"]["dry_run_delegate_kind"] == "router-rs"
    assert status["current_contract_truth"]["live_fallback_runtime_path_available"] is False
    assert status["current_contract_truth"]["live_fallback_mode"] == "retired"
    assert status["current_contract_truth"]["live_fallback_request_behavior"] == "surface-removed"
    assert status["current_contract_truth"]["live_fallback_request_surface"] == "removed"
    assert status["current_contract_truth"]["live_prompt_preview_passthrough_disabled"] is True
    assert status["current_response_metadata_truth"]["live_delegate_family"] == "rust-cli"
    assert status["current_response_metadata_truth"]["dry_run_delegate_family"] == "rust-cli"
    assert status["current_response_metadata_truth"]["dry_run_delegate_impl"] == "router-rs"
    assert status["remaining_python_owned_surfaces"] == []
    assert status["retirement_readiness"]["ready"] is True
    assert status["retirement_readiness"]["runtime_control_flow_change_required"] is False
    assert status["retirement_gates"]["public_runtime_contract_externalized"] is True
    assert status["retirement_gates"]["response_metadata_surface_externalized"] is True
    assert status["retirement_gates"]["delegate_family_impl_metadata_externalized"] is True
    assert status["retirement_gates"]["dry_run_delegate_still_python_owned"] is False
    assert status["retirement_gates"]["compatibility_fallback_runtime_path_removed"] is True
    assert status["retirement_gates"]["explicit_compatibility_requests_rejected"] is True
    assert status["retirement_gates"]["compatibility_fallback_agent_factory_still_python_owned"] is False
    assert (
        status["guardrails"]["claude_host_runtime_semantics_remain_host_owned"] is True
    )


def test_control_plane_contract_descriptors_share_one_python_source() -> None:
    descriptors = build_control_plane_contract_descriptors()

    assert descriptors["execution_controller_contract"] == build_execution_controller_contract()
    assert descriptors["delegation_contract"] == build_delegation_contract()
    assert descriptors["supervisor_state_contract"] == build_supervisor_state_contract()
    assert descriptors["execution_kernel_live_fallback_retirement_status"] == (
        build_execution_kernel_live_fallback_retirement_status()
    )
    assert descriptors["execution_kernel_live_response_serialization_contract"] == (
        build_execution_kernel_live_response_serialization_contract()
    )


def test_execution_kernel_live_response_serialization_contract_stays_contract_only() -> None:
    status = build_execution_kernel_live_response_serialization_contract()
    core_contract = build_execution_kernel_live_response_serialization_contract_core()

    assert status["framework_truth"] == "framework_core"
    assert status["status_contract"] == "execution_kernel_live_response_serialization_contract_v1"
    assert status["scope"] == "compatibility_live_response_serialization"
    assert status["public_response_fields"] == [
        "session_id",
        "user_id",
        "skill",
        "overlay",
        "live_run",
        "content",
        "usage",
        "prompt_preview",
        "model_id",
        "metadata",
    ]
    assert status["usage_contract"]["live_mode"] == "live"
    assert status["usage_contract"]["dry_run_mode"] == "estimated"
    assert status["runtime_response_metadata_fields"]["shared"] == [
        *RUNTIME_TRACE_METADATA_FIELDS,
    ]
    assert status["current_contract_truth"]["live_primary_schema_version"] == (
        "router-rs-execute-response-v1"
    )
    assert status["current_contract_truth"]["steady_state_response_shapes"] == [
        "live_primary",
        "dry_run",
    ]
    assert status["current_response_shape_truth"]["live_primary"]["prompt_preview_source"] == (
        "rust-owned-live-prompt"
    )
    assert status["current_response_shape_truth"]["dry_run"]["model_id_present"] is False
    assert status["current_response_shape_truth"]["dry_run"]["prompt_preview_source"] == (
        "rust-owned-dry-run-prompt"
    )
    assert status["public_response_fields"] == core_contract["public_response_fields"]
    assert status["runtime_response_metadata_fields"] == core_contract["runtime_response_metadata_fields"]
    assert status["current_response_shape_truth"] == core_contract["current_response_shape_truth"]
    assert status["retirement_gates"]["response_shape_contract_externalized"] is True
    assert (
        status["retirement_gates"]["compatibility_live_response_serialization_still_python_owned"]
        is False
    )
    assert status["guardrails"]["claude_host_runtime_semantics_remain_host_owned"] is True


def test_router_rs_profile_json_matches_outer_framework_contract() -> None:
    profile = build_framework_profile(
        profile_id="rust-profile",
        display_name="Rust Profile",
        rules_bundle={"rules": [{"id": "outer-owned"}]},
        skill_bundle={"skills": ["router", "memory-bridge"]},
        session_policy={"mode": "bounded", "approval_mode": "manual"},
        artifact_contract={"layout": "stable-v1"},
        model_policy={"provider": "openai", "model": "gpt-5"},
        memory_mounts=("project",),
        mcp_servers=("local-memory",),
    )

    with tempfile.TemporaryDirectory() as tmpdir:
        profile_path = Path(tmpdir) / "framework_profile.json"
        profile_path.write_text(json.dumps(profile.to_dict(), ensure_ascii=False), encoding="utf-8")

        proc = subprocess.run(
            [
                *_router_rs_command(),
                "--profile-json",
                "--framework-profile",
                str(profile_path),
            ],
            check=True,
            capture_output=True,
            text=True,
            cwd=PROJECT_ROOT,
        )

    payload = json.loads(proc.stdout)
    canonical_shared_contract = profile.shared_contract_surface()
    assert payload["profile_id"] == "rust-profile"
    assert payload["workspace_bootstrap"] == canonical_shared_contract["workspace_bootstrap"]
    assert payload["memory_mounts"] == canonical_shared_contract["memory_mounts"]
    assert payload["mcp_servers"] == canonical_shared_contract["mcp_servers"]
    assert payload["companion_projection"]["presetRules"][0]["id"] == "outer-owned"
    assert payload["companion_projection"]["enabledSkills"][1]["skill_id"] == "memory-bridge"
    assert payload["companion_projection"]["fallbackSemantics"]["fallback_adapter"] == "codex_desktop_adapter"
    assert payload["cli_common_adapter"]["controller_boundary"]["shared_adapter"] == "cli_common_adapter"
    assert payload["cli_common_adapter"]["shared_contract"]["workspace_bootstrap"] == (
        canonical_shared_contract["workspace_bootstrap"]
    )
    assert payload["cli_common_adapter"]["shared_contract"]["session_contract"] == (
        canonical_shared_contract["session_contract"]
    )
    assert payload["codex_common_adapter"]["metadata"]["adapter_alias_of"] == "cli_common_adapter"
    assert payload["cli_common_adapter"]["source_contract"]["bridge_contract_source"] == (
        "shared_contract.workspace_bootstrap.bridges"
    )
    assert payload["codex_desktop_adapter"]["common_contract"]["workspace_bootstrap"] == (
        canonical_shared_contract["workspace_bootstrap"]
    )
    assert payload["codex_desktop_adapter"]["bridge_contract"] == (
        canonical_shared_contract["workspace_bootstrap"]["bridges"]
    )
    assert payload["codex_desktop_adapter"]["source_contract"]["contract_source_fields"] == {
        "shared_contract": "common_contract",
        "runtime_surface": "runtime_surface",
        "bridge_contract": "bridge_contract",
        "entrypoint_surface": "entrypoint_contract",
    }
    assert payload["codex_cli_adapter"]["runtime_surface"]["workspace_bootstrap"] == (
        canonical_shared_contract["workspace_bootstrap"]
    )
    assert payload["codex_cli_adapter"]["bridge_contract"] == (
        canonical_shared_contract["workspace_bootstrap"]["bridges"]
    )
    assert payload["codex_cli_adapter"]["source_contract"]["contract_source_fields"] == {
        "shared_contract": "common_contract",
        "runtime_surface": "runtime_surface",
        "bridge_contract": "bridge_contract",
        "execution_surface": "execution_surface",
    }
    assert "legacy_codex_common_adapter" not in payload["codex_common_adapter"]["parity_contract"]
    assert payload["claude_code_adapter"]["host_projection"]["context_files"] == [
        "CLAUDE.md",
        "CLAUDE.local.md",
    ]
    assert payload["gemini_cli_adapter"]["host_projection"]["context_files"] == ["GEMINI.md"]
    assert payload["cli_family_parity_snapshot"]["all_shared_contract_checks_pass"] is True
    assert "codex_desktop_host_adapter" not in payload
    assert "codex_desktop_alias_retirement_status" not in payload


def test_router_rs_profile_json_can_opt_in_legacy_alias_output() -> None:
    profile = build_framework_profile(
        profile_id="rust-profile-legacy",
        display_name="Rust Profile Legacy",
        rules_bundle={"rules": [{"id": "outer-owned"}]},
        skill_bundle={"skills": ["router", "memory-bridge"]},
        session_policy={"mode": "bounded", "approval_mode": "manual"},
        artifact_contract={"layout": "stable-v1"},
        model_policy={"provider": "openai", "model": "gpt-5"},
        memory_mounts=("project",),
        mcp_servers=("local-memory",),
    )

    with tempfile.TemporaryDirectory() as tmpdir:
        profile_path = Path(tmpdir) / "framework_profile.json"
        profile_path.write_text(json.dumps(profile.to_dict(), ensure_ascii=False), encoding="utf-8")

        proc = subprocess.run(
            [
                *_router_rs_command(),
                "--profile-json",
                "--include-legacy-alias-artifact",
                "--framework-profile",
                str(profile_path),
            ],
            check=True,
            capture_output=True,
            text=True,
            cwd=PROJECT_ROOT,
        )

    payload = json.loads(proc.stdout)
    assert "codex_desktop_host_adapter" not in payload
    assert payload["compatibility_lane"]["codex_desktop_host_adapter"]["metadata"]["adapter_alias_of"] == (
        "codex_desktop_adapter"
    )
    assert payload["compatibility_lane"]["codex_desktop_host_adapter"]["source_contract"][
        "bridge_contract_source"
    ] == "common_contract.workspace_bootstrap.bridges"
    assert payload["compatibility_lane"]["codex_desktop_host_adapter"]["source_contract"]["alias_mode"] == (
        "mirror-only"
    )
    assert payload["codex_desktop_alias_retirement_status"]["alias_lifecycle"] == "retired-alias-only"


def test_router_rs_profile_json_resolves_host_capability_requirements_per_adapter() -> None:
    profile = build_framework_profile(
        profile_id="rust-profile-host-requirements",
        display_name="Rust Profile Host Requirements",
        host_capability_requirements={
            "default": {"required_host_capabilities": ["artifact_contract"]},
            "codex-desktop": {"required_host_capabilities": ["automation_bridge"]},
            "codex_desktop_adapter": {"required_host_capabilities": ["local_runtime"]},
            "codex-cli": {"required_host_capabilities": ["batch_execution"]},
        },
    )

    with tempfile.TemporaryDirectory() as tmpdir:
        profile_path = Path(tmpdir) / "framework_profile.json"
        profile_path.write_text(json.dumps(profile.to_dict(), ensure_ascii=False), encoding="utf-8")

        proc = subprocess.run(
            [
                *_router_rs_command(),
                "--profile-json",
                "--framework-profile",
                str(profile_path),
            ],
            check=True,
            capture_output=True,
            text=True,
            cwd=PROJECT_ROOT,
        )

    payload = json.loads(proc.stdout)
    assert payload["host_capability_requirements"] == profile.host_capability_requirements
    assert payload["cli_common_adapter"]["host_capability_requirements"] == {
        "required_host_capabilities": ["artifact_contract"],
    }
    assert payload["codex_desktop_adapter"]["host_capability_requirements"] == {
        "required_host_capabilities": [
            "artifact_contract",
            "automation_bridge",
            "local_runtime",
        ],
    }
    assert payload["codex_cli_adapter"]["host_capability_requirements"] == {
        "required_host_capabilities": [
            "artifact_contract",
            "batch_execution",
        ],
    }


def test_router_rs_profile_artifacts_json_exposes_first_class_codex_outputs() -> None:
    profile = build_framework_profile(
        profile_id="rust-profile-artifacts",
        display_name="Rust Profile Artifacts",
        rules_bundle={"rules": [{"id": "outer-owned"}]},
        skill_bundle={"skills": ["router"]},
        session_policy={"mode": "bounded"},
        artifact_contract={"layout": "stable-v1"},
        memory_mounts=("project",),
        mcp_servers=("local-memory",),
    )

    with tempfile.TemporaryDirectory() as tmpdir:
        profile_path = Path(tmpdir) / "framework_profile.json"
        profile_path.write_text(json.dumps(profile.to_dict(), ensure_ascii=False), encoding="utf-8")

        proc = subprocess.run(
            [
                *_router_rs_command(),
                "--profile-artifacts-json",
                "--framework-profile",
                str(profile_path),
            ],
            check=True,
            capture_output=True,
            text=True,
            cwd=PROJECT_ROOT,
        )

    payload = json.loads(proc.stdout)
    assert set(payload) == {
        "cli_common_adapter",
        "codex_common_adapter",
        "codex_desktop_adapter",
        "codex_cli_adapter",
        "claude_code_adapter",
        "gemini_cli_adapter",
        "cli_family_capability_discovery",
        "cli_family_parity_snapshot",
        "codex_dual_entry_parity_snapshot",
        "execution_controller_contract",
        "delegation_contract",
        "supervisor_state_contract",
        "execution_kernel_live_fallback_retirement_status",
        "execution_kernel_live_response_serialization_contract",
    }
    assert payload["cli_common_adapter"]["controller_boundary"]["shared_adapter"] == "cli_common_adapter"
    assert payload["codex_common_adapter"]["controller_boundary"]["framework_truth"] == "framework_core"
    assert payload["codex_common_adapter"]["metadata"]["adapter_alias_of"] == "cli_common_adapter"
    assert payload["codex_common_adapter"]["metadata"]["canonical_adapter_id"] == "cli_common_adapter"
    assert payload["codex_common_adapter"]["parity_contract"]["compatibility_aliases"] == [
        "codex_common_adapter"
    ]
    assert "legacy_codex_common_adapter" not in payload["codex_common_adapter"]["parity_contract"]
    assert "upgrade_compatibility_matrix" not in payload
    assert "codex_desktop_alias_retirement_status" not in payload
    assert payload["codex_desktop_adapter"]["entrypoint_contract"]["entrypoint_kind"] == "interactive"
    assert payload["codex_desktop_adapter"]["bridge_contract"] == (
        payload["codex_desktop_adapter"]["common_contract"]["workspace_bootstrap"]["bridges"]
    )
    assert payload["codex_cli_adapter"]["execution_surface"]["controller_is_cli"] is False
    assert payload["codex_cli_adapter"]["bridge_contract"] == (
        payload["codex_cli_adapter"]["common_contract"]["workspace_bootstrap"]["bridges"]
    )
    assert payload["claude_code_adapter"]["host_projection"]["settings_paths"] == [
        "~/.claude/settings.json",
        ".claude/settings.json",
        ".claude/settings.local.json",
    ]
    assert payload["gemini_cli_adapter"]["host_projection"]["context_files"] == ["GEMINI.md"]
    assert payload["execution_controller_contract"]["status_contract"] == (
        "execution_controller_contract_v1"
    )
    assert payload["delegation_contract"]["gate"]["gate_skill"] == "subagent-delegation"
    assert payload["supervisor_state_contract"]["state_artifact_path"] == ".supervisor_state.json"
    assert payload["cli_family_capability_discovery"]["all_cli_hosts_compatible"] is True
    assert payload["cli_family_capability_discovery"]["cli_hosts"]["codex_cli_adapter"][
        "supports_cron"
    ] is True
    assert payload["cli_family_capability_discovery"]["cli_hosts"]["codex_cli_adapter"][
        "session_supervisor_driver"
    ] == "codex_driver"
    assert payload["cli_family_capability_discovery"]["cli_hosts"]["claude_code_adapter"][
        "transport"
    ] == "headless-exec"
    assert payload["cli_family_capability_discovery"]["cli_hosts"]["claude_code_adapter"][
        "framework_alias_entrypoints"
    ] == {
        alias_name: alias_payload["host_entrypoints"]["claude-code"]
        for alias_name, alias_payload in framework_native_aliases().items()
    }
    assert payload["cli_family_parity_snapshot"]["all_shared_contract_checks_pass"] is True
    assert payload["codex_dual_entry_parity_snapshot"]["all_shared_contract_checks_pass"] is True
    assert payload["execution_kernel_live_fallback_retirement_status"]["live_primary"][
        "contract_mode"
    ] == "rust-live-primary"
    assert payload["execution_kernel_live_fallback_retirement_status"]["current_contract_truth"][
        "live_fallback_mode"
    ] == "retired"
    assert payload["execution_kernel_live_fallback_retirement_status"]["current_contract_truth"][
        "live_fallback_request_behavior"
    ] == "surface-removed"
    assert payload["execution_kernel_live_fallback_retirement_status"][
        "public_runtime_response_metadata_fields"
    ] == [
        "execution_kernel_delegate_family",
        "execution_kernel_delegate_impl",
    ]
    assert payload["execution_kernel_live_fallback_retirement_status"][
        "remaining_python_owned_surfaces"
    ] == []
    assert payload["execution_kernel_live_fallback_retirement_status"]["retirement_readiness"][
        "ready"
    ] is True
    assert payload["execution_kernel_live_fallback_retirement_status"]["retirement_gates"][
        "compatibility_fallback_runtime_path_removed"
    ] is True
    assert payload["execution_kernel_live_response_serialization_contract"]["status_contract"] == (
        "execution_kernel_live_response_serialization_contract_v1"
    )
    assert payload["execution_kernel_live_response_serialization_contract"][
        "runtime_response_metadata_fields"
    ]["shared"] == [
        "trace_event_count",
        "trace_output_path",
    ]
    assert payload["execution_kernel_live_response_serialization_contract"]["retirement_gates"][
        "compatibility_live_response_serialization_still_python_owned"
    ] is False


def test_router_rs_profile_artifacts_json_can_opt_in_continuity_alias_artifact() -> None:
    profile = build_framework_profile(
        profile_id="rust-profile-artifacts-legacy",
        display_name="Rust Profile Artifacts Legacy",
        rules_bundle={"rules": [{"id": "outer-owned"}]},
        skill_bundle={"skills": ["router"]},
        session_policy={"mode": "bounded"},
        artifact_contract={"layout": "stable-v1"},
        memory_mounts=("project",),
        mcp_servers=("local-memory",),
    )

    with tempfile.TemporaryDirectory() as tmpdir:
        profile_path = Path(tmpdir) / "framework_profile.json"
        profile_path.write_text(json.dumps(profile.to_dict(), ensure_ascii=False), encoding="utf-8")

        proc = subprocess.run(
            [
                *_router_rs_command(),
                "--profile-artifacts-json",
                "--include-compatibility-inventory",
                "--include-legacy-alias-artifact",
                "--framework-profile",
                str(profile_path),
            ],
            check=True,
            capture_output=True,
            text=True,
            cwd=PROJECT_ROOT,
        )

    payload = json.loads(proc.stdout)
    assert payload["cli_common_adapter"]["controller_boundary"]["shared_adapter"] == "cli_common_adapter"
    assert payload["codex_common_adapter"]["controller_boundary"]["framework_truth"] == "framework_core"
    assert payload["codex_desktop_adapter"]["entrypoint_contract"]["entrypoint_kind"] == "interactive"
    assert payload["codex_cli_adapter"]["execution_surface"]["entrypoint_kind"] == "headless"
    assert payload["claude_code_adapter"]["host_projection"]["context_files"][0] == "CLAUDE.md"
    assert payload["gemini_cli_adapter"]["host_projection"]["structured_output_modes"] == [
        "json",
        "stream-json",
    ]
    assert "codex_desktop_host_adapter" not in payload
    assert payload["codex_desktop_alias_retirement_status"]["alias_lifecycle"] == "retired-alias-only"
    assert payload["codex_desktop_alias_retirement_status"]["emitter_contract"]["rust_emits_alias_artifact"] is False
    assert payload["upgrade_compatibility_matrix"]["codex_desktop_host_adapter"]["compatible"] is True
    assert payload["upgrade_compatibility_matrix"]["aionrs_companion_adapter"]["legacy_surface"] is True


def test_router_rs_profile_artifacts_json_can_include_compatibility_inventory() -> None:
    profile = build_framework_profile(
        profile_id="rust-profile-artifacts-compat",
        display_name="Rust Profile Artifacts Compat",
        rules_bundle={"rules": [{"id": "outer-owned"}]},
        skill_bundle={"skills": ["router"]},
        session_policy={"mode": "bounded"},
        artifact_contract={"layout": "stable-v1"},
        memory_mounts=("project",),
        mcp_servers=("local-memory",),
    )

    with tempfile.TemporaryDirectory() as tmpdir:
        profile_path = Path(tmpdir) / "framework_profile.json"
        profile_path.write_text(json.dumps(profile.to_dict(), ensure_ascii=False), encoding="utf-8")

        proc = subprocess.run(
            [
                *_router_rs_command(),
                "--profile-artifacts-json",
                "--include-compatibility-inventory",
                "--framework-profile",
                str(profile_path),
            ],
            check=True,
            capture_output=True,
            text=True,
            cwd=PROJECT_ROOT,
        )

    payload = json.loads(proc.stdout)
    assert "codex_desktop_alias_retirement_status" not in payload
    assert payload["upgrade_compatibility_matrix"]["cli_common_adapter"]["compatible"] is True
    assert payload["upgrade_compatibility_matrix"]["codex_common_adapter"]["compatible"] is True
    assert payload["upgrade_compatibility_matrix"]["codex_common_adapter"]["legacy_surface"] is False
    assert (
        payload["upgrade_compatibility_matrix"]["codex_common_adapter"][
            "default_host_peer_set_member"
        ]
        is False
    )
    assert payload["upgrade_compatibility_matrix"]["codex_desktop_adapter"]["compatible"] is True
    assert "codex_desktop_host_adapter" not in payload["upgrade_compatibility_matrix"]
    assert "aionrs_companion_adapter" not in payload["upgrade_compatibility_matrix"]


def test_ensure_capabilities_rejects_missing_capability() -> None:
    profile = FrameworkProfile(
        profile_id="thin",
        display_name="Thin",
        core_capabilities=("runtime", "artifact"),
    )

    try:
        ensure_capabilities(profile, ("memory",))
    except ValueError as exc:
        assert "missing required capabilities" in str(exc)
    else:
        raise AssertionError("expected ValueError")
