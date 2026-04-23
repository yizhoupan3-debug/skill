#!/usr/bin/env python3
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path


def _repo_root_from_script() -> Path | None:
    script_path = Path(__file__).resolve()
    try:
        return script_path.parents[3]
    except IndexError:
        return None


def _candidate_binary_paths(repo_root: Path) -> list[Path]:
    target = repo_root / "rust_tools" / "target"
    return [
        target / "release" / "pptx_tool_rs",
        target / "debug" / "pptx_tool_rs",
    ]


def build_command(subcommand: str, argv: list[str]) -> list[str]:
    env_bin = os.environ.get("PPT_PPTX_RUST_TOOL_BIN")
    if env_bin:
        return [env_bin, subcommand, *argv]

    repo_root = _repo_root_from_script()
    if repo_root is not None:
        for binary in _candidate_binary_paths(repo_root):
            if binary.exists():
                return [str(binary), subcommand, *argv]

    manifest = os.environ.get("PPT_PPTX_RUST_TOOL_MANIFEST")
    if manifest:
        return ["cargo", "run", "--manifest-path", manifest, "--", subcommand, *argv]

    if repo_root is not None:
        manifest = repo_root / "rust_tools" / "pptx_tool_rs" / "Cargo.toml"
        if manifest.exists():
            return ["cargo", "run", "--manifest-path", str(manifest), "--", subcommand, *argv]

    raise SystemExit("Could not locate pptx_tool_rs binary or manifest.")


def run_rust_tool(subcommand: str, argv: list[str] | None = None) -> int:
    command = build_command(subcommand, argv or sys.argv[1:])
    completed = subprocess.run(command)
    return completed.returncode


def run_forwarded_tool(subcommand: str, argv: list[str] | None = None) -> int:
    command = build_command(subcommand, list(sys.argv[1:] if argv is None else argv))
    completed = subprocess.run(command)
    return completed.returncode
