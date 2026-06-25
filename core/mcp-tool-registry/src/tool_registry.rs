//! MCP tool registry: loads MCP_TOOL_REGISTRY.json and provides tool records.
//!
//! Caching layer uses TTL-based invalidation (default 60s) so edits to the JSON
//! file are picked up at most 60s after the last load, without requiring a restart
//! or manual `invalidate_tool_cache` call.

use std::fs;
use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::tool_types::McpToolRecord;

pub const EXPECTED_SCHEMA: &str = "mcp-tool-registry-v1";
const EXPECTED_SCHEMA_V2: &str = "mcp-tool-registry-v2";

/// Default TTL for the cached registry (60 seconds).
const CACHE_TTL_SECS: u64 = 60;

/// Load tool records from MCP_TOOL_REGISTRY.json.
/// Supports both v1 (columnar array) and v2 (object) formats — auto-detected
/// by checking whether the first tool entry is an array or object.
pub fn load_tool_records(registry_path: &Path) -> Result<Vec<McpToolRecord>, String> {
    let content = fs::read_to_string(registry_path)
        .map_err(|e| format!("failed to read {}: {e}", registry_path.display()))?;
    let root: Value = serde_json::from_str(&content)
        .map_err(|e| format!("failed to parse {}: {e}", registry_path.display()))?;

    let schema = root
        .get("schema_version")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if schema != EXPECTED_SCHEMA && schema != EXPECTED_SCHEMA_V2 {
        return Err(format!(
            "MCP_TOOL_REGISTRY.json schema mismatch: expected {EXPECTED_SCHEMA} or {EXPECTED_SCHEMA_V2}, got {schema}"
        ));
    }

    let tools = root
        .get("tools")
        .and_then(|v| v.as_array())
        .ok_or("MCP_TOOL_REGISTRY.json missing 'tools' array")?;

    if tools.is_empty() {
        return Ok(Vec::new());
    }

    // Auto-detect format: object → v2 (direct serde), array → v1 (columnar)
    match &tools[0] {
        Value::Object(_) => tools
            .iter()
            .enumerate()
            .map(|(idx, tool)| {
                serde_json::from_value::<McpToolRecord>(tool.clone())
                    .map_err(|e| format!("tool record at index {idx} is invalid: {e}"))
            })
            .collect(),
        Value::Array(_) => {
            let keys: Vec<String> = root
                .get("keys")
                .and_then(|v| v.as_array())
                .ok_or("MCP_TOOL_REGISTRY.json missing 'keys' array")?
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            if keys.is_empty() {
                return Err("MCP_TOOL_REGISTRY.json has empty 'keys' array".to_string());
            }
            tools
                .iter()
                .enumerate()
                .map(|(idx, tool)| parse_tool_record(tool, &keys, idx))
                .collect::<Result<Vec<_>, _>>()
        }
        _ => Err(
            "MCP_TOOL_REGISTRY.json 'tools' entries must be arrays or objects".to_string(),
        ),
    }
}

/// Cache entry: the loaded records plus when they were loaded.
struct CacheEntry {
    records: Vec<McpToolRecord>,
    loaded_at: Instant,
}

/// Process-level TTL cache. Safe to call from multiple threads.
static CACHED: OnceLock<std::sync::RwLock<Option<CacheEntry>>> = OnceLock::new();

fn cache() -> &'static std::sync::RwLock<Option<CacheEntry>> {
    CACHED.get_or_init(|| std::sync::RwLock::new(None))
}

/// Load tool records with process-level TTL caching.
///
/// First call loads from disk; subsequent calls within `CACHE_TTL_SECS` return
/// the cached result. After the TTL expires, the next call reloads from disk.
/// Returns `Err` on reload failure (stale cache data is preserved, so a
/// subsequent call after the TTL will retry).
pub fn load_tool_records_cached(registry_path: &Path) -> Result<Vec<McpToolRecord>, String> {
    let now = Instant::now();

    // Fast path: check cache (read lock)
    {
        if let Ok(guard) = cache().read()
            && let Some(entry) = guard.as_ref()
                && now.duration_since(entry.loaded_at) < Duration::from_secs(CACHE_TTL_SECS) {
                    return Ok(entry.records.clone());
                }
    }

    // TTL expired or cache empty: reload from disk (write lock)
    {
        let mut guard = cache().write().map_err(|_| "cache poisoned".to_string())?;
        // Double-check after acquiring write lock (another thread may have reloaded)
        if let Some(entry) = guard.as_ref()
            && now.duration_since(entry.loaded_at) < Duration::from_secs(CACHE_TTL_SECS) {
                return Ok(entry.records.clone());
            }
        let records = load_tool_records(registry_path)?;
        guard.replace(CacheEntry {
            records: records.clone(),
            loaded_at: Instant::now(),
        });
        Ok(records)
    }
}

/// Manually invalidate the TTL cache, forcing the next `load_tool_records_cached`
/// call to reload from disk. Useful for testing or after explicit edits.
pub fn invalidate_tool_cache() {
    if let Ok(mut guard) = cache().write() {
        *guard = None;
    }
}

fn parse_tool_record(row: &Value, keys: &[String], index: usize) -> Result<McpToolRecord, String> {
    let arr = row
        .as_array()
        .ok_or_else(|| format!("tool record at index {index} is not an array"))?;

    let mut record = McpToolRecord {
        slug: String::new(),
        display_name: String::new(),
        description: String::new(),
        layer: String::new(),
        dispatch_domain: String::new(),
        owner: String::new(),
        gate: "none".to_string(),
        trigger_hints: Vec::new(),
        host_platforms: Vec::new(),
        mcp_server: String::new(),
        tool_flags: Vec::new(),
        input_schema_json: None,
    };

    for (i, key) in keys.iter().enumerate() {
        // Handle input_schema (JSON object) before string-based val extraction,
        // because the object value would trigger a `continue` in the val match
        // below (non-string branch), skipping the handler entirely.
        if key == "input_schema" {
            if let Some(obj) = arr.get(i).and_then(|v| v.as_object()) {
                record.input_schema_json = Some(crate::tool_types::McpToolInputSchema {
                    schema_type: obj
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("object")
                        .to_string(),
                    properties: obj
                        .get("properties")
                        .and_then(|v| v.as_object())
                        .cloned()
                        .unwrap_or_default(),
                    required: obj
                        .get("required")
                        .and_then(|v| v.as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default(),
                });
            }
            continue;
        }

        let val = match arr.get(i) {
            Some(v) if v.is_string() => v.as_str().unwrap_or(""),
            Some(v) => {
                tracing::warn!(
                    "[mcp-tool-registry warning] tool record at index {index}, key '{key}': expected string, got {v}"
                );
                ""
            }
            None => "",
        };
        match key.as_str() {
            "slug" => record.slug = val.to_string(),
            "display_name" => record.display_name = val.to_string(),
            "description" => record.description = val.to_string(),
            "layer" => record.layer = val.to_string(),
            "dispatch_domain" => record.dispatch_domain = val.to_string(),
            "owner" => record.owner = val.to_string(),
            "gate" => record.gate = val.to_string(),
            "trigger_hints" => {
                record.trigger_hints = parse_string_array(arr.get(i));
            }
            "host_platforms" => {
                record.host_platforms = parse_string_array(arr.get(i));
            }
            "mcp_server" => record.mcp_server = val.to_string(),
            "tool_flags" => {
                record.tool_flags = parse_string_array(arr.get(i));
            }
            // input_schema is handled above before the val match
            "input_schema" => unreachable!(),
            _ => {}
        }
    }

    if record.slug.is_empty() {
        return Err(format!("tool record at index {index} missing slug"));
    }

    Ok(record)
}

fn parse_string_array(val: Option<&Value>) -> Vec<String> {
    val.and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}
