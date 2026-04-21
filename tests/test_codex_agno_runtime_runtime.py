"""Regression tests for the local Codex Agno runtime skeleton."""

from __future__ import annotations

import asyncio
import json
import sys
from contextlib import contextmanager
from pathlib import Path

import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = PROJECT_ROOT / "codex_agno_runtime" / "src"
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

from codex_agno_runtime.config import RuntimeSettings
from codex_agno_runtime.runtime import CodexAgnoRuntime
from codex_agno_runtime.schemas import (
    BackgroundRunRequest,
    PrepareSessionRequest,
    RunTaskRequest,
    RunTaskResponse,
    UsageMetrics,
)
from codex_agno_runtime.trace import (
    TRACE_EVENT_SCHEMA_VERSION,
    TRACE_EVENT_SINK_SCHEMA_VERSION,
    TRACE_METADATA_SCHEMA_VERSION,
    TRACE_EVENT_TRANSPORT_SCHEMA_VERSION,
    TRACE_REPLAY_CURSOR_SCHEMA_VERSION,
    TRACE_RESUME_MANIFEST_SCHEMA_VERSION,
)
from codex_agno_runtime.skill_loader import SkillLoader
from codex_agno_runtime.state import (
    BACKGROUND_STATE_SCHEMA_VERSION,
    BACKGROUND_SESSION_TAKEOVER_ARBITRATION_SCHEMA_VERSION,
    BackgroundJobStore,
    BackgroundJobStatusMutation,
    SessionConflictError,
)


_MINIMAL_SUPERVISOR_STATE = {
    "version": 1,
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
        assert health["rustification"]["python_host_role"] == "thin-projection"
        assert health["rustification"]["rustification_status"]["runtime_primary_owner"] == "rust-control-plane"
        assert health["rustification"]["rust_owned_service_count"] >= 8

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
            assert response.metadata["execution_kernel_live_fallback"] is None
            assert response.metadata["execution_kernel_live_fallback_authority"] is None
            assert response.metadata["execution_kernel_live_fallback_mode"] == "disabled"
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


def test_runtime_supervisor_projection_summarizes_top_level_verification_status_dict(tmp_path: Path) -> None:
    """Top-level verification dictionaries should reduce to a stable overall verdict."""

    supervisor_state = {
        "active_phase": "background-policy-rustified-still-not-full-done",
        "delegated_sidecars": [{"nickname": "Euler"}, {"nickname": "Gibbs"}],
        "verification_status": {
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

    async def fake_execute(*, ctx, dry_run: bool, trace_event_count: int, trace_output_path: str | None):
        seen["prompt"] = ctx.prompt
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
            prompt_preview=ctx.prompt,
            metadata={
                "execution_kernel": "fake-kernel",
                "execution_kernel_authority": "test-adapter",
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
        assert isinstance(seen["prompt"], str)
        assert seen["prompt"]
        assert seen["trace_event_count"] >= 4
        assert seen["trace_output_path"] == str(trace_path)
        assert response.metadata["execution_kernel"] == "fake-kernel"
        assert response.metadata["execution_kernel_authority"] == "test-adapter"
        assert response.metadata["execution_kernel_delegate"] == "router-rs"
        assert response.metadata["execution_kernel_delegate_authority"] == "rust-execution-cli"
        assert response.metadata["execution_kernel_delegate_family"] == "rust-cli"
        assert response.metadata["execution_kernel_delegate_impl"] == "router-rs"
        assert response.metadata["execution_kernel_live_primary"] == "router-rs"
        assert response.metadata["execution_kernel_live_primary_authority"] == "rust-execution-cli"
        assert "execution_kernel_live_fallback" not in response.metadata
        assert "execution_kernel_live_fallback_authority" not in response.metadata
        assert response.metadata["execution_kernel_live_fallback_mode"] == "disabled"
        assert response.metadata["trace_event_schema_version"] == TRACE_EVENT_SCHEMA_VERSION

    asyncio.run(_run())


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
                "execution_kernel_delegate": "router-rs",
                "execution_kernel_delegate_authority": "rust-execution-cli",
                "execution_kernel_live_primary": "router-rs",
                "execution_kernel_live_primary_authority": "rust-execution-cli",
                "execution_kernel_live_fallback": None,
                "execution_kernel_live_fallback_authority": None,
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
        assert response.metadata["execution_kernel_live_fallback_mode"] == "disabled"

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
            rust_execute_fallback_to_python=False,
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
        assert response.metadata["execution_kernel_live_fallback_enabled"] is False
        assert response.metadata["execution_kernel_live_fallback"] is None
        assert response.metadata["execution_kernel_live_fallback_authority"] is None
        assert response.metadata["execution_kernel_live_fallback_mode"] == "disabled"

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
            assert attached["transport"]["stream_id"] == transport["stream_id"]
            assert attached["binding_artifact_path"] == transport["binding_artifact_path"]
            assert attached["trace_stream_path"].endswith("TRACE_EVENTS.jsonl")
            assert attached["cleanup_semantics"] == "no_persisted_state"
            assert attached["cleanup_preserves_replay"] is True
            resumed_via_binding = attached_runtime.subscribe_attached_runtime_events(
                binding_artifact_path=transport["binding_artifact_path"],
                after_event_id=first_window["events"][-1]["event_id"],
                limit=20,
            )
            assert resumed_via_binding["events"]
            assert resumed_via_binding["after_event_id"] == first_window["events"][-1]["event_id"]

            handoff = runtime.describe_runtime_event_handoff(session_id=response.session_id)
            attached_via_manifest = attached_runtime.attach_runtime_event_transport(
                resume_manifest_path=handoff["resume_manifest_path"]
            )
            assert attached_via_manifest["handoff"] is None
            assert attached_via_manifest["resume_manifest"]["session_id"] == response.session_id
            assert attached_via_manifest["binding_artifact_path"] == transport["binding_artifact_path"]
            resumed_via_manifest = attached_runtime.subscribe_attached_runtime_events(
                resume_manifest_path=handoff["resume_manifest_path"],
                after_event_id=first_window["events"][-1]["event_id"],
                limit=20,
            )
            assert resumed_via_manifest["events"]
            assert resumed_via_manifest["after_event_id"] == first_window["events"][-1]["event_id"]
            manifest_cleanup = attached_runtime.cleanup_attached_runtime_event_transport(
                resume_manifest_path=handoff["resume_manifest_path"]
            )
            assert manifest_cleanup["cleanup_semantics"] == "no_persisted_state"
            assert manifest_cleanup["cleanup_preserves_replay"] is True

            handoff_path = Path(transport["binding_artifact_path"]).with_name("ATTACHED_RUNTIME_EVENT_HANDOFF.json")
            handoff_path.write_text(json.dumps(handoff, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
            attached_via_handoff = attached_runtime.attach_runtime_event_transport(handoff_path=str(handoff_path))
            assert attached_via_handoff["handoff"]["stream_id"] == handoff["stream_id"]
            assert attached_via_handoff["resume_manifest"]["session_id"] == response.session_id
            idle_via_handoff = attached_runtime.subscribe_attached_runtime_events(
                handoff_path=str(handoff_path),
                after_event_id=transport["latest_cursor"]["event_id"],
                heartbeat=True,
            )
            assert idle_via_handoff["events"] == []
            assert idle_via_handoff["heartbeat"]["status"] == "idle"
            attached_cleanup = attached_runtime.cleanup_attached_runtime_event_transport(handoff_path=str(handoff_path))
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
            assert attached["transport"]["binding_backend_family"] == "sqlite"
            assert attached["resume_manifest"]["session_id"] == response.session_id

            replay = attached_runtime.subscribe_attached_runtime_events(
                binding_artifact_path=transport["binding_artifact_path"],
                limit=20,
            )
            assert replay["events"]
            assert replay["events"][0]["session_id"] == response.session_id

            attached_via_manifest = attached_runtime.attach_runtime_event_transport(
                resume_manifest_path=handoff["resume_manifest_path"]
            )
            assert attached_via_manifest["artifact_backend_family"] == "sqlite"
            assert attached_via_manifest["binding_artifact_path"] == transport["binding_artifact_path"]

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


def test_prepare_session_rust_route_mode_matches_python_mode(tmp_path: Path) -> None:
    """The runtime should expose Rust route picking behind an explicit mode flag."""

    python_runtime = CodexAgnoRuntime(
        RuntimeSettings(
            codex_home=PROJECT_ROOT,
            data_dir=tmp_path / "python-runtime-data",
            live_model_override=False,
            route_engine_mode="python",
        )
    )
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
    python_prepared = python_runtime.prepare_session(request=request)
    rust_prepared = rust_runtime.prepare_session(request=request)

    assert rust_prepared.skill == python_prepared.skill
    assert rust_prepared.overlay == python_prepared.overlay
    assert rust_prepared.layer == python_prepared.layer
    assert rust_prepared.route_engine == "rust"


def test_prepare_session_shadow_mode_returns_soak_report(tmp_path: Path) -> None:
    """Shadow mode should keep Rust as the live route while returning a stable parity report."""

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
    assert prepared.rollback_to_python is False
    assert prepared.shadow_route_report is not None
    assert prepared.shadow_route_report.report_schema_version == "router-rs-route-report-v1"
    assert prepared.shadow_route_report.authority == "rust-route-core"
    assert prepared.shadow_route_report.mode == "shadow"
    assert prepared.shadow_route_report.selected_skill_match is True
    assert prepared.shadow_route_report.overlay_skill_match is True
    assert prepared.shadow_route_report.layer_match is True
    assert prepared.shadow_route_report.primary_engine == "rust"
    assert prepared.shadow_route_report.shadow_engine == "python"


def test_runtime_metadata_includes_shadow_route_report(tmp_path: Path) -> None:
    """Run-task metadata should surface shadow soak evidence for real-task replay queries."""

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
        assert response.metadata["rollback_to_python"] is False
        report = response.metadata["shadow_route_report"]
        assert report is not None
        assert report["report_schema_version"] == "router-rs-route-report-v1"
        assert report["authority"] == "rust-route-core"
        assert report["selected_skill_match"] is True
        assert report["overlay_skill_match"] is True
        assert report["layer_match"] is True
        assert report["primary_engine"] == "rust"
        assert report["shadow_engine"] == "python"

    asyncio.run(_run())

    data = json.loads(trace_path.read_text(encoding="utf-8"))
    route_event = next(event for event in data["events"] if event["kind"] == "route.selected")
    assert route_event["payload"]["route_engine_mode"] == "shadow"
    assert route_event["payload"]["shadow_route_report"]["report_schema_version"] == "router-rs-route-report-v1"
    assert route_event["payload"]["shadow_route_report"]["authority"] == "rust-route-core"
    assert route_event["payload"]["shadow_route_report"]["selected_skill_match"] is True
