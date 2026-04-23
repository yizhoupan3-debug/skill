from __future__ import annotations

import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.analyze_routing import check_index_coverage, load_trigger_index_skills


def test_load_trigger_index_skills_reads_live_routing_index() -> None:
    skills = load_trigger_index_skills()

    assert "skill-framework-developer" in skills
    assert "gh-pr-triage" in skills


def test_check_index_coverage_uses_live_routing_index() -> None:
    entries = [{"expected": "skill-framework-developer"}, {"expected": "gh-pr-triage"}]

    assert check_index_coverage(entries) == []
