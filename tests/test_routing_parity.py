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
