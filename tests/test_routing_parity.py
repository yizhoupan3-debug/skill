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

import codex_agno_runtime.rust_router as rust_router_module

from codex_agno_runtime.rust_router import RustRouteAdapter, route_adapter, route_decision_contract, search_skills
from codex_agno_runtime.schemas import (
    RouteDecisionContract,
    RouteDecisionSnapshot,
    RouteDiagnosticReport,
    RouteExecutionPolicy,
    SearchMatchesContract,
    SearchMatchResult,
    SkillMetadata,
)


def _write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def _write_json(path: Path, payload: dict[str, object]) -> None:
    _write_text(path, json.dumps(payload, ensure_ascii=False, indent=2) + "\n")


def _fixture_route_adapter() -> RustRouteAdapter:
    return RustRouteAdapter(
        PROJECT_ROOT,
        runtime_path=MISSING_RUNTIME_PATH,
        manifest_path=ROUTE_FIXTURE_PATH,
    )


def _live_route_adapter() -> RustRouteAdapter:
    return RustRouteAdapter(PROJECT_ROOT)


ROUTE_DECISION_KNOB_CASES = [
    (
        "深度review现在的路由系统和 skill 边界。",
        "skill-developer-codex",
        None,
        "L0",
        False,
        True,
    ),
]


def _fallback_route_contract_payload(*, adapter: RustRouteAdapter) -> dict[str, object]:
    return {
        "decision_schema_version": adapter.route_decision_schema_version,
        "authority": adapter.route_authority,
        "compile_authority": adapter.compile_authority,
        "task": "route adapter fallback regression",
        "session_id": "fallback-regression-session",
        "selected_skill": "execution-controller-coding",
        "overlay_skill": None,
        "layer": "L0",
        "score": 49.0,
        "reasons": ["Fallback transport exercised for regression."],
        "route_snapshot": {
            "engine": "rust",
            "selected_skill": "execution-controller-coding",
            "overlay_skill": None,
            "layer": "L0",
            "score": 49.0,
            "score_bucket": "40-49",
            "reasons": ["Fallback transport exercised for regression."],
            "reasons_class": "fallback transport",
        },
    }


def test_route_adapter_query_cli_args_emits_explicit_false_route_flags() -> None:
    command = _live_route_adapter().query_cli_args(
        query="route me",
        limit=5,
        route_json=True,
        allow_overlay=False,
        first_turn=False,
    )

    assert "--allow-overlay=false" in command
    assert "--first-turn=false" in command


def test_route_decision_contract_respects_false_overlay_and_first_turn_flags() -> None:
    decision = route_decision_contract(
        "这个仓库的修复你直接 gsd，推进到底，别停，主线程保持简短并给我验证证据",
        codex_home=PROJECT_ROOT,
        session_id="route-cli-regression",
        allow_overlay=False,
        first_turn=False,
        runtime_path=PROJECT_ROOT / "tests" / "_routing_missing_runtime.json",
        manifest_path=PROJECT_ROOT / "tests" / "routing_route_fixtures.json",
    ).model_dump(mode="json")

    assert decision["selected_skill"] == "execution-controller-coding"
    assert decision["overlay_skill"] is None
    assert all("Session-start" not in reason for reason in decision["reasons"])


def test_route_decision_contract_exposes_typed_decision() -> None:
    decision = route_decision_contract(
        "这个仓库的修复你直接 gsd，推进到底，别停，主线程保持简短并给我验证证据",
        codex_home=PROJECT_ROOT,
        session_id="route-cli-typed-regression",
        allow_overlay=False,
        first_turn=False,
        runtime_path=PROJECT_ROOT / "tests" / "_routing_missing_runtime.json",
        manifest_path=PROJECT_ROOT / "tests" / "routing_route_fixtures.json",
    )

    assert isinstance(decision, RouteDecisionContract)
    assert decision.selected_skill == "execution-controller-coding"
    assert decision.overlay_skill is None
    assert decision.route_snapshot.selected_skill == decision.selected_skill


@pytest.mark.parametrize(
    ("query", "selected_skill", "overlay_skill", "layer", "allow_overlay", "first_turn"),
    ROUTE_DECISION_KNOB_CASES,
)
def test_route_decision_contract_live_matches_python_for_knobbed_inputs(
    query: str,
    selected_skill: str,
    overlay_skill: str | None,
    layer: str,
    allow_overlay: bool,
    first_turn: bool,
) -> None:
    python_decision = route_decision_contract(
        query,
        codex_home=PROJECT_ROOT,
        session_id="route-knob-live-parity",
        allow_overlay=allow_overlay,
        first_turn=first_turn,
    ).model_dump(mode="json")
    rust_decision = _live_route_adapter().route_contract(
        query=query,
        session_id="route-knob-live-parity",
        allow_overlay=allow_overlay,
        first_turn=first_turn,
    ).model_dump(mode="json")

    assert rust_decision == python_decision
    assert rust_decision["selected_skill"] == selected_skill
    assert rust_decision["overlay_skill"] == overlay_skill
    assert rust_decision["layer"] == layer


def test_route_contract_falls_back_to_default_runner_when_stdio_reports_unsupported_operation(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    adapter = RustRouteAdapter(
        PROJECT_ROOT,
        runtime_path=MISSING_RUNTIME_PATH,
        manifest_path=ROUTE_FIXTURE_PATH,
    )

    class _UnsupportedOperationClient:
        def request(self, operation: str, payload: object) -> dict[str, object]:
            raise RuntimeError("unsupported stdio operation: route")

        def close(self) -> None:
            pass

    calls = {"json_runner_calls": 0}

    def fake_run_json_command(command: list[str], *, failure_label: str) -> dict[str, object]:
        calls["json_runner_calls"] += 1
        return _fallback_route_contract_payload(adapter=adapter)

    monkeypatch.setattr(adapter, "_stdio_client", lambda: _UnsupportedOperationClient())
    monkeypatch.setattr(adapter, "_reset_stdio_client", lambda: None)
    monkeypatch.setattr(adapter, "_run_json_command", fake_run_json_command)

    contract = adapter.route_contract(
        query="这个仓库的修复你直接 gsd，推进到底，别停，主线程保持简短并给我验证证据",
        session_id="route-contract-fallback-session",
        allow_overlay=True,
        first_turn=False,
    )

    assert calls["json_runner_calls"] == 1
    assert contract.selected_skill == "execution-controller-coding"
    assert contract.overlay_skill is None


def test_route_contract_rejects_unknown_decision_schema_shape(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    adapter = _fixture_route_adapter()

    def fake_run_hot_json_command(*args, **kwargs):
        payload = _fallback_route_contract_payload(adapter=adapter).copy()
        payload["decision_schema_version"] = "router-rs-route-decision-vX"
        return payload

    monkeypatch.setattr(adapter, "_run_hot_json_command", fake_run_hot_json_command)
    with pytest.raises(RuntimeError, match="unknown decision schema"):
        adapter.route_contract(
            query="这个仓库的修复你直接 gsd，推进到底，别停，主线程保持简短并给我验证证据",
            session_id="route-contract-unknown-schema",
            allow_overlay=True,
            first_turn=True,
        )


def test_route_contract_rejects_unknown_decision_authority_shape(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    adapter = _fixture_route_adapter()

    def fake_run_hot_json_command(*args, **kwargs):
        payload = _fallback_route_contract_payload(adapter=adapter).copy()
        payload["authority"] = "legacy-route-core"
        return payload

    monkeypatch.setattr(adapter, "_run_hot_json_command", fake_run_hot_json_command)
    with pytest.raises(RuntimeError, match="unexpected authority marker"):
        adapter.route_contract(
            query="这个仓库的修复你直接 gsd，推进到底，别停，主线程保持简短并给我验证证据",
            session_id="route-contract-unknown-authority",
            allow_overlay=True,
            first_turn=True,
        )


def test_search_skills_uses_route_adapter_hot_path(monkeypatch: pytest.MonkeyPatch) -> None:
    rows = [
        SearchMatchResult(
            record=SkillMetadata(
                name="iterative-optimizer",
                description="Iterative optimization loop",
                routing_layer="L2",
                routing_gate="none",
                routing_owner="codex",
            ),
            score=9.5,
            matched_terms=2,
            total_terms=2,
        )
    ]
    calls: list[dict[str, object]] = []

    class _FakeAdapter:
        def search_skill_matches(self, **kwargs):
            calls.append(dict(kwargs))
            return rows

    monkeypatch.setattr(rust_router_module, "route_adapter", lambda **kwargs: _FakeAdapter())

    results = rust_router_module.search_skills(
        "自迭代 10轮 优化 验证",
        codex_home=PROJECT_ROOT,
        limit=3,
        runtime_path=MISSING_RUNTIME_PATH,
        manifest_path=ROUTE_FIXTURE_PATH,
    )

    assert calls == [{"query": "自迭代 10轮 优化 验证", "limit": 3}]
    assert results[0].record.name == "iterative-optimizer"
    assert results[0].score == 9.5


def test_search_match_result_round_trips_transport_row() -> None:
    row = {
        "slug": "iterative-optimizer",
        "description": "Iterative optimization loop",
        "layer": "L2",
        "gate": "none",
        "owner": "codex",
        "score": 9.5,
        "matched_terms": 2,
        "total_terms": 2,
    }

    match = SearchMatchResult.from_transport_row(row)

    assert match.record.name == "iterative-optimizer"
    assert match.record.routing_layer == "L2"
    assert match.to_transport_row() == row


def test_search_skill_matches_contract_accepts_legacy_transport_rows(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    transport_rows = [
        {
            "slug": "iterative-optimizer",
            "description": "Iterative optimization loop",
            "layer": "L2",
            "gate": "none",
            "owner": "codex",
            "score": 9.5,
            "matched_terms": 2,
            "total_terms": 2,
        }
    ]
    adapter = RustRouteAdapter(
        PROJECT_ROOT,
        runtime_path=MISSING_RUNTIME_PATH,
        manifest_path=ROUTE_FIXTURE_PATH,
    )
    monkeypatch.setattr(
        adapter,
        "_run_hot_json_command",
        lambda *args, **kwargs: transport_rows,
    )

    contract = adapter.search_skill_matches_contract(query="typed first", limit=1)

    assert isinstance(contract, SearchMatchesContract)
    assert contract.search_schema_version == adapter.search_schema_version
    assert contract.authority == adapter.route_authority
    assert contract.query == "typed first"
    assert contract.to_transport_rows() == transport_rows


def test_search_skill_rows_json_text_exports_transport_payload_from_typed_matches(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    contract = SearchMatchesContract.model_validate(
        {
            "search_schema_version": "router-rs-search-results-v1",
            "authority": "rust-route-core",
            "query": "typed first",
            "matches": [
                {
                    "slug": "iterative-optimizer",
                    "description": "Iterative optimization loop",
                    "layer": "L2",
                    "gate": "none",
                    "owner": "codex",
                    "score": 9.5,
                    "matched_terms": 2,
                    "total_terms": 2,
                }
            ],
        }
    )
    adapter = RustRouteAdapter(
        PROJECT_ROOT,
        runtime_path=MISSING_RUNTIME_PATH,
        manifest_path=ROUTE_FIXTURE_PATH,
    )
    monkeypatch.setattr(adapter, "search_skill_matches_contract", lambda **kwargs: contract)

    payload = json.loads(adapter.search_skill_rows_json_text(query="typed first", limit=1))

    assert payload == contract.to_transport_payload()


def test_route_decision_contract_stays_typed_first_transport_payload(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    contract = RouteDecisionContract.model_validate(
        {
            "decision_schema_version": "router-rs-route-decision-v1",
            "authority": "rust-route-core",
            "compile_authority": "rust-route-compiler",
            "task": "typed-first transport export",
            "session_id": "typed-first-transport-session",
            "selected_skill": "execution-controller-coding",
            "overlay_skill": None,
            "layer": "L2",
            "score": 42.0,
            "reasons": ["Trigger phrase matched: gsd."],
            "route_snapshot": {
                "engine": "rust",
                "selected_skill": "execution-controller-coding",
                "overlay_skill": None,
                "layer": "L2",
                "score": 42.0,
                "score_bucket": "40-49",
                "reasons": ["Trigger phrase matched: gsd."],
                "reasons_class": "trigger phrase matched: gsd.",
            },
        }
    )

    monkeypatch.setattr(rust_router_module, "route_decision_contract", lambda *args, **kwargs: contract)

    assert rust_router_module.route_decision_contract("typed first", codex_home=PROJECT_ROOT).model_dump(mode="json") == contract.model_dump(mode="json")


def test_route_decision_contract_uses_rust_route_adapter(monkeypatch: pytest.MonkeyPatch) -> None:
    contract = RouteDecisionContract.model_validate(
        {
            "decision_schema_version": "router-rs-route-decision-v1",
            "authority": "rust-route-core",
            "compile_authority": "rust-route-compiler",
            "task": "adapter backed route",
            "session_id": "adapter-backed-session",
            "selected_skill": "execution-controller-coding",
            "overlay_skill": None,
            "layer": "L2",
            "score": 42.0,
            "reasons": ["adapter route"],
            "route_snapshot": {
                "engine": "rust",
                "selected_skill": "execution-controller-coding",
                "overlay_skill": None,
                "layer": "L2",
                "score": 42.0,
                "score_bucket": "40-49",
                "reasons": ["adapter route"],
                "reasons_class": "adapter route",
            },
        }
    )

    class _FakeAdapter:
        def route_contract(self, **kwargs):
            return contract

    monkeypatch.setattr(rust_router_module, "route_adapter", lambda **kwargs: _FakeAdapter())

    assert rust_router_module.route_decision_contract("adapter backed route", codex_home=PROJECT_ROOT) == contract


def test_route_decision_contract_exports_transport_payload_from_typed_contract(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    contract = RouteDecisionContract.model_validate(
        {
            "decision_schema_version": "router-rs-route-decision-v1",
            "authority": "rust-route-core",
            "compile_authority": "rust-route-compiler",
            "task": "route cli typed first",
            "session_id": "route-cli-typed-session",
            "selected_skill": "skill-developer-codex",
            "overlay_skill": "anti-laziness",
            "layer": "L2",
            "score": 55.0,
            "reasons": ["Trigger phrase matched: route."],
            "route_snapshot": {
                "engine": "rust",
                "selected_skill": "skill-developer-codex",
                "overlay_skill": "anti-laziness",
                "layer": "L2",
                "score": 55.0,
                "score_bucket": "50-59",
                "reasons": ["Trigger phrase matched: route."],
                "reasons_class": "trigger phrase matched: route.",
            },
        }
    )

    monkeypatch.setattr(rust_router_module, "route_decision_contract", lambda *args, **kwargs: contract)

    assert rust_router_module.route_decision_contract("route cli typed first", codex_home=PROJECT_ROOT).model_dump(mode="json") == contract.model_dump(mode="json")


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
def test_rust_search_contract_matches_python_search_results(query: str, limit: int) -> None:
    """Verify the search consumer path stays typed-first even when the CLI still prints rows."""

    contract = route_adapter(
        codex_home=PROJECT_ROOT,
        runtime_path=None,
        manifest_path=None,
    ).search_skill_matches_contract(query=query, limit=limit)
    hydrated = search_skills(query, codex_home=PROJECT_ROOT, limit=limit)

    assert contract.search_schema_version == "router-rs-search-results-v1"
    assert contract.authority == "rust-route-core"
    assert contract.query == query
    assert contract.matches == hydrated


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

    python_decision = route_decision_contract(
        query,
        codex_home=PROJECT_ROOT,
        session_id="fixture-session",
        allow_overlay=allow_overlay,
        first_turn=first_turn,
        runtime_path=MISSING_RUNTIME_PATH,
        manifest_path=ROUTE_FIXTURE_PATH,
    ).model_dump(mode="json")
    rust_contract = _fixture_route_adapter().route_contract(
        query=query,
        session_id="fixture-session",
        allow_overlay=allow_overlay,
        first_turn=first_turn,
    )
    rust_decision = rust_contract.model_dump(mode="json")

    assert rust_decision == python_decision

    expected = case["expected"]
    assert rust_contract.selected_skill == expected["selected_skill"]
    assert rust_contract.overlay_skill == expected["overlay_skill"]
    assert rust_contract.layer == expected["layer"]


@pytest.mark.parametrize("query", REAL_TASK_REPLAY_QUERIES)
def test_real_task_replay_queries_match_shadow_diff_fields(query: str) -> None:
    """Real-task replay queries should keep the stable shadow diff vocabulary aligned."""

    python_decision = route_decision_contract(
        query,
        codex_home=PROJECT_ROOT,
        session_id="shadow-replay-session",
        allow_overlay=True,
        first_turn=True,
    ).model_dump(mode="json")
    rust_decision = _live_route_adapter().route_contract(
        query=query,
        session_id="shadow-replay-session",
        allow_overlay=True,
        first_turn=True,
    ).model_dump(mode="json")

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

    python_decision = route_decision_contract(
        query,
        codex_home=PROJECT_ROOT,
        session_id="live-expectation-session",
        allow_overlay=True,
        first_turn=True,
    ).model_dump(mode="json")
    rust_decision = _live_route_adapter().route_contract(
        query=query,
        session_id="live-expectation-session",
        allow_overlay=True,
        first_turn=True,
    ).model_dump(mode="json")

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
    contract = adapter.route_policy_contract(mode=mode)

    assert contract.policy_schema_version == adapter.route_policy_schema_version
    assert contract.authority == adapter.route_authority
    assert contract.mode == mode
    for key, value in expected.items():
        assert getattr(contract, key) == value
    assert contract.primary_authority == "rust"
    assert contract.route_result_engine == "rust"


def test_rust_route_adapter_route_contract_returns_typed_rust_owned_contract() -> None:
    """The Python host should consume a typed Rust route contract, not stitch raw fields ad hoc."""

    adapter = RustRouteAdapter(PROJECT_ROOT)
    contract = adapter.route_contract(
        query="帮我写一个 Rust CLI 工具",
        session_id="typed-route-contract-session",
        allow_overlay=True,
        first_turn=True,
    )

    assert isinstance(contract, RouteDecisionContract)
    assert contract.decision_schema_version == adapter.route_decision_schema_version
    assert contract.authority == adapter.route_authority
    assert contract.route_snapshot.engine == "rust"
    assert contract.route_snapshot.selected_skill == contract.selected_skill
    assert contract.route_snapshot.overlay_skill == contract.overlay_skill
    assert contract.route_snapshot.layer == contract.layer


def test_route_decision_contract_rejects_snapshot_drift() -> None:
    """The typed route contract should fail closed if top-level fields drift from the Rust snapshot."""

    with pytest.raises(ValueError, match="selected_skill must match route_snapshot"):
        RouteDecisionContract.model_validate(
            {
                "decision_schema_version": "router-rs-route-decision-v1",
                "authority": "rust-route-core",
                "compile_authority": "rust-route-compiler",
                "task": "route drift regression",
                "session_id": "typed-route-contract-session",
                "selected_skill": "plan-to-code",
                "overlay_skill": "anti-laziness",
                "layer": "L2",
                "score": 42.0,
                "reasons": ["Trigger phrase matched: 直接做代码."],
                "route_snapshot": {
                    "engine": "rust",
                    "selected_skill": "idea-to-plan",
                    "overlay_skill": "anti-laziness",
                    "layer": "L2",
                    "score": 42.0,
                    "score_bucket": "40-49",
                    "reasons": ["Trigger phrase matched: 直接做代码."],
                    "reasons_class": "trigger phrase matched: 直接做代码.",
                },
            }
        )


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
    decision = route_decision_contract(
        "帮我写一个 Rust CLI 工具",
        codex_home=PROJECT_ROOT,
        session_id="route-report-contract-session",
        allow_overlay=True,
        first_turn=True,
    )
    baseline = RouteDecisionSnapshot.model_validate(
        decision.route_snapshot.model_dump(mode="json")
    )

    report = adapter.route_report_contract(
        mode="shadow",
        route_decision_contract=decision,
    )

    assert isinstance(report, RouteDiagnosticReport)
    assert report.report_schema_version == adapter.route_report_schema_version
    assert report.authority == adapter.route_authority
    assert report.mode == "shadow"
    assert report.primary_engine == "rust"
    assert report.evidence_kind == "rust-owned-snapshot"
    assert report.strict_verification is False
    assert report.verification_passed is True
    assert report.verified_contract_fields == ["engine", "selected_skill", "layer", "overlay_skill"]
    assert report.contract_mismatch_fields == []
    assert report.route_snapshot == baseline


def test_route_report_verify_mode_requires_strict_verification() -> None:
    """Verify mode should mark the diagnostic report as strict Rust verification."""

    adapter = RustRouteAdapter(PROJECT_ROOT)
    decision = route_decision_contract(
        "帮我写一个 Rust CLI 工具",
        codex_home=PROJECT_ROOT,
        session_id="route-report-verify-session",
        allow_overlay=True,
        first_turn=True,
    )
    baseline = RouteDecisionSnapshot.model_validate(
        decision.route_snapshot.model_dump(mode="json")
    )

    report = adapter.route_report_contract(
        mode="verify",
        route_decision_contract=decision,
    )

    assert isinstance(report, RouteDiagnosticReport)
    assert report.report_schema_version == adapter.route_report_schema_version
    assert report.authority == adapter.route_authority
    assert report.mode == "verify"
    assert report.primary_engine == "rust"
    assert report.evidence_kind == "rust-owned-snapshot"
    assert report.strict_verification is True
    assert report.verification_passed is True
    assert report.contract_mismatch_fields == []


def test_route_report_contract_accepts_snapshot_dict_for_compatibility_callers() -> None:
    """Compatibility callers can still pass a dict while the adapter validates into the typed contract."""

    adapter = RustRouteAdapter(PROJECT_ROOT)
    baseline = route_decision_contract(
        "帮我写一个 Rust CLI 工具",
        codex_home=PROJECT_ROOT,
        session_id="route-report-compat-session",
        allow_overlay=True,
        first_turn=True,
    ).model_dump(mode="json")["route_snapshot"]

    report = adapter.route_report_contract(
        mode="shadow",
        rust_route_snapshot=baseline,
    )

    assert isinstance(report, RouteDiagnosticReport)
    assert report.mode == "shadow"
    assert report.verified_contract_fields == []
    assert report.contract_mismatch_fields == []
    assert report.route_snapshot.selected_skill == baseline["selected_skill"]


def test_route_report_contract_marks_mismatched_contract_fields() -> None:
    """Rust route diagnostics should carry contract mismatches instead of relying on Python-side comparisons."""

    adapter = RustRouteAdapter(PROJECT_ROOT)
    decision = route_decision_contract(
        "帮我写一个 Rust CLI 工具",
        codex_home=PROJECT_ROOT,
        session_id="route-report-mismatch-session",
        allow_overlay=True,
        first_turn=True,
    ).model_dump(mode="json")
    decision["selected_skill"] = "wrong-skill"

    report = adapter.route_report_contract(
        mode="verify",
        route_decision_contract=decision,
    )

    assert report.strict_verification is True
    assert report.verification_passed is False
    assert "selected_skill" in report.contract_mismatch_fields


def test_route_report_contract_can_derive_snapshot_from_typed_decision() -> None:
    """Primary callers should be able to hand Rust the typed decision without duplicating snapshot JSON."""

    adapter = RustRouteAdapter(PROJECT_ROOT)
    decision = route_decision_contract(
        "帮我写一个 Rust CLI 工具",
        codex_home=PROJECT_ROOT,
        session_id="route-report-decision-only-session",
        allow_overlay=True,
        first_turn=True,
    )

    report = adapter.route_report_contract(
        mode="shadow",
        route_decision_contract=decision,
    )

    assert report.mode == "shadow"
    assert report.verification_passed is True
    assert report.route_snapshot == decision.route_snapshot


def test_route_report_contract_requires_snapshot_or_decision() -> None:
    """The compatibility shim should fail closed when callers omit both report inputs."""

    adapter = RustRouteAdapter(PROJECT_ROOT)

    with pytest.raises(
        ValueError,
        match="route_report_contract requires rust_route_snapshot or route_decision_contract",
    ):
        adapter.route_report_contract(mode="shadow")


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
