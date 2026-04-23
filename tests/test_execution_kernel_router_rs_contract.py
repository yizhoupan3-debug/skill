from __future__ import annotations

import asyncio
import os
import sys
import tempfile
from pathlib import Path
from types import SimpleNamespace

import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = PROJECT_ROOT / "framework_runtime" / "src"
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

from framework_runtime.execution_kernel import (
    ExecutionKernelRequest,
    RouterRsInfrastructureError,
    build_router_rs_execution_request_payload,
    decode_router_rs_execution_payload,
    execute_router_rs_request,
    preview_router_rs_request_prompt,
)
from framework_runtime.execution_kernel_contracts import (
    DRY_RUN_REQUIRED_RUNTIME_METADATA_FIELDS,
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
    LIVE_PRIMARY_REQUIRED_RUNTIME_METADATA_FIELDS,
    LIVE_PRIMARY_MODEL_ID_SOURCE,
    LIVE_PRIMARY_PASSTHROUGH_RUNTIME_METADATA_FIELDS,
    LIVE_PRIMARY_PROMPT_PREVIEW_OWNER,
    build_execution_kernel_live_response_serialization_contract_core,
    build_execution_kernel_runtime_metadata,
    build_trace_runtime_metadata,
    validate_router_rs_execution_metadata,
    validate_execution_kernel_steady_state_metadata,
)
from framework_runtime.rust_router import RustRouteAdapter
from framework_runtime.schemas import RoutingResult, SkillMetadata


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


def _steady_state_kernel_contract(
    *,
    response_shape: str = EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY,
    execution_kernel: str = EXECUTION_KERNEL_BRIDGE_KIND,
    execution_kernel_authority: str = EXECUTION_KERNEL_BRIDGE_AUTHORITY,
    execution_kernel_delegate: str = "router-rs",
    execution_kernel_delegate_authority: str = "rust-execution-cli",
    execution_kernel_delegate_family: str = "rust-cli",
    execution_kernel_delegate_impl: str = "router-rs",
    execution_kernel_live_primary: str | None = None,
    execution_kernel_live_primary_authority: str | None = None,
) -> dict[str, object]:
    metadata = build_execution_kernel_runtime_metadata(
        execution_kernel=execution_kernel,
        execution_kernel_authority=execution_kernel_authority,
        execution_kernel_delegate=execution_kernel_delegate,
        execution_kernel_delegate_authority=execution_kernel_delegate_authority,
        execution_kernel_delegate_family=execution_kernel_delegate_family,
        execution_kernel_delegate_impl=execution_kernel_delegate_impl,
        response_shape=response_shape,
        trace_event_count=0,
        trace_output_path=None,
        extra_fields={
            "execution_kernel_live_primary": (
                execution_kernel_live_primary or execution_kernel_delegate
            ),
            "execution_kernel_live_primary_authority": (
                execution_kernel_live_primary_authority
                or execution_kernel_delegate_authority
            ),
        },
    )
    return {
        field: metadata[field]
        for field in EXECUTION_KERNEL_STEADY_STATE_METADATA_FIELDS
    }


def _default_metadata_bridge() -> dict[str, object]:
    return {
        "steady_state_fields": [*EXECUTION_KERNEL_STEADY_STATE_METADATA_FIELDS],
        "runtime_fields": {
            "shared": ["trace_event_count", "trace_output_path"],
            "live_primary_required": [*LIVE_PRIMARY_REQUIRED_RUNTIME_METADATA_FIELDS],
            "live_primary_passthrough": [*LIVE_PRIMARY_PASSTHROUGH_RUNTIME_METADATA_FIELDS],
            "dry_run_required": [*DRY_RUN_REQUIRED_RUNTIME_METADATA_FIELDS],
        },
        "metadata_keys": {
            "metadata_schema_version": EXECUTION_KERNEL_METADATA_SCHEMA_VERSION_METADATA_KEY,
            "contract_mode": EXECUTION_KERNEL_CONTRACT_MODE_METADATA_KEY,
            "fallback_policy": EXECUTION_KERNEL_FALLBACK_POLICY_METADATA_KEY,
            "response_shape": EXECUTION_KERNEL_RESPONSE_SHAPE_METADATA_KEY,
            "prompt_preview_owner": EXECUTION_KERNEL_PROMPT_PREVIEW_OWNER_METADATA_KEY,
            "model_id_source": EXECUTION_KERNEL_MODEL_ID_SOURCE_METADATA_KEY,
        },
        "defaults": {
            "contract_mode": EXECUTION_KERNEL_RUST_PRIMARY_CONTRACT_MODE,
            "fallback_policy": EXECUTION_KERNEL_COMPATIBILITY_FALLBACK_POLICY,
            "prompt_preview_owner_by_mode": {
                EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY: LIVE_PRIMARY_PROMPT_PREVIEW_OWNER,
                EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN: LIVE_PRIMARY_PROMPT_PREVIEW_OWNER,
            },
            "live_primary_model_id_source": LIVE_PRIMARY_MODEL_ID_SOURCE,
            "supported_response_shapes": [
                EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY,
                EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN,
            ],
        },
    }


def _runtime_control_plane_payload(
    *,
    live_contract: dict[str, object] | None = None,
    dry_run_contract: dict[str, object] | None = None,
    metadata_bridge: dict[str, object] | None = None,
) -> dict[str, object]:
    return {
        "schema_version": RustRouteAdapter.runtime_control_plane_schema_version,
        "authority": RustRouteAdapter.runtime_control_plane_authority,
        "services": {
            "execution": {
                "kernel_metadata_bridge": metadata_bridge or _default_metadata_bridge(),
                "kernel_contract_by_mode": {
                    EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY: (
                        live_contract
                        or _steady_state_kernel_contract(
                            response_shape=EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY
                        )
                    ),
                    EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN: (
                        dry_run_contract
                        or _steady_state_kernel_contract(
                            response_shape=EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN
                        )
                    ),
                },
            }
        },
    }


def _metadata_bridge_with_trace_generation() -> dict[str, object]:
    return {
        "steady_state_fields": [*EXECUTION_KERNEL_STEADY_STATE_METADATA_FIELDS],
        "runtime_fields": {
            "shared": [*build_trace_runtime_metadata(trace_event_count=0, trace_output_path="").keys(), "trace_generation"],
            "live_primary_required": [*LIVE_PRIMARY_REQUIRED_RUNTIME_METADATA_FIELDS, "trace_generation"],
            "live_primary_passthrough": [*LIVE_PRIMARY_PASSTHROUGH_RUNTIME_METADATA_FIELDS, "trace_generation"],
            "dry_run_required": [*DRY_RUN_REQUIRED_RUNTIME_METADATA_FIELDS, "trace_generation"],
        },
        "metadata_keys": {
            "metadata_schema_version": EXECUTION_KERNEL_METADATA_SCHEMA_VERSION_METADATA_KEY,
            "contract_mode": EXECUTION_KERNEL_CONTRACT_MODE_METADATA_KEY,
            "fallback_policy": EXECUTION_KERNEL_FALLBACK_POLICY_METADATA_KEY,
            "response_shape": EXECUTION_KERNEL_RESPONSE_SHAPE_METADATA_KEY,
            "prompt_preview_owner": EXECUTION_KERNEL_PROMPT_PREVIEW_OWNER_METADATA_KEY,
            "model_id_source": EXECUTION_KERNEL_MODEL_ID_SOURCE_METADATA_KEY,
        },
        "defaults": {
            "contract_mode": EXECUTION_KERNEL_RUST_PRIMARY_CONTRACT_MODE,
            "fallback_policy": EXECUTION_KERNEL_COMPATIBILITY_FALLBACK_POLICY,
            "prompt_preview_owner_by_mode": {
                EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY: LIVE_PRIMARY_PROMPT_PREVIEW_OWNER,
                EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN: LIVE_PRIMARY_PROMPT_PREVIEW_OWNER,
            },
            "live_primary_model_id_source": LIVE_PRIMARY_MODEL_ID_SOURCE,
            "supported_response_shapes": [
                EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY,
                EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN,
            ],
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
    monkeypatch.setattr(adapter, "runtime_control_plane", _runtime_control_plane_payload)

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
        == "rust-execution-cli"
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


def test_router_rs_execution_request_prefers_rust_route_snapshot_reasons() -> None:
    settings = _settings()
    request = _request(dry_run=True)
    request.routing_result = request.routing_result.model_copy(
        update={
            "reasons": [
                "Trigger phrase matched: 直接做代码.",
                "Python router executed only as a thin compatibility projection under the Rust control plane.",
            ],
            "route_snapshot": {
                "engine": "rust",
                "selected_skill": "plan-to-code",
                "overlay_skill": "rust-pro",
                "layer": "L2",
                "score": 88.0,
                "score_bucket": "80-89",
                "reasons": ["Trigger phrase matched: 直接做代码."],
                "reasons_class": "direct-match",
            },
        }
    )

    payload = build_router_rs_execution_request_payload(request, settings=settings)

    assert payload["reasons"] == ["Trigger phrase matched: 直接做代码."]


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
    monkeypatch.setattr(adapter, "runtime_control_plane", _runtime_control_plane_payload)

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
    monkeypatch.setattr(adapter, "runtime_control_plane", _runtime_control_plane_payload)

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
    monkeypatch.setattr(adapter, "runtime_control_plane", _runtime_control_plane_payload)

    with pytest.raises(RuntimeError, match="execution_kernel='router-rs'"):
        asyncio.run(
            execute_router_rs_request(
                _request(),
                settings=settings,
                rust_adapter=adapter,
            )
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
    assert contract_core["runtime_response_metadata_fields"]["steady_state_kernel"] == [
        *EXECUTION_KERNEL_STEADY_STATE_METADATA_FIELDS
    ]
    assert "retired_compatibility_fallback" not in contract_core["runtime_response_metadata_fields"]

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


def test_execution_kernel_contract_core_can_follow_bridge_runtime_fields() -> None:
    bridge = _metadata_bridge_with_trace_generation()

    contract_core = build_execution_kernel_live_response_serialization_contract_core(
        metadata_bridge=bridge
    )

    assert contract_core["runtime_response_metadata_fields"]["shared"] == [
        "trace_event_count",
        "trace_output_path",
        "trace_generation",
    ]
    assert contract_core["runtime_response_metadata_fields"]["live_primary"] == [
        "run_id",
        "status",
        *LIVE_PRIMARY_PASSTHROUGH_RUNTIME_METADATA_FIELDS,
        "trace_generation",
        EXECUTION_KERNEL_MODEL_ID_SOURCE_METADATA_KEY,
    ]
    assert contract_core["current_response_shape_truth"]["live_primary"][
        "pass_through_metadata_fields"
    ] == [
        *LIVE_PRIMARY_PASSTHROUGH_RUNTIME_METADATA_FIELDS,
        "trace_generation",
    ]
    assert contract_core["current_response_shape_truth"]["dry_run"][
        "required_metadata_fields"
    ] == [
        *EXECUTION_KERNEL_STEADY_STATE_METADATA_FIELDS,
        *DRY_RUN_REQUIRED_RUNTIME_METADATA_FIELDS,
        "trace_generation",
    ]


def test_validate_router_rs_execution_metadata_uses_bridge_runtime_fields() -> None:
    bridge = _metadata_bridge_with_trace_generation()
    metadata = _steady_state_kernel_metadata(
        run_id="run-bridge",
        status="completed",
        trace_event_count=9,
        trace_output_path="/tmp/TRACE_METADATA.json",
        trace_generation=3,
        execution_mode="live",
        route_engine="rust",
        diagnostic_route_mode="none",
    )

    validated = validate_router_rs_execution_metadata(
        metadata=metadata,
        live_run=True,
        usage_mode="live",
        execution_kernel=EXECUTION_KERNEL_BRIDGE_KIND,
        execution_kernel_authority=EXECUTION_KERNEL_BRIDGE_AUTHORITY,
        metadata_bridge=bridge,
    )
    assert validated["trace_generation"] == 3

    metadata.pop("trace_generation")
    with pytest.raises(RuntimeError, match="trace_generation"):
        validate_router_rs_execution_metadata(
            metadata=metadata,
            live_run=True,
            usage_mode="live",
            execution_kernel=EXECUTION_KERNEL_BRIDGE_KIND,
            execution_kernel_authority=EXECUTION_KERNEL_BRIDGE_AUTHORITY,
            metadata_bridge=bridge,
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
    monkeypatch.setattr(adapter, "runtime_control_plane", _runtime_control_plane_payload)

    with pytest.raises(
        RouterRsInfrastructureError,
        match=rf"retired compatibility fallback field: {metadata_field}",
    ):
        asyncio.run(execute_router_rs_request(_request(), settings=settings, rust_adapter=adapter))


def test_decode_router_rs_execution_payload_rejects_runtime_renaming_without_contract() -> None:
    payload = {
        "session_id": "kernel-contract-session",
        "user_id": "tester",
        "skill": "plan-to-code",
        "overlay": "rust-pro",
        "live_run": True,
        "content": "router-rs content",
        "usage": {
            "input_tokens": 21,
            "output_tokens": 13,
            "total_tokens": 34,
            "mode": "live",
        },
        "prompt_preview": "Rust-owned live prompt",
        "model_id": "gpt-5.4",
        "metadata": _steady_state_kernel_metadata(
            execution_kernel="rust-runtime-owned-kernel",
            execution_kernel_authority="rust-runtime-owned-authority",
            execution_kernel_delegate="router-rs-live",
            execution_kernel_delegate_authority="rust-runtime-live-authority",
            execution_kernel_delegate_family="rust-direct-live",
            execution_kernel_delegate_impl="router-rs-http",
            execution_kernel_live_primary="router-rs-live-primary",
            execution_kernel_live_primary_authority="rust-primary-authority",
            run_id="run-1",
            status="completed",
            trace_event_count=9,
            trace_output_path="/tmp/TRACE_METADATA.json",
            execution_mode="live",
            route_engine="rust",
            diagnostic_route_mode="none",
        ),
    }

    with pytest.raises(RuntimeError, match="execution_kernel='rust-runtime-owned-kernel'"):
        decode_router_rs_execution_payload(payload)


def test_decode_router_rs_execution_payload_accepts_runtime_contract_bundle() -> None:
    live_contract = _steady_state_kernel_contract(
        response_shape=EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY,
        execution_kernel="rust-runtime-owned-kernel",
        execution_kernel_authority="rust-runtime-owned-authority",
        execution_kernel_delegate="router-rs-live",
        execution_kernel_delegate_authority="rust-runtime-live-authority",
        execution_kernel_delegate_family="rust-direct-live",
        execution_kernel_delegate_impl="router-rs-http",
        execution_kernel_live_primary="router-rs-live-primary",
        execution_kernel_live_primary_authority="rust-primary-authority",
    )
    payload = {
        "session_id": "kernel-contract-session",
        "user_id": "tester",
        "skill": "plan-to-code",
        "overlay": "rust-pro",
        "live_run": True,
        "content": "router-rs content",
        "usage": {
            "input_tokens": 21,
            "output_tokens": 13,
            "total_tokens": 34,
            "mode": "live",
        },
        "prompt_preview": "Rust-owned live prompt",
        "model_id": "gpt-5.4",
        "metadata": _steady_state_kernel_metadata(
            execution_kernel="rust-runtime-owned-kernel",
            execution_kernel_authority="rust-runtime-owned-authority",
            execution_kernel_delegate="router-rs-live",
            execution_kernel_delegate_authority="rust-runtime-live-authority",
            execution_kernel_delegate_family="rust-direct-live",
            execution_kernel_delegate_impl="router-rs-http",
            execution_kernel_live_primary="router-rs-live-primary",
            execution_kernel_live_primary_authority="rust-primary-authority",
            run_id="run-1",
            status="completed",
            trace_event_count=9,
            trace_output_path="/tmp/TRACE_METADATA.json",
            execution_mode="live",
            route_engine="rust",
            diagnostic_route_mode="none",
        ),
    }

    response = decode_router_rs_execution_payload(
        payload,
        kernel_contract=live_contract,
        metadata_bridge=_default_metadata_bridge(),
    )

    assert response.metadata["execution_kernel"] == "rust-runtime-owned-kernel"
    assert response.metadata["execution_kernel_authority"] == "rust-runtime-owned-authority"
    assert response.metadata["execution_kernel_delegate_impl"] == "router-rs-http"


def test_execute_router_rs_request_prefers_runtime_control_plane_contract_bundle(monkeypatch) -> None:
    settings = _settings()
    adapter = _adapter(settings)
    live_contract = _steady_state_kernel_contract(
        response_shape=EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY,
        execution_kernel="rust-runtime-owned-kernel",
        execution_kernel_authority="rust-runtime-owned-authority",
        execution_kernel_delegate="router-rs-live",
        execution_kernel_delegate_authority="rust-runtime-live-authority",
        execution_kernel_delegate_family="rust-direct-live",
        execution_kernel_delegate_impl="router-rs-http",
        execution_kernel_live_primary="router-rs-live-primary",
        execution_kernel_live_primary_authority="rust-primary-authority",
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
            "content": "runtime-owned live result",
            "usage": {
                "input_tokens": 21,
                "output_tokens": 13,
                "total_tokens": 34,
                "mode": "live",
            },
            "prompt_preview": payload["prompt_preview"],
            "model_id": "gpt-5.4",
            "metadata": _steady_state_kernel_metadata(
                execution_kernel="rust-runtime-owned-kernel",
                execution_kernel_authority="rust-runtime-owned-authority",
                execution_kernel_delegate="router-rs-live",
                execution_kernel_delegate_authority="rust-runtime-live-authority",
                execution_kernel_delegate_family="rust-direct-live",
                execution_kernel_delegate_impl="router-rs-http",
                execution_kernel_live_primary="router-rs-live-primary",
                execution_kernel_live_primary_authority="rust-primary-authority",
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
    monkeypatch.setattr(
        adapter,
        "runtime_control_plane",
        lambda: _runtime_control_plane_payload(live_contract=live_contract),
    )

    response = asyncio.run(execute_router_rs_request(_request(), settings=settings, rust_adapter=adapter))

    assert response.content == "runtime-owned live result"
    assert response.metadata["execution_kernel"] == "rust-runtime-owned-kernel"
    assert response.metadata["execution_kernel_delegate"] == "router-rs-live"
