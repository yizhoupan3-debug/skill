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
from codex_agno_runtime.schemas import RoutingResult, SkillMetadata


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
    assert response.usage.total_tokens == 34


def test_python_kernel_falls_back_only_when_rust_execute_fails(monkeypatch) -> None:
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
        return SimpleNamespace(returncode=1, stdout="", stderr="router-rs exploded")

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
    assert "router-rs exploded" in response.metadata["execution_kernel_fallback_reason"]
    assert fallback_agent.instructions == ["Keep execution Rust-first."]


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
                    },
                }
            ),
            stderr="",
        )

    monkeypatch.setattr("codex_agno_runtime.execution_kernel.subprocess.run", fake_run)

    response = asyncio.run(kernel.execute(_request()))

    assert seen["prompt_preview"] is None
    assert response.prompt_preview == "Rust-owned live prompt preview"


def test_python_kernel_raises_when_rust_execute_fails_and_live_fallback_is_disabled(monkeypatch) -> None:
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
        return SimpleNamespace(returncode=1, stdout="", stderr="router-rs exploded")

    monkeypatch.setattr("codex_agno_runtime.execution_kernel.subprocess.run", failing_run)

    with pytest.raises(RuntimeError, match="Python live fallback is disabled"):
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

    def unexpected_run(command, **kwargs):
        raise AssertionError("router-rs execute should not run for a dry-run request")

    monkeypatch.setattr("codex_agno_runtime.execution_kernel.subprocess.run", unexpected_run)

    response = asyncio.run(kernel.execute(_request(dry_run=True)))

    assert response.live_run is False
    assert response.content.startswith("[dry-run]")
    assert response.prompt_preview == "Keep execution Rust-first."
    assert response.metadata["execution_kernel"] == "python-agno"
    assert response.metadata["execution_kernel_authority"] == "python-agno-kernel-adapter"
    assert response.metadata["reason"] == (
        "Live model execution is disabled; returned a deterministic dry-run payload."
    )
    assert response.metadata["trace_event_count"] == 9
    assert response.metadata["trace_output_path"] == "/tmp/TRACE_METADATA.json"
    assert fallback_calls == 0
