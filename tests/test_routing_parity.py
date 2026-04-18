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

ROUTE_FIXTURE_PATH = PROJECT_ROOT / "tests" / "routing_route_fixtures.json"
MISSING_RUNTIME_PATH = PROJECT_ROOT / "tests" / "_routing_missing_runtime.json"
ROUTE_FIXTURES = json.loads(ROUTE_FIXTURE_PATH.read_text(encoding="utf-8"))
REAL_TASK_REPLAY_QUERIES = [
    "这是高负载跨文件任务，需要 sidecar delegation 并行处理",
    "帮我写一个 Rust CLI 工具",
    "把这份 runtime checklist 落成代码，并保留 Python host 接口",
    "review checklist 看这轮是否结束",
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


def test_route_report_contract_exposes_schema_and_stable_mismatch_vocabulary() -> None:
    """The Rust diff report should own schema, authority, and mismatch field vocabulary."""

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
    assert report["mismatch"] is True
    assert report["mismatch_fields"] == ["selected_skill", "score_bucket"]
