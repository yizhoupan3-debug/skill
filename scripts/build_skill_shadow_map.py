#!/usr/bin/env python3
"""Build source-precedence and shadow metadata for the skill library."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_skills


ROOT = Path(__file__).resolve().parents[1]
SKILLS_ROOT = ROOT / "skills"
SOURCE_MANIFEST_PATH = SKILLS_ROOT / "SKILL_SOURCE_MANIFEST.json"
SHADOW_MAP_PATH = SKILLS_ROOT / "SKILL_SHADOW_MAP.json"

DEFAULT_SOURCE_MANIFEST = {
    "version": 1,
    "resolution": "later-source-wins",
    "sources": [
        {"name": "system", "priority": 100, "position": 0},
        {"name": "vendor", "priority": 80, "position": 1},
        {"name": "user", "priority": 60, "position": 2},
        {"name": "project", "priority": 40, "position": 3},
    ],
}

SOURCE_ALIASES = {
    "local": "project",
    "community": "project",
    "community-adapted": "project",
    "local - trainer": "project",
}


def repo_relative(path: Path) -> str:
    """Return a repository-relative path string.

    Parameters:
        path: Target file or directory path.

    Returns:
        str: Repository-relative POSIX path when possible.
    """

    try:
        return path.resolve().relative_to(ROOT.resolve()).as_posix()
    except ValueError:
        return str(path)


def load_source_manifest(path: Path = SOURCE_MANIFEST_PATH) -> dict[str, Any]:
    """Load the source manifest or return the default contract.

    Parameters:
        path: Manifest path on disk.

    Returns:
        dict[str, Any]: Parsed source manifest data.
    """

    if not path.exists():
        return DEFAULT_SOURCE_MANIFEST
    return json.loads(path.read_text(encoding="utf-8"))


def normalize_source_name(raw_source: str | None) -> str:
    """Normalize a declared source name to the canonical precedence name.

    Parameters:
        raw_source: Source string from frontmatter or inference.

    Returns:
        str: Canonical source name.
    """

    if not raw_source:
        return "project"
    normalized = str(raw_source).strip().lower()
    return SOURCE_ALIASES.get(normalized, normalized)


def infer_skill_source(skill_dir: Path, metadata: dict[str, Any], skills_root: Path = SKILLS_ROOT) -> str:
    """Infer the skill source from frontmatter or path layout.

    Parameters:
        skill_dir: Directory containing the skill package.
        metadata: Parsed SKILL.md frontmatter.
        skills_root: Root directory containing skill packages.

    Returns:
        str: Canonical source category.
    """

    declared = normalize_source_name(str(metadata.get("source", "")).strip() or None)
    if declared != "project":
        return declared

    relative = skill_dir.resolve().relative_to(skills_root.resolve())
    head = relative.parts[0]
    if head == ".system":
        return "system"
    if head == "vendor":
        return "vendor"
    if head == "user":
        return "user"
    return "project"


def build_precedence_map(source_manifest: dict[str, Any]) -> dict[str, dict[str, Any]]:
    """Build precedence lookup metadata keyed by source name.

    Parameters:
        source_manifest: Source precedence manifest.

    Returns:
        dict[str, dict[str, Any]]: Normalized source metadata.
    """

    precedence: dict[str, dict[str, Any]] = {}
    for position, entry in enumerate(source_manifest.get("sources", [])):
        name = normalize_source_name(entry.get("name"))
        precedence[name] = {
            "name": name,
            "priority": entry.get("priority"),
            "position": entry.get("position", position),
        }
    return precedence


def collect_skill_entries(
    skills_root: Path = SKILLS_ROOT,
    source_manifest: dict[str, Any] | None = None,
    skill_documents: list[check_skills.SkillDocument] | None = None,
) -> list[dict[str, Any]]:
    """Collect skill metadata needed for source and shadow analysis.

    Parameters:
        skills_root: Root directory containing skill packages.
        source_manifest: Optional source precedence manifest.

    Returns:
        list[dict[str, Any]]: Skill entry records.
    """

    precedence = build_precedence_map(source_manifest or DEFAULT_SOURCE_MANIFEST)
    entries: list[dict[str, Any]] = []

    documents = skill_documents or check_skills.load_skill_documents(skills_root, include_system=True)
    for document in documents:
        slug = document.slug
        skill_dir = document.skill_dir
        metadata = document.metadata

        source_name = infer_skill_source(skill_dir, metadata, skills_root=skills_root)
        source_info = precedence.get(source_name, {"priority": None, "position": -1})

        entries.append(
            {
                "slug": slug,
                "path": repo_relative(skill_dir),
                "source": source_name,
                "source_priority": source_info.get("priority"),
                "source_position": source_info.get("position", -1),
                "routing_layer": str(metadata.get("routing_layer", "")).strip(),
                "routing_owner": str(metadata.get("routing_owner", "")).strip(),
                "routing_gate": str(metadata.get("routing_gate", "")).strip(),
                "session_start": str(metadata.get("session_start", "")).strip(),
            }
        )
    return entries


def build_shadow_map(
    skills_root: Path = SKILLS_ROOT,
    source_manifest: dict[str, Any] | None = None,
    skill_documents: list[check_skills.SkillDocument] | None = None,
    skill_entries: list[dict[str, Any]] | None = None,
) -> dict[str, Any]:
    """Build the per-skill shadow map using source precedence rules.

    Parameters:
        skills_root: Root directory containing skill packages.
        source_manifest: Optional source precedence manifest.

    Returns:
        dict[str, Any]: Shadow map payload.
    """

    manifest = source_manifest or load_source_manifest()
    entries = skill_entries or collect_skill_entries(
        skills_root=skills_root,
        source_manifest=manifest,
        skill_documents=skill_documents,
    )
    grouped: dict[str, list[dict[str, Any]]] = {}
    for entry in entries:
        grouped.setdefault(entry["slug"], []).append(entry)

    skills_payload: dict[str, Any] = {}
    for slug, group in sorted(grouped.items()):
        ordered = sorted(
            group,
            key=lambda item: (item["source_position"], item["path"]),
        )
        winner = ordered[-1]
        shadowed = [item for item in ordered[:-1]]
        skills_payload[slug] = {
            "winner": winner,
            "shadowed": shadowed,
            "shadowed_by": [] if not shadowed else [winner["path"]],
            "has_shadow": bool(shadowed),
        }

    return {
        "version": 1,
        "resolution": manifest.get("resolution", "later-source-wins"),
        "sources": manifest.get("sources", []),
        "skills": skills_payload,
    }


def write_json(path: Path, payload: dict[str, Any]) -> None:
    """Write a JSON payload using stable pretty formatting.

    Parameters:
        path: Target JSON file path.
        payload: JSON-serializable data.

    Returns:
        None.
    """

    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def main() -> int:
    """CLI entry point for manifest and shadow map generation.

    Parameters:
        None.

    Returns:
        int: Process exit code.
    """

    parser = argparse.ArgumentParser(description="Build source and shadow metadata for skills.")
    parser.add_argument("--skills-root", type=Path, default=SKILLS_ROOT, help="Path to the skills root.")
    parser.add_argument("--source-manifest", type=Path, default=SOURCE_MANIFEST_PATH, help="Path to the source manifest.")
    parser.add_argument("--shadow-map", type=Path, default=SHADOW_MAP_PATH, help="Path to the shadow map.")
    parser.add_argument("--apply", action="store_true", help="Write generated files to disk.")
    args = parser.parse_args()

    source_manifest = load_source_manifest(args.source_manifest)
    shadow_map = build_shadow_map(skills_root=args.skills_root, source_manifest=source_manifest)

    if args.apply:
        write_json(args.source_manifest, source_manifest)
        write_json(args.shadow_map, shadow_map)
        print(f"Wrote {repo_relative(args.source_manifest)} and {repo_relative(args.shadow_map)}")
    else:
        print(json.dumps({"source_manifest": source_manifest, "shadow_map": shadow_map}, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
