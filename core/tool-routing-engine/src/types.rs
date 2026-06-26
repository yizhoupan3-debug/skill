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

// ── Tool routing eval types (parallel to routing-engine/src/types.rs) ──

/// Tool routing eval case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRoutingEvalCasePayload {
    pub id: Option<String>,
    pub category: String,
    pub task: String,
    #[serde(default)]
    pub expected_tool: Option<String>,
    #[serde(default)]
    pub forbidden_tools: Vec<String>,
    #[serde(default)]
    pub host_id: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// A collection of tool routing eval cases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRoutingEvalCasesPayload {
    pub schema_version: String,
    pub cases: Vec<ToolRoutingEvalCasePayload>,
}

/// Metrics aggregated from an eval run.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolRoutingEvalMetricsPayload {
    pub case_count: usize,
    pub trigger_hit: usize,
    pub trigger_miss: usize,
    pub overtrigger: usize,
    pub tool_correct: usize,
}

/// Result for a single eval case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRoutingEvalResultPayload {
    pub id: Option<String>,
    pub category: String,
    pub task: String,
    pub expected_tool: Option<String>,
    pub selected_tool: Option<String>,
    pub trigger_hit: bool,
    pub overtrigger: bool,
    pub tool_correct: bool,
}

/// Full eval report, schema-versioned.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRoutingEvalReportPayload {
    pub schema_version: String,
    pub metrics: ToolRoutingEvalMetricsPayload,
    pub results: Vec<ToolRoutingEvalResultPayload>,
}

/// Internal — eval case with input index, before aggregation.
#[derive(Debug, Clone)]
pub struct EvaluatedToolRoutingCase {
    pub input_index: usize,
    pub result: ToolRoutingEvalResultPayload,
}
