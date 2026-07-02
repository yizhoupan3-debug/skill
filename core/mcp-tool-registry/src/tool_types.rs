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
use std::fmt;

// ── Constrained enum types for McpToolRecord ───────────────────────────────────

/// Tool classification.
///
/// Serialized as snake_case: `builtin`, `research`, `external`, `independent`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolLayer {
    Builtin,
    Research,
    External,
    Independent,
}

impl ToolLayer {
    /// Return the serialized string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            ToolLayer::Builtin => "builtin",
            ToolLayer::Research => "research",
            ToolLayer::External => "external",
            ToolLayer::Independent => "independent",
        }
    }
}

impl fmt::Display for ToolLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for ToolLayer {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Enables comparisons like `record.layer == "builtin"`.
impl PartialEq<&str> for ToolLayer {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

/// Which dispatch domain handles this tool.
///
/// Recognized values:
/// `domain:goal`, `domain:quality-gate`, `domain:closeout`, `domain:framework`,
/// `domain:orchestrator`, `domain:research`, `domain:browser`,
/// `domain:codegraph`, `domain:stdio-binary`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DispatchDomain {
    #[serde(rename = "domain:goal")]
    DomainGoal,
    #[serde(rename = "domain:quality-gate")]
    DomainQualityGate,
    #[serde(rename = "domain:closeout")]
    DomainCloseout,
    #[serde(rename = "domain:framework")]
    DomainFramework,
    #[serde(rename = "domain:orchestrator")]
    DomainOrchestrator,
    #[serde(rename = "domain:research", alias = "research")]
    Research,
    #[serde(rename = "domain:browser", alias = "browser")]
    Browser,
    #[serde(rename = "domain:codegraph", alias = "codegraph")]
    CodeGraph,
    #[serde(rename = "domain:stdio-binary", alias = "stdio-binary")]
    StdioBinary,
}

impl DispatchDomain {
    /// Return the serialized string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            DispatchDomain::DomainGoal => "domain:goal",
            DispatchDomain::DomainQualityGate => "domain:quality-gate",
            DispatchDomain::DomainCloseout => "domain:closeout",
            DispatchDomain::DomainFramework => "domain:framework",
            DispatchDomain::DomainOrchestrator => "domain:orchestrator",
            DispatchDomain::Research => "research",
            DispatchDomain::Browser => "browser",
            DispatchDomain::CodeGraph => "codegraph",
            DispatchDomain::StdioBinary => "stdio-binary",
        }
    }
}

impl fmt::Display for DispatchDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for DispatchDomain {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl PartialEq<&str> for DispatchDomain {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

/// Owning team or component.
///
/// Serialized as snake_case except `rust-tools` which uses an explicit rename.
/// Recognized values: `framework`, `research`, `browser`, `codegraph`,
/// `rust-tools`, `paperplain`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolOwner {
    Framework,
    Research,
    Browser,
    #[serde(rename = "codegraph")]
    CodeGraph,
    #[serde(rename = "rust-tools")]
    RustTools,
    #[serde(rename = "paperplain")]
    Paperplain,
}

impl ToolOwner {
    /// Return the serialized string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            ToolOwner::Framework => "framework",
            ToolOwner::Research => "research",
            ToolOwner::Browser => "browser",
            ToolOwner::CodeGraph => "codegraph",
            ToolOwner::RustTools => "rust-tools",
            ToolOwner::Paperplain => "paperplain",
        }
    }
}

impl fmt::Display for ToolOwner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for ToolOwner {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl PartialEq<&str> for ToolOwner {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
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
    pub layer: ToolLayer,
    /// Which dispatch domain handles this tool.
    pub dispatch_domain: DispatchDomain,
    /// Owning team/component.
    pub owner: ToolOwner,
    /// Natural language trigger phrases for routing.
    #[serde(default)]
    pub trigger_hints: Vec<String>,
    /// Target MCP server process name (e.g. "router-rs-cli", "browser-mcp", "mcp-pdf").
    pub mcp_server: String,
    /// Extension flags for specialized routing behavior.
    /// e.g. "deprecated" (auto-blacklist), "experimental", "host_filtered".
    #[serde(default)]
    pub tool_flags: Vec<String>,
    /// JSON Schema for framework-domain tools (used to generate MCP tools/list response).
    /// Only populated for `dispatch_domain.starts_with("domain:")` tools.
    #[serde(default, rename = "input_schema")]
    pub input_schema_json: Option<McpToolInputSchema>,

    // ── Precomputed routing tokens (populated at load time; serde never touches these) ──

    /// Lowercased slug, precomputed for routing.
    #[serde(skip)]
    pub slug_lower: String,
    /// Lowercased display_name, precomputed for routing.
    #[serde(skip)]
    pub display_name_lower: String,
    /// Tokens from slug split by `-_`.
    #[serde(skip)]
    pub name_tokens: std::collections::HashSet<String>,
    /// Tokens from trigger_hints.
    #[serde(skip)]
    pub keyword_tokens: std::collections::HashSet<String>,
    /// Tokens from description.
    #[serde(skip)]
    pub desc_tokens: std::collections::HashSet<String>,
    /// Tokens from display_name.
    #[serde(skip)]
    pub alias_tokens: std::collections::HashSet<String>,
}

impl McpToolRecord {
    pub fn slug(&self) -> &str { &self.slug }
    pub fn display_name(&self) -> &str { &self.display_name }
    pub fn description(&self) -> &str { &self.description }
    pub fn layer(&self) -> &ToolLayer { &self.layer }
    pub fn dispatch_domain(&self) -> &DispatchDomain { &self.dispatch_domain }
    pub fn owner(&self) -> &ToolOwner { &self.owner }
    pub fn trigger_hints(&self) -> &[String] { &self.trigger_hints }
    pub fn mcp_server(&self) -> &str { &self.mcp_server }
    pub fn tool_flags(&self) -> &[String] { &self.tool_flags }
    pub fn input_schema_json(&self) -> Option<&McpToolInputSchema> { self.input_schema_json.as_ref() }
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
