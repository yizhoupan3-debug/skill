#!/usr/bin/env python3
"""Audit routing coverage: trigger_hints completeness check.

Reads `skills/SKILL_ROUTING_RUNTIME.json` (positional array format) and
checks each skill for:
  - Minimum trigger_hints count (>= 5 recommended)
  - Bilingual coverage (both CN and EN trigger_hints present)
  - Skills with only `$slug` as entry point (no NL route)
  - Skills on disk with SKILL.md but NOT in the routing manifest

Skills with empty trigger_hints in their SKILL.md frontmatter, or listed
in NON_ROUTABLE_SKILLS below (framework-internal, not user-invocable),
are excluded from the "missing from manifest" check.

Usage:
  python3 scripts/audit_routing_coverage.py

Exit codes:
  0 — all checks pass (or only warnings)
  1 — at least one error found
"""

import json
import os
import re
import sys

PROJECT_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MANIFEST_PATH = os.path.join(PROJECT_ROOT, "skills", "SKILL_ROUTING_RUNTIME.json")
SKILLS_DIR = os.path.join(PROJECT_ROOT, "skills")

# Field indices in the positional array
IDX_SLUG = 0
IDX_LAYER = 1
IDX_HINTS = 7

# CJK character range check
CJK_RE = re.compile(r"[一-鿿㐀-䶿豈-﫿]")

# Framework-internal skills NOT expected in the routing manifest.
# Each entry must have a brief justification.
NON_ROUTABLE_SKILLS = {
    "observer-rs",        # Framework-operational: audit/health/telemetry
    "primary-runtime",    # Framework lifecycle orchestration, not user-facing
    "shared-references",  # Lookup library consumed by other skills
}


def has_cjk(text: str) -> bool:
    return bool(CJK_RE.search(text))


def main():
    if not os.path.exists(MANIFEST_PATH):
        print(f"ERROR: manifest not found: {MANIFEST_PATH}")
        sys.exit(1)

    with open(MANIFEST_PATH, encoding="utf-8") as f:
        manifest = json.load(f)

    records = manifest.get("skills", [])
    if not records:
        print("ERROR: no skills in manifest")
        sys.exit(1)

    # Build disk skill set (exclude non-routable)
    disk_skills = set()
    if os.path.isdir(SKILLS_DIR):
        for entry in os.listdir(SKILLS_DIR):
            skill_dir = os.path.join(SKILLS_DIR, entry)
            if os.path.isdir(skill_dir) and os.path.isfile(
                os.path.join(skill_dir, "SKILL.md")
            ) and entry not in NON_ROUTABLE_SKILLS:
                disk_skills.add(entry)

    errors = []
    warnings = []
    manifest_slugs = set()

    for rec in records:
        slug = rec[IDX_SLUG]
        layer = rec[IDX_LAYER] if len(rec) > IDX_LAYER else "?"
        hints = rec[IDX_HINTS] if len(rec) > IDX_HINTS else []
        if hints is None:
            hints = []
        manifest_slugs.add(slug)

        # Check minimum hints
        if len(hints) < 5:
            warnings.append(
                f"[WARN] {slug} (layer={layer}): only {len(hints)} trigger_hints "
                f"(recommended >= 5)"
            )

        # Check for only-slug entry (no NL route)
        if len(hints) <= 1 and (slug in hints or f"/{slug}" in hints):
            errors.append(
                f"[ERROR] {slug} (layer={layer}): only has slug-based trigger(s), "
                f"no NL entry point: {hints}"
            )

        # Check bilingual coverage
        has_cn = any(has_cjk(h) for h in hints)
        has_en = any(not has_cjk(h) for h in hints)
        if not has_cn and len(hints) > 0:
            warnings.append(
                f"[WARN] {slug} (layer={layer}): no Chinese trigger_hints"
            )
        if not has_en and len(hints) > 0:
            warnings.append(
                f"[WARN] {slug} (layer={layer}): no English trigger_hints"
            )

    # Skills on disk but NOT in manifest (excluding non-routable)
    for slug in sorted(disk_skills):
        if slug not in manifest_slugs:
            errors.append(
                f"[ERROR] {slug}: has skills/{slug}/SKILL.md but is NOT in "
                f"SKILL_ROUTING_RUNTIME.json"
            )

    print(f"=== Routing Coverage Audit ===")
    print(f"Manifest skills: {len(records)}")
    print(f"Routable skill dirs on disk: {len(disk_skills)}")
    print()

    for item in errors:
        print(item)
    for item in warnings:
        print(item)

    print()
    if errors:
        print(f"FAIL: {len(errors)} error(s), {len(warnings)} warning(s)")
        sys.exit(1)
    else:
        print(f"PASS: 0 errors, {len(warnings)} warning(s)")
        sys.exit(0)


if __name__ == "__main__":
    main()
