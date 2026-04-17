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

from codex_agno_runtime.checkpoint_store import FilesystemRuntimeCheckpointer
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
    assert store.health()["runtime_control_plane_schema_version"] == "router-rs-runtime-control-plane-v1"

    recovered = BackgroundJobStore(state_path=state_path)
    assert recovered.control_plane_descriptor().projection == "python-thin-projection"
    assert recovered.control_plane_descriptor().delegate_kind == "filesystem-state-store"


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

    persisted_manifest = json.loads(
        checkpointer.describe_paths().resume_manifest_path.read_text(encoding="utf-8")
    )
    assert persisted_manifest["control_plane"]["runtime_control_plane_authority"] == "rust-runtime-control-plane"
