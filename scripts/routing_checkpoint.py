#!/usr/bin/env python3
"""Record and query routing decisions for evolution tracking.

Writes structured JSONL entries to .evolution_journal.jsonl and automatically
stages the change in Git to ensure the evolution history is versioned.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

def get_git_root() -> Path:
    local_root = Path(__file__).resolve().parents[1]
    if (local_root / "skills").is_dir():
        return local_root

    try:
        proc = subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            capture_output=True, text=True, check=True
        )
        return Path(proc.stdout.strip())
    except Exception:
        # Fallback to file-relative if not in a git repo
        return local_root

ROOT = get_git_root()
JOURNAL_PATH = ROOT / "skills" / ".evolution_journal.jsonl"

def record(
    task_summary: str,
    initial_skill: str,
    final_skill: str,
    confidence: float = 1.0,
    difficulty: int = 1,
    rerouted: bool = False,
    struggle_count: int = 0,
    notes: str = "",
) -> dict:
    entry = {
        "ts": datetime.now(timezone.utc).isoformat(),
        "task": task_summary[:200],
        "init": initial_skill,
        "final": final_skill,
        "conf": round(confidence, 2),
        "diff": difficulty,
        "reroute": rerouted or (initial_skill != final_skill),
        "struggle": struggle_count,
        "notes": notes[:200] if notes else "",
    }
    JOURNAL_PATH.parent.mkdir(parents=True, exist_ok=True)
    with JOURNAL_PATH.open("a", encoding="utf-8") as f:
        f.write(json.dumps(entry, ensure_ascii=False) + "\n")
    
    # Git-native: Auto-stage the journal
    try:
        subprocess.run(["git", "add", str(JOURNAL_PATH)], cwd=ROOT, check=True, capture_output=True)
    except Exception as e:
        print(f"Warning: Failed to stage journal in git: {e}", file=sys.stderr)
        
    return entry

def query(last_n: int = 10) -> list[dict]:
    if not JOURNAL_PATH.is_file():
        return []
    lines = JOURNAL_PATH.read_text(encoding="utf-8").strip().splitlines()
    entries = []
    for line in lines[-last_n:]:
        try:
            entries.append(json.loads(line))
        except json.JSONDecodeError:
            continue
    entries.reverse()
    return entries

def main() -> int:
    parser = argparse.ArgumentParser(description="Git-native routing decision recorder.")
    sub = parser.add_subparsers(dest="cmd")

    rec = sub.add_parser("record", help="Record a routing decision and stage to Git.")
    rec.add_argument("--task", required=True, help="Task summary.")
    rec.add_argument("--init", required=True, help="Initial skill.")
    rec.add_argument("--final", required=True, help="Final skill used.")
    rec.add_argument("--confidence", type=float, default=1.0)
    rec.add_argument("--difficulty", type=int, default=1)
    rec.add_argument("--struggle", type=int, default=0)
    rec.add_argument("--notes", default="")

    qry = sub.add_parser("query", help="Query recent entries.")
    qry.add_argument("--last", type=int, default=10)
    qry.add_argument("--json", action="store_true", dest="json_out")

    args = parser.parse_args()

    if args.cmd == "record":
        entry = record(
            args.task, args.init, args.final,
            confidence=args.confidence, difficulty=args.difficulty,
            struggle_count=args.struggle, notes=args.notes,
        )
        print(json.dumps(entry, ensure_ascii=False, indent=2))
        return 0

    if args.cmd == "query":
        entries = query(args.last)
        if args.json_out:
            print(json.dumps(entries, ensure_ascii=False, indent=2))
        else:
            for e in entries:
                flag = "⚠" if e.get("reroute") else "✓"
                print(f"  {flag} {e['ts'][:16]}  {e['init']:30s} → {e['final']:30s}  d={e['diff']}  s={e['struggle']}")
        return 0

    parser.print_help()
    return 1

if __name__ == "__main__":
    raise SystemExit(main())
