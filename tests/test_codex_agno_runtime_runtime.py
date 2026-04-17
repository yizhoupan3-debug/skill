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
    BackgroundJobStore,
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
def _project_supervisor_state() -> Path:
    path = PROJECT_ROOT / ".supervisor_state.json"
    original = path.read_text(encoding="utf-8") if path.exists() else None
    path.write_text(json.dumps(_MINIMAL_SUPERVISOR_STATE, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
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
    assert target.when_to_use
    loader.load_body(target)
    assert target.body_loaded is True
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

            transport = runtime.describe_runtime_event_transport(session_id=response.session_id)
            assert transport["schema_version"] == TRACE_EVENT_TRANSPORT_SCHEMA_VERSION
            assert transport["session_id"] == response.session_id
            assert transport["transport_kind"] == "poll"
            assert transport["transport_family"] == "host-facing-bridge"
            assert transport["endpoint_kind"] == "runtime_method"
            assert transport["remote_capable"] is True
            assert transport["handoff_supported"] is True
            assert transport["handoff_method"] == "describe_runtime_event_handoff"
            assert transport["handoff_kind"] == "artifact_handoff"
            assert transport["binding_refresh_mode"] == "describe_or_checkpoint"
            assert transport["binding_artifact_format"] == "json"
            assert transport["binding_backend_family"] == "filesystem"
            assert transport["binding_artifact_path"].endswith(
                f"runtime_event_transports/{response.session_id}__{response.session_id}.json"
            )
            assert transport["describe_method"] == "describe_runtime_event_transport"
            assert transport["subscribe_method"] == "subscribe_runtime_events"
            assert transport["cleanup_method"] == "cleanup_runtime_events"
            assert transport["cleanup_semantics"] == "bridge_cache_only"
            assert transport["cleanup_preserves_replay"] is True
            assert transport["replay_reseed_supported"] is True
            assert transport["latest_cursor"]["schema_version"] == TRACE_REPLAY_CURSOR_SCHEMA_VERSION
            persisted = json.loads(Path(transport["binding_artifact_path"]).read_text(encoding="utf-8"))
            assert persisted["stream_id"] == transport["stream_id"]
            assert persisted["session_id"] == response.session_id
            assert persisted["latest_cursor"]["schema_version"] == TRACE_REPLAY_CURSOR_SCHEMA_VERSION

            handoff = runtime.describe_runtime_event_handoff(session_id=response.session_id)
            assert handoff["schema_version"] == "runtime-event-handoff-v1"
            assert handoff["stream_id"] == transport["stream_id"]
            assert handoff["checkpoint_backend_family"] == "filesystem"
            assert handoff["trace_stream_path"].endswith("TRACE_EVENTS.jsonl")
            assert handoff["resume_manifest_path"].endswith("TRACE_RESUME_MANIFEST.json")
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


def test_background_job_store_serializes_interrupt_takeovers() -> None:
    """Only one replacement job may reserve the next session handoff at a time."""

    store = BackgroundJobStore()
    store.set_status("job-1", status="queued", session_id="shared-session", timeout_seconds=30)

    assert store.reserve_session_takeover(session_id="shared-session", incoming_job_id="job-2") == "job-1"

    with pytest.raises(SessionConflictError):
        store.reserve_session_takeover(session_id="shared-session", incoming_job_id="job-3")

    store.set_status("job-1", status="interrupted", session_id="shared-session", timeout_seconds=30)
    store.claim_session_takeover(session_id="shared-session", incoming_job_id="job-2")
    queued = store.set_status("job-2", status="queued", session_id="shared-session", timeout_seconds=30)

    assert queued.status == "queued"
    assert store.get_active_job("shared-session") == "job-2"
    assert store.pending_session_takeovers() == 0


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

        await asyncio.sleep(0.05)
        status = runtime.get_background_status(first.job_id)
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
    """Shadow mode should preserve the Python route while returning a stable parity report."""

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

    assert prepared.route_engine == "python"
    assert prepared.rollback_to_python is False
    assert prepared.shadow_route_report is not None
    assert prepared.shadow_route_report.mode == "shadow"
    assert prepared.shadow_route_report.selected_skill_match is True
    assert prepared.shadow_route_report.overlay_skill_match is True
    assert prepared.shadow_route_report.layer_match is True


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
        assert response.metadata["route_engine"] == "python"
        assert response.metadata["rollback_to_python"] is False
        report = response.metadata["shadow_route_report"]
        assert report is not None
        assert report["selected_skill_match"] is True
        assert report["overlay_skill_match"] is True
        assert report["layer_match"] is True
        assert report["primary_engine"] == "python"
        assert report["shadow_engine"] == "rust"

    asyncio.run(_run())

    data = json.loads(trace_path.read_text(encoding="utf-8"))
    route_event = next(event for event in data["events"] if event["kind"] == "route.selected")
    assert route_event["payload"]["route_engine_mode"] == "shadow"
    assert route_event["payload"]["shadow_route_report"]["selected_skill_match"] is True
