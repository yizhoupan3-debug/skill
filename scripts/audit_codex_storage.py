#!/usr/bin/env python3
"""Audit local Codex storage size and top entries."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))


def collect_storage_report(root: Path, *, top: int = 10) -> dict[str, Any]:
    """Collect a size report for files under a root."""

    entries: list[dict[str, Any]] = []
    total_bytes = 0
    if root.exists():
        for path in root.rglob("*"):
            if not path.is_file():
                continue
            size = path.stat().st_size
            total_bytes += size
            entries.append({"path": str(path), "bytes": size, "mib": round(size / (1024 * 1024), 3)})
    entries.sort(key=lambda item: item["bytes"], reverse=True)
    return {
        "root": str(root),
        "total_mib": round(total_bytes / (1024 * 1024), 3),
        "top_entries": entries[:top],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Audit Codex storage.")
    parser.add_argument("--root", type=Path, default=Path.home() / ".codex")
    parser.add_argument("--top", type=int, default=10)
    parser.add_argument("--json", action="store_true", dest="json_output")
    args = parser.parse_args()
    report = collect_storage_report(args.root, top=args.top)
    if args.json_output:
        print(json.dumps(report, ensure_ascii=False, indent=2))
    else:
        print(report)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
