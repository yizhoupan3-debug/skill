#!/usr/bin/env python3
"""Package a skill directory into a distributable .skill archive (ZIP)."""

from __future__ import annotations

import argparse
import sys
import zipfile
from pathlib import Path

from check_skills import parse_frontmatter

EXCLUDE_PATTERNS = {
    "__pycache__",
    ".DS_Store",
    "node_modules",
    ".git",
    ".env",
    "*.pyc",
    "*.pyo",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
}


def should_exclude(path: Path) -> bool:
    """Check if a path should be excluded from the archive.

    Args:
        path: Relative path to check.

    Returns:
        True if the path should be excluded.
    """
    for part in path.parts:
        if part in EXCLUDE_PATTERNS:
            return True
        for pattern in EXCLUDE_PATTERNS:
            if pattern.startswith("*") and part.endswith(pattern[1:]):
                return True
    return False


def validate_skill(skill_dir: Path) -> tuple[str, str]:
    """Validate that the skill directory has a valid SKILL.md.

    Args:
        skill_dir: Path to the skill directory.

    Returns:
        Tuple of (name, description) from frontmatter.

    Raises:
        SystemExit: If SKILL.md is missing or invalid.
    """
    skill_md = skill_dir / "SKILL.md"
    if not skill_md.is_file():
        print(f"ERROR: missing SKILL.md in {skill_dir}", file=sys.stderr)
        sys.exit(1)

    text = skill_md.read_text(encoding="utf-8")
    metadata, _, error = parse_frontmatter(text)
    if error:
        print(f"ERROR: {error} in {skill_md}", file=sys.stderr)
        sys.exit(1)

    name = metadata.get("name", "").strip()
    description = metadata.get("description", "").strip()
    if not name:
        print(f"ERROR: missing 'name' in {skill_md}", file=sys.stderr)
        sys.exit(1)

    return name, description


def package_skill(skill_dir: Path, output_dir: Path) -> Path:
    """Package a skill directory into a .skill ZIP archive.

    Args:
        skill_dir: Path to the skill directory.
        output_dir: Directory to write the .skill file to.

    Returns:
        Path to the created .skill file.
    """
    name, description = validate_skill(skill_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    output_path = output_dir / f"{name}.skill"

    file_count = 0
    with zipfile.ZipFile(output_path, "w", zipfile.ZIP_DEFLATED) as zf:
        for file_path in sorted(skill_dir.rglob("*")):
            if not file_path.is_file():
                continue
            rel_path = file_path.relative_to(skill_dir.parent)
            if should_exclude(rel_path):
                continue
            zf.write(file_path, rel_path)
            file_count += 1

    size_kb = output_path.stat().st_size / 1024
    print(f"Packaged {name}: {file_count} files, {size_kb:.1f} KB → {output_path}")
    return output_path


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Package skill directories into .skill archives (ZIP).",
    )
    parser.add_argument(
        "skill_dir",
        type=Path,
        nargs="?",
        help="Path to a single skill directory to package.",
    )
    parser.add_argument(
        "output_dir",
        type=Path,
        nargs="?",
        default=Path(__file__).resolve().parents[1] / "skills" / "dist",
        help="Output directory for .skill files (default: skills/dist).",
    )
    parser.add_argument(
        "--all",
        action="store_true",
        help="Package all skills under skills/.",
    )
    parser.add_argument(
        "--skills-root",
        type=Path,
        default=Path(__file__).resolve().parents[1] / "skills",
        help="Path to the skills root directory.",
    )
    args = parser.parse_args()

    if args.all:
        skills_root = args.skills_root.resolve()
        if not skills_root.is_dir():
            print(f"Skills root not found: {skills_root}", file=sys.stderr)
            return 1

        count = 0
        for entry in sorted(skills_root.iterdir()):
            if not entry.is_dir():
                continue
            if entry.name.startswith(".") or entry.name == "dist":
                continue
            skill_md = entry / "SKILL.md"
            if not skill_md.is_file():
                continue
            package_skill(entry, args.output_dir)
            count += 1

        print(f"\nPackaged {count} skills to {args.output_dir}")
        return 0

    if args.skill_dir is None:
        parser.error("either provide a skill directory or use --all")

    skill_dir = args.skill_dir.resolve()
    if not skill_dir.is_dir():
        print(f"Skill directory not found: {skill_dir}", file=sys.stderr)
        return 1

    package_skill(skill_dir, args.output_dir)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
