from __future__ import annotations

import json
from copy import deepcopy
from functools import lru_cache
from pathlib import Path
from typing import Any

RUNTIME_REGISTRY_SCHEMA_VERSION = "framework-runtime-registry-v1"
_REPO_ROOT = Path(__file__).resolve().parents[3]
_DEFAULT_REGISTRY_PATH = _REPO_ROOT / "configs" / "framework" / "RUNTIME_REGISTRY.json"


def _registry_candidates(repo_root: Path | None = None) -> tuple[Path, ...]:
    candidates: list[Path] = []
    if repo_root is not None:
        candidates.append(repo_root / "configs" / "framework" / "RUNTIME_REGISTRY.json")
    candidates.append(_DEFAULT_REGISTRY_PATH)
    return tuple(dict.fromkeys(candidate.resolve() for candidate in candidates))


@lru_cache(maxsize=8)
def _load_runtime_registry_cached(cache_key: tuple[str, ...]) -> dict[str, Any]:
    for raw_path in cache_key:
        path = Path(raw_path)
        if not path.is_file():
            continue
        payload = json.loads(path.read_text(encoding="utf-8"))
        schema_version = payload.get("schema_version")
        if schema_version != RUNTIME_REGISTRY_SCHEMA_VERSION:
            raise ValueError(
                f"Unsupported runtime registry schema_version: {schema_version!r} at {path}"
            )
        return payload
    raise FileNotFoundError(
        "Could not find configs/framework/RUNTIME_REGISTRY.json in any runtime registry search path."
    )


def runtime_registry_path(repo_root: Path | None = None) -> Path:
    for path in _registry_candidates(repo_root):
        if path.is_file():
            return path
    return _DEFAULT_REGISTRY_PATH


def load_runtime_registry(repo_root: Path | None = None) -> dict[str, Any]:
    cache_key = tuple(str(path) for path in _registry_candidates(repo_root))
    return deepcopy(_load_runtime_registry_cached(cache_key))


def host_adapter_records(
    *,
    include_legacy_aliases: bool = False,
    repo_root: Path | None = None,
) -> tuple[dict[str, Any], ...]:
    payload = load_runtime_registry(repo_root)
    rows = payload.get("host_adapters")
    if not isinstance(rows, list):
        raise ValueError("Runtime registry host_adapters must be a list.")
    records: list[dict[str, Any]] = []
    for row in rows:
        if not isinstance(row, dict):
            continue
        lane = str(row.get("registry_lane", "default"))
        if not include_legacy_aliases and lane != "default":
            continue
        records.append(deepcopy(row))
    return tuple(records)


def host_adapter_record(
    adapter_id: str,
    *,
    include_legacy_aliases: bool = False,
    repo_root: Path | None = None,
) -> dict[str, Any]:
    for record in host_adapter_records(
        include_legacy_aliases=include_legacy_aliases,
        repo_root=repo_root,
    ):
        if record.get("adapter_id") == adapter_id:
            return deepcopy(record)
    raise KeyError(f"Unknown runtime-registry host adapter: {adapter_id}")


def default_host_peer_set(*, repo_root: Path | None = None) -> tuple[str, ...]:
    payload = load_runtime_registry(repo_root)
    rows = payload.get("default_host_peer_set")
    if not isinstance(rows, list):
        raise ValueError("Runtime registry default_host_peer_set must be a list.")
    return tuple(str(row) for row in rows)


def plugin_records(*, repo_root: Path | None = None) -> tuple[dict[str, Any], ...]:
    payload = load_runtime_registry(repo_root)
    rows = payload.get("plugins")
    if not isinstance(rows, list):
        raise ValueError("Runtime registry plugins must be a list.")
    return tuple(deepcopy(row) for row in rows if isinstance(row, dict))


def primary_plugin_record(*, repo_root: Path | None = None) -> dict[str, Any]:
    records = plugin_records(repo_root=repo_root)
    if not records:
        raise ValueError("Runtime registry must define at least one plugin record.")
    return deepcopy(records[0])


def workspace_bootstrap_defaults(*, repo_root: Path | None = None) -> dict[str, Any]:
    payload = load_runtime_registry(repo_root)
    defaults = payload.get("workspace_bootstrap_defaults")
    if not isinstance(defaults, dict):
        raise ValueError("Runtime registry workspace_bootstrap_defaults must be an object.")
    return deepcopy(defaults)
