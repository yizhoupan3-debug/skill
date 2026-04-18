"""Regression tests for skill manifest generation."""

from __future__ import annotations

import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts import check_skills
from scripts.build_skill_shadow_map import collect_skill_entries
from scripts.sync_skills import build_registry_and_manifest


def _write_skill(skill_dir: Path, *, name: str, source: str | None = None) -> None:
    skill_dir.mkdir(parents=True, exist_ok=True)
    frontmatter = [
        "---",
        f"name: {name}",
        'description: "test skill description"',
        "routing_layer: L2",
        "routing_owner: owner",
        "routing_gate: none",
        "session_start: n/a",
    ]
    if source:
        frontmatter.append(f"source: {source}")
    frontmatter.extend(
        [
            "---",
            "",
            "## When to use",
            "- test",
            "",
        ]
    )
    (skill_dir / "SKILL.md").write_text("\n".join(frontmatter), encoding="utf-8")


def test_build_registry_and_manifest_keeps_only_highest_precedence_skill_per_slug(
    tmp_path: Path,
) -> None:
    skills_root = tmp_path / "skills"
    _write_skill(skills_root / ".system" / "imagegen", name="imagegen", source="system")
    _write_skill(skills_root / "imagegen", name="imagegen", source="project")

    documents = check_skills.load_skill_documents(skills_root, include_system=True)
    skill_entries = collect_skill_entries(skills_root=skills_root, skill_documents=documents)
    registry, manifest = build_registry_and_manifest(
        skill_documents=documents,
        skill_entries=skill_entries,
        health_data={},
    )

    assert len(manifest["skills"]) == 1
    [row] = manifest["skills"]
    assert row[0] == "imagegen"
    assert row[9] == "project"
    assert row[10] == 3
    assert registry.count("`imagegen`") == 1
