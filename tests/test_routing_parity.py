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
from codex_agno_runtime.schemas import RouteExecutionPolicy
from scripts.route import (
    build_rust_router_command,
    route_decision_json,
    run_rust_route_json,
    run_rust_router_json,
    search_skills_json,
)


def _write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def _write_json(path: Path, payload: dict[str, object]) -> None:
    _write_text(path, json.dumps(payload, ensure_ascii=False, indent=2) + "\n")


def test_build_rust_router_command_emits_explicit_false_route_flags() -> None:
    command = build_rust_router_command(
        query="route me",
        limit=5,
        runtime_path=None,
        manifest_path=None,
        route_json=True,
        allow_overlay=False,
        first_turn=False,
    )

    assert "--allow-overlay=false" in command
    assert "--first-turn=false" in command


def test_run_rust_route_json_respects_false_overlay_and_first_turn_flags() -> None:
    decision = run_rust_route_json(
        "这个仓库的修复你直接 gsd，推进到底，别停，主线程保持简短并给我验证证据",
        session_id="route-cli-regression",
        allow_overlay=False,
        first_turn=False,
        runtime_path=PROJECT_ROOT / "tests" / "_routing_missing_runtime.json",
        manifest_path=PROJECT_ROOT / "tests" / "routing_route_fixtures.json",
    )

    assert decision["selected_skill"] == "execution-controller-coding"
    assert decision["overlay_skill"] is None
    assert all("Session-start" not in reason for reason in decision["reasons"])


def _seed_framework_runtime_artifacts(repo_root: Path, *, terminal: bool) -> None:
    task_id = (
        "checklist-series-final-closeout-20260418210000"
        if terminal
        else "active-bootstrap-repair-20260418210000"
    )
    task_root = repo_root / "artifacts" / "current" / task_id
    if terminal:
        summary_lines = [
            "- task: checklist-series final closeout",
            "- phase: finalized",
            "- status: completed",
        ]
        supervisor_state = {
            "task_id": task_id,
            "task_summary": "checklist-series final closeout",
            "active_phase": "finalized",
            "verification": {"verification_status": "completed"},
            "continuity": {"story_state": "completed", "resume_allowed": False},
            "execution_contract": {
                "goal": "Do not treat closeout as active continuity",
                "scope": ["memory/CLAUDE_MEMORY.md"],
            },
        }
        trace_metadata = {
            "task": "checklist-series final closeout",
            "matched_skills": ["checklist-fixer"],
        }
        next_actions = {
            "next_actions": ["Start a new standalone task before continuing related work"],
        }
    else:
        summary_lines = [
            "- task: active bootstrap repair",
            "- phase: implementation",
            "- status: in_progress",
        ]
        supervisor_state = {
            "task_id": task_id,
            "task_summary": "active bootstrap repair",
            "active_phase": "implementation",
            "verification": {"verification_status": "in_progress"},
            "continuity": {"story_state": "active", "resume_allowed": True},
            "primary_owner": "skill-developer-codex",
            "execution_contract": {
                "goal": "Repair stale bootstrap injection",
                "scope": ["scripts/memory_support.py"],
                "acceptance_criteria": [
                    "completed tasks never appear as current execution"
                ],
            },
            "blockers": {"open_blockers": ["Need regression coverage"]},
        }
        trace_metadata = {
            "task": "active bootstrap repair",
            "matched_skills": [
                "execution-controller-coding",
                "skill-developer-codex",
            ],
        }
        next_actions = {"next_actions": ["Patch classifier", "Run MCP regression tests"]}
    _write_text(task_root / "SESSION_SUMMARY.md", "\n".join(summary_lines) + "\n")
    _write_json(task_root / "NEXT_ACTIONS.json", next_actions)
    _write_json(task_root / "EVIDENCE_INDEX.json", {"artifacts": []})
    _write_json(task_root / "TRACE_METADATA.json", trace_metadata)
    _write_text(
        repo_root / "artifacts" / "current" / "SESSION_SUMMARY.md",
        "\n".join(summary_lines) + "\n",
    )
    _write_json(repo_root / "artifacts" / "current" / "NEXT_ACTIONS.json", next_actions)
    _write_json(
        repo_root / "artifacts" / "current" / "EVIDENCE_INDEX.json",
        {"artifacts": []},
    )
    _write_json(
        repo_root / "artifacts" / "current" / "TRACE_METADATA.json",
        trace_metadata,
    )
    _write_json(
        repo_root / "artifacts" / "current" / "active_task.json",
        {"task_id": task_id, "task": supervisor_state["task_summary"]},
    )
    _write_json(repo_root / ".supervisor_state.json", supervisor_state)


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
    ("mode", "expected"),
    [
        (
            "shadow",
            {
                "diagnostic_route_mode": "shadow",
                "primary_authority": "rust",
                "route_result_engine": "rust",
                "diagnostic_report_required": True,
                "strict_verification_required": False,
            },
        ),
        (
            "verify",
            {
                "diagnostic_route_mode": "verify",
                "primary_authority": "rust",
                "route_result_engine": "rust",
                "diagnostic_report_required": True,
                "strict_verification_required": True,
            },
        ),
        (
            "rust",
            {
                "diagnostic_route_mode": "none",
                "primary_authority": "rust",
                "route_result_engine": "rust",
                "diagnostic_report_required": False,
                "strict_verification_required": False,
            },
        ),
    ],
    ids=["shadow", "verify", "rust"],
)
def test_route_policy_mode_matrix_stays_rust_authoritative(
    mode: str,
    expected: dict[str, object],
) -> None:
    """Primary route-result policy should come from router-rs under the Rust-only contract."""

    adapter = RustRouteAdapter(PROJECT_ROOT)
    payload = adapter.route_policy(mode=mode)

    assert payload["policy_schema_version"] == adapter.route_policy_schema_version
    assert payload["authority"] == adapter.route_authority
    assert payload["mode"] == mode
    for key, value in expected.items():
        assert payload[key] == value
    assert payload["primary_authority"] == "rust"
    assert payload["route_result_engine"] == "rust"


def test_route_execution_policy_rejects_misaligned_rust_only_contract() -> None:
    """The Rust-only route policy validator should reject mismatched diagnostic semantics."""

    with pytest.raises(ValueError, match="rust route policy must disable diagnostic_route_mode"):
        RouteExecutionPolicy.model_validate(
            {
                "policy_schema_version": "router-rs-route-policy-v1",
                "authority": "rust-route-core",
                "mode": "rust",
                "diagnostic_route_mode": "shadow",
                "primary_authority": "rust",
                "route_result_engine": "rust",
                "diagnostic_report_required": False,
                "strict_verification_required": False,
            }
        )

    with pytest.raises(ValueError, match="shadow route policy must require report-only diagnostics"):
        RouteExecutionPolicy.model_validate(
            {
                "policy_schema_version": "router-rs-route-policy-v1",
                "authority": "rust-route-core",
                "mode": "shadow",
                "diagnostic_route_mode": "shadow",
                "primary_authority": "rust",
                "route_result_engine": "rust",
                "diagnostic_report_required": False,
                "strict_verification_required": False,
            }
        )


def test_route_report_contract_exposes_schema_and_rust_owned_snapshot_evidence() -> None:
    """The Rust diagnostic report should expose the Rust-only evidence contract."""

    adapter = RustRouteAdapter(PROJECT_ROOT)
    baseline = route_decision_json(
        "帮我写一个 Rust CLI 工具",
        session_id="route-report-contract-session",
        allow_overlay=True,
        first_turn=True,
    )["route_snapshot"]

    report = adapter.route_report(
        mode="shadow",
        rust_route_snapshot=baseline,
    )

    assert report["report_schema_version"] == adapter.route_report_schema_version
    assert report["authority"] == adapter.route_authority
    assert report["mode"] == "shadow"
    assert report["primary_engine"] == "rust"
    assert report["evidence_kind"] == "rust-owned-snapshot"
    assert report["strict_verification"] is False
    assert report["verification_passed"] is True
    assert report["route_snapshot"] == baseline


def test_route_report_verify_mode_requires_strict_verification() -> None:
    """Verify mode should mark the diagnostic report as strict Rust verification."""

    adapter = RustRouteAdapter(PROJECT_ROOT)
    baseline = route_decision_json(
        "帮我写一个 Rust CLI 工具",
        session_id="route-report-verify-session",
        allow_overlay=True,
        first_turn=True,
    )["route_snapshot"]

    report = adapter.route_report(
        mode="verify",
        rust_route_snapshot=baseline,
    )

    assert report["report_schema_version"] == adapter.route_report_schema_version
    assert report["authority"] == adapter.route_authority
    assert report["mode"] == "verify"
    assert report["primary_engine"] == "rust"
    assert report["evidence_kind"] == "rust-owned-snapshot"
    assert report["strict_verification"] is True
    assert report["verification_passed"] is True


def test_rust_route_adapter_framework_runtime_snapshot_reads_workspace_artifacts(
    tmp_path: Path,
) -> None:
    adapter = RustRouteAdapter(PROJECT_ROOT)
    _seed_framework_runtime_artifacts(tmp_path, terminal=False)

    snapshot = adapter.framework_runtime_snapshot(repo_root=tmp_path)

    assert snapshot["ok"] is True
    assert snapshot["workspace"] == tmp_path.name
    assert snapshot["continuity"]["state"] == "active"
    assert snapshot["continuity"]["current_execution"]["task"] == "active bootstrap repair"
    assert snapshot["supervisor_state"]["primary_owner"] == "skill-developer-codex"
    assert snapshot["paths"]["supervisor_state"].endswith(".supervisor_state.json")


def test_rust_route_adapter_framework_contract_summary_handles_completed_snapshot(
    tmp_path: Path,
) -> None:
    adapter = RustRouteAdapter(PROJECT_ROOT)
    _seed_framework_runtime_artifacts(tmp_path, terminal=True)

    summary = adapter.framework_contract_summary(repo_root=tmp_path)

    assert summary["ok"] is True
    assert summary["continuity"]["state"] == "completed"
    assert summary["goal"] is None
    assert summary["next_actions"] == []
    assert summary["recent_completed_execution"]["task"] == "checklist-series final closeout"


def test_rust_route_adapter_framework_runtime_snapshot_prefers_supervisor_owned_continuity(
    tmp_path: Path,
) -> None:
    adapter = RustRouteAdapter(PROJECT_ROOT)
    task_id = "active-bootstrap-repair-20260418210000"
    task_root = tmp_path / "artifacts" / "current" / task_id
    _write_json(
        tmp_path / "artifacts" / "current" / "active_task.json",
        {"task_id": task_id, "task": "active bootstrap repair"},
    )
    _write_text(
        task_root / "SESSION_SUMMARY.md",
        "\n".join(
            [
                "- task: active bootstrap repair",
                "- phase: implementation",
                "- status: in_progress",
            ]
        )
        + "\n",
    )
    _write_json(task_root / "NEXT_ACTIONS.json", {"next_actions": ["stale sidecar action"]})
    _write_json(task_root / "EVIDENCE_INDEX.json", {"artifacts": []})
    _write_json(
        task_root / "TRACE_METADATA.json",
        {
            "task": "active bootstrap repair",
            "matched_skills": ["legacy-skill"],
            "verification_status": "completed",
            "routing_runtime_version": 0,
        },
    )
    _write_json(
        tmp_path / ".supervisor_state.json",
        {
            "task_id": task_id,
            "task_summary": "active bootstrap repair",
            "active_phase": "implementation",
            "verification": {"verification_status": "in_progress"},
            "continuity": {"story_state": "active", "resume_allowed": True},
            "next_actions": [
                {
                    "title": "repair current continuity",
                    "status": "pending",
                }
            ],
            "controller": {
                "primary_owner": "execution-controller-coding",
                "gate": "subagent-delegation",
            },
        },
    )

    snapshot = adapter.framework_runtime_snapshot(repo_root=tmp_path)
    summary = adapter.framework_contract_summary(repo_root=tmp_path)

    assert snapshot["continuity"]["state"] == "active"
    assert snapshot["continuity"]["next_actions"] == ["repair current continuity"]
    assert snapshot["continuity"]["route"] == ["subagent-delegation", "execution-controller-coding"]
    assert snapshot["trace_skill_count"] == 2
    assert summary["next_actions"] == ["repair current continuity"]
    assert summary["trace_skills"] == ["subagent-delegation", "execution-controller-coding"]
