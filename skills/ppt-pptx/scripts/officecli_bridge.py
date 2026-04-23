#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any


def detect_officecli() -> tuple[str | None, str | None]:
    binary = shutil.which("officecli")
    if not binary:
        return None, None
    try:
        version = subprocess.run(
            [binary, "--version"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
    except Exception:
        version = None
    return binary, version


def require_officecli() -> tuple[str, str | None]:
    binary, version = detect_officecli()
    if not binary:
        raise SystemExit(
            "officecli not found. Install it first, then rerun this command."
        )
    return binary, version


def run_json(binary: str, args: list[str]) -> dict[str, Any]:
    proc = subprocess.run(
        [binary, *args, "--json"],
        check=False,
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        raise RuntimeError(
            f"officecli command failed: {' '.join([binary, *args, '--json'])}\n"
            f"stdout:\n{proc.stdout}\n"
            f"stderr:\n{proc.stderr}"
        )
    try:
        return json.loads(proc.stdout)
    except json.JSONDecodeError as exc:
        raise RuntimeError(
            f"officecli did not return valid JSON for args={args!r}\nstdout:\n{proc.stdout}"
        ) from exc


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


def print_text_summary(summary: dict[str, Any]) -> None:
    outline = summary["outline"]
    issues = summary["issues"]
    validation = summary["validation"]
    print(f"officecli: {summary.get('officecli_version') or 'unknown'}")
    print(f"file: {summary['file']}")
    print(f"slides: {outline.get('total_slides')}")
    print(
        "issues: "
        f"total={issues.get('count', 0)} "
        f"overflow={issues.get('overflow_count', 0)} "
        f"missing_title={issues.get('title_count', 0)}"
    )
    print(f"validation_ok: {validation.get('ok')}")
    if validation.get("message"):
        print(f"validation_message: {validation['message']}")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Bridge selected OfficeCLI capabilities into skills/ppt-pptx."
    )
    sub = parser.add_subparsers(dest="command", required=True)

    probe = sub.add_parser("probe", help="Detect local officecli and print version")
    probe.add_argument("--json", action="store_true")

    doctor = sub.add_parser("doctor", help="Run outline + issues + validate summary")
    doctor.add_argument("file")
    doctor.add_argument("--json", action="store_true")
    doctor.add_argument("--fail-on-issues", action="store_true")
    doctor.add_argument("--fail-on-validation", action="store_true")

    outline = sub.add_parser("outline", help="Run `officecli view ... outline`")
    outline.add_argument("file")
    outline.add_argument("--json", action="store_true")

    issues = sub.add_parser("issues", help="Run `officecli view ... issues`")
    issues.add_argument("file")
    issues.add_argument("--json", action="store_true")

    validate = sub.add_parser("validate", help="Run `officecli validate`")
    validate.add_argument("file")
    validate.add_argument("--json", action="store_true")

    get_cmd = sub.add_parser("get", help="Run `officecli get` safely with path quoting")
    get_cmd.add_argument("file")
    get_cmd.add_argument("path", nargs="?", default="/")
    get_cmd.add_argument("--depth", type=int, default=1)
    get_cmd.add_argument("--json", action="store_true")

    query = sub.add_parser("query", help="Run `officecli query`")
    query.add_argument("file")
    query.add_argument("selector")
    query.add_argument("--text")
    query.add_argument("--json", action="store_true")

    watch = sub.add_parser("watch", help="Run `officecli watch`")
    watch.add_argument("file")
    watch.add_argument("--port", type=int, default=18080)
    watch.add_argument("--browser", action="store_true")

    batch = sub.add_parser("batch", help="Run `officecli batch`")
    batch.add_argument("file")
    batch.add_argument("--input")
    batch.add_argument("--commands")
    batch.add_argument("--force", action="store_true")
    batch.add_argument("--json", action="store_true")

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    if args.command == "probe":
        binary, version = detect_officecli()
        payload = {"available": bool(binary), "binary": binary, "version": version}
        if args.json:
            print(json.dumps(payload, ensure_ascii=False, indent=2))
        else:
            if binary:
                print(f"officecli: {binary}")
                print(f"version: {version or 'unknown'}")
            else:
                print("officecli: missing")
        return 0 if binary else 1

    binary, version = require_officecli()

    if args.command == "doctor":
        outline_payload = run_json(binary, ["view", args.file, "outline"])
        issues_payload = run_json(binary, ["view", args.file, "issues"])
        validate_payload = run_json(binary, ["validate", args.file])
        summary = summarize_doctor(
            args.file,
            outline_payload,
            issues_payload,
            validate_payload,
            version,
        )
        if args.json:
            print(json.dumps(summary, ensure_ascii=False, indent=2))
        else:
            print_text_summary(summary)

        failed = False
        if args.fail_on_issues and summary["issues"]["count"] > 0:
            failed = True
        if args.fail_on_validation and not summary["validation"]["ok"]:
            failed = True
        return 1 if failed else 0

    if args.command == "outline":
        payload = run_json(binary, ["view", args.file, "outline"])
        print(json.dumps(payload, ensure_ascii=False, indent=2))
        return 0

    if args.command == "issues":
        payload = run_json(binary, ["view", args.file, "issues"])
        print(json.dumps(payload, ensure_ascii=False, indent=2))
        return 0

    if args.command == "validate":
        payload = run_json(binary, ["validate", args.file])
        print(json.dumps(payload, ensure_ascii=False, indent=2))
        return 0

    if args.command == "get":
        command = [binary, "get", args.file, args.path, "--depth", str(args.depth)]
        if args.json:
            command.append("--json")
        completed = subprocess.run(command)
        return completed.returncode

    if args.command == "query":
        command = [binary, "query", args.file, args.selector]
        if args.text:
            command.extend(["--text", args.text])
        if args.json:
            command.append("--json")
        completed = subprocess.run(command)
        return completed.returncode

    if args.command == "watch":
        command = [binary, "watch", args.file, "--port", str(args.port)]
        completed = subprocess.run(command)
        if completed.returncode != 0:
            return completed.returncode
        if args.browser:
            import webbrowser

            webbrowser.open(f"http://127.0.0.1:{args.port}")
        return 0

    if args.command == "batch":
        command = [binary, "batch", args.file]
        if args.input:
            command.extend(["--input", args.input])
        if args.commands:
            command.extend(["--commands", args.commands])
        if args.force:
            command.append("--force")
        if args.json:
            command.append("--json")
        completed = subprocess.run(command)
        return completed.returncode

    parser.error(f"unknown command: {args.command}")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
