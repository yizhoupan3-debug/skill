"""Regression tests for multilingual routing and health backfill."""

from __future__ import annotations

import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
SCRIPTS_ROOT = PROJECT_ROOT / "scripts"
if str(SCRIPTS_ROOT) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_ROOT))

from scripts.route import search_skills
from scripts.sync_skills import extract_trigger_phrases, normalize_health_manifest


def test_extract_trigger_phrases_preserves_multilingual_terms() -> None:
    """Verify trigger extraction keeps Chinese and English routing phrases.

    Returns:
        None.
    """

    phrases = extract_trigger_phrases(
        {
            "trigger_phrases": ["github深度调研", "issue PR 时间线"],
        },
        'Use for "github深度调研 / repo对标 / issue-PR演化分析" and code history.',
        "## When to use\n- 看这个开源项目怎么做的\n- issue PR 时间线\n",
    )

    assert "github深度调研" in phrases
    assert "repo对标" in phrases
    assert "issue PR 时间线" in phrases
    assert "history" in " ".join(phrases).lower()


def test_normalize_health_manifest_backfills_missing_skill_rows() -> None:
    """Verify health manifest normalization covers every manifest skill.

    Returns:
        None.
    """

    manifest = {
        "skills": [
            ["alpha-skill", "L2", "owner", "none", "P2", "Alpha", "n/a", "alpha", 95.0, "project", 40],
            ["beta-skill", "L2", "owner", "none", "P2", "Beta", "n/a", "beta", 95.0, "project", 40],
        ]
    }

    normalized = normalize_health_manifest(manifest)

    assert normalized["summary"]["total_skills"] == 2
    assert set(normalized["skills"]) == {"alpha-skill", "beta-skill"}
    assert normalized["skills"]["beta-skill"]["dynamic_score"] == 100.0


def test_search_skills_matches_multilingual_iterative_query() -> None:
    """Verify the router finds iterative optimizer from mixed Chinese queries.

    Returns:
        None.
    """

    results = search_skills("自迭代 10轮 优化 验证", limit=3)

    assert results
    assert results[0].record.slug == "iterative-optimizer"


def test_search_skills_matches_memory_and_native_debug_queries() -> None:
    """Verify multilingual mixed queries hit the intended specialist skills.

    Returns:
        None.
    """

    memory_results = search_skills("agent 长期记忆 跨会话 memory layer", limit=3)
    native_results = search_skills("Mac 桌面 app 原生 调试 wkwebview ipc", limit=3)

    assert memory_results
    assert memory_results[0].record.slug == "agent-memory"
    assert native_results
    assert native_results[0].record.slug == "native-app-debugging"
