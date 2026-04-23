"""Direct control-plane contract coverage for state/checkpoint thin hosts."""

from __future__ import annotations

import json
import sqlite3
import sys
from pathlib import Path

import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = PROJECT_ROOT / "framework_runtime" / "src"
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

from framework_runtime.checkpoint_store import (
    FilesystemRuntimeCheckpointer,
    InMemoryRuntimeStorageBackend,
    SQLiteRuntimeStorageBackend,
    select_runtime_storage_backend,
)
from framework_runtime.state import BackgroundJobStore
from framework_runtime.trace import RuntimeEventTransport


CONTROL_PLANE_DESCRIPTOR = {
    "schema_version": "router-rs-runtime-control-plane-v1",
    "authority": "rust-runtime-control-plane",
    "services": {
        "state": {
            "authority": "rust-runtime-control-plane",
            "role": "durable-background-state",
            "projection": "python-thin-projection",
            "delegate_kind": "filesystem-state-store",
        },
        "trace": {
            "authority": "rust-runtime-control-plane",
            "role": "trace-and-handoff",
            "projection": "python-thin-projection",
            "delegate_kind": "filesystem-trace-store",
        },
    },
}


def test_background_state_persists_control_plane_descriptor(tmp_path: Path) -> None:
    """Background job state should persist and recover the Rust-owned projection."""

    state_path = tmp_path / "runtime_background_jobs.json"
    store = BackgroundJobStore(
        state_path=state_path,
        control_plane_descriptor=CONTROL_PLANE_DESCRIPTOR,
    )
    store.set_status("job-1", status="queued", session_id="session-1", timeout_seconds=30)

    payload = json.loads(state_path.read_text(encoding="utf-8"))
    assert payload["control_plane"]["authority"] == "rust-runtime-control-plane"
    assert payload["control_plane"]["projection"] == "python-thin-projection"
    assert payload["control_plane"]["delegate_kind"] == "filesystem-state-store"
    assert payload["control_plane"]["supports_atomic_replace"] is True
    assert payload["control_plane"]["supports_compaction"] is False
    assert payload["control_plane"]["supports_snapshot_delta"] is False
    assert payload["control_plane"]["supports_remote_event_transport"] is True
    assert store.health()["runtime_control_plane_schema_version"] == "router-rs-runtime-control-plane-v1"
    assert store.health()["supports_atomic_replace"] is True
    assert store.health()["supports_compaction"] is False
    assert store.health()["supports_snapshot_delta"] is False
    assert store.health()["supports_remote_event_transport"] is True

    recovered = BackgroundJobStore(state_path=state_path)
    assert recovered.control_plane_descriptor().projection == "python-thin-projection"
    assert recovered.control_plane_descriptor().delegate_kind == "filesystem-state-store"
    assert recovered.control_plane_descriptor().supports_atomic_replace is True
    assert recovered.control_plane_descriptor().supports_snapshot_delta is False


def test_checkpointer_embeds_control_plane_into_manifest_and_transport(tmp_path: Path) -> None:
    """Checkpoint artifacts should carry Rust-owned authority/projection metadata."""

    checkpointer = FilesystemRuntimeCheckpointer(
        data_dir=tmp_path / "runtime-data",
        trace_output_path=tmp_path / "runtime-data" / "TRACE_METADATA.json",
        control_plane_descriptor=CONTROL_PLANE_DESCRIPTOR,
    )
    transport = RuntimeEventTransport(
        stream_id="stream::session-1",
        session_id="session-1",
        binding_backend_family="filesystem",
    )

    binding_path = checkpointer.write_transport_binding(transport)
    assert binding_path is not None
    binding_payload = json.loads(binding_path.read_text(encoding="utf-8"))
    assert binding_payload["control_plane_authority"] == "rust-runtime-control-plane"
    assert binding_payload["control_plane_projection"] == "python-thin-projection"
    assert binding_payload["transport_health"]["backend_family"] == "filesystem"
    assert binding_payload["transport_health"]["supports_atomic_replace"] is True
    assert binding_payload["transport_health"]["supports_compaction"] is False
    assert binding_payload["transport_health"]["supports_snapshot_delta"] is False

    manifest = checkpointer.checkpoint(
        session_id="session-1",
        job_id="job-1",
        status="completed",
        generation=0,
        latest_cursor=None,
        event_transport_path=str(binding_path),
        artifact_paths=[str(binding_path)],
    )
    assert manifest is not None
    assert manifest.control_plane is not None
    assert manifest.control_plane["trace_service"]["projection"] == "python-thin-projection"
    assert manifest.control_plane["state_service"]["delegate_kind"] == "filesystem-state-store"
    assert manifest.control_plane["supports_atomic_replace"] is True
    assert manifest.control_plane["supports_compaction"] is False
    assert manifest.control_plane["supports_snapshot_delta"] is False
    assert manifest.control_plane["supports_remote_event_transport"] is True

    persisted_manifest = json.loads(
        checkpointer.describe_paths().resume_manifest_path.read_text(encoding="utf-8")
    )
    assert persisted_manifest["control_plane"]["runtime_control_plane_authority"] == "rust-runtime-control-plane"
    assert persisted_manifest["control_plane"]["supports_snapshot_delta"] is False


def test_checkpoint_manifest_can_carry_parallel_group_summary(tmp_path: Path) -> None:
    """Resume checkpoints should preserve durable parallel-batch summaries."""

    checkpointer = FilesystemRuntimeCheckpointer(
        data_dir=tmp_path / "runtime-data",
        trace_output_path=tmp_path / "runtime-data" / "TRACE_METADATA.json",
        control_plane_descriptor=CONTROL_PLANE_DESCRIPTOR,
    )

    manifest = checkpointer.checkpoint(
        session_id="session-1",
        job_id="job-1",
        status="running",
        generation=3,
        latest_cursor=None,
        event_transport_path=None,
        artifact_paths=["/tmp/example.json"],
        parallel_group={
            "parallel_group_id": "pgroup_123",
            "job_ids": ["job-1", "job-2"],
            "session_ids": ["session-1", "session-2"],
            "lane_ids": ["lane-1", "lane-2"],
            "parent_job_ids": [],
            "status_counts": {"queued": 1, "running": 1},
            "active_job_count": 2,
            "terminal_job_count": 0,
            "total_job_count": 2,
            "latest_updated_at": "2026-04-22T12:00:00+00:00",
        },
    )

    assert manifest is not None
    assert manifest.parallel_group is not None
    assert manifest.parallel_group.parallel_group_id == "pgroup_123"
    assert manifest.parallel_group.status_counts == {"queued": 1, "running": 1}

    loaded = checkpointer.load_checkpoint()
    assert loaded is not None
    assert loaded.parallel_group is not None
    assert loaded.parallel_group.lane_ids == ["lane-1", "lane-2"]
    assert loaded.parallel_group.total_job_count == 2


def test_checkpointer_manifest_compilers_fail_closed_on_rust_errors(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """Transport, handoff, and resume manifests should not silently fall back to Python."""

    checkpointer = FilesystemRuntimeCheckpointer(
        data_dir=tmp_path / "runtime-data",
        trace_output_path=tmp_path / "runtime-data" / "TRACE_METADATA.json",
        control_plane_descriptor=CONTROL_PLANE_DESCRIPTOR,
    )
    transport = RuntimeEventTransport(
        stream_id="stream::session-1",
        session_id="session-1",
        job_id="job-1",
        binding_backend_family="filesystem",
    )

    monkeypatch.setattr(
        checkpointer._rust_adapter,
        "describe_transport",
        lambda payload: (_ for _ in ()).throw(RuntimeError("transport compiler drift")),
    )
    with pytest.raises(RuntimeError, match="transport compiler drift"):
        checkpointer.resolve_transport_manifest(
            session_id="session-1",
            job_id="job-1",
            latest_cursor=None,
        )

    monkeypatch.setattr(
        checkpointer._rust_adapter,
        "describe_handoff",
        lambda payload: (_ for _ in ()).throw(RuntimeError("handoff compiler drift")),
    )
    with pytest.raises(RuntimeError, match="handoff compiler drift"):
        checkpointer.resolve_handoff_manifest(
            session_id="session-1",
            job_id="job-1",
            transport=transport,
        )

    monkeypatch.setattr(
        checkpointer._rust_adapter,
        "checkpoint_resume_manifest",
        lambda payload: (_ for _ in ()).throw(RuntimeError("resume compiler drift")),
    )
    with pytest.raises(RuntimeError, match="resume compiler drift"):
        checkpointer.resolve_resume_manifest(
            session_id="session-1",
            job_id="job-1",
            status="running",
            generation=1,
            latest_cursor=None,
            event_transport_path=None,
            artifact_paths=[],
        )


def test_checkpointer_uses_rust_writers_only_for_filesystem_backend(monkeypatch, tmp_path: Path) -> None:
    """Filesystem persistence should prefer Rust writers and keep the same artifact paths."""

    checkpointer = FilesystemRuntimeCheckpointer(
        data_dir=tmp_path / "runtime-data",
        trace_output_path=tmp_path / "runtime-data" / "TRACE_METADATA.json",
        control_plane_descriptor=CONTROL_PLANE_DESCRIPTOR,
    )
    calls: list[tuple[str, str]] = []

    def fake_write_transport_binding(payload: dict[str, object]) -> dict[str, object]:
        path = Path(str(payload["path"]))
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps({"path": str(path), "via": "rust"}) + "\n", encoding="utf-8")
        calls.append(("transport", str(path)))
        return {
            "schema_version": checkpointer._rust_adapter.transport_binding_write_schema_version,
            "authority": checkpointer._rust_adapter.transport_binding_write_authority,
            "path": str(path),
            "bytes_written": path.stat().st_size,
        }

    def fake_write_resume_manifest(payload: dict[str, object]) -> dict[str, object]:
        path = Path(str(payload["path"]))
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps({"path": str(path), "via": "rust"}) + "\n", encoding="utf-8")
        calls.append(("manifest", str(path)))
        return {
            "schema_version": checkpointer._rust_adapter.checkpoint_manifest_write_schema_version,
            "authority": checkpointer._rust_adapter.checkpoint_manifest_write_authority,
            "path": str(path),
            "bytes_written": path.stat().st_size,
        }

    monkeypatch.setattr(checkpointer._rust_adapter, "write_transport_binding", fake_write_transport_binding)
    monkeypatch.setattr(checkpointer._rust_adapter, "write_checkpoint_resume_manifest", fake_write_resume_manifest)

    transport = RuntimeEventTransport(
        stream_id="stream::session-1",
        session_id="session-1",
        job_id="job-1",
        binding_backend_family="filesystem",
    )
    binding_path = checkpointer.write_transport_binding(transport)
    manifest = checkpointer.checkpoint(
        session_id="session-1",
        job_id="job-1",
        status="completed",
        generation=1,
        latest_cursor=None,
        event_transport_path=str(binding_path) if binding_path is not None else None,
        artifact_paths=[str(binding_path)] if binding_path is not None else [],
    )

    assert binding_path is not None
    assert manifest is not None
    assert calls == [
        ("transport", str(binding_path)),
        ("manifest", str(checkpointer.describe_paths().resume_manifest_path)),
    ]
    assert json.loads(binding_path.read_text(encoding="utf-8"))["via"] == "rust"
    assert json.loads(checkpointer.describe_paths().resume_manifest_path.read_text(encoding="utf-8"))["via"] == "rust"


def test_filesystem_backend_fails_closed_when_rust_writers_error(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """Filesystem-backed persistence should stop instead of falling back to Python writes."""

    checkpointer = FilesystemRuntimeCheckpointer(
        data_dir=tmp_path / "runtime-data",
        trace_output_path=tmp_path / "runtime-data" / "TRACE_METADATA.json",
        control_plane_descriptor=CONTROL_PLANE_DESCRIPTOR,
    )
    transport = RuntimeEventTransport(
        stream_id="stream::session-1",
        session_id="session-1",
        job_id="job-1",
        binding_backend_family="filesystem",
    )

    monkeypatch.setattr(
        checkpointer._rust_adapter,
        "write_transport_binding",
        lambda payload: (_ for _ in ()).throw(RuntimeError("transport writer drift")),
    )
    with pytest.raises(RuntimeError, match="transport writer drift"):
        checkpointer.write_transport_binding(transport)

    binding_path = checkpointer.transport_binding_path(session_id="session-1", job_id="job-1")
    assert binding_path is not None
    assert not binding_path.exists()

    def fake_write_transport_binding(payload: dict[str, object]) -> dict[str, object]:
        path = Path(str(payload["path"]))
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps({"path": str(path), "via": "rust"}) + "\n", encoding="utf-8")
        return {
            "schema_version": checkpointer._rust_adapter.transport_binding_write_schema_version,
            "authority": checkpointer._rust_adapter.transport_binding_write_authority,
            "path": str(path),
            "bytes_written": path.stat().st_size,
        }

    monkeypatch.setattr(checkpointer._rust_adapter, "write_transport_binding", fake_write_transport_binding)
    binding_path = checkpointer.write_transport_binding(transport)
    assert binding_path is not None
    assert binding_path.exists()

    monkeypatch.setattr(
        checkpointer._rust_adapter,
        "write_checkpoint_resume_manifest",
        lambda payload: (_ for _ in ()).throw(RuntimeError("manifest writer drift")),
    )
    with pytest.raises(RuntimeError, match="manifest writer drift"):
        checkpointer.checkpoint(
            session_id="session-1",
            job_id="job-1",
            status="completed",
            generation=1,
            latest_cursor=None,
            event_transport_path=str(binding_path),
            artifact_paths=[str(binding_path)],
        )

    resume_manifest_path = checkpointer.describe_paths().resume_manifest_path
    assert resume_manifest_path is not None
    assert not resume_manifest_path.exists()


def test_background_state_uses_non_filesystem_backend_family(tmp_path: Path) -> None:
    """Background state should persist through the backend seam without filesystem writes."""

    backend = InMemoryRuntimeStorageBackend()
    state_path = tmp_path / "runtime_background_jobs.json"
    store = BackgroundJobStore(
        state_path=state_path,
        storage_backend=backend,
        control_plane_descriptor=CONTROL_PLANE_DESCRIPTOR,
    )

    store.set_status("job-1", status="queued", session_id="session-1", timeout_seconds=30)

    assert backend.exists(state_path)
    assert not state_path.exists()
    payload = json.loads(backend.read_text(state_path))
    assert payload["control_plane"]["backend_family"] == "memory"
    assert payload["control_plane"]["delegate_kind"] == "memory-state-store"
    assert payload["control_plane"]["supports_atomic_replace"] is False
    assert payload["control_plane"]["supports_compaction"] is False
    assert payload["control_plane"]["supports_snapshot_delta"] is False
    assert payload["control_plane"]["supports_remote_event_transport"] is True
    assert payload["jobs"][0]["job_id"] == "job-1"

    recovered = BackgroundJobStore(
        state_path=state_path,
        storage_backend=backend,
    )
    recovered_row = recovered.get("job-1")
    assert recovered_row is not None
    assert recovered_row.status == "queued"
    assert recovered.health()["backend_family"] == "memory"
    assert recovered.health()["control_plane_delegate_kind"] == "memory-state-store"
    assert recovered.health()["supports_atomic_replace"] is False
    assert recovered.health()["supports_compaction"] is False
    assert recovered.health()["supports_snapshot_delta"] is False
    assert recovered.health()["supports_remote_event_transport"] is True
    assert recovered.get_active_job("session-1") == "job-1"


def test_checkpointer_uses_non_filesystem_backend_family(tmp_path: Path) -> None:
    """Checkpoint manifests and transport bindings should round-trip through the backend seam."""

    backend = InMemoryRuntimeStorageBackend()
    checkpointer = FilesystemRuntimeCheckpointer(
        data_dir=tmp_path / "runtime-data",
        trace_output_path=tmp_path / "runtime-data" / "TRACE_METADATA.json",
        storage_backend=backend,
        control_plane_descriptor=CONTROL_PLANE_DESCRIPTOR,
    )
    transport = RuntimeEventTransport(
        stream_id="stream::session-1",
        session_id="session-1",
        job_id="job-1",
        binding_backend_family="memory",
    )

    binding_path = checkpointer.write_transport_binding(transport)
    manifest = checkpointer.checkpoint(
        session_id="session-1",
        job_id="job-1",
        status="completed",
        generation=1,
        latest_cursor=None,
        event_transport_path=str(binding_path) if binding_path is not None else None,
        artifact_paths=["/tmp/example.json"],
    )
    loaded = checkpointer.load_checkpoint()
    paths = checkpointer.describe_paths()

    assert binding_path is not None
    assert manifest is not None
    assert loaded is not None
    assert backend.exists(binding_path)
    assert backend.exists(paths.resume_manifest_path)
    assert not binding_path.exists()
    assert not paths.resume_manifest_path.exists()
    assert loaded.session_id == "session-1"
    assert loaded.job_id == "job-1"
    assert loaded.control_plane is not None
    assert loaded.control_plane["backend_family"] == "memory"
    assert loaded.control_plane["trace_service"]["delegate_kind"] == "memory-trace-store"
    assert loaded.control_plane["state_service"]["delegate_kind"] == "memory-state-store"
    assert loaded.control_plane["supports_atomic_replace"] is False
    assert loaded.control_plane["supports_compaction"] is False
    assert loaded.control_plane["supports_snapshot_delta"] is False
    assert loaded.control_plane["supports_remote_event_transport"] is True
    assert checkpointer.health()["backend_family"] == "memory"
    assert checkpointer.health()["supports_atomic_replace"] is False
    assert checkpointer.health()["supports_compaction"] is False
    assert checkpointer.health()["supports_snapshot_delta"] is False
    assert checkpointer.health()["supports_remote_event_transport"] is True


def test_non_filesystem_backend_skips_rust_writers(monkeypatch, tmp_path: Path) -> None:
    """Memory-backed checkpoints should stay on the backend seam and never invoke Rust file writers."""

    backend = InMemoryRuntimeStorageBackend()
    checkpointer = FilesystemRuntimeCheckpointer(
        data_dir=tmp_path / "runtime-data",
        trace_output_path=tmp_path / "runtime-data" / "TRACE_METADATA.json",
        storage_backend=backend,
        control_plane_descriptor=CONTROL_PLANE_DESCRIPTOR,
    )

    def fail_write_transport_binding(payload: dict[str, object]) -> dict[str, object]:
        raise AssertionError(f"unexpected rust transport write: {payload}")

    def fail_write_resume_manifest(payload: dict[str, object]) -> dict[str, object]:
        raise AssertionError(f"unexpected rust manifest write: {payload}")

    monkeypatch.setattr(checkpointer._rust_adapter, "write_transport_binding", fail_write_transport_binding)
    monkeypatch.setattr(checkpointer._rust_adapter, "write_checkpoint_resume_manifest", fail_write_resume_manifest)

    transport = RuntimeEventTransport(
        stream_id="stream::session-1",
        session_id="session-1",
        job_id="job-1",
        binding_backend_family="memory",
    )
    binding_path = checkpointer.write_transport_binding(transport)
    manifest = checkpointer.checkpoint(
        session_id="session-1",
        job_id="job-1",
        status="completed",
        generation=1,
        latest_cursor=None,
        event_transport_path=str(binding_path) if binding_path is not None else None,
        artifact_paths=["/tmp/example.json"],
    )

    assert binding_path is not None
    assert manifest is not None
    assert backend.exists(binding_path)
    assert backend.exists(checkpointer.describe_paths().resume_manifest_path)


def test_sqlite_backend_family_can_be_selected_and_round_tripped_via_config(
    monkeypatch,
    tmp_path: Path,
) -> None:
    """The runtime should be able to promote sqlite to a concrete non-filesystem backend."""

    monkeypatch.setenv("CODEX_AGNO_CHECKPOINT_STORAGE_BACKEND_FAMILY", "sqlite")
    monkeypatch.setenv("CODEX_AGNO_CHECKPOINT_STORAGE_DB_FILE", "runtime_checkpoint_store.sqlite3")

    data_dir = tmp_path / "runtime-data"
    state_path = data_dir / "runtime_background_jobs.json"
    checkpointer = FilesystemRuntimeCheckpointer(
        data_dir=data_dir,
        trace_output_path=data_dir / "TRACE_METADATA.json",
        control_plane_descriptor=CONTROL_PLANE_DESCRIPTOR,
    )
    store = BackgroundJobStore(
        state_path=state_path,
        control_plane_descriptor=CONTROL_PLANE_DESCRIPTOR,
    )

    assert isinstance(checkpointer.storage_backend, SQLiteRuntimeStorageBackend)
    assert checkpointer.health()["backend_family"] == "sqlite"
    assert checkpointer.health()["supports_atomic_replace"] is True
    assert checkpointer.health()["supports_compaction"] is True
    assert checkpointer.health()["supports_snapshot_delta"] is True
    assert checkpointer.health()["supports_remote_event_transport"] is True

    store.set_status("job-1", status="queued", session_id="session-1", timeout_seconds=30)
    transport = RuntimeEventTransport(
        stream_id="stream::session-1",
        session_id="session-1",
        job_id="job-1",
        binding_backend_family=checkpointer.storage_capabilities().backend_family,
    )
    binding_path = checkpointer.write_transport_binding(transport)
    manifest = checkpointer.checkpoint(
        session_id="session-1",
        job_id="job-1",
        status="completed",
        generation=1,
        latest_cursor=None,
        event_transport_path=str(binding_path) if binding_path is not None else None,
        artifact_paths=[str(binding_path)] if binding_path is not None else [],
    )
    loaded = checkpointer.load_checkpoint()
    recovered = BackgroundJobStore(
        state_path=state_path,
        control_plane_descriptor=CONTROL_PLANE_DESCRIPTOR,
    )
    reopened = FilesystemRuntimeCheckpointer(
        data_dir=data_dir,
        trace_output_path=data_dir / "TRACE_METADATA.json",
        control_plane_descriptor=CONTROL_PLANE_DESCRIPTOR,
    )
    reopened_loaded = reopened.load_checkpoint()

    assert binding_path is not None
    assert manifest is not None
    assert loaded is not None
    assert reopened_loaded is not None
    assert recovered.get("job-1") is not None
    assert recovered.get("job-1").status == "queued"
    assert recovered.health()["backend_family"] == "sqlite"
    assert recovered.health()["supports_atomic_replace"] is True
    assert recovered.health()["supports_compaction"] is True
    assert recovered.health()["supports_snapshot_delta"] is True
    assert recovered.health()["supports_remote_event_transport"] is True
    assert state_path.exists() is False
    assert checkpointer.describe_paths().resume_manifest_path is not None
    assert checkpointer.describe_paths().resume_manifest_path.exists() is False
    assert checkpointer.describe_paths().event_stream_path is not None
    assert checkpointer.describe_paths().event_stream_path.exists() is False
    assert binding_path.exists() is False
    assert (
        data_dir / "runtime_checkpoint_store.sqlite3"
    ).exists(), "sqlite backend should materialize a concrete backing store file"

    binding_payload = json.loads(checkpointer.storage_backend.read_text(binding_path))
    assert binding_payload["control_plane_delegate_kind"] == "sqlite-trace-store"
    assert binding_payload["transport_health"]["backend_family"] == "sqlite"
    assert binding_payload["transport_health"]["supports_atomic_replace"] is True
    assert binding_payload["transport_health"]["supports_compaction"] is True
    assert binding_payload["transport_health"]["supports_snapshot_delta"] is True
    assert binding_payload["transport_health"]["supports_remote_event_transport"] is True
    assert loaded.control_plane is not None
    assert loaded.control_plane["backend_family"] == "sqlite"
    assert loaded.control_plane["trace_service"]["delegate_kind"] == "sqlite-trace-store"
    assert loaded.control_plane["state_service"]["delegate_kind"] == "sqlite-state-store"
    assert loaded.control_plane["supports_atomic_replace"] is True
    assert loaded.control_plane["supports_compaction"] is True
    assert loaded.control_plane["supports_snapshot_delta"] is True
    assert loaded.control_plane["supports_remote_event_transport"] is True
    assert manifest.control_plane is not None
    assert manifest.control_plane["backend_family"] == "sqlite"
    assert manifest.control_plane["trace_service"]["delegate_kind"] == "sqlite-trace-store"
    assert manifest.control_plane["state_service"]["delegate_kind"] == "sqlite-state-store"
    assert manifest.control_plane["supports_compaction"] is True
    assert manifest.control_plane["supports_snapshot_delta"] is True
    assert reopened_loaded.session_id == "session-1"
    assert reopened_loaded.job_id == "job-1"


def test_sqlite_backend_supports_trace_compaction_snapshot_delta(tmp_path: Path) -> None:
    """SQLite-backed trace artifacts should support compaction and snapshot-delta recovery."""

    data_dir = tmp_path / "runtime-data"
    backend = SQLiteRuntimeStorageBackend(
        db_path=data_dir / "runtime_checkpoint_store.sqlite3",
        storage_root=data_dir,
    )
    checkpointer = FilesystemRuntimeCheckpointer(
        data_dir=data_dir,
        trace_output_path=data_dir / "TRACE_METADATA.json",
        storage_backend=backend,
        control_plane_descriptor=CONTROL_PLANE_DESCRIPTOR,
    )
    recorder = checkpointer.build_trace_recorder()

    recorder.record(
        session_id="session-compact",
        job_id="job-compact",
        kind="job.started",
        stage="background",
        payload={"step": 1},
    )
    recorder.record(
        session_id="session-compact",
        job_id="job-compact",
        kind="job.progress",
        stage="background",
        payload={"step": 2},
    )

    compaction = recorder.compact(
        session_id="session-compact",
        job_id="job-compact",
        artifact_paths=[str(checkpointer.describe_paths().background_state_path)],
    )
    assert compaction.applied is True
    assert compaction.status == "compacted"
    assert compaction.backend_family == "sqlite"

    manifest = recorder.load_compaction_manifest(session_id="session-compact", job_id="job-compact")
    assert manifest is not None
    assert manifest.compaction_supported is True
    assert manifest.snapshot_delta_supported is True

    next_event = recorder.record(
        session_id="session-compact",
        job_id="job-compact",
        kind="job.completed",
        stage="background",
        payload={"step": 3},
    )
    assert next_event.generation == 1
    assert next_event.seq == 1

    recovery = recorder.recover_compacted_state(session_id="session-compact", job_id="job-compact")
    assert recovery is not None
    assert recovery.snapshot.generation == 0
    assert recovery.latest_recoverable_generation == 1
    assert recovery.latest_cursor is not None
    assert recovery.latest_cursor.generation == 1
    assert recovery.latest_cursor.seq == 1
    assert [delta.kind for delta in recovery.deltas] == ["job.completed"]


def test_sqlite_backend_reads_legacy_absolute_path_keys(tmp_path: Path) -> None:
    """SQLite backend should keep reading legacy absolute-path payload keys."""

    data_dir = tmp_path / "runtime-data"
    db_path = data_dir / "runtime_checkpoint_store.sqlite3"
    state_path = data_dir / "runtime_background_jobs.json"
    backend = SQLiteRuntimeStorageBackend(db_path=db_path, storage_root=data_dir)
    legacy_key = str(state_path.expanduser().resolve())
    payload = '{"schema_version":"runtime-background-state-v4","version":2,"control_plane":null,"jobs":[],"active_sessions":[],"pending_session_takeovers":[]}\n'

    db_path.parent.mkdir(parents=True, exist_ok=True)
    with sqlite3.connect(db_path, timeout=30.0) as conn:
        conn.execute(
            "CREATE TABLE IF NOT EXISTS runtime_storage_payloads (payload_key TEXT PRIMARY KEY, payload_text TEXT NOT NULL)"
        )
        conn.execute(
            "INSERT INTO runtime_storage_payloads (payload_key, payload_text) VALUES (?, ?)",
            (legacy_key, payload),
        )
        conn.commit()

    assert backend.exists(state_path) is True
    assert backend.read_text(state_path) == payload



def test_sqlite_backend_rejects_paths_outside_storage_root(tmp_path: Path) -> None:
    """SQLite backend should fail closed for paths outside the configured storage root."""

    data_dir = tmp_path / "runtime-data"
    backend = SQLiteRuntimeStorageBackend(
        db_path=data_dir / "runtime_checkpoint_store.sqlite3",
        storage_root=data_dir,
    )

    with pytest.raises(ValueError, match="must stay under storage root"):
        backend.write_text(tmp_path / "outside.json", "{}")
