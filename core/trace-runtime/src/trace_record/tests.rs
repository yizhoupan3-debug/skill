#![cfg(test)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use serde_json::{Map, Value, json};

use super::util::build_prefixed_id;

// ── build_prefixed_id ──

#[test]
fn build_prefixed_id_deterministic() {
    let a = build_prefixed_id("evt", "seed-value");
    let b = build_prefixed_id("evt", "seed-value");
    assert_eq!(a, b);
    assert!(a.starts_with("evt_"));
    assert_eq!(a.len(), "evt_".len() + 12);
}

#[test]
fn build_prefixed_id_different_prefixes_differ() {
    let a = build_prefixed_id("evt", "same");
    let b = build_prefixed_id("snap", "same");
    assert_ne!(a, b);
    assert!(a.starts_with("evt_"));
    assert!(b.starts_with("snap_"));
}

#[test]
fn build_trace_cursor_format() {
    let cursor = build_trace_cursor(3, 7, "evt_abc123");
    assert_eq!(cursor, "g3:s7:evt_abc123");
}

#[test]
fn trace_event_string_field_missing() {
    let event = Map::new();
    assert_eq!(trace_event_string_field(&event, "run_id"), None);
}

#[test]
fn trace_event_string_field_present() {
    let mut event = Map::new();
    event.insert("run_id".to_string(), Value::String("r1".to_string()));
    assert_eq!(
        trace_event_string_field(&event, "run_id"),
        Some("r1".to_string())
    );
}

#[test]
fn trace_event_usize_field_present() {
    let mut event = Map::new();
    event.insert("seq".to_string(), json!(42));
    assert_eq!(trace_event_usize_field(&event, "seq"), Some(42));
}

#[test]
fn trace_event_usize_field_non_numeric() {
    let mut event = Map::new();
    event.insert("seq".to_string(), Value::String("not-a-number".to_string()));
    assert_eq!(trace_event_usize_field(&event, "seq"), None);
}

#[test]
fn hydrate_trace_event_fills_defaults() {
    let mut event = Map::new();
    event.insert("kind".to_string(), Value::String("test".to_string()));
    let hydrated = hydrate_trace_event(event, 5);
    assert_eq!(
        trace_event_string_field(&hydrated, "event_id"),
        Some("evt_replay_000005".to_string())
    );
    assert_eq!(trace_event_usize_field(&hydrated, "seq"), Some(5));
    assert_eq!(trace_event_usize_field(&hydrated, "generation"), Some(0));
    assert_eq!(
        trace_event_string_field(&hydrated, "status"),
        Some("ok".to_string())
    );
    assert!(trace_event_string_field(&hydrated, "page_token").is_some());
}

#[test]
fn hydrate_trace_event_preserves_existing_fields() {
    let mut event = Map::new();
    event.insert("seq".to_string(), json!(99));
    event.insert(
        "event_id".to_string(),
        Value::String("custom_id".to_string()),
    );
    event.insert("status".to_string(), Value::String("error".to_string()));
    let hydrated = hydrate_trace_event(event, 1);
    assert_eq!(trace_event_usize_field(&hydrated, "seq"), Some(99));
    assert_eq!(
        trace_event_string_field(&hydrated, "event_id"),
        Some("custom_id".to_string())
    );
    assert_eq!(
        trace_event_string_field(&hydrated, "status"),
        Some("error".to_string())
    );
}

#[test]
fn trace_event_object_unwraps_event_wrapper() {
    let wrapped = json!({"event": {"kind": "test", "seq": 1}});
    let result = trace_event_object(wrapped).unwrap();
    assert_eq!(
        trace_event_string_field(&result, "kind"),
        Some("test".to_string())
    );
}

#[test]
fn trace_event_object_accepts_bare_object() {
    let bare = json!({"kind": "test", "seq": 1});
    let result = trace_event_object(bare).unwrap();
    assert_eq!(
        trace_event_string_field(&result, "kind"),
        Some("test".to_string())
    );
}

#[test]
fn trace_event_object_rejects_non_object() {
    let result = trace_event_object(Value::String("bad".to_string()));
    assert!(result.is_err());
}

#[test]
fn trace_event_object_rejects_non_object_event_wrapper() {
    let result = trace_event_object(json!({"event": "not-an-object"}));
    assert!(result.is_err());
}
