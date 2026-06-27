//! Disk-primary [`RUNTIME_REGISTRY.json`](../../../configs/framework/RUNTIME_REGISTRY.json) loader.
//! Unified entry for hook hot paths, host targets, and host integration (ADR-005).
//!
//! ## Generated tables
//!
//! Per-host lookup functions (`host_private_config_dir`, `review_gate_disable_env`,
//! `paper_prose_env`, `paper_adversarial_env`, `settings_guarded_paths`,
//! `generated_entrypoint_paths`) and constants (`ALL_KNOWN_HOST_DIRS`,
//! `EPHEMERAL_PATH_PATTERNS`, `EPHEMERAL_TASK_PREFIXES`) are **generated** from
//! `configs/framework/RUNTIME_REGISTRY.json` at compile time. Adding a new host
//! requires only editing the registry — the generated code stays in sync automatically.

use core_errors::FrameworkError;
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

// ── Generated host tables (compile-time from RUNTIME_REGISTRY.json) ──
// Includes: ALL_KNOWN_HOST_DIRS, EPHEMERAL_PATH_PATTERNS, EPHEMERAL_TASK_PREFIXES,
// is_ephemeral_task_id, host_home_dirs, host_private_config_dir, review_gate_disable_env,
// paper_prose_env, paper_adversarial_env, settings_guarded_paths, generated_entrypoint_paths.
include!(concat!(env!("OUT_DIR"), "/generated_host_tables.rs"));

// ── Generated stdio op tables (compile-time from RUNTIME_REGISTRY.json) ──
// Includes: classify_stdio_op_registry, STDIO_OP_DOMAINS.
include!(concat!(env!("OUT_DIR"), "/generated_stdio_tables.rs"));

pub const RUNTIME_REGISTRY_SCHEMA_VERSION: &str = "framework-runtime-registry-v2";
pub const RUNTIME_REGISTRY_PATH: &str = "configs/framework/RUNTIME_REGISTRY.json";

/// Canonical list of managed MCP server IDs. Used as fallback when registry is unavailable.
pub const DEFAULT_MANAGED_MCP_SERVER_IDS: &[&str] = &[
    "router-rs-framework",
    "browser-mcp",
    "mcp-codegraph",
    "paperplain",
];

/// Path to the host adapter contract spec (relative to framework root).
pub const HOST_ADAPTER_CONTRACT_PATH: &str = "AGENTS.md";

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

pub fn runtime_registry_path(repo_root: &Path) -> Result<PathBuf, FrameworkError> {
    let repo_candidate = repo_root.join(RUNTIME_REGISTRY_PATH);
    // Read-first pattern: check existence by attempting to read metadata,
    // avoiding TOCTOU between is_file() and subsequent open.
    match fs::metadata(&repo_candidate) {
        Ok(m) if m.is_file() => Ok(repo_candidate),
        Ok(_) => Err(FrameworkError::validation(format!(
            "Runtime registry path exists but is not a file: {}. Expected a regular file at {}.",
            repo_root.to_string_lossy(),
            repo_candidate.to_string_lossy()
        ))),
        Err(e) => Err(FrameworkError::validation(format!(
            "Runtime registry not found at active workspace root: {}. Expected {}. Fix by opening the framework repo root as the active workspace or passing --framework-root <framework-repo-root>. Error: {e}",
            repo_root.to_string_lossy(),
            repo_candidate.to_string_lossy()
        ))),
    }
}

pub fn load_runtime_registry_json(framework_root: &Path) -> Result<Value, FrameworkError> {
    let path = framework_root.join(RUNTIME_REGISTRY_PATH);
    // Read-first pattern: attempt to read directly, avoiding TOCTOU between is_file() and read_to_string().
    let payload = match fs::read_to_string(&path) {
        Ok(payload) => payload,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(FrameworkError::validation(format!(
                "runtime registry not found under framework root {} (expected {})",
                framework_root.display(),
                path.display()
            )));
        }
        Err(e) => {
            return Err(FrameworkError::validation(format!(
                "failed to read runtime registry {}: {e}",
                path.display()
            )));
        }
    };
    let parsed: Value = serde_json::from_str(&payload).map_err(|e| {
        FrameworkError::validation(format!(
            "invalid JSON in {}: {e}; see {HOST_ADAPTER_CONTRACT_PATH}",
            path.display()
        ))
    })?;
    let sv = parsed
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            FrameworkError::validation(format!(
                "RUNTIME_REGISTRY.json missing schema_version at {}",
                path.display()
            ))
        })?;
    if sv != RUNTIME_REGISTRY_SCHEMA_VERSION {
        return Err(FrameworkError::validation(format!(
            "unsupported RUNTIME_REGISTRY schema_version {:?} at {}",
            sv,
            path.display()
        )));
    }
    Ok(parsed)
}

pub fn load_runtime_registry_payload(repo_root: &Path) -> Result<Value, FrameworkError> {
    load_runtime_registry_json(repo_root).map_err(|e| {
        FrameworkError::validation(format!(
            "{e}. If the workspace root differs from the framework repo, \
             pass --framework-root <framework-repo-root> or open the framework repo as the active workspace."
        ))
    })
}

pub fn load_runtime_registry(repo_root: &Path) -> Result<RuntimeRegistry, FrameworkError> {
    let payload = load_runtime_registry_payload(repo_root)?;
    serde_json::from_value::<RuntimeRegistry>(payload).map_err(|err| FrameworkError::validation(err.to_string()))
}

/// Map MCP stdio host spellings to `host_projections` keys (avoid `hosts` import cycle).
///
/// Currently this only performs whitespace trimming.  In the future it may need
/// to normalize host identifiers — for example mapping `claude-desktop` to the
/// canonical `claude` projection key, or folding other host-family aliases —
/// once the registry's key convention stabilises.
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
