//! Trace stream replay / inspect / metadata I/O.

use serde_json::{Map, Value, json};
use std::fs;
use std::path::{Path, PathBuf};

use core_errors::FrameworkError;
use crate::io_utils::validate_write_path;
use framework_core::stdio_payload_types::{
    TraceMetadataWriteRequestPayload, TraceMetadataWriteResponsePayload,
    TraceStreamInspectRequestPayload, TraceStreamInspectResponsePayload,
    TraceStreamReplayCursorPayload, TraceStreamReplayRequestPayload,
    TraceStreamReplayResponsePayload,
};
use rt_storage::runtime_envelope_ids::{
    TRACE_METADATA_WRITE_AUTHORITY,
    TRACE_METADATA_WRITE_SCHEMA_VERSION, TRACE_STREAM_INSPECT_SCHEMA_VERSION,
    TRACE_STREAM_IO_AUTHORITY, TRACE_STREAM_REPLAY_SCHEMA_VERSION,
};
use rt_storage::runtime_storage::{resolve_storage_backend, storage_read_text};
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
        && trace_event_string_field(payload, "session_id").as_deref() != Some(expected_session_id)
        && trace_event_string_field(payload, "run_id").as_deref() != Some(expected_session_id)
    {
        return false;
    }
    if job_scoped
        && let Some(expected_job_id) = job_id
        && trace_event_string_field(payload, "job_id").as_deref() != Some(expected_job_id)
    {
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
) -> Result<Vec<Map<String, Value>>, FrameworkError> {
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
                FrameworkError::validation(format!(
                    "parse trace stream line {} failed: {err}",
                    line_number + 1
                ))
            })?)
            .map_err(|e| FrameworkError::validation(e.to_string()))?,
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
    session_id: Option<&'a str>,
    job_id: Option<&'a str>,
    stream_scope_fields: &'a Option<Vec<String>>,
}

impl<'a> TraceSourceRequest<'a> {
    fn from_metadata_payload(payload: &'a TraceMetadataWriteRequestPayload) -> Self {
        Self {
            path: payload.event_stream_path.as_deref(),
            event_stream_text: payload.event_stream_text.as_deref(),
            session_id: payload.session_id.as_deref(),
            job_id: payload.job_id.as_deref(),
            stream_scope_fields: &payload.stream_scope_fields,
        }
    }

    fn from_inspect_payload(payload: &'a TraceStreamInspectRequestPayload) -> Self {
        Self {
            path: payload.path.as_deref(),
            event_stream_text: payload.event_stream_text.as_deref(),
            session_id: payload.session_id.as_deref(),
            job_id: payload.job_id.as_deref(),
            stream_scope_fields: &payload.stream_scope_fields,
        }
    }

    fn from_replay_payload(payload: &'a TraceStreamReplayRequestPayload) -> Self {
        Self {
            path: payload.path.as_deref(),
            event_stream_text: payload.event_stream_text.as_deref(),
            session_id: payload.session_id.as_deref(),
            job_id: payload.job_id.as_deref(),
            stream_scope_fields: &payload.stream_scope_fields,
        }
    }
}

fn resolve_trace_source(
    request: TraceSourceRequest<'_>,
) -> Result<ResolvedTraceSource, FrameworkError> {
    let path = request
        .path
        .or(if request.event_stream_text.is_some() {
            Some("<inline-trace-stream>")
        } else {
            None
        })
        .ok_or_else(|| {
            FrameworkError::validation(
                "trace stream replay requires path or event_stream_text",
            )
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
) -> Result<TraceStreamInspectResponsePayload, FrameworkError> {
    let resolved = resolve_trace_source(TraceSourceRequest::from_inspect_payload(&payload))?;
    let reroute_count = trace_reroute_count(&resolved.events);
    let retry_count = trace_retry_count(&resolved.events);

    let mut response = TraceStreamInspectResponsePayload::new(
        resolved.path.display().to_string(),
        resolved.source_kind.to_string(),
        resolved.events.len(),
    );
    response.schema_version = TRACE_STREAM_INSPECT_SCHEMA_VERSION.to_string();
    response.authority = TRACE_STREAM_IO_AUTHORITY.to_string();
    response.latest_event_id = resolved.latest_event_id;
    response.latest_event_kind = resolved.latest_event_kind;
    response.latest_event_timestamp = resolved.latest_event_timestamp;
    response.latest_cursor = resolved.latest_cursor;
    response.recovery = resolved.recovery;
    response.reroute_count = reroute_count;
    response.retry_count = retry_count;
    Ok(response)
}

pub fn replay_trace_stream(
    payload: TraceStreamReplayRequestPayload,
) -> Result<TraceStreamReplayResponsePayload, FrameworkError> {
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
        return Err(FrameworkError::validation(format!(
            "Unknown event id for stream resume: {}",
            after_event_id.unwrap_or_default()
        )));
    }

    let window_start_index = anchor_index.map_or(0, |index| index + 1);
    let has_more = resolved.events.len() > window_start_index + events.len();
    let mut response = TraceStreamReplayResponsePayload::new(
        resolved.path.display().to_string(),
        resolved.source_kind.to_string(),
        resolved.events.len(),
        events,
    );
    response.schema_version = TRACE_STREAM_REPLAY_SCHEMA_VERSION.to_string();
    response.authority = TRACE_STREAM_IO_AUTHORITY.to_string();
    response.latest_event_id = resolved.latest_event_id;
    response.latest_event_kind = resolved.latest_event_kind;
    response.latest_event_timestamp = resolved.latest_event_timestamp;
    response.latest_cursor = resolved.latest_cursor;
    response.after_event_id = after_event_id;
    response.window_start_index = window_start_index;
    response.has_more = has_more;
    response.next_cursor = next_cursor;
    Ok(response)
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

fn default_trace_metadata_schema_version() -> String {
    "trace-metadata-v2".to_string()
}

fn default_trace_framework_version() -> String {
    "phase1".to_string()
}

fn timestamp_now() -> String {
    framework_core::time::now_iso()
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
}

pub fn write_trace_metadata(
    payload: TraceMetadataWriteRequestPayload,
) -> Result<TraceMetadataWriteResponsePayload, FrameworkError> {
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

    let serialized = serde_json::to_string_pretty(&Value::Object(document)).map_err(|err| {
        FrameworkError::validation(format!("serialize trace metadata failed: {err}"))
    })? + "\n";
    if payload.write_outputs {
        let outputs =
            std::iter::once(payload.output_path.clone()).chain(payload.mirror_paths.clone());
        for output in outputs {
            let path = PathBuf::from(&output);
            validate_write_path(&path, None)?;
            core_state_utils::atomic_write::write_atomic_text(&path, &serialized)?;
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

#[allow(clippy::unwrap_used, clippy::expect_used)]
#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_trace_events() -> String {
        r#"{"kind": "run.started", "session_id": "s1", "seq": 1}
{"kind": "route.selected", "session_id": "s1", "seq": 2}
{"kind": "run.failed", "session_id": "s1", "seq": 3}
{"kind": "run.started", "session_id": "s1", "seq": 4}
"#
        .to_string()
    }

    // ── inspect_trace_stream ──

    #[test]
    fn inspect_from_inline_text() {
        let payload = TraceStreamInspectRequestPayload {
            path: None,
            event_stream_text: Some(minimal_trace_events()),
            session_id: None,
            job_id: None,
            stream_scope_fields: None,
        };
        let result = inspect_trace_stream(payload).unwrap();
        assert_eq!(result.event_count, 4);
        assert_eq!(result.source_kind, "trace_stream");
        assert!(result.latest_event_id.is_some());
        // reroute_count = route.selected events - 1 = 0 (only 1)
        assert_eq!(result.reroute_count, 0);
        // retry_count = run.failed events = 1
        assert_eq!(result.retry_count, 1);
    }

    #[test]
    fn inspect_filters_by_session_id() {
        let events = r#"{"kind": "run.started", "session_id": "s1", "seq": 1}
{"kind": "run.started", "session_id": "s2", "seq": 2}
"#
        .to_string();
        let payload = TraceStreamInspectRequestPayload {
            event_stream_text: Some(events),
            session_id: Some("s1".into()),
            ..base_inspect()
        };
        let result = inspect_trace_stream(payload).unwrap();
        assert_eq!(result.event_count, 1);
    }

    #[test]
    fn inspect_reroute_count_multiple() {
        let events = r#"{"kind": "route.selected", "session_id": "s1", "seq": 1}
{"kind": "route.selected", "session_id": "s1", "seq": 2}
{"kind": "route.selected", "session_id": "s1", "seq": 3}
"#;
        let payload = TraceStreamInspectRequestPayload {
            event_stream_text: Some(events.into()),
            ..base_inspect()
        };
        let result = inspect_trace_stream(payload).unwrap();
        // reroute_count = 3 route.selected - 1 = 2
        assert_eq!(result.reroute_count, 2);
    }

    #[test]
    fn inspect_without_source_returns_err() {
        let payload = TraceStreamInspectRequestPayload {
            path: None,
            event_stream_text: None,
            ..base_inspect()
        };
        assert!(inspect_trace_stream(payload).is_err());
    }

    // ── replay_trace_stream ──

    #[test]
    fn replay_all_events() {
        let payload = TraceStreamReplayRequestPayload {
            path: None,
            event_stream_text: Some(minimal_trace_events()),
            session_id: None,
            job_id: None,
            stream_scope_fields: None,
            after_event_id: None,
            limit: None,
        };
        let result = replay_trace_stream(payload).unwrap();
        assert_eq!(result.event_count, 4);
        assert_eq!(result.events.len(), 4);
    }

    #[test]
    fn replay_after_event_id() {
        let payload = TraceStreamReplayRequestPayload {
            event_stream_text: Some(minimal_trace_events()),
            after_event_id: Some("evt_replay_000002".into()),
            session_id: Some("s1".into()),
            ..base_replay()
        };
        let result = replay_trace_stream(payload).unwrap();
        // After evt_replay_000002 (seq 2, line 3), remaining = 2 events
        assert_eq!(result.events.len(), 2, "events after anchor");
    }

    #[test]
    fn replay_with_limit() {
        let payload = TraceStreamReplayRequestPayload {
            event_stream_text: Some(minimal_trace_events()),
            limit: Some(2),
            ..base_replay()
        };
        let result = replay_trace_stream(payload).unwrap();
        assert_eq!(result.events.len(), 2);
        assert!(result.has_more, "more events available");
    }

    #[test]
    fn replay_unknown_after_event_id_returns_err() {
        let payload = TraceStreamReplayRequestPayload {
            event_stream_text: Some(minimal_trace_events()),
            after_event_id: Some("nonexistent".into()),
            ..base_replay()
        };
        assert!(replay_trace_stream(payload).is_err());
    }

    // ── Shared helpers ──

    fn base_inspect() -> TraceStreamInspectRequestPayload {
        TraceStreamInspectRequestPayload {
            path: None,
            event_stream_text: None,
            session_id: None,
            job_id: None,
            stream_scope_fields: None,
        }
    }

    fn base_replay() -> TraceStreamReplayRequestPayload {
        TraceStreamReplayRequestPayload {
            path: None,
            event_stream_text: None,
            session_id: None,
            job_id: None,
            stream_scope_fields: None,
            after_event_id: None,
            limit: None,
        }
    }
}
