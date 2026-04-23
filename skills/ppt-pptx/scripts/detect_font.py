#!/usr/bin/env python3
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from rust_bridge import build_command


TIMEOUT_SECONDS = 25


def emit_timeout_fallback(argv: list[str]) -> int:
    warning = (
        f"font detection timed out after {TIMEOUT_SECONDS}s; "
        "returning degraded result without resolved-font substitution data"
    )
    if "--json" in argv:
        payload = {
            "font_missing_overall": [],
            "font_missing_by_slide": {},
            "font_substituted_overall": [],
            "font_substituted_by_slide": {},
            "warning": warning,
        }
        print(json.dumps(payload, ensure_ascii=False, indent=2))
    else:
        print("font_missing_overall: ")
        print("font_missing_by_slide: {}")
        print("font_substituted_overall: ")
        print("font_substituted_by_slide: {}")
        print(f"warning: {warning}", file=sys.stderr)
    return 0


def main(argv: list[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    command = build_command("detect-fonts", args)
    try:
        completed = subprocess.run(
            command,
            text=True,
            capture_output=True,
            timeout=TIMEOUT_SECONDS,
        )
    except subprocess.TimeoutExpired:
        return emit_timeout_fallback(args)

    if completed.stdout:
        sys.stdout.write(completed.stdout)
    if completed.stderr:
        sys.stderr.write(completed.stderr)
    return completed.returncode


if __name__ == "__main__":
    raise SystemExit(main())
