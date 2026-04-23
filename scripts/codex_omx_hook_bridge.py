#!/usr/bin/env python3
"""Compatibility no-op for stale Codex OMX hook sessions.

This repository now uses `.codex/hooks.json` for live Codex hooks. Older Codex
sessions may still have a cached stop-hook command that points at this legacy
bridge path. Keep this file as a silent no-op so those stale sessions do not
surface file-not-found or missing-hook-script noise while new sessions migrate
to the shared hook manifest.
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
