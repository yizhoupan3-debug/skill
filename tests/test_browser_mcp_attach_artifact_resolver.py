from __future__ import annotations

import json
import os
import sqlite3
import subprocess
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]
RESOLVER_SCRIPT = (
    PROJECT_ROOT / "tools" / "browser-mcp" / "scripts" / "resolve_runtime_attach_artifact.mjs"
)
NODE_BIN = os.environ.get("NODE_BIN") or "node"


def _write_json(path: Path, payload: dict[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def _run_resolver_cli(search_root: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            NODE_BIN,
            str(RESOLVER_SCRIPT),
            "--search-root",
            str(search_root),
        ],
        text=True,
        capture_output=True,
        check=False,
    )


def test_resolver_prefers_newest_resume_manifest_event_transport_path(tmp_path: Path) -> None:
    search_root = tmp_path / "scratch"
    older_manifest = search_root / "older" / "TRACE_RESUME_MANIFEST.json"
    newer_manifest = search_root / "newer" / "TRACE_RESUME_MANIFEST.json"

    _write_json(
        older_manifest,
        {
            "schema_version": "runtime-resume-manifest-v1",
            "event_transport_path": "/tmp/runtime_event_transports/older.json",
            "updated_at": "2026-04-23T00:00:00+00:00",
        },
    )
    _write_json(
        newer_manifest,
        {
            "schema_version": "runtime-resume-manifest-v1",
            "event_transport_path": "/tmp/runtime_event_transports/newer.json",
            "updated_at": "2026-04-23T00:05:00+00:00",
        },
    )

    completed = _run_resolver_cli(search_root)
    assert completed.returncode == 0
    assert completed.stdout.strip() == str(newer_manifest.resolve())


def test_resolver_falls_back_to_manifest_file_recency_when_updated_at_is_invalid(tmp_path: Path) -> None:
    search_root = tmp_path / "scratch"
    older_manifest = search_root / "older" / "TRACE_RESUME_MANIFEST.json"
    newer_manifest = search_root / "newer" / "TRACE_RESUME_MANIFEST.json"

    _write_json(
        older_manifest,
        {
            "schema_version": "runtime-resume-manifest-v1",
            "event_transport_path": "/tmp/runtime_event_transports/older.json",
            "updated_at": "not-a-timestamp",
        },
    )
    _write_json(
        newer_manifest,
        {
            "schema_version": "runtime-resume-manifest-v1",
            "event_transport_path": "/tmp/runtime_event_transports/newer.json",
            "updated_at": "still-not-a-timestamp",
        },
    )
    os.utime(older_manifest, (1_700_000_000, 1_700_000_000))
    os.utime(newer_manifest, (1_700_000_100, 1_700_000_100))

    completed = _run_resolver_cli(search_root)
    assert completed.returncode == 0
    assert completed.stdout.strip() == str(newer_manifest.resolve())


def test_resolver_reads_sqlite_resume_manifest_payloads(tmp_path: Path) -> None:
    search_root = tmp_path / "scratch"
    db_path = search_root / "sqlite-run" / "runtime_checkpoint_store.sqlite3"
    db_path.parent.mkdir(parents=True, exist_ok=True)

    connection = sqlite3.connect(db_path)
    connection.execute(
        "CREATE TABLE runtime_storage_payloads (payload_key TEXT PRIMARY KEY, payload_text TEXT NOT NULL)"
    )
    payload = json.dumps(
        {
            "schema_version": "runtime-resume-manifest-v1",
            "event_transport_path": "/logical/sqlite/runtime_event_transports/session__job.json",
            "updated_at": "2026-04-23T00:10:00+00:00",
        }
    )
    connection.execute(
        "INSERT INTO runtime_storage_payloads (payload_key, payload_text) VALUES (?, ?)",
        ("runtime-data/TRACE_RESUME_MANIFEST.json", payload),
    )
    connection.commit()
    connection.close()

    completed = _run_resolver_cli(search_root)
    assert completed.returncode == 0
    assert completed.stdout.strip() == "runtime-data/TRACE_RESUME_MANIFEST.json"


def test_resolver_falls_back_to_sqlite_payload_key_for_binding_candidates(tmp_path: Path) -> None:
    search_root = tmp_path / "scratch"
    db_path = search_root / "sqlite-run" / "runtime_checkpoint_store.sqlite3"
    db_path.parent.mkdir(parents=True, exist_ok=True)

    connection = sqlite3.connect(db_path)
    connection.execute(
        "CREATE TABLE runtime_storage_payloads (payload_key TEXT PRIMARY KEY, payload_text TEXT NOT NULL)"
    )
    connection.execute(
        "INSERT INTO runtime_storage_payloads (payload_key, payload_text) VALUES (?, ?)",
        (
            "runtime-data/runtime_event_transports/sqlite-session.json",
            json.dumps(
                {
                    "schema_version": "runtime-event-transport-v1",
                    "binding_backend_family": "sqlite",
                }
            ),
        ),
    )
    connection.commit()
    connection.close()

    completed = _run_resolver_cli(search_root)
    assert completed.returncode == 0
    assert completed.stdout.strip() == "runtime-data/runtime_event_transports/sqlite-session.json"


def test_resolver_falls_back_to_binding_artifact_when_manifest_is_missing(tmp_path: Path) -> None:
    search_root = tmp_path / "scratch"
    binding_path = search_root / "run-a" / "data" / "runtime_event_transports" / "session__job.json"
    _write_json(
        binding_path,
        {
            "schema_version": "runtime-event-transport-v1",
            "binding_artifact_path": str(binding_path),
            "binding_backend_family": "sqlite",
        },
    )

    completed = _run_resolver_cli(search_root)
    assert completed.returncode == 0
    assert completed.stdout.strip() == str(binding_path)


def test_resolver_prefers_manifest_candidates_over_binding_candidates_when_manifest_has_timestamp(
    tmp_path: Path,
) -> None:
    search_root = tmp_path / "scratch"
    manifest_path = search_root / "manifest" / "TRACE_RESUME_MANIFEST.json"
    binding_path = search_root / "binding" / "data" / "runtime_event_transports" / "session__job.json"

    _write_json(
        manifest_path,
        {
            "schema_version": "runtime-resume-manifest-v1",
            "event_transport_path": "/tmp/runtime_event_transports/from-manifest.json",
            "updated_at": "2026-04-23T00:00:01+00:00",
        },
    )
    _write_json(
        binding_path,
        {
            "schema_version": "runtime-event-transport-v1",
            "binding_artifact_path": "/tmp/runtime_event_transports/from-binding.json",
            "binding_backend_family": "filesystem",
        },
    )
    os.utime(binding_path, (1_800_000_000, 1_800_000_000))

    completed = _run_resolver_cli(search_root)
    assert completed.returncode == 0
    assert completed.stdout.strip() == str(manifest_path.resolve())


def test_resolver_ignores_invalid_payloads_and_keeps_valid_binding_fallback(tmp_path: Path) -> None:
    search_root = tmp_path / "scratch"
    invalid_manifest = search_root / "broken" / "TRACE_RESUME_MANIFEST.json"
    invalid_manifest.parent.mkdir(parents=True, exist_ok=True)
    invalid_manifest.write_text("{not-json\n", encoding="utf-8")

    _write_json(
        search_root / "missing-path" / "TRACE_RESUME_MANIFEST.json",
        {
            "schema_version": "runtime-resume-manifest-v1",
            "updated_at": "2026-04-23T00:20:00+00:00",
        },
    )

    binding_path = search_root / "run-b" / "data" / "runtime_event_transports" / "session__job.json"
    _write_json(
        binding_path,
        {
            "schema_version": "runtime-event-transport-v1",
            "binding_backend_family": "sqlite",
        },
    )

    completed = _run_resolver_cli(search_root)
    assert completed.returncode == 0
    assert completed.stdout.strip() == str(binding_path)


def test_resolver_reads_sqlite_binding_payload_without_explicit_binding_path(tmp_path: Path) -> None:
    search_root = tmp_path / "scratch"
    db_path = search_root / "sqlite-run" / "runtime_checkpoint_store.sqlite3"
    db_path.parent.mkdir(parents=True, exist_ok=True)

    connection = sqlite3.connect(db_path)
    connection.execute(
        "CREATE TABLE runtime_storage_payloads (payload_key TEXT PRIMARY KEY, payload_text TEXT NOT NULL)"
    )
    connection.execute(
        "INSERT INTO runtime_storage_payloads (payload_key, payload_text) VALUES (?, ?)",
        (
            "runtime-data/runtime_event_transports/session__job.json",
            json.dumps(
                {
                    "schema_version": "runtime-event-transport-v1",
                    "binding_backend_family": "sqlite",
                }
            ),
        ),
    )
    connection.commit()
    connection.close()

    completed = _run_resolver_cli(search_root)
    assert completed.returncode == 0
    assert completed.stdout.strip() == "runtime-data/runtime_event_transports/session__job.json"


def test_resolver_returns_none_when_no_attach_candidates_exist(tmp_path: Path) -> None:
    search_root = tmp_path / "scratch"
    search_root.mkdir(parents=True, exist_ok=True)

    completed = _run_resolver_cli(search_root)
    assert completed.returncode == 1
    assert completed.stdout == ""


def test_resolver_ignores_sqlite_query_failures_and_uses_filesystem_fallback(tmp_path: Path) -> None:
    search_root = tmp_path / "scratch"
    db_path = search_root / "sqlite-run" / "runtime_checkpoint_store.sqlite3"
    db_path.parent.mkdir(parents=True, exist_ok=True)
    db_path.write_text("not-a-real-sqlite-db", encoding="utf-8")

    binding_path = search_root / "good" / "data" / "runtime_event_transports" / "session__job.json"
    _write_json(
        binding_path,
        {
            "schema_version": "runtime-event-transport-v1",
            "binding_artifact_path": str(binding_path),
            "binding_backend_family": "sqlite",
        },
    )

    completed = _run_resolver_cli(search_root)
    assert completed.returncode == 0
    assert completed.stdout.strip() == str(binding_path)


def test_resolver_cli_prints_resolved_attach_path_on_success(tmp_path: Path) -> None:
    search_root = tmp_path / "scratch"
    binding_path = search_root / "run-a" / "data" / "runtime_event_transports" / "session__job.json"
    _write_json(
        binding_path,
        {
            "schema_version": "runtime-event-transport-v1",
            "binding_artifact_path": str(binding_path),
            "binding_backend_family": "sqlite",
        },
    )

    completed = _run_resolver_cli(search_root)

    assert completed.returncode == 0
    assert completed.stdout.strip() == str(binding_path)
    assert completed.stderr == ""


def test_resolver_ignores_bare_filesystem_binding_without_replay_manifest(tmp_path: Path) -> None:
    search_root = tmp_path / "scratch"
    binding_path = search_root / "run-a" / "data" / "runtime_event_transports" / "session__job.json"
    _write_json(
        binding_path,
        {
            "schema_version": "runtime-event-transport-v1",
            "binding_artifact_path": str(binding_path),
            "binding_backend_family": "filesystem",
        },
    )

    completed = _run_resolver_cli(search_root)

    assert completed.returncode == 1
    assert completed.stdout == ""


def test_resolver_cli_exits_nonzero_when_no_candidates_exist(tmp_path: Path) -> None:
    search_root = tmp_path / "scratch"
    search_root.mkdir(parents=True, exist_ok=True)

    completed = _run_resolver_cli(search_root)

    assert completed.returncode == 1
    assert completed.stdout == ""
