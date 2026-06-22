//! Trace stream replay / inspect / metadata I/O.

use chrono::Utc;
use serde_json::{Map, Value, json};
use std::fs;
use std::path::{Path, PathBuf};

use super::io_utils::{append_text_with_process_lock, validate_write_path};
use rt_storage::runtime_envelope_ids::{
    TRACE_COMPACTION_DELTA_WRITE_SCHEMA_VERSION, TRACE_METADATA_WRITE_AUTHORITY,
    TRACE_METADATA_WRITE_SCHEMA_VERSION, TRACE_STREAM_INSPECT_SCHEMA_VERSION,
    TRACE_STREAM_IO_AUTHORITY, TRACE_STREAM_REPLAY_SCHEMA_VERSION,
};
use rt_storage::runtime_storage::{resolve_storage_backend, storage_artifact_exists, storage_read_text};
use framework_kernel::stdio_payload_types::{
    TraceCompactionDeltaWriteRequestPayload, TraceCompactionDeltaWriteResponsePayload,
    TraceMetadataWriteRequestPayload, TraceMetadataWriteResponsePayload,
    TraceStreamInspectRequestPayload, TraceStreamInspectResponsePayload,
    TraceStreamReplayCursorPayload, TraceStreamReplayRequestPayload,
    TraceStreamReplayResponsePayload,
};
use trace_runtime::{
    build_trace_cursor, hydrate_trace_event, trace_event_object, trace_event_string_field,
    trace_event_usize_field,
};

/// Hydrate a replayed trace event, overriding schema_version to the
/// framework-runtime canonical value.
fn hydrate_trace_event_object(
    payload: Map<String, Value>,
    line_number: usize,
) -> Map<String, Value> {
    let mut event = hydrate_trace_event(payload, line_number);
    event.insert(
        "schema_version".to_string(),
        Value::String("runtime-trace-v2".to_string()),
    );
    event
}

fn trace_event_matches_scope(
    payload: &Map<String, Value>,
    session_id: Option<&str>,
    job_id: Option<&str>,
    stream_scope_fields: Option<&[String]>,
) -> bool {
    let session_scoped = stream_scope_fields
        .map(|fields| fields.iter().any(|field| field == "session_id"))
        .unwrap_or(true);
    let job_scoped = stream_scope_fields
        .map(|fields| fields.iter().any(|field| field == "job_id"))
        .unwrap_or(true);
    if session_scoped
        && let Some(expected_session_id) = session_id
            && trace_event_string_field(payload, "session_id").as_deref()
                != Some(expected_session_id)
            {
                return false;
            }
    if job_scoped
        && let Some(expected_job_id) = job_id
            && trace_event_string_field(payload, "job_id").as_deref() != Some(expected_job_id) {
                return false;
            }
    true
}

fn trace_scope_fields(payload: &Option<Vec<String>>) -> Option<&[String]> {
    payload.as_deref().filter(|fields| !fields.is_empty())
}

fn trace_event_matches_request_scope(
    payload: &Map<String, Value>,
    session_id: Option<&str>,
    job_id: Option<&str>,
    stream_scope_fields: &Option<Vec<String>>,
) -> bool {
    trace_event_matches_scope(
        payload,
        session_id,
        job_id,
        trace_scope_fields(stream_scope_fields),
    )
}

fn load_trace_stream_events(
    path: &Path,
    event_stream_text: Option<&str>,
    session_id: Option<&str>,
    job_id: Option<&str>,
    stream_scope_fields: &Option<Vec<String>>,
) -> Result<Vec<Map<String, Value>>, String> {
    let mut events = Vec::new();
    let raw_payload = match event_stream_text {
        Some(value) => value.to_string(),
        None => {
            let storage_backend = resolve_storage_backend(&[path.to_path_buf()]);
            storage_read_text(path, storage_backend.as_ref())?
        }
    };

    for (line_number, raw_line) in raw_payload.lines().enumerate() {
        if raw_line.trim().is_empty() {
            continue;
        }
        let event_payload = hydrate_trace_event_object(
            trace_event_object(serde_json::from_str::<Value>(raw_line).map_err(|err| {
                format!("parse trace stream line {} failed: {err}", line_number + 1)
            })?)?,
            line_number + 1,
        );
        if trace_event_matches_request_scope(
            &event_payload,
            session_id,
            job_id,
            stream_scope_fields,
        ) {
            events.push(event_payload);
        }
    }
    Ok(events)
}

fn latest_cursor_from_trace_event(payload: &Map<String, Value>) -> Option<Value> {
    let session_id = trace_event_string_field(payload, "session_id")?;
    let seq = trace_event_usize_field(payload, "seq")?;
    let generation = trace_event_usize_field(payload, "generation").unwrap_or(0);
    let event_id = trace_event_string_field(payload, "event_id")?;
    let cursor = trace_event_string_field(payload, "cursor")
        .unwrap_or_else(|| build_trace_cursor(generation, seq, &event_id));
    Some(json!({
        "schema_version": "runtime-trace-cursor-v1",
        "session_id": session_id,
        "job_id": trace_event_string_field(payload, "job_id"),
        "generation": generation,
        "seq": seq,
        "event_id": event_id,
        "cursor": cursor,
    }))
}

fn compaction_delta_to_trace_event(
    payload: Value,
    line_number: usize,
) -> Result<Map<String, Value>, String> {
    let object = match payload {
        Value::Object(obj) => obj,
        _ => {
            return Err(format!(
                "trace compaction delta line {} must decode to a JSON object",
                line_number
            ));
        }
    };
    let generation = trace_event_usize_field(&object, "generation").unwrap_or(0);
    let seq = trace_event_usize_field(&object, "seq")
        .ok_or_else(|| format!("trace compaction delta line {line_number} missing seq"))?;
    let applies_to = object
        .get("applies_to")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("trace compaction delta line {line_number} missing applies_to"))?;
    let session_id = applies_to
        .get("session_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!("trace compaction delta line {line_number} missing applies_to.session_id")
        })?;
    let payload_object = object
        .get("payload")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("trace compaction delta line {line_number} missing payload"))?;
    let event_id = payload_object
        .get("event_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("evt_replay_{line_number:06}"));
    let cursor = payload_object
        .get("cursor")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| build_trace_cursor(generation, seq, &event_id));

    let mut event = Map::new();
    event.insert("event_id".to_string(), Value::String(event_id));
    event.insert("seq".to_string(), json!(seq));
    event.insert("generation".to_string(), json!(generation));
    event.insert("cursor".to_string(), Value::String(cursor));
    event.insert(
        "ts".to_string(),
        object
            .get("ts")
            .cloned()
            .unwrap_or_else(|| Value::String(String::new())),
    );
    event.insert(
        "session_id".to_string(),
        Value::String(session_id.to_string()),
    );
    event.insert(
        "job_id".to_string(),
        applies_to.get("job_id").cloned().unwrap_or(Value::Null),
    );
    event.insert(
        "kind".to_string(),
        object
            .get("kind")
            .cloned()
            .unwrap_or_else(|| Value::String(String::new())),
    );
    event.insert(
        "stage".to_string(),
        payload_object
            .get("stage")
            .cloned()
            .unwrap_or_else(|| Value::String("background".to_string())),
    );
    event.insert(
        "status".to_string(),
        payload_object
            .get("status")
            .cloned()
            .unwrap_or_else(|| Value::String("ok".to_string())),
    );
    event.insert(
        "payload".to_string(),
        payload_object
            .get("payload")
            .cloned()
            .unwrap_or_else(|| json!({})),
    );
    event.insert(
        "schema_version".to_string(),
        Value::String("runtime-trace-v2".to_string()),
    );
    Ok(event)
}

fn validate_compaction_artifact_digest(
    artifact_ref: &Map<String, Value>,
    payload_text: &str,
    label: &str,
) -> Result<(), String> {
    let expected = artifact_ref
        .get("digest")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("compaction {label} artifact ref is missing digest"))?;
    let actual = trace_runtime::sha256_hex(payload_text.as_bytes());
    if expected != actual {
        return Err(format!(
            "Compaction recovery failed closed because {label} artifact digest mismatched."
        ));
    }
    Ok(())
}

// sha256_hex is re-exported from trace-runtime via lib.rs.

struct ResolvedTraceSource {
    path: PathBuf,
    source_kind: &'static str,
    events: Vec<Map<String, Value>>,
    latest_cursor: Option<Value>,
    latest_event_id: Option<String>,
    latest_event_kind: Option<String>,
    latest_event_timestamp: Option<String>,
    recovery: Option<Value>,
}

struct TraceSourceRequest<'a> {
    path: Option<&'a str>,
    event_stream_text: Option<&'a str>,
    compaction_manifest_path: Option<&'a str>,
    compaction_manifest_text: Option<&'a str>,
    compaction_state_text: Option<&'a str>,
    compaction_artifact_index_text: Option<&'a str>,
    compaction_delta_text: Option<&'a str>,
    session_id: Option<&'a str>,
    job_id: Option<&'a str>,
    stream_scope_fields: &'a Option<Vec<String>>,
}

struct CompactionRecoveryRequest<'a> {
    manifest_path: &'a Path,
    manifest_text: Option<&'a str>,
    state_text: Option<&'a str>,
    artifact_index_text: Option<&'a str>,
    delta_text: Option<&'a str>,
    session_id: Option<&'a str>,
    job_id: Option<&'a str>,
    stream_scope_fields: &'a Option<Vec<String>>,
}

impl<'a> TraceSourceRequest<'a> {
    fn from_metadata_payload(payload: &'a TraceMetadataWriteRequestPayload) -> Self {
        Self {
            path: payload.event_stream_path.as_deref(),
            event_stream_text: payload.event_stream_text.as_deref(),
            compaction_manifest_path: payload.compaction_manifest_path.as_deref(),
            compaction_manifest_text: payload.compaction_manifest_text.as_deref(),
            compaction_state_text: payload.compaction_state_text.as_deref(),
            compaction_artifact_index_text: payload.compaction_artifact_index_text.as_deref(),
            compaction_delta_text: payload.compaction_delta_text.as_deref(),
            session_id: payload.session_id.as_deref(),
            job_id: payload.job_id.as_deref(),
            stream_scope_fields: &payload.stream_scope_fields,
        }
    }

    fn from_inspect_payload(payload: &'a TraceStreamInspectRequestPayload) -> Self {
        Self {
            path: payload.path.as_deref(),
            event_stream_text: payload.event_stream_text.as_deref(),
            compaction_manifest_path: payload.compaction_manifest_path.as_deref(),
            compaction_manifest_text: payload.compaction_manifest_text.as_deref(),
            compaction_state_text: payload.compaction_state_text.as_deref(),
            compaction_artifact_index_text: payload.compaction_artifact_index_text.as_deref(),
            compaction_delta_text: payload.compaction_delta_text.as_deref(),
            session_id: payload.session_id.as_deref(),
            job_id: payload.job_id.as_deref(),
            stream_scope_fields: &payload.stream_scope_fields,
        }
    }

    fn from_replay_payload(payload: &'a TraceStreamReplayRequestPayload) -> Self {
        Self {
            path: payload.path.as_deref(),
            event_stream_text: payload.event_stream_text.as_deref(),
            compaction_manifest_path: payload.compaction_manifest_path.as_deref(),
            compaction_manifest_text: payload.compaction_manifest_text.as_deref(),
            compaction_state_text: payload.compaction_state_text.as_deref(),
            compaction_artifact_index_text: payload.compaction_artifact_index_text.as_deref(),
            compaction_delta_text: payload.compaction_delta_text.as_deref(),
            session_id: payload.session_id.as_deref(),
            job_id: payload.job_id.as_deref(),
            stream_scope_fields: &payload.stream_scope_fields,
        }
    }
}

fn load_compaction_recovery(
    request: CompactionRecoveryRequest<'_>,
) -> Result<ResolvedTraceSource, String> {
    let CompactionRecoveryRequest {
        manifest_path,
        manifest_text,
        state_text,
        artifact_index_text,
        delta_text,
        session_id,
        job_id,
        stream_scope_fields,
    } = request;
    let storage_backend = resolve_storage_backend(&[manifest_path.to_path_buf()]);
    let manifest_raw = match manifest_text {
        Some(value) => value.to_string(),
        None => storage_read_text(manifest_path, storage_backend.as_ref())?,
    };
    let manifest_payload = serde_json::from_str::<Value>(&manifest_raw).map_err(|err| {
        format!(
            "parse compaction manifest failed for {}: {err}",
            manifest_path.display()
        )
    })?;
    let manifest = manifest_payload.as_object().ok_or_else(|| {
        format!(
            "compaction manifest must decode to a JSON object: {}",
            manifest_path.display()
        )
    })?;
    let snapshot = manifest
        .get("latest_stable_snapshot")
        .and_then(Value::as_object)
        .ok_or_else(|| "compaction manifest is missing latest_stable_snapshot".to_string())?;
    let state_ref = snapshot
        .get("state_ref")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "compaction manifest is missing required recovery artifact refs.".to_string()
        })?;
    let artifact_index_ref = snapshot
        .get("artifact_index_ref")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "compaction manifest is missing required recovery artifact refs.".to_string()
        })?;
    let state_ref_uri = state_ref
        .get("uri")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "compaction manifest is missing required recovery artifact refs.".to_string()
        })?;
    let artifact_index_uri = artifact_index_ref
        .get("uri")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "compaction manifest is missing required recovery artifact refs.".to_string()
        })?;
    let state_path = PathBuf::from(state_ref_uri);
    let artifact_index_path = PathBuf::from(artifact_index_uri);
    if state_text.is_none()
        && artifact_index_text.is_none()
        && (!storage_artifact_exists(&state_path, storage_backend.as_ref())
            || !storage_artifact_exists(&artifact_index_path, storage_backend.as_ref()))
    {
        return Err(
            "Compaction recovery failed closed because a referenced artifact is missing."
                .to_string(),
        );
    }
    let state_raw = match state_text {
        Some(value) => value.to_string(),
        None => storage_read_text(&state_path, storage_backend.as_ref())?,
    };
    validate_compaction_artifact_digest(state_ref, &state_raw, "state_ref")?;
    let state_payload = serde_json::from_str::<Value>(&state_raw).map_err(|err| {
        format!(
            "parse compaction state failed for {}: {err}",
            state_path.display()
        )
    })?;
    let artifact_index_raw = match artifact_index_text {
        Some(value) => value.to_string(),
        None => storage_read_text(&artifact_index_path, storage_backend.as_ref())?,
    };
    validate_compaction_artifact_digest(
        artifact_index_ref,
        &artifact_index_raw,
        "artifact_index_ref",
    )?;
    let artifact_index_payload =
        serde_json::from_str::<Value>(&artifact_index_raw).map_err(|err| {
            format!(
                "parse compaction artifact index failed for {}: {err}",
                artifact_index_path.display()
            )
        })?;

    let delta_path = manifest
        .get("delta_path")
        .and_then(Value::as_str)
        .map(PathBuf::from);
    let mut deltas = Vec::new();
    let mut events = Vec::new();
    if let Some(delta_path) = delta_path.as_ref() {
        let raw_delta_payload = delta_text.map(str::to_string).or_else(|| {
            if storage_artifact_exists(delta_path, storage_backend.as_ref()) {
                storage_read_text(delta_path, storage_backend.as_ref()).ok()
            } else {
                None
            }
        });
        if let Some(raw_delta_payload) = raw_delta_payload {
            for (line_number, raw_line) in raw_delta_payload.lines().enumerate() {
                if raw_line.trim().is_empty() {
                    continue;
                }
                let delta_payload = serde_json::from_str::<Value>(raw_line).map_err(|err| {
                    format!(
                        "parse compaction delta line {} failed: {err}",
                        line_number + 1
                    )
                })?;
                let event_payload =
                    compaction_delta_to_trace_event(delta_payload.clone(), line_number + 1)?;
                if trace_event_matches_request_scope(
                    &event_payload,
                    session_id,
                    job_id,
                    stream_scope_fields,
                ) {
                    deltas.push(delta_payload);
                    events.push(event_payload);
                }
            }
        }
    }

    let latest_cursor = events
        .last()
        .and_then(latest_cursor_from_trace_event)
        .or_else(|| state_payload.get("latest_cursor").cloned());
    let latest_event = events.last().cloned().or_else(|| {
        state_payload
            .get("latest_event")
            .and_then(Value::as_object)
            .cloned()
    });
    let latest_event_id = latest_cursor
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|payload| payload.get("event_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            latest_event
                .as_ref()
                .and_then(|payload| trace_event_string_field(payload, "event_id"))
        });
    let latest_event_kind = latest_event
        .as_ref()
        .and_then(|payload| trace_event_string_field(payload, "kind"));
    let latest_event_timestamp = latest_event
        .as_ref()
        .and_then(|payload| trace_event_string_field(payload, "ts"));
    let latest_recoverable_generation = latest_cursor
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|payload| payload.get("generation"))
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .or_else(|| {
            manifest
                .get("active_generation")
                .and_then(Value::as_u64)
                .map(|value| value as usize)
        })
        .unwrap_or(0);
    let recovery = Some(json!({
        "schema_version": "runtime-trace-compaction-recovery-v1",
        "session_id": manifest.get("session_id").cloned().unwrap_or(Value::Null),
        "job_id": manifest.get("job_id").cloned().unwrap_or(Value::Null),
        "latest_recoverable_generation": latest_recoverable_generation,
        "snapshot": Value::Object(snapshot.clone()),
        "deltas": deltas,
        "artifact_index": artifact_index_payload,
        "state": state_payload,
        "latest_cursor": latest_cursor.clone(),
    }));
    Ok(ResolvedTraceSource {
        path: manifest_path.to_path_buf(),
        source_kind: "compaction_manifest",
        events,
        latest_cursor,
        latest_event_id,
        latest_event_kind,
        latest_event_timestamp,
        recovery,
    })
}

fn resolve_trace_source(request: TraceSourceRequest<'_>) -> Result<ResolvedTraceSource, String> {
    if request.compaction_manifest_path.is_some() || request.compaction_manifest_text.is_some() {
        let compaction_path = request
            .compaction_manifest_path
            .unwrap_or("<inline-compaction-manifest>");
        return load_compaction_recovery(CompactionRecoveryRequest {
            manifest_path: &PathBuf::from(compaction_path),
            manifest_text: request.compaction_manifest_text,
            state_text: request.compaction_state_text,
            artifact_index_text: request.compaction_artifact_index_text,
            delta_text: request.compaction_delta_text,
            session_id: request.session_id,
            job_id: request.job_id,
            stream_scope_fields: request.stream_scope_fields,
        });
    }
    let path = request
        .path
        .or(if request.event_stream_text.is_some() {
            Some("<inline-trace-stream>")
        } else {
            None
        })
        .ok_or_else(|| {
            "trace stream replay requires path, event_stream_text, compaction_manifest_path, or compaction_manifest_text".to_string()
        })?;
    let path_buf = PathBuf::from(path);
    let events = load_trace_stream_events(
        &path_buf,
        request.event_stream_text,
        request.session_id,
        request.job_id,
        request.stream_scope_fields,
    )?;
    let latest_event = events.last();
    Ok(ResolvedTraceSource {
        path: path_buf,
        source_kind: "trace_stream",
        latest_cursor: latest_event.and_then(latest_cursor_from_trace_event),
        latest_event_id: latest_event
            .and_then(|payload| trace_event_string_field(payload, "event_id")),
        latest_event_kind: latest_event
            .and_then(|payload| trace_event_string_field(payload, "kind")),
        latest_event_timestamp: latest_event
            .and_then(|payload| trace_event_string_field(payload, "ts")),
        recovery: None,
        events,
    })
}

pub fn inspect_trace_stream(
    payload: TraceStreamInspectRequestPayload,
) -> Result<TraceStreamInspectResponsePayload, String> {
    let resolved = resolve_trace_source(TraceSourceRequest::from_inspect_payload(&payload))?;
    let reroute_count = trace_reroute_count(&resolved.events);
    let retry_count = trace_retry_count(&resolved.events);

    Ok(TraceStreamInspectResponsePayload {
        schema_version: TRACE_STREAM_INSPECT_SCHEMA_VERSION.to_string(),
        authority: TRACE_STREAM_IO_AUTHORITY.to_string(),
        path: resolved.path.display().to_string(),
        source_kind: resolved.source_kind.to_string(),
        event_count: resolved.events.len(),
        latest_event_id: resolved.latest_event_id,
        latest_event_kind: resolved.latest_event_kind,
        latest_event_timestamp: resolved.latest_event_timestamp,
        latest_cursor: resolved.latest_cursor,
        recovery: resolved.recovery,
        reroute_count,
        retry_count,
    })
}

pub fn replay_trace_stream(
    payload: TraceStreamReplayRequestPayload,
) -> Result<TraceStreamReplayResponsePayload, String> {
    let resolved = resolve_trace_source(TraceSourceRequest::from_replay_payload(&payload))?;
    let after_event_id = payload.after_event_id.clone();
    let limit = payload.limit.unwrap_or(usize::MAX);
    let mut anchor_found = after_event_id.is_none();
    let mut anchor_index = None;
    let mut next_cursor = None;
    let mut events = Vec::new();

    for (current_index, event_payload) in resolved.events.iter().enumerate() {
        let event_id = trace_event_string_field(event_payload, "event_id");
        if !anchor_found {
            if event_id.as_deref() == after_event_id.as_deref() {
                anchor_found = true;
                anchor_index = Some(current_index);
                continue;
            }
            continue;
        }
        if events.len() >= limit {
            continue;
        }
        next_cursor = Some(TraceStreamReplayCursorPayload {
            event_id: event_id.clone(),
            event_index: current_index,
        });
        events.push(Value::Object(event_payload.clone()));
    }

    if after_event_id.is_some() && !anchor_found {
        return Err(format!(
            "Unknown event id for stream resume: {}",
            after_event_id.unwrap_or_default()
        ));
    }

    let window_start_index = anchor_index.map_or(0, |index| index + 1);
    let has_more = resolved.events.len() > window_start_index + events.len();
    Ok(TraceStreamReplayResponsePayload {
        schema_version: TRACE_STREAM_REPLAY_SCHEMA_VERSION.to_string(),
        authority: TRACE_STREAM_IO_AUTHORITY.to_string(),
        path: resolved.path.display().to_string(),
        source_kind: resolved.source_kind.to_string(),
        event_count: resolved.events.len(),
        latest_event_id: resolved.latest_event_id,
        latest_event_kind: resolved.latest_event_kind,
        latest_event_timestamp: resolved.latest_event_timestamp,
        latest_cursor: resolved.latest_cursor,
        after_event_id,
        window_start_index,
        has_more,
        next_cursor,
        events,
    })
}

fn trace_reroute_count(events: &[Map<String, Value>]) -> usize {
    events
        .iter()
        .filter(|event| {
            trace_event_string_field(event, "kind").as_deref() == Some("route.selected")
        })
        .count()
        .saturating_sub(1)
}

fn trace_retry_count(events: &[Map<String, Value>]) -> usize {
    events
        .iter()
        .filter(|event| trace_event_string_field(event, "kind").as_deref() == Some("run.failed"))
        .count()
}

fn build_trace_stream_metadata(
    trace: &ResolvedTraceSource,
    control_plane: Option<&Value>,
) -> Value {
    let latest = trace.events.last();
    let mut stream = Map::new();
    stream.insert(
        "generation".to_string(),
        json!(
            latest
                .and_then(|event| trace_event_usize_field(event, "generation"))
                .unwrap_or(0)
        ),
    );
    stream.insert("replay_supported".to_string(), Value::Bool(true));
    stream.insert("event_stream_supported".to_string(), Value::Bool(true));
    stream.insert(
        "event_stream_schema_version".to_string(),
        Value::String("runtime-event-stream-v1".to_string()),
    );
    if let Some(control_plane) = control_plane.and_then(Value::as_object) {
        let field_map = [
            ("authority", "control_plane_authority"),
            ("role", "control_plane_role"),
            ("projection", "control_plane_projection"),
            ("delegate_kind", "control_plane_delegate_kind"),
            ("ownership_lane", "ownership_lane"),
            ("producer_owner", "producer_owner"),
            ("producer_authority", "producer_authority"),
            ("exporter_owner", "exporter_owner"),
            ("exporter_authority", "exporter_authority"),
            ("transport_family", "transport_family"),
            ("resume_mode", "resume_mode"),
            ("stream_scope_fields", "stream_scope_fields"),
            ("cleanup_scope_fields", "cleanup_scope_fields"),
        ];
        for (source, target) in field_map {
            if let Some(value) = control_plane.get(source) {
                stream.insert(target.to_string(), value.clone());
            }
        }
    }
    stream.insert(
        "event_stream_path".to_string(),
        Value::String(trace.path.display().to_string()),
    );
    stream.insert("compaction_manifest_path".to_string(), Value::Null);
    stream.insert("event_count".to_string(), json!(trace.events.len()));
    stream.insert(
        "latest_seq".to_string(),
        json!(
            latest
                .and_then(|event| trace_event_usize_field(event, "seq"))
                .unwrap_or(0)
        ),
    );
    stream.insert(
        "latest_event_id".to_string(),
        trace
            .latest_event_id
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    stream.insert(
        "latest_cursor".to_string(),
        trace.latest_cursor.clone().unwrap_or(Value::Null),
    );
    Value::Object(stream)
}

pub fn write_trace_compaction_delta(
    payload: TraceCompactionDeltaWriteRequestPayload,
) -> Result<TraceCompactionDeltaWriteResponsePayload, String> {
    let path = PathBuf::from(&payload.path);
    let serialized = serde_json::to_string(&payload.delta)
        .map_err(|err| format!("serialize trace compaction delta failed: {err}"))?
        + "\n";
    append_text_with_process_lock(&path, &serialized, "trace compaction delta")?;
    Ok(TraceCompactionDeltaWriteResponsePayload {
        schema_version: TRACE_COMPACTION_DELTA_WRITE_SCHEMA_VERSION.to_string(),
        authority: TRACE_STREAM_IO_AUTHORITY.to_string(),
        path: path.display().to_string(),
        bytes_written: serialized.len(),
    })
}

fn default_trace_metadata_schema_version() -> String {
    "trace-metadata-v2".to_string()
}

fn default_trace_framework_version() -> String {
    "phase1".to_string()
}

fn timestamp_now() -> String {
    framework_kernel::time::now_iso()
}

fn default_trace_runtime_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("skills")
        .join("SKILL_ROUTING_RUNTIME.json")
}

fn load_trace_routing_runtime_version(runtime_path: Option<&str>) -> u64 {
    let resolved_path = runtime_path
        .map(PathBuf::from)
        .unwrap_or_else(default_trace_runtime_path);
    let raw = match fs::read_to_string(&resolved_path) {
        Ok(value) => value,
        Err(_) => return 1,
    };
    match serde_json::from_str::<Value>(&raw) {
        Ok(Value::Object(payload)) => payload.get("version").and_then(Value::as_u64).unwrap_or(1),
        _ => 1,
    }
}

fn trace_source_explicitly_provided(payload: &TraceMetadataWriteRequestPayload) -> bool {
    payload.event_stream_path.is_some()
        || payload.event_stream_text.is_some()
        || payload.compaction_manifest_path.is_some()
        || payload.compaction_manifest_text.is_some()
}

pub fn write_trace_metadata(
    payload: TraceMetadataWriteRequestPayload,
) -> Result<TraceMetadataWriteResponsePayload, String> {
    let routing_runtime_version = payload
        .routing_runtime_version
        .unwrap_or_else(|| load_trace_routing_runtime_version(payload.runtime_path.as_deref()));
    let metadata_schema_version = payload
        .metadata_schema_version
        .as_deref()
        .map(str::to_string)
        .unwrap_or_else(default_trace_metadata_schema_version);
    let framework_version = payload
        .framework_version
        .as_deref()
        .map(str::to_string)
        .unwrap_or_else(default_trace_framework_version);
    let timestamp = payload.ts.clone().unwrap_or_else(timestamp_now);
    let should_resolve_trace = payload.events.is_none()
        || payload.stream.is_none()
        || payload.reroute_count.is_none()
        || payload.retry_count.is_none();
    let resolved_trace = if trace_source_explicitly_provided(&payload) {
        Some(resolve_trace_source(
            TraceSourceRequest::from_metadata_payload(&payload),
        )?)
    } else if should_resolve_trace {
        resolve_trace_source(TraceSourceRequest::from_metadata_payload(&payload)).ok()
    } else {
        None
    };
    let resolved_events = payload.events.clone().unwrap_or_else(|| {
        resolved_trace
            .as_ref()
            .map(|trace| {
                trace
                    .events
                    .iter()
                    .cloned()
                    .map(Value::Object)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });
    let reroute_count = payload.reroute_count.unwrap_or_else(|| {
        resolved_trace
            .as_ref()
            .map(|trace| trace_reroute_count(&trace.events))
            .unwrap_or(0)
    });
    let retry_count = payload.retry_count.unwrap_or_else(|| {
        resolved_trace
            .as_ref()
            .map(|trace| trace_retry_count(&trace.events))
            .unwrap_or(0)
    });
    let resolved_stream = payload.stream.clone().or_else(|| {
        resolved_trace
            .as_ref()
            .map(|trace| build_trace_stream_metadata(trace, payload.control_plane.as_ref()))
    });

    let mut document = Map::new();
    document.insert("version".to_string(), json!(1));
    document.insert(
        "schema_version".to_string(),
        json!(metadata_schema_version.clone()),
    );
    document.insert(
        "metadata_schema_version".to_string(),
        json!(metadata_schema_version),
    );
    document.insert("ts".to_string(), json!(timestamp));
    document.insert("task".to_string(), json!(payload.task));
    document.insert("framework_version".to_string(), json!(framework_version));
    document.insert(
        "routing_runtime_version".to_string(),
        json!(routing_runtime_version),
    );
    document.insert("matched_skills".to_string(), json!(payload.matched_skills));
    document.insert(
        "decision".to_string(),
        json!({
            "owner": payload.owner,
            "gate": payload.gate,
            "overlay": payload.overlay,
        }),
    );
    document.insert("reroute_count".to_string(), json!(reroute_count));
    document.insert("retry_count".to_string(), json!(retry_count));
    document.insert("artifact_paths".to_string(), json!(payload.artifact_paths));
    document.insert(
        "verification_status".to_string(),
        json!(payload.verification_status),
    );
    if let Some(value) = payload.trace_event_schema_version {
        document.insert("trace_event_schema_version".to_string(), json!(value));
    }
    if let Some(value) = payload.trace_event_sink_schema_version {
        document.insert("trace_event_sink_schema_version".to_string(), json!(value));
    }
    if let Some(value) = payload.parallel_group {
        document.insert("parallel_group".to_string(), value);
    }
    if let Some(value) = payload.supervisor_projection {
        document.insert("supervisor_projection".to_string(), value);
    }
    if let Some(value) = payload.control_plane {
        document.insert("control_plane".to_string(), value);
    }
    if let Some(value) = resolved_stream {
        document.insert("stream".to_string(), value);
    }
    if !resolved_events.is_empty() {
        document.insert("events".to_string(), Value::Array(resolved_events));
    }

    let serialized = serde_json::to_string_pretty(&Value::Object(document))
        .map_err(|err| format!("serialize trace metadata failed: {err}"))?
        + "\n";
    if payload.write_outputs {
        let outputs =
            std::iter::once(payload.output_path.clone()).chain(payload.mirror_paths.clone());
        for output in outputs {
            let path = PathBuf::from(&output);
            validate_write_path(&path, None)?;
            core_state::utils::atomic_write::write_atomic_text(&path, &serialized)?;
        }
    }

    Ok(TraceMetadataWriteResponsePayload {
        schema_version: TRACE_METADATA_WRITE_SCHEMA_VERSION.to_string(),
        authority: TRACE_METADATA_WRITE_AUTHORITY.to_string(),
        output_path: payload.output_path,
        mirror_paths: payload.mirror_paths,
        bytes_written: serialized.len(),
        routing_runtime_version,
        payload_text: serialized,
    })
}
