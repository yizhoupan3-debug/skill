//! Embedded JSON consistency: ensure include_str! schema_versions match disk files.
//!
//! Each embedded JSON (`include_str!`) must carry the same schema identifier
//! as the corresponding file on disk.  A mismatch indicates the compiled binary
//! is stale relative to the config it ships — or vice versa.

use serde_json::Value;
use std::fs;

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Parse a JSON file and extract the `schema_version` field (top-level string).
fn disk_schema_version(path: &std::path::Path) -> String {
    let data = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    let json: Value = serde_json::from_str(&data)
        .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()));
    json.get("schema_version")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Parse a JSON file and extract the `$schema` field (top-level string).
fn disk_dollar_schema(path: &std::path::Path) -> String {
    let data = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    let json: Value = serde_json::from_str(&data)
        .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()));
    json.get("$schema")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

// ---------------------------------------------------------------------------
// Tests — each calls the crate's embedded_schema_version() and compares
// with the corresponding disk file's schema identifier.
// ---------------------------------------------------------------------------

#[test]
fn nl_route_adjustments_embedded_matches_disk() {
    let embedded =
        routing_engine::route::nl_route_adjustments::embedded_schema_version();
    let disk = disk_schema_version(
        &repo_root().join("configs/framework/NL_ROUTE_ADJUSTMENTS.json"),
    );
    assert_eq!(
        embedded, disk,
        "NL_ROUTE_ADJUSTMENTS.json schema_version mismatch: embedded={embedded}, disk={disk}"
    );
}

#[test]
fn hook_observation_rules_embedded_matches_disk() {
    let embedded =
        rt_core_contracts::hook_observation_rules::embedded_schema_version();
    let disk = disk_schema_version(
        &repo_root()
            .join("configs/framework/ROUTER_RS_HOOK_OBSERVATION_RULES.json"),
    );
    assert_eq!(
        embedded, disk,
        "ROUTER_RS_HOOK_OBSERVATION_RULES.json schema_version mismatch: embedded={embedded}, disk={disk}"
    );
}

#[test]
fn scoring_weights_embedded_matches_disk() {
    let embedded = routing_engine::scoring_config::embedded_schema_version();
    // scoring_weights.json uses "$schema" rather than "schema_version".
    let disk = disk_dollar_schema(
        &repo_root().join("configs/scoring_weights.json"),
    );
    assert_eq!(
        embedded, disk,
        "scoring_weights.json $schema mismatch: embedded={embedded}, disk={disk}"
    );
}

#[test]
fn gate_hint_phrases_embedded_matches_disk() {
    let embedded =
        routing_engine::route::gate_hints::embedded_schema_version();
    let disk = disk_schema_version(
        &repo_root().join("configs/gate_hint_phrases.json"),
    );
    assert_eq!(
        embedded, disk,
        "gate_hint_phrases.json schema_version mismatch: embedded={embedded}, disk={disk}"
    );
}
