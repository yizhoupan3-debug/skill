"""Focused tests for runtime service seams and route-engine boundaries."""

from __future__ import annotations

import asyncio
import json
import sys
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path

import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = PROJECT_ROOT / "codex_agno_runtime" / "src"
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

from codex_agno_runtime.checkpoint_store import FilesystemRuntimeCheckpointer
from codex_agno_runtime.config import RuntimeSettings
from codex_agno_runtime.execution_kernel import (
    ExecutionKernelRequest,
    RouterRsInfrastructureError,
    SandboxExecutionPolicy,
    SandboxResourceBudget,
    SandboxRuntimeProbe,
)
from codex_agno_runtime.middleware import MiddlewareContext
from codex_agno_runtime.memory import FactMemoryStore
from codex_agno_runtime.schemas import RunTaskResponse, UsageMetrics
from codex_agno_runtime.services import (
    ExecutionEnvironmentService,
    MemoryService,
    RouterService,
    SandboxBudgetExceeded,
    SandboxCapabilityViolation,
    StateService,
    TraceService,
    _normalize_rusage_maxrss,
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


@dataclass(frozen=True)
class _MemoryStorageCapabilities:
    backend_family: str = "memory"
    supports_atomic_replace: bool = False
    supports_compaction: bool = False
    supports_snapshot_delta: bool = False
    supports_remote_event_transport: bool = True


class _InMemoryStorageBackend:
    """Backend double that keeps trace and checkpoint payloads in memory."""

    def __init__(self) -> None:
        self._payloads: dict[Path, str] = {}

    def capabilities(self) -> _MemoryStorageCapabilities:
        return _MemoryStorageCapabilities()

    def exists(self, path: Path) -> bool:
        return path in self._payloads

    def read_text(self, path: Path) -> str:
        return self._payloads[path]

    def write_text(self, path: Path, payload: str) -> None:
        self._payloads[path] = payload


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
    control_plane_descriptor = router_service.control_plane_descriptor
    state_service = StateService(checkpointer, control_plane_descriptor=control_plane_descriptor)
    trace_service = TraceService(checkpointer, control_plane_descriptor=control_plane_descriptor)
    memory_service = MemoryService(settings, control_plane_descriptor=control_plane_descriptor)
    execution_service = ExecutionEnvironmentService(
        settings,
        router_service.prompt_builder,
        max_background_jobs=4,
        background_job_timeout_seconds=30.0,
        control_plane_descriptor=control_plane_descriptor,
    )

    for service in (router_service, state_service, trace_service, memory_service, execution_service):
        service.startup()

    state_health = state_service.health()
    trace_health = trace_service.health()

    assert router_service.health()["loaded_skill_count"] > 0
    assert router_service.health()["primary_authority"] == "rust"
    assert router_service.health()["route_result_engine"] == "rust"
    assert router_service.health()["shadow_engine"] is None
    assert router_service.health()["python_router_loaded"] is False
    assert router_service.health()["python_router_required"] is False
    assert router_service.health()["default_route_mode"] == "rust"
    assert router_service.health()["control_plane_authority"] == "rust-route-core"
    assert router_service.health()["python_runtime_role"] == "thin-projection"
    assert router_service.health()["rustification_status"]["runtime_primary_owner"] == "rust-control-plane"
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
    assert state_health["state_path"].endswith("runtime_background_jobs.json")
    assert state_health["control_plane_authority"] == "rust-runtime-control-plane"
    assert state_health["control_plane_role"] == "durable-background-state"
    assert state_health["pending_session_takeovers"] == 0
    assert state_health["background_effect_host_contract"]["service"] == "state"
    assert state_health["background_effect_host_contract"]["control_plane_delegate_kind"] == (
        "filesystem-state-store"
    )
    assert state_health["background_effect_host_contract"]["python_host_role"] == "thin-projection"
    assert state_health["background_effect_host_contract"]["steady_state_owner"] == "rust-control-plane"
    assert state_health["background_effect_host_contract"]["remaining_python_role"] == (
        "compatibility-host"
    )
    assert state_health["background_effect_host_contract"]["progression"]["runtime_primary_owner"] == (
        "rust-control-plane"
    )
    assert state_health["background_effect_host_contract"]["progression"][
        "runtime_primary_owner_authority"
    ] == "rust-runtime-control-plane"
    assert state_health["background_effect_host_contract"]["progression"]["python_runtime_role"] == (
        "compatibility-host"
    )
    assert state_health["background_effect_host_contract"]["progression"][
        "steady_state_python_allowed"
    ] is False
    assert trace_health["checkpoint_backend_family"] == "filesystem"
    assert trace_health["trace_output_path"].endswith("TRACE_METADATA.json")
    assert trace_health["event_stream_path"].endswith("TRACE_EVENTS.jsonl")
    assert trace_health["resume_manifest_path"].endswith("TRACE_RESUME_MANIFEST.json")
    assert trace_health["event_transport_dir"].endswith("runtime_event_transports")
    assert trace_health["background_state_path"].endswith("runtime_background_jobs.json")
    assert trace_health["control_plane_authority"] == "rust-runtime-control-plane"
    assert trace_health["control_plane_role"] == "trace-and-handoff"
    assert trace_health["replay_supported"] is True
    assert trace_health["event_bridge_supported"] is True
    assert trace_health["event_bridge_schema_version"] == "runtime-event-bridge-v1"
    assert trace_health["background_effect_host_contract"]["service"] == "trace"
    assert trace_health["background_effect_host_contract"]["control_plane_delegate_kind"] == (
        "filesystem-trace-store"
    )
    assert trace_health["background_effect_host_contract"]["python_host_role"] == "thin-projection"
    assert trace_health["background_effect_host_contract"]["steady_state_owner"] == "rust-control-plane"
    assert trace_health["background_effect_host_contract"]["remaining_python_role"] == (
        "compatibility-host"
    )
    assert trace_health["background_effect_host_contract"]["progression"]["runtime_primary_owner"] == (
        "rust-control-plane"
    )
    assert trace_health["background_effect_host_contract"]["progression"][
        "runtime_primary_owner_authority"
    ] == "rust-runtime-control-plane"
    assert trace_health["background_effect_host_contract"]["progression"]["python_runtime_role"] == (
        "compatibility-host"
    )
    assert trace_health["background_effect_host_contract"]["progression"][
        "steady_state_python_allowed"
    ] is False
    assert memory_service.health()["memory_dir"].endswith("codex_agno_runtime/data/memory")
    assert memory_service.health()["control_plane_authority"] == "rust-runtime-control-plane"
    assert memory_service.health()["control_plane_role"] == "memory-lifecycle"
    assert memory_service.health()["control_plane_contract"]["fact_extraction_strategy"] == (
        "contract-regex-fact-extractor"
    )
    assert memory_service.health()["fact_extraction_pattern_count"] > 0
    assert trace_service.health()["control_plane_contract"]["aligned"] is True


def test_rusage_memory_normalization_matches_host_units(monkeypatch: pytest.MonkeyPatch) -> None:
    """ru_maxrss should be normalized to bytes before sandbox budget enforcement."""

    monkeypatch.setattr("codex_agno_runtime.services.sys.platform", "darwin")
    assert _normalize_rusage_maxrss(4096) == 4096.0

    monkeypatch.setattr("codex_agno_runtime.services.sys.platform", "linux")
    assert _normalize_rusage_maxrss(4096) == 4096.0 * 1024.0
    assert trace_service.health()["control_plane_contract"]["recorder"]["stream_scope_fields"] == [
        "session_id",
        "job_id",
    ]
    assert trace_service.health()["control_plane_contract"]["recorder"]["cleanup_scope_fields"] == [
        "session_id",
        "job_id",
    ]
    assert execution_service.health()["background_job_timeout_seconds"] == 30.0
    assert execution_service.health()["execution_mode_default"] == "dry_run"
    assert execution_service.health()["control_plane_authority"] == "rust-runtime-control-plane"
    assert execution_service.health()["control_plane_role"] == "execution-kernel-control"
    assert execution_service.health()["control_plane_projection"] == "python-thin-projection"
    assert execution_service.health()["control_plane_delegate_kind"] == "rust-execution-kernel-slice"
    assert execution_service.health()["kernel_adapter_kind"] == "rust-execution-kernel-slice"
    assert execution_service.health()["kernel_authority"] == "rust-execution-kernel-authority"
    assert execution_service.health()["kernel_owner_family"] == "rust"
    assert execution_service.health()["kernel_owner_impl"] == "execution-kernel-slice"
    assert execution_service.health()["kernel_contract_mode"] == "rust-live-primary"
    assert execution_service.health()["kernel_replace_ready"] is True
    assert execution_service.health()["kernel_in_process_replacement_complete"] is True
    assert execution_service.health()["kernel_live_backend_family"] == "rust-cli"
    assert execution_service.health()["kernel_live_backend_impl"] == "router-rs"
    assert execution_service.health()["kernel_live_delegate_kind"] == "router-rs"
    assert execution_service.health()["kernel_live_delegate_authority"] == "rust-execution-cli"
    assert execution_service.health()["kernel_live_delegate_family"] == "rust-cli"
    assert execution_service.health()["kernel_live_delegate_impl"] == "router-rs"
    assert execution_service.health()["kernel_live_fallback_kind"] is None
    assert execution_service.health()["kernel_live_fallback_authority"] is None
    assert execution_service.health()["kernel_live_fallback_family"] is None
    assert execution_service.health()["kernel_live_fallback_impl"] is None
    assert execution_service.health()["kernel_live_fallback_mode"] == "disabled"
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
    assert execution_service.health()["control_plane_contracts"][
        "execution_kernel_live_fallback_retirement_status"
    ]["status_contract"] == "execution_kernel_live_fallback_retirement_status_v1"
    assert execution_service.health()["control_plane_contracts"][
        "execution_kernel_live_fallback_retirement_status"
    ]["retirement_readiness"]["next_safe_slice"] == (
        "rustification_closed"
    )
    assert execution_service.health()["control_plane_contracts"][
        "execution_kernel_live_response_serialization_contract"
    ]["status_contract"] == "execution_kernel_live_response_serialization_contract_v1"
    assert execution_service.health()["control_plane_contracts"][
        "execution_kernel_live_response_serialization_contract"
    ]["runtime_response_metadata_fields"]["shared"] == [
        "trace_event_count",
        "trace_output_path",
    ]
    assert execution_service.health()["control_plane_contracts"]["runtime_control_plane"]["authority"] == (
        "rust-runtime-control-plane"
    )

    for service in (execution_service, memory_service, trace_service, state_service, router_service):
        service.shutdown()


def test_memory_store_fact_extraction_follows_rust_first_contract_patterns(tmp_path: Path) -> None:
    """Memory extraction should use the contract-provided pattern list rather than the default heuristic set."""

    store = FactMemoryStore(
        tmp_path / "memory",
        control_plane_descriptor={
            "schema_version": "router-rs-runtime-control-plane-v1",
            "authority": "rust-runtime-control-plane",
            "services": {
                "memory": {
                    "authority": "rust-runtime-control-plane",
                    "role": "memory-lifecycle",
                    "projection": "rust-first-memory-projection",
                    "delegate_kind": "rust-fact-memory-store",
                    "fact_extraction_strategy": "contract-regex-fact-extractor",
                    "fact_extraction_ignore_case": True,
                    "fact_extraction_patterns": [
                        r"\bmy runtime preference is (?P<value>[A-Za-z][A-Za-z0-9 _.-]{1,60}?)(?=[.!?\n]|$)",
                    ],
                },
            },
        },
    )

    facts = store.extract_facts_sync("My runtime preference is Rust. I prefer tea.")

    assert facts == ["Rust"]
    assert store.health()["fact_extraction_strategy"] == "contract-regex-fact-extractor"
    assert store.health()["fact_extraction_pattern_count"] == 1


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
        control_plane_descriptor=router_service.control_plane_descriptor,
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
        assert "execution_kernel_fallback_reason" not in response.metadata
        assert response.metadata["trace_event_count"] == 7

    asyncio.run(_run())


def test_execution_service_exposes_sandbox_lifecycle_health(tmp_path: Path) -> None:
    """Execution health should surface the minimal sandbox lifecycle contract."""

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
        control_plane_descriptor=router_service.control_plane_descriptor,
    )

    health = execution_service.health()["sandbox"]

    assert health["schema_version"] == "runtime-sandbox-lifecycle-v1"
    assert health["lifecycle_states"] == [
        "created",
        "warm",
        "busy",
        "draining",
        "recycled",
        "failed",
    ]
    assert health["capability_categories"] == [
        "read_only",
        "workspace_mutating",
        "networked",
        "high_risk",
    ]
    assert health["event_log_path"].endswith("runtime_sandbox_events.jsonl")
    assert health["state_counts"]["created"] == 0
    assert health["background_effect_host_contract"]["service"] == "execution"
    assert health["background_effect_host_contract"]["steady_state_owner"] == "rust-control-plane"


def test_execution_service_schedules_async_sandbox_cleanup(tmp_path: Path) -> None:
    """Successful executions should drain first, then recycle through async cleanup."""

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
        control_plane_descriptor=router_service.control_plane_descriptor,
    )
    routing_result = router_service.route(
        task="帮我写一个 Rust CLI 工具",
        session_id="sandbox-success-session",
        allow_overlay=True,
        first_turn=True,
    )
    request = ExecutionKernelRequest(
        task=routing_result.task,
        session_id=routing_result.session_id,
        user_id="tester",
        routing_result=routing_result,
        dry_run=True,
        prompt_preview="sandbox-success",
    )

    async def fake_execute(current_request: ExecutionKernelRequest) -> RunTaskResponse:
        return RunTaskResponse(
            session_id=current_request.session_id,
            user_id=current_request.user_id,
            skill=current_request.routing_result.selected_skill.name,
            overlay=(
                current_request.routing_result.overlay_skill.name
                if current_request.routing_result.overlay_skill
                else None
            ),
            live_run=False,
            content="ok",
            prompt_preview=current_request.prompt_preview,
            usage=UsageMetrics(input_tokens=4, output_tokens=2, total_tokens=6, mode="dry_run"),
            metadata={
                "execution_kernel": "rust-execution-kernel-slice",
                "execution_kernel_authority": "rust-execution-kernel-authority",
            },
        )

    async def _run() -> None:
        response = await execution_service.execute_request(request, executor=fake_execute)
        sandbox_id = response.metadata["sandbox_id"]
        assert response.metadata["sandbox_state"] == "draining"
        assert response.metadata["sandbox_cleanup_pending"] is True
        draining_snapshot = execution_service.describe_sandbox(sandbox_id)
        assert draining_snapshot["state"] == "draining"
        recycled_snapshot = await execution_service.await_sandbox_cleanup(sandbox_id)
        assert recycled_snapshot["state"] == "recycled"
        assert recycled_snapshot["cleanup_pending"] is False
        events = [
            json.loads(line)
            for line in Path(response.metadata["sandbox_event_log_path"]).read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
        kinds = [event["kind"] for event in events if event["sandbox_id"] == sandbox_id]
        assert "sandbox.execution_started" in kinds
        assert "sandbox.cleanup_started" in kinds
        assert "sandbox.cleanup_completed" in kinds

    asyncio.run(_run())


def test_execution_service_rejects_high_risk_without_dedicated_profile(tmp_path: Path) -> None:
    """High-risk capability requests must fail closed unless the sandbox profile is dedicated."""

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
        control_plane_descriptor=router_service.control_plane_descriptor,
    )
    routing_result = router_service.route(
        task="帮我写一个 Rust CLI 工具",
        session_id="sandbox-policy-session",
        allow_overlay=True,
        first_turn=True,
    )
    request = ExecutionKernelRequest(
        task=routing_result.task,
        session_id=routing_result.session_id,
        user_id="tester",
        routing_result=routing_result,
        dry_run=True,
        prompt_preview="sandbox-policy",
        sandbox_policy=SandboxExecutionPolicy(
            profile="shared-high-risk",
            capability_categories=("high_risk",),
            dedicated_profile=False,
        ),
        sandbox_tool_category="high_risk",
    )
    executor_called = False

    async def fake_execute(_request: ExecutionKernelRequest) -> RunTaskResponse:
        nonlocal executor_called
        executor_called = True
        raise AssertionError("executor must not be called when admission fails")

    async def _run() -> None:
        with pytest.raises(
            SandboxCapabilityViolation,
            match="policy_violation:high_risk_requires_dedicated_profile",
        ):
            await execution_service.execute_request(request, executor=fake_execute)
        assert executor_called is False
        failed = execution_service.health()["sandbox"]["latest_records"][-1]
        assert failed["state"] == "failed"
        assert failed["quarantined"] is True
        assert failed["last_failure_reason"] == "policy_violation:high_risk_requires_dedicated_profile"

    asyncio.run(_run())


def test_execution_service_enforces_budget_at_admission_and_runtime(tmp_path: Path) -> None:
    """Sandbox budgets should fail on non-positive admission budgets and runtime overruns."""

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
        control_plane_descriptor=router_service.control_plane_descriptor,
    )
    routing_result = router_service.route(
        task="帮我写一个 Rust CLI 工具",
        session_id="sandbox-budget-session",
        allow_overlay=True,
        first_turn=True,
    )
    admission_request = ExecutionKernelRequest(
        task=routing_result.task,
        session_id="sandbox-budget-admission",
        user_id="tester",
        routing_result=routing_result,
        dry_run=True,
        prompt_preview="budget-admission",
        sandbox_budget=SandboxResourceBudget(cpu=1.0, memory=1, wall_clock=0.0, output_size=1),
    )
    runtime_request = ExecutionKernelRequest(
        task=routing_result.task,
        session_id="sandbox-budget-runtime",
        user_id="tester",
        routing_result=routing_result,
        dry_run=True,
        prompt_preview="budget-runtime",
        sandbox_budget=SandboxResourceBudget(cpu=1.0, memory=128, wall_clock=1.0, output_size=4),
        sandbox_runtime_probe=SandboxRuntimeProbe(cpu=0.1, memory=32, wall_clock=0.1, output_size=12),
    )

    async def fake_execute(current_request: ExecutionKernelRequest) -> RunTaskResponse:
        return RunTaskResponse(
            session_id=current_request.session_id,
            user_id=current_request.user_id,
            skill=current_request.routing_result.selected_skill.name,
            overlay=None,
            live_run=False,
            content="ok",
            prompt_preview=current_request.prompt_preview,
            usage=UsageMetrics(input_tokens=4, output_tokens=2, total_tokens=6, mode="dry_run"),
            metadata={
                "execution_kernel": "rust-execution-kernel-slice",
                "execution_kernel_authority": "rust-execution-kernel-authority",
            },
        )

    async def _run() -> None:
        with pytest.raises(SandboxBudgetExceeded, match="budget_admission_failed:wall_clock_non_positive"):
            await execution_service.execute_request(admission_request, executor=fake_execute)
        admitted_failure = execution_service.health()["sandbox"]["latest_records"][-1]
        assert admitted_failure["state"] == "failed"
        assert admitted_failure["last_failure_reason"] == "budget_admission_failed:wall_clock_non_positive"

        with pytest.raises(SandboxBudgetExceeded, match="output_size_exceeded"):
            await execution_service.execute_request(runtime_request, executor=fake_execute)
        runtime_failure = execution_service.health()["sandbox"]["latest_records"][-1]
        assert runtime_failure["state"] == "draining"
        recycled = await execution_service.await_sandbox_cleanup(runtime_failure["sandbox_id"])
        assert recycled["state"] == "recycled"
        assert recycled["last_budget_violation"] == "output_size_exceeded"

    asyncio.run(_run())


def test_execution_service_failure_isolation_keeps_other_sandboxes_healthy(tmp_path: Path) -> None:
    """A failed sandbox must stay quarantined without contaminating later executions."""

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
        control_plane_descriptor=router_service.control_plane_descriptor,
    )
    routing_result = router_service.route(
        task="帮我写一个 Rust CLI 工具",
        session_id="sandbox-isolation-session",
        allow_overlay=True,
        first_turn=True,
    )
    failed_request = ExecutionKernelRequest(
        task=routing_result.task,
        session_id="sandbox-isolation-failed",
        user_id="tester",
        routing_result=routing_result,
        dry_run=True,
        prompt_preview="sandbox-isolation-failed",
        sandbox_policy=SandboxExecutionPolicy(
            profile="shared-high-risk",
            capability_categories=("high_risk",),
            dedicated_profile=False,
        ),
        sandbox_tool_category="high_risk",
    )
    healthy_request = ExecutionKernelRequest(
        task=routing_result.task,
        session_id="sandbox-isolation-healthy",
        user_id="tester",
        routing_result=routing_result,
        dry_run=True,
        prompt_preview="sandbox-isolation-healthy",
        sandbox_policy=SandboxExecutionPolicy(
            profile="workspace-low-risk",
            capability_categories=("read_only", "workspace_mutating"),
            dedicated_profile=False,
        ),
        sandbox_tool_category="workspace_mutating",
    )

    async def fake_execute(current_request: ExecutionKernelRequest) -> RunTaskResponse:
        return RunTaskResponse(
            session_id=current_request.session_id,
            user_id=current_request.user_id,
            skill=current_request.routing_result.selected_skill.name,
            overlay=None,
            live_run=False,
            content="healthy",
            prompt_preview=current_request.prompt_preview,
            usage=UsageMetrics(input_tokens=3, output_tokens=2, total_tokens=5, mode="dry_run"),
            metadata={
                "execution_kernel": "rust-execution-kernel-slice",
                "execution_kernel_authority": "rust-execution-kernel-authority",
            },
        )

    async def _run() -> None:
        with pytest.raises(SandboxCapabilityViolation):
            await execution_service.execute_request(failed_request, executor=fake_execute)
        healthy_response = await execution_service.execute_request(healthy_request, executor=fake_execute)
        healthy_snapshot = await execution_service.await_sandbox_cleanup(healthy_response.metadata["sandbox_id"])
        assert healthy_snapshot["state"] == "recycled"
        failed_snapshot = execution_service.health()["sandbox"]["latest_records"][0]
        assert failed_snapshot["state"] == "failed"
        assert failed_snapshot["quarantined"] is True
        state_counts = execution_service.health()["sandbox"]["state_counts"]
        assert state_counts["failed"] == 1
        assert state_counts["recycled"] == 1

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
        control_plane_descriptor=router_service.control_plane_descriptor,
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
        control_plane_descriptor=router_service.control_plane_descriptor,
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
    assert dry_run_contract["execution_kernel_delegate"] == "router-rs"
    assert dry_run_contract["execution_kernel_delegate_authority"] == "rust-execution-cli"
    assert dry_run_contract["execution_kernel_delegate_family"] == "rust-cli"
    assert dry_run_contract["execution_kernel_delegate_impl"] == "router-rs"
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
        assert response.metadata["execution_kernel_delegate"] == "router-rs"
        assert response.metadata["execution_kernel_delegate_authority"] == "rust-execution-cli"
        assert response.metadata["execution_kernel_delegate_family"] == "rust-cli"
        assert response.metadata["execution_kernel_delegate_impl"] == "router-rs"
        assert response.metadata["execution_kernel_live_primary"] == "router-rs"
        assert response.metadata["execution_kernel_live_primary_authority"] == "rust-execution-cli"
        assert response.metadata["execution_kernel_live_fallback"] is None
        assert response.metadata["execution_kernel_live_fallback_authority"] is None
        assert response.metadata["execution_kernel_live_fallback_enabled"] is False
        assert response.metadata["execution_kernel_live_fallback_mode"] == "disabled"
        assert "execution_kernel_fallback_reason" not in response.metadata

    asyncio.run(_run())


def test_execution_service_prefers_rust_live_metadata_when_present(tmp_path: Path) -> None:
    """Authority adapter should preserve Rust-emitted live metadata instead of rehydrating defaults."""

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
        control_plane_descriptor=router_service.control_plane_descriptor,
    )
    routing_result = router_service.route(
        task="帮我写一个 Rust CLI 工具",
        session_id="kernel-service-live-metadata-session",
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

    async def fake_execute(request):
        return RunTaskResponse(
            session_id=request.session_id,
            user_id=request.user_id,
            skill=request.routing_result.selected_skill.name,
            overlay=request.routing_result.overlay_skill.name if request.routing_result.overlay_skill else None,
            live_run=True,
            content="live result",
            prompt_preview=None,
            model_id="gpt-5.4",
            usage=UsageMetrics(input_tokens=8, output_tokens=5, total_tokens=13, mode="live"),
            metadata={
                "execution_kernel": "router-rs",
                "execution_kernel_authority": "rust-execution-cli",
                "execution_kernel_delegate_family": "rust-direct-live",
                "execution_kernel_delegate_impl": "router-rs-http",
                "execution_kernel_live_primary": "router-rs-live-primary",
                "execution_kernel_live_primary_authority": "rust-primary-authority",
                "execution_kernel_live_fallback": None,
                "execution_kernel_live_fallback_authority": None,
                "execution_kernel_live_fallback_enabled": False,
                "execution_kernel_live_fallback_mode": "disabled",
            },
        )

    execution_service.kernel._delegate.execute = fake_execute  # type: ignore[method-assign]

    async def _run() -> None:
        response = await execution_service.execute(
            ctx=ctx,
            dry_run=False,
            trace_event_count=5,
            trace_output_path="/tmp/TRACE_METADATA.json",
        )
        assert response.metadata["execution_kernel"] == "rust-execution-kernel-slice"
        assert response.metadata["execution_kernel_authority"] == "rust-execution-kernel-authority"
        assert response.metadata["execution_kernel_delegate"] == "router-rs"
        assert response.metadata["execution_kernel_delegate_authority"] == "rust-execution-cli"
        assert response.metadata["execution_kernel_delegate_family"] == "rust-direct-live"
        assert response.metadata["execution_kernel_delegate_impl"] == "router-rs-http"
        assert response.metadata["execution_kernel_live_primary"] == "router-rs-live-primary"
        assert response.metadata["execution_kernel_live_primary_authority"] == "rust-primary-authority"
        assert response.metadata["execution_kernel_live_fallback"] is None
        assert response.metadata["execution_kernel_live_fallback_authority"] is None
        assert response.metadata["execution_kernel_live_fallback_enabled"] is False
        assert response.metadata["execution_kernel_live_fallback_mode"] == "disabled"

    asyncio.run(_run())


def test_execution_service_live_path_uses_router_rs_before_python_fallback(tmp_path: Path) -> None:
    """Default live composition should hit router-rs directly with no Python fallback lane."""

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
        control_plane_descriptor=router_service.control_plane_descriptor,
    )
    routing_result = router_service.route(
        task="帮我写一个 Rust CLI 工具",
        session_id="kernel-service-rust-primary-live-session",
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
    async def fake_execute(request):
        return RunTaskResponse(
            session_id=request.session_id,
            user_id=request.user_id,
            skill=request.routing_result.selected_skill.name,
            overlay=request.routing_result.overlay_skill.name if request.routing_result.overlay_skill else None,
            live_run=True,
            content="rust live result",
            prompt_preview=None,
            model_id="gpt-5.4",
            usage=UsageMetrics(input_tokens=8, output_tokens=5, total_tokens=13, mode="live"),
            metadata={
                "execution_kernel": "router-rs",
                "execution_kernel_authority": "rust-execution-cli",
                "execution_kernel_delegate_family": "rust-cli",
                "execution_kernel_delegate_impl": "router-rs",
                "execution_kernel_live_primary": "router-rs",
                "execution_kernel_live_primary_authority": "rust-execution-cli",
            },
        )

    execution_service.kernel._delegate.execute = fake_execute  # type: ignore[method-assign]

    async def _run() -> None:
        response = await execution_service.execute(
            ctx=ctx,
            dry_run=False,
            trace_event_count=5,
            trace_output_path="/tmp/TRACE_METADATA.json",
        )
        assert response.content == "rust live result"
        assert response.metadata["execution_kernel_delegate"] == "router-rs"
        assert response.metadata["execution_kernel_delegate_authority"] == "rust-execution-cli"
        assert response.metadata["execution_kernel_live_primary"] == "router-rs"
        assert response.metadata["execution_kernel_live_primary_authority"] == "rust-execution-cli"
        assert response.metadata["execution_kernel_live_fallback"] is None
        assert response.metadata["execution_kernel_live_fallback_authority"] is None
        assert response.metadata["execution_kernel_live_fallback_mode"] == "disabled"

    asyncio.run(_run())


def test_execution_service_live_path_propagates_router_rs_infrastructure_errors(tmp_path: Path) -> None:
    """Default live composition should surface router-rs infrastructure failures directly."""

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
        control_plane_descriptor=router_service.control_plane_descriptor,
    )
    routing_result = router_service.route(
        task="帮我写一个 Rust CLI 工具",
        session_id="kernel-service-fallback-live-session",
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
    async def failing_execute(_request):
        raise RouterRsInfrastructureError("router-rs missing")

    execution_service.kernel._delegate.execute = failing_execute  # type: ignore[method-assign]

    async def _run() -> None:
        with pytest.raises(RouterRsInfrastructureError, match="router-rs missing"):
            await execution_service.execute(
                ctx=ctx,
                dry_run=False,
                trace_event_count=5,
                trace_output_path="/tmp/TRACE_METADATA.json",
            )

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
        control_plane_descriptor=router_service.control_plane_descriptor,
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
        control_plane_descriptor=router_service.control_plane_descriptor,
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
        "supervisor_state_contract_v2"
    )
    assert descriptors["supervisor_state_contract"]["state_artifact_path"] == ".supervisor_state.json"
    assert descriptors["supervisor_state_contract"]["compatibility_rules"][
        "no_shadow_replacement_artifact"
    ] is True
    assert descriptors["execution_kernel_live_fallback_retirement_status"]["status_contract"] == (
        "execution_kernel_live_fallback_retirement_status_v1"
    )
    assert descriptors["execution_kernel_live_fallback_retirement_status"]["live_primary"][
        "authority"
    ] == "rust-execution-cli"
    blockers = descriptors["execution_kernel_live_fallback_retirement_status"][
        "retirement_readiness"
    ]["blockers"]
    assert blockers == []
    assert descriptors["execution_kernel_live_response_serialization_contract"][
        "status_contract"
    ] == "execution_kernel_live_response_serialization_contract_v1"
    assert descriptors["execution_kernel_live_response_serialization_contract"][
        "current_contract_truth"
    ]["public_response_model"] == "RunTaskResponse"
    assert descriptors["execution_kernel_live_response_serialization_contract"][
        "retirement_gates"
    ]["compatibility_live_response_serialization_still_python_owned"] is False
    assert descriptors["runtime_control_plane"]["authority"] == "rust-runtime-control-plane"


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
    assert verify_result.shadow_route_report.report_schema_version == "router-rs-route-report-v1"
    assert verify_result.shadow_route_report.authority == "rust-route-core"
    assert verify_result.shadow_route_report.selected_skill_match is True
    assert verify_result.shadow_route_report.overlay_skill_match is True
    assert verify_result.shadow_route_report.layer_match is True


def test_router_service_shadow_mode_keeps_rust_primary_and_records_diff() -> None:
    """Shadow mode should keep Rust as the executed route and capture a stable diff payload."""

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

    assert result.route_engine == "rust"
    assert result.rollback_to_python is False
    assert result.shadow_route_report is not None
    assert result.shadow_route_report.report_schema_version == "router-rs-route-report-v1"
    assert result.shadow_route_report.authority == "rust-route-core"
    assert result.shadow_route_report.mode == "shadow"
    assert result.shadow_route_report.primary_engine == "rust"
    assert result.shadow_route_report.shadow_engine == "python"
    assert result.shadow_route_report.selected_skill_match is True
    assert result.shadow_route_report.overlay_skill_match is True
    assert result.shadow_route_report.layer_match is True
    assert result.shadow_route_report.python.selected_skill == result.selected_skill.name


def test_router_service_rust_mode_keeps_rollback_as_diagnostic_lane() -> None:
    """Rust mode should keep rollback as a diagnostic lane without changing the live route engine."""

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

    assert result.route_engine == "rust"
    assert result.rollback_to_python is False
    assert result.shadow_route_report is not None
    assert result.shadow_route_report.report_schema_version == "router-rs-route-report-v1"
    assert result.shadow_route_report.authority == "rust-route-core"
    assert result.shadow_route_report.rollback_active is True
    assert result.shadow_route_report.primary_engine == "rust"
    assert result.shadow_route_report.shadow_engine == "python"
    assert rollback_service.health()["primary_authority"] == "rust"
    assert rollback_service.health()["route_result_engine"] == "rust"
    assert rollback_service.health()["shadow_engine"] == "python"
    assert rollback_service.health()["rollback_to_python"] is False
    assert rollback_service.health()["python_router_required"] is False


def test_runtime_checkpointer_round_trips_resume_manifest(tmp_path: Path) -> None:
    """The unified checkpointer seam should preserve the existing manifest contract."""

    with _project_supervisor_state() as supervisor_state_path:
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
        assert str(supervisor_state_path.resolve()) in artifact_paths
        assert checkpointer.storage_capabilities().backend_family == "filesystem"
        assert checkpointer.health()["supports_snapshot_delta"] is False


def test_trace_service_uses_storage_backend_for_trace_and_resume_handoff(tmp_path: Path) -> None:
    """Trace service should persist stream, metadata, and handoff artifacts through the backend seam."""

    with _project_supervisor_state():
        backend = _InMemoryStorageBackend()
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
            storage_backend=backend,
        )
        router_service = RouterService(settings)
        trace_service = TraceService(checkpointer, control_plane_descriptor=router_service.control_plane_descriptor)

        trace_service.recorder.record(
            session_id="backend-trace-session",
            job_id="job-backend",
            kind="job.queued",
            stage="background",
        )
        trace_service.recorder.flush_metadata(
            task="backend trace seam",
            matched_skills=["trace-observability"],
            owner="trace-observability",
            gate="none",
            overlay=None,
            artifact_paths=["/tmp/example.json"],
            verification_status="dry_run",
        )
        trace_service.checkpoint(
            session_id="backend-trace-session",
            job_id="job-backend",
            status="completed",
            artifact_paths=["/tmp/example.json"],
            supervisor_projection={
                "supervisor_state_path": str(PROJECT_ROOT / ".supervisor_state.json"),
                "active_phase": "validated",
                "verification_status": "completed",
            },
        )

        transport = trace_service.describe_transport(session_id="backend-trace-session", job_id="job-backend")
        handoff = trace_service.describe_handoff(session_id="backend-trace-session", job_id="job-backend")
        manifest = checkpointer.load_checkpoint()
        paths = checkpointer.describe_paths()

        assert trace_service.health()["checkpoint_backend_family"] == "memory"
        assert transport.binding_backend_family == "memory"
        assert transport.binding_artifact_path is not None
        assert handoff.checkpoint_backend_family == "memory"
        assert handoff.trace_stream_path == str(paths.event_stream_path)
        assert handoff.resume_manifest_path == str(paths.resume_manifest_path)
        assert manifest is not None
        assert manifest.session_id == "backend-trace-session"
        assert manifest.status == "completed"
        assert manifest.latest_cursor is not None
        assert backend.exists(paths.trace_output_path)
        assert backend.exists(paths.event_stream_path)
        assert backend.exists(paths.resume_manifest_path)
        assert backend.exists(Path(transport.binding_artifact_path))
        assert not Path(paths.trace_output_path).exists()
        assert not Path(paths.event_stream_path).exists()
        assert not Path(paths.resume_manifest_path).exists()
        assert not Path(transport.binding_artifact_path).exists()
        persisted_metadata = json.loads(backend.read_text(paths.trace_output_path))
        persisted_transport = json.loads(backend.read_text(Path(transport.binding_artifact_path)))
        assert persisted_metadata["trace_event_sink_schema_version"] is not None
        assert persisted_metadata["stream"]["event_stream_path"] == str(paths.event_stream_path)
        assert persisted_transport["binding_backend_family"] == "memory"


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
    router_service = RouterService(settings)
    trace_service = TraceService(checkpointer, control_plane_descriptor=router_service.control_plane_descriptor)

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
    assert transport.remote_attach_supported is True
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
    assert transport.attach_target is not None
    assert transport.attach_target.endpoint_kind == "runtime_method"
    assert transport.attach_target.session_id == "transport-session"
    assert transport.attach_target.job_id == "job-transport"
    assert transport.replay_anchor is not None
    assert transport.replay_anchor.anchor_kind == "trace_replay_cursor"
    assert transport.replay_anchor.latest_cursor is not None
    assert transport.replay_anchor.latest_cursor.job_id == "job-transport"
    persisted = json.loads(Path(transport.binding_artifact_path).read_text(encoding="utf-8"))
    assert persisted["stream_id"] == transport.stream_id
    assert persisted["binding_backend_family"] == "filesystem"
    assert persisted["latest_cursor"]["job_id"] == "job-transport"
    assert persisted["attach_target"]["session_id"] == "transport-session"
    assert persisted["replay_anchor"]["anchor_kind"] == "trace_replay_cursor"

    handoff = trace_service.describe_handoff(session_id="transport-session", job_id="job-transport")
    assert handoff.stream_id == transport.stream_id
    assert handoff.checkpoint_backend_family == "filesystem"
    assert handoff.trace_stream_path is not None
    assert handoff.resume_manifest_path is not None
    assert handoff.remote_attach_strategy == "transport_descriptor_then_replay"
    assert handoff.cleanup_preserves_replay is True
    assert handoff.attach_target is not None
    assert handoff.attach_target.session_id == "transport-session"
    assert handoff.replay_anchor is not None
    assert handoff.replay_anchor.latest_cursor is not None
    assert handoff.recovery_artifacts == [
        transport.binding_artifact_path,
        handoff.resume_manifest_path,
        handoff.trace_stream_path,
    ]
    assert handoff.transport.binding_artifact_path == transport.binding_artifact_path
