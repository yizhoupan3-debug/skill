use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const TRACE_RECORD_EVENT_SCHEMA_VERSION: &str = "router-rs-trace-record-event-v1";
const TRACE_COMPACT_SCHEMA_VERSION: &str = "router-rs-trace-compact-v1";
const TRACE_COMPACTION_RESULT_SCHEMA_VERSION: &str = "runtime-trace-compaction-result-v1";
const TRACE_STREAM_IO_AUTHORITY: &str = "rust-runtime-trace-io";
const TRACE_EVENT_SCHEMA_VERSION: &str = "runtime-trace-v2";
const TRACE_EVENT_SINK_SCHEMA_VERSION: &str = "runtime-trace-sink-v2";
const TRACE_REPLAY_CURSOR_SCHEMA_VERSION: &str = "runtime-trace-cursor-v1";
const TRACE_COMPACTION_SNAPSHOT_SCHEMA_VERSION: &str = "runtime-trace-compaction-snapshot-v1";
const TRACE_COMPACTION_DELTA_SCHEMA_VERSION: &str = "runtime-trace-compaction-delta-v1";
const TRACE_COMPACTION_ARTIFACT_REF_SCHEMA_VERSION: &str = "runtime-trace-artifact-ref-v1";
const TRACE_COMPACTION_MANIFEST_SCHEMA_VERSION: &str = "runtime-trace-compaction-manifest-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceRecordEventRequestPayload {
    pub path: Option<String>,
    #[serde(default = "default_true")]
    pub write_outputs: bool,
    #[serde(default = "default_event_sink_schema_version")]
    pub sink_schema_version: String,
    #[serde(default = "default_event_schema_version")]
    pub event_schema_version: String,
    pub generation: usize,
    pub seq: usize,
    pub session_id: String,
    pub job_id: Option<String>,
    pub kind: String,
    pub stage: String,
    #[serde(default = "default_ok_status")]
    pub status: String,
    #[serde(default)]
    pub payload: Map<String, Value>,
    pub compaction_manifest_path: Option<String>,
    pub compaction_manifest_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceRecordEventResponsePayload {
    pub schema_version: String,
    pub authority: String,
    pub path: Option<String>,
    pub event: Value,
    pub sink_line: String,
    pub bytes_written: usize,
    pub delta_path: Option<String>,
    pub delta_line: Option<String>,
    pub delta_bytes_written: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceCompactRequestPayload {
    pub root_path: String,
    pub event_stream_path: Option<String>,
    pub output_path: Option<String>,
    pub session_id: String,
    pub job_id: Option<String>,
    pub backend_family: Option<String>,
    #[serde(default = "default_true")]
    pub supports_compaction: bool,
    #[serde(default = "default_true")]
    pub supports_snapshot_delta: bool,
    pub current_generation: usize,
    #[serde(default)]
    pub artifact_paths: Vec<String>,
    pub event_stream_text: Option<String>,
    pub output_text: Option<String>,
    pub previous_manifest_text: Option<String>,
    #[serde(default = "default_true")]
    pub write_outputs: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceCompactResponsePayload {
    pub schema_version: String,
    pub authority: String,
    pub applied: bool,
    pub status: String,
    pub reason: Option<String>,
    pub session_id: String,
    pub job_id: Option<String>,
    pub backend_family: Option<String>,
    pub current_generation: usize,
    pub next_generation: usize,
    pub latest_stable_snapshot: Option<Value>,
    pub manifest_path: Option<String>,
    #[serde(default)]
    pub writes: Vec<TraceTextWrite>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceTextWrite {
    pub path: String,
    pub payload_text: String,
}

pub fn record_trace_event(
    payload: TraceRecordEventRequestPayload,
) -> Result<TraceRecordEventResponsePayload, String> {
    let event_id = build_event_id(
        payload.seq,
        &payload.session_id,
        payload.job_id.as_deref(),
        &payload.kind,
    );
    let cursor = build_trace_cursor(payload.generation, payload.seq, &event_id);
    let mut event = Map::new();
    event.insert("event_id".to_string(), Value::String(event_id.clone()));
    event.insert("seq".to_string(), json!(payload.seq));
    event.insert("generation".to_string(), json!(payload.generation));
    event.insert("cursor".to_string(), Value::String(cursor.clone()));
    event.insert("ts".to_string(), Value::String(Utc::now().to_rfc3339()));
    event.insert(
        "session_id".to_string(),
        Value::String(payload.session_id.clone()),
    );
    event.insert(
        "job_id".to_string(),
        payload
            .job_id
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    event.insert("kind".to_string(), Value::String(payload.kind.clone()));
    event.insert("stage".to_string(), Value::String(payload.stage.clone()));
    event.insert("status".to_string(), Value::String(payload.status.clone()));
    event.insert(
        "payload".to_string(),
        Value::Object(payload.payload.clone()),
    );
    event.insert(
        "schema_version".to_string(),
        Value::String(payload.event_schema_version),
    );

    let sink_line = serde_json::to_string(&json!({
        "event": Value::Object(event.clone()),
        "sink_schema_version": payload.sink_schema_version,
    }))
    .map_err(|err| format!("serialize trace event sink line failed: {err}"))?
        + "\n";
    if payload.write_outputs {
        if let Some(path) = payload.path.as_deref() {
            append_text(Path::new(path), &sink_line)?;
        }
    }

    let (delta_path, delta_line, delta_bytes_written) = maybe_append_compaction_delta(
        &Value::Object(event.clone()),
        payload.compaction_manifest_path.as_deref(),
        payload.compaction_manifest_text.as_deref(),
        payload.write_outputs,
    )?;

    Ok(TraceRecordEventResponsePayload {
        schema_version: TRACE_RECORD_EVENT_SCHEMA_VERSION.to_string(),
        authority: TRACE_STREAM_IO_AUTHORITY.to_string(),
        path: payload.path,
        event: Value::Object(event),
        bytes_written: sink_line.len(),
        sink_line,
        delta_path,
        delta_line,
        delta_bytes_written,
    })
}

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
            session_id: payload.session_id,
            job_id: payload.job_id,
            backend_family: payload.backend_family,
            current_generation: payload.current_generation,
            next_generation: payload.current_generation,
            latest_stable_snapshot: None,
            manifest_path: None,
            writes: Vec::new(),
        });
    }
    let stream_text = match payload.event_stream_text.clone() {
        Some(value) => value,
        None => match payload.event_stream_path.as_deref() {
            Some(path) if Path::new(path).exists() => fs::read_to_string(path)
                .map_err(|err| format!("read trace stream failed for {path}: {err}"))?,
            _ => String::new(),
        },
    };
    let source_events = load_trace_events_from_text(
        &stream_text,
        Some(&payload.session_id),
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
            session_id: payload.session_id,
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
        &payload.session_id,
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
    state_payload.insert(
        "session_id".to_string(),
        Value::String(payload.session_id.clone()),
    );
    state_payload.insert(
        "job_id".to_string(),
        payload
            .job_id
            .clone()
            .map(Value::String)
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
    let state_value = Value::Object(state_payload.clone());
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
            .clone()
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
    snapshot.insert(
        "session_id".to_string(),
        Value::String(payload.session_id.clone()),
    );
    snapshot.insert(
        "job_id".to_string(),
        payload
            .job_id
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    snapshot.insert(
        "created_at".to_string(),
        Value::String(Utc::now().to_rfc3339()),
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
        "session_id": payload.session_id,
        "job_id": payload.job_id,
        "backend_family": payload.backend_family.clone().unwrap_or_else(|| "filesystem".to_string()),
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
        "updated_at": Utc::now().to_rfc3339(),
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
        session_id: manifest
            .get("session_id")
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

fn maybe_append_compaction_delta(
    event: &Value,
    manifest_path: Option<&str>,
    manifest_text: Option<&str>,
    write_outputs: bool,
) -> Result<(Option<String>, Option<String>, usize), String> {
    if manifest_path.is_none() && manifest_text.is_none() {
        return Ok((None, None, 0));
    }
    let manifest_payload = manifest_text
        .map(str::to_string)
        .or_else(|| manifest_path.and_then(|path| fs::read_to_string(path).ok()))
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok());
    let Some(manifest) = manifest_payload.and_then(|value| value.as_object().cloned()) else {
        return Ok((None, None, 0));
    };
    let active_generation = manifest
        .get("active_generation")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(0);
    let event_object = event
        .as_object()
        .ok_or_else(|| "trace event must be an object".to_string())?;
    if trace_event_usize_field(event_object, "generation").unwrap_or(0) != active_generation {
        return Ok((None, None, 0));
    }
    let Some(parent_snapshot_id) = manifest
        .get("active_parent_snapshot_id")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return Ok((None, None, 0));
    };
    let Some(delta_path) = manifest.get("delta_path").and_then(Value::as_str) else {
        return Ok((None, None, 0));
    };
    let event_id = trace_event_string_field(event_object, "event_id").unwrap_or_default();
    let delta = json!({
        "schema_version": TRACE_COMPACTION_DELTA_SCHEMA_VERSION,
        "generation": active_generation,
        "delta_id": build_prefixed_id("delta", &event_id),
        "parent_snapshot_id": parent_snapshot_id,
        "seq": trace_event_usize_field(event_object, "seq").unwrap_or(0),
        "ts": trace_event_string_field(event_object, "ts").unwrap_or_default(),
        "kind": trace_event_string_field(event_object, "kind").unwrap_or_default(),
        "payload": {
            "event_id": event_id,
            "cursor": trace_event_string_field(event_object, "cursor").unwrap_or_default(),
            "stage": trace_event_string_field(event_object, "stage").unwrap_or_else(|| "background".to_string()),
            "status": trace_event_string_field(event_object, "status").unwrap_or_else(|| "ok".to_string()),
            "payload": event_object.get("payload").cloned().unwrap_or_else(|| json!({})),
        },
        "artifact_refs": [],
        "applies_to": {
            "session_id": trace_event_string_field(event_object, "session_id").unwrap_or_default(),
            "job_id": event_object.get("job_id").cloned().unwrap_or(Value::Null),
        },
    });
    let delta_line = serde_json::to_string(&delta)
        .map_err(|err| format!("serialize trace compaction delta failed: {err}"))?
        + "\n";
    if write_outputs {
        append_text(Path::new(delta_path), &delta_line)?;
    }
    let bytes = delta_line.len();
    Ok((Some(delta_path.to_string()), Some(delta_line), bytes))
}

fn load_trace_events_from_text(
    stream_text: &str,
    session_id: Option<&str>,
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
        if trace_event_matches_scope(&event, session_id, job_id) {
            events.push(event);
        }
    }
    Ok(events)
}

fn trace_event_object(payload: Value) -> Result<Map<String, Value>, String> {
    match payload {
        Value::Object(mut object) => match object.remove("event") {
            Some(Value::Object(event)) => Ok(event),
            Some(other) => Err(format!(
                "trace stream line contained non-object event wrapper: {other}"
            )),
            None => Ok(object),
        },
        other => Err(format!(
            "trace stream line must decode to a JSON object: {other}"
        )),
    }
}

fn hydrate_trace_event(mut payload: Map<String, Value>, line_number: usize) -> Map<String, Value> {
    let seq = trace_event_usize_field(&payload, "seq").unwrap_or(line_number);
    let generation = trace_event_usize_field(&payload, "generation").unwrap_or(0);
    let event_id = trace_event_string_field(&payload, "event_id")
        .unwrap_or_else(|| format!("evt_replay_{line_number:06}"));
    let cursor = trace_event_string_field(&payload, "cursor")
        .unwrap_or_else(|| build_trace_cursor(generation, seq, &event_id));
    payload
        .entry("seq".to_string())
        .or_insert_with(|| json!(seq));
    payload
        .entry("generation".to_string())
        .or_insert_with(|| json!(generation));
    payload
        .entry("event_id".to_string())
        .or_insert_with(|| Value::String(event_id));
    payload
        .entry("cursor".to_string())
        .or_insert_with(|| Value::String(cursor));
    payload
        .entry("status".to_string())
        .or_insert_with(|| Value::String("ok".to_string()));
    payload
        .entry("schema_version".to_string())
        .or_insert_with(|| Value::String(TRACE_EVENT_SCHEMA_VERSION.to_string()));
    payload
}

fn trace_event_matches_scope(
    payload: &Map<String, Value>,
    session_id: Option<&str>,
    job_id: Option<&str>,
) -> bool {
    if let Some(expected_session_id) = session_id {
        if trace_event_string_field(payload, "session_id").as_deref() != Some(expected_session_id) {
            return false;
        }
    }
    if let Some(expected_job_id) = job_id {
        if trace_event_string_field(payload, "job_id").as_deref() != Some(expected_job_id) {
            return false;
        }
    }
    true
}

fn latest_cursor_from_event(payload: &Map<String, Value>) -> Option<Value> {
    let session_id = trace_event_string_field(payload, "session_id")?;
    let seq = trace_event_usize_field(payload, "seq")?;
    let generation = trace_event_usize_field(payload, "generation").unwrap_or(0);
    let event_id = trace_event_string_field(payload, "event_id")?;
    let cursor = trace_event_string_field(payload, "cursor")
        .unwrap_or_else(|| build_trace_cursor(generation, seq, &event_id));
    Some(json!({
        "schema_version": TRACE_REPLAY_CURSOR_SCHEMA_VERSION,
        "session_id": session_id,
        "job_id": trace_event_string_field(payload, "job_id"),
        "generation": generation,
        "seq": seq,
        "event_id": event_id,
        "cursor": cursor,
    }))
}

fn trace_event_string_field(payload: &Map<String, Value>, field: &str) -> Option<String> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn trace_event_usize_field(payload: &Map<String, Value>, field: &str) -> Option<usize> {
    payload
        .get(field)
        .and_then(|value| value.as_u64().map(|number| number as usize))
}

fn build_trace_cursor(generation: usize, seq: usize, event_id: &str) -> String {
    format!("g{generation}:s{seq}:{event_id}")
}

fn build_event_id(seq: usize, session_id: &str, job_id: Option<&str>, kind: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let seed = format!("{nanos}:{seq}:{session_id}:{}:{kind}", job_id.unwrap_or(""));
    build_prefixed_id("evt", &seed)
}

fn build_prefixed_id(prefix: &str, seed: &str) -> String {
    let digest = sha256_hex(seed.as_bytes());
    format!("{prefix}_{}", &digest[..12])
}

fn stable_digest(value: &Value) -> String {
    let serialized = serde_json::to_string(value).unwrap_or_default();
    sha256_hex(serialized.as_bytes())
}

fn sha256_hex(payload: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(payload);
    format!("{:x}", hasher.finalize())
}

fn build_artifact_ref(kind: &str, path: &Path, payload: &str, producer: &str) -> Value {
    json!({
        "schema_version": TRACE_COMPACTION_ARTIFACT_REF_SCHEMA_VERSION,
        "artifact_id": build_prefixed_id("art", &format!("{}:{payload}", path.display())),
        "kind": kind,
        "uri": path.display().to_string(),
        "digest": sha256_hex(payload.as_bytes()),
        "size_bytes": payload.len(),
        "created_at": Utc::now().to_rfc3339(),
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
        "created_at": Utc::now().to_rfc3339(),
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

fn compaction_paths(root_path: &str, session_id: &str, job_id: Option<&str>) -> CompactionPaths {
    let root = PathBuf::from(root_path).join("trace_compaction");
    let artifacts_dir = root.join("artifacts");
    let stream_key = build_compaction_stream_key(session_id, job_id);
    CompactionPaths {
        manifest: root.join(format!("{stream_key}.manifest.json")),
        snapshot: root.join(format!("{stream_key}.snapshot.json")),
        deltas: root.join(format!("{stream_key}.deltas.jsonl")),
        artifact_index: artifacts_dir.join(format!("{stream_key}.artifacts.json")),
        state: artifacts_dir.join(format!("{stream_key}.state.json")),
    }
}

fn build_compaction_stream_key(session_id: &str, job_id: Option<&str>) -> String {
    [session_id, job_id.unwrap_or("session")]
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
        .clone()
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

fn unique_strings(values: &[String]) -> Vec<String> {
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
        fs::create_dir_all(parent)
            .map_err(|err| format!("create trace parent failed for {}: {err}", parent.display()))?;
    }
    fs::write(path, payload)
        .map_err(|err| format!("write trace payload failed for {}: {err}", path.display()))
}

fn append_text(path: &Path, payload: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create trace parent failed for {}: {err}", parent.display()))?;
    }
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| format!("open trace append failed for {}: {err}", path.display()))?;
    file.write_all(payload.as_bytes())
        .map_err(|err| format!("append trace payload failed for {}: {err}", path.display()))
}

fn default_true() -> bool {
    true
}

fn default_event_sink_schema_version() -> String {
    TRACE_EVENT_SINK_SCHEMA_VERSION.to_string()
}

fn default_event_schema_version() -> String {
    TRACE_EVENT_SCHEMA_VERSION.to_string()
}

fn default_ok_status() -> String {
    "ok".to_string()
}
