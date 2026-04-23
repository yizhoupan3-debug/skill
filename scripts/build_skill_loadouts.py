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
SHADOW_MAP_PATH = SKILLS_ROOT / "SKILL_SHADOW_MAP.json"
APPROVAL_POLICY_PATH = SKILLS_ROOT / "SKILL_APPROVAL_POLICY.json"
HEALTH_MANIFEST_PATH = SKILLS_ROOT / "SKILL_HEALTH_MANIFEST.json"
OVERLAY_ONLY_SKILLS = {"iterative-optimizer", "execution-audit", "i18n-l10n", "humanizer"}

DEFAULT_LOADOUTS = {
    "version": 2,
    "schema_version": "skill-loadouts-v2",
    "activation_policy": {
        "default_loadouts": ["default_surface_loadout"],
        "explicit_opt_in_loadouts": [
            "research_loadout",
            "implementation_loadout",
            "audit_loadout",
            "framework_loadout",
            "ops_loadout",
        ],
        "experimental_tiers": "explicit_opt_in",
        "deprecated_tiers": "disabled",
        "compatibility_surfaces": "explicit_opt_in",
    },
    "loadouts": {
        "default_surface_loadout": {
            "activation": "default",
            "surface_class": "default",
            "owners": [
                "plan-to-code",
                "python-pro",
                "typescript-pro",
                "gitx",
                "shell-cli",
            ],
            "overlays": ["anti-laziness"],
            "exclude": [
                "academic-search",
                "autoresearch",
                "github-investigator",
                "brainstorm-research",
                "copywriting",
                "iterative-optimizer",
                "paper-writing",
                "seo-web",
                "sustech-mailer",
            ],
            "purpose": "Single default day-to-day surface for implementation-first repo work; research, audit, and compatibility lanes stay explicit.",
        },
        "research_loadout": {
            "activation": "explicit",
            "surface_class": "specialist",
            "owners": ["information-retrieval", "github-investigator", "academic-search"],
            "overlays": ["anti-laziness"],
            "exclude": ["plan-to-code", "react"],
            "purpose": "Bounded research, repo investigation, and evidence gathering.",
        },
        "implementation_loadout": {
            "activation": "explicit",
            "surface_class": "specialist",
            "owners": ["plan-to-code", "typescript-pro", "python-pro", "react", "test-engineering"],
            "overlays": ["frontend-code-quality"],
            "exclude": ["academic-search"],
            "purpose": "Concrete implementation and refactor execution with test support.",
        },
        "audit_loadout": {
            "activation": "explicit",
            "surface_class": "specialist",
            "owners": [],
            "overlays": ["execution-audit", "code-review", "security-audit", "anti-laziness"],
            "exclude": ["brainstorm-research"],
            "purpose": "Strict sign-off, audit, verification, and issue surfacing.",
        },
        "framework_loadout": {
            "activation": "explicit",
            "surface_class": "specialist",
            "owners": ["skill-framework-developer", "execution-controller-coding", "idea-to-plan", "checklist-normalizer"],
            "overlays": ["execution-audit"],
            "exclude": ["seo-web"],
            "purpose": "Framework design, routing policy, orchestrator evolution, and execution-shape normalization work.",
        },
        "ops_loadout": {
            "activation": "explicit",
            "surface_class": "specialist",
            "owners": ["gitx", "linux-server-ops", "observability"],
            "overlays": ["execution-audit"],
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


def _manifest_key_index(manifest: dict[str, Any]) -> dict[str, int]:
    keys = manifest.get("keys", [])
    return {
        str(key): idx
        for idx, key in enumerate(keys)
        if isinstance(key, str)
    }


def classify_skills(manifest: dict[str, Any]) -> tuple[set[str], set[str]]:
    """Return overlay-only and owner-only slug sets from the manifest."""

    key_index = _manifest_key_index(manifest)
    idx_slug = key_index.get("slug")
    idx_owner = key_index.get("owner")
    if idx_slug is None or idx_owner is None:
        return set(OVERLAY_ONLY_SKILLS), set()

    overlay_only = set(OVERLAY_ONLY_SKILLS)
    owner_only: set[str] = set()
    for row in manifest.get("skills", []):
        if not isinstance(row, list) or len(row) <= max(idx_slug, idx_owner):
            continue
        slug = str(row[idx_slug])
        owner = str(row[idx_owner]).strip()
        if owner == "overlay":
            overlay_only.add(slug)
        elif owner == "owner":
            owner_only.add(slug)
    return overlay_only, owner_only - overlay_only


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def validate_loadouts(
    payload: dict[str, Any],
    manifest: dict[str, Any],
    *,
    shadow_map: dict[str, Any] | None = None,
    approval_policy: dict[str, Any] | None = None,
    health_manifest: dict[str, Any] | None = None,
) -> None:
    """Validate that all referenced skills exist in the manifest.

    Parameters:
        payload: Candidate loadout payload.
        manifest: Parsed skill manifest.

    Returns:
        None.
    """

    known = available_skills(manifest)
    overlay_only, owner_only = classify_skills(manifest)
    winner_skills = set((shadow_map or {}).get("skills", {}).keys()) if shadow_map else known
    policy_skills = set((approval_policy or {}).get("skills", {}).keys()) if approval_policy else known
    health_skills = set((health_manifest or {}).get("skills", {}).keys()) if health_manifest else known
    missing: list[str] = []
    conflicts: list[str] = []
    activation_policy = payload.get("activation_policy", {})
    default_loadouts = list(activation_policy.get("default_loadouts", []))
    explicit_opt_in_loadouts = list(activation_policy.get("explicit_opt_in_loadouts", []))
    if len(default_loadouts) != 1:
        conflicts.append("activation_policy:default_loadouts:expected-exactly-one")
    unknown_activation_refs = sorted(
        set(default_loadouts + explicit_opt_in_loadouts) - set(payload.get("loadouts", {}).keys())
    )
    if unknown_activation_refs:
        conflicts.append(
            "activation_policy:unknown-loadout:"
            + ",".join(unknown_activation_refs)
        )
    allowed_activation_values = {"default", "explicit"}
    default_marked: list[str] = []
    for name, config in payload.get("loadouts", {}).items():
        activation = str(config.get("activation", "explicit"))
        if activation not in allowed_activation_values:
            conflicts.append(f"{name}:activation:invalid:{activation}")
        if activation == "default":
            default_marked.append(name)
        owners = list(config.get("owners", []))
        overlays = list(config.get("overlays", []))
        excluded = list(config.get("exclude", []))
        buckets = {
            "owners": owners,
            "overlays": overlays,
            "exclude": excluded,
        }
        for key in ("owners", "overlays", "exclude"):
            for skill in config.get(key, []):
                if skill not in known:
                    missing.append(f"{name}:{key}:{skill}")
                if skill not in winner_skills:
                    conflicts.append(f"{name}:{key}:{skill}:not-a-shadow-winner")
                if skill not in policy_skills:
                    conflicts.append(f"{name}:{key}:{skill}:missing-approval-policy")
                if skill not in health_skills:
                    conflicts.append(f"{name}:{key}:{skill}:missing-health-manifest")
        for bucket_name, skills in buckets.items():
            if len(skills) != len(set(skills)):
                conflicts.append(f"{name}:{bucket_name}:duplicate-skill")
        if set(owners) & set(overlays):
            overlaps = ", ".join(sorted(set(owners) & set(overlays)))
            conflicts.append(f"{name}:owners-overlays-overlap:{overlaps}")
        if set(owners) & set(excluded):
            overlaps = ", ".join(sorted(set(owners) & set(excluded)))
            conflicts.append(f"{name}:owners-exclude-overlap:{overlaps}")
        if set(overlays) & set(excluded):
            overlaps = ", ".join(sorted(set(overlays) & set(excluded)))
            conflicts.append(f"{name}:overlays-exclude-overlap:{overlaps}")
        for skill in owners:
            if skill in overlay_only:
                conflicts.append(f"{name}:owners:{skill}:overlay-only")
        for skill in overlays:
            if skill in owner_only:
                conflicts.append(f"{name}:overlays:{skill}:owner-only")
    if sorted(default_marked) != sorted(default_loadouts):
        conflicts.append(
            "activation_policy:default-mismatch:"
            + ",".join(sorted(set(default_marked) ^ set(default_loadouts)))
        )
    if missing:
        raise SystemExit(f"Unknown skills referenced by loadouts: {', '.join(missing)}")
    if conflicts:
        raise SystemExit(f"Invalid loadout semantics: {', '.join(conflicts)}")


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
    parser.add_argument("--shadow-map", type=Path, default=SHADOW_MAP_PATH, help="Shadow map path.")
    parser.add_argument("--approval-policy", type=Path, default=APPROVAL_POLICY_PATH, help="Approval policy path.")
    parser.add_argument("--health-manifest", type=Path, default=HEALTH_MANIFEST_PATH, help="Health manifest path.")
    args = parser.parse_args()

    manifest = load_manifest(args.manifest)
    payload = DEFAULT_LOADOUTS
    validate_loadouts(
        payload,
        manifest,
        shadow_map=load_json(args.shadow_map),
        approval_policy=load_json(args.approval_policy),
        health_manifest=load_json(args.health_manifest),
    )

    if args.apply:
        write_loadouts(args.output, payload)
        print(f"Wrote {args.output}")
    else:
        print(json.dumps(payload, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
