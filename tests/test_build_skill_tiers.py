"""Regression tests for skill tier generation."""

from __future__ import annotations

import json
import sys
from pathlib import Path

import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.build_skill_tiers import build_skill_tiers, validate_skill_tiers


def test_generated_skill_tiers_match_live_file() -> None:
    manifest = json.loads((PROJECT_ROOT / "skills" / "SKILL_MANIFEST.json").read_text(encoding="utf-8"))
    health_manifest = json.loads(
        (PROJECT_ROOT / "skills" / "SKILL_HEALTH_MANIFEST.json").read_text(encoding="utf-8")
    )
    loadouts = json.loads((PROJECT_ROOT / "skills" / "SKILL_LOADOUTS.json").read_text(encoding="utf-8"))
    expected = json.loads((PROJECT_ROOT / "skills" / "SKILL_TIERS.json").read_text(encoding="utf-8"))

    payload = build_skill_tiers(manifest, health_manifest, loadouts)
    validate_skill_tiers(payload, manifest)

    assert payload == expected


def test_generated_skill_tiers_classify_representative_live_skills() -> None:
    payload = json.loads((PROJECT_ROOT / "skills" / "SKILL_TIERS.json").read_text(encoding="utf-8"))
    skills = payload["skills"]

    assert skills["openai-docs"]["tier"] == "core"
    assert skills["openai-docs"]["surface"]["activation_mode"] == "default"
    assert skills["execution-controller-coding"]["tier"] == "core"
    assert skills["subagent-delegation"]["tier"] == "core"
    assert skills["plan-to-code"]["tier"] == "optional"
    assert skills["plan-to-code"]["surface"]["activation_mode"] == "default"
    assert skills["github-investigator"]["tier"] == "experimental"
    assert skills["github-investigator"]["surface"]["activation_mode"] == "explicit_opt_in"
    assert skills["iterative-optimizer"]["tier"] == "experimental"


def test_build_skill_tiers_ignores_exclude_memberships_for_core() -> None:
    manifest = {
        "keys": [
            "slug",
            "layer",
            "owner",
            "gate",
            "priority",
            "description",
            "session_start",
            "trigger_hints",
            "health",
            "source",
            "source_position",
        ],
        "skills": [
            ["excluded-skill", "L4", "owner", "none", "P2", "Excluded only", "n/a", [], 96.0, "project", 1],
            ["gate-skill", "L0", "gate", "source", "P1", "Gate skill", "required", [], 98.0, "project", 1],
        ],
    }
    health_manifest = {
        "skills": {
            "excluded-skill": {
                "dynamic_score": 96.0,
                "static_score": 95.0,
                "usage_30d": 0,
                "reroutes_30d": 0,
                "health_status": "Healthy",
            },
            "gate-skill": {
                "dynamic_score": 98.0,
                "static_score": 96.0,
                "usage_30d": 0,
                "reroutes_30d": 0,
                "health_status": "Healthy",
            },
        }
    }
    loadouts = {
        "version": 1,
        "loadouts": {
            "sample_loadout": {
                "owners": [],
                "overlays": [],
                "exclude": ["excluded-skill"],
                "purpose": "test",
            }
        },
    }

    payload = build_skill_tiers(manifest, health_manifest, loadouts)

    assert payload["skills"]["excluded-skill"]["tier"] == "optional"
    assert payload["skills"]["excluded-skill"]["surface"]["activation_mode"] == "explicit_opt_in"
    assert payload["skills"]["gate-skill"]["tier"] == "core"
    assert payload["skills"]["gate-skill"]["surface"]["activation_mode"] == "default"


def test_build_skill_tiers_marks_unused_low_health_rerouted_skill_as_deprecated() -> None:
    manifest = {
        "keys": [
            "slug",
            "layer",
            "owner",
            "gate",
            "priority",
            "description",
            "session_start",
            "trigger_hints",
            "health",
            "source",
            "source_position",
        ],
        "skills": [
            ["retire-me", "L4", "owner", "none", "P2", "Retire candidate", "n/a", [], 50.0, "project", 1],
            ["healthy-skill", "L4", "owner", "none", "P2", "Healthy skill", "n/a", [], 96.0, "project", 1],
        ],
    }
    health_manifest = {
        "skills": {
            "retire-me": {
                "dynamic_score": 50.0,
                "static_score": 78.0,
                "usage_30d": 0,
                "reroutes_30d": 2,
                "health_status": "Stable",
            },
            "healthy-skill": {
                "dynamic_score": 96.0,
                "static_score": 95.0,
                "usage_30d": 1,
                "reroutes_30d": 0,
                "health_status": "Healthy",
            },
        }
    }
    loadouts = {"version": 1, "loadouts": {}}

    payload = build_skill_tiers(manifest, health_manifest, loadouts)

    assert payload["skills"]["retire-me"]["tier"] == "deprecated"
    assert payload["skills"]["retire-me"]["surface"]["activation_mode"] == "disabled"
    assert payload["skills"]["healthy-skill"]["tier"] == "optional"
    assert payload["skills"]["healthy-skill"]["surface"]["activation_mode"] == "explicit_opt_in"


def test_validate_skill_tiers_rejects_missing_coverage() -> None:
    manifest = {
        "keys": ["slug", "layer", "owner", "gate", "priority", "description", "session_start", "trigger_hints", "health"],
        "skills": [["only-skill", "L4", "owner", "none", "P2", "Only skill", "n/a", [], 95.0]],
    }
    payload = {
        "tiers": {
            "core": [],
            "optional": [],
            "experimental": [],
            "deprecated": [],
        },
        "skills": {},
    }

    with pytest.raises(SystemExit, match="coverage:missing:only-skill"):
        validate_skill_tiers(payload, manifest)
