"""Regression tests for framework surface policy generation."""

from __future__ import annotations

import json
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.build_framework_surface_policy import build_framework_surface_policy


def test_generated_framework_surface_policy_matches_live_file() -> None:
    loadouts = json.loads((PROJECT_ROOT / "skills" / "SKILL_LOADOUTS.json").read_text(encoding="utf-8"))
    tiers = json.loads((PROJECT_ROOT / "skills" / "SKILL_TIERS.json").read_text(encoding="utf-8"))
    expected = json.loads(
        (PROJECT_ROOT / "configs" / "framework" / "FRAMEWORK_SURFACE_POLICY.json").read_text(
            encoding="utf-8"
        )
    )

    payload = build_framework_surface_policy(loadouts, tiers)

    assert payload == expected


def test_framework_surface_policy_keeps_kernel_small_and_default_surface_explicit() -> None:
    payload = json.loads(
        (PROJECT_ROOT / "configs" / "framework" / "FRAMEWORK_SURFACE_POLICY.json").read_text(
            encoding="utf-8"
        )
    )

    assert payload["kernel"]["canonical_axes"] == [
        "routing",
        "memory",
        "continuity",
        "host_projection",
    ]
    assert payload["migration_guardrails"]["avoid_runtime_kernel_fork"] is True
    assert payload["default_surface"]["default_loadouts"] == ["default_surface_loadout"]
    assert "research_loadout" in payload["default_surface"]["explicit_opt_in_loadouts"]
    assert payload["default_surface"]["lean_default_owners"] == [
        "plan-to-code",
        "python-pro",
        "typescript-pro",
        "git-workflow",
        "shell-cli",
    ]
    assert [metric["id"] for metric in payload["outcome_metrics"]] == [
        "first_attempt_success_rate",
        "cross_host_consistency",
        "checkpoint_resume_success_rate",
        "new_task_onboarding_cost",
    ]
