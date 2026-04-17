"""Lightweight fact memory store for the Codex Agno runtime."""

from __future__ import annotations

import json
import re
from pathlib import Path


USER_FACT_PATTERNS = [
    re.compile(r"\bmy name is (?P<value>[A-Za-z][A-Za-z0-9 _.-]{1,60}?)(?=[.!?\n]|$)", re.IGNORECASE),
    re.compile(r"\bi work at (?P<value>[A-Za-z0-9][A-Za-z0-9 &_.-]{1,80}?)(?=[.!?\n]|$)", re.IGNORECASE),
    re.compile(r"\bi prefer (?P<value>[A-Za-z0-9][A-Za-z0-9 &_.-]{1,80}?)(?=[.!?\n]|$)", re.IGNORECASE),
]
MEMORY_STORE_SCHEMA_VERSION = "runtime-memory-store-v1"
MEMORY_PROVENANCE_KIND = "filesystem.user-facts.v1"


def _slugify_user(user_id: str) -> str:
    """Create a filesystem-safe user key."""

    return re.sub(r"[^A-Za-z0-9._-]+", "-", user_id).strip("-") or "codex-user"


class FactMemoryStore:
    """Persist simple user fact lists on disk."""

    def __init__(self, memory_dir: Path, debounce_seconds: float = 5.0) -> None:
        self.memory_dir = Path(memory_dir)
        self.memory_dir.mkdir(parents=True, exist_ok=True)
        self.debounce_seconds = debounce_seconds

    def _facts_path(self, user_id: str) -> Path:
        """Return the storage path for one user."""

        return self.memory_dir / f"{_slugify_user(user_id)}.json"

    def load_facts(self, user_id: str) -> list[str]:
        """Load persisted facts for one user."""

        path = self._facts_path(user_id)
        if not path.is_file():
            return []
        try:
            payload = json.loads(path.read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            return []
        facts = payload.get("facts", [])
        if not isinstance(facts, list):
            return []
        return [str(item).strip() for item in facts if str(item).strip()]

    def save_facts(self, user_id: str, facts: list[str]) -> None:
        """Merge and persist new facts for one user."""

        path = self._facts_path(user_id)
        existing = self.load_facts(user_id)
        merged = self.dedupe_facts([*existing, *facts])
        path.write_text(
            json.dumps({"version": 1, "facts": merged}, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )

    def dedupe_facts(self, facts: list[str]) -> list[str]:
        """Apply the stable dedupe contract while preserving insertion order."""

        merged: list[str] = []
        seen: set[str] = set()
        for fact in facts:
            cleaned = fact.strip()
            lowered = cleaned.casefold()
            if not cleaned or lowered in seen:
                continue
            seen.add(lowered)
            merged.append(cleaned)
        return merged

    def retrieve_facts(self, user_id: str, *, limit: int | None = None) -> list[dict[str, object]]:
        """Return structured retrieval rows with deterministic rank and provenance."""

        path = self._facts_path(user_id)
        facts = self.load_facts(user_id)
        rows = [
            {
                "value": fact,
                "rank": index + 1,
                "provenance": {
                    "kind": MEMORY_PROVENANCE_KIND,
                    "storage_path": str(path),
                },
            }
            for index, fact in enumerate(facts)
        ]
        if limit is not None:
            return rows[:limit]
        return rows

    def contract_snapshot(self, user_id: str) -> dict[str, object]:
        """Return a versioned memory contract snapshot for fixtures/tests."""

        return {
            "schema_version": MEMORY_STORE_SCHEMA_VERSION,
            "user_id": user_id,
            "storage_path": str(self._facts_path(user_id)),
            "facts": self.retrieve_facts(user_id),
        }

    def extract_facts_sync(self, conversation: str) -> list[str]:
        """Extract explicit user facts from a conversation transcript."""

        extracted: list[str] = []
        for pattern in USER_FACT_PATTERNS:
            for match in pattern.finditer(conversation):
                value = " ".join(match.group("value").split())
                if value:
                    extracted.append(value)

        unique: list[str] = []
        seen: set[str] = set()
        for fact in extracted:
            lowered = fact.casefold()
            if lowered in seen:
                continue
            seen.add(lowered)
            unique.append(fact)
        return unique
