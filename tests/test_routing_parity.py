"""Parity tests for the Python bridge and Rust router JSON output."""

from __future__ import annotations

import json
import sys
from pathlib import Path

import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
SCRIPTS_ROOT = PROJECT_ROOT / "scripts"
if str(SCRIPTS_ROOT) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_ROOT))
RUNTIME_SRC = PROJECT_ROOT / "codex_agno_runtime" / "src"
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

from codex_agno_runtime.rust_router import RustRouteAdapter
from scripts.route import (
    route_decision_json,
    run_rust_route_json,
    run_rust_router_json,
    search_skills_json,
)


def test_rust_route_adapter_trace_descriptor_methods_validate_schema_and_authority(monkeypatch: pytest.MonkeyPatch) -> None:
    adapter = RustRouteAdapter(PROJECT_ROOT)
    expected = {
        "schema_version": adapter.trace_descriptor_schema_version,
        "authority": adapter.trace_descriptor_authority,
        "transport": {
            "schema_version": "runtime-event-transport-v1",
            "stream_id": "stream::session-1",
            "session_id": "session-1",
            "job_id": None,
            "transport_family": "host-facing-bridge",
            "transport_kind": "poll",
            "endpoint_kind": "runtime_method",
            "remote_capable": True,
            "remote_attach_supported": True,
            "handoff_supported": True,
            "handoff_method": "describe_runtime_event_handoff",
            "subscribe_method": "subscribe_runtime_events",
            "cleanup_method": "cleanup_runtime_events",
            "describe_method": "describe_runtime_event_transport",
            "handoff_kind": "artifact_handoff",
            "binding_refresh_mode": "describe_or_checkpoint",
            "binding_artifact_format": "json",
            "binding_backend_family": "filesystem",
            "binding_artifact_path": "/tmp/runtime_event_transports/session-1__session-1.json",
            "resume_mode": "after_event_id",
            "cleanup_semantics": "bridge_cache_only",
            "cleanup_preserves_replay": True,
            "replay_reseed_supported": True,
            "latest_cursor": {
                "schema_version": "runtime-trace-cursor-v1",
                "session_id": "session-1",
                "job_id": None,
                "generation": 0,
                "event_id": "evt-1",
                "seq": 1,
            },
            "attach_target": {
                "endpoint_kind": "runtime_method",
                "subscribe_method": "subscribe_runtime_events",
                "describe_method": "describe_runtime_event_transport",
                "cleanup_method": "cleanup_runtime_events",
                "handoff_method": "describe_runtime_event_handoff",
                "session_id": "session-1",
                "job_id": None,
            },
            "replay_anchor": {
                "anchor_kind": "trace_replay_cursor",
                "cursor_schema_version": "runtime-trace-cursor-v1",
                "resume_mode": "after_event_id",
                "latest_cursor": {
                    "schema_version": "runtime-trace-cursor-v1",
                    "session_id": "session-1",
                    "job_id": None,
                    "generation": 0,
                    "event_id": "evt-1",
                    "seq": 1,
                },
                "replay_supported": True,
            },
            "control_plane_authority": "rust-runtime-control-plane",
            "control_plane_role": "trace-and-handoff",
            "control_plane_projection": "python-thin-projection",
            "control_plane_delegate_kind": "filesystem-trace-store",
            "transport_health": {
                "backend_family": "filesystem",
                "supports_atomic_replace": True,
                "supports_compaction": False,
                "supports_snapshot_delta": False,
                "supports_remote_event_transport": True,
            },
        },
        "handoff": {
            "schema_version": "runtime-event-handoff-v1",
            "stream_id": "stream::session-1",
            "session_id": "session-1",
            "job_id": None,
            "checkpoint_backend_family": "filesystem",
            "trace_stream_path": "/tmp/TRACE_EVENTS.jsonl",
            "resume_manifest_path": "/tmp/TRACE_RESUME_MANIFEST.json",
            "remote_attach_strategy": "transport_descriptor_then_replay",
            "cleanup_preserves_replay": True,
            "attach_target": {
                "endpoint_kind": "runtime_method",
                "subscribe_method": "subscribe_runtime_events",
                "describe_method": "describe_runtime_event_transport",
                "cleanup_method": "cleanup_runtime_events",
                "handoff_method": "describe_runtime_event_handoff",
                "session_id": "session-1",
                "job_id": None,
            },
            "replay_anchor": {
                "anchor_kind": "trace_replay_cursor",
                "cursor_schema_version": "runtime-trace-cursor-v1",
                "resume_mode": "after_event_id",
                "latest_cursor": {
                    "schema_version": "runtime-trace-cursor-v1",
                    "session_id": "session-1",
                    "job_id": None,
                    "generation": 0,
                    "event_id": "evt-1",
                    "seq": 1,
                },
                "replay_supported": True,
            },
            "recovery_artifacts": [
                "/tmp/runtime_event_transports/session-1__session-1.json",
                "/tmp/TRACE_RESUME_MANIFEST.json",
                "/tmp/TRACE_EVENTS.jsonl",
            ],
            "control_plane": {
                "backend_family": "filesystem",
                "trace_service": {
                    "authority": "rust-runtime-control-plane",
                    "role": "trace-and-handoff",
                    "projection": "python-thin-projection",
                    "delegate_kind": "filesystem-trace-store",
                },
            },
            "transport": None,
        },
    }
    expected["handoff"]["transport"] = expected["transport"]

    def fake_run_json(command: list[str], *, failure_label: str) -> dict[str, object]:
        if "--describe-transport-json" in command:
            return {
                "schema_version": expected["schema_version"],
                "authority": expected["authority"],
                "transport": expected["transport"],
            }
        if "--describe-handoff-json" in command:
            return {
                "schema_version": expected["schema_version"],
                "authority": expected["authority"],
                "handoff": expected["handoff"],
            }
        raise AssertionError(f"unexpected command: {command}")

    monkeypatch.setattr(adapter, "_run_json_command", fake_run_json)

    transport = adapter.describe_transport(
        {
            "session_id": "session-1",
            "binding_backend_family": "filesystem",
            "binding_artifact_path": "/tmp/runtime_event_transports/session-1__session-1.json",
            "latest_cursor": {
                "schema_version": "runtime-trace-cursor-v1",
                "session_id": "session-1",
                "job_id": None,
                "generation": 0,
                "event_id": "evt-1",
                "seq": 1,
            },
            "control_plane": {
                "backend_family": "filesystem",
                "supports_atomic_replace": True,
                "supports_compaction": False,
                "supports_snapshot_delta": False,
                "supports_remote_event_transport": True,
                "trace_service": {
                    "authority": "rust-runtime-control-plane",
                    "role": "trace-and-handoff",
                    "projection": "python-thin-projection",
                    "delegate_kind": "filesystem-trace-store",
                },
            },
        }
    )
    handoff = adapter.describe_handoff(
        {
            "session_id": "session-1",
            "checkpoint_backend_family": "filesystem",
            "trace_stream_path": "/tmp/TRACE_EVENTS.jsonl",
            "resume_manifest_path": "/tmp/TRACE_RESUME_MANIFEST.json",
            "control_plane": {
                "backend_family": "filesystem",
                "trace_service": {
                    "authority": "rust-runtime-control-plane",
                    "role": "trace-and-handoff",
                    "projection": "python-thin-projection",
                    "delegate_kind": "filesystem-trace-store",
                },
            },
            "transport": expected["transport"],
        }
    )

    assert transport == expected["transport"]
    assert handoff == expected["handoff"]


def test_rust_route_adapter_checkpoint_resume_manifest_validates_contract(monkeypatch: pytest.MonkeyPatch) -> None:
    adapter = RustRouteAdapter(PROJECT_ROOT)
    manifest = {
        "schema_version": "runtime-resume-manifest-v1",
        "session_id": "session-1",
        "job_id": None,
        "status": "running",
        "generation": 3,
        "trace_output_path": "/tmp/TRACE_METADATA.json",
        "trace_stream_path": "/tmp/TRACE_EVENTS.jsonl",
        "event_transport_path": "/tmp/runtime_event_transports/session-1__session-1.json",
        "background_state_path": "/tmp/runtime_background_jobs.json",
        "latest_cursor": {
            "schema_version": "runtime-trace-cursor-v1",
            "session_id": "session-1",
            "job_id": None,
            "generation": 3,
            "event_id": "evt-9",
            "seq": 9,
        },
        "artifact_paths": [
            "/tmp/TRACE_METADATA.json",
            "/tmp/TRACE_EVENTS.jsonl",
            "/tmp/runtime_event_transports/session-1__session-1.json",
        ],
        "supervisor_projection": {
            "supervisor_state_path": "/tmp/.supervisor_state.json",
            "active_phase": "validated",
            "verification_status": "completed",
        },
        "control_plane": {
            "backend_family": "filesystem",
            "trace_service": {
                "authority": "rust-runtime-control-plane",
                "role": "trace-and-handoff",
                "projection": "python-thin-projection",
                "delegate_kind": "filesystem-trace-store",
            },
        },
    }

    def fake_run_json(command: list[str], *, failure_label: str) -> dict[str, object]:
        assert "--checkpoint-resume-manifest-json" in command
        return {
            "schema_version": adapter.checkpoint_resume_manifest_schema_version,
            "authority": adapter.checkpoint_resume_manifest_authority,
            "resume_manifest": manifest,
        }

    monkeypatch.setattr(adapter, "_run_json_command", fake_run_json)

    assert adapter.checkpoint_resume_manifest(
        {
            "session_id": "session-1",
            "status": "running",
            "generation": 3,
            "trace_output_path": "/tmp/TRACE_METADATA.json",
            "trace_stream_path": "/tmp/TRACE_EVENTS.jsonl",
            "event_transport_path": "/tmp/runtime_event_transports/session-1__session-1.json",
            "background_state_path": "/tmp/runtime_background_jobs.json",
            "latest_cursor": {
                "schema_version": "runtime-trace-cursor-v1",
                "session_id": "session-1",
                "job_id": None,
                "generation": 3,
                "event_id": "evt-9",
                "seq": 9,
            },
            "artifact_paths": [
                "/tmp/TRACE_METADATA.json",
                "/tmp/TRACE_EVENTS.jsonl",
                "/tmp/runtime_event_transports/session-1__session-1.json",
            ],
            "supervisor_projection": {
                "supervisor_state_path": "/tmp/.supervisor_state.json",
                "active_phase": "validated",
                "verification_status": "completed",
            },
            "control_plane": {
                "backend_family": "filesystem",
                "trace_service": {
                    "authority": "rust-runtime-control-plane",
                    "role": "trace-and-handoff",
                    "projection": "python-thin-projection",
                    "delegate_kind": "filesystem-trace-store",
                },
            },
        }
    ) == manifest


def test_rust_route_adapter_write_methods_validate_schema_authority_and_ack(monkeypatch: pytest.MonkeyPatch) -> None:
    adapter = RustRouteAdapter(PROJECT_ROOT)
    transport_ack = {
        "schema_version": adapter.transport_binding_write_schema_version,
        "authority": adapter.transport_binding_write_authority,
        "path": "/tmp/runtime_event_transports/session-1__session-1.json",
        "bytes_written": 512,
    }
    manifest_ack = {
        "schema_version": adapter.checkpoint_manifest_write_schema_version,
        "authority": adapter.checkpoint_manifest_write_authority,
        "path": "/tmp/TRACE_RESUME_MANIFEST.json",
        "bytes_written": 768,
    }

    def fake_run_json(command: list[str], *, failure_label: str) -> dict[str, object]:
        if "--write-transport-binding-json" in command:
            return transport_ack
        if "--write-checkpoint-resume-manifest-json" in command:
            return manifest_ack
        raise AssertionError(f"unexpected command: {command}")

    monkeypatch.setattr(adapter, "_run_json_command", fake_run_json)

    assert adapter.write_transport_binding(
        {
            "path": transport_ack["path"],
            "session_id": "session-1",
            "binding_artifact_path": transport_ack["path"],
        }
    ) == transport_ack
    assert adapter.write_checkpoint_resume_manifest(
        {
            "path": manifest_ack["path"],
            "session_id": "session-1",
            "status": "running",
        }
    ) == manifest_ack

ROUTE_FIXTURE_PATH = PROJECT_ROOT / "tests" / "routing_route_fixtures.json"
MISSING_RUNTIME_PATH = PROJECT_ROOT / "tests" / "_routing_missing_runtime.json"
ROUTE_FIXTURES = json.loads(ROUTE_FIXTURE_PATH.read_text(encoding="utf-8"))
REAL_TASK_REPLAY_QUERIES = [
    "这是高负载跨文件任务，需要 sidecar delegation 并行处理",
    "帮我写一个 Rust CLI 工具",
    "把这份 runtime checklist 落成代码，并保留 Python host 接口",
    "review checklist 看这轮是否结束",
    "这个 skill 框架 owner gate overlay 边界重叠了，顺手把路由策略修一下并减少 token 消耗。",
    "帮我看 OpenAI Responses API 最新官方文档并说明怎么用。",
]


@pytest.mark.parametrize(
    ("query", "limit"),
    [
        ("自迭代 10轮 优化 验证", 3),
        ("agent 长期记忆 跨会话 memory layer", 3),
        ("Mac 桌面 app 原生 调试 wkwebview ipc", 3),
        ("github 深度 调研 issue PR 演化分析", 5),
    ],
)
def test_rust_router_json_matches_python_search_json(query: str, limit: int) -> None:
    """Verify the Rust router and Python bridge produce the same JSON payload."""

    assert run_rust_router_json(query, limit=limit) == search_skills_json(query, limit=limit)


@pytest.mark.parametrize(
    "case",
    ROUTE_FIXTURES["cases"],
    ids=[case["name"] for case in ROUTE_FIXTURES["cases"]],
)
def test_rust_route_json_matches_python_route_decision(case: dict[str, object]) -> None:
    """Verify final route decision parity between Python and Rust."""

    query = str(case["query"])
    allow_overlay = bool(case.get("allow_overlay", True))
    first_turn = bool(case.get("first_turn", True))

    python_decision = route_decision_json(
        query,
        session_id="fixture-session",
        allow_overlay=allow_overlay,
        first_turn=first_turn,
        runtime_path=MISSING_RUNTIME_PATH,
        manifest_path=ROUTE_FIXTURE_PATH,
    )
    rust_decision = run_rust_route_json(
        query,
        session_id="fixture-session",
        allow_overlay=allow_overlay,
        first_turn=first_turn,
        runtime_path=MISSING_RUNTIME_PATH,
        manifest_path=ROUTE_FIXTURE_PATH,
    )

    assert rust_decision == python_decision

    expected = case["expected"]
    assert rust_decision["selected_skill"] == expected["selected_skill"]
    assert rust_decision["overlay_skill"] == expected["overlay_skill"]
    assert rust_decision["layer"] == expected["layer"]


@pytest.mark.parametrize("query", REAL_TASK_REPLAY_QUERIES)
def test_real_task_replay_queries_match_shadow_diff_fields(query: str) -> None:
    """Real-task replay queries should keep the stable shadow diff vocabulary aligned."""

    python_decision = route_decision_json(
        query,
        session_id="shadow-replay-session",
        allow_overlay=True,
        first_turn=True,
    )
    rust_decision = run_rust_route_json(
        query,
        session_id="shadow-replay-session",
        allow_overlay=True,
        first_turn=True,
    )

    assert rust_decision["selected_skill"] == python_decision["selected_skill"]
    assert rust_decision["overlay_skill"] == python_decision["overlay_skill"]
    assert rust_decision["layer"] == python_decision["layer"]
    assert rust_decision["route_snapshot"]["score_bucket"] == python_decision["route_snapshot"]["score_bucket"]
    assert rust_decision["route_snapshot"]["reasons_class"] == python_decision["route_snapshot"]["reasons_class"]


@pytest.mark.parametrize(
    ("query", "selected_skill", "overlay_skill"),
    [
        (
            "深度review现在的路由系统和 skill 边界。",
            "skill-developer-codex",
            "code-review",
        ),
        (
            "framework-review",
            "skill-developer-codex",
            "code-review",
        ),
        (
            "帮我看 OpenAI Responses API 最新官方文档并说明怎么用。",
            "openai-docs",
            "anti-laziness",
        ),
    ],
)
def test_live_route_expectations_hold_for_framework_and_openai_queries(
    query: str,
    selected_skill: str,
    overlay_skill: str | None,
) -> None:
    """Framework-review and OpenAI-doc queries should keep stable live routing."""

    python_decision = route_decision_json(
        query,
        session_id="live-expectation-session",
        allow_overlay=True,
        first_turn=True,
    )
    rust_decision = run_rust_route_json(
        query,
        session_id="live-expectation-session",
        allow_overlay=True,
        first_turn=True,
    )

    assert rust_decision == python_decision
    assert rust_decision["selected_skill"] == selected_skill
    assert rust_decision["overlay_skill"] == overlay_skill


@pytest.mark.parametrize(
    ("mode", "rollback_to_python", "expected"),
    [
        (
            "python",
            False,
            {
                "primary_authority": "python",
                "route_result_engine": "python",
                "shadow_engine": None,
                "diff_report_required": False,
                "verify_parity_required": False,
                "rollback_active": False,
                "diagnostic_python_lane": False,
            },
        ),
        (
            "shadow",
            False,
            {
                "primary_authority": "rust",
                "route_result_engine": "rust",
                "shadow_engine": "python",
                "diff_report_required": True,
                "verify_parity_required": False,
                "rollback_active": False,
                "diagnostic_python_lane": True,
            },
        ),
        (
            "verify",
            False,
            {
                "primary_authority": "rust",
                "route_result_engine": "rust",
                "shadow_engine": "python",
                "diff_report_required": True,
                "verify_parity_required": True,
                "rollback_active": False,
                "diagnostic_python_lane": True,
            },
        ),
        (
            "rust",
            False,
            {
                "primary_authority": "rust",
                "route_result_engine": "rust",
                "shadow_engine": None,
                "diff_report_required": False,
                "verify_parity_required": False,
                "rollback_active": False,
                "diagnostic_python_lane": False,
            },
        ),
        (
            "rust",
            True,
            {
                "primary_authority": "rust",
                "route_result_engine": "rust",
                "shadow_engine": "python",
                "diff_report_required": True,
                "verify_parity_required": False,
                "rollback_active": True,
                "diagnostic_python_lane": True,
            },
        ),
    ],
    ids=["python", "shadow", "verify", "rust", "rust-rollback"],
)
def test_route_policy_mode_matrix_stays_rust_authoritative(
    mode: str,
    rollback_to_python: bool,
    expected: dict[str, object],
) -> None:
    """Primary route-result policy should come from router-rs, not Python-side recompute."""

    adapter = RustRouteAdapter(PROJECT_ROOT)
    payload = adapter.route_policy(mode=mode, rollback_to_python=rollback_to_python)

    assert payload["policy_schema_version"] == adapter.route_policy_schema_version
    assert payload["authority"] == adapter.route_authority
    assert payload["mode"] == mode
    for key, value in expected.items():
        assert payload[key] == value
    if payload["diff_report_required"] or payload["verify_parity_required"]:
        assert payload["diagnostic_python_lane"] is True
    if payload["verify_parity_required"]:
        assert payload["diff_report_required"] is True
    if payload["diagnostic_python_lane"]:
        assert payload["shadow_engine"] == "python"
    if payload["rollback_active"]:
        assert payload["primary_authority"] == "rust"
        assert payload["route_result_engine"] == "rust"


def test_route_report_contract_exposes_schema_and_stable_mismatch_vocabulary() -> None:
    """The Rust diff report should own schema, authority, and diagnostic evidence vocabulary."""

    adapter = RustRouteAdapter(PROJECT_ROOT)
    baseline = route_decision_json(
        "帮我写一个 Rust CLI 工具",
        session_id="route-report-contract-session",
        allow_overlay=True,
        first_turn=True,
    )["route_snapshot"]
    rust_snapshot = dict(baseline)
    rust_snapshot["selected_skill"] = "python-pro"
    rust_snapshot["score_bucket"] = "40-49"

    report = adapter.route_report(
        mode="shadow",
        python_route_snapshot=baseline,
        rust_route_snapshot=rust_snapshot,
        rollback_active=False,
    )

    assert report["report_schema_version"] == adapter.route_report_schema_version
    assert report["authority"] == adapter.route_authority
    assert report["mode"] == "shadow"
    assert report["primary_engine"] == "rust"
    assert report["shadow_engine"] == "python"
    assert report["rollback_active"] is False
    assert report["mismatch"] is True
    assert report["mismatch_fields"] == ["selected_skill", "score_bucket"]


def test_route_report_rollback_lane_keeps_rust_primary_engine() -> None:
    """Rollback evidence should stay diagnostic and preserve Rust as the live engine."""

    adapter = RustRouteAdapter(PROJECT_ROOT)
    baseline = route_decision_json(
        "帮我写一个 Rust CLI 工具",
        session_id="route-report-rollback-session",
        allow_overlay=True,
        first_turn=True,
    )["route_snapshot"]

    report = adapter.route_report(
        mode="rust",
        python_route_snapshot=baseline,
        rust_route_snapshot=baseline,
        rollback_active=True,
    )

    assert report["report_schema_version"] == adapter.route_report_schema_version
    assert report["authority"] == adapter.route_authority
    assert report["mode"] == "rust"
    assert report["primary_engine"] == "rust"
    assert report["shadow_engine"] == "python"
    assert report["rollback_active"] is True
    assert report["mismatch"] is False
