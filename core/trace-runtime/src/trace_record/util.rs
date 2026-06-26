use crate::TraceError;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use super::TRACE_EVENT_SCHEMA_VERSION;

/// SHA-256 hash of a byte payload, returned as a hex string.
pub fn sha256_hex(payload: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(payload);
    hex::encode(hasher.finalize())
}

/// Build a trace cursor string from generation, seq, and event_id components.
pub fn build_trace_cursor(generation: usize, seq: usize, event_id: &str) -> String {
    format!("g{generation}:s{seq}:{event_id}")
}

/// Extract a string field from a trace event JSON object.
pub fn trace_event_string_field(payload: &Map<String, Value>, field: &str) -> Option<String> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Extract a usize field from a trace event JSON object.
pub fn trace_event_usize_field(payload: &Map<String, Value>, field: &str) -> Option<usize> {
    payload
        .get(field)
        .and_then(|value| value.as_u64().map(|number| number as usize))
}

/// Extract the inner event JSON object from a trace stream line.
/// Handles both `{"event": {...}}` wrapper format and bare object format.
pub fn trace_event_object(payload: Value) -> Result<Map<String, Value>, TraceError> {
    match payload {
        Value::Object(mut object) => match object.remove("event") {
            Some(Value::Object(event)) => Ok(event),
            Some(other) => Err(TraceError::validation(format!(
                "trace stream line contained non-object event wrapper: {other}"
            ))),
            None => Ok(object),
        },
        other => Err(TraceError::validation(format!(
            "trace stream line must decode to a JSON object: {other}"
        ))),
    }
}

/// Hydrate a trace event with default values for seq, generation, event_id, page_token, status, and schema_version.
pub fn hydrate_trace_event(
    mut payload: Map<String, Value>,
    line_number: usize,
) -> Map<String, Value> {
    let seq = trace_event_usize_field(&payload, "seq").unwrap_or(line_number);
    let generation = trace_event_usize_field(&payload, "generation").unwrap_or(0);
    let event_id = trace_event_string_field(&payload, "event_id")
        .unwrap_or_else(|| format!("evt_replay_{line_number:06}"));
    let page_token = trace_event_string_field(&payload, "page_token")
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
        .entry("page_token".to_string())
        .or_insert_with(|| Value::String(page_token));
    payload
        .entry("status".to_string())
        .or_insert_with(|| Value::String("ok".to_string()));
    payload
        .entry("schema_version".to_string())
        .or_insert_with(|| Value::String(TRACE_EVENT_SCHEMA_VERSION.to_string()));
    payload
}

/// Build a prefixed ID from a seed string (used internally by record and compact modules).
pub(super) fn build_prefixed_id(prefix: &str, seed: &str) -> String {
    let digest = sha256_hex(seed.as_bytes());
    format!("{prefix}_{}", &digest[..12])
}
