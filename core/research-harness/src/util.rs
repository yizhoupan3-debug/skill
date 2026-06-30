//! Shared JSON value helpers for the research harness.
//!
//! Consolidates duplicated helper functions from state, render, smoke,
//! claims, search, and CLI modules.
//!
//! NOTE: Canonical helpers live here; local duplicates in claims/lifecycle.rs
//! and search modules import from this module.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Extract a string field, returning `""` if missing.
pub(crate) fn str_field<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

/// Extract a string field with a custom default.
pub(crate) fn str_field_default<'a>(value: &'a Value, key: &str, default: &'a str) -> &'a str {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or(default)
}

/// Get an array slice, returning `&[]` if missing.
pub(crate) fn arr<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|a| a.as_slice())
        .unwrap_or(&[])
}

/// Get a mutable array, inserting `[]` if missing. Returns `Err` if the
/// value cannot be coerced to an array.
pub(crate) fn arr_mut<'a>(value: &'a mut Value, key: &str) -> anyhow::Result<&'a mut Vec<Value>> {
    obj_mut(value)
        .entry(key.to_string())
        .or_insert_with(|| serde_json::json!([]))
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("expected array for key '{key}'"))
}

/// Get the underlying mutable object map, coercing non-objects to `{}`.
pub(crate) fn obj_mut(value: &mut Value) -> &mut serde_json::Map<String, Value> {
    if !value.is_object() {
        *value = serde_json::json!({});
    }
    #[allow(clippy::expect_used)]
    // safety: obj_mut just coerced this to `{}` above; as_object_mut() always succeeds.
    value
        .as_object_mut()
        .expect("obj_mut: value must be object after coercion")
}

/// Insert a key-value pair into a mutable object.
pub(crate) fn set_key(value: &mut Value, key: &str, child: Value) {
    obj_mut(value).insert(key.to_string(), child);
}

/// Get the `novelty_gate` sub-object (immutable).
pub(crate) fn novelty_gate(state: &Value) -> &Value {
    state.get("novelty_gate").unwrap_or(&Value::Null)
}

/// Get the `novelty_gate` sub-object (mutable), inserting `{}` if missing.
pub(crate) fn novelty_gate_mut(value: &mut Value) -> &mut serde_json::Map<String, Value> {
    #[allow(clippy::expect_used)]
    // safety: or_insert_with(json!({})) guarantees the entry is an object;
    // as_object_mut() cannot return None here.
    obj_mut(value)
        .entry("novelty_gate".to_string())
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .expect("novelty_gate must be object")
}

/// Extract a list of strings from a JSON array field.
pub(crate) fn value_as_string_list(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Render a Value as a string (string values pass through, numbers become text).
pub(crate) fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "-".into(),
        other => other.to_string(),
    }
}

// ── HTTP Client Factories ──

/// Build a blocking HTTP client with a bounded timeout.
/// Uses a cached client per timeout value to reuse TCP connection pools
/// across repeated search calls within a session.
pub(crate) fn blocking_client(timeout_secs: u64) -> anyhow::Result<reqwest::blocking::Client> {
    use anyhow::Context;
    let timeout = timeout_secs.clamp(3, 120);
    // Cache a shared client per distinct timeout so the connection pool
    // survives across calls (avoids TCP/TLS handshake per search).
    static CLIENT_CACHE: OnceLock<Mutex<HashMap<u64, reqwest::blocking::Client>>> =
        OnceLock::new();
    let cache = CLIENT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = cache.lock().map_err(|e| anyhow::anyhow!("client cache lock: {e}"))?;
    if let Some(cached) = guard.get(&timeout) {
        return Ok(cached.clone());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout))
        .build()
        .context("failed to build blocking HTTP client")?;
    guard.insert(timeout, client.clone());
    Ok(client)
}

/// Build a blocking HTTP client with DNS pinning for SSRF-safe requests.
///
/// Pins the given host to pre-validated addresses, preventing DNS rebinding
/// TOCTOU between validation and actual HTTP connection.
/// Use this for user-provided URLs (DOIs, arbitrary fetch targets).
#[allow(dead_code)]
pub(crate) fn build_pinned_blocking_client(
    host: &str,
    addrs: &[std::net::SocketAddr],
    timeout_secs: u64,
) -> anyhow::Result<reqwest::blocking::Client> {
    use anyhow::Context;
    let mut builder = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs.clamp(3, 120)))
        .redirect(reqwest::redirect::Policy::none());
    for addr in addrs {
        builder = builder.resolve(host, *addr);
    }
    builder.build().context("failed to build pinned blocking HTTP client")
}

// ── SSRF Guard (delegates to runtime_core::web_fetch_guard) ──
// Uses the canonical implementation from runtime-core instead of an inlined
// duplicate. This ensures redirect-chain security checks (scheme downgrade
// detection, DNS pinning) are always in sync.

use runtime_core::web_fetch_guard;

/// Validate a URL for SSRF safety before making an HTTP request.
/// Checks scheme, host suffixes, IP literals, and DNS resolution.
pub(crate) fn validate_url_for_fetch(url: &str) -> anyhow::Result<()> {
    web_fetch_guard::validate_and_resolve_web_fetch_url(url)
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!("{e}"))
}

/// Validate a URL for SSRF safety AND return resolved addresses for DNS pinning.
///
/// Like `validate_url_for_fetch`, but returns the host and resolved addresses
/// so callers can build a DNS-pinned client.
/// Use this for user-provided URLs (DOIs, arbitrary fetch targets) to
/// prevent DNS rebinding TOCTOU between validation and HTTP connection.
///
/// # Errors
/// Returns error on invalid URL, forbidden host/IP, or unresolvable DNS.
pub(crate) fn validate_and_resolve_for_fetch(
    url: &str,
) -> anyhow::Result<(String, Vec<std::net::SocketAddr>)> {
    let (parsed, addrs) = web_fetch_guard::validate_and_resolve_web_fetch_url(url)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let host = parsed
        .host_str()
        .unwrap_or("")
        .to_string();
    Ok((host, addrs))
}

// ── Novelty Gate Helpers ──

/// Get a string field from the `novelty_gate` sub-object.
pub(crate) fn novelty_str<'a>(state: &'a Value, key: &str, default: &'a str) -> &'a str {
    novelty_gate(state)
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
}

/// Get an array from the `novelty_gate` sub-object.
pub(crate) fn novelty_arr<'a>(state: &'a Value, key: &str) -> &'a [Value] {
    novelty_gate(state)
        .get(key)
        .and_then(Value::as_array)
        .map(|a| a.as_slice())
        .unwrap_or(&[])
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn arr_mut_returns_result_and_creates_missing_key() {
        let mut val = serde_json::json!({});
        let arr = arr_mut(&mut val, "items").unwrap();
        arr.push(serde_json::json!(42));
        assert_eq!(val["items"][0], 42);
    }

    #[test]
    fn arr_mut_works_with_existing_array() {
        let mut val = serde_json::json!({"items": [1, 2]});
        let arr = arr_mut(&mut val, "items").unwrap();
        arr.push(serde_json::json!(3));
        assert_eq!(val["items"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn novelty_str_returns_default_when_missing() {
        let state = serde_json::json!({"novelty_gate": {}});
        assert_eq!(novelty_str(&state, "status", "fallback"), "fallback");
    }

    #[test]
    fn novelty_str_returns_value() {
        let state = serde_json::json!({"novelty_gate": {"status": "passed"}});
        assert_eq!(novelty_str(&state, "status", "fallback"), "passed");
    }

    #[test]
    fn novelty_arr_empty_when_missing() {
        let state = serde_json::json!({});
        assert!(novelty_arr(&state, "claim_records").is_empty());
    }

    #[test]
    fn novelty_arr_returns_data() {
        let state = serde_json::json!({"novelty_gate": {"items": [1, 2, 3]}});
        assert_eq!(novelty_arr(&state, "items").len(), 3);
    }

    #[test]
    fn validate_url_blocks_localhost() {
        assert!(validate_url_for_fetch("http://localhost/secret").is_err());
        assert!(validate_url_for_fetch("http://localhost:8080/secret").is_err());
    }

    #[test]
    fn validate_url_blocks_private_ip() {
        assert!(validate_url_for_fetch("http://10.0.0.1/secret").is_err());
        assert!(validate_url_for_fetch("http://192.168.1.1/secret").is_err());
        assert!(validate_url_for_fetch("http://172.16.0.1/secret").is_err());
    }

    #[test]
    fn validate_url_blocks_metadata_endpoint() {
        assert!(validate_url_for_fetch("http://169.254.169.254/latest/meta-data/").is_err());
    }

    #[test]
    fn validate_url_blocks_non_http() {
        assert!(validate_url_for_fetch("file:///etc/passwd").is_err());
        assert!(validate_url_for_fetch("data:text/html,<script>").is_err());
    }

    #[test]
    fn validate_url_blocks_loopback_ip() {
        assert!(validate_url_for_fetch("http://127.0.0.1/secret").is_err());
        assert!(validate_url_for_fetch("http://[::1]/secret").is_err());
    }

    #[test]
    fn validate_and_resolve_for_fetch_accepts_public_url_and_returns_addrs() {
        let result = validate_and_resolve_for_fetch("https://8.8.8.8/");
        assert!(result.is_ok(), "public IP should be accepted: {:?}", result.err());
        let (host, addrs) = result.unwrap();
        assert_eq!(host, "8.8.8.8");
        assert!(!addrs.is_empty());
    }

    #[test]
    fn validate_and_resolve_for_fetch_rejects_loopback() {
        assert!(validate_and_resolve_for_fetch("http://127.0.0.1/").is_err());
        assert!(validate_and_resolve_for_fetch("http://localhost/").is_err());
        assert!(validate_and_resolve_for_fetch("http://[::1]/").is_err());
    }

    #[test]
    fn validate_and_resolve_for_fetch_rejects_non_http() {
        assert!(validate_and_resolve_for_fetch("file:///etc/passwd").is_err());
    }

    #[test]
    fn validate_and_resolve_for_fetch_rejects_private_ip() {
        assert!(validate_and_resolve_for_fetch("http://10.0.0.1/").is_err());
        assert!(validate_and_resolve_for_fetch("http://192.168.1.1/").is_err());
    }

    #[test]
    fn str_field_default_works() {
        let val = serde_json::json!({"name": "test"});
        assert_eq!(str_field_default(&val, "name", "def"), "test");
        assert_eq!(str_field_default(&val, "missing", "def"), "def");
        assert_eq!(str_field_default(&val, "empty", "def"), "def");
    }

    #[test]
    fn value_as_string_list_handles_missing() {
        let val = serde_json::json!({});
        assert!(value_as_string_list(&val, "tags").is_empty());
    }
}

// ── Hash Helpers ──
