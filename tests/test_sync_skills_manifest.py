"""Regression tests for skill manifest generation."""

from __future__ import annotations

import os
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts import check_skills, sync_skills
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


def test_resolve_skill_compiler_binary_ignores_debug_binary_until_release_is_fresh(
    tmp_path: Path,
    monkeypatch,
) -> None:
    crate_root = tmp_path / "skill-compiler-rs"
    release_bin = tmp_path / "target" / "release" / "skill-compiler-rs"
    debug_bin = tmp_path / "target" / "debug" / "skill-compiler-rs"
    manifest_path = crate_root / "Cargo.toml"
    src_main = crate_root / "src" / "main.rs"
    src_main.parent.mkdir(parents=True)
    debug_bin.parent.mkdir(parents=True)
    debug_bin.write_text("debug", encoding="utf-8")
    manifest_path.write_text("[package]\nname='skill-compiler-rs'\nversion='0.1.0'\n", encoding="utf-8")
    src_main.write_text("fn main() {}\n", encoding="utf-8")

    monkeypatch.setattr(sync_skills, "SKILL_COMPILER_RS_RELEASE_BIN", release_bin)
    monkeypatch.setattr(sync_skills, "SKILL_COMPILER_RS_DEBUG_BIN", debug_bin)
    monkeypatch.setattr(sync_skills, "SKILL_COMPILER_RS_DIR", crate_root)

    assert sync_skills.resolve_skill_compiler_binary() is None

    release_bin.parent.mkdir(parents=True, exist_ok=True)
    release_bin.write_text("release", encoding="utf-8")
    src_mtime = 1_700_000_100
    release_mtime = 1_700_000_200
    manifest_mtime = 1_700_000_050
    os.utime(manifest_path, (manifest_mtime, manifest_mtime))
    os.utime(src_main, (src_mtime, src_mtime))
    os.utime(release_bin, (release_mtime, release_mtime))

    assert sync_skills.resolve_skill_compiler_binary() == release_bin


def test_resolve_skill_compiler_binary_rejects_stale_release_binary(
    tmp_path: Path,
    monkeypatch,
) -> None:
    crate_root = tmp_path / "skill-compiler-rs"
    release_bin = tmp_path / "target" / "release" / "skill-compiler-rs"
    manifest_path = crate_root / "Cargo.toml"
    src_main = crate_root / "src" / "main.rs"
    src_main.parent.mkdir(parents=True)
    release_bin.parent.mkdir(parents=True)
    manifest_path.write_text("[package]\nname='skill-compiler-rs'\nversion='0.1.0'\n", encoding="utf-8")
    src_main.write_text("fn main() {}\n", encoding="utf-8")
    release_bin.write_text("release", encoding="utf-8")

    monkeypatch.setattr(sync_skills, "SKILL_COMPILER_RS_RELEASE_BIN", release_bin)
    monkeypatch.setattr(sync_skills, "SKILL_COMPILER_RS_DIR", crate_root)

    release_mtime = 1_700_000_100
    src_mtime = 1_700_000_200
    manifest_mtime = 1_700_000_150
    os.utime(release_bin, (release_mtime, release_mtime))
    os.utime(manifest_path, (manifest_mtime, manifest_mtime))
    os.utime(src_main, (src_mtime, src_mtime))

    assert sync_skills.resolve_skill_compiler_binary() is None
