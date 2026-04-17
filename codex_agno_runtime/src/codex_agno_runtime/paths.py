"""Filesystem path helpers for the Codex Agno runtime."""

from __future__ import annotations

import os
from pathlib import Path


def default_codex_home() -> Path:
    """Return the default Codex home for local runtime development.

    Preference order:
    1. Explicit environment configuration
    2. Nearest ancestor containing a local `skills/` directory
    3. Current working directory when it contains `skills/`
    4. User-level `~/.codex`

    Returns:
        Path: The resolved Codex home path.
    """

    env_value = os.environ.get("CODEX_HOME") or os.environ.get("CODEX_AGNO_CODEX_HOME")
    if env_value:
        return Path(env_value).expanduser().resolve()

    current = Path(__file__).resolve()
    for parent in current.parents:
        if (parent / "skills").is_dir():
            return parent

    cwd = Path.cwd().resolve()
    if (cwd / "skills").is_dir():
        return cwd

    return (Path.home() / ".codex").expanduser()
