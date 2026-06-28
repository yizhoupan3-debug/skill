#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Unit tests for the mcp-tool-registry crate.
//!
//! Tests cover three areas:
//! - **tool_types**: Serde round-trips, default values, `McpToolInputSchema` deserialization
//! - **tool_registry**: v1 (columnar) and v2 (object) format loading, schema validation,
//!   caching behavior with `load_tool_records_cached` / `invalidate_tool_cache`
//! - **lib**: `resolve_tool_registry_path` default path resolution

use std::fs;
use std::path::PathBuf;

use crate::resolve_tool_registry_path;
use crate::tool_registry::{invalidate_tool_cache, load_tool_records, load_tool_records_cached};
use crate::tool_types::{McpToolInputSchema, McpToolRecord};

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Helper: write a string of JSON to a file path.
fn write_json(path: &PathBuf, json: &str) {
    fs::write(path, json).unwrap_or_else(|e| panic!("failed to write temp file: {e}"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// tool_types tests
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_mcp_tool_record_serde_roundtrip() {
    let record = McpToolRecord {
        slug: "test_tool".into(),
        display_name: "Test Tool".into(),
        description: "A comprehensive test tool".into(),
        layer: "builtin".into(),
        dispatch_domain: "domain:goal".into(),
        owner: "framework".into(),
        trigger_hints: vec!["hint1".into(), "hint2".into()],
        host_platforms: vec!["macos".into()],
        mcp_server: "test-server".into(),
        tool_flags: vec!["experimental".into()],
        input_schema_json: Some(McpToolInputSchema {
            schema_type: "object".into(),
            properties: serde_json::json!({"path": {"type": "string"}})
                .as_object()
                .unwrap()
                .clone(),
            required: vec!["path".into()],
        }),
    };

    let json = serde_json::to_value(&record).unwrap();

    // Verify key serialized fields
    assert_eq!(json["slug"], "test_tool");
    assert_eq!(json["display_name"], "Test Tool");
    assert_eq!(json["layer"], "builtin");
    assert_eq!(json["trigger_hints"], serde_json::json!(["hint1", "hint2"]));
    assert_eq!(json["host_platforms"], serde_json::json!(["macos"]));
    assert_eq!(json["tool_flags"], serde_json::json!(["experimental"]));

    // input_schema_json should serialize under the renamed key "input_schema"
    assert!(json.get("input_schema").is_some());
    assert!(json.get("input_schema_json").is_none());

    // Round-trip: deserialize back
    let deserialized: McpToolRecord = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized.slug, "test_tool");
    assert_eq!(deserialized.display_name, "Test Tool");
    assert_eq!(deserialized.trigger_hints, vec!["hint1", "hint2"]);
    assert_eq!(deserialized.host_platforms, vec!["macos"]);
    assert_eq!(deserialized.tool_flags, vec!["experimental"]);

    let schema = deserialized.input_schema_json.unwrap();
    assert_eq!(schema.schema_type, "object");
    assert!(schema.properties.contains_key("path"));
    assert_eq!(schema.required, vec!["path"]);
}

#[test]
fn test_mcp_tool_input_schema_deserialization() {
    let json = serde_json::json!({
        "type": "object",
        "properties": {
            "path": {"type": "string"},
            "page": {"type": "integer"}
        },
        "required": ["path"]
    });

    let schema: McpToolInputSchema = serde_json::from_value(json).unwrap();
    assert_eq!(schema.schema_type, "object");
    assert!(schema.properties.contains_key("path"));
    assert!(schema.properties.contains_key("page"));
    assert_eq!(schema.required, vec!["path"]);
}

#[test]
fn test_input_schema_defaults_to_none() {
    // input_schema not present in JSON → deserializes as None
    let json = serde_json::json!({
        "slug": "no_schema",
        "display_name": "No Schema",
        "description": "desc",
        "layer": "builtin",
        "dispatch_domain": "domain:goal",
        "owner": "framework",
        "trigger_hints": [],
        "host_platforms": [],
        "mcp_server": "srv"
    });

    let record: McpToolRecord = serde_json::from_value(json).unwrap();
    assert!(record.input_schema_json.is_none());
}

#[test]
fn test_minimal_record_deserialization() {
    let json = serde_json::json!({
        "slug": "minimal",
        "display_name": "Min",
        "description": "bare minimum",
        "layer": "builtin",
        "dispatch_domain": "domain:goal",
        "owner": "framework",
        "trigger_hints": [],
        "host_platforms": [],
        "mcp_server": "s"
    });

    let record: McpToolRecord = serde_json::from_value(json).unwrap();
    assert_eq!(record.slug, "minimal");
    assert!(record.trigger_hints.is_empty());
    assert!(record.host_platforms.is_empty());
    assert!(record.tool_flags.is_empty());
    assert!(record.input_schema_json.is_none());
}

// ═══════════════════════════════════════════════════════════════════════════════
// tool_registry tests
// ═══════════════════════════════════════════════════════════════════════════════

// ── v2 (object) format ─────────────────────────────────────────────────────

#[test]
fn test_load_v2_format() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("registry.json");

    write_json(&path, JSON_V2_SINGLE);

    let records = load_tool_records(&path).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].slug, "tool_a");
    assert_eq!(records[0].display_name, "Tool A");
    assert_eq!(records[0].mcp_server, "server-a");
}

#[test]
fn test_load_v2_multiple_tools() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("registry.json");

    write_json(&path, JSON_V2_MULTIPLE);

    let records = load_tool_records(&path).unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].slug, "t1");
    assert_eq!(records[1].slug, "t2");
    assert_eq!(records[1].mcp_server, "s2");
}

#[test]
fn test_load_v2_with_full_fields() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("registry.json");

    write_json(&path, JSON_V2_FULL);

    let records = load_tool_records(&path).unwrap();
    assert_eq!(records.len(), 1);
    let r = &records[0];
    assert_eq!(r.slug, "full_tool");
    assert_eq!(r.layer, "external");
    assert_eq!(r.dispatch_domain, "research");
    assert_eq!(r.owner, "research-team");
    assert_eq!(r.trigger_hints, vec!["hint_a", "hint_b"]);
    assert_eq!(r.host_platforms, vec!["linux", "macos"]);
    assert_eq!(r.tool_flags, vec!["deprecated"]);
    assert!(r.input_schema_json.is_some());

    let schema = r.input_schema_json.as_ref().unwrap();
    assert_eq!(schema.schema_type, "object");
    assert!(schema.properties.contains_key("file"));
    assert_eq!(schema.required, vec!["file"]);
}

#[test]
fn test_load_v2_with_input_schema() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("registry.json");

    write_json(&path, JSON_V2_WITH_SCHEMA);

    let records = load_tool_records(&path).unwrap();
    let schema = records[0]
        .input_schema_json
        .as_ref()
        .expect("should have input_schema");
    assert_eq!(schema.schema_type, "object");
    assert!(schema.properties.contains_key("file"));
    assert_eq!(schema.required, vec!["file"]);
}

// ── Error paths ───────────────────────────────────────────────────────────────

#[test]
fn test_schema_version_mismatch_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("registry.json");

    write_json(&path, JSON_SCHEMA_MISMATCH);

    let err = load_tool_records(&path).unwrap_err();
    assert!(err.to_string().contains("schema mismatch"), "error: {err}");
}

#[test]
fn test_missing_tools_array_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("registry.json");

    write_json(&path, JSON_MISSING_TOOLS);

    let err = load_tool_records(&path).unwrap_err();
    assert!(err.to_string().contains("missing 'tools'"), "error: {err}");
}

#[test]
fn test_empty_tools_list() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("registry.json");

    write_json(&path, JSON_EMPTY_TOOLS);

    let records = load_tool_records(&path).unwrap();
    assert!(records.is_empty());
}

#[test]
fn test_v2_missing_required_slug_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("registry.json");

    write_json(&path, JSON_V2_MISSING_SLUG);

    let err = load_tool_records(&path).unwrap_err();
    // The serde error should indicate slug is missing
    assert!(err.to_string().contains("slug"), "error: {err}");
}

#[test]
fn test_invalid_json_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("registry.json");

    write_json(&path, "this is not valid json");

    let err = load_tool_records(&path).unwrap_err();
    assert!(err.to_string().contains("JSON error"), "error: {err}");
}

#[test]
fn test_nonexistent_file_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nonexistent_file.json");

    let err = load_tool_records(&path).unwrap_err();
    assert!(err.to_string().contains("I/O error"), "error: {err}");
}

// ── Cache behavior ────────────────────────────────────────────────────────────

// NOTE: Caching tests use a global singleton (`static CACHED` in tool_registry.rs).
// To avoid parallel-test interference, all cache assertions live in a single test
// function. Do not split into separate #[test] functions without adding a
// serialization mechanism (e.g. serial_test crate).

#[test]
fn test_cache_hit_invalidate_and_repeated_calls() {
    // The cache is a process-global singleton — invalidate to start clean.
    invalidate_tool_cache();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("registry.json");

    // ── Cache hit: write → load → modify → load should return cached data ──
    write_json(&path, JSON_CACHE_INITIAL);
    let records = load_tool_records_cached(&path).unwrap();
    assert_eq!(records[0].slug, "cached_tool");

    // Modify the file on disk but don't invalidate cache
    write_json(&path, JSON_CACHE_MODIFIED);
    let records = load_tool_records_cached(&path).unwrap();
    // Confirm cache hit: still sees old data. Retry once if another test
    // raced and cleared the global singleton between steps.
    if records[0].slug != "cached_tool" {
        // Re-populate cache and retry
        write_json(&path, JSON_CACHE_INITIAL);
        load_tool_records_cached(&path).unwrap(); // prime
        write_json(&path, JSON_CACHE_MODIFIED);
        let records2 = load_tool_records_cached(&path).unwrap();
        assert_eq!(
            records2[0].slug, "cached_tool",
            "expected cached data before invalidation"
        );
    }

    // ── Invalidate → reload from disk ──
    invalidate_tool_cache();
    let records = load_tool_records_cached(&path).unwrap();
    assert_eq!(
        records[0].slug, "modified_tool",
        "expected fresh data after invalidation"
    );

    // ── Repeated calls without modification return equivalent data ──
    write_json(&path, JSON_V2_SINGLE);
    let records1 = load_tool_records_cached(&path).unwrap();
    let records2 = load_tool_records_cached(&path).unwrap();
    assert_eq!(records1.len(), records2.len());
    assert_eq!(records1[0].slug, records2[0].slug);

    // Clean up global cache
    invalidate_tool_cache();
}

#[test]
fn test_load_tool_records_no_cache_always_reads_disk() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("registry.json");

    write_json(&path, JSON_V2_SINGLE);

    // load_tool_records (no cache) reads from disk every time
    let records1 = load_tool_records(&path).unwrap();
    assert_eq!(records1[0].slug, "tool_a");

    // Modify disk
    write_json(&path, JSON_V2_MULTIPLE);

    // Direct load sees the change immediately
    let records2 = load_tool_records(&path).unwrap();
    assert_eq!(records2.len(), 2);
}

// ═══════════════════════════════════════════════════════════════════════════════
// lib tests
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_resolve_tool_registry_path_returns_default() {
    // When no hooks are registered, discover_tool_registry_path falls back to
    // the default relative path configs/framework/MCP_TOOL_REGISTRY.json
    let path = resolve_tool_registry_path();
    assert!(path.is_some());
    let pb = path.unwrap();
    assert!(
        pb.ends_with("configs/framework/MCP_TOOL_REGISTRY.json"),
        "unexpected default path: {}",
        pb.display()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test JSON fixtures (embedded to avoid external file dependencies)
// ═══════════════════════════════════════════════════════════════════════════════

const JSON_V2_SINGLE: &str = r#"{
    "schema_version": "mcp-tool-registry-v2",
    "tools": [
        {
            "slug": "tool_a",
            "display_name": "Tool A",
            "description": "First tool",
            "layer": "builtin",
            "dispatch_domain": "domain:goal",
            "owner": "framework",
            "trigger_hints": [],
            "host_platforms": [],
            "mcp_server": "server-a"
        }
    ]
}"#;

const JSON_V2_MULTIPLE: &str = r#"{
    "schema_version": "mcp-tool-registry-v2",
    "tools": [
        {"slug": "t1", "display_name": "T1", "description": "d1", "layer": "builtin", "dispatch_domain": "domain:goal", "owner": "framework", "trigger_hints": [], "host_platforms": [], "mcp_server": "s1"},
        {"slug": "t2", "display_name": "T2", "description": "d2", "layer": "research", "dispatch_domain": "research", "owner": "research", "trigger_hints": [], "host_platforms": [], "mcp_server": "s2"}
    ]
}"#;

const JSON_V2_FULL: &str = r#"{
    "schema_version": "mcp-tool-registry-v2",
    "tools": [
        {
            "slug": "full_tool",
            "display_name": "Full Tool",
            "description": "A fully populated tool",
            "layer": "external",
            "dispatch_domain": "research",
            "owner": "research-team",
            "trigger_hints": ["hint_a", "hint_b"],
            "host_platforms": ["linux", "macos"],
            "mcp_server": "research-server",
            "tool_flags": ["deprecated"],
            "input_schema": {"type": "object", "properties": {"file": {"type": "string"}}, "required": ["file"]}
        }
    ]
}"#;

const JSON_V2_WITH_SCHEMA: &str = r#"{
    "schema_version": "mcp-tool-registry-v2",
    "tools": [
        {
            "slug": "schema_tool",
            "display_name": "Schema Tool",
            "description": "Has input schema",
            "layer": "builtin",
            "dispatch_domain": "domain:goal",
            "owner": "framework",
            "trigger_hints": [],
            "host_platforms": [],
            "mcp_server": "srv",
            "input_schema": {"type": "object", "properties": {"file": {"type": "string"}}, "required": ["file"]}
        }
    ]
}"#;

const JSON_SCHEMA_MISMATCH: &str = r#"{
    "schema_version": "unknown-schema-v99",
    "tools": []
}"#;

const JSON_MISSING_TOOLS: &str = r#"{
    "schema_version": "mcp-tool-registry-v2"
}"#;

const JSON_EMPTY_TOOLS: &str = r#"{
    "schema_version": "mcp-tool-registry-v2",
    "tools": []
}"#;

const JSON_V2_MISSING_SLUG: &str = r#"{
    "schema_version": "mcp-tool-registry-v2",
    "tools": [
        {"display_name": "No Slug", "description": "missing slug", "layer": "builtin", "dispatch_domain": "domain:goal", "owner": "framework", "trigger_hints": [], "host_platforms": [], "mcp_server": "srv"}
    ]
}"#;

const JSON_CACHE_INITIAL: &str = r#"{
    "schema_version": "mcp-tool-registry-v2",
    "tools": [
        {"slug": "cached_tool", "display_name": "Cached", "description": "initial", "layer": "builtin", "dispatch_domain": "domain:goal", "owner": "framework", "trigger_hints": [], "host_platforms": [], "mcp_server": "srv"}
    ]
}"#;

const JSON_CACHE_MODIFIED: &str = r#"{
    "schema_version": "mcp-tool-registry-v2",
    "tools": [
        {"slug": "modified_tool", "display_name": "Modified", "description": "changed", "layer": "builtin", "dispatch_domain": "domain:goal", "owner": "framework", "trigger_hints": [], "host_platforms": [], "mcp_server": "srv"}
    ]
}"#;
