#!/usr/bin/env python3
"""Compatibility no-op for stale Codex OMX hook sessions.

This repository no longer enables project-level Codex hooks. Older live Codex
sessions may still have a cached stop-hook command that points at this path.
Keep this file as a silent no-op so those stale sessions do not keep surfacing
file-not-found or missing-hook-script noise while new sessions remain hook-free.
"""

from __future__ import annotations

import sys


def main() -> int:
    _ = sys.argv
    try:
        _ = sys.stdin.buffer.read()
    except Exception:
        pass
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
