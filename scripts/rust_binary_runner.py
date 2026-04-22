#!/usr/bin/env python3
"""Shared self-healing launcher for local Rust helper binaries."""

from __future__ import annotations

import subprocess
from pathlib import Path


def resolve_binary_candidate(*candidates: Path) -> Path | None:
    existing: list[tuple[float, int, Path]] = []
    for index, candidate in enumerate(candidates):
        if candidate.is_file():
            existing.append((candidate.stat().st_mtime, -index, candidate))
    if not existing:
        return None
    return max(existing)[2]


def latest_crate_source_mtime(crate_root: Path) -> float:
    candidates = [
        crate_root / "Cargo.toml",
        crate_root / "Cargo.lock",
        *crate_root.joinpath("src").rglob("*.rs"),
    ]
    return max((path.stat().st_mtime for path in candidates if path.is_file()), default=0.0)


def ensure_rust_binary(
    *,
    crate_root: Path,
    binary_name: str,
    release: bool,
    allow_stale_fallback: bool = True,
    allow_cross_profile_fallback: bool = True,
    cwd: Path | None = None,
) -> Path:
    manifest_path = crate_root / "Cargo.toml"
    profile = "release" if release else "debug"
    primary_candidate = crate_root / "target" / profile / binary_name
    fallback_candidate = crate_root / "target" / ("debug" if release else "release") / binary_name
    candidate_paths = (
        (primary_candidate, fallback_candidate)
        if allow_cross_profile_fallback
        else (primary_candidate,)
    )
    existing_binary = resolve_binary_candidate(*candidate_paths)
    latest_source_mtime = latest_crate_source_mtime(crate_root)

    if existing_binary is not None and existing_binary.stat().st_mtime >= latest_source_mtime:
        return existing_binary

    build_command = ["cargo", "build", "--quiet", "--manifest-path", str(manifest_path)]
    if release:
        build_command.insert(2, "--release")

    try:
        subprocess.run(
            build_command,
            cwd=cwd or crate_root.parent.parent,
            check=True,
            text=True,
            capture_output=True,
        )
    except subprocess.CalledProcessError as exc:
        if existing_binary is not None and allow_stale_fallback:
            return existing_binary
        stderr = (exc.stderr or exc.stdout or "").strip()
        raise RuntimeError(
            f"failed to build {binary_name} from {manifest_path}: {stderr}"
        ) from exc

    resolved_binary = resolve_binary_candidate(*candidate_paths)
    if resolved_binary is not None:
        return resolved_binary
    raise RuntimeError(f"expected compiled binary {binary_name!r} after building {manifest_path}")
