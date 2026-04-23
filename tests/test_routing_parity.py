"""Parity tests for legacy route fixtures and Rust router JSON output."""

from __future__ import annotations

import json
import sys
from pathlib import Path

import pytest
from pydantic import ValidationError

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
SCRIPTS_ROOT = PROJECT_ROOT / "scripts"
if str(SCRIPTS_ROOT) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_ROOT))
RUNTIME_SRC = PROJECT_ROOT / "framework_runtime" / "src"
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

import framework_runtime.rust_router as rust_router_module

from framework_runtime.rust_router import RustRouteAdapter, route_adapter, route_decision_contract, search_skills
from framework_runtime.schemas import (
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
        "skill-framework-developer",
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
def test_route_decision_contract_live_matches_legacy_for_knobbed_inputs(
    query: str,
    selected_skill: str,
    overlay_skill: str | None,
    layer: str,
    allow_overlay: bool,
    first_turn: bool,
) -> None:
    legacy_decision = route_decision_contract(
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

    assert rust_decision == legacy_decision
    assert rust_decision["selected_skill"] == selected_skill
    assert rust_decision["overlay_skill"] == overlay_skill
    assert rust_decision["layer"] == layer


def test_route_contract_fails_closed_when_stdio_reports_unsupported_operation(
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

    monkeypatch.setattr(adapter, "_stdio_client", lambda: _UnsupportedOperationClient())
    monkeypatch.setattr(adapter, "_reset_stdio_client", lambda: None)

    with pytest.raises(RuntimeError, match="does not support 'route'"):
        adapter.route_contract(
            query="这个仓库的修复你直接 gsd，推进到底，别停，主线程保持简短并给我验证证据",
            session_id="route-contract-fallback-session",
            allow_overlay=True,
            first_turn=False,
        )


def test_route_contract_rejects_unknown_decision_schema_shape(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    adapter = _fixture_route_adapter()

    def fake_run_hot_json_command(*args, **kwargs):
        payload = _fallback_route_contract_payload(adapter=adapter).copy()
        payload["decision_schema_version"] = "router-rs-route-decision-vX"
        return payload

    monkeypatch.setattr(adapter, "_run_hot_json_command", fake_run_hot_json_command)
    with pytest.raises(RuntimeError, match="invalid typed decision contract"):
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
    with pytest.raises(RuntimeError, match="invalid typed decision contract"):
        adapter.route_contract(
            query="这个仓库的修复你直接 gsd，推进到底，别停，主线程保持简短并给我验证证据",
            session_id="route-contract-unknown-authority",
            allow_overlay=True,
            first_turn=True,
        )


def test_background_state_uses_hot_json_runner(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    adapter = _fixture_route_adapter()
    calls = {"json_runner_calls": 0, "hot_runner_calls": 0}

    def fake_run_json_command(command: list[str], *, failure_label: str) -> dict[str, object]:
        calls["json_runner_calls"] += 1
        return {
            "schema_version": adapter.background_state_store_schema_version,
            "authority": adapter.background_state_store_authority,
            "status": "ok",
        }

    def fake_run_hot_json_command(*args, **kwargs):
        calls["hot_runner_calls"] += 1
        return {
            "schema_version": adapter.background_state_store_schema_version,
            "authority": adapter.background_state_store_authority,
            "status": "ok",
        }

    monkeypatch.setattr(adapter, "_run_json_command", fake_run_json_command)
    monkeypatch.setattr(adapter, "_run_hot_json_command", fake_run_hot_json_command)

    payload = adapter.background_state({"operation": "set", "key": "job-1", "value": {"status": "queued"}})

    assert calls["json_runner_calls"] == 0
    assert calls["hot_runner_calls"] == 1
    assert payload["schema_version"] == adapter.background_state_store_schema_version
    assert payload["authority"] == adapter.background_state_store_authority




def test_router_stdio_pool_defaults_to_rust_control_plane_size(monkeypatch: pytest.MonkeyPatch) -> None:
    adapter = _fixture_route_adapter()
    monkeypatch.delenv("CODEX_ROUTER_STDIO_POOL_SIZE", raising=False)

    assert adapter._stdio_pool_size() == 4


def test_router_stdio_pool_size_honors_environment_override(monkeypatch: pytest.MonkeyPatch) -> None:
    adapter = _fixture_route_adapter()
    monkeypatch.setenv("CODEX_ROUTER_STDIO_POOL_SIZE", "7")

    assert adapter._stdio_pool_size() == 7


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


def test_search_match_result_accepts_typed_record_payload() -> None:
    match = SearchMatchResult.model_validate(
        {
            "record": {
                "name": "iterative-optimizer",
                "description": "Iterative optimization loop",
                "routing_layer": "L2",
                "routing_gate": "none",
                "routing_owner": "codex",
            },
            "score": 9.5,
            "matched_terms": 2,
            "total_terms": 2,
        }
    )

    assert match.record.name == "iterative-optimizer"
    assert match.record.routing_layer == "L2"


def test_search_skill_matches_contract_rejects_legacy_transport_rows(
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

    with pytest.raises(RuntimeError, match="unexpected payload"):
        adapter.search_skill_matches_contract(query="typed first", limit=1)


def test_search_skill_matches_contract_rejects_rows_only_payload(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    adapter = RustRouteAdapter(
        PROJECT_ROOT,
        runtime_path=MISSING_RUNTIME_PATH,
        manifest_path=ROUTE_FIXTURE_PATH,
    )
    monkeypatch.setattr(
        adapter,
        "_run_hot_json_command",
        lambda *args, **kwargs: {
            "search_schema_version": adapter.search_schema_version,
            "authority": adapter.route_authority,
            "query": "typed first",
            "rows": [
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
        },
    )

    with pytest.raises(RuntimeError, match="invalid typed search contract"):
        adapter.search_skill_matches_contract(query="typed first", limit=1)


def test_search_matches_contract_rejects_transport_rows_outside_adapter_boundary() -> None:
    with pytest.raises(ValidationError, match="record"):
        SearchMatchesContract.model_validate(
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


def test_search_matches_contract_rejects_non_rust_contract_markers() -> None:
    with pytest.raises(ValidationError, match="search_schema_version"):
        SearchMatchesContract.model_validate(
            {
                "search_schema_version": "legacy-search-results-v1",
                "authority": "rust-route-core",
                "query": "typed first",
                "matches": [],
            }
        )
    with pytest.raises(ValidationError, match="authority"):
        SearchMatchesContract.model_validate(
            {
                "search_schema_version": "router-rs-search-results-v1",
                "authority": "legacy-route-core",
                "query": "typed first",
                "matches": [],
            }
        )


def test_search_skill_matches_contract_accepts_typed_matches_without_legacy_rows(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    adapter = RustRouteAdapter(
        PROJECT_ROOT,
        runtime_path=MISSING_RUNTIME_PATH,
        manifest_path=ROUTE_FIXTURE_PATH,
    )
    monkeypatch.setattr(
        adapter,
        "_run_hot_json_command",
        lambda *args, **kwargs: {
            "search_schema_version": adapter.search_schema_version,
            "authority": adapter.route_authority,
            "query": "typed first",
            "matches": [
                {
                    "record": {
                        "name": "iterative-optimizer",
                        "description": "Iterative optimization loop",
                        "routing_layer": "L2",
                        "routing_gate": "none",
                        "routing_owner": "codex",
                    },
                    "score": 9.5,
                    "matched_terms": 2,
                    "total_terms": 2,
                }
            ],
        },
    )

    contract = adapter.search_skill_matches_contract(query="typed first", limit=1)

    assert contract.matches[0].record.name == "iterative-optimizer"
    assert contract.matches[0].record.routing_layer == "L2"


def test_route_decision_contract_rejects_non_rust_contract_markers() -> None:
    payload = _fallback_route_contract_payload(adapter=_fixture_route_adapter())
    payload["decision_schema_version"] = "legacy-route-decision-v1"
    with pytest.raises(ValidationError, match="decision_schema_version"):
        RouteDecisionContract.model_validate(payload)

    payload = _fallback_route_contract_payload(adapter=_fixture_route_adapter())
    payload["authority"] = "legacy-route-core"
    with pytest.raises(ValidationError, match="authority"):
        RouteDecisionContract.model_validate(payload)

    payload = _fallback_route_contract_payload(adapter=_fixture_route_adapter())
    payload["compile_authority"] = "legacy-route-compiler"
    with pytest.raises(ValidationError, match="compile_authority"):
        RouteDecisionContract.model_validate(payload)

    payload = _fallback_route_contract_payload(adapter=_fixture_route_adapter())
    payload["route_snapshot"]["engine"] = "legacy"
    with pytest.raises(ValidationError, match="engine"):
        RouteDecisionContract.model_validate(payload)


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
            "selected_skill": "skill-framework-developer",
            "overlay_skill": "anti-laziness",
            "layer": "L2",
            "score": 55.0,
            "reasons": ["Trigger phrase matched: route."],
            "route_snapshot": {
                "engine": "rust",
                "selected_skill": "skill-framework-developer",
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
            "primary_owner": "skill-framework-developer",
            "execution_contract": {
                "goal": "Repair stale bootstrap injection",
                "scope": ["scripts/router-rs/src/framework_runtime.rs"],
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
                "skill-framework-developer",
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
            "transport_family": "host-facing-transport",
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
            "cleanup_semantics": "stream_cache_only",
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
            "control_plane_projection": "rust-native-projection",
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
                    "projection": "rust-native-projection",
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
                    "projection": "rust-native-projection",
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
                    "projection": "rust-native-projection",
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
                "projection": "rust-native-projection",
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
                    "projection": "rust-native-projection",
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
    trace_metadata_ack = {
        "schema_version": adapter.trace_metadata_write_schema_version,
        "authority": adapter.trace_metadata_write_authority,
        "output_path": "/tmp/TRACE_METADATA.json",
        "mirror_paths": ["/tmp/artifacts/current/TRACE_METADATA.json"],
        "bytes_written": 1024,
        "routing_runtime_version": 7,
    }

    def fake_run_json(command: list[str], *, failure_label: str) -> dict[str, object]:
        if "--write-transport-binding-json" in command:
            return transport_ack
        if "--write-checkpoint-resume-manifest-json" in command:
            return manifest_ack
        if "--write-trace-metadata-json" in command:
            return trace_metadata_ack
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
    assert adapter.write_trace_metadata(
        {
            "output_path": trace_metadata_ack["output_path"],
            "task": "trace metadata parity",
            "matched_skills": ["execution-controller-coding"],
            "owner": "execution-controller-coding",
            "gate": "none",
            "overlay": None,
            "reroute_count": 0,
            "retry_count": 0,
            "artifact_paths": [],
            "verification_status": "passed",
        }
    ) == trace_metadata_ack

ROUTE_FIXTURE_PATH = PROJECT_ROOT / "tests" / "routing_route_fixtures.json"
MISSING_RUNTIME_PATH = PROJECT_ROOT / "tests" / "_routing_missing_runtime.json"
ROUTE_FIXTURES = json.loads(ROUTE_FIXTURE_PATH.read_text(encoding="utf-8"))
REAL_TASK_REPLAY_QUERIES = [
    "这是高负载跨文件任务，需要 sidecar delegation 并行处理",
    "帮我写一个 Rust CLI 工具",
    "把这份 runtime checklist 落成代码，并保留 host 接口",
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
def test_rust_search_contract_matches_legacy_search_results(query: str, limit: int) -> None:
    """Verify the search consumer path stays typed-first with one Rust-owned match contract."""

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
def test_rust_route_json_matches_legacy_route_decision(case: dict[str, object]) -> None:
    """Verify final route decision parity between legacy and Rust."""

    query = str(case["query"])
    allow_overlay = bool(case.get("allow_overlay", True))
    first_turn = bool(case.get("first_turn", True))

    legacy_decision = route_decision_contract(
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

    assert rust_decision == legacy_decision

    expected = case["expected"]
    assert rust_contract.selected_skill == expected["selected_skill"]
    assert rust_contract.overlay_skill == expected["overlay_skill"]
    assert rust_contract.layer == expected["layer"]


@pytest.mark.parametrize("query", REAL_TASK_REPLAY_QUERIES)
def test_real_task_replay_queries_match_shadow_diff_fields(query: str) -> None:
    """Real-task replay queries should keep the stable shadow diff vocabulary aligned."""

    legacy_decision = route_decision_contract(
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

    assert rust_decision["selected_skill"] == legacy_decision["selected_skill"]
    assert rust_decision["overlay_skill"] == legacy_decision["overlay_skill"]
    assert rust_decision["layer"] == legacy_decision["layer"]
    assert rust_decision["route_snapshot"]["score_bucket"] == legacy_decision["route_snapshot"]["score_bucket"]
    assert rust_decision["route_snapshot"]["reasons_class"] == legacy_decision["route_snapshot"]["reasons_class"]


@pytest.mark.parametrize(
    ("query", "selected_skill", "overlay_skill"),
    [
        (
            "深度review现在的路由系统和 skill 边界。",
            "skill-framework-developer",
            "code-review",
        ),
        (
            "framework-review",
            "skill-framework-developer",
            "code-review",
        ),
        (
            "帮我看 OpenAI Responses API 最新官方文档并说明怎么用。",
            "openai-docs",
            "anti-laziness",
        ),
        (
            "请直接 autopilot 这轮修复，推进到底",
            "autopilot",
            "anti-laziness",
        ),
        (
            "请 deepinterview 这轮 review",
            "deepinterview",
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

    legacy_decision = route_decision_contract(
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

    assert rust_decision == legacy_decision
    assert rust_decision["selected_skill"] == selected_skill
    assert rust_decision["overlay_skill"] == overlay_skill


@pytest.mark.parametrize(
    ("query", "selected_skill"),
    [
        ("/autopilot", "autopilot"),
        ("$autopilot", "autopilot"),
        ("/deepinterview", "deepinterview"),
        ("$deepinterview", "deepinterview"),
        ("/team", "team"),
        ("$team", "team"),
    ],
)
def test_framework_aliases_only_route_from_explicit_entrypoints(
    query: str,
    selected_skill: str,
) -> None:
    legacy_decision = route_decision_contract(
        query,
        codex_home=PROJECT_ROOT,
        session_id="explicit-framework-alias-session",
        allow_overlay=True,
        first_turn=True,
    ).model_dump(mode="json")
    rust_decision = _live_route_adapter().route_contract(
        query=query,
        session_id="explicit-framework-alias-session",
        allow_overlay=True,
        first_turn=True,
    ).model_dump(mode="json")

    assert rust_decision == legacy_decision
    assert rust_decision["selected_skill"] == selected_skill
    assert rust_decision["overlay_skill"] == "anti-laziness"


@pytest.mark.parametrize(
    ("query", "selected_skill"),
    [
        ("进入 autopilot", "autopilot"),
        ("使用 deepinterview", "deepinterview"),
        ("切到 team", "team"),
    ],
)
def test_framework_aliases_route_from_explicit_activation_phrases(
    query: str,
    selected_skill: str,
) -> None:
    legacy_decision = route_decision_contract(
        query,
        codex_home=PROJECT_ROOT,
        session_id="explicit-framework-alias-activation-session",
        allow_overlay=True,
        first_turn=True,
    ).model_dump(mode="json")
    rust_decision = _live_route_adapter().route_contract(
        query=query,
        session_id="explicit-framework-alias-activation-session",
        allow_overlay=True,
        first_turn=True,
    ).model_dump(mode="json")

    assert rust_decision == legacy_decision
    assert rust_decision["selected_skill"] == selected_skill
    assert rust_decision["overlay_skill"] == "anti-laziness"


@pytest.mark.parametrize(
    ("query", "should_route_team"),
    [("team mode", True), ("agent team", True), ("worker orchestration", False)],
)
def test_team_short_activation_phrases_route_more_stably_in_codex(
    query: str,
    should_route_team: bool,
) -> None:
    legacy_decision = route_decision_contract(
        query,
        codex_home=PROJECT_ROOT,
        session_id="implicit-framework-alias-session",
        allow_overlay=True,
        first_turn=True,
    ).model_dump(mode="json")
    rust_decision = _live_route_adapter().route_contract(
        query=query,
        session_id="implicit-framework-alias-session",
        allow_overlay=True,
        first_turn=True,
    ).model_dump(mode="json")

    assert rust_decision == legacy_decision
    if should_route_team:
        assert rust_decision["selected_skill"] == "team"
        assert rust_decision["overlay_skill"] == "anti-laziness"
    else:
        assert rust_decision["selected_skill"] != "team"


def test_framework_alias_strong_orchestration_signals_can_route_team() -> None:
    query = "需要 team orchestration，worker lifecycle、integration、qa、cleanup 和 resume recovery 都由 supervisor 主线持续管理"
    legacy_decision = route_decision_contract(
        query,
        codex_home=PROJECT_ROOT,
        session_id="strong-team-orchestration-session",
        allow_overlay=True,
        first_turn=True,
    ).model_dump(mode="json")
    rust_decision = _live_route_adapter().route_contract(
        query=query,
        session_id="strong-team-orchestration-session",
        allow_overlay=True,
        first_turn=True,
    ).model_dump(mode="json")

    assert rust_decision == legacy_decision
    assert rust_decision["selected_skill"] == "team"


@pytest.mark.parametrize(
    "query",
    [
        "需要多阶段 supervisor orchestration，worker lifecycle、integration、qa、cleanup 都要保留",
        "这个任务要 team orchestration，supervisor-owned continuity 和 resume recovery 都要覆盖",
    ],
)
def test_framework_alias_strong_team_orchestration_signals_can_route_team_implicitly(query: str) -> None:
    legacy_decision = route_decision_contract(
        query,
        codex_home=PROJECT_ROOT,
        session_id="implicit-team-route-session",
        allow_overlay=True,
        first_turn=True,
    ).model_dump(mode="json")
    rust_decision = _live_route_adapter().route_contract(
        query=query,
        session_id="implicit-team-route-session",
        allow_overlay=True,
        first_turn=True,
    ).model_dump(mode="json")

    assert rust_decision == legacy_decision
    assert rust_decision["selected_skill"] == "team"


@pytest.mark.parametrize(
    "query",
    [
        "这是多阶段任务，但只需要 bounded sidecars，不要 team orchestration",
        "这个任务要多 agent 并行，但只是 sidecar，不要进入 team",
        "先做 delegation plan，再决定 stay local 还是 subagent",
    ],
)
def test_bounded_multiagent_requests_prefer_subagent_over_team(query: str) -> None:
    legacy_decision = route_decision_contract(
        query,
        codex_home=PROJECT_ROOT,
        session_id="bounded-subagent-route-session",
        allow_overlay=True,
        first_turn=True,
    ).model_dump(mode="json")
    rust_decision = _live_route_adapter().route_contract(
        query=query,
        session_id="bounded-subagent-route-session",
        allow_overlay=True,
        first_turn=True,
    ).model_dump(mode="json")

    assert rust_decision == legacy_decision
    assert rust_decision["selected_skill"] == "subagent-delegation"
    assert rust_decision["overlay_skill"] == "anti-laziness"


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
    """The host should consume a typed Rust route contract, not stitch raw fields ad hoc."""

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


def test_route_resolution_contract_returns_typed_policy_and_shadow_report() -> None:
    """The Rust route-resolution lane should return the policy and report together."""

    adapter = RustRouteAdapter(PROJECT_ROOT)
    decision = route_decision_contract(
        "帮我写一个 Rust CLI 工具",
        codex_home=PROJECT_ROOT,
        session_id="route-resolution-contract-session",
        allow_overlay=True,
        first_turn=True,
    )

    policy, report = adapter.route_resolution_contract(
        mode="shadow",
        route_decision_contract=decision,
    )

    assert isinstance(policy, RouteExecutionPolicy)
    assert policy.policy_schema_version == adapter.route_policy_schema_version
    assert policy.mode == "shadow"
    assert policy.diagnostic_route_mode == "shadow"
    assert policy.diagnostic_report_required is True
    assert report is not None
    assert isinstance(report, RouteDiagnosticReport)
    assert report.report_schema_version == adapter.route_report_schema_version
    assert report.mode == "shadow"
    assert report.route_snapshot == decision.route_snapshot


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


def test_route_report_contract_rejects_snapshot_dict_compatibility_inputs() -> None:
    """Raw dict snapshots should be rejected instead of guessed into the typed route contract."""

    adapter = RustRouteAdapter(PROJECT_ROOT)
    baseline = route_decision_contract(
        "帮我写一个 Rust CLI 工具",
        codex_home=PROJECT_ROOT,
        session_id="route-report-compat-session",
        allow_overlay=True,
        first_turn=True,
    ).model_dump(mode="json")["route_snapshot"]

    with pytest.raises(
        TypeError,
        match="route_report_contract requires RouteDecisionSnapshot for rust_route_snapshot",
    ):
        adapter.route_report_contract(
            mode="shadow",
            rust_route_snapshot=baseline,
        )


def test_route_report_contract_marks_mismatched_contract_fields() -> None:
    """Rust route diagnostics should carry contract mismatches instead of relying on native-side comparisons."""

    adapter = RustRouteAdapter(PROJECT_ROOT)
    decision = route_decision_contract(
        "帮我写一个 Rust CLI 工具",
        codex_home=PROJECT_ROOT,
        session_id="route-report-mismatch-session",
        allow_overlay=True,
        first_turn=True,
    ).model_copy(update={"selected_skill": "wrong-skill"})

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


def test_runtime_storage_contract_round_trips_sqlite_payload(tmp_path: Path) -> None:
    """The Rust runtime-storage transport should own SQLite payload IO end to end."""

    adapter = RustRouteAdapter(PROJECT_ROOT)
    storage_root = tmp_path / "runtime-data"
    db_path = storage_root / "runtime_checkpoint_store.sqlite3"
    payload_path = storage_root / "runtime_background_jobs.json"

    assert (
        adapter.runtime_storage_exists(
            path=payload_path,
            backend_family="sqlite",
            sqlite_db_path=db_path,
            storage_root=storage_root,
        )
        is False
    )
    assert (
        adapter.runtime_storage_write_text(
            path=payload_path,
            backend_family="sqlite",
            sqlite_db_path=db_path,
            storage_root=storage_root,
            payload_text='{"jobs":[]}\n',
        )
        > 0
    )
    assert (
        adapter.runtime_storage_append_text(
            path=payload_path,
            backend_family="sqlite",
            sqlite_db_path=db_path,
            storage_root=storage_root,
            payload_text='{"jobs":[1]}\n',
        )
        > 0
    )
    assert (
        adapter.runtime_storage_read_text(
            path=payload_path,
            backend_family="sqlite",
            sqlite_db_path=db_path,
            storage_root=storage_root,
        )
        == '{"jobs":[]}\n{"jobs":[1]}\n'
    )


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
    assert snapshot["supervisor_state"]["primary_owner"] == "skill-framework-developer"
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


def test_rust_route_adapter_claude_lifecycle_hook_uses_hot_path(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    adapter = _fixture_route_adapter()
    calls: dict[str, object] = {}

    def fake_run_hot_json_command(operation: str, payload: dict[str, object], command: list[str], *, failure_label: str):
        calls["operation"] = operation
        calls["payload"] = payload
        calls["command"] = command
        calls["failure_label"] = failure_label
        return {
            "schema_version": adapter.claude_hook_schema_version,
            "authority": adapter.claude_hook_authority,
            "command": "session-end",
        }

    monkeypatch.setattr(adapter, "_run_hot_json_command", fake_run_hot_json_command)

    payload = adapter.claude_lifecycle_hook(command="session-end", repo_root=tmp_path, max_lines=4)

    assert calls["operation"] == "claude_lifecycle_hook"
    assert calls["payload"] == {
        "command": "session-end",
        "repo_root": str(tmp_path),
        "max_lines": 4,
    }
    assert calls["failure_label"] == "Claude lifecycle hook"
    assert "--claude-hook-command" in calls["command"]
    assert payload["schema_version"] == adapter.claude_hook_schema_version
    assert payload["authority"] == adapter.claude_hook_authority

def test_rust_route_adapter_framework_refresh_copies_prompt_to_clipboard(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    adapter = RustRouteAdapter(PROJECT_ROOT)
    _seed_framework_runtime_artifacts(tmp_path, terminal=False)
    (tmp_path / ".codex" / "memory").mkdir(parents=True, exist_ok=True)
    (tmp_path / ".codex" / "memory" / "MEMORY.md").write_text(
        "# 项目长期记忆\n\n## Active Patterns\n\n- AP-1: Externalize task state\n",
        encoding="utf-8",
    )
    clipboard_path = tmp_path / "refresh_clipboard.txt"
    monkeypatch.setenv("ROUTER_RS_CLIPBOARD_PATH", str(clipboard_path))

    refresh = adapter.framework_refresh(repo_root=tmp_path, max_lines=6)

    copied = clipboard_path.read_text(encoding="utf-8")
    assert refresh["ok"] is True
    assert refresh["confirmation"] == "下一轮执行 prompt 已准备好，并且已经复制到剪贴板。"
    assert refresh["clipboard"]["backend"] == "file"
    assert refresh["prompt"] == copied
    assert "继续当前仓库，先看这些恢复锚点：" in copied
    assert "先做：" in copied
    assert "按既定串并行分工直接开始执行。" in copied
    assert "当前上下文：" not in copied
    assert "必须先做的下一步：" not in copied


def test_rust_route_adapter_framework_alias_builds_compact_autopilot_contract(
    tmp_path: Path,
) -> None:
    adapter = RustRouteAdapter(PROJECT_ROOT)
    _seed_framework_runtime_artifacts(tmp_path, terminal=False)

    alias = adapter.framework_alias(repo_root=tmp_path, alias="autopilot", max_lines=5)

    assert alias["ok"] is True
    assert alias["name"] == "autopilot"
    assert alias["host_entrypoint"] == "$autopilot"
    assert alias["canonical_owner"] == "execution-controller-coding"
    assert alias["upstream_source"]["tag"] == "v4.13.2"
    assert "root-cause-first-when-unknown" in alias["implementation_bar"]
    assert alias["routing_hints"]["reroute_when_ambiguous"] == "idea-to-plan"
    assert alias["interaction_invariants"]["requires_explicit_entrypoint"] is True
    assert alias["interaction_invariants"]["explicit_entrypoints"] == ["/autopilot", "$autopilot"]
    assert alias["interaction_invariants"]["implicit_route_policy"] == "never"
    assert alias["state_machine"]["current_state"] == "resume_active_needs_verification"
    assert alias["state_machine"]["recommended_action"] == "verify_before_done"
    assert alias["state_machine"]["evidence_missing"] is True
    assert alias["entry_contract"]["context"]["execution_readiness"] == "needs_verification"
    assert alias["entry_contract"]["decision_contract"]["verify_when"][0] == "implementation changed but evidence is still missing"
    assert alias["entry_contract"]["route_rules"][0] == "模糊需求 -> `idea-to-plan`"
    assert "进入 autopilot" in alias["entry_prompt"]
    assert "本地 Rust" in alias["entry_prompt"]
    assert "路由：" in alias["entry_prompt"]
    assert "下一步：" in alias["entry_prompt"]
    assert alias["entry_prompt_token_estimate"] > 0
    assert alias["compact"] is False


def test_rust_route_adapter_framework_alias_builds_compact_deepinterview_contract(
    tmp_path: Path,
) -> None:
    adapter = RustRouteAdapter(PROJECT_ROOT)
    _seed_framework_runtime_artifacts(tmp_path, terminal=False)

    alias = adapter.framework_alias(repo_root=tmp_path, alias="deepinterview", max_lines=5)

    assert alias["ok"] is True
    assert alias["name"] == "deepinterview"
    assert alias["host_entrypoint"] == "$deepinterview"
    assert alias["canonical_owner"] == "code-review"
    assert alias["upstream_source"]["official_skill_path"] == "skills/deep-interview/SKILL.md"
    assert "findings-first-with-severity-order" in alias["implementation_bar"]
    assert "architect-review" in alias["routing_hints"]["review_lanes"]
    assert alias["interaction_invariants"]["requires_explicit_entrypoint"] is True
    assert alias["interaction_invariants"]["explicit_entrypoints"] == ["/deepinterview", "$deepinterview"]
    assert alias["interaction_invariants"]["implicit_route_policy"] == "never"
    assert alias["state_machine"]["handoff"]["rules"][1]["target"] == "autopilot"
    assert alias["entry_contract"]["route_rules"][0] == "主 owner -> `code-review`"
    assert "进入 deepinterview" in alias["entry_prompt"]
    assert "每轮只问一个问题" in alias["entry_prompt"]
    assert "review lanes ->" in alias["entry_prompt"]


def test_rust_route_adapter_framework_alias_builds_compact_team_contract(
    tmp_path: Path,
) -> None:
    adapter = RustRouteAdapter(PROJECT_ROOT)
    _seed_framework_runtime_artifacts(tmp_path, terminal=False)

    alias = adapter.framework_alias(repo_root=tmp_path, alias="team", max_lines=5)

    assert alias["ok"] is True
    assert alias["name"] == "team"
    assert alias["host_entrypoint"] == "$team"
    assert alias["canonical_owner"] == "execution-controller-coding"
    assert alias["upstream_source"]["official_skill_path"] == "skills/team/SKILL.md"
    assert "supervisor-owned-continuity" in alias["implementation_bar"]
    assert alias["routing_hints"]["delegation_gate"] == "subagent-delegation"
    assert alias["interaction_invariants"]["requires_explicit_entrypoint"] is True
    assert alias["interaction_invariants"]["explicit_entrypoints"] == ["/team", "$team"]
    assert alias["interaction_invariants"]["implicit_route_policy"] == "strong-orchestration-only"
    assert "worker lifecycle" in alias["interaction_invariants"]["implicit_route_signals"]
    assert alias["routing_hints"]["auto_route_allowed"] is True
    assert alias["routing_hints"]["route_mode"] == "team-orchestration"
    assert "team orchestration" in alias["interaction_invariants"]["implicit_route_signals"]
    assert "integration+qa+cleanup" in alias["interaction_invariants"]["implicit_route_signals"]
    assert "execution-controller-coding" in alias["routing_hints"]["execution_owners"]
    assert "spawn-blocked" in alias["routing_hints"]["transition_states"]
    assert "failed-recoverable" in alias["routing_hints"]["worker_lifecycle"]
    assert alias["state_machine"]["handoff"]["rules"][1]["target"] == "subagent-delegation"
    assert alias["state_machine"]["handoff"]["rules"][1]["action"] == "use_bounded_subagent_lane"
    assert alias["state_machine"]["handoff"]["rules"][2]["target"] == "team"
    assert alias["entry_contract"]["route_rules"][0] == "主 owner -> `execution-controller-coding`"
    assert any(rule == "worker write scope -> `lane-local-delta-only`" for rule in alias["entry_contract"]["route_rules"])
    assert any(rule.startswith("lane contract -> lane_id") for rule in alias["entry_contract"]["route_rules"])
    assert "进入 team" in alias["entry_prompt"]
    assert "full orchestration route -> `team`" in alias["entry_prompt"]
    assert "bounded subagent lane -> `subagent-delegation`" in alias["entry_prompt"]
    assert "worker write scope -> `lane-local-delta-only`" in alias["entry_prompt"]
    assert alias["state_machine"]["current_state"] in {
        "scoping-active",
        "delegation-planned",
        "worker-running",
        "integration-pending",
        "qa-in-progress",
        "cleanup-pending",
        "fresh-entry",
        "stale-continuity",
        "inconsistent-continuity",
        "cleanup-completed",
    }


def test_rust_route_adapter_framework_alias_compact_mode_omits_heavy_metadata(
    tmp_path: Path,
) -> None:
    adapter = RustRouteAdapter(PROJECT_ROOT)
    _seed_framework_runtime_artifacts(tmp_path, terminal=False)

    alias = adapter.framework_alias(repo_root=tmp_path, alias="autopilot", max_lines=3, compact=True)

    assert alias["compact"] is True
    assert alias["interaction_invariants"]["requires_explicit_entrypoint"] is True
    assert alias["interaction_invariants"]["explicit_entrypoints"] == ["/autopilot", "$autopilot"]
    assert alias["interaction_invariants"]["implicit_route_policy"] == "never"
    assert "entry_prompt" not in alias
    assert "entry_prompt_token_estimate" not in alias
    assert "upstream_source" not in alias
    assert "official_workflow" not in alias
    assert "local_adaptations" not in alias
    assert alias["host_entrypoint"] == "$autopilot"
    assert alias["state_machine"]["resume"]["mode"] == "continue-current-task"
    assert alias["state_machine"]["evidence_missing"] is True
    assert alias["entry_contract"]["context"]["execution_readiness"] == "needs_verification"
    assert alias["state_machine"]["required_anchors"] == [
        "SESSION_SUMMARY",
        "NEXT_ACTIONS",
        "TRACE_METADATA",
        "SUPERVISOR_STATE",
    ]
    assert "task" not in alias["state_machine"]["resume"]
    assert alias["entry_contract"]["skill_fallback_path"] == "skills/autopilot/SKILL.md"
    assert alias["entry_contract"]["decision_contract"] is None


def test_rust_route_adapter_framework_alias_supports_claude_host_entrypoints(
    tmp_path: Path,
) -> None:
    adapter = RustRouteAdapter(PROJECT_ROOT)
    _seed_framework_runtime_artifacts(tmp_path, terminal=False)

    alias = adapter.framework_alias(
        repo_root=tmp_path,
        alias="autopilot",
        max_lines=3,
        compact=True,
        host_id="claude-code",
    )

    assert alias["host_entrypoint"] == "/autopilot"


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
