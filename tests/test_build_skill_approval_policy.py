"""Regression tests for approval policy registry generation."""

from __future__ import annotations

import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.build_skill_approval_policy import build_policy


def test_approval_policy_contains_declarative_fields_for_controller_skill() -> None:
    """Verify policy generation captures declarative approval fields.

    Parameters:
        None.

    Returns:
        None.
    """

    policy = build_policy(PROJECT_ROOT / "skills")
    assert policy["schema_version"] == "skill-approval-policy-v2"
    controller = policy["skills"]["execution-controller-coding"]
    assert "git push" in controller["approval_required_tools"]
    assert "repo" in controller["filesystem_scope"]
    assert "SESSION_SUMMARY.md" in controller["artifact_outputs"]


def test_build_policy_normalizes_filesystem_scope_to_list() -> None:
    policy = build_policy(PROJECT_ROOT / "skills")

    assert isinstance(policy["skills"]["pdf"]["filesystem_scope"], list)
    assert "artifacts" in policy["skills"]["pdf"]["filesystem_scope"]
