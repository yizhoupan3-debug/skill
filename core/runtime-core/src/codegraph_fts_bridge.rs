//! Codegraph FTS-accelerated MCP tool registry lookup (v6.5 integration).
//!
//! Bridges `CodeGraphIndex::search_mcp_tool` with
//! `managed_mcp_server_for_tool` from framework-kernel,
//! providing an O(1) indexed lookup path with linear-scan fallback.

use codegraph_rs::CodeGraphIndex;
use serde_json::Value;

use crate::runtime_registry::{
    managed_mcp_server_for_tool, parse_host_mcp_tool_fqn,
};

/// Resolve a bare MCP tool name to its managed server id via codegraph FTS index.
///
/// Attempts the indexed lookup first (O(1) symbol match); falls back to
/// the linear scan from `managed_mcp_server_for_tool` when the tool
/// is not yet indexed.
pub fn managed_mcp_server_for_tool_with_fts(
    index: &CodeGraphIndex,
    registry: &Value,
    tool_name: &str,
) -> Option<String> {
    if let Some(server_id) = index.search_mcp_tool(tool_name) {
        return Some(server_id);
    }
    managed_mcp_server_for_tool(registry, tool_name)
}

/// FTS-accelerated version of `resolves_managed_mcp_tool`.
///
/// Supports both bare tool names and Cursor-style FQNs
/// (`mcp__{server_id}__{tool_name}`).
pub fn resolves_managed_mcp_tool_with_fts(
    index: &CodeGraphIndex,
    registry: &Value,
    tool_name_or_fqn: &str,
) -> bool {
    let raw = tool_name_or_fqn.trim();
    if raw.is_empty() {
        return false;
    }
    if let Some((server_id, tool_name)) = parse_host_mcp_tool_fqn(raw) {
        return managed_mcp_server_for_tool_with_fts(index, registry, &tool_name).as_deref()
            == Some(server_id.as_str());
    }
    managed_mcp_server_for_tool_with_fts(index, registry, raw).is_some()
}

/// Ingest all MCP tools from RUNTIME_REGISTRY.json into codegraph FTS index.
///
/// Idempotent — deletes existing `mcp_tool` nodes before inserting.
/// Returns the number of tools ingested.
pub fn ingest_mcp_tools_from_registry(
    index: &CodeGraphIndex,
    registry: &Value,
) -> Result<usize, String> {
    index.ingest_mcp_tools(registry).map_err(|e| e.to_string())
}

/// Ingest all skills from SKILL_MANIFEST.json into codegraph FTS index.
///
/// Idempotent — deletes existing `skill` nodes before inserting.
/// Returns the number of skills ingested.
pub fn ingest_skills_from_manifest(
    index: &CodeGraphIndex,
    manifest: &Value,
) -> Result<usize, String> {
    index.ingest_skills(manifest).map_err(|e| e.to_string())
}

/// Find a skill by slug via codegraph index.
pub fn find_skill_with_fts(index: &CodeGraphIndex, slug: &str) -> Option<codegraph_rs::Node> {
    index.find_skill(slug)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_index() -> (std::path::PathBuf, CodeGraphIndex) {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("fts-bridge-{suffix}"));
        std::fs::create_dir_all(&root).expect("create temp dir");
        let index = CodeGraphIndex::open(&root).expect("open index");
        (root, index)
    }

    fn sample_registry() -> Value {
        json!({
            "managed_mcp_servers": {
                "router-rs-framework": {
                    "tools": ["framework_snapshot", "skill_route", "skill_read"]
                },
                "browser-mcp": {
                    "tools": ["browser_open", "browser_click"]
                },
                "mcp-codegraph": {
                    "tools": ["codegraph_search", "codegraph_status"]
                }
            }
        })
    }

    #[test]
    fn fts_lookup_resolves_all_server_groups() {
        let (root, index) = temp_index();
        let registry = sample_registry();
        let count = index.ingest_mcp_tools(&registry).expect("ingest");
        assert_eq!(count, 7);

        assert_eq!(
            managed_mcp_server_for_tool_with_fts(&index, &registry, "framework_snapshot"),
            Some("router-rs-framework".to_string())
        );
        assert_eq!(
            managed_mcp_server_for_tool_with_fts(&index, &registry, "browser_open"),
            Some("browser-mcp".to_string())
        );
        assert_eq!(
            managed_mcp_server_for_tool_with_fts(&index, &registry, "codegraph_search"),
            Some("mcp-codegraph".to_string())
        );
        assert_eq!(
            managed_mcp_server_for_tool_with_fts(&index, &registry, "unknown_tool"),
            None
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fts_resolves_fqn() {
        let (root, index) = temp_index();
        let registry = sample_registry();
        index.ingest_mcp_tools(&registry).expect("ingest");

        assert!(resolves_managed_mcp_tool_with_fts(
            &index,
            &registry,
            "mcp__mcp-codegraph__codegraph_search"
        ));
        assert!(resolves_managed_mcp_tool_with_fts(
            &index,
            &registry,
            "mcp__browser-mcp__browser_click"
        ));
        assert!(!resolves_managed_mcp_tool_with_fts(
            &index,
            &registry,
            "mcp__mcp-codegraph__nonexistent"
        ));
        assert!(!resolves_managed_mcp_tool_with_fts(
            &index,
            &registry,
            ""
        ));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fallback_to_linear_scan_when_not_indexed() {
        let (root, index) = temp_index();
        let registry = sample_registry();
        // Do NOT ingest — the index is empty
        // Should still resolve via linear fallback
        assert_eq!(
            managed_mcp_server_for_tool_with_fts(&index, &registry, "framework_snapshot"),
            Some("router-rs-framework".to_string())
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
