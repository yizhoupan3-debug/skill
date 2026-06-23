//! Tool record types.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRecord {
    pub slug: String,
    pub kind: String,
    pub binary: String,
    pub mcp_endpoint: String,
    pub description: String,
    pub trigger_hints: Vec<String>,
    pub gate: String,
    pub host_platforms: Vec<String>,
    pub name_tokens: HashSet<String>,
    pub keyword_tokens: HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDecision {
    pub selected_tool: String,
    pub score: f64,
    pub reasons: Vec<String>,
    pub mcp_endpoint: String,
    pub binary: String,
}
