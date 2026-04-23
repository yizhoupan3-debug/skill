"""Shared text and identity helpers for the Codex Agno runtime."""

from __future__ import annotations

import hashlib
import re
import unicodedata
from pathlib import Path

TOKEN_PATTERN = re.compile(r"[A-Za-z0-9.+#/_-]+|[\u4e00-\u9fff]{2,}")
SLUG_PATTERN = re.compile(r"[^a-z0-9._-]+")


def normalize_text(text: str) -> str:
    """Normalize text for routing comparisons.

    Parameters:
        text: Raw text.

    Returns:
        str: Normalized comparison text.
    """

    normalized = unicodedata.normalize("NFKC", text or "").casefold()
    return " ".join(normalized.split())


def tokenize(text: str) -> list[str]:
    """Tokenize mixed Chinese/English routing text.

    Parameters:
        text: Raw text.

    Returns:
        list[str]: Ordered tokens.
    """

    return TOKEN_PATTERN.findall(normalize_text(text))


def estimate_tokens(text: str) -> int:
    """Estimate token count conservatively for prompt budgeting.

    Parameters:
        text: Prompt text.

    Returns:
        int: Estimated token count.
    """

    normalized = text or ""
    rough_by_chars = max(1, len(normalized) // 4)
    rough_by_terms = max(1, len(tokenize(normalized)))
    return max(rough_by_chars, rough_by_terms)


def build_session_id(project_id: str | None, task: str, codex_home: Path, session_id: str | None = None) -> str:
    """Build a stable session identifier.

    Parameters:
        project_id: Optional project identifier.
        task: Original task text.
        codex_home: Codex home path.
        session_id: Optional explicit session id override.

    Returns:
        str: Stable session identifier.
    """

    if session_id:
        return session_id

    base = normalize_text(project_id or codex_home.name or "codex-runtime")
    base = SLUG_PATTERN.sub("-", base).strip("-") or "codex-runtime"
    digest = hashlib.sha1(f"{base}\0{task}".encode("utf-8")).hexdigest()[:10]
    return f"{base[:40]}-{digest}"
