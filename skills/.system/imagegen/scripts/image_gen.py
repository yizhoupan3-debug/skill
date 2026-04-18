#!/usr/bin/env python3
"""Delegate to the local imagegen CLI implementation."""

from __future__ import annotations

from pathlib import Path
import runpy
import sys


def main() -> int:
    target = Path(__file__).resolve().parents[3] / "imagegen" / "scripts" / "image_gen.py"
    if not target.exists():
        print(f"Error: target CLI not found: {target}", file=sys.stderr)
        return 1
    runpy.run_path(str(target), run_name="__main__")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
