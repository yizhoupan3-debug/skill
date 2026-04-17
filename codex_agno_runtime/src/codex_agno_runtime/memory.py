"""Lightweight fact memory store for the Codex Agno runtime."""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any, Mapping

from pydantic import BaseModel


USER_FACT_PATTERNS = [
    re.compile(r"\bmy name is (?P<value>[A-Za-z][A-Za-z0-9 _.-]{1,60}?)(?=[.!?\n]|$)", re.IGNORECASE),
    re.compile(r"\bi work at (?P<value>[A-Za-z0-9][A-Za-z0-9 &_.-]{1,80}?)(?=[.!?\n]|$)", re.IGNORECASE),
    re.compile(r"\bi prefer (?P<value>[A-Za-z0-9][A-Za-z0-9 &_.-]{1,80}?)(?=[.!?\n]|$)", re.IGNORECASE),
]
MEMORY_STORE_SCHEMA_VERSION = "runtime-memory-store-v1"
MEMORY_CONTROL_PLANE_SCHEMA_VERSION = "runtime-memory-control-plane-v1"
MEMORY_PROVENANCE_KIND = "filesystem.user-facts.v1"
_MEMORY_SERVICE_NAME = "memory"
_DEFAULT_MEMORY_SERVICE_DESCRIPTOR = {
    "authority": "rust-runtime-control-plane",
    "role": "memory-lifecycle",
    "projection": "python-thin-projection",
    "delegate_kind": "fact-memory-store",
}


def _slugify_user(user_id: str) -> str:
    """Create a filesystem-safe user key."""

    return re.sub(r"[^A-Za-z0-9._-]+", "-", user_id).strip("-") or "codex-user"


class MemoryControlPlaneDescriptor(BaseModel):
    """Rust-owned memory descriptor consumed by the Python compatibility host."""

    schema_version: str = MEMORY_CONTROL_PLANE_SCHEMA_VERSION
    runtime_control_plane_schema_version: str | None = None
    runtime_control_plane_authority: str = _DEFAULT_MEMORY_SERVICE_DESCRIPTOR["authority"]
    service: str = _MEMORY_SERVICE_NAME
    authority: str = _DEFAULT_MEMORY_SERVICE_DESCRIPTOR["authority"]
    role: str = _DEFAULT_MEMORY_SERVICE_DESCRIPTOR["role"]
    projection: str = _DEFAULT_MEMORY_SERVICE_DESCRIPTOR["projection"]
    delegate_kind: str = _DEFAULT_MEMORY_SERVICE_DESCRIPTOR["delegate_kind"]
    storage_family: str = "filesystem"
    extraction_kind: str = "regex-fact-extractor"
    provenance_kind: str = MEMORY_PROVENANCE_KIND
    memory_dir: str


class _PersistedFactMemory(BaseModel):
    """Versioned memory payload kept compatible with older fact-only snapshots."""

    version: int = 1
    schema_version: str = MEMORY_STORE_SCHEMA_VERSION
    control_plane: dict[str, Any] | None = None
    facts: list[str]


def _build_memory_control_plane_descriptor(
    *,
    control_plane_descriptor: Mapping[str, Any] | None,
    memory_dir: Path,
) -> MemoryControlPlaneDescriptor:
    payload: dict[str, Any] = {
        "memory_dir": str(memory_dir),
    }
    if isinstance(control_plane_descriptor, Mapping):
        payload["runtime_control_plane_schema_version"] = control_plane_descriptor.get("schema_version")
        payload["runtime_control_plane_authority"] = str(
            control_plane_descriptor.get("authority") or _DEFAULT_MEMORY_SERVICE_DESCRIPTOR["authority"]
        )
        services = control_plane_descriptor.get("services")
        if isinstance(services, Mapping):
            service = services.get(_MEMORY_SERVICE_NAME)
            if isinstance(service, Mapping):
                for field in ("authority", "role", "projection", "delegate_kind"):
                    value = service.get(field)
                    if value is not None:
                        payload[field] = value
    return MemoryControlPlaneDescriptor.model_validate(payload)


class FactMemoryStore:
    """Persist simple user fact lists on disk."""

    def __init__(
        self,
        memory_dir: Path,
        debounce_seconds: float = 5.0,
        *,
        control_plane_descriptor: Mapping[str, Any] | None = None,
    ) -> None:
        self.memory_dir = Path(memory_dir)
        self.memory_dir.mkdir(parents=True, exist_ok=True)
        self.debounce_seconds = debounce_seconds
        self._control_plane = _build_memory_control_plane_descriptor(
            control_plane_descriptor=control_plane_descriptor,
            memory_dir=self.memory_dir,
        )

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
        if isinstance(payload, list):
            facts = payload
        else:
            facts = payload.get("facts", [])
        if not isinstance(facts, list):
            return []
        return [str(item).strip() for item in facts if str(item).strip()]

    def save_facts(self, user_id: str, facts: list[str]) -> None:
        """Merge and persist new facts for one user."""

        path = self._facts_path(user_id)
        existing = self.load_facts(user_id)
        merged = self.dedupe_facts([*existing, *facts])
        payload = _PersistedFactMemory(
            facts=merged,
            control_plane=self._control_plane.model_dump(mode="json"),
        )
        path.write_text(
            payload.model_dump_json(indent=2) + "\n",
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
                    "kind": self._control_plane.provenance_kind,
                    "storage_path": str(path),
                    "control_plane_authority": self._control_plane.authority,
                    "control_plane_projection": self._control_plane.projection,
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
            "control_plane": self._control_plane.model_dump(mode="json"),
            "facts": self.retrieve_facts(user_id),
        }

    def control_plane_descriptor(self) -> MemoryControlPlaneDescriptor:
        """Return the Rust-owned control-plane projection for memory state."""

        return self._control_plane.model_copy()

    def health(self) -> dict[str, Any]:
        """Return host-visible memory health derived from the control plane."""

        descriptor = self.control_plane_descriptor()
        return {
            "control_plane_authority": descriptor.authority,
            "control_plane_role": descriptor.role,
            "control_plane_projection": descriptor.projection,
            "control_plane_delegate_kind": descriptor.delegate_kind,
            "runtime_control_plane_authority": descriptor.runtime_control_plane_authority,
            "runtime_control_plane_schema_version": descriptor.runtime_control_plane_schema_version,
            "storage_family": descriptor.storage_family,
            "extraction_kind": descriptor.extraction_kind,
            "memory_dir": descriptor.memory_dir,
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
