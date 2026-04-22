#!/usr/bin/env python3
"""Sync generated skill routing artifacts and related repo hooks."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).parent))

try:
    import check_skills
except ImportError:
    check_skills = None

from build_skill_approval_policy import build_policy
from build_skill_loadouts import DEFAULT_LOADOUTS, validate_loadouts
from build_skill_shadow_map import (
    build_shadow_map,
    collect_skill_entries,
    load_source_manifest,
)
from build_skill_tiers import build_skill_tiers
from materialize_cli_host_entrypoints import sync_repo_host_entrypoints


def get_git_root() -> Path:
    """Return the repository root when available."""
    local_root = Path(__file__).resolve().parents[1]
    if (local_root / "skills").is_dir():
        return local_root

    try:
        proc = subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            capture_output=True,
            text=True,
            check=True,
        )
        return Path(proc.stdout.strip())
    except Exception:
        return local_root


ROOT = get_git_root()
SKILLS_ROOT = ROOT / "skills"
SKILL_COMPILER_RS_DIR = ROOT / "scripts" / "skill-compiler-rs"
SKILL_COMPILER_RS_RELEASE_BIN = SKILL_COMPILER_RS_DIR / "target" / "release" / "skill-compiler-rs"
SKILL_COMPILER_RS_DEBUG_BIN = SKILL_COMPILER_RS_DIR / "target" / "debug" / "skill-compiler-rs"
REGISTRY_PATH = SKILLS_ROOT / "SKILL_ROUTING_REGISTRY.md"
INDEX_PATH = SKILLS_ROOT / "SKILL_ROUTING_INDEX.md"
RUNTIME_INDEX_PATH = SKILLS_ROOT / "SKILL_ROUTING_RUNTIME.json"
MANIFEST_PATH = SKILLS_ROOT / "SKILL_MANIFEST.json"
HEALTH_MANIFEST_PATH = SKILLS_ROOT / "SKILL_HEALTH_MANIFEST.json"
SOURCE_MANIFEST_PATH = SKILLS_ROOT / "SKILL_SOURCE_MANIFEST.json"
SHADOW_MAP_PATH = SKILLS_ROOT / "SKILL_SHADOW_MAP.json"
LOADOUTS_PATH = SKILLS_ROOT / "SKILL_LOADOUTS.json"
APPROVAL_POLICY_PATH = SKILLS_ROOT / "SKILL_APPROVAL_POLICY.json"
TIERS_PATH = SKILLS_ROOT / "SKILL_TIERS.json"
HOOKS_PATH = ROOT / ".githooks"
REQUIRED_ROUTING_FIELDS = ("routing_layer", "routing_owner", "routing_gate", "session_start")

INDEX_CHECKLIST = [
    "Extract object / action / constraints / deliverable first.",
    "Check source gates before owners when the task starts from external evidence or official docs.",
    "Check artifact gates when the primary object is a PDF, DOCX, XLSX, or similar file artifact.",
    "Check evidence gates when screenshots, rendered pages, browser interaction, or root-cause debugging are central.",
    "Check delegation gate before owner selection when the task is complex and parallel sidecars would help.",
    "Only then choose the narrowest owner and add at most one overlay.",
]


def run_git(*args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    """Run a git command from the repository root."""
    return subprocess.run(
        ["git", *args],
        cwd=ROOT,
        check=check,
        text=True,
        capture_output=True,
    )


def repo_relative(path: Path) -> str:
    """Return a repository-relative display path when possible."""
    try:
        return path.resolve().relative_to(ROOT.resolve()).as_posix()
    except ValueError:
        return str(path)


def resolve_skill_compiler_binary() -> Path | None:
    """Return the compiled Rust skill compiler when available."""

    for candidate in (SKILL_COMPILER_RS_RELEASE_BIN, SKILL_COMPILER_RS_DEBUG_BIN):
        if candidate.is_file():
            return candidate
    return None


def load_compiled_skill_artifacts() -> dict[str, Any] | None:
    """Load generated skill artifacts from the Rust compiler when available."""

    binary = resolve_skill_compiler_binary()
    if binary is None:
        return None

    try:
        proc = subprocess.run(
            [
                str(binary),
                "--skills-root",
                str(SKILLS_ROOT),
                "--source-manifest",
                str(SOURCE_MANIFEST_PATH),
                "--health-manifest",
                str(HEALTH_MANIFEST_PATH),
                "--json",
            ],
            capture_output=True,
            text=True,
            check=True,
        )
    except Exception:
        return None

    try:
        payload = json.loads(proc.stdout)
    except json.JSONDecodeError:
        return None

    required = {"registry", "index", "manifest", "runtime_index", "shadow_map", "approval_policy"}
    if not isinstance(payload, dict) or not required.issubset(payload):
        return None

    manifest = payload.get("manifest")
    runtime_index = payload.get("runtime_index")
    shadow_map = payload.get("shadow_map")
    manifest_keys = manifest.get("keys") if isinstance(manifest, dict) else None
    runtime_keys = runtime_index.get("keys") if isinstance(runtime_index, dict) else None
    winning_rule = shadow_map.get("winning_rule") if isinstance(shadow_map, dict) else None
    if (
        not isinstance(manifest_keys, list)
        or "trigger_hints" not in manifest_keys
        or "source_position" not in manifest_keys
        or not isinstance(runtime_keys, list)
        or "trigger_hints" not in runtime_keys
        or winning_rule != "highest-position-wins"
    ):
        return None
    return payload


def load_health_payload() -> dict[str, Any]:
    """Load the raw health manifest payload when present.

    Returns:
        dict[str, Any]: Raw health manifest payload or an empty mapping.
    """

    if not HEALTH_MANIFEST_PATH.exists():
        return {}

    try:
        raw = json.loads(HEALTH_MANIFEST_PATH.read_text(encoding="utf-8"))
    except Exception:
        return {}
    return raw if isinstance(raw, dict) else {}


def load_health_data(raw: dict[str, Any] | None = None) -> dict[str, Any]:
    """Load health metadata keyed by skill slug.

    Returns:
        dict[str, Any]: Skill-health mapping keyed by slug.
    """

    raw = raw if isinstance(raw, dict) else load_health_payload()
    skills = raw.get("skills", {})
    if isinstance(skills, dict):
        return skills
    if isinstance(skills, list):
        return {
            entry.get("name", ""): entry
            for entry in skills
            if isinstance(entry, dict) and entry.get("name")
        }
    return {}


def extract_trigger_hints(
    frontmatter: dict[str, Any],
    description: str,
    body: str,
    limit: int = 16,
) -> list[str]:
    """Extract compact multilingual trigger phrases for routing manifests.

    Parameters:
        frontmatter: Parsed skill frontmatter.
        description: Description text from the frontmatter.
        body: Skill body markdown.
        limit: Maximum number of phrases to keep.

    Returns:
        list[str]: Ordered unique trigger phrases.
    """

    trigger_hints = check_skills.collect_trigger_hints(frontmatter)
    phrases: list[str] = []
    seen: set[str] = set()

    def push(phrase: str) -> None:
        cleaned = re.sub(r"\s+", " ", phrase).strip(" -–—•,:;()[]{}'\"`“”‘’/")
        if len(cleaned) < 2:
            return
        key = cleaned.lower()
        if key in seen:
            return
        seen.add(key)
        phrases.append(cleaned)

    for item in trigger_hints:
        push(str(item))

    # Explicit frontmatter trigger hints are canonical. Do not auto-enrich them
    # from the skill body, or runtime artifacts will accumulate broad fragments
    # like "write", "路由", or half-sentences that distort routing.
    if phrases:
        return phrases[:limit]

    # For skills without explicit trigger_hints, keep fallback extraction scoped
    # to the frontmatter description. Mining arbitrary body lines produces broad
    # fragments like "这个" or "为什么" that destabilize routing.
    source = description

    for match in re.findall(r'[\"“](.+?)[\"”]', source):
        push(match)

    return phrases[:limit]


def join_trigger_hints(frontmatter: dict[str, Any], description: str, body: str) -> str:
    """Render trigger hints for human-readable registry output.

    Parameters:
        frontmatter: Parsed skill frontmatter.
        description: Description text from the frontmatter.
        body: Skill body markdown.

    Returns:
        str: Slash-delimited trigger hint string.
    """

    return " / ".join(extract_trigger_hints(frontmatter, description, body))


def normalize_health_manifest(
    manifest: dict[str, Any],
    raw_health_payload: dict[str, Any] | None = None,
    health_data: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Backfill the health manifest so every routed skill has a health row.

    Parameters:
        manifest: Generated compact skill manifest.

    Returns:
        dict[str, Any]: Normalized health manifest payload.
    """

    payload = raw_health_payload if isinstance(raw_health_payload, dict) else load_health_payload()
    current = health_data if isinstance(health_data, dict) else load_health_data(payload)
    normalized: dict[str, Any] = {}
    scores: list[float] = []
    critical: list[str] = []

    for row in manifest.get("skills", []):
        slug = row[0]
        info = current.get(slug, {}) if isinstance(current, dict) else {}
        dynamic_score = float(info.get("dynamic_score", 100.0))
        static_score = float(info.get("static_score", dynamic_score))
        usage = int(info.get("usage_30d", 0))
        reroutes = int(info.get("reroutes_30d", 0))
        if dynamic_score >= 85.0:
            status = "Healthy"
        elif dynamic_score >= 60.0:
            status = "Stable"
        else:
            status = "Critical"
            critical.append(slug)
        normalized[slug] = {
            "dynamic_score": round(dynamic_score, 1),
            "static_score": round(static_score, 1),
            "usage_30d": usage,
            "reroutes_30d": reroutes,
            "health_status": status,
        }
        scores.append(dynamic_score)

    avg_health = round(sum(scores) / len(scores), 1) if scores else 0.0
    return {
        # Preserve the last recorded timestamp so verify-sync remains deterministic.
        "ts": payload.get("ts") if isinstance(payload.get("ts"), str) else datetime.now(timezone.utc).isoformat(),
        "summary": {
            "total_skills": len(normalized),
            "critical_skills": len(critical),
            "avg_health": avg_health,
        },
        "skills": normalized,
        "critical_outliers": critical,
        "source": payload.get("source", "sync_skills_backfill"),
    }

def summarize_text(text: str, limit: int = 80) -> str:
    """Collapse multi-line text for compact routing artifacts."""
    one_line = " ".join(part.strip() for part in text.splitlines() if part.strip())
    return one_line[:limit]


def pick_runtime_summary(frontmatter: dict[str, Any], limit: int = 80) -> str:
    """Prefer short_description to keep runtime routing artifacts lean."""
    short_description = str(frontmatter.get("short_description", "")).strip()
    if short_description:
        return summarize_text(short_description, limit=limit)
    return summarize_text(str(frontmatter.get("description", "")), limit=limit)


def select_manifest_skill_documents(
    skill_documents: list[check_skills.SkillDocument],
    skill_entries: list[dict[str, Any]],
) -> list[check_skills.SkillDocument]:
    """Keep only the highest-precedence document for each slug in MANIFEST/REGISTRY."""

    winner_paths: dict[str, str] = {}
    for entry in sorted(
        skill_entries,
        key=lambda item: (item.get("source_position", -1), str(item.get("path", ""))),
    ):
        winner_paths[str(entry["slug"])] = str(entry["path"])

    selected: list[check_skills.SkillDocument] = []
    seen: set[str] = set()
    for document in skill_documents:
        slug = document.slug
        if slug in seen:
            continue
        if winner_paths.get(slug) != repo_relative(document.skill_dir):
            continue
        selected.append(document)
        seen.add(slug)
    return selected


def build_registry_and_manifest(
    skill_documents: list[check_skills.SkillDocument] | None = None,
    skill_entries: list[dict[str, Any]] | None = None,
    health_data: dict[str, Any] | None = None,
) -> tuple[str, dict[str, Any]]:
    """Build registry markdown and compact JSON manifest from skill files."""
    if not check_skills:
        return "", {"keys": [], "skills": []}

    health_data = health_data if isinstance(health_data, dict) else load_health_data()
    rows: list[str] = []
    skills_data: list[list[Any]] = []
    skill_documents = skill_documents or check_skills.load_skill_documents(SKILLS_ROOT, include_system=True)
    if skill_entries is None:
        source_manifest = load_source_manifest(SOURCE_MANIFEST_PATH)
        skill_entries = collect_skill_entries(
            SKILLS_ROOT,
            source_manifest=source_manifest,
            skill_documents=skill_documents,
        )
    source_entries = {entry["slug"]: entry for entry in skill_entries}
    selected_documents = select_manifest_skill_documents(skill_documents, skill_entries)
    keys = [
        "slug",
        "layer",
        "owner",
        "gate",
        "priority",
        "description",
        "session_start",
        "trigger_hints",
        "health",
        "source",
        "source_position",
    ]

    for document in selected_documents:
        slug = document.slug
        skill_dir = document.skill_dir
        skill_file = document.skill_file
        fm = document.metadata
        body = document.body

        missing = [field for field in REQUIRED_ROUTING_FIELDS if not str(fm.get(field, "")).strip()]
        if missing:
            missing_fields = ", ".join(missing)
            raise ValueError(f"{repo_relative(skill_file)} missing required routing fields: {missing_fields}")

        status = fm.get("status", "Active")
        priority = fm.get("routing_priority") or fm.get("priority", "P2")
        layer = str(fm["routing_layer"]).strip()
        owner = str(fm["routing_owner"]).strip()
        gate = str(fm["routing_gate"]).strip()
        session_start = str(fm["session_start"]).strip()
        description = str(fm.get("description", "")).strip()
        trigger_hints = extract_trigger_hints(fm, description, body)
        summary = pick_runtime_summary(fm)
        long_summary = pick_runtime_summary(fm, limit=200)
        source_entry = source_entries.get(slug, {})
        source = str(source_entry.get("source", "project"))
        source_position = source_entry.get("source_position")

        health_info = health_data.get(slug, {}) if isinstance(health_data, dict) else {}
        try:
            health_score = float(health_info.get("dynamic_score", 100.0))
        except (TypeError, ValueError):
            health_score = 100.0
        indicator = "✓" if health_score >= 85 else ("⚠" if health_score >= 60 else "❌")

        rows.append(
            f"| `{slug}` | {status} | {priority} | {layer} | {owner} | {gate} | {source} | {indicator} {health_score:.1f} | {summary} |"
        )
        skills_data.append([
            slug,
            layer,
            owner,
            gate,
            priority,
            long_summary,
            session_start,
            trigger_hints,
            round(health_score, 1),
            source,
            source_position,
        ])

    registry = (
        "# Skill Routing Registry\n\n"
        "| Skill | Status | P | Layer | Owner | Gate | Source | Health | Description |\n"
        "|---|---|---|---|---|---|---|---|---|\n"
        + "\n".join(rows)
        + "\n"
    )
    return registry, {"keys": keys, "skills": skills_data}


def build_index(manifest: dict[str, Any]) -> str:
    """Build the lightweight routing index used at conversation start."""
    selected = select_runtime_skills(manifest)

    lines = [
        "# Skill Routing Index",
        "",
        "> Entry point for rapid lookup.",
        "> Prefer `skills/SKILL_ROUTING_RUNTIME.json` for the lean machine-readable route map.",
        "> Prefer `skills/SKILL_MANIFEST.json` for the full manifest (includes owner, priority, source, etc.).",
        "> RUNTIME (v2) is a compact 8-key subset: slug, layer, owner, gate, session_start, summary, trigger_hints, health.",
        "> MANIFEST is the full 11-key record: slug, layer, owner, gate, priority, description, session_start, trigger_hints, health, source, source_position.",
        "",
        "## 6-rule gate checklist",
    ]
    for idx, item in enumerate(INDEX_CHECKLIST, start=1):
        lines.append(f"{idx}. {item}")

    lines.extend([
        "",
        "## Gates & Meta",
        "| Name | Layer | Owner | Gate | Description |",
        "|---|---|---|---|---|",
    ])

    for skill in selected:
        lines.append(f"| `{skill[0]}` | {skill[1]} | {skill[2]} | {skill[3]} | {skill[5][:80]} |")

    lines.extend([
        "",
        "See `skills/SKILL_ROUTING_LAYERS.md` for the full owner map and reroute rules.",
        "",
    ])
    return "\n".join(lines)


def select_runtime_skills(manifest: dict[str, Any]) -> list[list[Any]]:
    """Select and order all skills for full-coverage routing.

    All skills are included so that INDEX and RUNTIME provide complete
    coverage across L0-L4. Ordering: required-gates first, then preferred,
    then by layer (L0→L4), then alphabetically within each layer.
    """
    skills = manifest.get("skills", [])
    selected: list[list[Any]] = []
    seen: set[str] = set()
    layer_order = {"L0": 0, "L1": 1, "L2": 2, "L3": 3, "L4": 4}

    for skill in skills:
        if not isinstance(skill, list) or len(skill) < 6:
            continue
        slug = skill[0]
        if slug in seen:
            continue
        selected.append(skill)
        seen.add(slug)

    selected.sort(
        key=lambda skill: (
            0 if skill[6] == "required" else 1 if skill[6] == "preferred" else 2,
            0 if skill[3] != "none" else 1,
            layer_order.get(skill[1], 99),
            skill[0],
        )
    )
    return selected


def build_runtime_index(manifest: dict[str, Any]) -> dict[str, Any]:
    """Build a compact machine-readable index for Codex-first routing."""
    selected = select_runtime_skills(manifest)
    return {
        "version": 2,
        "checklist": INDEX_CHECKLIST,
        "keys": ["slug", "layer", "owner", "gate", "session_start", "summary", "trigger_hints", "health"],
        "skills": [
            [skill[0], skill[1], skill[2], skill[3], skill[6], summarize_text(str(skill[5]), limit=96), skill[7], skill[8]]
            for skill in selected
        ],
    }


def write_generated_files(
    apply: bool = False,
    skill_documents: list[check_skills.SkillDocument] | None = None,
    skill_entries: list[dict[str, Any]] | None = None,
) -> list[str]:
    """Return generated file drift, optionally writing updates."""
    compiled = load_compiled_skill_artifacts()
    raw_health_payload = load_health_payload()
    health_data = load_health_data(raw_health_payload)
    source_manifest = load_source_manifest(SOURCE_MANIFEST_PATH)
    if compiled is not None:
        registry = str(compiled["registry"])
        manifest = compiled["manifest"]
        index = str(compiled["index"])
        runtime_index = compiled["runtime_index"]
        shadow_map = compiled["shadow_map"]
        approval_policy = compiled["approval_policy"]
    else:
        skill_documents = (
            skill_documents
            if skill_documents is not None
            else (check_skills.load_skill_documents(SKILLS_ROOT, include_system=True) if check_skills else [])
        )
        skill_entries = (
            skill_entries
            if skill_entries is not None
            else (
                collect_skill_entries(
                    SKILLS_ROOT,
                    source_manifest=source_manifest,
                    skill_documents=skill_documents,
                ) if check_skills else []
            )
        )
        registry, manifest = build_registry_and_manifest(
            skill_documents=skill_documents,
            skill_entries=skill_entries,
            health_data=health_data,
        ) if check_skills else ("", {"keys": [], "skills": []})
        index = build_index(manifest)
        runtime_index = build_runtime_index(manifest)
        shadow_map = build_shadow_map(
            SKILLS_ROOT,
            source_manifest=source_manifest,
            skill_documents=skill_documents,
            skill_entries=skill_entries,
        )
        approval_policy = build_policy(SKILLS_ROOT, skill_documents=skill_documents)
    health_manifest = normalize_health_manifest(
        manifest,
        raw_health_payload=raw_health_payload,
        health_data=health_data,
    )
    validate_loadouts(
        DEFAULT_LOADOUTS,
        manifest,
        shadow_map=shadow_map,
        approval_policy=approval_policy,
        health_manifest=health_manifest,
    )
    tiers = build_skill_tiers(
        manifest,
        health_manifest=health_manifest,
        loadouts=DEFAULT_LOADOUTS,
    )
    targets = {
        REGISTRY_PATH: registry,
        MANIFEST_PATH: json.dumps(manifest, ensure_ascii=False, separators=(",", ":")),
        INDEX_PATH: index,
        RUNTIME_INDEX_PATH: json.dumps(runtime_index, ensure_ascii=False, separators=(",", ":")),
        SOURCE_MANIFEST_PATH: json.dumps(source_manifest, ensure_ascii=False, indent=2) + "\n",
        SHADOW_MAP_PATH: json.dumps(shadow_map, ensure_ascii=False, indent=2) + "\n",
        LOADOUTS_PATH: json.dumps(DEFAULT_LOADOUTS, ensure_ascii=False, indent=2) + "\n",
        APPROVAL_POLICY_PATH: json.dumps(approval_policy, ensure_ascii=False, indent=2) + "\n",
        HEALTH_MANIFEST_PATH: json.dumps(health_manifest, ensure_ascii=False, indent=2) + "\n",
        TIERS_PATH: json.dumps(tiers, ensure_ascii=False, indent=2) + "\n",
    }

    changed: list[str] = []
    for path, content in targets.items():
        current = path.read_text(encoding="utf-8") if path.exists() else None
        if current != content:
            changed.append(repo_relative(path))
            if apply:
                path.write_text(content, encoding="utf-8")

    host_entrypoints = sync_repo_host_entrypoints(ROOT, apply=apply)
    changed.extend(host_entrypoints["written"])
    return changed


def install_hooks() -> None:
    """Install repo-local git hooks."""
    if not HOOKS_PATH.is_dir():
        raise SystemExit(f"missing hooks directory: {HOOKS_PATH}")
    run_git("config", "core.hooksPath", repo_relative(HOOKS_PATH))
    print(f"Installed git hooks: core.hooksPath={repo_relative(HOOKS_PATH)}")


def stage_generated_files(paths: list[str]) -> None:
    """Stage generated files if they changed."""
    if not paths:
        return
    run_git("add", *paths)
    print(f"Staged generated files: {', '.join(paths)}")


def post_commit_push() -> int:
    """Push the current branch after commit when hooks request it."""
    result = run_git("push", check=False)
    if result.returncode == 0:
        print(result.stdout.strip() or "git push succeeded")
        return 0
    print(result.stderr.strip() or result.stdout.strip() or "git push failed", file=sys.stderr)
    return result.returncode


def main() -> int:
    """CLI entry point."""
    parser = argparse.ArgumentParser(description="Sync Codex skill generated files.")
    parser.add_argument("--apply", action="store_true", help="Write generated files to disk.")
    parser.add_argument("--staged", action="store_true", help="Run in pre-commit style mode.")
    parser.add_argument("--stage-generated", action="store_true", help="Stage generated files after sync.")
    parser.add_argument("--install-hooks", action="store_true", help="Set git core.hooksPath to .githooks.")
    parser.add_argument("--post-commit-push", action="store_true", help="Push the current branch for post-commit hook automation.")
    args = parser.parse_args()

    if args.install_hooks:
        install_hooks()
        return 0

    if args.post_commit_push:
        return post_commit_push()

    changed = write_generated_files(apply=args.apply or args.staged)
    if changed:
        print(f"Generated files {'updated' if (args.apply or args.staged) else 'out of date'}: {', '.join(changed)}")
    else:
        print("Generated files are in sync.")

    if args.stage_generated:
        stage_generated_files(changed)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
