//! Tool routing process: routing decision and candidate types.
//!
//! These types are produced by the tool routing engine and consumed by
//! dispatch handlers. `McpToolRecord` (the registry/data type) lives in
//! `mcp-tool-registry`; routing-only types live here in the routing layer.

use serde::{Deserialize, Serialize};

/// Routing decision output for tool selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDecision {
    pub decision_schema_version: String,
    pub selected_tool: String,
    pub score: f64,
    pub reasons: Vec<String>,
    pub matched_token_count: usize,
    pub dispatch_domain: String,
    pub mcp_server: String,
    /// True if this decision was produced by fuzzy (trigram) rescue.
    #[serde(default)]
    pub fuzzy_match: bool,
}

/// A candidate tool with its score during routing.
#[derive(Debug, Clone)]
pub struct ToolCandidate<'a> {
    pub record: &'a mcp_tool_registry::McpToolRecord,
    pub score: f64,
    pub reasons: Vec<String>,
    pub matched_token_count: usize,
}
