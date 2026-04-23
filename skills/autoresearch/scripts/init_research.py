#!/usr/bin/env python3
"""Backward-compatible wrapper around research_ctl init."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from research_ctl import init_workspace


def main() -> None:
    parser = argparse.ArgumentParser(description="Initialize an autoresearch workspace")
    parser.add_argument("--project", required=True, help="Project name")
    parser.add_argument("--question", required=True, help="One-sentence research question")
    parser.add_argument("--dir", default=".", help="Parent directory (default: current)")
    parser.add_argument("--mode", choices=["quick", "full"], default="quick")
    args = parser.parse_args()

    root = init_workspace(args.project, args.question, args.dir, args.mode)
    print(f"Initialized autoresearch workspace at {root}")


if __name__ == "__main__":
    main()
