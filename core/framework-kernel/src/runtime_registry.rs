//! Disk-primary [`RUNTIME_REGISTRY.json`](../../../configs/framework/RUNTIME_REGISTRY.json) loader.
//! Unified entry for hook hot paths, host targets, and host integration (ADR-005).

use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub const RUNTIME_REGISTRY_SCHEMA_VERSION: &str = "framework-runtime-registry-v2";
pub const RUNTIME_REGISTRY_PATH: &str = "configs/framework/RUNTIME_REGISTRY.json";
pub const HOST_ADAPTER_CONTRACT_PATH: &str = "docs/spec.md";

// ---------------------------------------------------------------------------
// Typed registry subset (host integration)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeRegistry {
    #[serde(rename = "schema_version")]
    _schema_version: String,
    #[serde(default)]
    pub workspace_bootstrap_defaults: RuntimeWorkspaceBootstrapDefaults,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RuntimeWorkspaceBootstrapDefaults {
    #[serde(default)]
    pub skills: RuntimeSkillsDefaults,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RuntimeSkillsDefaults {
    #[serde(default)]
    pub source_rel: Option<String>,
}

pub fn runtime_registry_path(repo_root: &Path) -> Result<PathBuf, String> {
    let repo_candidate = repo_root.join(RUNTIME_REGISTRY_PATH);
    if repo_candidate.is_file() {
        return Ok(repo_candidate);
    }
    Err(format!(
        "Runtime registry not found at active workspace root: {}. Expected {}. Fix by opening the framework repo root as the active workspace or passing --framework-root <framework-repo-root>.",
        repo_root.to_string_lossy(),
        repo_candidate.to_string_lossy()
    ))
}

pub fn load_runtime_registry_json(framework_root: &Path) -> Result<Value, String> {
    let path = framework_root.join(RUNTIME_REGISTRY_PATH);
    if !path.is_file() {
        return Err(format!(
            "runtime registry not found under framework root {} (expected {})",
            framework_root.display(),
            path.display()
        ));
    }
    let payload = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let parsed: Value = serde_json::from_str(&payload).map_err(|e| {
        format!(
            "invalid JSON in {}: {e}; see {HOST_ADAPTER_CONTRACT_PATH}",
            path.display()
        )
    })?;
    let sv = parsed
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!(
                "RUNTIME_REGISTRY.json missing schema_version at {}",
                path.display()
            )
        })?;
    if sv != RUNTIME_REGISTRY_SCHEMA_VERSION {
        return Err(format!(
            "unsupported RUNTIME_REGISTRY schema_version {:?} at {}",
            sv,
            path.display()
        ));
    }
    Ok(parsed)
}

pub fn load_runtime_registry_payload(repo_root: &Path) -> Result<Value, String> {
    match runtime_registry_path(repo_root) {
        Ok(_) => {}
        Err(e) => return Err(e),
    }
    load_runtime_registry_json(repo_root)
}

pub fn load_runtime_registry_payload_if_repo_local(
    repo_root: &Path,
) -> Result<Option<Value>, String> {
    let path = repo_root.join(RUNTIME_REGISTRY_PATH);
    if !path.is_file() {
        return Ok(None);
    }
    let payload = fs::read_to_string(&path).map_err(|err| err.to_string())?;
    let parsed = serde_json::from_str::<Value>(&payload).map_err(|err| err.to_string())?;
    let schema_version = parsed
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!(
                "Runtime registry missing schema_version at {}",
                path.to_string_lossy()
            )
        })?;
    if schema_version != RUNTIME_REGISTRY_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported runtime registry schema_version {:?} at {}",
            schema_version,
            path.to_string_lossy()
        ));
    }
    Ok(Some(parsed))
}

pub fn load_runtime_registry(repo_root: &Path) -> Result<RuntimeRegistry, String> {
    let payload = load_runtime_registry_payload(repo_root)?;
    serde_json::from_value::<RuntimeRegistry>(payload).map_err(|err| err.to_string())
}

/// Map MCP stdio host spellings to `host_projections` keys (avoid `hosts` import cycle).
fn registry_projection_host_key(host_id: &str) -> &str {
    host_id.trim()
}

pub fn host_projection_object<'a>(
    registry: &'a Value,
    host_id: &str,
) -> Option<&'a serde_json::Map<String, Value>> {
    let key = registry_projection_host_key(host_id);
    registry
        .get("host_projections")
        .and_then(|v| v.get(key))
        .and_then(Value::as_object)
}

pub fn harness_capability_exception_entry<'a>(
    projection: &'a serde_json::Map<String, Value>,
    cap: &str,
) -> Option<&'a Value> {
    projection
        .get("harness_capability_exceptions")
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter()
                .find(|row| row.get("cap").and_then(Value::as_str) == Some(cap))
        })
}

pub fn harness_capability_exception_rationale(
    repo_root: &Path,
    host_id: &str,
    cap: &str,
) -> Option<String> {
    let registry = load_runtime_registry_payload(repo_root).ok()?;
    let projection = host_projection_object(&registry, host_id)?;
    let entry = harness_capability_exception_entry(projection, cap)?;
    entry
        .get("rationale")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Resolve a bare MCP tool name (e.g. `codegraph_search`) to a managed server id.
pub fn managed_mcp_server_for_tool(registry: &Value, tool_name: &str) -> Option<String> {
    let normalized = tool_name.trim();
    if normalized.is_empty() {
        return None;
    }
    let managed = registry.get("managed_mcp_servers")?.as_object()?;
    for (server_id, entry) in managed {
        let tools = entry.get("tools").and_then(Value::as_array)?;
        if tools.iter().any(|tool| tool.as_str() == Some(normalized)) {
            return Some(server_id.clone());
        }
    }
    None
}

/// Return all managed MCP server IDs from the registry.
pub fn managed_mcp_server_ids(registry: &Value) -> Vec<String> {
    registry
        .get("managed_mcp_servers")
        .and_then(Value::as_object)
        .map(|obj| obj.keys().cloned().collect())
        .unwrap_or_default()
}

/// Parse Cursor-style MCP tool FQN: `mcp__{server_id}__{tool_name}`.
pub fn parse_host_mcp_tool_fqn(fqn: &str) -> Option<(String, String)> {
    let rest = fqn.strip_prefix("mcp__")?;
    let (server_id, tool_name) = rest.rsplit_once("__")?;
    if server_id.is_empty() || tool_name.is_empty() {
        return None;
    }
    Some((server_id.to_string(), tool_name.to_string()))
}

/// True when `tool_name` or host FQN resolves to a registry-managed MCP server.
pub fn resolves_managed_mcp_tool(registry: &Value, tool_name_or_fqn: &str) -> bool {
    let raw = tool_name_or_fqn.trim();
    if raw.is_empty() {
        return false;
    }
    if let Some((server_id, tool_name)) = parse_host_mcp_tool_fqn(raw) {
        return managed_mcp_server_for_tool(registry, &tool_name).as_deref()
            == Some(server_id.as_str());
    }
    managed_mcp_server_for_tool(registry, raw).is_some()
}

pub fn closeout_evidence_hooks_unsupported_on_host(repo_root: &Path, host_id: &str) -> bool {
    let Ok(registry) = load_runtime_registry_payload(repo_root) else {
        return false;
    };
    let Some(projection) = host_projection_object(&registry, host_id) else {
        return false;
    };
    harness_capability_exception_entry(projection, "closeout_evidence_hooks")
        .and_then(|row| row.get("status"))
        .and_then(Value::as_str)
        == Some("unsupported")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn registry_projection_host_key_passes_through_unknown() {
        assert_eq!(
            registry_projection_host_key("claude-desktop"),
            "claude-desktop"
        );
        assert_eq!(
            registry_projection_host_key("claude"),
            "claude"
        );
    }

    #[test]
    fn managed_mcp_tool_semantic_dispatch_resolves_codegraph_tools() {
        let registry = json!({
            "managed_mcp_servers": {
                "mcp-codegraph": {
                    "tools": ["codegraph_search", "codegraph_status"]
                }
            }
        });
        assert_eq!(
            managed_mcp_server_for_tool(&registry, "codegraph_search").as_deref(),
            Some("mcp-codegraph")
        );
        assert!(resolves_managed_mcp_tool(
            &registry,
            "mcp__mcp-codegraph__codegraph_status"
        ));
        assert!(!resolves_managed_mcp_tool(&registry, "grep"));
    }
}

// Review gate re-exports (core_policy::registry_review_gate) remain in
// runtime-core to avoid adding core-policy as a framework-kernel dependency.
