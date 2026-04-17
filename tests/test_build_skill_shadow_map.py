"""Regression tests for source precedence and shadow map generation."""

from __future__ import annotations

import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.build_skill_shadow_map import build_shadow_map


def _write_skill(skill_dir: Path, *, name: str, source: str | None = None) -> None:
    """Write a minimal SKILL.md fixture for shadow map tests.

    Parameters:
        skill_dir: Target skill directory.
        name: Frontmatter skill name.
        source: Optional source override.

    Returns:
        None.
    """

    skill_dir.mkdir(parents=True, exist_ok=True)
    frontmatter_lines = [
        "---",
        f"name: {name}",
        'description: "test skill description"',
        "routing_layer: L2",
        "routing_owner: owner",
        "routing_gate: none",
        "session_start: n/a",
    ]
    if source:
        frontmatter_lines.append(f"source: {source}")
    frontmatter_lines.extend(
        [
            "---",
            "",
            "# test",
            "",
            "## When to use",
            "- test",
            "",
            "## Do not use",
            "- test",
            "",
        ]
    )
    (skill_dir / "SKILL.md").write_text("\n".join(frontmatter_lines), encoding="utf-8")


def test_shadow_map_prefers_later_source_by_precedence(tmp_path: Path) -> None:
    """Verify project skills shadow system skills with the same slug.

    Parameters:
        tmp_path: Temporary pytest directory fixture.

    Returns:
        None.
    """

    skills_root = tmp_path / "skills"
    _write_skill(skills_root / ".system" / "duplicate-skill", name="duplicate-skill", source="system")
    _write_skill(skills_root / "duplicate-skill", name="duplicate-skill", source="project")

    shadow_map = build_shadow_map(skills_root=skills_root)
    duplicate = shadow_map["skills"]["duplicate-skill"]

    assert duplicate["has_shadow"] is True
    assert duplicate["winner"]["source"] == "project"
    assert duplicate["shadowed"][0]["source"] == "system"


def test_shadow_map_tracks_non_shadowed_skill_with_winner_metadata(tmp_path: Path) -> None:
    """Verify a unique skill still receives source metadata in the shadow map.

    Parameters:
        tmp_path: Temporary pytest directory fixture.

    Returns:
        None.
    """

    skills_root = tmp_path / "skills"
    _write_skill(skills_root / "solo-skill", name="solo-skill")

    shadow_map = build_shadow_map(skills_root=skills_root)
    solo = shadow_map["skills"]["solo-skill"]

    assert solo["has_shadow"] is False
    assert solo["winner"]["source"] == "project"
    assert solo["shadowed"] == []


def test_shadow_map_discovers_nested_skill_dirs_without_container_entries(tmp_path: Path) -> None:
    skills_root = tmp_path / "skills"
    _write_skill(skills_root / "codex-primary-runtime" / "spreadsheets", name="spreadsheets")
    (skills_root / "codex-primary-runtime" / "notes").mkdir(parents=True, exist_ok=True)

    shadow_map = build_shadow_map(skills_root=skills_root)

    assert "spreadsheets" in shadow_map["skills"]
    assert "codex-primary-runtime" not in shadow_map["skills"]
    assert (
        shadow_map["skills"]["spreadsheets"]["winner"]["path"]
        == str((skills_root / "codex-primary-runtime" / "spreadsheets"))
    )
