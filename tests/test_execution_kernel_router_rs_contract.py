from __future__ import annotations

import asyncio
import os
import sys
import tempfile
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
    RouterRsInfrastructureError,
    build_router_rs_execution_request_payload,
    execute_router_rs_request,
    preview_router_rs_request_prompt,
)
from codex_agno_runtime.execution_kernel_contracts import (
    EXECUTION_KERNEL_BRIDGE_AUTHORITY,
    EXECUTION_KERNEL_BRIDGE_KIND,
    EXECUTION_KERNEL_COMPATIBILITY_AGENT_AUTHORITY_METADATA_KEY,
    EXECUTION_KERNEL_COMPATIBILITY_AGENT_CONTRACT_METADATA_KEY,
    EXECUTION_KERNEL_COMPATIBILITY_AGENT_CONTRACT_VERSION,
    EXECUTION_KERNEL_COMPATIBILITY_AGENT_KIND_METADATA_KEY,
    EXECUTION_KERNEL_COMPATIBILITY_FALLBACK_POLICY,
    EXECUTION_KERNEL_CONTRACT_MODE_METADATA_KEY,
    EXECUTION_KERNEL_FALLBACK_REASON_METADATA_KEY,
    EXECUTION_KERNEL_FALLBACK_POLICY_METADATA_KEY,
    EXECUTION_KERNEL_METADATA_BRIDGE_SCHEMA_VERSION,
    EXECUTION_KERNEL_METADATA_SCHEMA_VERSION,
    EXECUTION_KERNEL_METADATA_SCHEMA_VERSION_METADATA_KEY,
    EXECUTION_KERNEL_MODEL_ID_SOURCE_METADATA_KEY,
    EXECUTION_KERNEL_PROMPT_PREVIEW_OWNER_METADATA_KEY,
    EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN,
    EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY,
    EXECUTION_KERNEL_RESPONSE_SHAPE_METADATA_KEY,
    EXECUTION_KERNEL_RUST_CANONICAL_STEADY_STATE_METADATA_FIELDS,
    EXECUTION_KERNEL_RUST_PRIMARY_CONTRACT_MODE,
    EXECUTION_KERNEL_STEADY_STATE_METADATA_FIELDS,
    LIVE_PRIMARY_MODEL_ID_SOURCE,
    LIVE_PRIMARY_PROMPT_PREVIEW_OWNER,
    build_execution_kernel_dry_run_response,
    build_execution_kernel_live_response_serialization_contract_core,
    build_execution_kernel_runtime_metadata,
    build_trace_runtime_metadata,
    validate_execution_kernel_steady_state_metadata,
)
from codex_agno_runtime.rust_router import RustRouteAdapter
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
        dry_run=dry_run,
        trace_event_count=9,
        trace_output_path="/tmp/TRACE_METADATA.json",
    )


def _steady_state_kernel_metadata(**extra: object) -> dict[str, object]:
    merged_extra = dict(extra)
    merged_extra.setdefault(EXECUTION_KERNEL_MODEL_ID_SOURCE_METADATA_KEY, LIVE_PRIMARY_MODEL_ID_SOURCE)
    return build_execution_kernel_runtime_metadata(
        execution_kernel=EXECUTION_KERNEL_BRIDGE_KIND,
        execution_kernel_authority=EXECUTION_KERNEL_BRIDGE_AUTHORITY,
        trace_event_count=int(merged_extra.pop("trace_event_count")),
        trace_output_path=str(merged_extra.pop("trace_output_path")),
        response_shape=EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY,
        extra_fields=merged_extra,
    )


def _kernel_contract(*, response_shape: str) -> dict[str, object]:
    metadata = build_execution_kernel_runtime_metadata(
        execution_kernel=EXECUTION_KERNEL_BRIDGE_KIND,
        execution_kernel_authority=EXECUTION_KERNEL_BRIDGE_AUTHORITY,
        trace_event_count=0,
        trace_output_path=None,
        response_shape=response_shape,
    )
    return {
        field: metadata[field]
        for field in EXECUTION_KERNEL_RUST_CANONICAL_STEADY_STATE_METADATA_FIELDS
    }


def _metadata_bridge(
    *,
    live_prompt_preview_owner: str = LIVE_PRIMARY_PROMPT_PREVIEW_OWNER,
    dry_run_prompt_preview_owner: str = LIVE_PRIMARY_PROMPT_PREVIEW_OWNER,
    live_primary_model_id_source: str = LIVE_PRIMARY_MODEL_ID_SOURCE,
) -> dict[str, object]:
    return {
        "schema_version": EXECUTION_KERNEL_METADATA_BRIDGE_SCHEMA_VERSION,
        "authority": "rust-runtime-control-plane",
        "projection": "python-thin-projection",
        "steady_state_fields": [
            *EXECUTION_KERNEL_RUST_CANONICAL_STEADY_STATE_METADATA_FIELDS
        ],
        "metadata_keys": {
            "metadata_schema_version": EXECUTION_KERNEL_METADATA_SCHEMA_VERSION_METADATA_KEY,
            "contract_mode": EXECUTION_KERNEL_CONTRACT_MODE_METADATA_KEY,
            "fallback_policy": EXECUTION_KERNEL_FALLBACK_POLICY_METADATA_KEY,
            "response_shape": EXECUTION_KERNEL_RESPONSE_SHAPE_METADATA_KEY,
            "prompt_preview_owner": EXECUTION_KERNEL_PROMPT_PREVIEW_OWNER_METADATA_KEY,
            "model_id_source": EXECUTION_KERNEL_MODEL_ID_SOURCE_METADATA_KEY,
        },
        "defaults": {
            "contract_mode": "rust-live-primary",
            "fallback_policy": "infrastructure-only-explicit",
            "prompt_preview_owner_by_mode": {
                "live_primary": live_prompt_preview_owner,
                "dry_run": dry_run_prompt_preview_owner,
            },
            "live_primary_model_id_source": live_primary_model_id_source,
            "supported_response_shapes": ["live_primary", "dry_run"],
        },
    }


def _settings() -> SimpleNamespace:
    return SimpleNamespace(
        codex_home=PROJECT_ROOT,
        rust_router_timeout_seconds=5.0,
        default_output_tokens=512,
        model_id="gpt-5.4",
        aggregator_base_url="http://127.0.0.1:20128/v1",
        aggregator_api_key="sk-test",
    )


def _adapter(settings: SimpleNamespace) -> RustRouteAdapter:
    return RustRouteAdapter(
        settings.codex_home,
        timeout_seconds=settings.rust_router_timeout_seconds,
    )


def test_router_rs_execution_kernel_prefers_release_binary() -> None:
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
        os.utime(router_dir / "Cargo.toml", (1_700_000_000, 1_700_000_000))
        os.utime(source_dir / "main.rs", (1_700_000_050, 1_700_000_050))
        os.utime(debug_bin, (1_700_000_100, 1_700_000_100))
        os.utime(release_bin, (1_700_000_200, 1_700_000_200))

        settings = _settings()
        settings.codex_home = codex_home
        adapter = _adapter(settings)
        adapter.router_dir = router_dir
        adapter.release_bin = release_bin
        adapter.debug_bin = debug_bin

        assert adapter._binary_command() == [str(release_bin)]


def test_router_rs_execution_kernel_requires_prebuilt_binary_when_missing() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        codex_home = Path(tmpdir)
        router_dir = codex_home / "scripts" / "router-rs"
        router_dir.mkdir(parents=True)
        (router_dir / "Cargo.toml").write_text("[package]\nname='router-rs'\nversion='0.1.0'\n", encoding="utf-8")

        settings = _settings()
        settings.codex_home = codex_home
        adapter = _adapter(settings)
        adapter.router_dir = router_dir
        adapter.release_bin = router_dir / "target" / "release" / "router-rs"
        adapter.debug_bin = router_dir / "target" / "debug" / "router-rs"

        with pytest.raises(RuntimeError, match="requires a prebuilt binary"):
            adapter._binary_command()


def test_router_rs_execution_kernel_decodes_cli_contract(monkeypatch) -> None:
    settings = _settings()
    adapter = _adapter(settings)

    def fake_execute(payload):
        return {
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

    monkeypatch.setattr(adapter, "execute", fake_execute)

    response = asyncio.run(execute_router_rs_request(_request(), settings=settings, rust_adapter=adapter))

    assert response.live_run is True
    assert response.content == "router-rs content"
    assert response.metadata["execution_kernel"] == EXECUTION_KERNEL_BRIDGE_KIND
    assert response.metadata["execution_kernel_authority"] == EXECUTION_KERNEL_BRIDGE_AUTHORITY
    assert (
        response.metadata[EXECUTION_KERNEL_METADATA_SCHEMA_VERSION_METADATA_KEY]
        == EXECUTION_KERNEL_METADATA_SCHEMA_VERSION
    )
    assert response.metadata["execution_kernel_delegate_family"] == "rust-cli"
    assert response.metadata["execution_kernel_delegate_impl"] == "router-rs"
    assert response.metadata["execution_kernel_live_primary"] == "router-rs"
    assert response.metadata["execution_kernel_live_primary_authority"] == "rust-execution-cli"
    assert "execution_kernel_live_fallback" not in response.metadata
    assert "execution_kernel_live_fallback_authority" not in response.metadata
    assert response.metadata.get("execution_kernel_live_fallback_enabled") is None
    assert response.metadata.get("execution_kernel_live_fallback_mode") is None
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


def test_router_rs_execution_request_ignores_python_prompt_preview_even_for_dry_run() -> None:
    settings = _settings()

    payload = build_router_rs_execution_request_payload(
        _request(dry_run=True),
        settings=settings,
    )

    assert payload["dry_run"] is True
    assert payload["prompt_preview"] is None


def test_preview_router_rs_request_prompt_uses_dry_run_contract(monkeypatch) -> None:
    settings = _settings()
    adapter = _adapter(settings)

    def fake_execute(payload):
        return {
            "execution_schema_version": "router-rs-execute-response-v1",
            "authority": "rust-execution-cli",
            "session_id": payload["session_id"],
            "user_id": payload["user_id"],
            "skill": payload["selected_skill"],
            "overlay": payload["overlay_skill"],
            "live_run": False,
            "content": "",
            "usage": {
                "input_tokens": 21,
                "output_tokens": 0,
                "total_tokens": 21,
                "mode": "estimated",
            },
            "prompt_preview": "Rust-owned dry-run prompt",
            "model_id": settings.model_id,
                "metadata": build_execution_kernel_runtime_metadata(
                    execution_kernel=EXECUTION_KERNEL_BRIDGE_KIND,
                    execution_kernel_authority=EXECUTION_KERNEL_BRIDGE_AUTHORITY,
                    trace_event_count=payload["trace_event_count"],
                    trace_output_path=payload["trace_output_path"],
                    extra_fields={
                        "execution_mode": "dry_run",
                        "reason": "Live model execution is disabled; returned a deterministic dry-run payload.",
                    },
                ),
            }

    monkeypatch.setattr(adapter, "execute", fake_execute)

    prompt_preview = preview_router_rs_request_prompt(
        _request(dry_run=True),
        settings=settings,
        rust_adapter=adapter,
    )

    assert prompt_preview == "Rust-owned dry-run prompt"


def test_router_rs_execution_kernel_rejects_missing_steady_state_metadata(monkeypatch) -> None:
    settings = _settings()
    adapter = _adapter(settings)

    def fake_execute(payload):
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
        return {
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

    monkeypatch.setattr(adapter, "execute", fake_execute)

    with pytest.raises(RuntimeError, match="execution_kernel_delegate_authority"):
        asyncio.run(execute_router_rs_request(_request(), settings=settings, rust_adapter=adapter))


def test_router_rs_execution_kernel_rejects_legacy_delegate_first_metadata(monkeypatch) -> None:
    settings = _settings()
    adapter = _adapter(settings)

    def fake_execute(payload):
        legacy_metadata = _steady_state_kernel_metadata(
            run_id="run-legacy",
            status="completed",
            trace_event_count=payload["trace_event_count"],
            trace_output_path=payload["trace_output_path"],
            execution_mode="live",
            route_engine=payload["route_engine"],
            diagnostic_route_mode=payload["diagnostic_route_mode"],
        )
        legacy_metadata["execution_kernel"] = "router-rs"
        legacy_metadata["execution_kernel_authority"] = "rust-execution-cli"
        return {
            "execution_schema_version": "router-rs-execute-response-v1",
            "authority": "rust-execution-cli",
            "session_id": payload["session_id"],
            "user_id": payload["user_id"],
            "skill": payload["selected_skill"],
            "overlay": payload["overlay_skill"],
            "live_run": True,
            "content": "legacy router-rs content",
            "usage": {
                "input_tokens": 21,
                "output_tokens": 13,
                "total_tokens": 34,
                "mode": "live",
            },
            "prompt_preview": payload["prompt_preview"],
            "model_id": "gpt-5.4",
            "metadata": legacy_metadata,
        }

    monkeypatch.setattr(adapter, "execute", fake_execute)

    with pytest.raises(RuntimeError, match="execution_kernel='router-rs'"):
        asyncio.run(execute_router_rs_request(_request(), settings=settings, rust_adapter=adapter))


def test_router_rs_execution_kernel_can_follow_rust_metadata_bridge(monkeypatch) -> None:
    settings = _settings()
    adapter = _adapter(settings)
    kernel_contract = _kernel_contract(response_shape=EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY)
    metadata_bridge = _metadata_bridge(
        live_prompt_preview_owner="rust-router-preview-owner",
        live_primary_model_id_source="router-rs-upstream.model",
    )

    def fake_execute(payload):
        return {
            "execution_schema_version": "router-rs-execute-response-v1",
            "authority": "rust-execution-cli",
            "session_id": payload["session_id"],
            "user_id": payload["user_id"],
            "skill": payload["selected_skill"],
            "overlay": payload["overlay_skill"],
            "live_run": True,
            "content": "bridge-driven content",
            "usage": {
                "input_tokens": 21,
                "output_tokens": 13,
                "total_tokens": 34,
                "mode": "live",
            },
            "prompt_preview": payload["prompt_preview"],
            "model_id": "gpt-5.4",
            "metadata": _steady_state_kernel_metadata(
                run_id="run-bridge",
                status="completed",
                trace_event_count=payload["trace_event_count"],
                trace_output_path=payload["trace_output_path"],
                execution_mode="live",
                route_engine=payload["route_engine"],
                diagnostic_route_mode=payload["diagnostic_route_mode"],
                execution_kernel_prompt_preview_owner="rust-router-preview-owner",
                execution_kernel_model_id_source="router-rs-upstream.model",
            ),
        }

    monkeypatch.setattr(adapter, "execute", fake_execute)

    response = asyncio.run(
        execute_router_rs_request(
            _request(),
            settings=settings,
            rust_adapter=adapter,
            kernel_contract=kernel_contract,
            metadata_bridge=metadata_bridge,
        )
    )

    assert (
        response.metadata[EXECUTION_KERNEL_PROMPT_PREVIEW_OWNER_METADATA_KEY]
        == "rust-router-preview-owner"
    )
    assert (
        response.metadata[EXECUTION_KERNEL_MODEL_ID_SOURCE_METADATA_KEY]
        == "router-rs-upstream.model"
    )


def test_execution_kernel_contract_helpers_stay_rust_primary() -> None:
    trace_metadata = build_trace_runtime_metadata(
        trace_event_count=9,
        trace_output_path="/tmp/TRACE_METADATA.json",
    )
    runtime_metadata = build_execution_kernel_runtime_metadata(
        execution_kernel=EXECUTION_KERNEL_BRIDGE_KIND,
        execution_kernel_authority=EXECUTION_KERNEL_BRIDGE_AUTHORITY,
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
    assert runtime_metadata["execution_kernel"] == EXECUTION_KERNEL_BRIDGE_KIND
    assert runtime_metadata["execution_kernel_authority"] == EXECUTION_KERNEL_BRIDGE_AUTHORITY
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

    validated_contract = validate_execution_kernel_steady_state_metadata(
        metadata={
            field: runtime_metadata[field]
            for field in EXECUTION_KERNEL_RUST_CANONICAL_STEADY_STATE_METADATA_FIELDS
        },
        execution_kernel=EXECUTION_KERNEL_BRIDGE_KIND,
        execution_kernel_authority=EXECUTION_KERNEL_BRIDGE_AUTHORITY,
    )
    assert validated_contract["execution_kernel_response_shape"] == (
        EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN
    )


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
        execution_kernel=EXECUTION_KERNEL_BRIDGE_KIND,
        execution_kernel_authority=EXECUTION_KERNEL_BRIDGE_AUTHORITY,
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
    assert dry_run_response.metadata["execution_kernel"] == EXECUTION_KERNEL_BRIDGE_KIND
    assert dry_run_response.metadata["execution_kernel_authority"] == EXECUTION_KERNEL_BRIDGE_AUTHORITY
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


@pytest.mark.parametrize(
    ("metadata_field", "metadata_value"),
    [
        (
            EXECUTION_KERNEL_FALLBACK_REASON_METADATA_KEY,
            "legacy-python-fallback",
        ),
        (
            EXECUTION_KERNEL_COMPATIBILITY_AGENT_CONTRACT_METADATA_KEY,
            EXECUTION_KERNEL_COMPATIBILITY_AGENT_CONTRACT_VERSION,
        ),
        (
            EXECUTION_KERNEL_COMPATIBILITY_AGENT_KIND_METADATA_KEY,
            "python-agno-kernel-adapter",
        ),
        (
            EXECUTION_KERNEL_COMPATIBILITY_AGENT_AUTHORITY_METADATA_KEY,
            "python-agno-kernel-adapter",
        ),
    ],
)
def test_router_rs_execution_kernel_rejects_retired_python_fallback_metadata(
    monkeypatch,
    metadata_field: str,
    metadata_value: object,
) -> None:
    settings = _settings()
    adapter = _adapter(settings)

    def fake_execute(payload):
        metadata = _steady_state_kernel_metadata(
            run_id="run-legacy",
            status="completed",
            trace_event_count=payload["trace_event_count"],
            trace_output_path=payload["trace_output_path"],
            execution_mode="live",
            route_engine=payload["route_engine"],
            diagnostic_route_mode=payload["diagnostic_route_mode"],
        )
        metadata[metadata_field] = metadata_value
        return {
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
            "metadata": metadata,
        }

    monkeypatch.setattr(adapter, "execute", fake_execute)

    with pytest.raises(
        RouterRsInfrastructureError,
        match=rf"retired compatibility fallback field: {metadata_field}",
    ):
        asyncio.run(execute_router_rs_request(_request(), settings=settings, rust_adapter=adapter))
