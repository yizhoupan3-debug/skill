"""Direct control-plane contract coverage for state/checkpoint thin hosts."""

from __future__ import annotations

import json
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = PROJECT_ROOT / "codex_agno_runtime" / "src"
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

from codex_agno_runtime.checkpoint_store import (
    FilesystemRuntimeCheckpointer,
    InMemoryRuntimeStorageBackend,
    SQLiteRuntimeStorageBackend,
)
from codex_agno_runtime.state import BackgroundJobStore
from codex_agno_runtime.trace import RuntimeEventTransport


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
    assert loaded.control_plane["supports_atomic_replace"] is False
    assert loaded.control_plane["supports_compaction"] is False
    assert loaded.control_plane["supports_snapshot_delta"] is False
    assert loaded.control_plane["supports_remote_event_transport"] is True
    assert checkpointer.health()["backend_family"] == "memory"
    assert checkpointer.health()["supports_atomic_replace"] is False
    assert checkpointer.health()["supports_compaction"] is False
    assert checkpointer.health()["supports_snapshot_delta"] is False
    assert checkpointer.health()["supports_remote_event_transport"] is True


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
    assert checkpointer.health()["supports_compaction"] is False
    assert checkpointer.health()["supports_snapshot_delta"] is False
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

    assert binding_path is not None
    assert manifest is not None
    assert loaded is not None
    assert recovered.get("job-1") is not None
    assert recovered.get("job-1").status == "queued"
    assert recovered.health()["backend_family"] == "sqlite"
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
    assert binding_payload["transport_health"]["backend_family"] == "sqlite"
    assert binding_payload["transport_health"]["supports_atomic_replace"] is True
    assert binding_payload["transport_health"]["supports_compaction"] is False
    assert binding_payload["transport_health"]["supports_snapshot_delta"] is False
    assert binding_payload["transport_health"]["supports_remote_event_transport"] is True
    assert loaded.control_plane is not None
    assert loaded.control_plane["backend_family"] == "sqlite"
    assert loaded.control_plane["supports_atomic_replace"] is True
    assert loaded.control_plane["supports_compaction"] is False
    assert loaded.control_plane["supports_snapshot_delta"] is False
    assert loaded.control_plane["supports_remote_event_transport"] is True
    assert manifest.control_plane is not None
    assert manifest.control_plane["backend_family"] == "sqlite"
