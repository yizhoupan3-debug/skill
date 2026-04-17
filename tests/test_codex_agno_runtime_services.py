"""Focused tests for runtime service seams and route-engine boundaries."""

from __future__ import annotations

import asyncio
import json
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = PROJECT_ROOT / "codex_agno_runtime" / "src"
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

from codex_agno_runtime.checkpoint_store import FilesystemRuntimeCheckpointer
from codex_agno_runtime.config import RuntimeSettings
from codex_agno_runtime.middleware import MiddlewareContext
from codex_agno_runtime.schemas import RunTaskResponse, UsageMetrics
from codex_agno_runtime.services import (
    ExecutionEnvironmentService,
    MemoryService,
    RouterService,
    StateService,
    TraceService,
)


def test_runtime_services_expose_health_boundaries(tmp_path: Path) -> None:
    """Each extracted service seam should advertise startup/health boundaries."""

    settings = RuntimeSettings(
        codex_home=PROJECT_ROOT,
        data_dir=tmp_path / "runtime-data",
        trace_output_path=tmp_path / "TRACE_METADATA.json",
        live_model_override=False,
        route_engine_mode="rust",
    )
    checkpointer = FilesystemRuntimeCheckpointer(
        data_dir=settings.resolved_data_dir,
        trace_output_path=settings.resolved_trace_output_path,
    )
    router_service = RouterService(settings)
    state_service = StateService(checkpointer)
    trace_service = TraceService(checkpointer)
    memory_service = MemoryService(settings)
    execution_service = ExecutionEnvironmentService(
        settings,
        router_service.prompt_builder,
        max_background_jobs=4,
        background_job_timeout_seconds=30.0,
    )

    for service in (router_service, state_service, trace_service, memory_service, execution_service):
        service.startup()

    assert router_service.health()["loaded_skill_count"] > 0
    assert router_service.health()["primary_authority"] == "rust"
    assert router_service.health()["route_result_engine"] == "rust"
    assert router_service.health()["shadow_engine"] is None
    assert router_service.health()["python_router_loaded"] is False
    assert router_service.health()["python_router_required"] is False
    assert router_service.health()["route_policy"]["policy_schema_version"] == "router-rs-route-policy-v1"
    assert router_service.health()["rust_adapter"]["route_authority"] == "rust-route-core"
    assert router_service.health()["rust_adapter"]["compile_authority"] == "rust-route-compiler"
    assert (
        router_service.health()["rust_adapter"]["route_policy_schema_version"]
        == "router-rs-route-policy-v1"
    )
    assert (
        router_service.health()["rust_adapter"]["route_snapshot_schema_version"]
        == "router-rs-route-snapshot-v1"
    )
    assert state_service.health()["state_path"].endswith("runtime_background_jobs.json")
    assert state_service.health()["pending_session_takeovers"] == 0
    assert trace_service.health()["checkpoint_backend_family"] == "filesystem"
    assert trace_service.health()["trace_output_path"].endswith("TRACE_METADATA.json")
    assert trace_service.health()["event_stream_path"].endswith("TRACE_EVENTS.jsonl")
    assert trace_service.health()["resume_manifest_path"].endswith("TRACE_RESUME_MANIFEST.json")
    assert trace_service.health()["event_transport_dir"].endswith("runtime_event_transports")
    assert trace_service.health()["background_state_path"].endswith("runtime_background_jobs.json")
    assert trace_service.health()["replay_supported"] is True
    assert trace_service.health()["event_bridge_supported"] is True
    assert trace_service.health()["event_bridge_schema_version"] == "runtime-event-bridge-v1"
    assert memory_service.health()["memory_dir"].endswith("codex_agno_runtime/data/memory")
    assert execution_service.health()["background_job_timeout_seconds"] == 30.0
    assert execution_service.health()["execution_mode_default"] == "dry_run"
    assert execution_service.health()["kernel_adapter_kind"] == "rust-execution-kernel-slice"
    assert execution_service.health()["kernel_authority"] == "rust-execution-kernel-authority"
    assert execution_service.health()["kernel_owner_family"] == "rust"
    assert execution_service.health()["kernel_owner_impl"] == "execution-kernel-slice"
    assert execution_service.health()["kernel_contract_mode"] == "rust-live-primary"
    assert execution_service.health()["kernel_replace_ready"] is True
    assert execution_service.health()["kernel_in_process_replacement_complete"] is False
    assert execution_service.health()["kernel_live_backend_family"] == "rust-cli"
    assert execution_service.health()["kernel_live_backend_impl"] == "router-rs"
    assert execution_service.health()["kernel_live_delegate_kind"] == "router-rs"
    assert execution_service.health()["kernel_live_delegate_authority"] == "rust-execution-cli"
    assert execution_service.health()["kernel_live_delegate_family"] == "rust-cli"
    assert execution_service.health()["kernel_live_delegate_impl"] == "router-rs"
    assert execution_service.health()["kernel_live_fallback_kind"] == "python-agno"
    assert execution_service.health()["kernel_live_fallback_authority"] == "python-agno-kernel-adapter"
    assert execution_service.health()["kernel_live_fallback_family"] == "python"
    assert execution_service.health()["kernel_live_fallback_impl"] == "agno"
    assert execution_service.health()["kernel_live_fallback_mode"] == "compatibility"
    assert execution_service.health()["kernel_mode_support"] == ["dry_run", "live"]
    assert execution_service.health()["control_plane_contracts"]["execution_controller_contract"]["controller"][
        "primary_owner"
    ] == "execution-controller-coding"
    assert execution_service.health()["control_plane_contracts"]["delegation_contract"]["gate"][
        "gate_skill"
    ] == "subagent-delegation"
    assert execution_service.health()["control_plane_contracts"]["supervisor_state_contract"][
        "state_artifact_path"
    ] == ".supervisor_state.json"

    for service in (execution_service, memory_service, trace_service, state_service, router_service):
        service.shutdown()


def test_execution_environment_service_routes_through_kernel_adapter(tmp_path: Path) -> None:
    """Execution service should expose the single kernel entry for runtime execution."""

    settings = RuntimeSettings(
        codex_home=PROJECT_ROOT,
        data_dir=tmp_path / "runtime-data",
        trace_output_path=tmp_path / "TRACE_METADATA.json",
        live_model_override=False,
    )
    router_service = RouterService(settings)
    execution_service = ExecutionEnvironmentService(
        settings,
        router_service.prompt_builder,
        max_background_jobs=4,
        background_job_timeout_seconds=30.0,
    )
    routing_result = router_service.route(
        task="帮我写一个 Rust CLI 工具",
        session_id="kernel-service-session",
        allow_overlay=True,
        first_turn=True,
    )
    ctx = MiddlewareContext(
        task=routing_result.task,
        session_id=routing_result.session_id,
        user_id="tester",
        routing_result=routing_result,
        prompt="kernel-prompt",
    )

    async def _run() -> None:
        response = await execution_service.execute(
            ctx=ctx,
            dry_run=True,
            trace_event_count=7,
            trace_output_path="/tmp/TRACE_METADATA.json",
        )
        assert isinstance(response, RunTaskResponse)
        assert response.live_run is False
        assert response.prompt_preview == "kernel-prompt"
        assert response.metadata["execution_kernel"] == "rust-execution-kernel-slice"
        assert response.metadata["execution_kernel_authority"] == "rust-execution-kernel-authority"
        assert response.metadata["execution_kernel_contract_mode"] == "rust-live-primary"
        assert response.metadata["execution_kernel_in_process_replacement_complete"] is False
        assert response.metadata["execution_kernel_delegate"] == "python-agno"
        assert response.metadata["execution_kernel_delegate_authority"] == "python-agno-kernel-adapter"
        assert response.metadata["execution_kernel_delegate_family"] == "python"
        assert response.metadata["execution_kernel_delegate_impl"] == "agno"
        assert response.metadata["execution_kernel_live_primary"] == "router-rs"
        assert response.metadata["execution_kernel_live_primary_authority"] == "rust-execution-cli"
        assert response.metadata["execution_kernel_live_fallback"] == "python-agno"
        assert response.metadata["execution_kernel_live_fallback_authority"] == "python-agno-kernel-adapter"
        assert response.metadata["execution_kernel_live_fallback_mode"] == "compatibility"
        assert "execution_kernel_fallback_reason" not in response.metadata
        assert response.metadata["trace_event_count"] == 7

    asyncio.run(_run())


def test_execution_environment_service_live_mode_omits_python_prompt_preview(tmp_path: Path) -> None:
    """Live execution should not pass Python prompt text through the kernel seam."""

    settings = RuntimeSettings(
        codex_home=PROJECT_ROOT,
        data_dir=tmp_path / "runtime-data",
        trace_output_path=tmp_path / "TRACE_METADATA.json",
        live_model_override=True,
    )
    router_service = RouterService(settings)
    execution_service = ExecutionEnvironmentService(
        settings,
        router_service.prompt_builder,
        max_background_jobs=4,
        background_job_timeout_seconds=30.0,
    )
    routing_result = router_service.route(
        task="帮我写一个 Rust CLI 工具",
        session_id="kernel-service-live-session",
        allow_overlay=True,
        first_turn=True,
    )
    ctx = MiddlewareContext(
        task=routing_result.task,
        session_id=routing_result.session_id,
        user_id="tester",
        routing_result=routing_result,
        prompt="legacy-python-prompt",
    )
    seen: dict[str, object] = {}

    async def fake_execute(request):
        seen["prompt_preview"] = request.prompt_preview
        seen["dry_run"] = request.dry_run
        return RunTaskResponse(
            session_id=request.session_id,
            user_id=request.user_id,
            skill=request.routing_result.selected_skill.name,
            overlay=request.routing_result.overlay_skill.name if request.routing_result.overlay_skill else None,
            live_run=True,
            content="live result",
            prompt_preview=None,
            model_id="gpt-5.4",
            usage=UsageMetrics(input_tokens=5, output_tokens=3, total_tokens=8, mode="live"),
            metadata={
                "execution_kernel": "rust-execution-kernel-slice",
                "execution_kernel_authority": "rust-execution-kernel-authority",
            },
        )

    execution_service.kernel.execute = fake_execute  # type: ignore[method-assign]

    async def _run() -> None:
        response = await execution_service.execute(
            ctx=ctx,
            dry_run=False,
            trace_event_count=5,
            trace_output_path="/tmp/TRACE_METADATA.json",
        )
        assert seen["dry_run"] is False
        assert seen["prompt_preview"] is None
        assert response.live_run is True
        assert response.prompt_preview is None
        assert response.model_id == "gpt-5.4"

    asyncio.run(_run())


def test_execution_service_can_disable_python_live_fallback(tmp_path: Path) -> None:
    """Health contracts should expose Rust-only live mode without breaking dry-run."""

    settings = RuntimeSettings(
        codex_home=PROJECT_ROOT,
        data_dir=tmp_path / "runtime-data",
        trace_output_path=tmp_path / "TRACE_METADATA.json",
        live_model_override=False,
        rust_execute_fallback_to_python=False,
    )
    router_service = RouterService(settings)
    execution_service = ExecutionEnvironmentService(
        settings,
        router_service.prompt_builder,
        max_background_jobs=4,
        background_job_timeout_seconds=30.0,
    )

    health = execution_service.health()
    contract = execution_service.describe_kernel_contract(dry_run=False)
    dry_run_contract = execution_service.describe_kernel_contract(dry_run=True)

    assert health["kernel_contract_mode"] == "rust-live-primary"
    assert health["kernel_live_fallback_enabled"] is False
    assert health["kernel_live_fallback_kind"] is None
    assert health["kernel_live_fallback_authority"] is None
    assert health["kernel_live_fallback_family"] is None
    assert health["kernel_live_fallback_impl"] is None
    assert health["kernel_live_fallback_mode"] == "disabled"
    assert contract["execution_kernel_live_fallback_enabled"] is False
    assert contract["execution_kernel_live_fallback"] is None
    assert contract["execution_kernel_live_fallback_authority"] is None
    assert contract["execution_kernel_live_fallback_mode"] == "disabled"
    assert contract["execution_kernel_delegate_family"] == "rust-cli"
    assert contract["execution_kernel_delegate_impl"] == "router-rs"
    assert dry_run_contract["execution_kernel_delegate"] == "python-agno"
    assert dry_run_contract["execution_kernel_delegate_authority"] == "python-agno-kernel-adapter"
    assert dry_run_contract["execution_kernel_delegate_family"] == "python"
    assert dry_run_contract["execution_kernel_delegate_impl"] == "agno"
    assert dry_run_contract["execution_kernel_live_fallback_enabled"] is False
    assert dry_run_contract["execution_kernel_live_fallback"] is None
    assert dry_run_contract["execution_kernel_live_fallback_authority"] is None
    assert dry_run_contract["execution_kernel_live_fallback_mode"] == "disabled"

    routing_result = router_service.route(
        task="帮我写一个 Rust CLI 工具",
        session_id="kernel-service-rust-only-session",
        allow_overlay=True,
        first_turn=True,
    )
    ctx = MiddlewareContext(
        task=routing_result.task,
        session_id=routing_result.session_id,
        user_id="tester",
        routing_result=routing_result,
        prompt="kernel-prompt",
    )

    async def _run() -> None:
        response = await execution_service.execute(
            ctx=ctx,
            dry_run=True,
            trace_event_count=7,
            trace_output_path="/tmp/TRACE_METADATA.json",
        )
        assert isinstance(response, RunTaskResponse)
        assert response.live_run is False
        assert response.metadata["execution_kernel"] == "rust-execution-kernel-slice"
        assert response.metadata["execution_kernel_authority"] == "rust-execution-kernel-authority"
        assert response.metadata["execution_kernel_contract_mode"] == "rust-live-primary"
        assert response.metadata["execution_kernel_delegate"] == "python-agno"
        assert response.metadata["execution_kernel_delegate_authority"] == "python-agno-kernel-adapter"
        assert response.metadata["execution_kernel_delegate_family"] == "python"
        assert response.metadata["execution_kernel_delegate_impl"] == "agno"
        assert response.metadata["execution_kernel_live_primary"] == "router-rs"
        assert response.metadata["execution_kernel_live_primary_authority"] == "rust-execution-cli"
        assert response.metadata["execution_kernel_live_fallback"] is None
        assert response.metadata["execution_kernel_live_fallback_authority"] is None
        assert response.metadata["execution_kernel_live_fallback_enabled"] is False
        assert response.metadata["execution_kernel_live_fallback_mode"] == "disabled"
        assert "execution_kernel_fallback_reason" not in response.metadata

    asyncio.run(_run())


def test_execution_service_kernel_payload_prefers_explicit_metadata(tmp_path: Path) -> None:
    """Kernel payload projection should preserve explicit runtime overrides."""

    settings = RuntimeSettings(
        codex_home=PROJECT_ROOT,
        data_dir=tmp_path / "runtime-data",
        trace_output_path=tmp_path / "TRACE_METADATA.json",
        live_model_override=True,
        rust_execute_fallback_to_python=False,
    )
    router_service = RouterService(settings)
    execution_service = ExecutionEnvironmentService(
        settings,
        router_service.prompt_builder,
        max_background_jobs=4,
        background_job_timeout_seconds=30.0,
    )

    payload = execution_service.kernel_payload(
        dry_run=False,
        metadata={
            "execution_kernel_authority": "custom-runtime-authority",
            "execution_kernel_delegate_family": "custom-live-family",
            "execution_kernel_delegate_impl": "custom-live-impl",
            "execution_kernel_live_fallback": None,
            "execution_kernel_live_fallback_authority": None,
        },
    )

    assert payload["execution_kernel"] == "rust-execution-kernel-slice"
    assert payload["execution_kernel_authority"] == "custom-runtime-authority"
    assert payload["execution_kernel_delegate"] == "router-rs"
    assert payload["execution_kernel_delegate_family"] == "custom-live-family"
    assert payload["execution_kernel_delegate_impl"] == "custom-live-impl"
    assert payload["execution_kernel_live_primary"] == "router-rs"
    assert payload["execution_kernel_live_fallback_enabled"] is False
    assert payload["execution_kernel_live_fallback_mode"] == "disabled"
    assert "execution_kernel_live_fallback" not in payload
    assert "execution_kernel_live_fallback_authority" not in payload


def test_execution_environment_service_exposes_control_plane_contract_descriptors(tmp_path: Path) -> None:
    """Execution service should expose control-plane-only contract descriptors."""

    settings = RuntimeSettings(
        codex_home=PROJECT_ROOT,
        data_dir=tmp_path / "runtime-data",
        trace_output_path=tmp_path / "TRACE_METADATA.json",
        live_model_override=False,
    )
    router_service = RouterService(settings)
    execution_service = ExecutionEnvironmentService(
        settings,
        router_service.prompt_builder,
        max_background_jobs=2,
        background_job_timeout_seconds=15.0,
    )

    descriptors = execution_service.describe_control_plane_contracts()

    assert descriptors["execution_controller_contract"]["status_contract"] == (
        "execution_controller_contract_v1"
    )
    assert descriptors["execution_controller_contract"]["controller"]["primary_owner"] == (
        "execution-controller-coding"
    )
    assert descriptors["execution_controller_contract"]["boundaries"][
        "runtime_branching_changes_required"
    ] is False
    assert descriptors["delegation_contract"]["status_contract"] == "delegation_contract_v1"
    assert descriptors["delegation_contract"]["gate"]["gate_skill"] == "subagent-delegation"
    assert descriptors["delegation_contract"]["local_supervisor_mode"][
        "allowed_when_runtime_blocks_spawning"
    ] is True
    assert descriptors["supervisor_state_contract"]["status_contract"] == (
        "supervisor_state_contract_v1"
    )
    assert descriptors["supervisor_state_contract"]["state_artifact_path"] == ".supervisor_state.json"
    assert descriptors["supervisor_state_contract"]["compatibility_rules"][
        "no_shadow_replacement_artifact"
    ] is True


    """Verify mode should compare Python and Rust route decisions at runtime."""

    python_service = RouterService(
        RuntimeSettings(
            codex_home=PROJECT_ROOT,
            live_model_override=False,
            route_engine_mode="python",
        )
    )
    verify_service = RouterService(
        RuntimeSettings(
            codex_home=PROJECT_ROOT,
            live_model_override=False,
            route_engine_mode="verify",
        )
    )

    python_result = python_service.route(
        task="帮我写一个 Rust CLI 工具",
        session_id="verify-route-session",
        allow_overlay=True,
        first_turn=True,
    )
    verify_result = verify_service.route(
        task="帮我写一个 Rust CLI 工具",
        session_id="verify-route-session",
        allow_overlay=True,
        first_turn=True,
    )

    assert verify_result.selected_skill.name == python_result.selected_skill.name
    assert (
        verify_result.overlay_skill.name if verify_result.overlay_skill else None
    ) == (python_result.overlay_skill.name if python_result.overlay_skill else None)
    assert verify_result.layer == python_result.layer
    assert python_result.route_snapshot is not None
    assert python_result.route_snapshot.engine == "python"
    assert verify_result.route_engine == "rust"
    assert verify_result.shadow_route_report is not None
    assert verify_result.shadow_route_report.selected_skill_match is True
    assert verify_result.shadow_route_report.overlay_skill_match is True
    assert verify_result.shadow_route_report.layer_match is True


def test_router_service_shadow_mode_keeps_python_primary_and_records_diff() -> None:
    """Shadow mode should keep Python as the executed route and capture a stable diff payload."""

    shadow_service = RouterService(
        RuntimeSettings(
            codex_home=PROJECT_ROOT,
            live_model_override=False,
            route_engine_mode="shadow",
        )
    )

    result = shadow_service.route(
        task="帮我写一个 Rust CLI 工具",
        session_id="shadow-route-session",
        allow_overlay=True,
        first_turn=True,
    )

    assert result.route_engine == "python"
    assert result.rollback_to_python is False
    assert result.shadow_route_report is not None
    assert result.shadow_route_report.mode == "shadow"
    assert result.shadow_route_report.primary_engine == "python"
    assert result.shadow_route_report.shadow_engine == "rust"
    assert result.shadow_route_report.selected_skill_match is True
    assert result.shadow_route_report.overlay_skill_match is True
    assert result.shadow_route_report.layer_match is True
    assert result.shadow_route_report.python.selected_skill == result.selected_skill.name


def test_router_service_rust_mode_supports_single_switch_python_rollback() -> None:
    """Rust mode should support an instant Python rollback without changing the configured mode."""

    rollback_service = RouterService(
        RuntimeSettings(
            codex_home=PROJECT_ROOT,
            live_model_override=False,
            route_engine_mode="rust",
            rust_route_rollback_to_python=True,
        )
    )

    result = rollback_service.route(
        task="帮我写一个 Rust CLI 工具",
        session_id="rust-rollback-session",
        allow_overlay=True,
        first_turn=True,
    )

    assert result.route_engine == "python"
    assert result.rollback_to_python is True
    assert result.shadow_route_report is not None
    assert result.shadow_route_report.rollback_active is True
    assert result.shadow_route_report.primary_engine == "python"
    assert rollback_service.health()["primary_authority"] == "python"
    assert rollback_service.health()["route_result_engine"] == "python"
    assert rollback_service.health()["shadow_engine"] == "rust"
    assert rollback_service.health()["rollback_to_python"] is True
    assert rollback_service.health()["python_router_required"] is True


def test_runtime_checkpointer_round_trips_resume_manifest(tmp_path: Path) -> None:
    """The unified checkpointer seam should preserve the existing manifest contract."""

    checkpointer = FilesystemRuntimeCheckpointer(
        data_dir=tmp_path / "runtime-data",
        trace_output_path=tmp_path / "TRACE_METADATA.json",
    )

    manifest = checkpointer.checkpoint(
        session_id="checkpoint-session",
        job_id="job-1",
        status="completed",
        generation=3,
        latest_cursor=None,
        event_transport_path=None,
        artifact_paths=["/tmp/example.json"],
    )

    assert manifest is not None
    loaded = checkpointer.load_checkpoint()
    assert loaded is not None
    assert loaded.session_id == "checkpoint-session"
    assert loaded.job_id == "job-1"
    assert loaded.status == "completed"
    assert loaded.generation == 3
    assert loaded.background_state_path.endswith("runtime_background_jobs.json")
    artifact_paths = checkpointer.artifact_paths(codex_home=PROJECT_ROOT)
    assert str((PROJECT_ROOT / ".supervisor_state.json").resolve()) in artifact_paths
    assert checkpointer.storage_capabilities().backend_family == "filesystem"
    assert checkpointer.health()["supports_snapshot_delta"] is False


def test_trace_service_describes_host_facing_transport(tmp_path: Path) -> None:
    """Trace service should expose a versioned polling transport descriptor."""

    settings = RuntimeSettings(
        codex_home=PROJECT_ROOT,
        data_dir=tmp_path / "runtime-data",
        trace_output_path=tmp_path / "TRACE_METADATA.json",
        live_model_override=False,
        route_engine_mode="rust",
    )
    checkpointer = FilesystemRuntimeCheckpointer(
        data_dir=settings.resolved_data_dir,
        trace_output_path=settings.resolved_trace_output_path,
    )
    trace_service = TraceService(checkpointer)

    trace_service.recorder.record(
        session_id="transport-session",
        job_id="job-transport",
        kind="job.queued",
        stage="background",
    )
    transport = trace_service.describe_transport(session_id="transport-session", job_id="job-transport")

    assert transport.transport_kind == "poll"
    assert transport.transport_family == "host-facing-bridge"
    assert transport.endpoint_kind == "runtime_method"
    assert transport.remote_capable is True
    assert transport.handoff_supported is True
    assert transport.handoff_method == "describe_runtime_event_handoff"
    assert transport.handoff_kind == "artifact_handoff"
    assert transport.binding_refresh_mode == "describe_or_checkpoint"
    assert transport.binding_artifact_format == "json"
    assert transport.binding_backend_family == "filesystem"
    assert transport.binding_artifact_path is not None
    assert transport.binding_artifact_path.endswith("runtime_event_transports/transport-session__job-transport.json")
    assert transport.describe_method == "describe_runtime_event_transport"
    assert transport.subscribe_method == "subscribe_runtime_events"
    assert transport.cleanup_method == "cleanup_runtime_events"
    assert transport.cleanup_semantics == "bridge_cache_only"
    assert transport.cleanup_preserves_replay is True
    assert transport.replay_reseed_supported is True
    assert transport.latest_cursor is not None
    assert transport.latest_cursor.job_id == "job-transport"
    persisted = json.loads(Path(transport.binding_artifact_path).read_text(encoding="utf-8"))
    assert persisted["stream_id"] == transport.stream_id
    assert persisted["binding_backend_family"] == "filesystem"
    assert persisted["latest_cursor"]["job_id"] == "job-transport"

    handoff = trace_service.describe_handoff(session_id="transport-session", job_id="job-transport")
    assert handoff.stream_id == transport.stream_id
    assert handoff.checkpoint_backend_family == "filesystem"
    assert handoff.trace_stream_path is not None
    assert handoff.resume_manifest_path is not None
    assert handoff.transport.binding_artifact_path == transport.binding_artifact_path
