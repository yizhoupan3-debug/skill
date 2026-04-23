#!/usr/bin/env python3
"""Inspect an XLSX workbook through the Rust OOXML parser."""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
MANIFEST = REPO_ROOT / "rust_tools" / "ooxml_parser_rs" / "Cargo.toml"
RELEASE_BIN = REPO_ROOT / "rust_tools" / "target" / "release" / "ooxml_parser_rs"
DEBUG_BIN = REPO_ROOT / "rust_tools" / "target" / "debug" / "ooxml_parser_rs"


def rust_command() -> list[str]:
    for candidate in (RELEASE_BIN, DEBUG_BIN):
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return [str(candidate)]

    cargo = shutil.which("cargo")
    if not cargo:
        raise SystemExit("Rust binary not built and `cargo` is unavailable.")

    return [
        cargo,
        "run",
        "--quiet",
        "--release",
        "--manifest-path",
        str(MANIFEST),
        "--",
    ]


def main() -> int:
    parser = argparse.ArgumentParser(description="Inspect an XLSX workbook.")
    parser.add_argument("workbook", type=Path, help="Path to .xlsx workbook")
    parser.add_argument("--json", action="store_true", help="Emit JSON summary")
    args = parser.parse_args()

    if not args.workbook.is_file():
        raise SystemExit(f"Workbook not found: {args.workbook}")

    command = rust_command() + ["xlsx", str(args.workbook.resolve())]
    if args.json:
        command.append("--json")

    proc = subprocess.run(command)
    return proc.returncode


if __name__ == "__main__":
    raise SystemExit(main())
