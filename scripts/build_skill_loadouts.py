#!/usr/bin/env python3
"""Build the canonical skill loadout registry for controller and subagent use."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
SKILLS_ROOT = ROOT / "skills"
MANIFEST_PATH = SKILLS_ROOT / "SKILL_MANIFEST.json"
LOADOUTS_PATH = SKILLS_ROOT / "SKILL_LOADOUTS.json"

DEFAULT_LOADOUTS = {
    "version": 1,
    "loadouts": {
        "research_loadout": {
            "owners": ["information-retrieval", "github-investigator", "academic-search"],
            "overlays": ["anti-laziness"],
            "exclude": ["plan-to-code", "react"],
            "purpose": "Bounded research, repo investigation, and evidence gathering.",
        },
        "implementation_loadout": {
            "owners": ["plan-to-code", "typescript-pro", "python-pro", "react"],
            "overlays": ["test-engineering", "frontend-code-quality"],
            "exclude": ["academic-search"],
            "purpose": "Concrete implementation and refactor execution with test support.",
        },
        "audit_loadout": {
            "owners": ["execution-audit-codex", "code-review", "security-audit"],
            "overlays": ["anti-laziness"],
            "exclude": ["brainstorm-research"],
            "purpose": "Strict sign-off, audit, verification, and issue surfacing.",
        },
        "framework_loadout": {
            "owners": ["skill-developer-codex", "execution-controller-coding", "idea-to-plan"],
            "overlays": ["execution-audit-codex"],
            "exclude": ["seo-web"],
            "purpose": "Framework design, routing policy, and orchestrator evolution work.",
        },
        "ops_loadout": {
            "owners": ["git-workflow", "linux-server-ops", "observability"],
            "overlays": ["execution-audit-codex"],
            "exclude": ["paper-writing"],
            "purpose": "Operational changes, deployment support, and production diagnostics.",
        },
    },
}


def load_manifest(path: Path = MANIFEST_PATH) -> dict[str, Any]:
    """Load the skill manifest from disk.

    Parameters:
        path: Skill manifest path.

    Returns:
        dict[str, Any]: Parsed manifest payload.
    """

    return json.loads(path.read_text(encoding="utf-8"))


def available_skills(manifest: dict[str, Any]) -> set[str]:
    """Collect skill slugs from the compressed manifest.

    Parameters:
        manifest: Parsed skill manifest.

    Returns:
        set[str]: Available skill slugs.
    """

    return {row[0] for row in manifest.get("skills", []) if isinstance(row, list) and row}


def validate_loadouts(payload: dict[str, Any], manifest: dict[str, Any]) -> None:
    """Validate that all referenced skills exist in the manifest.

    Parameters:
        payload: Candidate loadout payload.
        manifest: Parsed skill manifest.

    Returns:
        None.
    """

    known = available_skills(manifest)
    missing: list[str] = []
    for name, config in payload.get("loadouts", {}).items():
        for key in ("owners", "overlays", "exclude"):
            for skill in config.get(key, []):
                if skill not in known:
                    missing.append(f"{name}:{key}:{skill}")
    if missing:
        raise SystemExit(f"Unknown skills referenced by loadouts: {', '.join(missing)}")


def write_loadouts(path: Path, payload: dict[str, Any]) -> None:
    """Write the loadout payload using stable formatting.

    Parameters:
        path: Target output path.
        payload: Loadout payload to persist.

    Returns:
        None.
    """

    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def main() -> int:
    """CLI entry point for building skill loadouts.

    Parameters:
        None.

    Returns:
        int: Process exit code.
    """

    parser = argparse.ArgumentParser(description="Build the canonical skill loadout registry.")
    parser.add_argument("--manifest", type=Path, default=MANIFEST_PATH, help="Skill manifest path.")
    parser.add_argument("--output", type=Path, default=LOADOUTS_PATH, help="Loadout output path.")
    parser.add_argument("--apply", action="store_true", help="Write the loadout file to disk.")
    args = parser.parse_args()

    manifest = load_manifest(args.manifest)
    payload = DEFAULT_LOADOUTS
    validate_loadouts(payload, manifest)

    if args.apply:
        write_loadouts(args.output, payload)
        print(f"Wrote {args.output}")
    else:
        print(json.dumps(payload, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
