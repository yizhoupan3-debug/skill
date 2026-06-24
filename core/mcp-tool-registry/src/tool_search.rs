//! Tool search API: top-k matching results from the unified registry.

use crate::tool_types::{tokenize_text, McpToolRecord};
use crate::tool_routing::score_tool;
use crate::scoring_config::tool_scoring_weights;

/// A single search result with score and matched tokens.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolSearchResult {
    pub slug: String,
    pub display_name: String,
    pub score: f64,
    pub reasons: Vec<String>,
    pub matched_token_count: usize,
    pub dispatch_domain: String,
    pub mcp_server: String,
}

/// Maximum query length in bytes.
const MAX_QUERY_LEN: usize = 4096;

/// Maximum top_k value to prevent abuse.
const MAX_TOP_K: usize = 100;

/// Search tools by query, returning top-k results sorted by score.
/// Uses the same scoring pipeline as route_tool for consistency.
pub fn search_tools(
    query: &str,
    records: &[McpToolRecord],
    top_k: usize,
) -> Vec<ToolSearchResult> {
    if records.is_empty() || query.trim().is_empty() || top_k == 0 {
        return Vec::new();
    }
    let top_k = top_k.min(MAX_TOP_K);
    if query.len() > MAX_QUERY_LEN {
        return Vec::new();
    }

    let weights = tool_scoring_weights();
    let query_lower = query.to_lowercase();
    let query_tokens = tokenize_text(&query_lower);

    let mut results: Vec<ToolSearchResult> = records
        .iter()
        .filter_map(|record| {
            let (score, reasons, matched_token_count) =
                score_tool(record, &query_lower, &query_tokens, &weights);

            if score <= 0.0 {
                return None;
            }

            Some(ToolSearchResult {
                slug: record.slug.clone(),
                display_name: record.display_name.clone(),
                score,
                reasons,
                matched_token_count,
                dispatch_domain: record.dispatch_domain.clone(),
                mcp_server: record.mcp_server.clone(),
            })
        })
        .collect();

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(top_k);
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool_types::McpToolRecord;
    use std::collections::HashSet;

    fn test_record(slug: &str, keywords: &[&str]) -> McpToolRecord {
        let mut record = McpToolRecord {
            slug: slug.to_string(),
            slug_lower: String::new(),
            display_name_lower: String::new(),
            display_name: format!("Display {slug}"),
            description: format!("Description for {slug}"),
            layer: "builtin".to_string(),
            dispatch_domain: "composite".to_string(),
            owner: "framework".to_string(),
            gate: "none".to_string(),
            trigger_hints: keywords.iter().map(|s| s.to_string()).collect(),
            name_tokens: HashSet::new(),
            keyword_tokens: HashSet::new(),
            desc_tokens: HashSet::new(),
            alias_tokens: HashSet::new(),
            do_not_use_tokens: HashSet::new(),
            host_platforms: vec!["claude".to_string()],
            mcp_server: "router-rs".to_string(),
            tool_flags: vec![],
            input_schema_json: None,
        };
        McpToolRecord::derive_tokens(&mut record);
        record
    }

    #[test]
    fn search_returns_top_k() {
        let records = vec![
            test_record("pdf_read", &["pdf", "文档"]),
            test_record("pdf_write", &["pdf", "写入"]),
            test_record("browser_screenshot", &["截图", "浏览器"]),
            test_record("browser_click", &["点击", "浏览器"]),
        ];
        let results = search_tools("pdf", &records, 2);
        assert_eq!(results.len(), 2);
        assert!(results[0].slug.contains("pdf"));
    }

    #[test]
    fn search_empty_query() {
        let records = vec![test_record("pdf_read", &["pdf"])];
        assert!(search_tools("", &records, 10).is_empty());
    }

    #[test]
    fn search_zero_top_k() {
        let records = vec![test_record("pdf_read", &["pdf"])];
        assert!(search_tools("pdf", &records, 0).is_empty());
    }
}
