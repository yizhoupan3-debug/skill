from __future__ import annotations

import asyncio
import json
import sys
from pathlib import Path
from types import SimpleNamespace

import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = PROJECT_ROOT / "codex_agno_runtime" / "src"
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

from codex_agno_runtime.execution_kernel import (
    ExecutionKernelRequest,
    PythonAgnoExecutionKernel,
    RouterRsExecutionKernel,
)
from codex_agno_runtime.execution_kernel_contracts import (
    EXECUTION_KERNEL_FALLBACK_REASON_METADATA_KEY,
    build_compatibility_fallback_metadata,
    build_execution_kernel_compatibility_agent_instructions,
    build_execution_kernel_compatibility_live_response,
    build_execution_kernel_dry_run_response,
    build_execution_kernel_live_response_serialization_contract_core,
    build_execution_kernel_runtime_metadata,
    build_trace_runtime_metadata,
    resolve_execution_kernel_prompt_preview,
)
from codex_agno_runtime.schemas import RoutingResult, SkillMetadata, UsageMetrics


class _PromptBuilder:
    def build_prompt(self, routing_result: RoutingResult) -> str:
        return f"Prompt for {routing_result.selected_skill.name}"


class _Status:
    value = "completed"


class _Metrics:
    input_tokens = 11
    output_tokens = 7
    total_tokens = 18


class _RunOutput:
    run_id = "py-fallback-run"
    model = "python-fallback-model"
    status = _Status()
    metrics = _Metrics()

    def get_content_as_string(self) -> str:
        return "python fallback content"


class _FallbackAgent:
    instructions: list[str] = []

    async def arun(self, *args, **kwargs):
        return _RunOutput()


def _routing_result() -> RoutingResult:
    return RoutingResult(
        task="Replace the python live delegate",
        session_id="kernel-contract-session",
        selected_skill=SkillMetadata(name="plan-to-code"),
        overlay_skill=SkillMetadata(name="rust-pro"),
        layer="L2",
        reasons=["Trigger phrase matched: 直接做代码."],
        route_engine="rust",
    )


def _request(*, dry_run: bool = False) -> ExecutionKernelRequest:
    return ExecutionKernelRequest(
        task="Replace the python live delegate",
        session_id="kernel-contract-session",
        user_id="tester",
        routing_result=_routing_result(),
        prompt_preview="Keep execution Rust-first.",
        dry_run=dry_run,
        trace_event_count=9,
        trace_output_path="/tmp/TRACE_METADATA.json",
    )


def test_router_rs_execution_kernel_decodes_cli_contract(monkeypatch) -> None:
    settings = SimpleNamespace(
        codex_home=PROJECT_ROOT,
        rust_router_timeout_seconds=5.0,
        default_output_tokens=512,
        model_id="gpt-5.4",
        aggregator_base_url="http://127.0.0.1:20128/v1",
        aggregator_api_key="sk-test",
    )
    kernel = RouterRsExecutionKernel(settings)  # type: ignore[arg-type]

    def fake_run(command, **kwargs):
        assert "--execute-json" in command
        payload = json.loads(command[command.index("--execute-input-json") + 1])
        assert payload["selected_skill"] == "plan-to-code"
        return SimpleNamespace(
            returncode=0,
            stdout=json.dumps(
                {
                    "execution_schema_version": "router-rs-execute-response-v1",
                    "authority": "rust-execution-cli",
                    "session_id": payload["session_id"],
                    "user_id": payload["user_id"],
                    "skill": payload["selected_skill"],
                    "overlay": payload["overlay_skill"],
                    "live_run": True,
                    "content": "router-rs content",
                    "usage": {
                        "input_tokens": 21,
                        "output_tokens": 13,
                        "total_tokens": 34,
                        "mode": "live",
                    },
                    "prompt_preview": payload["prompt_preview"],
                    "model_id": "gpt-5.4",
                    "metadata": {
                        "execution_kernel": "router-rs",
                        "execution_kernel_authority": "rust-execution-cli",
                        "execution_kernel_delegate_family": "rust-cli",
                        "execution_kernel_delegate_impl": "router-rs",
                        "execution_kernel_live_primary": "router-rs",
                        "execution_kernel_live_primary_authority": "rust-execution-cli",
                        "execution_kernel_live_fallback": "python-agno",
                        "execution_kernel_live_fallback_authority": "python-agno-kernel-adapter",
                        "execution_kernel_live_fallback_enabled": True,
                        "execution_kernel_live_fallback_mode": "compatibility",
                        "status": "completed",
                    },
                }
            ),
            stderr="",
        )

    monkeypatch.setattr("codex_agno_runtime.execution_kernel.subprocess.run", fake_run)

    response = asyncio.run(kernel.execute(_request()))

    assert response.live_run is True
    assert response.content == "router-rs content"
    assert response.metadata["execution_kernel"] == "router-rs"
    assert response.metadata["execution_kernel_authority"] == "rust-execution-cli"
    assert response.metadata["execution_kernel_delegate_family"] == "rust-cli"
    assert response.metadata["execution_kernel_delegate_impl"] == "router-rs"
    assert response.metadata["execution_kernel_live_primary"] == "router-rs"
    assert response.metadata["execution_kernel_live_primary_authority"] == "rust-execution-cli"
    assert response.metadata["execution_kernel_live_fallback"] == "python-agno"
    assert response.metadata["execution_kernel_live_fallback_authority"] == "python-agno-kernel-adapter"
    assert response.metadata["execution_kernel_live_fallback_enabled"] is True
    assert response.metadata["execution_kernel_live_fallback_mode"] == "compatibility"
    assert response.usage.total_tokens == 34


def test_python_kernel_falls_back_only_when_router_rs_infrastructure_fails(monkeypatch) -> None:
    settings = SimpleNamespace(
        codex_home=PROJECT_ROOT,
        rust_router_timeout_seconds=5.0,
        rust_execute_fallback_to_python=True,
        default_output_tokens=512,
        model_id="gpt-5.4",
        aggregator_base_url="http://127.0.0.1:20128/v1",
        aggregator_api_key="sk-test",
    )
    fallback_agent = _FallbackAgent()
    prompt_builder = _PromptBuilder()
    agent_factory = SimpleNamespace(build_compatibility_agent=lambda *args, **kwargs: fallback_agent)
    kernel = PythonAgnoExecutionKernel(settings, prompt_builder, agent_factory=agent_factory)  # type: ignore[arg-type]

    def failing_run(command, **kwargs):
        raise OSError("router-rs missing")

    monkeypatch.setattr("codex_agno_runtime.execution_kernel.subprocess.run", failing_run)

    response = asyncio.run(kernel.execute(_request()))

    assert response.live_run is True
    assert response.content == "python fallback content"
    assert response.metadata["execution_kernel"] == "python-agno"
    assert response.metadata["execution_kernel_authority"] == "python-agno-kernel-adapter"
    assert response.metadata["run_id"] == "py-fallback-run"
    assert response.metadata["status"] == "completed"
    assert response.metadata["trace_event_count"] == 9
    assert response.metadata["trace_output_path"] == "/tmp/TRACE_METADATA.json"
    assert response.metadata["execution_kernel_primary"] == "router-rs"
    assert response.metadata["execution_kernel_primary_authority"] == "rust-execution-cli"
    assert "router-rs execute could not be launched" in response.metadata[
        EXECUTION_KERNEL_FALLBACK_REASON_METADATA_KEY
    ]
    assert fallback_agent.instructions == ["Prompt for plan-to-code"]


def test_execution_kernel_contract_helpers_define_compatibility_fallback_metadata() -> None:
    trace_metadata = build_trace_runtime_metadata(
        trace_event_count=9,
        trace_output_path="/tmp/TRACE_METADATA.json",
    )
    fallback_metadata = build_compatibility_fallback_metadata(
        primary_adapter_kind="router-rs",
        primary_authority="rust-execution-cli",
        error="router-rs exploded",
    )

    assert trace_metadata == {
        "trace_event_count": 9,
        "trace_output_path": "/tmp/TRACE_METADATA.json",
    }
    assert fallback_metadata == {
        "execution_kernel_primary": "router-rs",
        "execution_kernel_primary_authority": "rust-execution-cli",
        EXECUTION_KERNEL_FALLBACK_REASON_METADATA_KEY: "router-rs exploded",
    }

    dry_run_metadata = build_execution_kernel_runtime_metadata(
        execution_kernel="python-agno",
        execution_kernel_authority="python-agno-kernel-adapter",
        trace_event_count=9,
        trace_output_path="/tmp/TRACE_METADATA.json",
        extra_fields={
            "reason": "Live model execution is disabled; returned a deterministic dry-run payload."
        },
    )
    assert dry_run_metadata == {
        "execution_kernel": "python-agno",
        "execution_kernel_authority": "python-agno-kernel-adapter",
        "reason": "Live model execution is disabled; returned a deterministic dry-run payload.",
        "trace_event_count": 9,
        "trace_output_path": "/tmp/TRACE_METADATA.json",
    }

    contract_core = build_execution_kernel_live_response_serialization_contract_core()
    assert contract_core["current_contract_truth"]["public_response_model"] == "RunTaskResponse"
    assert contract_core["runtime_response_metadata_fields"]["compatibility_fallback"] == [
        "run_id",
        "status",
        "execution_kernel_primary",
        "execution_kernel_primary_authority",
        EXECUTION_KERNEL_FALLBACK_REASON_METADATA_KEY,
    ]
    assert contract_core["current_response_shape_truth"]["compatibility_fallback"][
        "required_metadata_fields"
    ] == [
        "run_id",
        "status",
        "trace_event_count",
        "trace_output_path",
        "execution_kernel_primary",
        "execution_kernel_primary_authority",
        EXECUTION_KERNEL_FALLBACK_REASON_METADATA_KEY,
    ]

    dry_run_response = build_execution_kernel_dry_run_response(
        session_id="kernel-contract-session",
        user_id="tester",
        skill="plan-to-code",
        overlay="rust-pro",
        content="[dry-run] response",
        prompt_preview="Keep execution Rust-first.",
        input_tokens=12,
        output_tokens=34,
        execution_kernel="python-agno",
        execution_kernel_authority="python-agno-kernel-adapter",
        trace_event_count=9,
        trace_output_path="/tmp/TRACE_METADATA.json",
    )
    assert dry_run_response.live_run is False
    assert dry_run_response.usage.mode == "estimated"
    assert dry_run_response.metadata["reason"] == (
        "Live model execution is disabled; returned a deterministic dry-run payload."
    )

    live_response = build_execution_kernel_compatibility_live_response(
        session_id="kernel-contract-session",
        user_id="tester",
        skill="plan-to-code",
        overlay="rust-pro",
        content="python fallback content",
        usage=UsageMetrics(input_tokens=11, output_tokens=7, total_tokens=18, mode="live"),
        prompt_preview="Keep execution Rust-first.",
        model_id="python-fallback-model",
        run_id="py-fallback-run",
        status="completed",
        execution_kernel="python-agno",
        execution_kernel_authority="python-agno-kernel-adapter",
        trace_event_count=9,
        trace_output_path="/tmp/TRACE_METADATA.json",
    )
    assert live_response.live_run is True
    assert live_response.metadata["run_id"] == "py-fallback-run"
    assert live_response.metadata["status"] == "completed"


def test_execution_kernel_prompt_preview_resolution_prefers_existing_preview() -> None:
    routing_result = _routing_result()
    builder_calls = 0

    def _build_prompt(current: RoutingResult) -> str:
        nonlocal builder_calls
        builder_calls += 1
        return f"Prompt for {current.selected_skill.name}"

    preview = resolve_execution_kernel_prompt_preview(
        prompt_preview="existing preview",
        routing_result=routing_result,
        build_prompt=_build_prompt,
    )
    assert preview == "existing preview"
    assert builder_calls == 0

    routing_result.prompt_preview = None
    generated_preview = resolve_execution_kernel_prompt_preview(
        prompt_preview=None,
        routing_result=routing_result,
        build_prompt=_build_prompt,
    )
    assert generated_preview == "Prompt for plan-to-code"
    assert routing_result.prompt_preview == "Prompt for plan-to-code"
    assert builder_calls == 1

    instructions = build_execution_kernel_compatibility_agent_instructions(
        routing_result=routing_result,
        build_prompt=_build_prompt,
    )
    assert instructions == ["Prompt for plan-to-code"]
    assert builder_calls == 1


def test_python_kernel_omits_python_prompt_preview_on_rust_live_path(monkeypatch) -> None:
    settings = SimpleNamespace(
        codex_home=PROJECT_ROOT,
        rust_router_timeout_seconds=5.0,
        rust_execute_fallback_to_python=True,
        default_output_tokens=512,
        model_id="gpt-5.4",
        aggregator_base_url="http://127.0.0.1:20128/v1",
        aggregator_api_key="sk-test",
    )
    prompt_builder = _PromptBuilder()
    agent_factory = SimpleNamespace(build_compatibility_agent=lambda *args, **kwargs: _FallbackAgent())
    kernel = PythonAgnoExecutionKernel(settings, prompt_builder, agent_factory=agent_factory)  # type: ignore[arg-type]
    seen: dict[str, object] = {}

    def fake_run(command, **kwargs):
        payload = json.loads(command[command.index("--execute-input-json") + 1])
        seen["prompt_preview"] = payload["prompt_preview"]
        return SimpleNamespace(
            returncode=0,
            stdout=json.dumps(
                {
                    "execution_schema_version": "router-rs-execute-response-v1",
                    "authority": "rust-execution-cli",
                    "session_id": payload["session_id"],
                    "user_id": payload["user_id"],
                    "skill": payload["selected_skill"],
                    "overlay": payload["overlay_skill"],
                    "live_run": True,
                    "content": "router-rs content",
                    "usage": {
                        "input_tokens": 21,
                        "output_tokens": 13,
                        "total_tokens": 34,
                        "mode": "live",
                    },
                    "prompt_preview": "Rust-owned live prompt preview",
                    "model_id": "gpt-5.4",
                    "metadata": {
                        "execution_kernel": "router-rs",
                        "execution_kernel_authority": "rust-execution-cli",
                        "execution_kernel_delegate_family": "rust-cli",
                        "execution_kernel_delegate_impl": "router-rs",
                        "execution_kernel_live_primary": "router-rs",
                        "execution_kernel_live_primary_authority": "rust-execution-cli",
                        "execution_kernel_live_fallback": "python-agno",
                        "execution_kernel_live_fallback_authority": "python-agno-kernel-adapter",
                        "execution_kernel_live_fallback_enabled": True,
                        "execution_kernel_live_fallback_mode": "compatibility",
                    },
                }
            ),
            stderr="",
        )

    monkeypatch.setattr("codex_agno_runtime.execution_kernel.subprocess.run", fake_run)

    response = asyncio.run(kernel.execute(_request()))

    assert seen["prompt_preview"] is None
    assert response.prompt_preview == "Rust-owned live prompt preview"
    assert response.metadata["execution_kernel_delegate_family"] == "rust-cli"
    assert response.metadata["execution_kernel_delegate_impl"] == "router-rs"
    assert response.metadata["execution_kernel_live_primary"] == "router-rs"
    assert response.metadata["execution_kernel_live_primary_authority"] == "rust-execution-cli"
    assert response.metadata["execution_kernel_live_fallback"] == "python-agno"
    assert response.metadata["execution_kernel_live_fallback_authority"] == "python-agno-kernel-adapter"
    assert response.metadata["execution_kernel_live_fallback_enabled"] is True
    assert response.metadata["execution_kernel_live_fallback_mode"] == "compatibility"


def test_python_kernel_raises_when_router_rs_infrastructure_fails_and_live_fallback_is_disabled(
    monkeypatch,
) -> None:
    settings = SimpleNamespace(
        codex_home=PROJECT_ROOT,
        rust_router_timeout_seconds=5.0,
        default_output_tokens=512,
        model_id="gpt-5.4",
        aggregator_base_url="http://127.0.0.1:20128/v1",
        aggregator_api_key="sk-test",
        rust_execute_fallback_to_python=False,
    )
    fallback_calls = 0

    class _TrackingFallbackAgent(_FallbackAgent):
        async def arun(self, *args, **kwargs):
            nonlocal fallback_calls
            fallback_calls += 1
            return await super().arun(*args, **kwargs)

    prompt_builder = _PromptBuilder()
    agent_factory = SimpleNamespace(build_compatibility_agent=lambda *args, **kwargs: _TrackingFallbackAgent())
    kernel = PythonAgnoExecutionKernel(settings, prompt_builder, agent_factory=agent_factory)  # type: ignore[arg-type]

    def failing_run(command, **kwargs):
        raise OSError("router-rs missing")

    monkeypatch.setattr("codex_agno_runtime.execution_kernel.subprocess.run", failing_run)

    with pytest.raises(RuntimeError, match="infrastructure failed while Python live fallback is disabled"):
        asyncio.run(kernel.execute(_request()))

    assert fallback_calls == 0


def test_python_kernel_does_not_fallback_for_router_rs_live_execute_errors(monkeypatch) -> None:
    settings = SimpleNamespace(
        codex_home=PROJECT_ROOT,
        rust_router_timeout_seconds=5.0,
        rust_execute_fallback_to_python=True,
        default_output_tokens=512,
        model_id="gpt-5.4",
        aggregator_base_url="http://127.0.0.1:20128/v1",
        aggregator_api_key="sk-test",
    )
    fallback_calls = 0

    class _TrackingFallbackAgent(_FallbackAgent):
        async def arun(self, *args, **kwargs):
            nonlocal fallback_calls
            fallback_calls += 1
            return await super().arun(*args, **kwargs)

    prompt_builder = _PromptBuilder()
    agent_factory = SimpleNamespace(build_compatibility_agent=lambda *args, **kwargs: _TrackingFallbackAgent())
    kernel = PythonAgnoExecutionKernel(settings, prompt_builder, agent_factory=agent_factory)  # type: ignore[arg-type]

    def failing_run(command, **kwargs):
        return SimpleNamespace(returncode=1, stdout="", stderr="router-rs live execute returned HTTP 502")

    monkeypatch.setattr("codex_agno_runtime.execution_kernel.subprocess.run", failing_run)

    with pytest.raises(RuntimeError, match="without entering the Python compatibility fallback"):
        asyncio.run(kernel.execute(_request()))

    assert fallback_calls == 0


def test_python_kernel_dry_run_still_works_when_live_fallback_is_disabled(monkeypatch) -> None:
    settings = SimpleNamespace(
        codex_home=PROJECT_ROOT,
        rust_router_timeout_seconds=5.0,
        default_output_tokens=512,
        model_id="gpt-5.4",
        aggregator_base_url="http://127.0.0.1:20128/v1",
        aggregator_api_key="sk-test",
        rust_execute_fallback_to_python=False,
    )
    fallback_calls = 0

    class _TrackingFallbackAgent(_FallbackAgent):
        async def arun(self, *args, **kwargs):
            nonlocal fallback_calls
            fallback_calls += 1
            return await super().arun(*args, **kwargs)

    prompt_builder = _PromptBuilder()
    agent_factory = SimpleNamespace(build_compatibility_agent=lambda *args, **kwargs: _TrackingFallbackAgent())
    kernel = PythonAgnoExecutionKernel(settings, prompt_builder, agent_factory=agent_factory)  # type: ignore[arg-type]

    def dry_run_success(command, **kwargs):
        payload = json.loads(command[command.index("--execute-input-json") + 1])
        assert payload["dry_run"] is True
        assert payload["prompt_preview"] == "Keep execution Rust-first."
        return SimpleNamespace(
            returncode=0,
            stdout=json.dumps(
                {
                    "execution_schema_version": "router-rs-execute-response-v1",
                    "authority": "rust-execution-cli",
                    "session_id": payload["session_id"],
                    "user_id": payload["user_id"],
                    "skill": payload["selected_skill"],
                    "overlay": payload["overlay_skill"],
                    "live_run": False,
                    "content": "[dry-run] Routed to `plan-to-code` on L2. Session `kernel-contract-session` is ready for Rust-owned execution.",
                    "usage": {
                        "input_tokens": 21,
                        "output_tokens": 13,
                        "total_tokens": 34,
                        "mode": "estimated",
                    },
                    "prompt_preview": payload["prompt_preview"],
                    "model_id": None,
                    "metadata": {
                        "reason": "router-rs returned a deterministic dry-run payload.",
                        "trace_event_count": payload["trace_event_count"],
                        "trace_output_path": payload["trace_output_path"],
                        "execution_kernel": "router-rs",
                        "execution_kernel_authority": "rust-execution-cli",
                        "execution_mode": "dry_run",
                        "route_engine": payload["route_engine"],
                        "rollback_to_python": payload["rollback_to_python"],
                    },
                }
            ),
            stderr="",
        )

    monkeypatch.setattr("codex_agno_runtime.execution_kernel.subprocess.run", dry_run_success)

    response = asyncio.run(kernel.execute(_request(dry_run=True)))

    assert response.live_run is False
    assert response.content.startswith("[dry-run]")
    assert response.prompt_preview == "Keep execution Rust-first."
    assert response.metadata["execution_kernel"] == "router-rs"
    assert response.metadata["execution_kernel_authority"] == "rust-execution-cli"
    assert response.metadata["reason"] == "router-rs returned a deterministic dry-run payload."
    assert response.metadata["trace_event_count"] == 9
    assert response.metadata["trace_output_path"] == "/tmp/TRACE_METADATA.json"
    assert fallback_calls == 0
