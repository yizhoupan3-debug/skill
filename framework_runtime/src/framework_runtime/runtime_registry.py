from __future__ import annotations

import json
from copy import deepcopy
from functools import lru_cache
from pathlib import Path
from typing import Any

RUNTIME_REGISTRY_SCHEMA_VERSION = "framework-runtime-registry-v1"
_REPO_ROOT = Path(__file__).resolve().parents[3]
_DEFAULT_REGISTRY_PATH = _REPO_ROOT / "configs" / "framework" / "RUNTIME_REGISTRY.json"


def _resolved_repo_root(repo_root: Path | None) -> Path:
    return (repo_root or _REPO_ROOT).resolve()


def _validate_runtime_registry_payload(payload: dict[str, Any], *, source: str) -> dict[str, Any]:
    schema_version = payload.get("schema_version")
    if schema_version != RUNTIME_REGISTRY_SCHEMA_VERSION:
        raise ValueError(f"Unsupported runtime registry schema_version: {schema_version!r} at {source}")
    return payload


@lru_cache(maxsize=8)
def _load_runtime_registry_cached(repo_root_key: str) -> dict[str, Any]:
    repo_root = Path(repo_root_key)
    registry_path = runtime_registry_path(repo_root)
    payload = json.loads(registry_path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"Runtime registry payload must be an object at {registry_path}")
    return _validate_runtime_registry_payload(payload, source=str(registry_path))


def _load_runtime_registry_or_none(repo_root: Path | None = None) -> dict[str, Any] | None:
    path = runtime_registry_path(repo_root)
    if not path.is_file():
        return None
    return deepcopy(_load_runtime_registry_cached(str(_resolved_repo_root(repo_root))))


def runtime_registry_path(repo_root: Path | None = None) -> Path:
    if repo_root is None:
        return _DEFAULT_REGISTRY_PATH
    return _resolved_repo_root(repo_root) / "configs" / "framework" / "RUNTIME_REGISTRY.json"


def load_runtime_registry(repo_root: Path | None = None) -> dict[str, Any]:
    payload = _load_runtime_registry_or_none(repo_root)
    if payload is None:
        raise FileNotFoundError(f"Missing runtime registry: {runtime_registry_path(repo_root)}")
    return payload


def host_adapter_records(
    *,
    include_legacy_aliases: bool = False,
    repo_root: Path | None = None,
) -> tuple[dict[str, Any], ...]:
    if include_legacy_aliases:
        raise ValueError("legacy host adapter aliases are retired; use runtime-registry default lane only.")
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


def shared_project_mcp_servers(*, repo_root: Path | None = None) -> tuple[str, ...]:
    payload = load_runtime_registry(repo_root)
    rows = payload.get("shared_project_mcp_servers")
    if not isinstance(rows, list):
        raise ValueError("Runtime registry shared_project_mcp_servers must be a list.")
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


def framework_native_aliases(*, repo_root: Path | None = None) -> dict[str, Any]:
    payload = load_runtime_registry(repo_root)
    aliases = payload.get("framework_native_aliases")
    if not isinstance(aliases, dict):
        raise ValueError("Runtime registry framework_native_aliases must be an object.")
    return deepcopy(aliases)


def omc_retirement_contract(*, repo_root: Path | None = None) -> dict[str, Any]:
    payload = load_runtime_registry(repo_root)
    contract = payload.get("omc_retirement_contract")
    if not isinstance(contract, dict):
        raise ValueError("Runtime registry omc_retirement_contract must be an object.")
    return deepcopy(contract)
