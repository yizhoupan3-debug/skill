#![cfg(test)]

use super::*;
use serde_json::{json, Map, Value};

use super::compact::{
    build_compaction_stream_key, load_trace_events_from_text, stable_digest,
    trace_event_matches_scope, unique_strings,
};
use super::util::build_prefixed_id;

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
fn stable_digest_deterministic() {
    let v = json!({"key": "value", "num": 42});
    let a = stable_digest(&v);
    let b = stable_digest(&v);
    assert_eq!(a, b);
    assert_eq!(a.len(), 64);
}

#[test]
fn stable_digest_different_for_different_values() {
    let a = stable_digest(&json!({"x": 1}));
    let b = stable_digest(&json!({"x": 2}));
    assert_ne!(a, b);
}

#[test]
fn unique_strings_preserves_order_deduplicates() {
    let input = vec![
        "alpha".to_string(),
        "beta".to_string(),
        "alpha".to_string(),
        "gamma".to_string(),
        "beta".to_string(),
    ];
    let result = unique_strings(&input);
    assert_eq!(result, vec!["alpha", "beta", "gamma"]);
}

#[test]
fn unique_strings_empty() {
    assert!(unique_strings(&[]).is_empty());
}

#[test]
fn build_compaction_stream_key_normalizes() {
    let key = build_compaction_stream_key("run-123", Some("job/456"));
    assert_eq!(key, "run-123__job_456");
}

#[test]
fn build_compaction_stream_key_no_job() {
    let key = build_compaction_stream_key("run-abc", None);
    assert_eq!(key, "run-abc__session");
}

#[test]
fn build_compaction_stream_key_empty_becomes_stream() {
    let key = build_compaction_stream_key("", None);
    assert_eq!(key, "stream__session");
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
fn trace_event_matches_scope_allows_no_filter() {
    let mut event = Map::new();
    event.insert("run_id".to_string(), Value::String("r1".to_string()));
    assert!(trace_event_matches_scope(&event, None, None));
}

#[test]
fn trace_event_matches_scope_filters_by_run_id() {
    let mut event = Map::new();
    event.insert("run_id".to_string(), Value::String("r1".to_string()));
    assert!(trace_event_matches_scope(&event, Some("r1"), None));
    assert!(!trace_event_matches_scope(&event, Some("r2"), None));
}

#[test]
fn trace_event_matches_scope_filters_by_job_id() {
    let mut event = Map::new();
    event.insert("run_id".to_string(), Value::String("r1".to_string()));
    event.insert("job_id".to_string(), Value::String("j1".to_string()));
    assert!(trace_event_matches_scope(&event, Some("r1"), Some("j1")));
    assert!(!trace_event_matches_scope(&event, Some("r1"), Some("j2")));
}

#[test]
fn trace_event_matches_scope_missing_run_id_fallback_to_session_id() {
    let mut event = Map::new();
    event.insert("session_id".to_string(), Value::String("s1".to_string()));
    assert!(trace_event_matches_scope(&event, Some("s1"), None));
    assert!(!trace_event_matches_scope(&event, Some("s2"), None));
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
    event.insert("event_id".to_string(), Value::String("custom_id".to_string()));
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

#[test]
fn load_trace_events_filters_by_run_id() {
    let stream = format!(
        "{}\n{}\n",
        json!({"run_id": "r1", "kind": "a", "seq": 1}),
        json!({"run_id": "r2", "kind": "b", "seq": 2}),
    );
    let events = load_trace_events_from_text(&stream, Some("r1"), None).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(
        trace_event_string_field(&events[0], "kind"),
        Some("a".to_string())
    );
}

#[test]
fn load_trace_events_skips_empty_lines() {
    let stream = "\n\n{\"run_id\": \"r1\", \"kind\": \"a\", \"seq\": 1}\n\n";
    let events = load_trace_events_from_text(stream, Some("r1"), None).unwrap();
    assert_eq!(events.len(), 1);
}

#[test]
fn load_trace_events_returns_error_on_malformed_json() {
    let stream = "not-json\n";
    let result = load_trace_events_from_text(stream, None, None);
    assert!(result.is_err());
}

#[test]
fn load_trace_events_handles_event_wrapper() {
    let stream = json!({"event": {"run_id": "r1", "kind": "x", "seq": 1}}).to_string() + "\n";
    let events = load_trace_events_from_text(&stream, Some("r1"), None).unwrap();
    assert_eq!(events.len(), 1);
}
