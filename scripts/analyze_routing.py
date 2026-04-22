#!/usr/bin/env python3
"""Analyze routing feedback log and generate optimization suggestions.

Parses .routing_feedback.md for routing miss entries, aggregates statistics,
and generates actionable description improvement suggestions.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_LOG = ROOT / "skills" / ".routing_feedback.md"

# Match table rows: | date | expected | actual | reason | suggestion |
TABLE_ROW_RE = re.compile(
    r"^\|\s*([^|]+?)\s*\|\s*`?([^|`]+?)`?\s*\|\s*`?([^|`]+?)`?\s*\|\s*([^|]*?)\s*\|\s*([^|]*?)\s*\|$"
)


def parse_feedback_log(path: Path) -> list[dict]:
    """Parse the routing feedback markdown table into structured records.

    Args:
        path: Path to .routing_feedback.md.

    Returns:
        List of dicts with keys: date, expected, actual, reason, suggestion.
    """
    if not path.is_file():
        return []

    entries = []
    lines = path.read_text(encoding="utf-8").splitlines()
    in_table = False
    for line in lines:
        line = line.strip()
        if line.startswith("| Date"):
            in_table = True
            continue
        if in_table and line.startswith("|---"):
            continue
        if in_table and line.startswith("|"):
            match = TABLE_ROW_RE.match(line)
            if match and match.group(1).strip() != "_example_":
                entries.append({
                    "date": match.group(1).strip(),
                    "expected": match.group(2).strip(),
                    "actual": match.group(3).strip(),
                    "reason": match.group(4).strip(),
                    "suggestion": match.group(5).strip(),
                })
        elif in_table and not line.startswith("|"):
            in_table = False

    return entries


def analyze_misses(entries: list[dict]) -> dict:
    """Aggregate routing miss statistics.

    Args:
        entries: Parsed feedback entries.

    Returns:
        Analysis dict with miss counts, frequent misses, and suggestions.
    """
    if not entries:
        return {
            "total_misses": 0,
            "message": "No routing misses recorded. The routing system is working well!",
        }

    miss_counter = Counter(e["expected"] for e in entries)
    misroute_pairs = Counter((e["expected"], e["actual"]) for e in entries)

    # Skills that are frequently missed
    frequent_misses = [
        {"skill": skill, "miss_count": count}
        for skill, count in miss_counter.most_common(10)
    ]

    # Most common misroute pairs
    frequent_pairs = [
        {"expected": pair[0], "routed_to": pair[1], "count": count}
        for pair, count in misroute_pairs.most_common(10)
    ]

    # Suggestions grouped by skill
    suggestion_map: dict[str, list[str]] = {}
    for entry in entries:
        skill = entry["expected"]
        if entry["suggestion"]:
            suggestion_map.setdefault(skill, []).append(entry["suggestion"])

    # Generate auto-suggestions for skills with multiple misses
    auto_suggestions = []
    for skill, count in miss_counter.most_common():
        if count >= 2:
            skill_md = ROOT / "skills" / skill / "SKILL.md"
            suggestion = {
                "skill": skill,
                "miss_count": count,
                "action": "review_description",
                "detail": f"'{skill}' was missed {count} times. "
                          f"Review its description and trigger index entry.",
            }
            if skill_md.is_file():
                text = skill_md.read_text(encoding="utf-8")
                if "Use when" not in text and "适用于" not in text:
                    suggestion["detail"] += (
                        " SKILL.md description is missing 'Use when' / '适用于' phrasing."
                    )
            if skill in suggestion_map:
                suggestion["user_suggestions"] = list(set(suggestion_map[skill]))
            auto_suggestions.append(suggestion)

    return {
        "total_misses": len(entries),
        "total_unique_skills": len(miss_counter),
        "frequent_misses": frequent_misses,
        "frequent_misroute_pairs": frequent_pairs,
        "auto_suggestions": auto_suggestions,
    }


def load_trigger_index_skills() -> set[str]:
    """Load skill names from the live routing index.

    Returns:
        Set of skill names found in the routing index.
    """
    runtime_file = ROOT / "skills" / "SKILL_ROUTING_RUNTIME.json"
    if runtime_file.is_file():
        payload = json.loads(runtime_file.read_text(encoding="utf-8"))
        rows = payload.get("skills")
        keys = payload.get("keys")
        if isinstance(rows, list) and isinstance(keys, list) and "slug" in keys:
            slug_index = keys.index("slug")
            return {
                str(row[slug_index])
                for row in rows
                if isinstance(row, list) and len(row) > slug_index and str(row[slug_index]).strip()
            }

    index_file = ROOT / "skills" / "SKILL_ROUTING_INDEX.md"
    if not index_file.is_file():
        return set()

    skills = set()
    text = index_file.read_text(encoding="utf-8")
    # Match backtick-quoted slugs in any format (table rows, list items, inline)
    for match in re.finditer(r"`([a-z][a-z0-9]*(?:-[a-z0-9]+)*)`", text):
        skills.add(match.group(1))
    return skills


def check_index_coverage(entries: list[dict]) -> list[str]:
    """Check if missed skills are in the trigger index.

    Args:
        entries: Parsed feedback entries.

    Returns:
        List of missed skills not found in the trigger index.
    """
    indexed = load_trigger_index_skills()
    missed_skills = {e["expected"] for e in entries}
    return sorted(missed_skills - indexed)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Analyze routing feedback and generate optimization suggestions.",
    )
    parser.add_argument(
        "--log",
        type=Path,
        default=DEFAULT_LOG,
        help="Path to the routing feedback log file.",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        dest="json_output",
        help="Output as JSON instead of text.",
    )
    args = parser.parse_args()

    entries = parse_feedback_log(args.log)
    analysis = analyze_misses(entries)

    # Check index coverage
    not_indexed = check_index_coverage(entries)
    if not_indexed:
        analysis["skills_missing_from_trigger_index"] = not_indexed

    if args.json_output:
        print(json.dumps(analysis, indent=2, ensure_ascii=False))
        return 0

    # Text output
    print(f"Routing Feedback Analysis")
    print(f"========================")
    print(f"Total misses recorded: {analysis.get('total_misses', 0)}")

    if analysis["total_misses"] == 0:
        print(analysis["message"])
        return 0

    print(f"Unique skills missed: {analysis.get('total_unique_skills', 0)}")
    print()

    if analysis.get("frequent_misses"):
        print("Most frequently missed skills:")
        for item in analysis["frequent_misses"]:
            print(f"  {item['skill']}: {item['miss_count']} miss(es)")
        print()

    if analysis.get("frequent_misroute_pairs"):
        print("Most common misroute pairs:")
        for item in analysis["frequent_misroute_pairs"]:
            print(f"  expected: {item['expected']} → routed to: {item['routed_to']} ({item['count']}x)")
        print()

    if not_indexed:
        print(f"⚠ Skills missing from trigger index: {', '.join(not_indexed)}")
        print()

    if analysis.get("auto_suggestions"):
        print("Optimization suggestions:")
        for s in analysis["auto_suggestions"]:
            print(f"  [{s['skill']}] {s['detail']}")
            if s.get("user_suggestions"):
                for us in s["user_suggestions"]:
                    print(f"    → {us}")
        print()

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
