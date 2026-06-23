use rt_storage::runtime_storage::acquire_runtime_path_lock;
use serde_json::{Map, Value, json};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    TRACE_COMPACTION_DELTA_SCHEMA_VERSION, TRACE_RECORD_EVENT_SCHEMA_VERSION,
    TRACE_STREAM_IO_AUTHORITY, TraceRecordEventRequestPayload, TraceRecordEventResponsePayload,
};
use super::util::build_prefixed_id;
use super::util::{
    build_trace_cursor, trace_event_string_field, trace_event_usize_field,
};

#[tracing::instrument(level = "debug", skip_all)]
pub fn record_trace_event(
    payload: TraceRecordEventRequestPayload,
) -> Result<TraceRecordEventResponsePayload, String> {
    let event_id = build_event_id(
        payload.seq,
        &payload.run_id,
        payload.job_id.as_deref(),
        &payload.kind,
    );
    let cursor = build_trace_cursor(payload.generation, payload.seq, &event_id);
    let mut event = Map::new();
    event.insert("event_id".to_string(), Value::String(event_id));
    event.insert("seq".to_string(), json!(payload.seq));
    event.insert("generation".to_string(), json!(payload.generation));
    event.insert("cursor".to_string(), Value::String(cursor));
    event.insert(
        "ts".to_string(),
        Value::String(framework_kernel::time::now_iso()),
    );
    event.insert("run_id".to_string(), Value::String(payload.run_id.clone()));
    event.insert(
        "job_id".to_string(),
        payload
            .job_id
            .as_deref()
            .map(|s| Value::String(s.to_string()))
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

    let event_value = Value::Object(event);
    let sink_line = serde_json::to_string(&json!({
        "event": &event_value,
        "sink_schema_version": payload.sink_schema_version,
    }))
    .map_err(|err| format!("serialize trace event sink line failed: {err}"))?
        + "\n";
    if payload.write_outputs
        && let Some(path) = payload.path.as_deref()
    {
        append_text(Path::new(path), &sink_line)?;
    }

    let (delta_path, delta_line, delta_bytes_written) = maybe_append_compaction_delta(
        &event_value,
        payload.compaction_manifest_path.as_deref(),
        payload.compaction_manifest_text.as_deref(),
        payload.write_outputs,
    )?;

    Ok(TraceRecordEventResponsePayload {
        schema_version: TRACE_RECORD_EVENT_SCHEMA_VERSION.to_string(),
        authority: TRACE_STREAM_IO_AUTHORITY.to_string(),
        path: payload.path,
        event: event_value,
        bytes_written: sink_line.len(),
        sink_line,
        delta_path,
        delta_line,
        delta_bytes_written,
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
            "run_id": trace_event_string_field(event_object, "run_id").or_else(|| trace_event_string_field(event_object, "session_id")).unwrap_or_default(),
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

fn build_event_id(seq: usize, run_id: &str, job_id: Option<&str>, kind: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let seed = format!("{nanos}:{seq}:{run_id}:{}:{kind}", job_id.unwrap_or(""));
    build_prefixed_id("evt", &seed)
}

fn append_text(path: &Path, payload: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!("create trace parent failed for {}: {err}", parent.display())
        })?;
    }
    // Within-process serialization (cheap fast path).
    let _proc_guard = trace_append_lock().lock().map_err(|e| {
        eprintln!("[router-rs] trace append lock poisoned: {e}");
        "trace append lock poisoned".to_string()
    })?;
    // Cross-process serialization: prevents JSONL line interleaving when
    // codex, cursor and parallel test harnesses all tail the same trace.
    let _path_lock = acquire_runtime_path_lock(path)?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| format!("open trace append failed for {}: {err}", path.display()))?;
    file.write_all(payload.as_bytes())
        .map_err(|err| format!("append trace payload failed for {}: {err}", path.display()))?;
    file.sync_data()
        .map_err(|err| format!("sync trace payload failed for {}: {err}", path.display()))
}

fn trace_append_lock() -> &'static Mutex<()> {
    static TRACE_APPEND_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    TRACE_APPEND_LOCK.get_or_init(|| Mutex::new(()))
}
