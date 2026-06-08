//! Roadmap v5 §2.3 P0 / §6.2: independent `trace_runtime` compaction smoke (module-local).

use crate::trace_runtime::{
    compact_trace_stream, record_trace_event, TraceCompactRequestPayload,
    TraceRecordEventRequestPayload,
};
use serde_json::{json, Map};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_trace_root(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("router-rs-trace-{name}-{nonce}"))
}

fn base_compact_payload(root: &PathBuf, session_id: &str) -> TraceCompactRequestPayload {
    TraceCompactRequestPayload {
        root_path: root.display().to_string(),
        event_stream_path: None,
        output_path: None,
        session_id: session_id.to_string(),
        job_id: None,
        backend_family: Some("filesystem".to_string()),
        supports_compaction: true,
        supports_snapshot_delta: true,
        current_generation: 0,
        artifact_paths: Vec::new(),
        event_stream_text: None,
        output_text: None,
        previous_manifest_text: None,
        write_outputs: false,
    }
}

#[test]
fn compact_trace_stream_unsupported_backend_smoke() {
    let root = temp_trace_root("unsupported");
    let mut payload = base_compact_payload(&root, "sess-unsupported");
    payload.supports_compaction = false;

    let response = compact_trace_stream(payload).expect("compact");
    assert!(!response.applied);
    assert_eq!(response.status, "unsupported");
    assert!(response.reason.is_some());
}

#[test]
fn compact_trace_stream_no_matching_events_smoke() {
    let root = temp_trace_root("no-events");
    let mut payload = base_compact_payload(&root, "empty-session");
    payload.event_stream_text = Some(String::new());

    let response = compact_trace_stream(payload).expect("compact");
    assert!(!response.applied);
    assert_eq!(response.status, "no_events");
}

#[test]
fn compact_trace_stream_applies_snapshot_smoke() {
    let root = temp_trace_root("apply");
    let stream = concat!(
        r#"{"event_id":"evt-compact-1","seq":1,"generation":0,"session_id":"cmp-sess","#,
        r#""kind":"job.started","stage":"background","status":"ok","cursor":"g0:s1:evt-compact-1"}"#
    );
    let mut payload = base_compact_payload(&root, "cmp-sess");
    payload.event_stream_text = Some(stream.to_string());
    payload.write_outputs = true;

    let response = compact_trace_stream(payload).expect("compact");
    assert!(response.applied);
    assert_eq!(response.status, "compacted");
    assert_eq!(response.next_generation, 1);
    assert!(response.latest_stable_snapshot.is_some());
    assert_eq!(response.writes.len(), 5);

    let manifest_path = root.join("trace_compaction/cmp-sess__session.manifest.json");
    assert!(manifest_path.is_file(), "manifest written under root_path");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn record_trace_event_compaction_delta_generation_match_smoke() {
    let root = temp_trace_root("delta");
    let manifest_path = root.join("trace_compaction/sess-delta__session.manifest.json");
    fs::create_dir_all(manifest_path.parent().unwrap()).expect("mkdir");
    fs::write(
        &manifest_path,
        json!({
            "schema_version": "runtime-trace-compaction-manifest-v1",
            "active_generation": 1,
            "active_parent_snapshot_id": "snap_parent",
            "delta_path": root.join("trace_compaction/sess-delta__session.deltas.jsonl").display().to_string(),
        })
        .to_string(),
    )
    .expect("write manifest");

    let delta_path = root
        .join("trace_compaction/sess-delta__session.deltas.jsonl");
    let response = record_trace_event(TraceRecordEventRequestPayload {
        path: None,
        write_outputs: true,
        sink_schema_version: "runtime-trace-sink-v2".to_string(),
        event_schema_version: "runtime-trace-v2".to_string(),
        generation: 1,
        seq: 2,
        session_id: "sess-delta".to_string(),
        job_id: None,
        kind: "job.progress".to_string(),
        stage: "background".to_string(),
        status: "ok".to_string(),
        payload: Map::new(),
        compaction_manifest_path: Some(manifest_path.display().to_string()),
        compaction_manifest_text: None,
    })
    .expect("record");

    assert!(response.delta_bytes_written > 0);
    assert_eq!(
        response.delta_path.as_deref(),
        Some(delta_path.display().to_string().as_str())
    );
    let delta_text = fs::read_to_string(&delta_path).expect("delta file");
    assert!(delta_text.contains("runtime-trace-compaction-delta-v1"));
    assert!(delta_text.contains("job.progress"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compact_trace_stream_snapshot_delta_unsupported_smoke() {
    let root = temp_trace_root("snapshot-delta-off");
    let mut payload = base_compact_payload(&root, "sess-snap-off");
    payload.supports_snapshot_delta = false;

    let response = compact_trace_stream(payload).expect("compact");
    assert!(!response.applied);
    assert_eq!(response.status, "unsupported");
    assert!(response.reason.is_some());
}

#[test]
fn record_trace_event_no_manifest_writes_no_delta_smoke() {
    let response = record_trace_event(TraceRecordEventRequestPayload {
        path: None,
        write_outputs: false,
        sink_schema_version: "runtime-trace-sink-v2".to_string(),
        event_schema_version: "runtime-trace-v2".to_string(),
        generation: 0,
        seq: 1,
        session_id: "sess-plain".to_string(),
        job_id: None,
        kind: "job.started".to_string(),
        stage: "background".to_string(),
        status: "ok".to_string(),
        payload: Map::new(),
        compaction_manifest_path: None,
        compaction_manifest_text: None,
    })
    .expect("record");

    assert_eq!(response.delta_bytes_written, 0);
    assert!(response.delta_path.is_none());
    assert!(response.sink_line.contains("job.started"));
}

#[test]
fn record_trace_event_generation_mismatch_skips_delta_smoke() {
    let root = temp_trace_root("gen-mismatch");
    let manifest_path = root.join("trace_compaction/sess-gen__session.manifest.json");
    fs::create_dir_all(manifest_path.parent().unwrap()).expect("mkdir");
    fs::write(
        &manifest_path,
        json!({
            "schema_version": "runtime-trace-compaction-manifest-v1",
            "active_generation": 2,
            "active_parent_snapshot_id": "snap_parent",
            "delta_path": root.join("trace_compaction/sess-gen__session.deltas.jsonl").display().to_string(),
        })
        .to_string(),
    )
    .expect("write manifest");

    let response = record_trace_event(TraceRecordEventRequestPayload {
        path: None,
        write_outputs: true,
        sink_schema_version: "runtime-trace-sink-v2".to_string(),
        event_schema_version: "runtime-trace-v2".to_string(),
        generation: 1,
        seq: 1,
        session_id: "sess-gen".to_string(),
        job_id: None,
        kind: "job.progress".to_string(),
        stage: "background".to_string(),
        status: "ok".to_string(),
        payload: Map::new(),
        compaction_manifest_path: Some(manifest_path.display().to_string()),
        compaction_manifest_text: None,
    })
    .expect("record");

    assert_eq!(response.delta_bytes_written, 0);
    assert!(response.delta_path.is_none());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn record_trace_event_inline_manifest_text_delta_smoke() {
    let root = temp_trace_root("inline-manifest");
    let delta_path = root.join("trace_compaction/sess-inline__session.deltas.jsonl");
    let manifest_text = json!({
        "schema_version": "runtime-trace-compaction-manifest-v1",
        "active_generation": 1,
        "active_parent_snapshot_id": "snap_inline",
        "delta_path": delta_path.display().to_string(),
    })
    .to_string();

    let response = record_trace_event(TraceRecordEventRequestPayload {
        path: None,
        write_outputs: true,
        sink_schema_version: "runtime-trace-sink-v2".to_string(),
        event_schema_version: "runtime-trace-v2".to_string(),
        generation: 1,
        seq: 3,
        session_id: "sess-inline".to_string(),
        job_id: None,
        kind: "job.completed".to_string(),
        stage: "background".to_string(),
        status: "ok".to_string(),
        payload: Map::new(),
        compaction_manifest_path: None,
        compaction_manifest_text: Some(manifest_text),
    })
    .expect("record");

    assert!(response.delta_bytes_written > 0);
    assert_eq!(
        response.delta_path.as_deref(),
        Some(delta_path.display().to_string().as_str())
    );
    let delta_text = fs::read_to_string(&delta_path).expect("delta file");
    assert!(delta_text.contains("job.completed"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compact_trace_stream_job_id_filters_events_smoke() {
    let root = temp_trace_root("job-scope");
    let stream = concat!(
        r#"{"event_id":"evt-a","seq":1,"generation":0,"session_id":"scope-sess","job_id":"job-a","#,
        r#""kind":"job.started","stage":"background","status":"ok","cursor":"g0:s1:evt-a"}"#,
        "\n",
        r#"{"event_id":"evt-b","seq":2,"generation":0,"session_id":"scope-sess","job_id":"job-b","#,
        r#""kind":"job.started","stage":"background","status":"ok","cursor":"g0:s2:evt-b"}"#
    );
    let mut payload = base_compact_payload(&root, "scope-sess");
    payload.job_id = Some("job-a".to_string());
    payload.event_stream_text = Some(stream.to_string());

    let response = compact_trace_stream(payload).expect("compact");
    assert!(response.applied);
    let snapshot_value = response.latest_stable_snapshot.expect("snapshot");
    let snapshot = snapshot_value.as_object().expect("object");
    assert_eq!(snapshot.get("job_id").and_then(|v| v.as_str()), Some("job-a"));
    let summary = snapshot
        .get("summary")
        .and_then(|v| v.as_object())
        .expect("summary");
    assert_eq!(summary.get("event_count").and_then(|v| v.as_u64()), Some(1));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compact_trace_stream_increments_generation_with_previous_manifest_smoke() {
    let root = temp_trace_root("second-gen");
    let previous_manifest = json!({
        "schema_version": "runtime-trace-compaction-manifest-v1",
        "latest_stable_snapshot": {
            "schema_version": "runtime-trace-compaction-snapshot-v1",
            "generation": 0,
            "snapshot_id": "snap_gen0",
            "session_id": "gen-sess"
        }
    });
    let stream = concat!(
        r#"{"event_id":"evt-gen1","seq":5,"generation":1,"session_id":"gen-sess","#,
        r#""kind":"job.resumed","stage":"background","status":"ok","cursor":"g1:s5:evt-gen1"}"#
    );
    let mut payload = base_compact_payload(&root, "gen-sess");
    payload.current_generation = 1;
    payload.event_stream_text = Some(stream.to_string());
    payload.previous_manifest_text = Some(previous_manifest.to_string());
    payload.write_outputs = true;

    let response = compact_trace_stream(payload).expect("compact");
    assert!(response.applied);
    assert_eq!(response.current_generation, 1);
    assert_eq!(response.next_generation, 2);
    let snapshot_value = response.latest_stable_snapshot.expect("snapshot");
    let snapshot = snapshot_value.as_object().expect("object");
    assert_eq!(
        snapshot.get("parent_snapshot_id").and_then(|v| v.as_str()),
        Some("snap_gen0")
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compact_trace_stream_reads_event_stream_from_disk_smoke() {
    let root = temp_trace_root("disk-stream");
    let stream_path = root.join("events.jsonl");
    fs::create_dir_all(&root).expect("mkdir");
    fs::write(
        &stream_path,
        concat!(
            r#"{"event_id":"evt-disk","seq":1,"generation":0,"session_id":"disk-sess","#,
            r#""kind":"job.started","stage":"background","status":"ok","cursor":"g0:s1:evt-disk"}"#
        ),
    )
    .expect("write stream");

    let mut payload = base_compact_payload(&root, "disk-sess");
    payload.event_stream_path = Some(stream_path.display().to_string());
    payload.write_outputs = true;

    let response = compact_trace_stream(payload).expect("compact");
    assert!(response.applied);
    assert_eq!(response.status, "compacted");
    assert!(root.join("trace_compaction/disk-sess__session.manifest.json").is_file());

    let _ = fs::remove_dir_all(&root);
}
