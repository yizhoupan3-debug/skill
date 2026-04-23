#!/usr/bin/env python3
from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from rust_bridge import build_command, run_forwarded_tool


def summarize_doctor(
    file_path: str,
    outline_payload: dict[str, Any],
    issues_payload: dict[str, Any],
    validate_payload: dict[str, Any],
    version: str | None,
) -> dict[str, Any]:
    outline_data = outline_payload.get("data") or {}
    issues_data = issues_payload.get("data") or {}
    issue_list = issues_data.get("Issues") or []
    validate_message = validate_payload.get("message") or ""
    validation_ok = "0 validation error" in validate_message.lower() or (
        validate_payload.get("success") is True and "validation error" not in validate_message.lower()
    )
    overflow_issues = [
        item for item in issue_list if "overflow" in str(item.get("Message", "")).lower()
    ]
    title_issues = [
        item for item in issue_list if "no title" in str(item.get("Message", "")).lower()
    ]
    return {
        "officecli_version": version,
        "file": file_path,
        "outline": {
            "total_slides": outline_data.get("totalSlides"),
            "slides": outline_data.get("slides"),
        },
        "issues": {
            "count": issues_data.get("Count", len(issue_list)),
            "overflow_count": len(overflow_issues),
            "title_count": len(title_issues),
            "items": issue_list,
        },
        "validation": {
            "ok": validation_ok,
            "message": validate_message,
        },
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="OfficeCLI compatibility wrapper; forwards to pptx_tool_rs office."
    )
    sub = parser.add_subparsers(dest="command", required=True)

    probe = sub.add_parser("probe")
    probe.add_argument("--json", action="store_true")

    doctor = sub.add_parser("doctor")
    doctor.add_argument("file")
    doctor.add_argument("--json", action="store_true")
    doctor.add_argument("--fail-on-issues", action="store_true")
    doctor.add_argument("--fail-on-validation", action="store_true")

    outline = sub.add_parser("outline")
    outline.add_argument("file")
    outline.add_argument("--json", action="store_true")

    issues = sub.add_parser("issues")
    issues.add_argument("file")
    issues.add_argument("--json", action="store_true")

    validate = sub.add_parser("validate")
    validate.add_argument("file")
    validate.add_argument("--json", action="store_true")

    get_cmd = sub.add_parser("get")
    get_cmd.add_argument("file")
    get_cmd.add_argument("path", nargs="?", default="/")
    get_cmd.add_argument("--depth", type=int, default=1)
    get_cmd.add_argument("--json", action="store_true")

    query = sub.add_parser("query")
    query.add_argument("file")
    query.add_argument("selector")
    query.add_argument("--text")
    query.add_argument("--json", action="store_true")

    watch = sub.add_parser("watch")
    watch.add_argument("file")
    watch.add_argument("--port", type=int, default=18080)
    watch.add_argument("--browser", action="store_true")

    batch = sub.add_parser("batch")
    batch.add_argument("file")
    batch.add_argument("--input")
    batch.add_argument("--commands")
    batch.add_argument("--force", action="store_true")
    batch.add_argument("--json", action="store_true")

    return parser


def main(argv: list[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    if not args:
        return 2
    return run_forwarded_tool("office", args)


if __name__ == "__main__":
    raise SystemExit(main())
