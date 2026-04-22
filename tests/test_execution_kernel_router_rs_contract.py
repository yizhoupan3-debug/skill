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

from codex_agno_runtime.execution_kernel import ExecutionKernelRequest, RouterRsExecutionKernel
from codex_agno_runtime.execution_kernel_contracts import (
    EXECUTION_KERNEL_COMPATIBILITY_AGENT_AUTHORITY_METADATA_KEY,
    EXECUTION_KERNEL_COMPATIBILITY_AGENT_CONTRACT_METADATA_KEY,
    EXECUTION_KERNEL_COMPATIBILITY_AGENT_CONTRACT_VERSION,
    EXECUTION_KERNEL_COMPATIBILITY_AGENT_KIND_METADATA_KEY,
    EXECUTION_KERNEL_COMPATIBILITY_FALLBACK_POLICY,
    EXECUTION_KERNEL_CONTRACT_MODE_METADATA_KEY,
    EXECUTION_KERNEL_FALLBACK_REASON_METADATA_KEY,
    EXECUTION_KERNEL_FALLBACK_POLICY_METADATA_KEY,
    EXECUTION_KERNEL_METADATA_SCHEMA_VERSION,
    EXECUTION_KERNEL_METADATA_SCHEMA_VERSION_METADATA_KEY,
    EXECUTION_KERNEL_MODEL_ID_SOURCE_METADATA_KEY,
    EXECUTION_KERNEL_PROMPT_PREVIEW_OWNER_METADATA_KEY,
    EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN,
    EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY,
    EXECUTION_KERNEL_RESPONSE_SHAPE_METADATA_KEY,
    EXECUTION_KERNEL_RUST_PRIMARY_CONTRACT_MODE,
    EXECUTION_KERNEL_STEADY_STATE_METADATA_FIELDS,
    LIVE_PRIMARY_MODEL_ID_SOURCE,
    LIVE_PRIMARY_PROMPT_PREVIEW_OWNER,
    build_execution_kernel_dry_run_response,
    build_execution_kernel_live_response_serialization_contract_core,
    build_execution_kernel_runtime_metadata,
    build_trace_runtime_metadata,
)
from codex_agno_runtime.schemas import RoutingResult, SkillMetadata


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


def _steady_state_kernel_metadata(**extra: object) -> dict[str, object]:
    merged_extra = dict(extra)
    merged_extra.setdefault(EXECUTION_KERNEL_MODEL_ID_SOURCE_METADATA_KEY, LIVE_PRIMARY_MODEL_ID_SOURCE)
    return build_execution_kernel_runtime_metadata(
        execution_kernel="router-rs",
        execution_kernel_authority="rust-execution-cli",
        trace_event_count=int(merged_extra.pop("trace_event_count")),
        trace_output_path=str(merged_extra.pop("trace_output_path")),
        response_shape=EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY,
        extra_fields=merged_extra,
    )


def _settings() -> SimpleNamespace:
    return SimpleNamespace(
        codex_home=PROJECT_ROOT,
        rust_router_timeout_seconds=5.0,
        default_output_tokens=512,
        model_id="gpt-5.4",
        aggregator_base_url="http://127.0.0.1:20128/v1",
        aggregator_api_key="sk-test",
    )


def test_router_rs_execution_kernel_decodes_cli_contract(monkeypatch) -> None:
    kernel = RouterRsExecutionKernel(_settings())  # type: ignore[arg-type]

    def fake_run(command, **kwargs):
        assert "--execute-json" in command
        payload = json.loads(command[command.index("--execute-input-json") + 1])
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
                    "metadata": _steady_state_kernel_metadata(
                        run_id="run-1",
                        status="completed",
                        trace_event_count=payload["trace_event_count"],
                        trace_output_path=payload["trace_output_path"],
                        execution_mode="live",
                        route_engine=payload["route_engine"],
                        diagnostic_route_mode=payload["diagnostic_route_mode"],
                    ),
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
    assert (
        response.metadata[EXECUTION_KERNEL_METADATA_SCHEMA_VERSION_METADATA_KEY]
        == EXECUTION_KERNEL_METADATA_SCHEMA_VERSION
    )
    assert response.metadata["execution_kernel_delegate_family"] == "rust-cli"
    assert response.metadata["execution_kernel_delegate_impl"] == "router-rs"
    assert response.metadata["execution_kernel_live_primary"] == "router-rs"
    assert response.metadata["execution_kernel_live_primary_authority"] == "rust-execution-cli"
    assert response.metadata["execution_kernel_live_fallback"] is None
    assert response.metadata["execution_kernel_live_fallback_authority"] is None
    assert response.metadata["execution_kernel_live_fallback_enabled"] is False
    assert response.metadata["execution_kernel_live_fallback_mode"] == "disabled"
    assert (
        response.metadata[EXECUTION_KERNEL_RESPONSE_SHAPE_METADATA_KEY]
        == EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY
    )
    assert (
        response.metadata[EXECUTION_KERNEL_PROMPT_PREVIEW_OWNER_METADATA_KEY]
        == LIVE_PRIMARY_PROMPT_PREVIEW_OWNER
    )
    assert (
        response.metadata[EXECUTION_KERNEL_MODEL_ID_SOURCE_METADATA_KEY]
        == LIVE_PRIMARY_MODEL_ID_SOURCE
    )
    assert response.usage.total_tokens == 34


def test_router_rs_execution_kernel_rejects_missing_steady_state_metadata(monkeypatch) -> None:
    kernel = RouterRsExecutionKernel(_settings())  # type: ignore[arg-type]

    def fake_run(command, **kwargs):
        payload = json.loads(command[command.index("--execute-input-json") + 1])
        broken_metadata = _steady_state_kernel_metadata(
            run_id="run-1",
            status="completed",
            trace_event_count=payload["trace_event_count"],
            trace_output_path=payload["trace_output_path"],
            execution_mode="live",
            route_engine=payload["route_engine"],
            diagnostic_route_mode=payload["diagnostic_route_mode"],
        )
        broken_metadata.pop("execution_kernel_delegate_authority")
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
                    "metadata": broken_metadata,
                }
            ),
            stderr="",
        )

    monkeypatch.setattr("codex_agno_runtime.execution_kernel.subprocess.run", fake_run)

    with pytest.raises(RuntimeError, match="execution_kernel_delegate_authority"):
        asyncio.run(kernel.execute(_request()))


def test_execution_kernel_contract_helpers_stay_rust_primary() -> None:
    trace_metadata = build_trace_runtime_metadata(
        trace_event_count=9,
        trace_output_path="/tmp/TRACE_METADATA.json",
    )
    runtime_metadata = build_execution_kernel_runtime_metadata(
        execution_kernel="router-rs",
        execution_kernel_authority="rust-execution-cli",
        trace_event_count=9,
        trace_output_path="/tmp/TRACE_METADATA.json",
        extra_fields={
            EXECUTION_KERNEL_CONTRACT_MODE_METADATA_KEY: EXECUTION_KERNEL_RUST_PRIMARY_CONTRACT_MODE,
            EXECUTION_KERNEL_FALLBACK_POLICY_METADATA_KEY: (
                EXECUTION_KERNEL_COMPATIBILITY_FALLBACK_POLICY
            ),
            "reason": "Live model execution is disabled; returned a deterministic dry-run payload.",
        },
    )

    assert trace_metadata == {
        "trace_event_count": 9,
        "trace_output_path": "/tmp/TRACE_METADATA.json",
    }
    assert runtime_metadata["execution_kernel"] == "router-rs"
    assert runtime_metadata["execution_kernel_authority"] == "rust-execution-cli"
    assert (
        runtime_metadata[EXECUTION_KERNEL_METADATA_SCHEMA_VERSION_METADATA_KEY]
        == EXECUTION_KERNEL_METADATA_SCHEMA_VERSION
    )
    assert (
        runtime_metadata[EXECUTION_KERNEL_CONTRACT_MODE_METADATA_KEY]
        == EXECUTION_KERNEL_RUST_PRIMARY_CONTRACT_MODE
    )
    assert (
        runtime_metadata[EXECUTION_KERNEL_FALLBACK_POLICY_METADATA_KEY]
        == EXECUTION_KERNEL_COMPATIBILITY_FALLBACK_POLICY
    )
    assert (
        runtime_metadata[EXECUTION_KERNEL_RESPONSE_SHAPE_METADATA_KEY]
        == EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN
    )
    assert (
        runtime_metadata[EXECUTION_KERNEL_PROMPT_PREVIEW_OWNER_METADATA_KEY]
        == "rust-execution-cli"
    )
    assert runtime_metadata["reason"] == (
        "Live model execution is disabled; returned a deterministic dry-run payload."
    )
    assert runtime_metadata["trace_event_count"] == 9
    assert runtime_metadata["trace_output_path"] == "/tmp/TRACE_METADATA.json"

    contract_core = build_execution_kernel_live_response_serialization_contract_core()
    assert contract_core["current_contract_truth"]["public_response_model"] == "RunTaskResponse"
    assert contract_core["current_contract_truth"]["execution_request_schema_version"] == (
        "router-rs-execute-request-v1"
    )
    assert contract_core["current_contract_truth"]["steady_state_metadata_schema_version"] == (
        "router-rs-execution-kernel-metadata-v1"
    )
    assert contract_core["current_contract_truth"]["steady_state_response_shapes"] == [
        "live_primary",
        "dry_run",
    ]
    assert contract_core["current_contract_truth"]["compatibility_fallback_runtime_path"] == (
        "retired"
    )
    assert contract_core["runtime_response_metadata_fields"]["steady_state_kernel"] == [
        *EXECUTION_KERNEL_STEADY_STATE_METADATA_FIELDS
    ]
    assert contract_core["runtime_response_metadata_fields"]["retired_compatibility_fallback"] == [
        "run_id",
        "status",
        EXECUTION_KERNEL_CONTRACT_MODE_METADATA_KEY,
        EXECUTION_KERNEL_FALLBACK_POLICY_METADATA_KEY,
        "execution_kernel_primary",
        "execution_kernel_primary_authority",
        EXECUTION_KERNEL_FALLBACK_REASON_METADATA_KEY,
        EXECUTION_KERNEL_COMPATIBILITY_AGENT_CONTRACT_METADATA_KEY,
        EXECUTION_KERNEL_COMPATIBILITY_AGENT_KIND_METADATA_KEY,
        EXECUTION_KERNEL_COMPATIBILITY_AGENT_AUTHORITY_METADATA_KEY,
    ]


def test_execution_kernel_dry_run_response_stays_rust_primary() -> None:
    dry_run_response = build_execution_kernel_dry_run_response(
        session_id="kernel-contract-session",
        user_id="tester",
        skill="plan-to-code",
        overlay="rust-pro",
        content="[dry-run] response",
        prompt_preview="Keep execution Rust-first.",
        input_tokens=12,
        output_tokens=34,
        execution_kernel="router-rs",
        execution_kernel_authority="rust-execution-cli",
        trace_event_count=9,
        trace_output_path="/tmp/TRACE_METADATA.json",
        extra_metadata={
            EXECUTION_KERNEL_CONTRACT_MODE_METADATA_KEY: EXECUTION_KERNEL_RUST_PRIMARY_CONTRACT_MODE,
            EXECUTION_KERNEL_FALLBACK_POLICY_METADATA_KEY: (
                EXECUTION_KERNEL_COMPATIBILITY_FALLBACK_POLICY
            ),
        },
    )

    assert dry_run_response.live_run is False
    assert dry_run_response.usage.mode == "estimated"
    assert dry_run_response.metadata["execution_kernel"] == "router-rs"
    assert dry_run_response.metadata["execution_kernel_authority"] == "rust-execution-cli"
    assert (
        dry_run_response.metadata[EXECUTION_KERNEL_METADATA_SCHEMA_VERSION_METADATA_KEY]
        == EXECUTION_KERNEL_METADATA_SCHEMA_VERSION
    )
    assert dry_run_response.metadata["reason"] == (
        "Live model execution is disabled; returned a deterministic dry-run payload."
    )
    assert dry_run_response.metadata[EXECUTION_KERNEL_CONTRACT_MODE_METADATA_KEY] == (
        EXECUTION_KERNEL_RUST_PRIMARY_CONTRACT_MODE
    )
    assert dry_run_response.metadata[EXECUTION_KERNEL_FALLBACK_POLICY_METADATA_KEY] == (
        EXECUTION_KERNEL_COMPATIBILITY_FALLBACK_POLICY
    )
    assert (
        dry_run_response.metadata[EXECUTION_KERNEL_RESPONSE_SHAPE_METADATA_KEY]
        == EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN
    )
    assert (
        dry_run_response.metadata[EXECUTION_KERNEL_PROMPT_PREVIEW_OWNER_METADATA_KEY]
        == "rust-execution-cli"
    )
