use super::common::*;
use super::*;

use serde_json::{Map, Value, json};

#[test]
fn trace_stream_replay_unwraps_wrapped_events_and_supports_resume() {
    let trace_path = temp_trace_path("trace-replay");
    fs::write(
            &trace_path,
            concat!(
                "{\"sink_schema_version\":\"runtime-trace-sink-v2\",\"event\":{\"event_id\":\"evt-1\",\"kind\":\"job.started\",\"ts\":\"2026-04-22T10:00:00.000Z\"}}\n",
                "{\"event_id\":\"evt-2\",\"kind\":\"job.completed\",\"ts\":\"2026-04-22T10:00:01.000Z\"}\n"
            ),
        )
        .expect("write trace stream");

    let replay = replay_trace_stream(TraceStreamReplayRequestPayload {
        path: Some(trace_path.display().to_string()),
        event_stream_text: None,
        compaction_manifest_path: None,
        compaction_manifest_text: None,
        compaction_state_text: None,
        compaction_artifact_index_text: None,
        compaction_delta_text: None,
        session_id: None,
        job_id: None,
        stream_scope_fields: None,
        after_event_id: Some("evt-1".to_string()),
        limit: Some(10),
    })
    .expect("replay trace stream");

    assert_eq!(replay.schema_version, TRACE_STREAM_REPLAY_SCHEMA_VERSION);
    assert_eq!(replay.authority, TRACE_STREAM_IO_AUTHORITY);
    assert_eq!(replay.event_count, 2);
    assert_eq!(replay.source_kind, "trace_stream");
    assert_eq!(replay.events.len(), 1);
    assert_eq!(
        replay.events[0]["event_id"],
        Value::String("evt-2".to_string())
    );
    assert_eq!(
        replay.events[0]["kind"],
        Value::String("job.completed".to_string())
    );
    assert!(!replay.has_more);
    assert_eq!(
        replay.next_cursor.expect("next cursor").event_id.as_deref(),
        Some("evt-2")
    );
    assert!(replay.latest_cursor.is_none());

    fs::remove_file(&trace_path).expect("cleanup trace stream");
}

#[test]
fn trace_stream_inspect_reports_latest_event_metadata() {
    let trace_path = temp_trace_path("trace-inspect");
    fs::write(
            &trace_path,
            concat!(
                "{\"event_id\":\"evt-1\",\"kind\":\"job.started\",\"ts\":\"2026-04-22T10:00:00.000Z\"}\n",
                "{\"sink_schema_version\":\"runtime-trace-sink-v2\",\"event\":{\"event_id\":\"evt-2\",\"kind\":\"job.completed\",\"ts\":\"2026-04-22T10:00:01.000Z\"}}\n"
            ),
        )
        .expect("write trace stream");

    let summary = inspect_trace_stream(TraceStreamInspectRequestPayload {
        path: Some(trace_path.display().to_string()),
        event_stream_text: None,
        compaction_manifest_path: None,
        compaction_manifest_text: None,
        compaction_state_text: None,
        compaction_artifact_index_text: None,
        compaction_delta_text: None,
        session_id: None,
        job_id: None,
        stream_scope_fields: None,
    })
    .expect("inspect trace stream");

    assert_eq!(summary.schema_version, TRACE_STREAM_INSPECT_SCHEMA_VERSION);
    assert_eq!(summary.authority, TRACE_STREAM_IO_AUTHORITY);
    assert_eq!(summary.source_kind, "trace_stream");
    assert_eq!(summary.event_count, 2);
    assert_eq!(summary.latest_event_id.as_deref(), Some("evt-2"));
    assert_eq!(summary.latest_event_kind.as_deref(), Some("job.completed"));
    assert_eq!(
        summary.latest_event_timestamp.as_deref(),
        Some("2026-04-22T10:00:01.000Z")
    );
    assert!(summary.latest_cursor.is_none());

    fs::remove_file(&trace_path).expect("cleanup trace stream");
}

#[test]
fn trace_stream_replay_filters_by_scope_and_hydrates_cursor_fields() {
    let trace_path = temp_trace_path("trace-scope");
    fs::write(
            &trace_path,
            concat!(
                "{\"sink_schema_version\":\"runtime-trace-sink-v2\",\"event\":{\"session_id\":\"session-1\",\"job_id\":\"job-1\",\"event_id\":\"evt-1\",\"kind\":\"job.started\",\"stage\":\"background\",\"ts\":\"2026-04-22T10:00:00.000Z\"}}\n",
                "{\"session_id\":\"session-1\",\"job_id\":\"job-2\",\"event_id\":\"evt-2\",\"kind\":\"job.completed\",\"stage\":\"background\",\"ts\":\"2026-04-22T10:00:01.000Z\"}\n"
            ),
        )
        .expect("write trace stream");

    let replay = replay_trace_stream(TraceStreamReplayRequestPayload {
        path: Some(trace_path.display().to_string()),
        event_stream_text: None,
        compaction_manifest_path: None,
        compaction_manifest_text: None,
        compaction_state_text: None,
        compaction_artifact_index_text: None,
        compaction_delta_text: None,
        session_id: Some("session-1".to_string()),
        job_id: Some("job-1".to_string()),
        stream_scope_fields: None,
        after_event_id: None,
        limit: Some(10),
    })
    .expect("replay scoped trace stream");

    assert_eq!(replay.event_count, 1);
    assert_eq!(replay.events.len(), 1);
    assert_eq!(
        replay.events[0]["event_id"],
        Value::String("evt-1".to_string())
    );
    assert_eq!(replay.events[0]["seq"], json!(1));
    assert_eq!(replay.events[0]["generation"], json!(0));
    assert_eq!(
        replay.events[0]["cursor"],
        Value::String("g0:s1:evt-1".to_string())
    );
    assert_eq!(
        replay.latest_cursor.expect("latest cursor")["cursor"],
        Value::String("g0:s1:evt-1".to_string())
    );

    fs::remove_file(&trace_path).expect("cleanup trace stream");
}

#[test]
fn attach_runtime_event_transport_preserves_resume_manifest_resolution_on_descriptor_roundtrip() {
    let binding_artifact_path = temp_json_path("attach-transport");
    let resume_manifest_path = temp_json_path("attach-resume-manifest");
    let trace_stream_path = temp_trace_path("attach-trace-stream");

    fs::write(
        &binding_artifact_path,
        serde_json::to_string_pretty(&json!({
            "stream_id": "stream-attach-roundtrip",
            "session_id": "session-attach-roundtrip",
            "job_id": "job-attach-roundtrip",
            "binding_backend_family": "filesystem",
            "resume_mode": "after_event_id"
        }))
        .expect("serialize binding artifact"),
    )
    .expect("write binding artifact");
    fs::write(&trace_stream_path, "").expect("write empty trace stream");
    fs::write(
        &resume_manifest_path,
        serde_json::to_string_pretty(&json!({
            "session_id": "session-attach-roundtrip",
            "job_id": "job-attach-roundtrip",
            "event_transport_path": binding_artifact_path.display().to_string(),
            "trace_stream_path": trace_stream_path.display().to_string()
        }))
        .expect("serialize resume manifest"),
    )
    .expect("write resume manifest");

    let attached = attach_runtime_event_transport(json!({
        "resume_manifest_path": resume_manifest_path.display().to_string()
    }))
    .expect("attach via resume manifest");
    let attach_descriptor = attached
        .get("attach_descriptor")
        .cloned()
        .expect("attach descriptor");
    assert_eq!(
        attach_descriptor["resolution"]["binding_artifact_path"],
        Value::String("resume_manifest".to_string())
    );
    assert_eq!(
        attach_descriptor["resolution"]["resume_manifest_path"],
        Value::String("explicit_request".to_string())
    );

    let roundtrip = attach_runtime_event_transport(json!({
        "attach_descriptor": attach_descriptor
    }))
    .expect("attach via descriptor roundtrip");
    assert_eq!(
        roundtrip["attach_descriptor"]["resolution"]["binding_artifact_path"],
        Value::String("resume_manifest".to_string())
    );
    assert_eq!(
        roundtrip["attach_descriptor"]["resolution"]["resume_manifest_path"],
        Value::String("explicit_request".to_string())
    );
    assert_eq!(
        roundtrip["binding_artifact_path"],
        Value::String(binding_artifact_path.display().to_string())
    );
    assert_eq!(
        roundtrip["resume_manifest_path"],
        Value::String(resume_manifest_path.display().to_string())
    );

    fs::remove_file(&binding_artifact_path).expect("cleanup binding artifact");
    fs::remove_file(&resume_manifest_path).expect("cleanup resume manifest");
    fs::remove_file(&trace_stream_path).expect("cleanup trace stream");
}

#[test]
fn attach_runtime_event_transport_reads_sqlite_resume_manifest_trace_stream() {
    let root = temp_json_path("attach-sqlite-root")
        .with_extension("")
        .join("runtime-data");
    let db_path = root.join("runtime_checkpoint_store.sqlite3");
    let binding_artifact_path = root
        .join("runtime_event_transports")
        .join("session-sqlite__job-sqlite.json");
    let resume_manifest_path = root.join("TRACE_RESUME_MANIFEST.json");
    let trace_stream_path = root.join("TRACE_EVENTS.jsonl");

    fs::create_dir_all(binding_artifact_path.parent().expect("binding parent"))
        .expect("create sqlite fixture dir");
    let conn = rusqlite::Connection::open(&db_path).expect("open sqlite fixture");
    conn.execute(
            "CREATE TABLE runtime_storage_payloads (payload_key TEXT PRIMARY KEY, payload_text TEXT NOT NULL)",
            [],
        )
        .expect("create runtime storage payload table");
    for (path, payload) in [
            (
                binding_artifact_path.clone(),
                serde_json::to_string_pretty(&json!({
                    "schema_version": "runtime-event-transport-v1",
                    "stream_id": "stream::job-sqlite",
                    "session_id": "session-sqlite",
                    "job_id": "job-sqlite",
                    "binding_backend_family": "sqlite",
                    "resume_mode": "after_event_id",
                    "cleanup_preserves_replay": true
                }))
                .expect("serialize binding"),
            ),
            (
                resume_manifest_path.clone(),
                serde_json::to_string_pretty(&json!({
                    "schema_version": "runtime-resume-manifest-v1",
                    "session_id": "session-sqlite",
                    "job_id": "job-sqlite",
                    "event_transport_path": binding_artifact_path.display().to_string(),
                    "trace_stream_path": trace_stream_path.display().to_string(),
                    "updated_at": "2026-04-23T00:00:01+00:00"
                }))
                .expect("serialize resume"),
            ),
            (
                trace_stream_path.clone(),
                "{\"event_id\":\"evt-sqlite-1\",\"kind\":\"job.started\",\"ts\":\"2026-04-23T00:00:00.000Z\"}\n".to_string(),
            ),
        ] {
            let relative_key = path
                .strip_prefix(&root)
                .expect("path under sqlite root")
                .to_string_lossy()
                .replace('\\', "/");
            let stable_key = format!(
                "{}::{}",
                root.display().to_string().replace('\\', "/"),
                relative_key
            );
            conn.execute(
                "INSERT OR REPLACE INTO runtime_storage_payloads (payload_key, payload_text) VALUES (?1, ?2)",
                rusqlite::params![stable_key, payload],
            )
            .expect("insert sqlite fixture payload");
        }
    drop(conn);

    let attached = attach_runtime_event_transport(json!({
        "resume_manifest_path": resume_manifest_path.display().to_string()
    }))
    .expect("attach via sqlite resume manifest");
    assert_eq!(
        attached["artifact_backend_family"],
        Value::String("sqlite".to_string())
    );
    assert_eq!(
        attached["trace_stream_path"],
        Value::String(trace_stream_path.display().to_string())
    );

    let replay = replay_trace_stream(TraceStreamReplayRequestPayload {
        path: Some(trace_stream_path.display().to_string()),
        event_stream_text: None,
        compaction_manifest_path: None,
        compaction_manifest_text: None,
        compaction_state_text: None,
        compaction_artifact_index_text: None,
        compaction_delta_text: None,
        session_id: None,
        job_id: None,
        stream_scope_fields: None,
        after_event_id: None,
        limit: Some(10),
    })
    .expect("replay sqlite trace stream");
    assert_eq!(replay.event_count, 1);
    assert_eq!(
        replay.events[0]["event_id"],
        Value::String("evt-sqlite-1".to_string())
    );

    fs::remove_dir_all(root.parent().expect("fixture parent")).expect("cleanup sqlite fixture");
}

#[test]
fn trace_compaction_inspect_and_replay_read_snapshot_plus_deltas() {
    let temp_root = temp_trace_path("trace-compaction");
    let trace_root = temp_root.parent().expect("temp root parent").join(
        temp_root
            .file_stem()
            .expect("temp root stem")
            .to_string_lossy()
            .to_string(),
    );
    let manifest_path = trace_root.join("stream.manifest.json");
    let delta_path = trace_root.join("stream.deltas.jsonl");
    let artifact_dir = trace_root.join("artifacts");
    let state_path = artifact_dir.join("stream.state.json");
    let artifact_index_path = artifact_dir.join("stream.artifacts.json");
    fs::create_dir_all(&artifact_dir).expect("create artifact dir");
    let state_text = serde_json::to_string_pretty(&json!({
        "session_id": "session-compact",
        "job_id": "job-compact",
        "latest_cursor": {
            "schema_version": "runtime-trace-cursor-v1",
            "session_id": "session-compact",
            "job_id": "job-compact",
            "generation": 0,
            "seq": 2,
            "event_id": "evt-snapshot",
            "cursor": "g0:s2:evt-snapshot"
        },
        "latest_event": {
            "event_id": "evt-snapshot",
            "kind": "job.progress",
            "stage": "background",
            "status": "ok",
            "ts": "2026-04-22T10:00:00.000Z"
        }
    }))
    .expect("serialize state");
    fs::write(&state_path, &state_text).expect("write state");
    let state_digest = sha256_hex(state_text.as_bytes());
    let artifact_index_text = serde_json::to_string_pretty(&json!([
        {
            "schema_version": "runtime-trace-artifact-ref-v1",
            "artifact_id": "art-state",
            "kind": "state_ref",
            "uri": state_path.display().to_string(),
            "digest": state_digest,
            "size_bytes": state_text.len()
        }
    ]))
    .expect("serialize artifact index");
    fs::write(&artifact_index_path, &artifact_index_text).expect("write artifact index");
    let artifact_index_digest = sha256_hex(artifact_index_text.as_bytes());
    fs::write(
            &delta_path,
            concat!(
                "{\"schema_version\":\"runtime-trace-compaction-delta-v1\",\"generation\":1,\"delta_id\":\"delta-1\",\"parent_snapshot_id\":\"snap-1\",\"seq\":1,\"ts\":\"2026-04-22T10:00:01.000Z\",\"kind\":\"job.resumed\",\"payload\":{\"event_id\":\"evt-1\",\"cursor\":\"g1:s1:evt-1\",\"stage\":\"background\",\"status\":\"ok\",\"payload\":{\"step\":3}},\"artifact_refs\":[],\"applies_to\":{\"session_id\":\"session-compact\",\"job_id\":\"job-compact\"}}\n",
                "{\"schema_version\":\"runtime-trace-compaction-delta-v1\",\"generation\":1,\"delta_id\":\"delta-2\",\"parent_snapshot_id\":\"snap-1\",\"seq\":2,\"ts\":\"2026-04-22T10:00:02.000Z\",\"kind\":\"job.completed\",\"payload\":{\"event_id\":\"evt-2\",\"cursor\":\"g1:s2:evt-2\",\"stage\":\"background\",\"status\":\"ok\",\"payload\":{\"step\":4}},\"artifact_refs\":[],\"applies_to\":{\"session_id\":\"session-compact\",\"job_id\":\"job-compact\"}}\n"
            ),
        )
        .expect("write deltas");
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&json!({
            "schema_version": "runtime-trace-compaction-manifest-v1",
            "session_id": "session-compact",
            "job_id": "job-compact",
            "backend_family": "filesystem",
            "compaction_supported": true,
            "snapshot_delta_supported": true,
            "latest_stable_snapshot": {
                "schema_version": "runtime-trace-compaction-snapshot-v1",
                "generation": 0,
                "snapshot_id": "snap-1",
                "session_id": "session-compact",
                "job_id": "job-compact",
                "state_digest": "state-digest",
                "artifact_index_ref": {
                    "schema_version": "runtime-trace-artifact-ref-v1",
                    "artifact_id": "art-index",
                    "kind": "artifact_index_ref",
                    "uri": artifact_index_path.display().to_string(),
                    "digest": artifact_index_digest,
                    "size_bytes": artifact_index_text.len()
                },
                "state_ref": {
                    "schema_version": "runtime-trace-artifact-ref-v1",
                    "artifact_id": "art-state",
                    "kind": "state_ref",
                    "uri": state_path.display().to_string(),
                    "digest": state_digest,
                    "size_bytes": state_text.len()
                }
            },
            "active_generation": 1,
            "active_parent_snapshot_id": "snap-1",
            "manifest_path": manifest_path.display().to_string(),
            "delta_path": delta_path.display().to_string(),
            "artifact_index_path": artifact_index_path.display().to_string(),
            "state_path": state_path.display().to_string()
        }))
        .expect("serialize manifest"),
    )
    .expect("write manifest");

    let summary = inspect_trace_stream(TraceStreamInspectRequestPayload {
        path: None,
        event_stream_text: None,
        compaction_manifest_path: Some(manifest_path.display().to_string()),
        compaction_manifest_text: None,
        compaction_state_text: None,
        compaction_artifact_index_text: None,
        compaction_delta_text: None,
        session_id: Some("session-compact".to_string()),
        job_id: Some("job-compact".to_string()),
        stream_scope_fields: None,
    })
    .expect("inspect compaction manifest");
    assert_eq!(summary.source_kind, "compaction_manifest");
    assert_eq!(summary.event_count, 2);
    assert_eq!(summary.latest_event_id.as_deref(), Some("evt-2"));
    assert_eq!(
        summary.recovery.expect("recovery")["latest_recoverable_generation"],
        json!(1)
    );

    let replay = replay_trace_stream(TraceStreamReplayRequestPayload {
        path: None,
        event_stream_text: None,
        compaction_manifest_path: Some(manifest_path.display().to_string()),
        compaction_manifest_text: None,
        compaction_state_text: None,
        compaction_artifact_index_text: None,
        compaction_delta_text: None,
        session_id: Some("session-compact".to_string()),
        job_id: Some("job-compact".to_string()),
        stream_scope_fields: None,
        after_event_id: Some("evt-1".to_string()),
        limit: Some(10),
    })
    .expect("replay compaction manifest");
    assert_eq!(replay.source_kind, "compaction_manifest");
    assert_eq!(replay.events.len(), 1);
    assert_eq!(
        replay.events[0]["event_id"],
        Value::String("evt-2".to_string())
    );

    fs::remove_dir_all(&trace_root).expect("cleanup compaction root");
}

#[test]
fn trace_compaction_recovery_fails_closed_on_artifact_digest_mismatch() {
    let temp_root = temp_trace_path("trace-compaction-digest-mismatch");
    let trace_root = temp_root.parent().expect("temp root parent").join(
        temp_root
            .file_stem()
            .expect("temp root stem")
            .to_string_lossy()
            .to_string(),
    );
    let manifest_path = trace_root.join("stream.manifest.json");
    let artifact_dir = trace_root.join("artifacts");
    let state_path = artifact_dir.join("stream.state.json");
    let artifact_index_path = artifact_dir.join("stream.artifacts.json");
    fs::create_dir_all(&artifact_dir).expect("create digest mismatch artifact dir");
    let state_text = serde_json::to_string_pretty(&json!({
        "session_id": "session-compact",
        "job_id": "job-compact",
        "latest_cursor": {
            "schema_version": "runtime-trace-cursor-v1",
            "session_id": "session-compact",
            "job_id": "job-compact",
            "generation": 0,
            "seq": 1,
            "event_id": "evt-snapshot",
            "cursor": "g0:s1:evt-snapshot"
        }
    }))
    .expect("serialize digest mismatch state");
    fs::write(&state_path, &state_text).expect("write digest mismatch state");
    let artifact_index_text = "[]";
    fs::write(&artifact_index_path, artifact_index_text)
        .expect("write digest mismatch artifact index");
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&json!({
            "schema_version": "runtime-trace-compaction-manifest-v1",
            "session_id": "session-compact",
            "job_id": "job-compact",
            "latest_stable_snapshot": {
                "schema_version": "runtime-trace-compaction-snapshot-v1",
                "generation": 0,
                "snapshot_id": "snap-1",
                "session_id": "session-compact",
                "job_id": "job-compact",
                "state_ref": {
                    "schema_version": "runtime-trace-artifact-ref-v1",
                    "artifact_id": "art-state",
                    "kind": "state_ref",
                    "uri": state_path.display().to_string(),
                    "digest": "not-the-real-digest",
                    "size_bytes": state_text.len()
                },
                "artifact_index_ref": {
                    "schema_version": "runtime-trace-artifact-ref-v1",
                    "artifact_id": "art-index",
                    "kind": "artifact_index_ref",
                    "uri": artifact_index_path.display().to_string(),
                    "digest": sha256_hex(artifact_index_text.as_bytes()),
                    "size_bytes": artifact_index_text.len()
                }
            },
            "active_generation": 1,
            "active_parent_snapshot_id": "snap-1",
            "manifest_path": manifest_path.display().to_string(),
            "artifact_index_path": artifact_index_path.display().to_string(),
            "state_path": state_path.display().to_string()
        }))
        .expect("serialize digest mismatch manifest"),
    )
    .expect("write digest mismatch manifest");

    let err = inspect_trace_stream(TraceStreamInspectRequestPayload {
        path: None,
        event_stream_text: None,
        compaction_manifest_path: Some(manifest_path.display().to_string()),
        compaction_manifest_text: None,
        compaction_state_text: None,
        compaction_artifact_index_text: None,
        compaction_delta_text: None,
        session_id: Some("session-compact".to_string()),
        job_id: Some("job-compact".to_string()),
        stream_scope_fields: None,
    })
    .expect_err("digest mismatch must fail closed");

    assert!(
        err.to_string().contains("artifact digest mismatched"),
        "expected digest mismatch error, got: {err}"
    );

    fs::remove_dir_all(&trace_root).expect("cleanup digest mismatch compaction root");
}

#[test]
fn write_trace_compaction_delta_appends_one_jsonl_line() {
    let delta_path = temp_trace_path("trace-delta-write");
    let response = write_trace_compaction_delta(TraceCompactionDeltaWriteRequestPayload {
        path: delta_path.display().to_string(),
        delta: json!({
            "schema_version": "runtime-trace-compaction-delta-v1",
            "delta_id": "delta-1",
            "seq": 1
        }),
    })
    .expect("write delta");

    assert_eq!(
        response.schema_version,
        TRACE_COMPACTION_DELTA_WRITE_SCHEMA_VERSION
    );
    assert_eq!(response.authority, TRACE_STREAM_IO_AUTHORITY);
    assert_eq!(response.path, delta_path.display().to_string());
    assert!(response.bytes_written > 0);
    let persisted = fs::read_to_string(&delta_path).expect("read delta");
    assert!(persisted.contains("\"delta_id\":\"delta-1\""));

    fs::remove_file(&delta_path).expect("cleanup delta path");
}

#[test]
fn trace_append_preserves_jsonl_records_under_concurrency() {
    let trace_path = temp_trace_path("trace-record-event-concurrent");
    let mut workers = Vec::new();
    for seq in 0..32 {
        let path = trace_path.clone();
        workers.push(spawn(move || {
            record_trace_event(TraceRecordEventRequestPayload {
                path: Some(path.display().to_string()),
                write_outputs: true,
                sink_schema_version: "runtime-trace-sink-v2".to_string(),
                event_schema_version: "runtime-trace-v2".to_string(),
                generation: 1,
                seq,
                run_id: "concurrent-trace".to_string(),
                job_id: None,
                kind: "test.event".to_string(),
                stage: "append".to_string(),
                status: "ok".to_string(),
                payload: Map::new(),
                compaction_manifest_path: None,
                compaction_manifest_text: None,
            })
            .expect("record trace event");
        }));
    }
    for worker in workers {
        worker.join().expect("join trace worker");
    }

    let persisted = fs::read_to_string(&trace_path).expect("read trace jsonl");
    let lines = persisted.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 32);
    let mut seen = HashSet::new();
    for line in lines {
        let record = serde_json::from_str::<Value>(line).expect("parse trace jsonl");
        seen.insert(record["event"]["seq"].as_u64().expect("seq"));
    }
    assert_eq!(seen.len(), 32);

    fs::remove_file(&trace_path).expect("cleanup trace path");
}

#[test]
fn stdio_request_dispatches_write_trace_compaction_delta_payload() {
    let delta_path = temp_trace_path("trace-delta-write-stdio");
    let response = handle_stdio_json_line(&format!(
        "{{\"id\":2,\"op\":\"write_trace_compaction_delta\",\"payload\":{{\"path\":\"{}\",\"delta\":{{\"schema_version\":\"runtime-trace-compaction-delta-v1\",\"delta_id\":\"delta-stdio\",\"seq\":2}}}}}}",
        delta_path.display()
    ));
    assert!(response.ok);
    assert_eq!(response.id, json!(2));
    assert_eq!(
        response.payload.expect("payload")["schema_version"],
        json!(TRACE_COMPACTION_DELTA_WRITE_SCHEMA_VERSION)
    );
    let persisted = fs::read_to_string(&delta_path).expect("read stdio delta");
    assert!(persisted.contains("\"delta_id\":\"delta-stdio\""));
    fs::remove_file(&delta_path).expect("cleanup stdio delta path");
}

#[test]
fn write_trace_metadata_persists_primary_and_mirror_outputs() {
    let output_path = temp_json_path("trace-metadata-write");
    let mirror_path = output_path
        .parent()
        .expect("output parent")
        .join("artifacts")
        .join("current")
        .join("TRACE_METADATA.json");
    let response = write_trace_metadata(TraceMetadataWriteRequestPayload {
        output_path: output_path.display().to_string(),
        mirror_paths: vec![mirror_path.display().to_string()],
        write_outputs: true,
        task: "trace metadata rustification".to_string(),
        matched_skills: vec!["goal_drive".to_string()],
        owner: "goal_drive".to_string(),
        gate: "none".to_string(),
        overlay: None,
        reroute_count: Some(0),
        retry_count: Some(1),
        artifact_paths: vec!["artifacts/current/SESSION_SUMMARY.md".to_string()],
        verification_status: "passed".to_string(),
        session_id: None,
        job_id: None,
        event_stream_path: None,
        event_stream_text: None,
        compaction_manifest_path: None,
        compaction_manifest_text: None,
        compaction_state_text: None,
        compaction_artifact_index_text: None,
        compaction_delta_text: None,
        stream_scope_fields: None,
        framework_version: Some("phase1".to_string()),
        metadata_schema_version: Some("trace-metadata-v2".to_string()),
        routing_runtime_version: Some(9),
        runtime_path: None,
        ts: Some("2026-04-23T00:00:00Z".to_string()),
        trace_event_schema_version: None,
        trace_event_sink_schema_version: None,
        parallel_group: None,
        supervisor_projection: None,
        control_plane: None,
        stream: None,
        events: None,
    })
    .expect("write trace metadata");

    assert_eq!(response.schema_version, TRACE_METADATA_WRITE_SCHEMA_VERSION);
    assert_eq!(response.authority, TRACE_METADATA_WRITE_AUTHORITY);
    assert_eq!(response.output_path, output_path.display().to_string());
    assert_eq!(response.routing_runtime_version, 9);
    assert!(response.payload_text.contains("\"version\": 1"));
    let primary = fs::read_to_string(&output_path).expect("read primary trace metadata");
    let mirror = fs::read_to_string(&mirror_path).expect("read mirror trace metadata");
    assert_eq!(primary, mirror);
    assert!(primary.contains("\"schema_version\": \"trace-metadata-v2\""));
    assert!(primary.contains("\"task\": \"trace metadata rustification\""));

    fs::remove_file(&output_path).expect("cleanup primary trace metadata");
    fs::remove_file(&mirror_path).expect("cleanup mirror trace metadata");
    fs::remove_dir_all(
        mirror_path
            .parent()
            .and_then(Path::parent)
            .expect("cleanup mirror root"),
    )
    .expect("cleanup mirror directories");
}

#[test]
fn stdio_request_dispatches_write_trace_metadata_payload() {
    let output_path = temp_json_path("trace-metadata-write-stdio");
    let response = handle_stdio_json_line(&format!(
        "{{\"id\":3,\"op\":\"write_trace_metadata\",\"payload\":{{\"output_path\":\"{}\",\"task\":\"trace metadata stdio\",\"matched_skills\":[\"goal_drive\"],\"owner\":\"goal_drive\",\"gate\":\"none\",\"overlay\":null,\"reroute_count\":0,\"retry_count\":0,\"artifact_paths\":[],\"verification_status\":\"passed\",\"metadata_schema_version\":\"trace-metadata-v2\",\"routing_runtime_version\":11}}}}",
        output_path.display()
    ));
    assert!(response.ok);
    assert_eq!(response.id, json!(3));
    assert_eq!(
        response.payload.expect("payload")["schema_version"],
        json!(TRACE_METADATA_WRITE_SCHEMA_VERSION)
    );
    let persisted = fs::read_to_string(&output_path).expect("read stdio trace metadata");
    assert!(persisted.contains("\"routing_runtime_version\": 11"));
    fs::remove_file(&output_path).expect("cleanup stdio trace metadata");
}

#[test]
fn write_trace_metadata_fails_closed_for_explicit_bad_trace_source() {
    let output_path = temp_json_path("trace-metadata-bad-source");
    let missing_trace_path = temp_trace_path("trace-metadata-missing-source");
    let response = write_trace_metadata(TraceMetadataWriteRequestPayload {
        output_path: output_path.display().to_string(),
        mirror_paths: Vec::new(),
        write_outputs: true,
        task: "trace metadata missing source".to_string(),
        matched_skills: Vec::new(),
        owner: "goal_drive".to_string(),
        gate: "none".to_string(),
        overlay: None,
        reroute_count: Some(0),
        retry_count: Some(0),
        artifact_paths: Vec::new(),
        verification_status: "passed".to_string(),
        session_id: None,
        job_id: None,
        event_stream_path: Some(missing_trace_path.display().to_string()),
        event_stream_text: None,
        compaction_manifest_path: None,
        compaction_manifest_text: None,
        compaction_state_text: None,
        compaction_artifact_index_text: None,
        compaction_delta_text: None,
        stream_scope_fields: None,
        framework_version: None,
        metadata_schema_version: Some("trace-metadata-v2".to_string()),
        routing_runtime_version: Some(11),
        runtime_path: None,
        ts: Some("2026-04-23T00:00:00Z".to_string()),
        trace_event_schema_version: None,
        trace_event_sink_schema_version: None,
        parallel_group: None,
        supervisor_projection: None,
        control_plane: None,
        stream: None,
        events: Some(Vec::new()),
    });

    assert!(response.is_err());
    assert!(!output_path.exists());
}

#[test]
fn subscribe_attached_runtime_events_returns_cursor_not_event_payload() {
    let binding_artifact_path = temp_json_path("subscribe-transport");
    let resume_manifest_path = temp_json_path("subscribe-resume-manifest");
    let trace_stream_path = temp_trace_path("subscribe-trace-stream");

    fs::write(
        &binding_artifact_path,
        serde_json::to_string_pretty(&json!({
            "schema_version": "runtime-event-transport-v1",
            "stream_id": "stream::job-subscribe",
            "session_id": "session-subscribe",
            "job_id": "job-subscribe",
            "binding_backend_family": "filesystem",
            "resume_mode": "after_event_id"
        }))
        .expect("serialize binding artifact"),
    )
    .expect("write binding artifact");
    fs::write(
        &resume_manifest_path,
        serde_json::to_string_pretty(&json!({
            "schema_version": "runtime-resume-manifest-v1",
            "session_id": "session-subscribe",
            "job_id": "job-subscribe",
            "event_transport_path": binding_artifact_path.display().to_string(),
            "trace_stream_path": trace_stream_path.display().to_string()
        }))
        .expect("serialize resume manifest"),
    )
    .expect("write resume manifest");
    fs::write(
            &trace_stream_path,
            concat!(
                "{\"event_id\":\"evt-1\",\"kind\":\"job.started\",\"session_id\":\"session-subscribe\",\"job_id\":\"job-subscribe\"}\n",
                "{\"event_id\":\"evt-2\",\"kind\":\"job.completed\",\"session_id\":\"session-subscribe\",\"job_id\":\"job-subscribe\"}\n"
            ),
        )
        .expect("write trace stream");

    let response = subscribe_attached_runtime_events(json!({
        "resume_manifest_path": resume_manifest_path.display().to_string(),
        "after_event_id": "evt-1",
        "limit": 1
    }))
    .expect("subscribe attached events");

    assert_eq!(response["events"].as_array().expect("events").len(), 1);
    assert_eq!(
        response["next_cursor"],
        json!({"event_id": "evt-2", "event_index": 1})
    );
    assert_eq!(response["next_cursor"]["kind"], Value::Null);

    fs::remove_file(&binding_artifact_path).expect("cleanup binding artifact");
    fs::remove_file(&resume_manifest_path).expect("cleanup resume manifest");
    fs::remove_file(&trace_stream_path).expect("cleanup trace stream");
}
