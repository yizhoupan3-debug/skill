use serde_json::{Map, Value, json};
use std::fs;
use std::path::{Path, PathBuf};

use super::{
    TRACE_COMPACT_SCHEMA_VERSION, TRACE_COMPACTION_ARTIFACT_REF_SCHEMA_VERSION,
    TRACE_COMPACTION_MANIFEST_SCHEMA_VERSION, TRACE_COMPACTION_RESULT_SCHEMA_VERSION,
    TRACE_COMPACTION_SNAPSHOT_SCHEMA_VERSION, TRACE_REPLAY_CURSOR_SCHEMA_VERSION,
    TRACE_STREAM_IO_AUTHORITY, TraceCompactRequestPayload, TraceCompactResponsePayload,
    TraceTextWrite,
};
use super::util::build_prefixed_id;
use super::util::{
    build_trace_cursor, hydrate_trace_event, sha256_hex, trace_event_object,
    trace_event_string_field, trace_event_usize_field,
};

#[tracing::instrument(level = "debug", skip_all)]
pub fn compact_trace_stream(
    payload: TraceCompactRequestPayload,
) -> Result<TraceCompactResponsePayload, String> {
    if !payload.supports_compaction || !payload.supports_snapshot_delta {
        return Ok(TraceCompactResponsePayload {
            schema_version: TRACE_COMPACTION_RESULT_SCHEMA_VERSION.to_string(),
            authority: TRACE_STREAM_IO_AUTHORITY.to_string(),
            applied: false,
            status: "unsupported".to_string(),
            reason: Some(
                "storage backend does not advertise compaction + snapshot-delta support"
                    .to_string(),
            ),
            run_id: payload.run_id,
            job_id: payload.job_id,
            backend_family: payload.backend_family,
            current_generation: payload.current_generation,
            next_generation: payload.current_generation,
            latest_stable_snapshot: None,
            manifest_path: None,
            writes: Vec::new(),
        });
    }
    let stream_text = match payload.event_stream_text.as_deref() {
        Some(value) => value.to_string(),
        None => match payload.event_stream_path.as_deref() {
            Some(path) if Path::new(path).exists() => fs::read_to_string(path)
                .map_err(|err| format!("read trace stream failed for {path}: {err}"))?,
            _ => String::new(),
        },
    };
    let source_events = load_trace_events_from_text(
        &stream_text,
        Some(&payload.run_id),
        payload.job_id.as_deref(),
    )?;
    let active_generation = source_events
        .last()
        .and_then(|event| trace_event_usize_field(event, "generation"))
        .unwrap_or(payload.current_generation);
    let active_events: Vec<Map<String, Value>> = source_events
        .into_iter()
        .filter(|event| {
            trace_event_usize_field(event, "generation").unwrap_or(0) == active_generation
        })
        .collect();
    if active_events.is_empty() {
        return Ok(TraceCompactResponsePayload {
            schema_version: TRACE_COMPACT_SCHEMA_VERSION.to_string(),
            authority: TRACE_STREAM_IO_AUTHORITY.to_string(),
            applied: false,
            status: "no_events".to_string(),
            reason: Some("no matching events available for compaction".to_string()),
            run_id: payload.run_id,
            job_id: payload.job_id,
            backend_family: payload.backend_family,
            current_generation: payload.current_generation,
            next_generation: payload.current_generation,
            latest_stable_snapshot: None,
            manifest_path: None,
            writes: Vec::new(),
        });
    }

    let paths = compaction_paths(
        &payload.root_path,
        &payload.run_id,
        payload.job_id.as_deref(),
    );
    let previous_manifest = previous_manifest_payload(&payload, &paths.manifest)?;
    let parent_snapshot = previous_manifest
        .as_ref()
        .and_then(|manifest| manifest.get("latest_stable_snapshot"))
        .cloned();
    let tail = active_events.last().expect("active events checked");
    let latest_cursor = latest_cursor_from_event(tail).unwrap_or(Value::Null);
    let mut state_payload = Map::new();
    state_payload.insert("run_id".to_string(), Value::String(payload.run_id.clone()));
    state_payload.insert(
        "job_id".to_string(),
        payload
            .job_id
            .as_deref()
            .map(|s| Value::String(s.to_string()))
            .unwrap_or(Value::Null),
    );
    state_payload.insert("generation".to_string(), json!(active_generation));
    state_payload.insert(
        "watermark_event_id".to_string(),
        trace_event_string_field(tail, "event_id")
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    state_payload.insert(
        "delta_cursor".to_string(),
        trace_event_string_field(tail, "cursor")
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    state_payload.insert("latest_cursor".to_string(), latest_cursor.clone());
    state_payload.insert("event_count".to_string(), json!(active_events.len()));
    state_payload.insert("latest_event".to_string(), Value::Object(tail.clone()));
    state_payload.insert(
        "continuity_artifacts".to_string(),
        Value::Array(
            unique_strings(&payload.artifact_paths)
                .into_iter()
                .map(Value::String)
                .collect(),
        ),
    );
    let state_value = Value::Object(state_payload);
    let state_serialized = pretty_json_line(&state_value)?;
    let state_ref = build_artifact_ref(
        "state_ref",
        &paths.state,
        &state_serialized,
        "runtime-trace-recorder",
    );

    let mut artifact_refs = vec![state_ref.clone()];
    if let Some(output_path) = payload.output_path.as_deref() {
        let output_payload = payload
            .output_text
            .as_deref()
            .map(str::to_string)
            .or_else(|| fs::read_to_string(output_path).ok());
        if let Some(output_payload) = output_payload {
            artifact_refs.push(build_artifact_ref(
                "trace_output",
                Path::new(output_path),
                &output_payload,
                "runtime-trace-recorder",
            ));
        }
    }
    if let Some(stream_path) = payload.event_stream_path.as_deref() {
        artifact_refs.push(build_artifact_ref(
            "trace_stream",
            Path::new(stream_path),
            &stream_text,
            "runtime-trace-recorder",
        ));
    }
    for artifact_path in unique_strings(&payload.artifact_paths) {
        artifact_refs.push(build_external_artifact_ref(&artifact_path));
    }
    let artifact_index = Value::Array(artifact_refs);
    let artifact_index_serialized = pretty_json_line(&artifact_index)?;
    let artifact_index_ref = build_artifact_ref(
        "artifact_index_ref",
        &paths.artifact_index,
        &artifact_index_serialized,
        "runtime-trace-recorder",
    );

    let snapshot_id = build_prefixed_id("snap", &state_serialized);
    let mut snapshot = Map::new();
    snapshot.insert(
        "schema_version".to_string(),
        Value::String(TRACE_COMPACTION_SNAPSHOT_SCHEMA_VERSION.to_string()),
    );
    snapshot.insert("generation".to_string(), json!(active_generation));
    snapshot.insert(
        "snapshot_id".to_string(),
        Value::String(snapshot_id.clone()),
    );
    snapshot.insert(
        "parent_generation".to_string(),
        parent_snapshot
            .as_ref()
            .and_then(|value| value.get("generation"))
            .cloned()
            .unwrap_or(Value::Null),
    );
    snapshot.insert(
        "parent_snapshot_id".to_string(),
        parent_snapshot
            .as_ref()
            .and_then(|value| value.get("snapshot_id"))
            .cloned()
            .unwrap_or(Value::Null),
    );
    snapshot.insert("run_id".to_string(), Value::String(payload.run_id.clone()));
    snapshot.insert(
        "job_id".to_string(),
        payload
            .job_id
            .as_deref()
            .map(|s| Value::String(s.to_string()))
            .unwrap_or(Value::Null),
    );
    snapshot.insert(
        "created_at".to_string(),
        Value::String(framework_kernel::time::now_iso()),
    );
    snapshot.insert(
        "watermark_event_id".to_string(),
        trace_event_string_field(tail, "event_id")
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    snapshot.insert(
        "state_digest".to_string(),
        Value::String(stable_digest(&state_value)),
    );
    snapshot.insert("artifact_index_ref".to_string(), artifact_index_ref);
    snapshot.insert("state_ref".to_string(), state_ref);
    snapshot.insert(
        "delta_cursor".to_string(),
        trace_event_string_field(tail, "cursor")
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    snapshot.insert(
        "summary".to_string(),
        json!({
            "latest_event_id": trace_event_string_field(tail, "event_id"),
            "latest_seq": trace_event_usize_field(tail, "seq").unwrap_or(0),
            "event_count": active_events.len(),
            "latest_cursor": latest_cursor,
            "kind": trace_event_string_field(tail, "kind"),
            "stage": trace_event_string_field(tail, "stage"),
            "status": trace_event_string_field(tail, "status").unwrap_or_else(|| "ok".to_string()),
        }),
    );
    let snapshot_value = Value::Object(snapshot);
    let snapshot_serialized = pretty_json_line(&snapshot_value)?;
    let next_generation = active_generation + 1;
    let manifest = json!({
        "schema_version": TRACE_COMPACTION_MANIFEST_SCHEMA_VERSION,
        "run_id": payload.run_id,
        "job_id": payload.job_id,
        "backend_family": payload.backend_family.as_deref().unwrap_or("filesystem"),
        "compaction_supported": true,
        "snapshot_delta_supported": true,
        "latest_stable_snapshot": snapshot_value,
        "active_generation": next_generation,
        "active_parent_snapshot_id": snapshot_id,
        "manifest_path": paths.manifest.display().to_string(),
        "snapshot_path": paths.snapshot.display().to_string(),
        "delta_path": paths.deltas.display().to_string(),
        "artifact_index_path": paths.artifact_index.display().to_string(),
        "state_path": paths.state.display().to_string(),
        "updated_at": framework_kernel::time::now_iso(),
    });
    let manifest_serialized = pretty_json_line(&manifest)?;
    let writes = vec![
        TraceTextWrite {
            path: paths.state.display().to_string(),
            payload_text: state_serialized,
        },
        TraceTextWrite {
            path: paths.artifact_index.display().to_string(),
            payload_text: artifact_index_serialized,
        },
        TraceTextWrite {
            path: paths.snapshot.display().to_string(),
            payload_text: snapshot_serialized,
        },
        TraceTextWrite {
            path: paths.deltas.display().to_string(),
            payload_text: String::new(),
        },
        TraceTextWrite {
            path: paths.manifest.display().to_string(),
            payload_text: manifest_serialized,
        },
    ];
    if payload.write_outputs {
        for write in &writes {
            write_text(Path::new(&write.path), &write.payload_text)?;
        }
    }

    Ok(TraceCompactResponsePayload {
        schema_version: TRACE_COMPACTION_RESULT_SCHEMA_VERSION.to_string(),
        authority: TRACE_STREAM_IO_AUTHORITY.to_string(),
        applied: true,
        status: "compacted".to_string(),
        reason: None,
        run_id: manifest
            .get("run_id")
            .or_else(|| manifest.get("session_id"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        job_id: manifest
            .get("job_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        backend_family: payload.backend_family,
        current_generation: active_generation,
        next_generation,
        latest_stable_snapshot: manifest.get("latest_stable_snapshot").cloned(),
        manifest_path: Some(paths.manifest.display().to_string()),
        writes,
    })
}

pub(super) fn load_trace_events_from_text(
    stream_text: &str,
    run_id: Option<&str>,
    job_id: Option<&str>,
) -> Result<Vec<Map<String, Value>>, String> {
    let mut events = Vec::new();
    for (line_number, raw_line) in stream_text.lines().enumerate() {
        if raw_line.trim().is_empty() {
            continue;
        }
        let payload = serde_json::from_str::<Value>(raw_line)
            .map_err(|err| format!("parse trace stream line {} failed: {err}", line_number + 1))?;
        let event = hydrate_trace_event(trace_event_object(payload)?, line_number + 1);
        if trace_event_matches_scope(&event, run_id, job_id) {
            events.push(event);
        }
    }
    Ok(events)
}

pub(super) fn trace_event_matches_scope(
    payload: &Map<String, Value>,
    run_id: Option<&str>,
    job_id: Option<&str>,
) -> bool {
    if let Some(expected_run_id) = run_id {
        let actual_run_id = trace_event_string_field(payload, "run_id")
            .or_else(|| trace_event_string_field(payload, "session_id"));
        if actual_run_id.as_deref() != Some(expected_run_id) {
            return false;
        }
    }
    if let Some(expected_job_id) = job_id
        && trace_event_string_field(payload, "job_id").as_deref() != Some(expected_job_id)
    {
        return false;
    }
    true
}

fn latest_cursor_from_event(payload: &Map<String, Value>) -> Option<Value> {
    let run_id = trace_event_string_field(payload, "run_id")
        .or_else(|| trace_event_string_field(payload, "session_id"))?;
    let seq = trace_event_usize_field(payload, "seq")?;
    let generation = trace_event_usize_field(payload, "generation").unwrap_or(0);
    let event_id = trace_event_string_field(payload, "event_id")?;
    let cursor = trace_event_string_field(payload, "cursor")
        .unwrap_or_else(|| build_trace_cursor(generation, seq, &event_id));
    Some(json!({
        "schema_version": TRACE_REPLAY_CURSOR_SCHEMA_VERSION,
        "run_id": run_id,
        "job_id": trace_event_string_field(payload, "job_id"),
        "generation": generation,
        "seq": seq,
        "event_id": event_id,
        "cursor": cursor,
    }))
}

pub(super) fn stable_digest(value: &Value) -> String {
    let serialized = serde_json::to_string(value).unwrap_or_default();
    sha256_hex(serialized.as_bytes())
}

fn build_artifact_ref(kind: &str, path: &Path, payload: &str, producer: &str) -> Value {
    json!({
        "schema_version": TRACE_COMPACTION_ARTIFACT_REF_SCHEMA_VERSION,
        "artifact_id": build_prefixed_id("art", &format!("{}:{payload}", path.display())),
        "kind": kind,
        "uri": path.display().to_string(),
        "digest": sha256_hex(payload.as_bytes()),
        "size_bytes": payload.len(),
        "created_at": framework_kernel::time::now_iso(),
        "producer": producer,
    })
}

fn build_external_artifact_ref(path: &str) -> Value {
    json!({
        "schema_version": TRACE_COMPACTION_ARTIFACT_REF_SCHEMA_VERSION,
        "artifact_id": build_prefixed_id("art", path),
        "kind": "continuity_artifact",
        "uri": path,
        "digest": sha256_hex(path.as_bytes()),
        "size_bytes": path.len(),
        "created_at": framework_kernel::time::now_iso(),
        "producer": "runtime-trace-recorder-external",
    })
}

struct CompactionPaths {
    manifest: PathBuf,
    snapshot: PathBuf,
    deltas: PathBuf,
    artifact_index: PathBuf,
    state: PathBuf,
}

fn compaction_paths(root_path: &str, run_id: &str, job_id: Option<&str>) -> CompactionPaths {
    let root = PathBuf::from(root_path).join("trace_compaction");
    let artifacts_dir = root.join("artifacts");
    let stream_key = build_compaction_stream_key(run_id, job_id);
    CompactionPaths {
        manifest: root.join(format!("{stream_key}.manifest.json")),
        snapshot: root.join(format!("{stream_key}.snapshot.json")),
        deltas: root.join(format!("{stream_key}.deltas.jsonl")),
        artifact_index: artifacts_dir.join(format!("{stream_key}.artifacts.json")),
        state: artifacts_dir.join(format!("{stream_key}.state.json")),
    }
}

pub(super) fn build_compaction_stream_key(run_id: &str, job_id: Option<&str>) -> String {
    [run_id, job_id.unwrap_or("session")]
        .iter()
        .map(|part| {
            let normalized: String = part
                .chars()
                .map(|ch| {
                    if ch.is_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                        ch
                    } else {
                        '_'
                    }
                })
                .collect();
            if normalized.is_empty() {
                "stream".to_string()
            } else {
                normalized
            }
        })
        .collect::<Vec<_>>()
        .join("__")
}

fn previous_manifest_payload(
    payload: &TraceCompactRequestPayload,
    manifest_path: &Path,
) -> Result<Option<Value>, String> {
    let raw = payload
        .previous_manifest_text
        .as_deref()
        .map(str::to_string)
        .or_else(|| fs::read_to_string(manifest_path).ok());
    raw.map(|value| {
        serde_json::from_str::<Value>(&value).map_err(|err| {
            format!(
                "parse previous compaction manifest failed for {}: {err}",
                manifest_path.display()
            )
        })
    })
    .transpose()
}

pub(super) fn unique_strings(values: &[String]) -> Vec<String> {
    let mut output = Vec::new();
    for value in values {
        if !output.contains(value) {
            output.push(value.clone());
        }
    }
    output
}

fn pretty_json_line(value: &Value) -> Result<String, String> {
    serde_json::to_string_pretty(value)
        .map(|value| value + "\n")
        .map_err(|err| format!("serialize trace payload failed: {err}"))
}

fn write_text(path: &Path, payload: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!("create trace parent failed for {}: {err}", parent.display())
        })?;
    }
    fs::write(path, payload)
        .map_err(|err| format!("write trace payload failed for {}: {err}", path.display()))
}
