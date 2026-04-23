#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent


def run(
    cmd: list[str],
    *,
    cwd: Path | None = None,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    proc = subprocess.run(
        cmd,
        cwd=str(cwd) if cwd else None,
        text=True,
        capture_output=True,
    )
    if check and proc.returncode != 0:
        raise RuntimeError(
            f"command failed: {' '.join(cmd)}\nstdout:\n{proc.stdout}\nstderr:\n{proc.stderr}"
        )
    return proc


def run_json(cmd: list[str], *, cwd: Path | None = None) -> dict[str, Any]:
    proc = run(cmd, cwd=cwd)
    payload_text = proc.stdout.strip()
    try:
        return json.loads(payload_text)
    except json.JSONDecodeError as exc:
        start = payload_text.find("{")
        end = payload_text.rfind("}")
        if start != -1 and end != -1 and end >= start:
            try:
                return json.loads(payload_text[start : end + 1])
            except json.JSONDecodeError:
                pass
        raise RuntimeError(
            f"command did not return valid JSON: {' '.join(cmd)}\nstdout:\n{proc.stdout}"
        ) from exc


def qa_summary(deck_path: str, rendered_dir: str) -> dict[str, Any]:
    render_proc = run(
        [sys.executable, str(SCRIPT_DIR / "render_slides.py"), deck_path, "--output_dir", rendered_dir]
    )
    render_paths = [line.strip() for line in render_proc.stdout.splitlines() if line.strip()]

    slides_proc = run(
        [sys.executable, str(SCRIPT_DIR / "slides_test.py"), deck_path],
        check=False,
    )
    fonts_payload = run_json(
        [sys.executable, str(SCRIPT_DIR / "detect_font.py"), deck_path, "--json"]
    )
    officecli_payload = run_json(
        [sys.executable, str(SCRIPT_DIR / "officecli_bridge.py"), "doctor", deck_path, "--json"]
    )

    return {
        "deck": deck_path,
        "render": {
            "rendered_dir": rendered_dir,
            "png_count": len(render_paths),
            "paths": render_paths,
        },
        "overflow_check": {
            "ok": slides_proc.returncode == 0,
            "stdout": slides_proc.stdout.strip(),
            "stderr": slides_proc.stderr.strip(),
        },
        "font_check": fonts_payload,
        "officecli": officecli_payload,
    }


def intake_summary(deck_path: str) -> dict[str, Any]:
    structure = run_json(
        [sys.executable, str(SCRIPT_DIR / "extract_pptx_structure.py"), deck_path]
    )
    officecli = run_json(
        [sys.executable, str(SCRIPT_DIR / "officecli_bridge.py"), "doctor", deck_path, "--json"]
    )
    return {
        "deck": deck_path,
        "structure": structure,
        "officecli": officecli,
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Hybrid pipeline for ppt-pptx: deck.js authoring + Rust QA + OfficeCLI audit."
    )
    sub = parser.add_subparsers(dest="command", required=True)

    qa = sub.add_parser("qa", help="Run Rust QA + OfficeCLI doctor on an existing deck")
    qa.add_argument("deck")
    qa.add_argument("--rendered-dir", default="rendered")
    qa.add_argument("--json", action="store_true")

    intake = sub.add_parser(
        "intake",
        help="Run rebuild intake on an existing deck using Rust structure extraction + OfficeCLI doctor",
    )
    intake.add_argument("deck")
    intake.add_argument("--json", action="store_true")

    build_qa = sub.add_parser(
        "build-qa",
        help="Run `node deck.js` in a workspace, then execute hybrid QA on the produced deck.pptx",
    )
    build_qa.add_argument("--workdir", default=".")
    build_qa.add_argument("--entry", default="deck.js")
    build_qa.add_argument("--deck", default="deck.pptx")
    build_qa.add_argument("--rendered-dir", default="rendered")
    build_qa.add_argument("--json", action="store_true")

    watch = sub.add_parser(
        "watch",
        help="Run OfficeCLI watch on an existing deck for live HTML preview",
    )
    watch.add_argument("deck")
    watch.add_argument("--port", type=int, default=18080)

    return parser


def emit(payload: dict[str, Any], as_json: bool) -> None:
    if as_json:
        print(json.dumps(payload, ensure_ascii=False, indent=2))
        return
    print(json.dumps(payload, ensure_ascii=False, indent=2))


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    if args.command == "qa":
        payload = qa_summary(args.deck, args.rendered_dir)
        emit(payload, args.json)
        return 0

    if args.command == "intake":
        payload = intake_summary(args.deck)
        emit(payload, args.json)
        return 0

    if args.command == "build-qa":
        workdir = Path(args.workdir).resolve()
        run(["node", args.entry], cwd=workdir)
        payload = qa_summary(str(workdir / args.deck), str(workdir / args.rendered_dir))
        emit(payload, args.json)
        return 0

    if args.command == "watch":
        completed = subprocess.run(
            [
                sys.executable,
                str(SCRIPT_DIR / "officecli_bridge.py"),
                "watch",
                args.deck,
                "--port",
                str(args.port),
            ]
        )
        return completed.returncode

    parser.error(f"unknown command: {args.command}")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
