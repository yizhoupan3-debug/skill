#!/usr/bin/env python3
"""Compatibility entrypoint for framework-mcp configured via Python module.

Legacy `.codex/config.toml` entries still invoke:
`python3 -m scripts.framework_mcp`.

This shim now launches the Rust framework MCP directly so startup stays aligned
with the current implementation while keeping old config files functional.
"""

from __future__ import annotations

import os
import shutil
import sys
from pathlib import Path


def _resolve_router_binary(repo_root: Path) -> str | None:
    release_binary = (
        repo_root / "scripts" / "router-rs" / "target" / "release" / "router-rs"
    )
    if release_binary.is_file() and os.access(release_binary, os.X_OK):
        return str(release_binary)

    debug_binary = (
        repo_root / "scripts" / "router-rs" / "target" / "debug" / "router-rs"
    )
    if debug_binary.is_file() and os.access(debug_binary, os.X_OK):
        return str(debug_binary)

    return None


def _parse_repo_root(argv: list[str], repo_root: Path) -> str:
    if "--repo-root" not in argv:
        return str(repo_root)
    index = argv.index("--repo-root")
    if index + 1 < len(argv):
        return argv[index + 1]
    return str(repo_root)


def _prune_legacy_args(argv: list[str]) -> list[str]:
    filtered: list[str] = []
    skip_next = False
    for value in argv:
        if skip_next:
            skip_next = False
            continue

        if value == "--framework-mcp-stdio":
            continue
        if value == "--repo-root":
            skip_next = True
            continue
        if value.startswith("--repo-root="):
            continue
        filtered.append(value)
    return filtered


def main() -> int:
    repo_root = Path.cwd().resolve()
    extra_args = sys.argv[1:]
    resolved_repo_root = _parse_repo_root(extra_args, repo_root)
    forward_args = _prune_legacy_args(extra_args)

    binary = _resolve_router_binary(repo_root)
    if binary:
        os.execv(
            binary,
            [
                binary,
                "--framework-mcp-stdio",
                "--repo-root",
                resolved_repo_root,
                *forward_args,
            ],
        )

    cargo = shutil.which("cargo")
    if not cargo:
        print("Could not locate router-rs binary and `cargo` is unavailable.", file=sys.stderr)
        return 1

    manifest = repo_root / "scripts" / "router-rs" / "Cargo.toml"
    os.execv(
        cargo,
        [
            "cargo",
            "run",
            "--manifest-path",
            str(manifest),
            "--quiet",
            "--",
            "--framework-mcp-stdio",
            "--repo-root",
            resolved_repo_root,
            *forward_args,
        ],
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
