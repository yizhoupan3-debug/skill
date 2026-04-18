#!/usr/bin/env python3
"""Build the approval-policy registry from declarative skill frontmatter."""

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
POLICY_PATH = SKILLS_ROOT / "SKILL_APPROVAL_POLICY.json"


def normalize_list(value: Any) -> list[str]:
    """Normalize a frontmatter field into a list of strings.

    Parameters:
        value: Raw frontmatter value.

    Returns:
        list[str]: Normalized list values.
    """

    if value is None:
        return []
    if isinstance(value, str):
        return [value]
    if isinstance(value, list):
        return [str(item) for item in value]
    return [str(value)]


def build_policy(
    skills_root: Path = SKILLS_ROOT,
    skill_documents: list[check_skills.SkillDocument] | None = None,
) -> dict[str, Any]:
    """Build the skill approval policy registry.

    Parameters:
        skills_root: Root directory containing skill packages.

    Returns:
        dict[str, Any]: Approval policy payload.
    """

    skills: dict[str, Any] = {}
    documents = skill_documents or check_skills.load_skill_documents(skills_root, include_system=True)
    for document in documents:
        slug = document.slug
        metadata = document.metadata

        skills[slug] = {
            "allowed_tools": normalize_list(metadata.get("allowed_tools")),
            "approval_required_tools": normalize_list(metadata.get("approval_required_tools")),
            "filesystem_scope": normalize_list(metadata.get("filesystem_scope")),
            "network_access": metadata.get("network_access", "unspecified"),
            "destructive_risk": metadata.get("destructive_risk", "unspecified"),
            "bridge_behavior": metadata.get("bridge_behavior", "default"),
            "artifact_outputs": normalize_list(metadata.get("artifact_outputs")),
        }

    return {"version": 2, "schema_version": "skill-approval-policy-v2", "skills": skills}


def write_policy(path: Path, payload: dict[str, Any]) -> None:
    """Write the approval policy file to disk.

    Parameters:
        path: Output JSON path.
        payload: Approval policy payload.

    Returns:
        None.
    """

    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def main() -> int:
    """CLI entry point for building the approval policy registry.

    Parameters:
        None.

    Returns:
        int: Process exit code.
    """

    parser = argparse.ArgumentParser(description="Build skill approval policy registry.")
    parser.add_argument("--skills-root", type=Path, default=SKILLS_ROOT, help="Skills root path.")
    parser.add_argument("--output", type=Path, default=POLICY_PATH, help="Output JSON path.")
    parser.add_argument("--apply", action="store_true", help="Write the output file to disk.")
    args = parser.parse_args()

    payload = build_policy(args.skills_root)
    if args.apply:
        write_policy(args.output, payload)
        print(f"Wrote {args.output}")
    else:
        print(json.dumps(payload, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
