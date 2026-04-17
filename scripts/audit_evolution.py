#!/usr/bin/env python3
"""Analyze evolution journal and generate actionable optimization report.

Reads .evolution_journal.jsonl and produces statistics on routing quality,
reroute frequency, struggle patterns, and skill coverage gaps.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from datetime import datetime, timedelta, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
JOURNAL_PATH = ROOT / "skills" / ".evolution_journal.jsonl"


def load_entries(path: Path, days: int = 30) -> list[dict]:
    """Load journal entries from the last N days.

    Args:
        path: Path to JSONL journal file.
        days: Only include entries from the last N days.

    Returns:
        List of entry dicts.
    """
    if not path.is_file():
        return []
    cutoff = datetime.now(timezone.utc) - timedelta(days=days)
    entries = []
    for line in path.read_text(encoding="utf-8").strip().splitlines():
        if not line.strip():
            continue
        try:
            e = json.loads(line)
        except json.JSONDecodeError:
            continue
        try:
            ts = datetime.fromisoformat(e["ts"])
            if ts >= cutoff:
                entries.append(e)
        except (KeyError, ValueError):
            entries.append(e)
    return entries


def analyze(entries: list[dict]) -> dict:
    """Produce analysis from journal entries.

    Args:
        entries: List of journal entry dicts.

    Returns:
        Analysis dict with counts, patterns, and suggestions.
    """
    if not entries:
        return {
            "total": 0,
            "reroute_count": 0,
            "struggle_count": 0,
            "message": "No routing data yet.",
        }

    reroutes = [e for e in entries if e.get("reroute")]
    struggles = [e for e in entries if e.get("struggle", 0) >= 3]

    init_counter = Counter(e.get("init", "?") for e in entries)
    reroute_pairs = Counter(
        (e.get("init", "?"), e.get("final", "?"))
        for e in reroutes
    )

    suggestions = []
    for (init, final), count in reroute_pairs.most_common(5):
        if count >= 2:
            suggestions.append(
                f"`{init}` → `{final}` occurred {count}x — "
                f"tighten `{init}` description or add trigger phrase from `{final}`"
            )

    return {
        "total": len(entries),
        "reroute_count": len(reroutes),
        "struggle_count": len(struggles),
        "top_skills": init_counter.most_common(10),
        "top_reroute_pairs": reroute_pairs.most_common(5),
        "suggestions": suggestions,
        "avg_difficulty": round(
            sum(e.get("diff", 1) for e in entries) / len(entries), 1
        ),
        "avg_confidence": round(
            sum(e.get("conf", 1.0) for e in entries) / len(entries), 2
        ),
    }


def format_text(analysis: dict) -> str:
    """Format analysis as human-readable text.

    Args:
        analysis: Analysis dict from analyze().

    Returns:
        Formatted string.
    """
    if analysis["total"] == 0:
        return analysis.get("message", "No data.")

    lines = [
        f"Evolution Audit ({analysis['total']} decisions)",
        "=" * 50,
        f"Reroutes: {analysis['reroute_count']}  |  "
        f"Struggles: {analysis['struggle_count']}  |  "
        f"Avg difficulty: {analysis['avg_difficulty']}  |  "
        f"Avg confidence: {analysis['avg_confidence']}",
        "",
    ]
    if analysis.get("suggestions"):
        lines.append("Suggestions:")
        for s in analysis["suggestions"]:
            lines.append(f"  • {s}")
        lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Analyze routing evolution journal."
    )
    parser.add_argument("--days", type=int, default=30,
                        help="Look back N days (default: 30).")
    parser.add_argument("--json", action="store_true", dest="json_out",
                        help="Output as JSON.")
    parser.add_argument("--source", type=Path, default=JOURNAL_PATH,
                        help="Path to journal file.")
    args = parser.parse_args()

    entries = load_entries(args.source, days=args.days)
    result = analyze(entries)

    if args.json_out:
        print(json.dumps(result, ensure_ascii=False, indent=2, default=str))
    else:
        print(format_text(result))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
