"""Regression tests for the local Codex Agno runtime skeleton."""

from __future__ import annotations

import asyncio
import json
import sys
from contextlib import contextmanager
from pathlib import Path

import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = PROJECT_ROOT / "framework_runtime" / "src"
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

import framework_runtime.runtime as runtime_module

from framework_runtime.config import RuntimeSettings
from framework_runtime.runtime import CodexAgnoRuntime
from framework_runtime.schemas import (
    BackgroundParallelGroupSummary,
    BackgroundRunRequest,
    BackgroundRunStatus,
    PrepareSessionRequest,
    RunTaskRequest,
    RunTaskResponse,
    UsageMetrics,
)
from framework_runtime.trace import (
    TRACE_EVENT_SCHEMA_VERSION,
    TRACE_EVENT_SINK_SCHEMA_VERSION,
    TRACE_METADATA_SCHEMA_VERSION,
    TRACE_EVENT_TRANSPORT_SCHEMA_VERSION,
    TRACE_REPLAY_CURSOR_SCHEMA_VERSION,
    TRACE_RESUME_MANIFEST_SCHEMA_VERSION,
)
from framework_runtime.skill_loader import SkillLoader
from framework_runtime.state import (
    BACKGROUND_STATE_SCHEMA_VERSION,
    BACKGROUND_SESSION_TAKEOVER_ARBITRATION_SCHEMA_VERSION,
    BackgroundJobStore,
    BackgroundJobStatusMutation,
    SessionConflictError,
)


_MINIMAL_SUPERVISOR_STATE = {
    "schema_version": "supervisor-state-v2",
    "controller": "execution-controller-coding",
    "active_phase": "completed",
    "delegation": {
        "delegation_plan_created": True,
        "spawn_attempted": False,
        "fallback_mode": "local-supervisor",
        "delegated_sidecars": [],
    },
    "verification": {
        "verification_status": "completed",
    },
}


@contextmanager
def _project_supervisor_state(state: dict | None = None) -> Path:
    path = PROJECT_ROOT / ".supervisor_state.json"
    original = path.read_text(encoding="utf-8") if path.exists() else None
    path.write_text(json.dumps(state or _MINIMAL_SUPERVISOR_STATE, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    try:
        yield path
    finally:
        if original is None:
            path.unlink(missing_ok=True)
        else:
            path.write_text(original, encoding="utf-8")


def test_skill_loader_supports_lazy_body_hydration() -> None:
    """Verify skills can be loaded without bodies and hydrated later."""

    loader = SkillLoader(PROJECT_ROOT / "skills")
    skills = loader.load(refresh=True, load_bodies=False)
    assert skills

    target = next(skill for skill in skills if skill.name == "subagent-delegation")
    assert target.body_loaded is False
    assert target.when_to_use == ""
    loader.load_body(target)
    assert target.body_loaded is True
    assert target.when_to_use
    assert "Runtime-policy adaptation" in target.body


def test_skill_loader_ignores_legacy_trigger_phrases_frontmatter(tmp_path: Path) -> None:
    """Verify the loader only accepts canonical trigger_hints."""

    skills_root = tmp_path / "skills"
    skill_dir = skills_root / "legacy-skill"
    skill_dir.mkdir(parents=True)
    (skill_dir / "SKILL.md").write_text(
        """---
name: legacy-skill
description: Legacy trigger field only
trigger_phrases:
  - 旧字段
---
## When to use
- test
""",
        encoding="utf-8",
    )

    loader = SkillLoader(skills_root)
    [skill] = loader.load(refresh=True, load_bodies=False)

    assert skill.name == "legacy-skill"
    assert skill.trigger_hints == []




def test_runtime_uses_rust_control_plane_concurrency_defaults(monkeypatch: pytest.MonkeyPatch) -> None:
    settings = RuntimeSettings(codex_home=PROJECT_ROOT, live_model_override=False)
    control_plane = {
        "runtime_host": {
            "concurrency_contract": {
                "max_background_jobs": 16,
                "background_job_timeout_seconds": 600,
            }
        },
        "services": {
            "middleware": {
                "subagent_limit_contract": {
                    "max_concurrent_subagents": 8,
                    "timeout_seconds": 900,
                }
            }
        },
    }

    monkeypatch.delenv("CODEX_MAX_BACKGROUND_JOBS", raising=False)
    monkeypatch.delenv("CODEX_BACKGROUND_JOB_TIMEOUT", raising=False)
    monkeypatch.delenv("CODEX_AGNO_MAX_CONCURRENT_SUBAGENTS", raising=False)
    monkeypatch.delenv("CODEX_AGNO_SUBAGENT_TIMEOUT_SECONDS", raising=False)

    assert settings.max_concurrent_subagents == 3
    assert runtime_module._runtime_background_defaults(control_plane) == (16, 600.0)
    assert runtime_module._runtime_subagent_defaults(settings, control_plane) == (8, 900)


def test_runtime_uses_rust_control_plane_defaults_over_settings(tmp_path: Path) -> None:
    settings = RuntimeSettings(
        codex_home=PROJECT_ROOT,
        data_dir=tmp_path / "runtime-data",
        trace_output_path=tmp_path / "TRACE_METADATA.json",
        live_model_override=False,
        max_concurrent_subagents=99,
        subagent_timeout_seconds=111,
    )

    runtime = CodexAgnoRuntime(settings)

    assert runtime._max_background_jobs == 16
    assert runtime._background_job_timeout == 600.0
    assert runtime.execution_service.max_background_jobs == 16
    assert runtime.execution_service.background_job_timeout_seconds == 600.0
    assert runtime.background_service._max_background_jobs == 16
    assert runtime._max_concurrent_subagents == 8
    assert runtime._subagent_timeout_seconds == 900


def test_runtime_shares_one_rust_adapter_across_route_and_execute(tmp_path: Path) -> None:
    settings = RuntimeSettings(
        codex_home=PROJECT_ROOT,
        data_dir=tmp_path / "runtime-data",
        trace_output_path=tmp_path / "TRACE_METADATA.json",
        live_model_override=False,
    )

    runtime = CodexAgnoRuntime(settings)

    assert runtime.router_service._rust_adapter is runtime.rust_adapter
    assert runtime.execution_service._rust_adapter is runtime.rust_adapter
    assert runtime.checkpointer._rust_adapter is runtime.rust_adapter
    assert runtime.trace_service.recorder._rust_adapter is runtime.rust_adapter


def test_runtime_dry_run_works_without_agno_and_writes_trace(tmp_path: Path) -> None:
    """Verify the runtime remains usable when the Python-backed kernel delegate is unavailable."""

    with _project_supervisor_state() as supervisor_state_path:
        trace_path = tmp_path / "TRACE_METADATA.json"
        settings = RuntimeSettings(
            codex_home=PROJECT_ROOT,
            data_dir=tmp_path / "runtime-data",
            trace_output_path=trace_path,
            live_model_override=False,
        )
        runtime = CodexAgnoRuntime(settings)
        health = runtime.health()
        assert health["runtime_host"]["role"] == "runtime-orchestration"
        assert health["runtime_host"]["startup_order"] == [
            "router",
            "state",
            "trace",
            "memory",
            "execution",
            "background",
        ]
        assert health["runtime_host"]["shutdown_order"] == [
            "background",
            "execution",
            "memory",
            "trace",
            "state",
            "router",
        ]
        assert health["rustification"]["python_host_role"] == "thin-projection"
        assert health["rustification"]["rustification_status"]["runtime_primary_owner"] == "rust-control-plane"
        assert health["rustification"]["rust_owned_service_count"] >= 8
        assert health["execution_environment"]["sandbox"]["contract"]["authority"] == "rust-runtime-control-plane"
        assert (
            health["execution_environment"]["sandbox"]["contract"]["cleanup_mode"]
            == "async-drain-and-recycle"
        )
        assert health["trace"]["observability"]["ownership_lane"] == "rust-contract-lane"
        assert health["trace"]["observability"]["dashboard_schema_version"] == (
            "runtime-observability-dashboard-v1"
        )
        assert health["trace"]["observability"]["exporter"]["producer_authority"] == (
            "rust-runtime-control-plane"
        )

        async def _run() -> None:
            response = await runtime.run_task(
                RunTaskRequest(
                    task="帮我写一个 Rust CLI 工具",
                    user_id="tester",
                    dry_run=True,
                )
            )
            assert response.live_run is False
            assert response.skill
            assert response.prompt_preview
            assert response.metadata["trace_event_count"] >= 6
            assert response.metadata["trace_event_schema_version"] == TRACE_EVENT_SCHEMA_VERSION
            assert response.metadata["trace_metadata_schema_version"] == TRACE_METADATA_SCHEMA_VERSION
            assert response.metadata["trace_event_sink_schema_version"] == TRACE_EVENT_SINK_SCHEMA_VERSION
            assert response.metadata["trace_replay_cursor_schema_version"] == TRACE_REPLAY_CURSOR_SCHEMA_VERSION
            assert response.metadata["trace_replay_supported"] is True
            assert response.metadata["trace_event_bridge_supported"] is True
            assert response.metadata["trace_event_bridge_schema_version"] == "runtime-event-bridge-v1"
            assert response.metadata["trace_event_transport_schema_version"] == TRACE_EVENT_TRANSPORT_SCHEMA_VERSION
            assert response.metadata["trace_event_transport_family"] == "host-facing-bridge"
            assert response.metadata["trace_event_transport_endpoint_kind"] == "runtime_method"
            assert response.metadata["trace_event_transport_remote_capable"] is True
            assert response.metadata["trace_event_transport_handoff_supported"] is True
            assert response.metadata["trace_event_transport_attach_mode"] == "process_external_artifact_replay"
            assert response.metadata["trace_event_transport_binding_role"] == "primary_attach_descriptor"
            assert response.metadata["trace_event_transport_recommended_method"] == "describe_runtime_event_handoff"
            assert response.metadata["trace_resume_manifest_role"] == "checkpoint_recovery_anchor"
            assert response.metadata["trace_event_handoff_schema_version"] == "runtime-event-handoff-v1"
            assert response.metadata["trace_resume_manifest_binding_path"].endswith(
                f"runtime_event_transports/{response.session_id}__{response.session_id}.json"
            )
            assert response.metadata["trace_replay_anchor_kind"] == "trace_replay_cursor"
            assert response.metadata["trace_replay_resume_mode"] == "after_event_id"
            assert response.metadata["execution_kernel"] == "rust-execution-kernel-slice"
            assert response.metadata["execution_kernel_authority"] == "rust-execution-kernel-authority"
            assert response.metadata["execution_kernel_contract_mode"] == "rust-live-primary"
            assert response.metadata["execution_kernel_in_process_replacement_complete"] is True
            assert response.metadata["execution_kernel_delegate"] == "router-rs"
            assert response.metadata["execution_kernel_delegate_authority"] == "rust-execution-cli"
            assert response.metadata["execution_kernel_delegate_family"] == "rust-cli"
            assert response.metadata["execution_kernel_delegate_impl"] == "router-rs"
            assert response.metadata["execution_kernel_live_primary"] == "router-rs"
            assert response.metadata["execution_kernel_live_primary_authority"] == "rust-execution-cli"
            assert response.metadata["trace_generation"] == 0
            assert response.metadata["trace_latest_seq"] >= 6
            assert response.metadata["trace_resume_cursor"]["seq"] >= 6
            assert response.metadata["reroute_count"] == 0
            assert response.metadata["retry_count"] == 0

        asyncio.run(_run())

        data = json.loads(trace_path.read_text(encoding="utf-8"))
        assert data["task"] == "帮我写一个 Rust CLI 工具"
        assert data["decision"]["owner"]
        assert data["metadata_schema_version"] == TRACE_METADATA_SCHEMA_VERSION
        assert data["trace_event_schema_version"] == TRACE_EVENT_SCHEMA_VERSION
        assert data["trace_event_sink_schema_version"] == TRACE_EVENT_SINK_SCHEMA_VERSION
        assert data["reroute_count"] == 0
        assert data["retry_count"] == 0
        assert str(trace_path) in data["artifact_paths"]
        assert data["supervisor_projection"]["supervisor_state_path"] == str(supervisor_state_path.resolve())
        supervisor_state = json.loads(supervisor_state_path.read_text(encoding="utf-8"))
        assert data["supervisor_projection"]["active_phase"] == supervisor_state["active_phase"]
        assert data["supervisor_projection"]["verification_status"] == supervisor_state["verification"][
            "verification_status"
        ]
        assert data["supervisor_projection"]["delegation"] == {
            "plan_created": supervisor_state["delegation"]["delegation_plan_created"],
            "spawn_attempted": supervisor_state["delegation"]["spawn_attempted"],
            "fallback_mode": supervisor_state["delegation"]["fallback_mode"],
            "sidecar_count": len(supervisor_state["delegation"]["delegated_sidecars"]),
        }
        assert data["stream"]["replay_supported"] is True
        assert data["stream"]["event_bridge_supported"] is True
        assert data["stream"]["event_bridge_schema_version"] == "runtime-event-bridge-v1"
        assert data["stream"]["latest_cursor"]["schema_version"] == TRACE_REPLAY_CURSOR_SCHEMA_VERSION
        assert any(event["kind"] == "route.selected" for event in data["events"])
        assert any(event["kind"] == "middleware.enter" for event in data["events"])
        assert any(event["kind"] == "middleware.exit" for event in data["events"])
        assert any(event["kind"] == "run.completed" for event in data["events"])

        stream_path = trace_path.with_name("TRACE_EVENTS.jsonl")
        lines = [json.loads(line) for line in stream_path.read_text(encoding="utf-8").splitlines() if line.strip()]
        assert lines
        assert lines[0]["sink_schema_version"] == TRACE_EVENT_SINK_SCHEMA_VERSION
        assert lines[0]["event"]["schema_version"] == TRACE_EVENT_SCHEMA_VERSION
        assert lines[0]["event"]["seq"] == 1
        assert lines[-1]["event"]["cursor"].startswith("g0:s")

        resume_manifest = json.loads(trace_path.with_name("TRACE_RESUME_MANIFEST.json").read_text(encoding="utf-8"))
        assert resume_manifest["schema_version"] == TRACE_RESUME_MANIFEST_SCHEMA_VERSION
        assert resume_manifest["session_id"]
        assert resume_manifest["status"] == "dry_run"
        assert resume_manifest["trace_output_path"] == str(trace_path)
        assert resume_manifest["latest_cursor"]["schema_version"] == TRACE_REPLAY_CURSOR_SCHEMA_VERSION
        assert resume_manifest["supervisor_projection"]["supervisor_state_path"] == str(
            supervisor_state_path.resolve()
        )
        assert resume_manifest["supervisor_projection"]["active_phase"] == supervisor_state["active_phase"]
        assert (
            resume_manifest["supervisor_projection"]["verification_status"]
            == supervisor_state["verification"]["verification_status"]
        )
        assert resume_manifest["supervisor_projection"]["delegation"]["sidecar_count"] == len(
            supervisor_state["delegation"]["delegated_sidecars"]
        )


def test_runtime_dry_run_emits_empty_supervisor_projection_without_state_file(tmp_path: Path) -> None:
    """Trace/resume artifacts should degrade cleanly when no supervisor state file exists."""

    isolated_home = tmp_path / "isolated-home"
    isolated_home.mkdir()
    (isolated_home / "skills").symlink_to(PROJECT_ROOT / "skills", target_is_directory=True)
    (isolated_home / "scripts").symlink_to(PROJECT_ROOT / "scripts", target_is_directory=True)
    trace_path = tmp_path / "TRACE_METADATA.json"
    runtime = CodexAgnoRuntime(
        RuntimeSettings(
            codex_home=isolated_home,
            data_dir=tmp_path / "runtime-data",
            trace_output_path=trace_path,
            live_model_override=False,
        )
    )

    async def _run() -> None:
        response = await runtime.run_task(
            RunTaskRequest(
                task="帮我写一个 Rust CLI 工具",
                user_id="tester",
                dry_run=True,
            )
        )
        assert response.live_run is False

    asyncio.run(_run())

    data = json.loads(trace_path.read_text(encoding="utf-8"))
    assert data["supervisor_projection"] == {
        "supervisor_state_path": None,
        "active_phase": None,
        "verification_status": None,
        "delegation": None,
    }
    assert str((isolated_home / ".supervisor_state.json").resolve()) not in data["artifact_paths"]

    resume_manifest = json.loads(trace_path.with_name("TRACE_RESUME_MANIFEST.json").read_text(encoding="utf-8"))
    assert resume_manifest["supervisor_projection"] == {
        "supervisor_state_path": None,
        "active_phase": None,
        "verification_status": None,
        "delegation": None,
    }
    assert str((isolated_home / ".supervisor_state.json").resolve()) not in resume_manifest["artifact_paths"]


def test_runtime_supervisor_projection_summarizes_structured_verification_status_dict(tmp_path: Path) -> None:
    """Structured verification dictionaries should reduce to a stable overall verdict."""

    supervisor_state = {
        "schema_version": "supervisor-state-v2",
        "active_phase": "background-policy-rustified-still-not-full-done",
        "delegation": {
            "delegated_sidecars": [{"nickname": "Euler"}, {"nickname": "Gibbs"}],
        },
        "verification": {
            "cargo_test_router_rs": "passed",
            "pytest_targeted_suite": "background/runtime/services targeted passed",
            "compileall_runtime": "passed",
        },
    }
    with _project_supervisor_state(supervisor_state) as supervisor_state_path:
        settings = RuntimeSettings(
            codex_home=PROJECT_ROOT,
            data_dir=tmp_path / "runtime-data",
            trace_output_path=tmp_path / "TRACE_METADATA.json",
            live_model_override=False,
        )
        runtime = CodexAgnoRuntime(settings)
        projection = runtime._build_supervisor_projection()

        assert projection.supervisor_state_path == str(supervisor_state_path.resolve())
        assert projection.active_phase == supervisor_state["active_phase"]
        assert projection.verification_status == "passed"
        assert projection.delegation is not None
        assert projection.delegation.model_dump(mode="json") == {
            "plan_created": None,
            "spawn_attempted": None,
            "fallback_mode": None,
            "sidecar_count": 2,
        }


def test_runtime_health_respects_rust_like_runtime_host_descriptor(tmp_path: Path) -> None:
    runtime = CodexAgnoRuntime(
        RuntimeSettings(
            codex_home=PROJECT_ROOT,
            data_dir=tmp_path / "runtime-data",
            trace_output_path=tmp_path / "TRACE_METADATA.json",
            live_model_override=False,
        )
    )
    runtime._apply_control_plane_descriptor(
        {
            **runtime.control_plane_descriptor,
            "runtime_host": {
                "authority": "rust-runtime-control-plane",
                "role": "runtime-orchestration",
                "projection": "python-diagnosis-only-projection",
                "delegate_kind": "rust-runtime-control-plane",
                "startup_order": ["router", "trace", "state", "memory", "execution", "background"],
                "shutdown_order": ["background", "execution", "memory", "state", "trace", "router"],
                "health_sections": ["router", "trace", "execution_environment"],
            },
        }
    )

    health = runtime.health()

    assert health["runtime_host"]["startup_order"] == [
        "router",
        "trace",
        "state",
        "memory",
        "execution",
        "background",
    ]
    assert health["runtime_host"]["shutdown_order"] == [
        "background",
        "execution",
        "memory",
        "state",
        "trace",
        "router",
    ]
    assert set(health) >= {"control_plane", "runtime_host", "rustification", "router", "trace", "execution_environment"}
    assert "state" not in health
    assert "memory" not in health
    assert "background" not in health


def test_runtime_run_task_delegates_execution_to_service_kernel(tmp_path: Path) -> None:
    """Runtime should treat the execution service as the single kernel entry point."""

    trace_path = tmp_path / "TRACE_METADATA.json"
    runtime = CodexAgnoRuntime(
        RuntimeSettings(
            codex_home=PROJECT_ROOT,
            data_dir=tmp_path / "runtime-data",
            trace_output_path=trace_path,
            live_model_override=False,
        )
    )
    seen: dict[str, object] = {}
    expected_kernel_metadata = runtime.execution_service.kernel_payload(dry_run=True)

    async def fake_execute(*, ctx, dry_run: bool, trace_event_count: int, trace_output_path: str | None):
        seen["prompt"] = ctx.prompt
        seen["metadata"] = dict(ctx.metadata)
        seen["dry_run"] = dry_run
        seen["trace_event_count"] = trace_event_count
        seen["trace_output_path"] = trace_output_path
        return RunTaskResponse(
            session_id=ctx.session_id,
            user_id=ctx.user_id,
            skill=ctx.routing_result.selected_skill.name,
            overlay=ctx.routing_result.overlay_skill.name if ctx.routing_result.overlay_skill else None,
            live_run=False,
            content="delegated",
            prompt_preview="Rust-owned dry-run prompt",
            metadata={
                **expected_kernel_metadata,
                "trace_event_count": trace_event_count,
                "trace_output_path": trace_output_path,
            },
        )

    runtime.execution_service.execute = fake_execute  # type: ignore[method-assign]

    async def _run() -> None:
        response = await runtime.run_task(
            RunTaskRequest(
                task="帮我写一个 Rust CLI 工具",
                user_id="tester",
                dry_run=True,
            )
        )
        assert response.content == "delegated"
        assert seen["dry_run"] is True
        assert seen["prompt"] == ""
        assert seen["metadata"]["max_concurrent_subagents"] == 8
        assert seen["metadata"]["subagent_timeout_seconds"] == 900
        assert seen["trace_event_count"] >= 4
        assert seen["trace_output_path"] == str(trace_path)
        assert response.prompt_preview == "Rust-owned dry-run prompt"
        assert response.metadata["execution_kernel"] == expected_kernel_metadata["execution_kernel"]
        assert response.metadata["execution_kernel_authority"] == expected_kernel_metadata["execution_kernel_authority"]
        assert response.metadata["execution_kernel_delegate"] == expected_kernel_metadata["execution_kernel_delegate"]
        assert response.metadata["execution_kernel_delegate_authority"] == expected_kernel_metadata["execution_kernel_delegate_authority"]
        assert response.metadata["execution_kernel_delegate_family"] == expected_kernel_metadata["execution_kernel_delegate_family"]
        assert response.metadata["execution_kernel_delegate_impl"] == expected_kernel_metadata["execution_kernel_delegate_impl"]
        assert response.metadata["execution_kernel_live_primary"] == expected_kernel_metadata["execution_kernel_live_primary"]
        assert response.metadata["execution_kernel_live_primary_authority"] == expected_kernel_metadata["execution_kernel_live_primary_authority"]
        assert response.metadata["trace_event_schema_version"] == TRACE_EVENT_SCHEMA_VERSION

    asyncio.run(_run())


def test_runtime_write_resume_manifest_reuses_resolved_transport(tmp_path: Path) -> None:
    """Resume-manifest writes should not re-resolve transport when a fresh binding already exists."""

    trace_path = tmp_path / "TRACE_METADATA.json"
    runtime = CodexAgnoRuntime(
        RuntimeSettings(
            codex_home=PROJECT_ROOT,
            data_dir=tmp_path / "runtime-data",
            trace_output_path=trace_path,
            live_model_override=False,
        )
    )
    runtime._trace.record(
        session_id="reuse-transport-session",
        kind="job.started",
        stage="background",
    )
    transport = runtime.trace_service.describe_transport(session_id="reuse-transport-session")

    runtime.trace_service.describe_transport = lambda **_: (_ for _ in ()).throw(  # type: ignore[method-assign]
        AssertionError("transport should be reused instead of re-resolved")
    )

    runtime._write_resume_manifest(
        session_id="reuse-transport-session",
        job_id=None,
        status="dry_run",
        artifact_paths=[],
        transport=transport,
    )

    resume_manifest_path = trace_path.with_name("TRACE_RESUME_MANIFEST.json")
    resume_manifest = json.loads(resume_manifest_path.read_text(encoding="utf-8"))
    assert resume_manifest["event_transport_path"] == transport.binding_artifact_path


def test_runtime_live_path_tolerates_empty_python_prompt_context(tmp_path: Path) -> None:
    """Live execution should not require Python middleware to populate prompt text."""

    trace_path = tmp_path / "TRACE_METADATA.json"
    runtime = CodexAgnoRuntime(
        RuntimeSettings(
            codex_home=PROJECT_ROOT,
            data_dir=tmp_path / "runtime-data",
            trace_output_path=trace_path,
            live_model_override=True,
        )
    )
    seen: dict[str, object] = {}

    async def fake_execute(*, ctx, dry_run: bool, trace_event_count: int, trace_output_path: str | None):
        seen["prompt"] = ctx.prompt
        seen["dry_run"] = dry_run
        return RunTaskResponse(
            session_id=ctx.session_id,
            user_id=ctx.user_id,
            skill=ctx.routing_result.selected_skill.name,
            overlay=ctx.routing_result.overlay_skill.name if ctx.routing_result.overlay_skill else None,
            live_run=True,
            content="live rust result",
            prompt_preview=None,
            model_id="gpt-5.4",
            usage=UsageMetrics(input_tokens=13, output_tokens=8, total_tokens=21, mode="live"),
            metadata={
                "execution_kernel": "rust-execution-kernel-slice",
                "execution_kernel_authority": "rust-execution-kernel-authority",
                "execution_kernel_contract_mode": "rust-live-primary",
                "execution_kernel_fallback_policy": "infrastructure-only-explicit",
                "execution_kernel_in_process_replacement_complete": True,
                "execution_kernel_delegate": "router-rs",
                "execution_kernel_delegate_authority": "rust-execution-cli",
                "execution_kernel_delegate_family": "rust-cli",
                "execution_kernel_delegate_impl": "router-rs",
                "execution_kernel_live_primary": "router-rs",
                "execution_kernel_live_primary_authority": "rust-execution-cli",
                "execution_kernel_metadata_schema_version": "router-rs-execution-kernel-metadata-v1",
                "execution_kernel_response_shape": "live_primary",
                "execution_kernel_prompt_preview_owner": "rust-execution-cli",
            },
        )

    runtime.execution_service.execute = fake_execute  # type: ignore[method-assign]

    async def _run() -> None:
        response = await runtime.run_task(
            RunTaskRequest(
                task="帮我写一个 Rust CLI 工具",
                user_id="tester",
                dry_run=False,
            )
        )
        assert seen["dry_run"] is False
        assert seen["prompt"] == ""
        assert response.live_run is True
        assert response.prompt_preview is None
        assert response.model_id == "gpt-5.4"
        assert response.metadata["execution_kernel"] == "rust-execution-kernel-slice"
        assert response.metadata["execution_kernel_authority"] == "rust-execution-kernel-authority"
        assert response.metadata["execution_kernel_delegate"] == "router-rs"
        assert response.metadata["execution_kernel_delegate_authority"] == "rust-execution-cli"
        assert response.metadata["execution_kernel_live_primary"] == "router-rs"
        assert response.metadata["execution_kernel_live_primary_authority"] == "rust-execution-cli"

    asyncio.run(_run())


def test_runtime_dry_run_keeps_working_when_live_fallback_is_disabled(tmp_path: Path) -> None:
    """Dry-run should stay available even when the Python live fallback is turned off."""

    trace_path = tmp_path / "TRACE_METADATA.json"
    runtime = CodexAgnoRuntime(
        RuntimeSettings(
            codex_home=PROJECT_ROOT,
            data_dir=tmp_path / "runtime-data",
            trace_output_path=trace_path,
            live_model_override=False,
        )
    )

    async def _run() -> None:
        response = await runtime.run_task(
            RunTaskRequest(
                task="帮我写一个 Rust CLI 工具",
                user_id="tester",
                dry_run=True,
            )
        )
        assert response.live_run is False
        assert response.metadata["execution_kernel_contract_mode"] == "rust-live-primary"
        assert response.metadata["execution_kernel_delegate"] == "router-rs"
        assert response.metadata["execution_kernel_delegate_authority"] == "rust-execution-cli"
        assert response.metadata["execution_kernel_delegate_family"] == "rust-cli"
        assert response.metadata["execution_kernel_delegate_impl"] == "router-rs"
        assert response.metadata["execution_kernel_live_primary"] == "router-rs"
        assert response.metadata["execution_kernel_live_primary_authority"] == "rust-execution-cli"

    asyncio.run(_run())


def test_runtime_event_bridge_can_subscribe_resume_and_cleanup(tmp_path: Path) -> None:
    """Runtime should expose the event bridge for host-adapter style consumption."""

    with _project_supervisor_state() as supervisor_state_path:
        trace_path = tmp_path / "TRACE_METADATA.json"
        runtime = CodexAgnoRuntime(
            RuntimeSettings(
                codex_home=PROJECT_ROOT,
                data_dir=tmp_path / "runtime-data",
                trace_output_path=trace_path,
                live_model_override=False,
            )
        )

        async def _run() -> None:
            response = await runtime.run_task(
                RunTaskRequest(
                    task="帮我写一个 Rust CLI 工具",
                    session_id="bridge-session",
                    user_id="tester",
                    dry_run=True,
                )
            )
            first_window = runtime.subscribe_runtime_events(session_id=response.session_id, limit=2)
            assert first_window["schema_version"] == "runtime-event-bridge-v1"
            assert len(first_window["events"]) == 2
            assert first_window["has_more"] is True

            attached_runtime = CodexAgnoRuntime(
                RuntimeSettings(
                    codex_home=PROJECT_ROOT,
                    data_dir=tmp_path / "attached-runtime-data",
                    trace_output_path=tmp_path / "ATTACHED_TRACE_METADATA.json",
                    live_model_override=False,
                )
            )
            transport = runtime.describe_runtime_event_transport(session_id=response.session_id)
            attached = attached_runtime.attach_runtime_event_transport(
                binding_artifact_path=transport["binding_artifact_path"]
            )
            assert attached["attach_mode"] == "process_external_artifact_replay"
            assert attached["authority"] == attached_runtime.rust_adapter.attached_runtime_event_transport_authority
            assert attached["transport"]["stream_id"] == transport["stream_id"]
            assert attached["binding_artifact_path"] == transport["binding_artifact_path"]
            assert attached["trace_stream_path"].endswith("TRACE_EVENTS.jsonl")
            assert attached["cleanup_semantics"] == "no_persisted_state"
            assert attached["cleanup_preserves_replay"] is True
            assert attached["source_handoff_method"] == "describe_runtime_event_handoff"
            assert attached["source_transport_method"] == "describe_runtime_event_transport"
            assert attached["attach_method"] == "attach_runtime_event_transport"
            assert attached["subscribe_method"] == "subscribe_attached_runtime_events"
            assert attached["cleanup_method"] == "cleanup_attached_runtime_event_transport"
            assert attached["resume_mode"] == "after_event_id"
            assert attached["attach_descriptor"]["schema_version"] == "runtime-event-attach-descriptor-v1"
            assert attached["attach_descriptor"]["source_handoff_method"] == "describe_runtime_event_handoff"
            assert attached["attach_descriptor"]["source_transport_method"] == "describe_runtime_event_transport"
            assert attached["attach_descriptor"]["attach_method"] == "attach_runtime_event_transport"
            assert attached["attach_descriptor"]["subscribe_method"] == "subscribe_attached_runtime_events"
            assert attached["attach_descriptor"]["cleanup_method"] == "cleanup_attached_runtime_event_transport"
            assert attached["attach_descriptor"]["resume_mode"] == "after_event_id"
            assert attached["attach_descriptor"]["cleanup_semantics"] == "no_persisted_state"
            assert attached["attach_descriptor"]["attach_capabilities"] == {
                "artifact_replay": True,
                "live_remote_stream": False,
                "cleanup_preserves_replay": True,
            }
            assert attached["attach_descriptor"]["resolved_artifacts"]["binding_artifact_path"] == (
                transport["binding_artifact_path"]
            )
            assert attached["attach_descriptor"]["resolution"]["binding_artifact_path"] == "explicit_request"
            resumed_via_binding = attached_runtime.subscribe_attached_runtime_events(
                attach_descriptor=attached["attach_descriptor"],
                after_event_id=first_window["events"][-1]["event_id"],
                limit=20,
            )
            assert resumed_via_binding["schema_version"] == "runtime-event-bridge-v1"
            assert resumed_via_binding["events"]
            assert resumed_via_binding["after_event_id"] == first_window["events"][-1]["event_id"]

            handoff = runtime.describe_runtime_event_handoff(session_id=response.session_id)
            attached_via_manifest = attached_runtime.attach_runtime_event_transport(
                resume_manifest_path=handoff["resume_manifest_path"]
            )
            assert attached_via_manifest["handoff"] is None
            assert attached_via_manifest["resume_manifest"]["session_id"] == response.session_id
            assert attached_via_manifest["binding_artifact_path"] == transport["binding_artifact_path"]
            assert attached_via_manifest["attach_descriptor"]["resolution"]["binding_artifact_path"] == (
                "resume_manifest"
            )
            assert attached_via_manifest["attach_descriptor"]["resolution"]["resume_manifest_path"] == (
                "explicit_request"
            )
            resumed_via_manifest = attached_runtime.subscribe_attached_runtime_events(
                attach_descriptor=attached_via_manifest["attach_descriptor"],
                after_event_id=first_window["events"][-1]["event_id"],
                limit=20,
            )
            assert resumed_via_manifest["events"]
            assert resumed_via_manifest["after_event_id"] == first_window["events"][-1]["event_id"]
            manifest_cleanup = attached_runtime.cleanup_attached_runtime_event_transport(
                attach_descriptor=attached_via_manifest["attach_descriptor"]
            )
            assert manifest_cleanup["authority"] == attached_runtime.rust_adapter.attached_runtime_event_transport_authority
            assert manifest_cleanup["cleanup_semantics"] == "no_persisted_state"
            assert manifest_cleanup["cleanup_preserves_replay"] is True

            handoff_path = Path(transport["binding_artifact_path"]).with_name("ATTACHED_RUNTIME_EVENT_HANDOFF.json")
            handoff_path.write_text(json.dumps(handoff, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
            attached_via_handoff = attached_runtime.attach_runtime_event_transport(handoff_path=str(handoff_path))
            assert attached_via_handoff["handoff"]["stream_id"] == handoff["stream_id"]
            assert attached_via_handoff["resume_manifest"]["session_id"] == response.session_id
            assert attached_via_handoff["attach_descriptor"]["resolution"]["handoff_path"] == "explicit_request"
            assert attached_via_handoff["attach_descriptor"]["resolution"]["resume_manifest_path"] == (
                "handoff_manifest"
            )
            idle_via_handoff = attached_runtime.subscribe_attached_runtime_events(
                attach_descriptor=attached_via_handoff["attach_descriptor"],
                after_event_id=transport["latest_cursor"]["event_id"],
                heartbeat=True,
            )
            assert idle_via_handoff["events"] == []
            assert idle_via_handoff["heartbeat"]["status"] == "idle"
            attached_cleanup = attached_runtime.cleanup_attached_runtime_event_transport(
                attach_descriptor=attached_via_handoff["attach_descriptor"]
            )
            assert attached_cleanup["authority"] == attached_runtime.rust_adapter.attached_runtime_event_transport_authority
            assert attached_cleanup["cleanup_semantics"] == "no_persisted_state"
            assert attached_cleanup["cleanup_preserves_replay"] is True

            transport = runtime.describe_runtime_event_transport(session_id=response.session_id)
            assert transport["schema_version"] == TRACE_EVENT_TRANSPORT_SCHEMA_VERSION
            assert transport["session_id"] == response.session_id
            assert transport["transport_kind"] == "poll"
            assert transport["transport_family"] == "host-facing-bridge"
            assert transport["endpoint_kind"] == "runtime_method"
            assert transport["remote_capable"] is True
            assert transport["remote_attach_supported"] is True
            assert response.metadata["trace_event_transport_attach_mode"] == "process_external_artifact_replay"
            assert transport["attach_mode"] == "process_external_artifact_replay"
            assert transport["binding_artifact_role"] == "primary_attach_descriptor"
            assert transport["recommended_remote_attach_method"] == "describe_runtime_event_handoff"
            assert transport["handoff_supported"] is True
            assert transport["handoff_method"] == "describe_runtime_event_handoff"
            assert transport["handoff_kind"] == "artifact_handoff"
            assert transport["binding_refresh_mode"] == "describe_or_checkpoint"
            assert transport["binding_artifact_format"] == "json"
            assert transport["binding_backend_family"] == "filesystem"
            assert transport["binding_artifact_path"].endswith(
                f"runtime_event_transports/{response.session_id}__{response.session_id}.json"
            )
            assert response.metadata["trace_resume_manifest_binding_path"] == transport["binding_artifact_path"]
            assert response.metadata["trace_event_transport_path"] == transport["binding_artifact_path"]
            assert transport["describe_method"] == "describe_runtime_event_transport"
            assert transport["subscribe_method"] == "subscribe_runtime_events"
            assert transport["cleanup_method"] == "cleanup_runtime_events"
            assert transport["cleanup_semantics"] == "bridge_cache_only"
            assert transport["cleanup_preserves_replay"] is True
            assert transport["replay_reseed_supported"] is True
            assert transport["latest_cursor"]["schema_version"] == TRACE_REPLAY_CURSOR_SCHEMA_VERSION
            assert transport["attach_target"]["endpoint_kind"] == "runtime_method"
            assert transport["attach_target"]["session_id"] == response.session_id
            assert transport["replay_anchor"]["anchor_kind"] == "trace_replay_cursor"
            assert transport["replay_anchor"]["latest_cursor"]["schema_version"] == TRACE_REPLAY_CURSOR_SCHEMA_VERSION
            persisted = json.loads(Path(transport["binding_artifact_path"]).read_text(encoding="utf-8"))
            assert persisted["stream_id"] == transport["stream_id"]
            assert persisted["session_id"] == response.session_id
            assert persisted["latest_cursor"]["schema_version"] == TRACE_REPLAY_CURSOR_SCHEMA_VERSION
            assert persisted["attach_target"]["session_id"] == response.session_id

            handoff = runtime.describe_runtime_event_handoff(session_id=response.session_id)
            assert handoff["schema_version"] == "runtime-event-handoff-v1"
            assert handoff["stream_id"] == transport["stream_id"]
            assert handoff["checkpoint_backend_family"] == "filesystem"
            assert handoff["attach_mode"] == "process_external_artifact_replay"
            assert handoff["resume_manifest_role"] == "checkpoint_recovery_anchor"
            assert handoff["trace_stream_path"].endswith("TRACE_EVENTS.jsonl")
            assert handoff["resume_manifest_path"].endswith("TRACE_RESUME_MANIFEST.json")
            assert handoff["remote_attach_strategy"] == "transport_descriptor_then_replay"
            assert handoff["cleanup_preserves_replay"] is True
            assert handoff["attach_target"]["session_id"] == response.session_id
            assert handoff["replay_anchor"]["anchor_kind"] == "trace_replay_cursor"
            assert handoff["recovery_artifacts"] == [
                transport["binding_artifact_path"],
                handoff["resume_manifest_path"],
                handoff["trace_stream_path"],
            ]
            assert handoff["transport"]["binding_artifact_path"] == transport["binding_artifact_path"]

            after_event_id = first_window["events"][-1]["event_id"]
            assert first_window["next_cursor"]["event_id"] == after_event_id
            resumed = runtime.subscribe_runtime_events(
                session_id=response.session_id,
                after_event_id=after_event_id,
                limit=20,
            )
            assert resumed["events"]
            assert resumed["after_event_id"] == after_event_id

            tail_event_id = resumed["events"][-1]["event_id"]
            idle = runtime.subscribe_runtime_events(
                session_id=response.session_id,
                after_event_id=tail_event_id,
                heartbeat=True,
            )
            assert idle["events"] == []
            assert idle["heartbeat"]["kind"] == "bridge.heartbeat"
            assert idle["heartbeat"]["status"] == "idle"

            resumed_after_cleanup = runtime.subscribe_runtime_events(
                session_id=response.session_id,
                after_event_id=after_event_id,
                limit=20,
            )
            assert resumed_after_cleanup["events"]

            runtime.cleanup_runtime_events(session_id=response.session_id)
            reseeded = runtime.subscribe_runtime_events(
                session_id=response.session_id,
                after_event_id=after_event_id,
                limit=20,
            )
            assert reseeded["events"]
            assert reseeded["after_event_id"] == after_event_id

            resume_manifest = json.loads(trace_path.with_name("TRACE_RESUME_MANIFEST.json").read_text(encoding="utf-8"))
            assert resume_manifest["event_transport_path"] == transport["binding_artifact_path"]
            assert transport["binding_artifact_path"] in resume_manifest["artifact_paths"]
            assert str(supervisor_state_path.resolve()) in resume_manifest["artifact_paths"]
            assert resume_manifest["supervisor_projection"]["supervisor_state_path"] == str(
                supervisor_state_path.resolve()
            )
            supervisor_state = json.loads(supervisor_state_path.read_text(encoding="utf-8"))
            assert resume_manifest["supervisor_projection"]["delegation"]["sidecar_count"] == len(
                supervisor_state["delegation"]["delegated_sidecars"]
            )

        asyncio.run(_run())


def test_runtime_event_attach_replays_from_sqlite_backend(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    """External attach should replay through the SQLite backend when artifacts are not materialized as files."""

    monkeypatch.setenv("CODEX_AGNO_CHECKPOINT_STORAGE_BACKEND_FAMILY", "sqlite")
    monkeypatch.setenv("CODEX_AGNO_CHECKPOINT_STORAGE_DB_FILE", "runtime_checkpoint_store.sqlite3")

    with _project_supervisor_state():
        data_dir = tmp_path / "runtime-data"
        trace_path = data_dir / "TRACE_METADATA.json"
        settings = RuntimeSettings(
            codex_home=PROJECT_ROOT,
            data_dir=data_dir,
            trace_output_path=trace_path,
            live_model_override=False,
        )
        runtime = CodexAgnoRuntime(settings)

        async def _run() -> None:
            response = await runtime.run_task(
                RunTaskRequest(
                    task="帮我写一个 Rust CLI 工具",
                    user_id="tester",
                    dry_run=True,
                )
            )

            transport = runtime.describe_runtime_event_transport(session_id=response.session_id)
            handoff = runtime.describe_runtime_event_handoff(session_id=response.session_id)
            sqlite_db_path = settings.resolved_data_dir / "runtime_checkpoint_store.sqlite3"

            assert transport["binding_backend_family"] == "sqlite"
            assert handoff["checkpoint_backend_family"] == "sqlite"
            assert sqlite_db_path.exists()
            assert not Path(transport["binding_artifact_path"]).exists()
            assert not Path(handoff["resume_manifest_path"]).exists()
            assert not Path(handoff["trace_stream_path"]).exists()

            attached_runtime = CodexAgnoRuntime(
                RuntimeSettings(
                    codex_home=PROJECT_ROOT,
                    data_dir=tmp_path / "attached-runtime-data",
                    trace_output_path=tmp_path / "attached-runtime-data" / "ATTACHED_TRACE_METADATA.json",
                    live_model_override=False,
                )
            )
            attached = attached_runtime.attach_runtime_event_transport(
                binding_artifact_path=transport["binding_artifact_path"]
            )
            assert attached["artifact_backend_family"] == "sqlite"
            assert attached["authority"] == attached_runtime.rust_adapter.attached_runtime_event_transport_authority
            assert attached["transport"]["binding_backend_family"] == "sqlite"
            assert attached["resume_manifest"]["session_id"] == response.session_id
            assert attached["attach_descriptor"]["resolution"]["binding_artifact_path"] == "explicit_request"

            replay = attached_runtime.subscribe_attached_runtime_events(
                attach_descriptor=attached["attach_descriptor"],
                limit=20,
            )
            assert replay["events"]
            assert replay["events"][0]["session_id"] == response.session_id

            attached_via_manifest = attached_runtime.attach_runtime_event_transport(
                resume_manifest_path=handoff["resume_manifest_path"]
            )
            assert attached_via_manifest["artifact_backend_family"] == "sqlite"
            assert attached_via_manifest["binding_artifact_path"] == transport["binding_artifact_path"]
            assert attached_via_manifest["attach_descriptor"]["resolution"]["binding_artifact_path"] == (
                "resume_manifest"
            )

        asyncio.run(_run())


def test_runtime_event_attach_rejects_mismatched_handoff_binding_artifact(tmp_path: Path) -> None:
    """External attach should fail closed when handoff metadata points at a different binding artifact."""

    with _project_supervisor_state():
        trace_path = tmp_path / "TRACE_METADATA.json"
        runtime = CodexAgnoRuntime(
            RuntimeSettings(
                codex_home=PROJECT_ROOT,
                data_dir=tmp_path / "runtime-data",
                trace_output_path=trace_path,
                live_model_override=False,
            )
        )

        async def _run() -> None:
            response = await runtime.run_task(
                RunTaskRequest(
                    task="帮我写一个 Rust CLI 工具",
                    session_id="attach-mismatch-session",
                    user_id="tester",
                    dry_run=True,
                )
            )
            transport = runtime.describe_runtime_event_transport(session_id=response.session_id)
            handoff = runtime.describe_runtime_event_handoff(session_id=response.session_id)

            mismatched_handoff = dict(handoff)
            mismatched_handoff["transport"] = dict(handoff["transport"])
            mismatched_handoff["transport"]["binding_artifact_path"] = str(
                Path(transport["binding_artifact_path"]).with_name("WRONG_RUNTIME_EVENT_TRANSPORT.json")
            )
            handoff_path = Path(transport["binding_artifact_path"]).with_name("MISMATCHED_RUNTIME_EVENT_HANDOFF.json")
            handoff_path.write_text(
                json.dumps(mismatched_handoff, ensure_ascii=False, indent=2) + "\n",
                encoding="utf-8",
            )

            attached_runtime = CodexAgnoRuntime(
                RuntimeSettings(
                    codex_home=PROJECT_ROOT,
                    data_dir=tmp_path / "attached-runtime-data",
                    trace_output_path=tmp_path / "ATTACHED_TRACE_METADATA.json",
                    live_model_override=False,
                )
            )
            with pytest.raises(
                ValueError,
                match="mismatched transport/handoff binding artifact paths",
            ):
                attached_runtime.attach_runtime_event_transport(
                    binding_artifact_path=transport["binding_artifact_path"],
                    handoff_path=str(handoff_path),
                )

        asyncio.run(_run())


def test_runtime_event_attach_rejects_mismatched_resume_binding_artifact(tmp_path: Path) -> None:
    """External attach should fail closed when resume metadata points at a different binding artifact."""

    with _project_supervisor_state():
        trace_path = tmp_path / "TRACE_METADATA.json"
        runtime = CodexAgnoRuntime(
            RuntimeSettings(
                codex_home=PROJECT_ROOT,
                data_dir=tmp_path / "runtime-data",
                trace_output_path=trace_path,
                live_model_override=False,
            )
        )

        async def _run() -> None:
            response = await runtime.run_task(
                RunTaskRequest(
                    task="帮我写一个 Rust CLI 工具",
                    session_id="resume-mismatch-session",
                    user_id="tester",
                    dry_run=True,
                )
            )
            transport = runtime.describe_runtime_event_transport(session_id=response.session_id)
            handoff = runtime.describe_runtime_event_handoff(session_id=response.session_id)
            resume_manifest_path = Path(handoff["resume_manifest_path"])
            resume_manifest = json.loads(resume_manifest_path.read_text(encoding="utf-8"))
            resume_manifest["event_transport_path"] = str(
                Path(transport["binding_artifact_path"]).with_name("WRONG_RUNTIME_EVENT_TRANSPORT.json")
            )
            mismatched_resume_path = resume_manifest_path.with_name("MISMATCHED_TRACE_RESUME_MANIFEST.json")
            mismatched_resume_path.write_text(
                json.dumps(resume_manifest, ensure_ascii=False, indent=2) + "\n",
                encoding="utf-8",
            )

            attached_runtime = CodexAgnoRuntime(
                RuntimeSettings(
                    codex_home=PROJECT_ROOT,
                    data_dir=tmp_path / "attached-runtime-data",
                    trace_output_path=tmp_path / "ATTACHED_TRACE_METADATA.json",
                    live_model_override=False,
                )
            )
            with pytest.raises(
                ValueError,
                match="mismatched transport/resume binding artifact paths",
            ):
                attached_runtime.attach_runtime_event_transport(
                    binding_artifact_path=transport["binding_artifact_path"],
                    resume_manifest_path=str(mismatched_resume_path),
                )

        asyncio.run(_run())


def test_runtime_event_attach_rejects_conflicting_attach_descriptor_args(tmp_path: Path) -> None:
    """Stable attach descriptors should fail closed when callers also pass conflicting direct paths."""

    with _project_supervisor_state():
        trace_path = tmp_path / "TRACE_METADATA.json"
        runtime = CodexAgnoRuntime(
            RuntimeSettings(
                codex_home=PROJECT_ROOT,
                data_dir=tmp_path / "runtime-data",
                trace_output_path=trace_path,
                live_model_override=False,
            )
        )

        async def _run() -> None:
            response = await runtime.run_task(
                RunTaskRequest(
                    task="帮我写一个 Rust CLI 工具",
                    session_id="attach-descriptor-conflict-session",
                    user_id="tester",
                    dry_run=True,
                )
            )
            transport = runtime.describe_runtime_event_transport(session_id=response.session_id)
            attached_runtime = CodexAgnoRuntime(
                RuntimeSettings(
                    codex_home=PROJECT_ROOT,
                    data_dir=tmp_path / "attached-runtime-data",
                    trace_output_path=tmp_path / "ATTACHED_TRACE_METADATA.json",
                    live_model_override=False,
                )
            )
            attach_descriptor = attached_runtime.attach_runtime_event_transport(
                binding_artifact_path=transport["binding_artifact_path"]
            )["attach_descriptor"]

            with pytest.raises(ValueError, match="conflicting 'binding_artifact_path' values"):
                attached_runtime.attach_runtime_event_transport(
                    attach_descriptor=attach_descriptor,
                    binding_artifact_path=str(
                        Path(transport["binding_artifact_path"]).with_name("WRONG_RUNTIME_EVENT_TRANSPORT.json")
                    ),
                )

        asyncio.run(_run())


def test_runtime_event_attach_rejects_missing_explicit_binding_artifact(tmp_path: Path) -> None:
    """Explicit artifact paths should not be ignored just because another handoff artifact exists."""

    with _project_supervisor_state():
        trace_path = tmp_path / "TRACE_METADATA.json"
        runtime = CodexAgnoRuntime(
            RuntimeSettings(
                codex_home=PROJECT_ROOT,
                data_dir=tmp_path / "runtime-data",
                trace_output_path=trace_path,
                live_model_override=False,
            )
        )

        async def _run() -> None:
            response = await runtime.run_task(
                RunTaskRequest(
                    task="帮我写一个 Rust CLI 工具",
                    session_id="attach-missing-binding-session",
                    user_id="tester",
                    dry_run=True,
                )
            )
            transport = runtime.describe_runtime_event_transport(session_id=response.session_id)
            handoff = runtime.describe_runtime_event_handoff(session_id=response.session_id)
            handoff_path = Path(transport["binding_artifact_path"]).with_name("ATTACHED_RUNTIME_EVENT_HANDOFF.json")
            handoff_path.write_text(json.dumps(handoff, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

            attached_runtime = CodexAgnoRuntime(
                RuntimeSettings(
                    codex_home=PROJECT_ROOT,
                    data_dir=tmp_path / "attached-runtime-data",
                    trace_output_path=tmp_path / "ATTACHED_TRACE_METADATA.json",
                    live_model_override=False,
                )
            )
            with pytest.raises(ValueError, match="requested 'binding_artifact_path' that does not exist"):
                attached_runtime.attach_runtime_event_transport(
                    binding_artifact_path=str(
                        Path(transport["binding_artifact_path"]).with_name("MISSING_RUNTIME_EVENT_TRANSPORT.json")
                    ),
                    handoff_path=str(handoff_path),
                )

        asyncio.run(_run())


def test_runtime_event_attach_rejects_mismatched_handoff_trace_stream_against_binding(tmp_path: Path) -> None:
    """External attach should fail closed when handoff trace stream disagrees with binding adjacency."""

    with _project_supervisor_state():
        trace_path = tmp_path / "TRACE_METADATA.json"
        runtime = CodexAgnoRuntime(
            RuntimeSettings(
                codex_home=PROJECT_ROOT,
                data_dir=tmp_path / "runtime-data",
                trace_output_path=trace_path,
                live_model_override=False,
            )
        )

        async def _run() -> None:
            response = await runtime.run_task(
                RunTaskRequest(
                    task="帮我写一个 Rust CLI 工具",
                    session_id="handoff-trace-mismatch-session",
                    user_id="tester",
                    dry_run=True,
                )
            )
            transport = runtime.describe_runtime_event_transport(session_id=response.session_id)
            handoff = runtime.describe_runtime_event_handoff(session_id=response.session_id)
            mismatched_handoff = dict(handoff)
            wrong_trace_stream_path = str(
                Path(handoff["trace_stream_path"]).with_name("WRONG_TRACE_EVENTS.jsonl")
            )
            mismatched_handoff["trace_stream_path"] = str(
                wrong_trace_stream_path
            )
            resume_manifest_path = Path(handoff["resume_manifest_path"])
            resume_manifest = json.loads(resume_manifest_path.read_text(encoding="utf-8"))
            resume_manifest["trace_stream_path"] = wrong_trace_stream_path
            mismatched_resume_path = resume_manifest_path.with_name(
                "MISMATCHED_TRACE_STREAM_HANDOFF_RESUME_MANIFEST.json"
            )
            mismatched_resume_path.write_text(
                json.dumps(resume_manifest, ensure_ascii=False, indent=2) + "\n",
                encoding="utf-8",
            )
            mismatched_handoff["resume_manifest_path"] = str(mismatched_resume_path)
            handoff_path = Path(transport["binding_artifact_path"]).with_name("MISMATCHED_TRACE_STREAM_HANDOFF.json")
            handoff_path.write_text(
                json.dumps(mismatched_handoff, ensure_ascii=False, indent=2) + "\n",
                encoding="utf-8",
            )

            attached_runtime = CodexAgnoRuntime(
                RuntimeSettings(
                    codex_home=PROJECT_ROOT,
                    data_dir=tmp_path / "attached-runtime-data",
                    trace_output_path=tmp_path / "ATTACHED_TRACE_METADATA.json",
                    live_model_override=False,
                )
            )
            with pytest.raises(ValueError, match="mismatched binding/handoff trace stream paths"):
                attached_runtime.attach_runtime_event_transport(
                    binding_artifact_path=transport["binding_artifact_path"],
                    handoff_path=str(handoff_path),
                )

        asyncio.run(_run())


def test_runtime_event_attach_rejects_mismatched_resume_trace_stream_against_binding(tmp_path: Path) -> None:
    """External attach should fail closed when resume trace stream disagrees with binding adjacency."""

    with _project_supervisor_state():
        trace_path = tmp_path / "TRACE_METADATA.json"
        runtime = CodexAgnoRuntime(
            RuntimeSettings(
                codex_home=PROJECT_ROOT,
                data_dir=tmp_path / "runtime-data",
                trace_output_path=trace_path,
                live_model_override=False,
            )
        )

        async def _run() -> None:
            response = await runtime.run_task(
                RunTaskRequest(
                    task="帮我写一个 Rust CLI 工具",
                    session_id="resume-trace-mismatch-session",
                    user_id="tester",
                    dry_run=True,
                )
            )
            transport = runtime.describe_runtime_event_transport(session_id=response.session_id)
            handoff = runtime.describe_runtime_event_handoff(session_id=response.session_id)
            resume_manifest_path = Path(handoff["resume_manifest_path"])
            resume_manifest = json.loads(resume_manifest_path.read_text(encoding="utf-8"))
            resume_manifest["trace_stream_path"] = str(
                Path(handoff["trace_stream_path"]).with_name("WRONG_TRACE_EVENTS.jsonl")
            )
            mismatched_resume_path = resume_manifest_path.with_name("MISMATCHED_TRACE_STREAM_RESUME_MANIFEST.json")
            mismatched_resume_path.write_text(
                json.dumps(resume_manifest, ensure_ascii=False, indent=2) + "\n",
                encoding="utf-8",
            )

            attached_runtime = CodexAgnoRuntime(
                RuntimeSettings(
                    codex_home=PROJECT_ROOT,
                    data_dir=tmp_path / "attached-runtime-data",
                    trace_output_path=tmp_path / "ATTACHED_TRACE_METADATA.json",
                    live_model_override=False,
                )
            )
            with pytest.raises(ValueError, match="mismatched binding/resume trace stream paths"):
                attached_runtime.attach_runtime_event_transport(
                    binding_artifact_path=transport["binding_artifact_path"],
                    resume_manifest_path=str(mismatched_resume_path),
                )

        asyncio.run(_run())


def test_runtime_event_attach_rejects_descriptor_without_artifact_replay_contract(tmp_path: Path) -> None:
    """Attach descriptors must keep the replay-only contract instead of silently degrading."""

    with _project_supervisor_state():
        trace_path = tmp_path / "TRACE_METADATA.json"
        runtime = CodexAgnoRuntime(
            RuntimeSettings(
                codex_home=PROJECT_ROOT,
                data_dir=tmp_path / "runtime-data",
                trace_output_path=trace_path,
                live_model_override=False,
            )
        )

        async def _run() -> None:
            response = await runtime.run_task(
                RunTaskRequest(
                    task="帮我写一个 Rust CLI 工具",
                    session_id="attach-capability-session",
                    user_id="tester",
                    dry_run=True,
                )
            )
            transport = runtime.describe_runtime_event_transport(session_id=response.session_id)
            attached_runtime = CodexAgnoRuntime(
                RuntimeSettings(
                    codex_home=PROJECT_ROOT,
                    data_dir=tmp_path / "attached-runtime-data",
                    trace_output_path=tmp_path / "ATTACHED_TRACE_METADATA.json",
                    live_model_override=False,
                )
            )
            attach_descriptor = attached_runtime.attach_runtime_event_transport(
                binding_artifact_path=transport["binding_artifact_path"]
            )["attach_descriptor"]
            attach_descriptor["attach_capabilities"] = {
                **attach_descriptor["attach_capabilities"],
                "artifact_replay": False,
            }

            with pytest.raises(ValueError, match="attach_capabilities\\.artifact_replay=True"):
                attached_runtime.attach_runtime_event_transport(attach_descriptor=attach_descriptor)

        asyncio.run(_run())


def test_runtime_attached_replay_rejects_descriptor_that_drifts_from_canonical_trace_stream(
    tmp_path: Path,
) -> None:
    """Replay should fail closed when a caller mutates the canonical descriptor trace stream."""

    with _project_supervisor_state():
        trace_path = tmp_path / "TRACE_METADATA.json"
        runtime = CodexAgnoRuntime(
            RuntimeSettings(
                codex_home=PROJECT_ROOT,
                data_dir=tmp_path / "runtime-data",
                trace_output_path=trace_path,
                live_model_override=False,
            )
        )

        async def _run() -> None:
            response = await runtime.run_task(
                RunTaskRequest(
                    task="帮我写一个 Rust CLI 工具",
                    session_id="attach-drift-session",
                    user_id="tester",
                    dry_run=True,
                )
            )
            transport = runtime.describe_runtime_event_transport(session_id=response.session_id)
            attached_runtime = CodexAgnoRuntime(
                RuntimeSettings(
                    codex_home=PROJECT_ROOT,
                    data_dir=tmp_path / "attached-runtime-data",
                    trace_output_path=tmp_path / "ATTACHED_TRACE_METADATA.json",
                    live_model_override=False,
                )
            )
            attach_descriptor = attached_runtime.attach_runtime_event_transport(
                binding_artifact_path=transport["binding_artifact_path"]
            )["attach_descriptor"]
            attach_descriptor["resolved_artifacts"] = {
                **attach_descriptor["resolved_artifacts"],
                "trace_stream_path": str(
                    Path(transport["binding_artifact_path"]).with_name("WRONG_TRACE_EVENTS.jsonl")
                ),
            }

            with pytest.raises(
                ValueError,
                match="canonical 'resolved_artifacts\\.trace_stream_path'",
            ):
                attached_runtime.subscribe_attached_runtime_events(
                    attach_descriptor=attach_descriptor,
                    limit=5,
                )

        asyncio.run(_run())


def test_runtime_event_attach_rejects_descriptor_with_noncanonical_cleanup_resume_contract(
    tmp_path: Path,
) -> None:
    """External attach should fail closed when cleanup/resume vocabulary drifts from the Rust-owned contract."""

    with _project_supervisor_state():
        trace_path = tmp_path / "TRACE_METADATA.json"
        runtime = CodexAgnoRuntime(
            RuntimeSettings(
                codex_home=PROJECT_ROOT,
                data_dir=tmp_path / "runtime-data",
                trace_output_path=trace_path,
                live_model_override=False,
            )
        )

        async def _run() -> None:
            response = await runtime.run_task(
                RunTaskRequest(
                    task="帮我写一个 Rust CLI 工具",
                    session_id="attach-contract-drift-session",
                    user_id="tester",
                    dry_run=True,
                )
            )
            transport = runtime.describe_runtime_event_transport(session_id=response.session_id)
            attached_runtime = CodexAgnoRuntime(
                RuntimeSettings(
                    codex_home=PROJECT_ROOT,
                    data_dir=tmp_path / "attached-runtime-data",
                    trace_output_path=tmp_path / "ATTACHED_TRACE_METADATA.json",
                    live_model_override=False,
                )
            )
            attach_descriptor = attached_runtime.attach_runtime_event_transport(
                binding_artifact_path=transport["binding_artifact_path"]
            )["attach_descriptor"]
            attach_descriptor["cleanup_method"] = "cleanup_runtime_events"

            with pytest.raises(
                ValueError,
                match="cleanup_method='cleanup_attached_runtime_event_transport'",
            ):
                attached_runtime.attach_runtime_event_transport(attach_descriptor=attach_descriptor)

            attach_descriptor = attached_runtime.attach_runtime_event_transport(
                binding_artifact_path=transport["binding_artifact_path"]
            )["attach_descriptor"]
            attach_descriptor["resume_mode"] = "event_index"

            with pytest.raises(ValueError, match="resume_mode='after_event_id'"):
                attached_runtime.subscribe_attached_runtime_events(
                    attach_descriptor=attach_descriptor,
                    limit=5,
                )

        asyncio.run(_run())


def test_runtime_tracks_reroute_count_for_reused_session(tmp_path: Path) -> None:
    """Reuse of one session should increment reroute_count from trace history."""

    trace_path = tmp_path / "TRACE_METADATA.json"
    settings = RuntimeSettings(
        codex_home=PROJECT_ROOT,
        data_dir=tmp_path / "runtime-data",
        trace_output_path=trace_path,
        live_model_override=False,
    )
    runtime = CodexAgnoRuntime(settings)

    async def _run() -> None:
        first = await runtime.run_task(
            RunTaskRequest(
                task="帮我写一个 Rust CLI 工具",
                session_id="reroute-session",
                user_id="tester",
                dry_run=True,
            )
        )
        second = await runtime.run_task(
            RunTaskRequest(
                task="帮我写一个 Rust CLI 工具",
                session_id="reroute-session",
                user_id="tester",
                dry_run=True,
            )
        )
        assert first.metadata["reroute_count"] == 0
        assert second.metadata["reroute_count"] == 1
        assert second.metadata["retry_count"] == 0

    asyncio.run(_run())

    data = json.loads(trace_path.read_text(encoding="utf-8"))
    assert data["reroute_count"] == 1
    assert data["retry_count"] == 0


def test_background_job_store_rejects_duplicate_active_sessions() -> None:
    """Only one queued or running job may own a session at a time."""

    store = BackgroundJobStore()
    first = store.set_status("job-1", status="queued", session_id="shared-session", timeout_seconds=30)
    assert first.status == "queued"
    assert store.get_active_job("shared-session") == "job-1"

    with pytest.raises(SessionConflictError):
        store.set_status("job-2", status="queued", session_id="shared-session", timeout_seconds=30)

    store.set_status("job-1", status="running", session_id="shared-session", timeout_seconds=30, claimed_by="job-1")
    store.set_status("job-1", status="completed", session_id="shared-session", timeout_seconds=30, claimed_by="job-1")
    assert store.get_active_job("shared-session") is None

    second = store.set_status("job-2", status="queued", session_id="shared-session", timeout_seconds=30)
    assert second.status == "queued"
    assert store.get_active_job("shared-session") == "job-2"


def test_background_job_store_arbitrates_interrupt_takeovers() -> None:
    """The state reducer should serialize reserve/claim handoffs for one session."""

    store = BackgroundJobStore()
    store.set_status("job-1", status="queued", session_id="shared-session", timeout_seconds=30)

    decision = store.arbitrate_session_takeover(
        session_id="shared-session",
        incoming_job_id="job-2",
        operation="reserve",
    )
    assert decision.schema_version == BACKGROUND_SESSION_TAKEOVER_ARBITRATION_SCHEMA_VERSION
    assert decision.operation == "reserve"
    assert decision.outcome == "pending"
    assert decision.changed is True
    assert decision.previous_active_job_id == "job-1"
    assert decision.active_job_id == "job-1"
    assert decision.previous_pending_job_id is None
    assert decision.pending_job_id == "job-2"
    assert store.reserve_session_takeover(session_id="shared-session", incoming_job_id="job-2") == "job-1"

    with pytest.raises(SessionConflictError):
        store.arbitrate_session_takeover(
            session_id="shared-session",
            incoming_job_id="job-3",
            operation="reserve",
        )

    store.set_status("job-1", status="interrupted", session_id="shared-session", timeout_seconds=30)
    claim = store.arbitrate_session_takeover(
        session_id="shared-session",
        incoming_job_id="job-2",
        operation="claim",
    )
    assert claim.schema_version == BACKGROUND_SESSION_TAKEOVER_ARBITRATION_SCHEMA_VERSION
    assert claim.operation == "claim"
    assert claim.outcome == "claimed"
    assert claim.changed is True
    assert claim.previous_active_job_id is None
    assert claim.previous_pending_job_id == "job-2"
    assert claim.active_job_id == "job-2"
    assert claim.pending_job_id is None
    queued = store.set_status("job-2", status="queued", session_id="shared-session", timeout_seconds=30)
    noop_release = store.arbitrate_session_takeover(
        session_id="shared-session",
        incoming_job_id="job-2",
        operation="release",
    )

    assert queued.status == "queued"
    assert store.get_active_job("shared-session") == "job-2"
    assert store.pending_session_takeovers() == 0
    assert noop_release.outcome == "noop"
    assert noop_release.changed is False


def test_background_job_store_release_only_clears_pending_takeover() -> None:
    """Release should drop a pending takeover without tearing down the current owner."""

    store = BackgroundJobStore()
    store.set_status("job-1", status="queued", session_id="shared-session", timeout_seconds=30)

    store.arbitrate_session_takeover(
        session_id="shared-session",
        incoming_job_id="job-2",
        operation="reserve",
    )
    released = store.arbitrate_session_takeover(
        session_id="shared-session",
        incoming_job_id="job-2",
        operation="release",
    )

    assert released.schema_version == BACKGROUND_SESSION_TAKEOVER_ARBITRATION_SCHEMA_VERSION
    assert released.operation == "release"
    assert released.outcome == "released"
    assert released.changed is True
    assert released.active_job_id == "job-1"
    assert released.pending_job_id is None
    assert store.get_active_job("shared-session") == "job-1"
    assert store.pending_session_takeovers() == 0

    with pytest.raises(SessionConflictError):
        store.arbitrate_session_takeover(
            session_id="shared-session",
            incoming_job_id="job-2",
            operation="claim",
        )


def test_background_job_store_persists_versioned_state(tmp_path: Path) -> None:
    """Durable state should survive process restarts with stable schema fields."""

    state_path = tmp_path / "runtime_background_jobs.json"
    store = BackgroundJobStore(state_path=state_path)
    store.set_status("job-1", status="queued", session_id="session-1", timeout_seconds=30)
    store.set_status("job-1", status="running", session_id="session-1", timeout_seconds=30)
    store.set_status(
        "job-1",
        status="completed",
        session_id="session-1",
        timeout_seconds=30,
    )

    payload = json.loads(state_path.read_text(encoding="utf-8"))
    assert payload["schema_version"] == BACKGROUND_STATE_SCHEMA_VERSION
    assert payload["version"] == 2
    assert payload["jobs"][0]["job_id"] == "job-1"
    assert payload["jobs"][0]["status"] == "completed"
    assert payload["jobs"][0]["multitask_strategy"] == "reject"
    assert payload["jobs"][0]["max_attempts"] == 1
    assert payload["jobs"][0]["retry_count"] == 0
    assert payload["pending_session_takeovers"] == []

    recovered = BackgroundJobStore(state_path=state_path)
    recovered_row = recovered.get("job-1")
    assert recovered_row is not None
    assert recovered_row.status == "completed"
    assert recovered.get_active_job("session-1") is None

    recovered.set_status("job-2", status="queued", session_id="session-2", timeout_seconds=30)
    reloaded = BackgroundJobStore(state_path=state_path)
    assert reloaded.get_active_job("session-2") == "job-2"


def test_background_job_store_persists_pending_takeovers(tmp_path: Path) -> None:
    """Pending interrupt replacements should survive durable state round-trips."""

    state_path = tmp_path / "runtime_background_jobs.json"
    store = BackgroundJobStore(state_path=state_path)
    store.set_status("job-1", status="queued", session_id="shared-session", timeout_seconds=30)
    store.set_status("job-2", status="queued", session_id="replacement-session", timeout_seconds=30)

    assert store.reserve_session_takeover(session_id="shared-session", incoming_job_id="job-2") == "job-1"

    payload = json.loads(state_path.read_text(encoding="utf-8"))
    assert payload["version"] == 2
    assert payload["pending_session_takeovers"] == [
        {"incoming_job_id": "job-2", "session_id": "shared-session"}
    ]

    recovered = BackgroundJobStore(state_path=state_path)
    assert recovered.pending_session_takeovers() == 1


def test_background_job_store_preserves_takeover_claim_roundtrip_without_replacement_row(
    tmp_path: Path,
) -> None:
    """Reserve/claim should stay durable across reload even before the replacement row is materialized."""

    state_path = tmp_path / "runtime_background_jobs.json"
    store = BackgroundJobStore(state_path=state_path)
    store.set_status("job-1", status="queued", session_id="shared-session", timeout_seconds=30)

    reserved = store.arbitrate_session_takeover(
        session_id="shared-session",
        incoming_job_id="job-2",
        operation="reserve",
    )
    assert reserved.pending_job_id == "job-2"

    reloaded = BackgroundJobStore(state_path=state_path)
    store.set_status("job-1", status="interrupted", session_id="shared-session", timeout_seconds=30)
    reloaded_after_interrupt = BackgroundJobStore(state_path=state_path)
    claimed = reloaded_after_interrupt.arbitrate_session_takeover(
        session_id="shared-session",
        incoming_job_id="job-2",
        operation="claim",
    )

    assert reloaded.pending_session_takeovers() == 1
    assert claimed.outcome == "claimed"
    assert claimed.active_job_id == "job-2"
    assert claimed.pending_job_id is None

    claimed_reloaded = BackgroundJobStore(state_path=state_path)
    assert claimed_reloaded.get_active_job("shared-session") == "job-2"


def test_background_job_store_release_roundtrip_keeps_current_owner_without_replacement_row(
    tmp_path: Path,
) -> None:
    """Release should clear only the pending takeover after reload, not the active owner."""

    state_path = tmp_path / "runtime_background_jobs.json"
    store = BackgroundJobStore(state_path=state_path)
    store.set_status("job-1", status="queued", session_id="shared-session", timeout_seconds=30)
    store.arbitrate_session_takeover(
        session_id="shared-session",
        incoming_job_id="job-2",
        operation="reserve",
    )

    reloaded = BackgroundJobStore(state_path=state_path)
    released = reloaded.arbitrate_session_takeover(
        session_id="shared-session",
        incoming_job_id="job-2",
        operation="release",
    )

    assert released.outcome == "released"
    assert released.active_job_id == "job-1"
    assert released.pending_job_id is None

    released_reloaded = BackgroundJobStore(state_path=state_path)
    assert released_reloaded.pending_session_takeovers() == 0
    assert released_reloaded.get_active_job("shared-session") == "job-1"


def test_background_job_store_aggregates_parallel_group_summaries() -> None:
    """Parallel background batches should expose one durable aggregate summary."""

    store = BackgroundJobStore()
    store.set_status(
        "job-1",
        status="queued",
        session_id="session-1",
        parallel_group_id="pgroup-1",
        lane_id="lane-1",
        parent_job_id="parent-job",
        timeout_seconds=30,
    )
    store.set_status(
        "job-2",
        status="queued",
        session_id="session-2",
        parallel_group_id="pgroup-1",
        lane_id="lane-2",
        parent_job_id="parent-job",
        timeout_seconds=30,
    )
    store.set_status(
        "job-2",
        status="running",
        session_id="session-2",
        parallel_group_id="pgroup-1",
        lane_id="lane-2",
        parent_job_id="parent-job",
        timeout_seconds=30,
        claimed_by="job-2",
    )
    store.set_status(
        "job-2",
        status="completed",
        session_id="session-2",
        parallel_group_id="pgroup-1",
        lane_id="lane-2",
        parent_job_id="parent-job",
        timeout_seconds=30,
    )

    summary = store.parallel_group_summary("pgroup-1")

    assert summary is not None
    assert summary.parallel_group_id == "pgroup-1"
    assert summary.job_ids == ["job-1", "job-2"]
    assert summary.session_ids == ["session-1", "session-2"]
    assert summary.lane_ids == ["lane-1", "lane-2"]
    assert summary.parent_job_ids == ["parent-job"]
    assert summary.status_counts == {"completed": 1, "queued": 1}
    assert summary.active_job_count == 1
    assert summary.terminal_job_count == 1
    assert summary.total_job_count == 2
    assert store.parallel_group_summary("missing-group") is None
    assert store.health()["parallel_group_count"] == 1


def test_background_job_status_mutation_contract_defaults_and_merges() -> None:
    """The state reducer should own creation defaults and update preservation."""

    response = RunTaskResponse(
        session_id="session-1",
        user_id="tester",
        skill="test-skill",
        live_run=False,
        content="done",
        usage=UsageMetrics(total_tokens=3),
    )
    created = BackgroundJobStatusMutation(
        status="queued",
        session_id="session-1",
        result=response,
        timeout_seconds=30,
    ).apply(job_id="job-1", existing=None)

    assert created.job_id == "job-1"
    assert created.session_id == "session-1"
    assert created.status == "queued"
    assert created.multitask_strategy == "reject"
    assert created.attempt == 1
    assert created.retry_count == 0
    assert created.max_attempts == 1
    assert created.backoff_base_seconds == 0.0
    assert created.backoff_multiplier == 2.0
    assert created.result == response

    preserved_fields = created.model_dump(
        mode="python",
        exclude={"job_id", "status", "created_at", "updated_at", "result"},
    )
    updated = BackgroundJobStatusMutation(
        status="running",
        result=created.result,
        **preserved_fields,
    ).apply(job_id="job-1", existing=created)

    assert updated.status == "running"
    assert updated.created_at == created.created_at
    assert updated.updated_at != created.updated_at
    assert updated.result == created.result
    assert updated.model_dump(mode="python", exclude={"status", "updated_at", "result"}) == created.model_dump(
        mode="python",
        exclude={"status", "updated_at", "result"},
    )


def test_runtime_background_queue_rejects_duplicate_session_ids(tmp_path: Path) -> None:
    """Runtime integration should reject duplicate background runs for one session."""

    settings = RuntimeSettings(
        codex_home=PROJECT_ROOT,
        data_dir=tmp_path / "runtime-data",
        live_model_override=False,
    )
    runtime = CodexAgnoRuntime(settings)

    async def _run() -> None:
        request = BackgroundRunRequest(
            task="帮我写一个 Rust CLI 工具",
            user_id="tester",
            session_id="shared-session",
            dry_run=True,
        )
        first = await runtime.enqueue_background_run(request)
        second = await runtime.enqueue_background_run(request)

        assert first.status == "queued"
        assert first.session_id == "shared-session"
        assert second.status == "failed"
        assert second.session_id == "shared-session"
        assert "already active" in (second.error or "")

        status = None
        for _ in range(20):
            await asyncio.sleep(0.05)
            status = runtime.get_background_status(first.job_id)
            if status is not None and status.status == "completed":
                break
        assert status is not None
        assert status.status == "completed"

    asyncio.run(_run())


def test_runtime_background_queue_can_interrupt_duplicate_session_ids(tmp_path: Path) -> None:
    """Interrupt strategy should preempt the prior job and let the replacement run."""

    settings = RuntimeSettings(
        codex_home=PROJECT_ROOT,
        data_dir=tmp_path / "runtime-data",
        live_model_override=False,
    )
    runtime = CodexAgnoRuntime(settings)
    first_started = asyncio.Event()
    first_cancelled = asyncio.Event()

    async def fake_run_task(request: BackgroundRunRequest) -> RunTaskResponse:
        if request.task == "first-job":
            first_started.set()
            try:
                await asyncio.sleep(10)
            except asyncio.CancelledError:
                first_cancelled.set()
                raise
        return RunTaskResponse(
            session_id=request.session_id or "shared-session",
            user_id=request.user_id or "tester",
            skill="test-skill",
            live_run=False,
            content=request.task,
            usage=UsageMetrics(),
        )

    runtime.run_task = fake_run_task  # type: ignore[method-assign]

    async def _run() -> None:
        first = await runtime.enqueue_background_run(
            BackgroundRunRequest(
                task="first-job",
                user_id="tester",
                session_id="shared-session",
                dry_run=True,
            )
        )
        await first_started.wait()

        second = await runtime.enqueue_background_run(
            BackgroundRunRequest(
                task="replacement-job",
                user_id="tester",
                session_id="shared-session",
                dry_run=True,
                multitask_strategy="interrupt",
            )
        )

        assert second.status == "queued"
        assert second.multitask_strategy == "interrupt"

        await asyncio.sleep(0.05)
        first_final = runtime.get_background_status(first.job_id)
        second_final = runtime.get_background_status(second.job_id)

        assert first_final is not None
        assert first_final.status == "interrupted"
        assert first_cancelled.is_set()
        assert second_final is not None
        assert second_final.status == "completed"
        assert second_final.result is not None
        assert second_final.result.content == "replacement-job"

    asyncio.run(_run())


def test_runtime_background_batch_persists_parallel_group_summary(tmp_path: Path) -> None:
    """Batch enqueue should assign one durable group id plus lane ids."""

    settings = RuntimeSettings(
        codex_home=PROJECT_ROOT,
        data_dir=tmp_path / "runtime-data",
        live_model_override=False,
    )
    runtime = CodexAgnoRuntime(settings)

    async def fake_run_task(request: BackgroundRunRequest) -> RunTaskResponse:
        await asyncio.sleep(0.01)
        return RunTaskResponse(
            session_id=request.session_id or "batch-session",
            user_id=request.user_id or "tester",
            skill="test-skill",
            live_run=False,
            content=request.task,
            usage=UsageMetrics(),
        )

    runtime.run_task = fake_run_task  # type: ignore[method-assign]

    async def _run() -> None:
        batch = await runtime.enqueue_background_batch(
            [
                BackgroundRunRequest(task="lane-a", user_id="tester", session_id="batch-a", dry_run=True),
                BackgroundRunRequest(task="lane-b", user_id="tester", session_id="batch-b", dry_run=True),
            ]
        )

        assert batch.parallel_group_id.startswith("pgroup_")
        assert [status.parallel_group_id for status in batch.statuses] == [
            batch.parallel_group_id,
            batch.parallel_group_id,
        ]
        assert [status.lane_id for status in batch.statuses] == ["lane-1", "lane-2"]
        assert batch.summary.parallel_group_id == batch.parallel_group_id
        assert batch.summary.total_job_count == 2
        assert batch.summary.active_job_count == 2
        assert sorted(batch.summary.session_ids) == ["batch-a", "batch-b"]

        for _ in range(40):
            await asyncio.sleep(0.02)
            summary = runtime.get_background_parallel_group_summary(batch.parallel_group_id)
            if summary is not None and summary.terminal_job_count == 2:
                break

        summary = runtime.get_background_parallel_group_summary(batch.parallel_group_id)
        assert summary is not None
        assert summary.total_job_count == 2
        assert summary.terminal_job_count == 2
        assert summary.status_counts["completed"] == 2
        assert summary.active_job_count == 0
        assert summary.lane_ids == ["lane-1", "lane-2"]
        assert summary.job_ids == sorted(status.job_id for status in batch.statuses)
        listed = runtime.list_background_parallel_groups()
        assert len(listed) == 1
        assert listed[0].parallel_group_id == batch.parallel_group_id

    asyncio.run(_run())


def test_runtime_background_batch_rejects_misaligned_parallel_group_ids(tmp_path: Path) -> None:
    """One batch should not fan out across multiple durable group ids."""

    settings = RuntimeSettings(
        codex_home=PROJECT_ROOT,
        data_dir=tmp_path / "runtime-data",
        live_model_override=False,
    )
    runtime = CodexAgnoRuntime(settings)

    async def _run() -> None:
        with pytest.raises(ValueError, match="parallel_group_id"):
            await runtime.enqueue_background_batch(
                [
                    BackgroundRunRequest(
                        task="lane-a",
                        user_id="tester",
                        session_id="batch-a",
                        parallel_group_id="pgroup-a",
                        dry_run=True,
                    ),
                    BackgroundRunRequest(
                        task="lane-b",
                        user_id="tester",
                        session_id="batch-b",
                        parallel_group_id="pgroup-b",
                        dry_run=True,
                    ),
                ]
            )

    asyncio.run(_run())


def test_runtime_background_batch_enqueues_lanes_concurrently(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Parallel batch admission should not serialize independent lane enqueue operations."""

    settings = RuntimeSettings(
        codex_home=PROJECT_ROOT,
        data_dir=tmp_path / "runtime-data",
        live_model_override=False,
    )
    runtime = CodexAgnoRuntime(settings)
    host = runtime.background_service

    active_enqueues = 0
    max_active_enqueues = 0
    concurrency_lock = asyncio.Lock()
    captured_statuses: dict[str, BackgroundRunStatus] = {}

    async def fake_enqueue_job(
        request: BackgroundRunRequest,
        *,
        session_id_resolver,
        run_task,
    ) -> BackgroundRunStatus:
        nonlocal active_enqueues, max_active_enqueues
        async with concurrency_lock:
            active_enqueues += 1
            max_active_enqueues = max(max_active_enqueues, active_enqueues)
        await asyncio.sleep(0.05)
        resolved_session_id = session_id_resolver(request)
        status = BackgroundRunStatus(
            job_id=f"job-{request.lane_id}",
            session_id=resolved_session_id,
            status="queued",
            parallel_group_id=request.parallel_group_id,
            lane_id=request.lane_id,
            parent_job_id=request.parent_job_id,
        )
        captured_statuses[status.job_id] = status
        async with concurrency_lock:
            active_enqueues -= 1
        return status

    def fake_parallel_group_summary(parallel_group_id: str) -> BackgroundParallelGroupSummary:
        statuses = sorted(captured_statuses.values(), key=lambda item: item.job_id)
        return BackgroundParallelGroupSummary(
            parallel_group_id=parallel_group_id,
            job_ids=[status.job_id for status in statuses],
            session_ids=sorted(
                {
                    status.session_id
                    for status in statuses
                    if status.session_id is not None
                }
            ),
            lane_ids=sorted(
                {
                    status.lane_id
                    for status in statuses
                    if status.lane_id is not None
                }
            ),
            parent_job_ids=sorted(
                {
                    status.parent_job_id
                    for status in statuses
                    if status.parent_job_id is not None
                }
            ),
            status_counts={"queued": len(statuses)},
            active_job_count=len(statuses),
            terminal_job_count=0,
            total_job_count=len(statuses),
            latest_updated_at=max(
                (status.updated_at for status in statuses),
                default=None,
            ),
        )

    monkeypatch.setattr(host, "enqueue_job", fake_enqueue_job)
    monkeypatch.setattr(host, "parallel_group_summary", fake_parallel_group_summary)

    async def _run() -> None:
        batch = await host.enqueue_batch(
            [
                BackgroundRunRequest(task="lane-a", user_id="tester", session_id="batch-a", dry_run=True),
                BackgroundRunRequest(task="lane-b", user_id="tester", session_id="batch-b", dry_run=True),
            ],
            session_id_resolver=lambda request: request.session_id or "batch-session",
            run_task=runtime.run_task,
        )

        assert batch.parallel_group_id.startswith("pgroup_")
        assert [status.lane_id for status in batch.statuses] == ["lane-1", "lane-2"]
        assert max_active_enqueues == 2
        assert batch.summary.total_job_count == 2

    asyncio.run(_run())


def test_runtime_background_queue_rejects_unsupported_multitask_strategy(tmp_path: Path) -> None:
    """Unsupported multitask strategies should fail deterministically."""

    settings = RuntimeSettings(
        codex_home=PROJECT_ROOT,
        data_dir=tmp_path / "runtime-data",
        live_model_override=False,
    )
    runtime = CodexAgnoRuntime(settings)

    async def _run() -> None:
        status = await runtime.enqueue_background_run(
            BackgroundRunRequest(
                task="bad-strategy",
                user_id="tester",
                session_id="bad-strategy-session",
                dry_run=True,
                multitask_strategy="rollback",
            )
        )

        assert status.status == "failed"
        assert status.multitask_strategy == "rollback"
        assert "Unsupported multitask strategy" in (status.error or "")

    asyncio.run(_run())


def test_runtime_background_batch_persists_parallel_group_resume_summary(tmp_path: Path) -> None:
    """Parallel batch admission should persist one grouped resume/health summary."""

    trace_path = tmp_path / "TRACE_METADATA.json"
    settings = RuntimeSettings(
        codex_home=PROJECT_ROOT,
        data_dir=tmp_path / "runtime-data",
        trace_output_path=trace_path,
        live_model_override=False,
    )
    runtime = CodexAgnoRuntime(settings)

    async def _wait_for_completion(job_ids: list[str]) -> None:
        deadline = asyncio.get_running_loop().time() + 2.0
        while asyncio.get_running_loop().time() < deadline:
            statuses = [runtime.get_background_status(job_id) for job_id in job_ids]
            if all(status is not None and status.status == "completed" for status in statuses):
                return
            await asyncio.sleep(0.02)
        raise AssertionError(f"Timed out waiting for batch completion: {job_ids}")

    async def _run() -> None:
        batch = await runtime.enqueue_background_batch(
            [
                BackgroundRunRequest(
                    task="并行 lane 1",
                    user_id="tester",
                    session_id="parallel-session-1",
                    parent_job_id="parent-job",
                    dry_run=True,
                ),
                BackgroundRunRequest(
                    task="并行 lane 2",
                    user_id="tester",
                    session_id="parallel-session-2",
                    parent_job_id="parent-job",
                    dry_run=True,
                ),
            ],
            parallel_group_id="pgroup-contract",
        )

        assert batch.parallel_group_id == "pgroup-contract"
        assert [status.parallel_group_id for status in batch.statuses] == [
            "pgroup-contract",
            "pgroup-contract",
        ]
        assert [status.lane_id for status in batch.statuses] == ["lane-1", "lane-2"]
        assert [status.parent_job_id for status in batch.statuses] == ["parent-job", "parent-job"]
        assert batch.summary.parallel_group_id == "pgroup-contract"
        assert batch.summary.total_job_count == 2
        assert batch.summary.parent_job_ids == ["parent-job"]
        assert batch.summary.lane_ids == ["lane-1", "lane-2"]
        assert batch.summary.active_job_count >= 1

        await _wait_for_completion([status.job_id for status in batch.statuses])

        summary = runtime.get_background_parallel_group_summary("pgroup-contract")
        assert summary is not None
        assert summary.parallel_group_id == "pgroup-contract"
        assert summary.total_job_count == 2
        assert summary.terminal_job_count == 2
        assert summary.active_job_count == 0
        assert summary.parent_job_ids == ["parent-job"]
        assert summary.lane_ids == ["lane-1", "lane-2"]

        listed = runtime.list_background_parallel_groups()
        assert len(listed) == 1
        assert listed[0].parallel_group_id == "pgroup-contract"

        health = runtime.background_service.health()
        assert health["parallel_group_count"] == 1
        assert health["active_parallel_group_count"] == 0
        assert health["orchestration_contract"]["policy_schema_version"] == "router-rs-background-control-v1"
        assert health["orchestration_contract"]["terminal_statuses"] == [
            "completed",
            "failed",
            "interrupted",
            "retry_exhausted",
        ]
        assert health["orchestration_contract"]["active_statuses"] == [
            "queued",
            "running",
            "interrupt_requested",
            "retry_scheduled",
            "retry_claimed",
        ]
        assert health["orchestration_contract"]["policy_operations"] == [
            "batch-plan",
            "enqueue",
            "claim",
            "interrupt",
            "interrupt-finalize",
            "retry",
            "retry-claim",
            "complete",
            "completion-race",
            "session-release",
        ]

        resume_manifest = json.loads(trace_path.with_name("TRACE_RESUME_MANIFEST.json").read_text(encoding="utf-8"))
        assert resume_manifest["parallel_group"]["parallel_group_id"] == "pgroup-contract"
        assert resume_manifest["parallel_group"]["total_job_count"] == 2
        assert resume_manifest["parallel_group"]["terminal_job_count"] == 2
        assert resume_manifest["parallel_group"]["active_job_count"] == 0
        assert resume_manifest["parallel_group"]["parent_job_ids"] == ["parent-job"]
        assert resume_manifest["parallel_group"]["lane_ids"] == ["lane-1", "lane-2"]
        assert sorted(resume_manifest["parallel_group"]["session_ids"]) == [
            "parallel-session-1",
            "parallel-session-2",
        ]
        assert sorted(resume_manifest["parallel_group"]["job_ids"]) == sorted(
            status.job_id for status in batch.statuses
        )

        trace_metadata = json.loads(trace_path.read_text(encoding="utf-8"))
        assert trace_metadata["verification_status"] == "dry_run"
        assert trace_metadata["parallel_group"]["parallel_group_id"] == "pgroup-contract"
        assert trace_metadata["parallel_group"]["total_job_count"] == 2
        assert trace_metadata["parallel_group"]["terminal_job_count"] == 2
        assert trace_metadata["parallel_group"]["active_job_count"] == 0
        assert trace_metadata["parallel_group"]["parent_job_ids"] == ["parent-job"]
        assert trace_metadata["parallel_group"]["lane_ids"] == ["lane-1", "lane-2"]
        assert sorted(trace_metadata["parallel_group"]["session_ids"]) == [
            "parallel-session-1",
            "parallel-session-2",
        ]
        assert sorted(trace_metadata["parallel_group"]["job_ids"]) == sorted(
            status.job_id for status in batch.statuses
        )

    asyncio.run(_run())


def test_prepare_session_rust_mode_returns_rust_only_contract(tmp_path: Path) -> None:
    """The runtime should expose the Rust-only baseline contract in rust mode."""

    rust_runtime = CodexAgnoRuntime(
        RuntimeSettings(
            codex_home=PROJECT_ROOT,
            data_dir=tmp_path / "rust-runtime-data",
            live_model_override=False,
            route_engine_mode="rust",
        )
    )

    request = PrepareSessionRequest(
        task="帮我写一个 Rust CLI 工具",
        session_id="rust-runtime-route-session",
        user_id="tester",
    )
    rust_prepared = rust_runtime.prepare_session(request=request)

    assert rust_prepared.route_engine == "rust"
    assert rust_prepared.diagnostic_route_mode == "none"
    assert rust_prepared.route_diagnostic_report is None
    assert not hasattr(rust_prepared, "prompt_preview")
    assert rust_prepared.skill
    assert rust_prepared.layer


def test_prepare_session_shadow_mode_returns_soak_report(tmp_path: Path) -> None:
    """Shadow mode should keep Rust as the live route while returning a Rust-only diagnostic report."""

    runtime = CodexAgnoRuntime(
        RuntimeSettings(
            codex_home=PROJECT_ROOT,
            data_dir=tmp_path / "shadow-runtime-data",
            live_model_override=False,
            route_engine_mode="shadow",
        )
    )

    prepared = runtime.prepare_session(
        PrepareSessionRequest(
            task="帮我写一个 Rust CLI 工具",
            session_id="shadow-runtime-route-session",
            user_id="tester",
        )
    )

    assert prepared.route_engine == "rust"
    assert prepared.diagnostic_route_mode == "shadow"
    assert prepared.route_diagnostic_report is not None
    assert prepared.route_diagnostic_report.report_schema_version == "router-rs-route-report-v2"
    assert prepared.route_diagnostic_report.authority == "rust-route-core"
    assert prepared.route_diagnostic_report.mode == "shadow"
    assert prepared.route_diagnostic_report.primary_engine == "rust"
    assert prepared.route_diagnostic_report.evidence_kind == "rust-owned-snapshot"
    assert prepared.route_diagnostic_report.strict_verification is False
    assert prepared.route_diagnostic_report.verification_passed is True
    assert prepared.route_diagnostic_report.contract_mismatch_fields == []
    assert prepared.route_diagnostic_report.route_snapshot.selected_skill == prepared.skill


def test_prepare_session_preview_returns_rust_owned_prompt_text(tmp_path: Path) -> None:
    """Preview callers should use the explicit preview API instead of prepare_session."""

    runtime = CodexAgnoRuntime(
        RuntimeSettings(
            codex_home=PROJECT_ROOT,
            data_dir=tmp_path / "preview-runtime-data",
            live_model_override=False,
            route_engine_mode="rust",
        )
    )

    preview = runtime.prepare_session_preview(
        PrepareSessionRequest(
            task="帮我写一个 Rust CLI 工具",
            session_id="preview-runtime-route-session",
            user_id="tester",
        )
    )

    assert isinstance(preview, str)
    assert preview


def test_runtime_metadata_includes_route_diagnostic_report(tmp_path: Path) -> None:
    """Run-task metadata should surface Rust-only diagnostic evidence for shadow mode."""

    trace_path = tmp_path / "TRACE_METADATA.json"
    runtime = CodexAgnoRuntime(
        RuntimeSettings(
            codex_home=PROJECT_ROOT,
            data_dir=tmp_path / "shadow-runtime-data",
            trace_output_path=trace_path,
            live_model_override=False,
            route_engine_mode="shadow",
        )
    )

    async def _run() -> None:
        response = await runtime.run_task(
            RunTaskRequest(
                task="帮我写一个 Rust CLI 工具",
                user_id="tester",
                dry_run=True,
            )
        )
        assert response.metadata["route_engine_mode"] == "shadow"
        assert response.metadata["route_engine"] == "rust"
        assert response.metadata["diagnostic_route_mode"] == "shadow"
        report = response.metadata["route_diagnostic_report"]
        assert report is not None
        assert report["report_schema_version"] == "router-rs-route-report-v2"
        assert report["authority"] == "rust-route-core"
        assert report["primary_engine"] == "rust"
        assert report["evidence_kind"] == "rust-owned-snapshot"
        assert report["strict_verification"] is False
        assert report["verification_passed"] is True
        assert report["contract_mismatch_fields"] == []

    asyncio.run(_run())

    data = json.loads(trace_path.read_text(encoding="utf-8"))
    route_event = next(event for event in data["events"] if event["kind"] == "route.selected")
    assert route_event["payload"]["route_engine_mode"] == "shadow"
    assert route_event["payload"]["diagnostic_route_mode"] == "shadow"
    assert route_event["payload"]["routing_gate"] == "none"
    assert route_event["payload"]["routing_owner"]
    assert route_event["payload"]["route_diagnostic_report"]["report_schema_version"] == "router-rs-route-report-v2"
    assert route_event["payload"]["route_diagnostic_report"]["authority"] == "rust-route-core"
    assert route_event["payload"]["route_diagnostic_report"]["verification_passed"] is True
    assert route_event["payload"]["route_diagnostic_report"]["contract_mismatch_fields"] == []


def test_runtime_parallel_group_trace_metadata_updates_for_interrupted_terminal_state(tmp_path: Path) -> None:
    """Interrupted grouped background jobs should also project the top-level batch trace summary."""

    trace_path = tmp_path / "TRACE_METADATA.json"
    runtime = CodexAgnoRuntime(
        RuntimeSettings(
            codex_home=PROJECT_ROOT,
            data_dir=tmp_path / "interrupt-runtime-data",
            trace_output_path=trace_path,
            live_model_override=False,
        )
    )
    started = asyncio.Event()

    async def fake_execute(*, ctx, dry_run, trace_event_count, trace_output_path):  # type: ignore[no-untyped-def]
        started.set()
        await asyncio.sleep(10)
        return RunTaskResponse(
            session_id=ctx.session_id,
            user_id=ctx.user_id,
            skill=ctx.routing_result.selected_skill.name,
            overlay=ctx.routing_result.overlay_skill.name if ctx.routing_result.overlay_skill else None,
            live_run=not dry_run,
            content="should-not-complete",
            usage=UsageMetrics(),
        )

    runtime.execution_service.execute = fake_execute  # type: ignore[method-assign]

    async def _run() -> None:
        batch = await runtime.enqueue_background_batch(
            [
                BackgroundRunRequest(
                    task="请处理中断批次",
                    user_id="tester",
                    session_id="interrupt-session",
                    dry_run=True,
                )
            ],
            parallel_group_id="pgroup-interrupted",
        )
        await started.wait()
        interrupted = await runtime.request_background_interrupt(batch.statuses[0].job_id)

        deadline = asyncio.get_running_loop().time() + 2.0
        while asyncio.get_running_loop().time() < deadline:
            current = runtime.get_background_status(batch.statuses[0].job_id)
            if current is not None and current.status == "interrupted":
                interrupted = current
                break
            await asyncio.sleep(0.02)

        assert interrupted is not None
        assert interrupted.status == "interrupted"

        trace_metadata = json.loads(trace_path.read_text(encoding="utf-8"))
        assert trace_metadata["verification_status"] == "interrupted"
        assert trace_metadata["parallel_group"]["parallel_group_id"] == "pgroup-interrupted"
        assert trace_metadata["parallel_group"]["total_job_count"] == 1
        assert trace_metadata["parallel_group"]["terminal_job_count"] == 1
        assert trace_metadata["parallel_group"]["active_job_count"] == 0
        assert trace_metadata["parallel_group"]["status_counts"]["interrupted"] == 1

    asyncio.run(_run())


def test_runtime_parallel_group_trace_metadata_updates_for_retry_exhausted_terminal_state(tmp_path: Path) -> None:
    """Retry-exhausted grouped background jobs should project the top-level batch trace summary."""

    trace_path = tmp_path / "TRACE_METADATA.json"
    runtime = CodexAgnoRuntime(
        RuntimeSettings(
            codex_home=PROJECT_ROOT,
            data_dir=tmp_path / "retry-runtime-data",
            trace_output_path=trace_path,
            live_model_override=False,
        )
    )

    async def failing_execute(*, ctx, dry_run, trace_event_count, trace_output_path):  # type: ignore[no-untyped-def]
        raise RuntimeError(f"boom for {ctx.session_id}")

    runtime.execution_service.execute = failing_execute  # type: ignore[method-assign]

    async def _run() -> None:
        batch = await runtime.enqueue_background_batch(
            [
                BackgroundRunRequest(
                    task="请处理失败批次",
                    user_id="tester",
                    session_id="retry-session",
                    dry_run=True,
                    max_attempts=2,
                    backoff_base_seconds=0.01,
                )
            ],
            parallel_group_id="pgroup-retry-exhausted",
        )

        deadline = asyncio.get_running_loop().time() + 2.0
        exhausted = None
        while asyncio.get_running_loop().time() < deadline:
            current = runtime.get_background_status(batch.statuses[0].job_id)
            if current is not None and current.status == "retry_exhausted":
                exhausted = current
                break
            await asyncio.sleep(0.02)

        assert exhausted is not None
        assert exhausted.status == "retry_exhausted"

        trace_metadata = json.loads(trace_path.read_text(encoding="utf-8"))
        assert trace_metadata["verification_status"] == "retry_exhausted"
        assert trace_metadata["parallel_group"]["parallel_group_id"] == "pgroup-retry-exhausted"
        assert trace_metadata["parallel_group"]["total_job_count"] == 1
        assert trace_metadata["parallel_group"]["terminal_job_count"] == 1
        assert trace_metadata["parallel_group"]["active_job_count"] == 0
        assert trace_metadata["parallel_group"]["status_counts"]["retry_exhausted"] == 1

    asyncio.run(_run())


def test_runtime_parallel_group_trace_metadata_updates_for_pre_execution_failed_admission(tmp_path: Path) -> None:
    """Grouped pre-execution failures should also project the top-level batch trace summary."""

    trace_path = tmp_path / "TRACE_METADATA.json"
    runtime = CodexAgnoRuntime(
        RuntimeSettings(
            codex_home=PROJECT_ROOT,
            data_dir=tmp_path / "failed-admission-runtime-data",
            trace_output_path=trace_path,
            live_model_override=False,
        )
    )

    async def _run() -> None:
        batch = await runtime.enqueue_background_batch(
            [
                BackgroundRunRequest(
                    task="请处理预执行失败批次",
                    user_id="tester",
                    session_id="failed-admission-session",
                    dry_run=True,
                    multitask_strategy="rollback",
                )
            ],
            parallel_group_id="pgroup-failed-admission",
        )

        failed = batch.statuses[0]
        assert failed.status == "failed"
        assert failed.parallel_group_id == "pgroup-failed-admission"

        trace_metadata = json.loads(trace_path.read_text(encoding="utf-8"))
        assert trace_metadata["verification_status"] == "failed"
        assert trace_metadata["matched_skills"] == ["background-runtime-host"]
        assert trace_metadata["decision"]["owner"] == "background-runtime-host"
        assert trace_metadata["parallel_group"]["parallel_group_id"] == "pgroup-failed-admission"
        assert trace_metadata["parallel_group"]["total_job_count"] == 1
        assert trace_metadata["parallel_group"]["terminal_job_count"] == 1
        assert trace_metadata["parallel_group"]["active_job_count"] == 0
        assert trace_metadata["parallel_group"]["status_counts"]["failed"] == 1

    asyncio.run(_run())


def test_runtime_pre_execution_failed_admission_without_group_does_not_flush_top_level_trace(
    tmp_path: Path,
) -> None:
    """Standalone pre-execution failures should stay out of top-level trace projection."""

    trace_path = tmp_path / "TRACE_METADATA.json"
    runtime = CodexAgnoRuntime(
        RuntimeSettings(
            codex_home=PROJECT_ROOT,
            data_dir=tmp_path / "standalone-failed-admission-runtime-data",
            trace_output_path=trace_path,
            live_model_override=False,
        )
    )

    async def _run() -> None:
        status = await runtime.enqueue_background_run(
            BackgroundRunRequest(
                task="请处理单任务预执行失败",
                user_id="tester",
                session_id="standalone-failed-admission-session",
                dry_run=True,
                multitask_strategy="rollback",
            )
        )

        assert status.status == "failed"
        assert status.parallel_group_id is None
        assert trace_path.exists() is False

    asyncio.run(_run())
