//! Literature search tool — dispatches to `crate::search::orchestration::search_raw`.
//!
//! # Dispatch
//! Called from `super::handle_research_tool` for `research_literature_search`.
//! Registered in `MCP_TOOL_REGISTRY.json` with `mcp_server: "research-harness"`.

use core_errors::FrameworkError;
use serde_json::Value;

/// Perform a literature search across configured external sources (arXiv, Semantic Scholar).
pub(super) fn tool_literature_search(arguments: &Value) -> Result<String, FrameworkError> {
    let query = arguments
        .get("query")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "research_literature_search requires 'query' (string)",
        ))?;
    let limit = arguments
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(20) as usize;
    let source_str = arguments
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or("all");
    let source = match source_str {
        "semantic-scholar" => crate::search::ExternalSourceArg::SemanticScholar,
        "arxiv" => crate::search::ExternalSourceArg::Arxiv,
        _ => crate::search::ExternalSourceArg::All,
    };
    let year_from = arguments.get("year_from").and_then(Value::as_u64).map(|y| y as u32);
    let year_to = arguments.get("year_to").and_then(Value::as_u64).map(|y| y as u32);
    let sort_by = match arguments.get("sort_by").and_then(Value::as_str) {
        Some("date") => crate::search::SortBy::Date,
        _ => crate::search::SortBy::Relevance,
    };
    let categories = arguments
        .get("categories")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(String::from);
    let advanced_query = arguments
        .get("advanced_query")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(String::from);
    let fuzzy_query = arguments
        .get("fuzzy_query")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let prefer_authoritative = arguments
        .get("prefer_authoritative")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let opts = crate::search::SearchOptions {
        query: query.to_string(),
        limit,
        source,
        year_from,
        year_to,
        sort_by,
        categories,
        advanced_query,
        fuzzy_query,
        prefer_authoritative,
        ..crate::search::SearchOptions::new(query)
    };
    let result = crate::search::orchestration::search_raw(&opts)
        .map_err(|e| FrameworkError::validation(format!("literature search failed: {e}")))?;
    serde_json::to_string_pretty(&result).map_err(FrameworkError::Json)
}
