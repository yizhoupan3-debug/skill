"""Regression tests for skill loadout generation."""

from __future__ import annotations

import json
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.build_skill_loadouts import DEFAULT_LOADOUTS, validate_loadouts


def test_default_loadouts_reference_existing_skills() -> None:
    """Verify default loadouts only reference skills present in the manifest.

    Parameters:
        None.

    Returns:
        None.
    """

    manifest = json.loads((PROJECT_ROOT / "skills" / "SKILL_MANIFEST.json").read_text(encoding="utf-8"))
    validate_loadouts(DEFAULT_LOADOUTS, manifest)


def test_default_loadouts_include_required_phase2_sets() -> None:
    """Verify the required loadout names exist in the default payload.

    Parameters:
        None.

    Returns:
        None.
    """

    loadouts = DEFAULT_LOADOUTS["loadouts"]
    assert {"research_loadout", "implementation_loadout", "audit_loadout", "framework_loadout", "ops_loadout"} <= set(loadouts)
