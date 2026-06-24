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
    /// Supported host platforms.
    pub host_platforms: Vec<String>,
    /// Target MCP server process name (e.g. "router-rs", "browser-mcp", "mcp-pdf").
    pub mcp_server: String,
    /// Extension flags for specialized routing behavior.
    /// **Phase 2 预留**：当前未被评分管道使用，用于未来扩展（如 `"deprecated"`、`"experimental"`、`"host_filtered"` 等标记）。
    #[serde(default)]
    pub tool_flags: Vec<String>,
}

impl McpToolRecord {
    /// Build name_tokens, keyword_tokens, and desc_tokens from slug, trigger_hints, and description.
    /// Uses CJK-aware tokenization so Chinese trigger hints produce
    /// single-character tokens that match CJK query tokens.
    pub fn derive_tokens(record: &mut McpToolRecord) {
        // name_tokens: split slug on - and _ (no CJK in slugs)
        record.name_tokens = record
            .slug
            .to_lowercase()
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
}

/// A candidate tool with its score during routing.
#[derive(Debug, Clone)]
pub struct ToolCandidate<'a> {
    pub record: &'a McpToolRecord,
    pub score: f64,
    pub reasons: Vec<String>,
    pub matched_token_count: usize,
}
