//! MCP tool registry: loads MCP_TOOL_REGISTRY.json and provides tool records.

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use serde_json::Value;

use crate::tool_types::McpToolRecord;

const EXPECTED_SCHEMA: &str = "mcp-tool-registry-v1";

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

/// Cached records singleton. Only caches successful loads.
static CACHED_RECORDS: OnceLock<Vec<McpToolRecord>> = OnceLock::new();

/// Load tool records with process-level caching. First successful call loads from disk;
/// subsequent calls return the cached result. If the first call fails, no cache is
/// stored and the next call will retry from disk.
pub fn load_tool_records_cached(registry_path: &Path) -> Result<&'static Vec<McpToolRecord>, String> {
    if let Some(cached) = CACHED_RECORDS.get() {
        return Ok(cached);
    }
    let records = load_tool_records(registry_path)?;
    // Only cache if load succeeded; race is safe with OnceLock.
    let _ = CACHED_RECORDS.set(records);
    Ok(CACHED_RECORDS.get().unwrap())
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
        name_tokens: HashSet::new(),
        keyword_tokens: HashSet::new(),
        desc_tokens: HashSet::new(),
        host_platforms: Vec::new(),
        mcp_server: String::new(),
        tool_flags: Vec::new(),
    };

    for (i, key) in keys.iter().enumerate() {
        let val = match arr.get(i) {
            Some(v) if v.is_string() => v.as_str().unwrap_or(""),
            Some(v) => {
                // Non-string value: warn and skip
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
