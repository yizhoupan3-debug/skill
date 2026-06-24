//! MCP tool registry: unified tool types for the Tool Layer.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

// ── Re-export shared text utilities from core-state-utils ────────────────────
pub use core_state_utils::text_utils::{is_ascii_word, is_cjk};
pub use core_state_utils::text_utils::tokenize_cjk_aware as tokenize_text;

// ── Core types ──────────────────────────────────────────────────────────────

/// A single MCP tool record in the unified registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolRecord {
    /// Unique tool identifier (e.g. "pdf_read", "browser_screenshot").
    pub slug: String,
    /// Pre-computed lowercase slug for O(1) exact matching.
    #[serde(default, skip)]
    pub slug_lower: String,
    /// Pre-computed lowercase display_name.
    #[serde(default, skip)]
    pub display_name_lower: String,
    /// Human-readable name (e.g. "PDF 文本提取").
    pub display_name: String,
    /// Detailed description of the tool's capability.
    pub description: String,
    /// Tool classification: "builtin" | "research" | "external" | "independent".
    pub layer: String,
    /// Which dispatch domain handles this tool: "composite" | "research" | "browser" | "codegraph" | "stdio-binary".
    pub dispatch_domain: String,
    /// Owning team/component: "framework" | "research" | "browser" | "codegraph" | "rust-tools".
    pub owner: String,
    /// Gate requirement: "none" | "guard" | "sandbox".
    pub gate: String,
    /// Natural language trigger phrases for routing.
    pub trigger_hints: Vec<String>,
    /// Tokenized slug for name matching (auto-derived from slug).
    pub name_tokens: HashSet<String>,
    /// Tokenized trigger hints for keyword matching (auto-derived).
    pub keyword_tokens: HashSet<String>,
    /// Tokenized description for description matching (auto-derived).
    /// Pre-computed to avoid re-tokenizing on every scoring call.
    #[serde(default, skip)]
    pub desc_tokens: HashSet<String>,
    /// Tokenized display_name for alias matching (auto-derived from display_name).
    #[serde(default, skip)]
    pub alias_tokens: HashSet<String>,
    /// Tokens that indicate this tool should NOT be used (auto-derived from tool_flags "deprecated").
    #[serde(default, skip)]
    pub do_not_use_tokens: HashSet<String>,
    /// Supported host platforms.
    pub host_platforms: Vec<String>,
    /// Target MCP server process name (e.g. "router-rs", "browser-mcp", "mcp-pdf").
    pub mcp_server: String,
    /// Extension flags for specialized routing behavior.
    /// e.g. "deprecated" (auto-blacklist), "experimental", "host_filtered".
    #[serde(default)]
    pub tool_flags: Vec<String>,
    /// JSON Schema for composite-domain tools (used to generate MCP tools/list response).
    /// Only populated for `dispatch_domain == "composite"` tools.
    #[serde(default)]
    pub input_schema_json: Option<McpToolInputSchema>,
}

/// JSON Schema for a composite-domain MCP tool's input parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolInputSchema {
    #[serde(rename = "type")]
    pub schema_type: String,
    #[serde(default)]
    pub properties: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub required: Vec<String>,
}

impl McpToolRecord {
    /// Build all derived fields (name_tokens, keyword_tokens, desc_tokens, alias_tokens,
    /// do_not_use_tokens, slug_lower, display_name_lower).
    pub fn derive_tokens(record: &mut McpToolRecord) {
        record.slug_lower = record.slug.to_lowercase();
        record.display_name_lower = record.display_name.to_lowercase();

        // name_tokens: split slug on - and _ (no CJK in slugs)
        record.name_tokens = record
            .slug_lower
            .split(|c: char| c == '-' || c == '_')
            .filter(|t| !t.is_empty())
            .map(|t| t.to_string())
            .collect();

        // keyword_tokens: use CJK-aware tokenize_text for trigger hints
        record.keyword_tokens = record
            .trigger_hints
            .iter()
            .flat_map(|hint| {
                tokenize_text(&hint.to_lowercase())
                    .into_iter()
                    .collect::<Vec<_>>()
            })
            .collect();

        // desc_tokens: pre-tokenize description for O(1) lookup during scoring
        record.desc_tokens = tokenize_text(&record.description.to_lowercase())
            .into_iter()
            .collect();

        // alias_tokens: pre-tokenize display_name for alias matching (CJK-aware)
        record.alias_tokens = tokenize_text(&record.display_name_lower)
            .into_iter()
            .collect();

        // do_not_use_tokens: auto-derive from tool_flags "deprecated"
        record.do_not_use_tokens = HashSet::new();
        if record.tool_flags.iter().any(|f| f == "deprecated") {
            // Add the slug and common skip-phrases as do-not-use tokens
            record.do_not_use_tokens = record.name_tokens.clone();
            record.do_not_use_tokens.insert("deprecated".to_string());
        }
    }
}

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
    pub record: &'a McpToolRecord,
    pub score: f64,
    pub reasons: Vec<String>,
    pub matched_token_count: usize,
}
