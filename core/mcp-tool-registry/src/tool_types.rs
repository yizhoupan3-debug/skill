//! MCP tool registry: unified tool types for the Tool Layer.
//!
//! ## Types moved to tool-routing-engine
//!
//! Routing-only types have been moved to the routing layer:
//! - `McpToolDecision` → `tool_routing_engine::types::McpToolDecision`
//! - `ToolCandidate` → `tool_routing_engine::types::ToolCandidate`
//!
//! Token derivation (name_tokens, keyword_tokens, etc.) is now handled
//! internally by `tool-routing-engine` during scoring.
//!
//! This crate retains: `McpToolRecord` (the registry/data type) and
//! `McpToolInputSchema`.

use serde::{Deserialize, Serialize};

/// Default gate value for serde(default) when field is absent from JSON.
fn default_gate() -> String {
    "none".to_string()
}

// ── Core types ──────────────────────────────────────────────────────────────

/// A single MCP tool record in the unified registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolRecord {
    /// Unique tool identifier (e.g. "pdf_read", "browser_screenshot").
    pub slug: String,
    /// Human-readable name (e.g. "PDF 文本提取").
    pub display_name: String,
    /// Detailed description of the tool's capability.
    pub description: String,
    /// Tool classification: "builtin" | "research" | "external" | "independent".
    pub layer: String,
    /// Which dispatch domain handles this tool: "domain:goal" | "domain:quality-gate"
    /// | "domain:closeout" | "domain:routing-evolution" | "domain:framework"
    /// | "research" | "browser" | "codegraph" | "stdio-binary".
    pub dispatch_domain: String,
    /// Owning team/component: "framework" | "research" | "browser" | "codegraph" | "rust-tools".
    pub owner: String,
    /// Gate requirement: "none" | "guard" | "sandbox".
    /// Retained for future safety policy integration.
    #[doc(hidden)]
    #[serde(default = "default_gate")]
    pub gate: String,
    /// Natural language trigger phrases for routing.
    pub trigger_hints: Vec<String>,
    /// Supported host platforms. Empty = all platforms supported.
    pub host_platforms: Vec<String>,
    /// Target MCP server process name (e.g. "router-rs", "browser-mcp", "mcp-pdf").
    pub mcp_server: String,
    /// Extension flags for specialized routing behavior.
    /// e.g. "deprecated" (auto-blacklist), "experimental", "host_filtered".
    #[serde(default)]
    pub tool_flags: Vec<String>,
    /// JSON Schema for framework-domain tools (used to generate MCP tools/list response).
    /// Only populated for `dispatch_domain.starts_with("domain:")` tools.
    #[serde(default, rename = "input_schema")]
    pub input_schema_json: Option<McpToolInputSchema>,
}

/// JSON Schema for a framework-domain MCP tool's input parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolInputSchema {
    #[serde(rename = "type")]
    pub schema_type: String,
    #[serde(default)]
    pub properties: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub required: Vec<String>,
}
