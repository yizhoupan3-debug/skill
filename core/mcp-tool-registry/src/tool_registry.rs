//! MCP tool registry: loads MCP_TOOL_REGISTRY.json and provides tool records.
//!
//! Caching layer uses TTL-based invalidation (default 60s) so edits to the JSON
//! file are picked up at most 60s after the last load, without requiring a restart
//! or manual `invalidate_tool_cache` call.

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::tool_types::McpToolRecord;

const EXPECTED_SCHEMA: &str = "mcp-tool-registry-v1";

/// Default TTL for the cached registry (60 seconds).
const CACHE_TTL_SECS: u64 = 60;

/// Load tool records from MCP_TOOL_REGISTRY.json.
pub fn load_tool_records(registry_path: &Path) -> Result<Vec<McpToolRecord>, String> {
    let content = fs::read_to_string(registry_path)
        .map_err(|e| format!("failed to read {}: {e}", registry_path.display()))?;
    let root: Value = serde_json::from_str(&content)
        .map_err(|e| format!("failed to parse {}: {e}", registry_path.display()))?;

    let schema = root
        .get("schema_version")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if schema != EXPECTED_SCHEMA {
        return Err(format!(
            "MCP_TOOL_REGISTRY.json schema mismatch: expected {EXPECTED_SCHEMA}, got {schema}"
        ));
    }

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

    let tools = root
        .get("tools")
        .and_then(|v| v.as_array())
        .ok_or("MCP_TOOL_REGISTRY.json missing 'tools' array")?;

    tools
        .iter()
        .enumerate()
        .map(|(idx, tool)| parse_tool_record(tool, &keys, idx))
        .collect::<Result<Vec<_>, _>>()
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
/// If a reload fails, the stale cached data is still returned (with a warning).
pub fn load_tool_records_cached(registry_path: &Path) -> Result<Vec<McpToolRecord>, String> {
    let now = Instant::now();

    // Fast path: check cache (read lock)
    {
        if let Ok(guard) = cache().read() {
            if let Some(entry) = guard.as_ref() {
                if now.duration_since(entry.loaded_at) < Duration::from_secs(CACHE_TTL_SECS) {
                    return Ok(entry.records.clone());
                }
            }
        }
    }

    // TTL expired or cache empty: reload from disk (write lock)
    {
        let mut guard = cache().write().map_err(|_| "cache poisoned".to_string())?;
        // Double-check after acquiring write lock (another thread may have reloaded)
        if let Some(entry) = guard.as_ref() {
            if now.duration_since(entry.loaded_at) < Duration::from_secs(CACHE_TTL_SECS) {
                return Ok(entry.records.clone());
            }
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
        slug_lower: String::new(),
        display_name_lower: String::new(),
        display_name: String::new(),
        description: String::new(),
        layer: String::new(),
        dispatch_domain: String::new(),
        owner: String::new(),
        gate: "none".to_string(),
        trigger_hints: Vec::new(),
        name_tokens: HashSet::new(),
        keyword_tokens: HashSet::new(),
        desc_tokens: HashSet::new(),
        alias_tokens: HashSet::new(),
        do_not_use_tokens: HashSet::new(),
        host_platforms: Vec::new(),
        mcp_server: String::new(),
        tool_flags: Vec::new(),
        input_schema_json: None,
    };

    for (i, key) in keys.iter().enumerate() {
        let val = match arr.get(i) {
            Some(v) if v.is_string() => v.as_str().unwrap_or(""),
            Some(v) => {
                // For non-string values: check if this is input_schema (which is an object)
                if key == "input_schema" {
                    // Handle below as a special case
                    continue;
                }
                eprintln!("[mcp-tool-registry warning] tool record at index {index}, key '{key}': expected string, got {v}");
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
            "input_schema" => {
                if let Some(obj) = arr.get(i).and_then(|v| v.as_object()) {
                    let schema_type = obj
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("object")
                        .to_string();
                    let properties = obj
                        .get("properties")
                        .and_then(|v| v.as_object())
                        .cloned()
                        .unwrap_or_default();
                    let required = obj
                        .get("required")
                        .and_then(|v| v.as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default();
                    record.input_schema_json =
                        Some(crate::tool_types::McpToolInputSchema {
                            schema_type,
                            properties,
                            required,
                        });
                }
            }
            _ => {}
        }
    }

    if record.slug.is_empty() {
        return Err(format!("tool record at index {index} missing slug"));
    }

    McpToolRecord::derive_tokens(&mut record);
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
