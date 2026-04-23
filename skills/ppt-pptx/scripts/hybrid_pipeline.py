#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from rust_bridge import run_forwarded_tool


def build_parser():
    import argparse

    parser = argparse.ArgumentParser(
        description="Hybrid pipeline compatibility wrapper; forwards to pptx_tool_rs."
    )
    sub = parser.add_subparsers(dest="command", required=True)

    qa = sub.add_parser("qa")
    qa.add_argument("deck")
    qa.add_argument("--rendered-dir", default="rendered")
    qa.add_argument("--json", action="store_true")

    intake = sub.add_parser("intake")
    intake.add_argument("deck")
    intake.add_argument("--json", action="store_true")

    build_qa = sub.add_parser("build-qa")
    build_qa.add_argument("--workdir", default=".")
    build_qa.add_argument("--entry", default="deck.js")
    build_qa.add_argument("--deck", default="deck.pptx")
    build_qa.add_argument("--rendered-dir", default="rendered")
    build_qa.add_argument("--json", action="store_true")

    watch = sub.add_parser("watch")
    watch.add_argument("deck")
    watch.add_argument("--port", type=int, default=18080)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    if args and args[0] == "watch":
        forwarded = ["office", "watch", *args[1:]]
    else:
        forwarded = args
    return run_forwarded_tool(forwarded[0], forwarded[1:])


if __name__ == "__main__":
    raise SystemExit(main())
