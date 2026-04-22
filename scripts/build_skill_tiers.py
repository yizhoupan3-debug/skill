#!/usr/bin/env python3
"""Build a machine-readable skill tier catalog from existing registry signals."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
SKILLS_ROOT = ROOT / "skills"
MANIFEST_PATH = SKILLS_ROOT / "SKILL_MANIFEST.json"
HEALTH_MANIFEST_PATH = SKILLS_ROOT / "SKILL_HEALTH_MANIFEST.json"
LOADOUTS_PATH = SKILLS_ROOT / "SKILL_LOADOUTS.json"
TIERS_PATH = SKILLS_ROOT / "SKILL_TIERS.json"

KNOWN_TIERS = ("core", "optional", "experimental", "deprecated")
CORE_LOADOUTS = {"framework_loadout"}
CORE_OWNER_ROLES = {"gate", "@app-controller", "@kernel-controller", "@strategic-orchestrator"}
ACTIVATION_MODES = ("default", "explicit_opt_in", "disabled")
DEPRECATED_DYNAMIC_SCORE_MAX = 60.0
EXPERIMENTAL_DYNAMIC_SCORE_MAX = 85.0
STABLE_HEALTH_STATUSES = {"Healthy"}


def load_json(path: Path) -> dict[str, Any]:
    """Load a JSON file from disk."""

    return json.loads(path.read_text(encoding="utf-8"))


def _manifest_key_index(manifest: dict[str, Any]) -> dict[str, int]:
    """Return compressed manifest key indexes."""

    return {
        str(key): idx
        for idx, key in enumerate(manifest.get("keys", []))
        if isinstance(key, str)
    }


def normalize_manifest(manifest: dict[str, Any]) -> dict[str, dict[str, Any]]:
    """Expand the compressed skill manifest into slug-keyed dictionaries."""

    key_index = _manifest_key_index(manifest)
    skills: dict[str, dict[str, Any]] = {}
    for row in manifest.get("skills", []):
        if not isinstance(row, list):
            continue
        entry = {
            key: row[idx]
            for key, idx in key_index.items()
            if idx < len(row)
        }
        slug = entry.get("slug")
        if isinstance(slug, str) and slug:
            skills[slug] = entry
    return skills


def normalize_health(health_manifest: dict[str, Any]) -> dict[str, dict[str, Any]]:
    """Return health metadata keyed by skill slug."""

    raw = health_manifest.get("skills", {})
    return raw if isinstance(raw, dict) else {}


def collect_loadout_memberships(loadouts: dict[str, Any]) -> dict[str, list[dict[str, str]]]:
    """Map each skill slug to the loadouts and buckets it participates in."""

    memberships: dict[str, list[dict[str, str]]] = {}
    for loadout_name, config in loadouts.get("loadouts", {}).items():
        if not isinstance(config, dict):
            continue
        for bucket in ("owners", "overlays", "exclude"):
            for slug in config.get(bucket, []):
                memberships.setdefault(str(slug), []).append(
                    {"loadout": str(loadout_name), "bucket": bucket}
                )
    return memberships


def collect_loadout_activation(loadouts: dict[str, Any]) -> tuple[list[str], list[str]]:
    """Return default and explicit-opt-in loadout names."""

    activation_policy = loadouts.get("activation_policy", {})
    configured_default = [
        str(name)
        for name in activation_policy.get("default_loadouts", [])
        if isinstance(name, str) and name
    ]
    loadout_configs = loadouts.get("loadouts", {})
    if configured_default:
        default_loadouts = configured_default
    else:
        default_loadouts = sorted(
            str(name)
            for name, config in loadout_configs.items()
            if isinstance(config, dict) and str(config.get("activation", "explicit")) == "default"
        )
    explicit_loadouts = sorted(
        str(name)
        for name, config in loadout_configs.items()
        if isinstance(config, dict) and str(name) not in set(default_loadouts)
    )
    return sorted(default_loadouts), explicit_loadouts


def _looks_deprecated(
    manifest_entry: dict[str, Any],
    health_entry: dict[str, Any] | None,
) -> bool:
    description = str(manifest_entry.get("description", ""))
    trigger_hints = manifest_entry.get("trigger_hints", [])
    health_status = str((health_entry or {}).get("health_status", ""))
    lowered = " ".join(
        [
            description,
            *(str(item) for item in trigger_hints if isinstance(item, str)),
            health_status,
        ]
    ).lower()
    return any(token in lowered for token in ("deprecated", "sunset", "retired"))


def tier_skill(
    manifest_entry: dict[str, Any],
    health_entry: dict[str, Any] | None,
    memberships: list[dict[str, str]],
) -> tuple[str, list[str]]:
    """Classify a single skill into a tier and explain why."""

    owner = str(manifest_entry.get("owner", ""))
    gate = str(manifest_entry.get("gate", ""))
    layer = str(manifest_entry.get("layer", ""))
    priority = str(manifest_entry.get("priority", ""))
    session_start = str(manifest_entry.get("session_start", ""))
    dynamic_score = float((health_entry or {}).get("dynamic_score", manifest_entry.get("health", 0.0)))
    health_status = str((health_entry or {}).get("health_status", "Unknown"))
    usage_30d = int((health_entry or {}).get("usage_30d", 0) or 0)
    reroutes_30d = int((health_entry or {}).get("reroutes_30d", 0) or 0)
    active_loadout_names = sorted(
        {row["loadout"] for row in memberships if row.get("bucket") != "exclude"}
    )

    reasons: list[str] = []

    deprecated_candidate = _looks_deprecated(manifest_entry, health_entry) or (
        dynamic_score <= DEPRECATED_DYNAMIC_SCORE_MAX
        and usage_30d == 0
        and reroutes_30d > 0
    )
    experimental_candidate = (
        health_status not in STABLE_HEALTH_STATUSES
        or dynamic_score < EXPERIMENTAL_DYNAMIC_SCORE_MAX
        or reroutes_30d >= 5
    )
    core_candidate = (
        owner in CORE_OWNER_ROLES
        or session_start == "required"
        or priority == "P0"
        or bool(CORE_LOADOUTS & set(active_loadout_names))
    )

    if owner in CORE_OWNER_ROLES:
        reasons.append(f"owner:{owner}")
    if gate != "none":
        reasons.append(f"gate:{gate}")
    if session_start == "required":
        reasons.append("session_start:required")
    if priority == "P0":
        reasons.append("priority:P0")
    if CORE_LOADOUTS & set(active_loadout_names):
        reasons.append("loadout:framework")
    if health_status not in STABLE_HEALTH_STATUSES:
        reasons.append(f"health_status:{health_status}")
    if dynamic_score < EXPERIMENTAL_DYNAMIC_SCORE_MAX:
        reasons.append(f"dynamic_score:{dynamic_score:.1f}")
    if reroutes_30d > 0:
        reasons.append(f"reroutes_30d:{reroutes_30d}")
    if usage_30d > 0:
        reasons.append(f"usage_30d:{usage_30d}")
    if layer in {"L0", "L-1"}:
        reasons.append(f"layer:{layer}")

    if deprecated_candidate:
        return "deprecated", reasons
    if experimental_candidate:
        return "experimental", reasons
    if core_candidate:
        return "core", reasons
    return "optional", reasons or ["specialist-opt-in"]


def build_skill_tiers(
    manifest: dict[str, Any],
    health_manifest: dict[str, Any] | None = None,
    loadouts: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Build the tier catalog payload."""

    manifest_skills = normalize_manifest(manifest)
    health_skills = normalize_health(health_manifest or {})
    memberships = collect_loadout_memberships(loadouts or {})
    default_loadouts, explicit_loadouts = collect_loadout_activation(loadouts or {})

    groups = {tier: [] for tier in KNOWN_TIERS}
    activation_counts = {mode: 0 for mode in ACTIVATION_MODES}
    skills: dict[str, Any] = {}
    for slug in sorted(manifest_skills):
        manifest_entry = manifest_skills[slug]
        health_entry = health_skills.get(slug, {})
        loadout_memberships = memberships.get(slug, [])
        tier, reasons = tier_skill(manifest_entry, health_entry, loadout_memberships)
        default_loadout_memberships = [
            row
            for row in loadout_memberships
            if row.get("bucket") != "exclude" and row.get("loadout") in default_loadouts
        ]
        default_surface_enabled = tier == "core" or bool(default_loadout_memberships)
        if tier == "deprecated":
            activation_mode = "disabled"
        elif tier == "experimental":
            activation_mode = "explicit_opt_in"
        elif default_surface_enabled:
            activation_mode = "default"
        else:
            activation_mode = "explicit_opt_in"
        groups[tier].append(slug)
        activation_counts[activation_mode] += 1
        skills[slug] = {
            "tier": tier,
            "reasons": reasons,
            "surface": {
                "activation_mode": activation_mode,
                "default_surface_enabled": default_surface_enabled,
                "default_loadout_memberships": default_loadout_memberships,
            },
            "signals": {
                "layer": manifest_entry.get("layer"),
                "owner": manifest_entry.get("owner"),
                "gate": manifest_entry.get("gate"),
                "priority": manifest_entry.get("priority"),
                "session_start": manifest_entry.get("session_start"),
                "source": manifest_entry.get("source"),
                "source_position": manifest_entry.get("source_position"),
                "health": {
                    "dynamic_score": health_entry.get("dynamic_score", manifest_entry.get("health")),
                    "static_score": health_entry.get("static_score"),
                    "usage_30d": health_entry.get("usage_30d", 0),
                    "reroutes_30d": health_entry.get("reroutes_30d", 0),
                    "health_status": health_entry.get("health_status"),
                },
                "loadouts": loadout_memberships,
            },
        }

    return {
        "version": 1,
        "schema_version": "skill-tier-catalog-v1",
        "tier_order": list(KNOWN_TIERS),
        "generation_policy": {
            "core": "gate/controller/framework skills with healthy signals",
            "optional": "healthy non-core skills",
            "experimental": "skills with unstable or low-health routing signals",
            "deprecated": "very-low-health and unused skills with reroute pressure",
        },
        "surface_policy": {
            "default_loadouts": default_loadouts,
            "explicit_opt_in_loadouts": explicit_loadouts,
            "tier_activation_defaults": {
                "core": "default",
                "optional": "explicit_opt_in",
                "experimental": "explicit_opt_in",
                "deprecated": "disabled",
            },
        },
        "summary": {
            "total_skills": len(skills),
            "tier_counts": {tier: len(groups[tier]) for tier in KNOWN_TIERS},
            "activation_counts": activation_counts,
        },
        "tiers": groups,
        "skills": skills,
    }


def validate_skill_tiers(payload: dict[str, Any], manifest: dict[str, Any]) -> None:
    """Validate a candidate tier payload against the manifest."""

    manifest_skills = normalize_manifest(manifest)
    known = set(manifest_skills)
    seen: set[str] = set()
    errors: list[str] = []

    groups = payload.get("tiers", {})
    skills = payload.get("skills", {})
    for tier in KNOWN_TIERS:
        slugs = groups.get(tier, [])
        if not isinstance(slugs, list):
            errors.append(f"tiers:{tier}:not-a-list")
            continue
        if len(slugs) != len(set(slugs)):
            errors.append(f"tiers:{tier}:duplicate-slug")
        for slug in slugs:
            if slug not in known:
                errors.append(f"tiers:{tier}:{slug}:unknown-skill")
                continue
            if slug in seen:
                errors.append(f"tiers:{tier}:{slug}:duplicate-coverage")
            seen.add(slug)

    if seen != known:
        missing = sorted(known - seen)
        extra = sorted(seen - known)
        if missing:
            errors.append(f"coverage:missing:{','.join(missing[:10])}")
        if extra:
            errors.append(f"coverage:extra:{','.join(extra[:10])}")

    for slug, entry in skills.items():
        if slug not in known:
            errors.append(f"skills:{slug}:unknown-skill")
            continue
        if not isinstance(entry, dict):
            errors.append(f"skills:{slug}:not-a-dict")
            continue
        tier = entry.get("tier")
        if tier not in KNOWN_TIERS:
            errors.append(f"skills:{slug}:invalid-tier:{tier}")
            continue
        if slug not in groups.get(tier, []):
            errors.append(f"skills:{slug}:tier-mismatch:{tier}")
        surface = entry.get("surface", {})
        activation_mode = surface.get("activation_mode")
        if activation_mode not in ACTIVATION_MODES:
            errors.append(f"skills:{slug}:invalid-activation-mode:{activation_mode}")

    if errors:
        raise SystemExit(f"Invalid skill tiers: {', '.join(errors)}")


def write_skill_tiers(path: Path, payload: dict[str, Any]) -> None:
    """Write the tier catalog using stable formatting."""

    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def main() -> int:
    """CLI entry point."""

    parser = argparse.ArgumentParser(description="Build the machine-readable skill tier catalog.")
    parser.add_argument("--manifest", type=Path, default=MANIFEST_PATH, help="Skill manifest path.")
    parser.add_argument(
        "--health-manifest",
        type=Path,
        default=HEALTH_MANIFEST_PATH,
        help="Skill health manifest path.",
    )
    parser.add_argument("--loadouts", type=Path, default=LOADOUTS_PATH, help="Skill loadouts path.")
    parser.add_argument("--output", type=Path, default=TIERS_PATH, help="Output JSON path.")
    parser.add_argument("--apply", action="store_true", help="Write the tier file to disk.")
    args = parser.parse_args()

    manifest = load_json(args.manifest)
    health_manifest = load_json(args.health_manifest) if args.health_manifest.exists() else {}
    loadouts = load_json(args.loadouts) if args.loadouts.exists() else {}
    payload = build_skill_tiers(manifest, health_manifest, loadouts)
    validate_skill_tiers(payload, manifest)

    if args.apply:
        write_skill_tiers(args.output, payload)
        print(f"Wrote {args.output}")
    else:
        print(json.dumps(payload, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
