//! Tool record loading from TOOL_REGISTRY.json.

use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::tool_types::ToolRecord;

const EXPECTED_SCHEMA: &str = "tool-registry-v1";

/// Load tool records from TOOL_REGISTRY.json.
pub fn load_tool_records(registry_path: &Path) -> Result<Vec<ToolRecord>, String> {
    let content = fs::read_to_string(registry_path)
        .map_err(|e| format!("failed to read {}: {e}", registry_path.display()))?;
    let root: Value = serde_json::from_str(&content)
        .map_err(|e| format!("failed to parse {}: {e}", registry_path.display()))?;

    let schema = root.get("schema_version").and_then(|v| v.as_str()).unwrap_or("");
    if schema != EXPECTED_SCHEMA {
        return Err(format!(
            "TOOL_REGISTRY.json schema mismatch: expected {EXPECTED_SCHEMA}, got {schema}"
        ));
    }

    let tools = root.get("tools").and_then(|v| v.as_array()).ok_or_else(|| {
        format!(
            "TOOL_REGISTRY.json missing tools array: {}",
            registry_path.display()
        )
    })?;

    tools
        .iter()
        .map(|tool| parse_tool_record(tool))
        .collect::<Result<Vec<_>, _>>()
}

fn parse_tool_record(tool: &Value) -> Result<ToolRecord, String> {
    let slug = tool.get("slug").and_then(|v| v.as_str()).ok_or("tool missing slug")?;
    let kind = tool.get("kind").and_then(|v| v.as_str()).unwrap_or("tool");
    let binary = tool.get("binary").and_then(|v| v.as_str()).unwrap_or("");
    let mcp_endpoint = tool.get("mcp_endpoint").and_then(|v| v.as_str()).unwrap_or("");
    let description = tool.get("description").and_then(|v| v.as_str()).unwrap_or("");
    let gate = tool.get("gate").and_then(|v| v.as_str()).unwrap_or("none");
    let host_platforms = tool
        .get("host_platforms")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let trigger_hints: Vec<String> = tool
        .get("trigger_hints")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let slug_lower = slug.to_lowercase();
    let name_tokens: HashSet<String> = slug_lower
        .split(|c: char| c == '-' || c == '_')
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
        .collect();

    let keyword_tokens: HashSet<String> = trigger_hints
        .iter()
        .flat_map(|hint| {
            hint.to_lowercase()
                .split(|c: char| c.is_whitespace() || c == ',' || c == '，')
                .filter(|t| !t.is_empty())
                .map(|t| t.to_string())
                .collect::<Vec<_>>()
        })
        .collect();

    Ok(ToolRecord {
        slug: slug.to_string(),
        kind: kind.to_string(),
        binary: binary.to_string(),
        mcp_endpoint: mcp_endpoint.to_string(),
        description: description.to_string(),
        trigger_hints,
        gate: gate.to_string(),
        host_platforms,
        name_tokens,
        keyword_tokens,
    })
}
