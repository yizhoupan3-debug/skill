use crate::TraceError;
use rt_storage::runtime_storage::acquire_runtime_path_lock;
use serde_json::{Map, Value, json};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use super::util::{build_prefixed_id, build_trace_cursor};
use super::{
    TRACE_RECORD_EVENT_SCHEMA_VERSION, TRACE_STREAM_IO_AUTHORITY, TraceRecordEventRequestPayload,
    TraceRecordEventResponsePayload,
};

#[tracing::instrument(level = "debug", skip_all)]
pub fn record_trace_event(
    payload: TraceRecordEventRequestPayload,
) -> Result<TraceRecordEventResponsePayload, TraceError> {
    let event_id = build_event_id(
        payload.seq,
        &payload.run_id,
        payload.job_id.as_deref(),
        &payload.kind,
    );
    let page_token = build_trace_cursor(payload.generation, payload.seq, &event_id);
    let mut event = Map::new();
    event.insert("event_id".to_string(), Value::String(event_id.clone()));
    event.insert("seq".to_string(), json!(payload.seq));
    event.insert("generation".to_string(), json!(payload.generation));
    event.insert("page_token".to_string(), Value::String(page_token));
    event.insert(
        "ts".to_string(),
        Value::String(framework_core::time::now_iso()),
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
    .map_err(|err| {
        TraceError::validation(format!("serialize trace event sink line failed: {err}"))
    })? + "\n";
    if payload.write_outputs
        && let Some(path) = payload.path.as_deref()
    {
        if let Err(err) = append_text(Path::new(path), &sink_line) {
            tracing::error!(event_id = %event_id, kind = %payload.kind, path = %path, "append trace sink line failed: {err}");
            return Err(err);
        }
    }

    tracing::debug!(event_id = %event_id, kind = %payload.kind, bytes_written = %sink_line.len(), "trace event recorded");

    Ok(TraceRecordEventResponsePayload {
        schema_version: TRACE_RECORD_EVENT_SCHEMA_VERSION.to_string(),
        authority: TRACE_STREAM_IO_AUTHORITY.to_string(),
        path: payload.path,
        event: event_value,
        bytes_written: sink_line.len(),
        sink_line,
    })
}


fn build_event_id(seq: usize, run_id: &str, job_id: Option<&str>, kind: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let seed = format!("{nanos}:{seq}:{run_id}:{}:{kind}", job_id.unwrap_or(""));
    build_prefixed_id("evt", &seed)
}

fn append_text(path: &Path, payload: &str) -> Result<(), TraceError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            TraceError::validation(format!(
                "create trace parent failed for {}: {err}",
                parent.display()
            ))
        })?;
    }
    // Within-process serialization (cheap fast path).
    let _proc_guard = trace_append_lock().lock().map_err(|e| {
        tracing::error!("[router-rs] trace append lock poisoned: {e}");
        TraceError::Poisoned("trace append lock".to_string())
    })?;
    // Cross-process serialization: prevents JSONL line interleaving when
    // codex, cursor and parallel test harnesses all tail the same trace.
    // Note: this is the 2nd of 3 serialization layers — (1) process Mutex,
    // (2) cross-process file lock, (3) OS append-mode write atomicity.
    // The ordering is safe: path lock is acquired AFTER the Mutex so there
    // is no lock-order inversion risk (both are write-only, no back-edge).
    let _path_lock =
        acquire_runtime_path_lock(path).map_err(|e| TraceError::validation(e.to_string()))?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| {
            TraceError::validation(format!(
                "open trace append failed for {}: {err}",
                path.display()
            ))
        })?;
    file.write_all(payload.as_bytes()).map_err(|err| {
        TraceError::validation(format!(
            "append trace payload failed for {}: {err}",
            path.display()
        ))
    })?;
    file.sync_data().map_err(|err| {
        TraceError::validation(format!(
            "sync trace payload failed for {}: {err}",
            path.display()
        ))
    })
}

fn trace_append_lock() -> &'static Mutex<()> {
    static TRACE_APPEND_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    TRACE_APPEND_LOCK.get_or_init(|| Mutex::new(()))
}
