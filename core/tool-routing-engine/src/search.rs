//! Tool search API: top-k matching results from the unified registry.
//!
//! Returns `McpToolDecision` results (same type as `route_tool`) for consistency.
//! Includes a fuzzy rescue fallback (trigram Jaccard on trigger hints) when
//! the primary scoring pipeline produces no matches — matching `route_tool` behavior.

use crate::fuzzy::best_fuzzy_score;
use crate::routing::score_tool;
use crate::scoring_config::tool_scoring_weights;
use crate::types::McpToolDecision;
use core_state_utils::text_utils::tokenize_cjk_aware as tokenize_text;
use mcp_tool_registry::McpToolRecord;

const DECISION_SCHEMA_VERSION: &str = "1.0.0";

/// Maximum top_k value to prevent abuse.
const MAX_TOP_K: usize = 100;

/// Search tools by query, returning top-k results sorted by score.
/// Uses the same scoring pipeline as route_tool for consistency.
/// Falls back to fuzzy trigram matching on trigger hints when scoring
/// produces no results.
pub fn search_tools(
    query: &str,
    records: &[McpToolRecord],
    top_k: usize,
    host_id: Option<&str>,
) -> Vec<McpToolDecision> {
    if records.is_empty() || query.trim().is_empty() || top_k == 0 {
        return Vec::new();
    }
    let top_k = top_k.min(MAX_TOP_K);
    if query.len() > crate::MAX_QUERY_LEN {
        return Vec::new();
    }

    let weights = tool_scoring_weights();
    let query_lower = query.to_lowercase();
    let query_tokens = tokenize_text(&query_lower);

    // Pre-compute host filter for efficiency
    let hid_lower = host_id.map(|h| h.to_lowercase());

    // Primary: token-based scoring (skip no_routing, deprecated, host-mismatched)
    let mut results: Vec<McpToolDecision> = records
        .iter()
        .filter_map(|record| {
            // Exclude deprecated and no_routing tools from search results
            if record.tool_flags.iter().any(|f| f == "deprecated" || f == "no_routing") {
                return None;
            }
            // Apply host filtering: exclude host-mismatched records
            if let Some(ref hid) = hid_lower
                && !record.host_platforms.is_empty()
                    && !record.host_platforms.iter().any(|p| p.to_lowercase() == *hid)
                {
                    return None;
                }

            let (score, reasons, matched_token_count) =
                score_tool(record, &query_lower, &query_tokens, &weights);

            if score <= 0.0 {
                return None;
            }

            Some(McpToolDecision {
                decision_schema_version: DECISION_SCHEMA_VERSION.to_string(),
                selected_tool: record.slug.clone(),
                score,
                reasons,
                matched_token_count,
                dispatch_domain: record.dispatch_domain.clone(),
                mcp_server: record.mcp_server.clone(),
                fuzzy_match: false,
            })
        })
        .collect();

    // Fallback: fuzzy rescue (trigram Jaccard on trigger hints), respecting host filter
    if results.is_empty() {
        for record in records {
            // Apply host filtering in fuzzy rescue too
            if let Some(ref hid) = hid_lower
                && !record.host_platforms.is_empty()
                    && !record.host_platforms.iter().any(|p| p.to_lowercase() == *hid)
                {
                    continue;
                }
            // Skip deprecated and no_routing tools in fuzzy rescue
            if record.tool_flags.iter().any(|f| f == "deprecated" || f == "no_routing") {
                continue;
            }
            if let Some(fuzzy_score) = best_fuzzy_score(&query_lower, &record.trigger_hints) {
                results.push(McpToolDecision {
                    decision_schema_version: DECISION_SCHEMA_VERSION.to_string(),
                    selected_tool: record.slug.clone(),
                    score: fuzzy_score,
                    reasons: vec![format!("fuzzy_rescue: trigram similarity {fuzzy_score:.1}")],
                    matched_token_count: 0,
                    dispatch_domain: record.dispatch_domain.clone(),
                    mcp_server: record.mcp_server.clone(),
                    fuzzy_match: true,
                });
            }
        }
    }

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

    fn test_tool_record(slug: &str, keywords: &[&str]) -> McpToolRecord {
        McpToolRecord {
            slug: slug.to_string(),
            display_name: format!("Display {slug}"),
            description: format!("Description for {slug}"),
            layer: "builtin".to_string(),
            dispatch_domain: "composite".to_string(),
            owner: "framework".to_string(),
            trigger_hints: keywords.iter().map(|s| s.to_string()).collect(),
            host_platforms: vec!["claude".to_string()],
            mcp_server: "router-rs".to_string(),
            tool_flags: vec![],
            input_schema_json: None,
        }
    }

    #[test]
    fn search_returns_top_k() {
        let records = vec![
            test_tool_record("pdf_read", &["pdf", "文档"]),
            test_tool_record("pdf_write", &["pdf", "写入"]),
            test_tool_record("browser_screenshot", &["截图", "浏览器"]),
            test_tool_record("browser_click", &["点击", "浏览器"]),
        ];
        let results = search_tools("pdf", &records, 2, None);
        assert_eq!(results.len(), 2);
        assert!(results[0].selected_tool.contains("pdf"));
    }

    #[test]
    fn search_empty_query() {
        let records = vec![test_tool_record("pdf_read", &["pdf"])];
        assert!(search_tools("", &records, 10, None).is_empty());
    }

    #[test]
    fn search_zero_top_k() {
        let records = vec![test_tool_record("pdf_read", &["pdf"])];
        assert!(search_tools("pdf", &records, 0, None).is_empty());
    }

    #[test]
    fn search_fuzzy_rescue_handles_typo() {
        let records = vec![test_tool_record(
            "browser_screenshot",
            &["截图", "screenshot"],
        )];
        // "screeenshot" is a typo that scores 0 in token-based scoring but
        // fuzzy-matches "screenshot" via trigram Jaccard
        let results = search_tools("screeenshot", &records, 5, None);
        assert!(!results.is_empty(), "typo should fuzzy-match in search");
        assert!(results[0].fuzzy_match, "should be flagged as fuzzy match");
        assert_eq!(results[0].selected_tool, "browser_screenshot");
    }

    #[test]
    fn search_host_filter_excludes_mismatch() {
        let records = vec![
            test_tool_record("pdf_read", &["pdf", "文档"]),
            test_tool_record("browser_screenshot", &["截图", "screenshot"]),
        ];
        // Query matches "screenshot" but host is "cursor" — pdf_read has host_platforms=["claude"]
        // Both records should be excluded by host filter
        let results = search_tools("screenshot", &records, 5, Some("cursor"));
        assert!(results.is_empty(), "host filter should exclude all records");
    }

    #[test]
    fn search_host_filter_fuzzy_rescue() {
        let records = vec![test_tool_record(
            "browser_screenshot",
            &["screenshot"],
        )];
        // Fuzzy match but host mismatch — must not appear in results
        let results = search_tools("screeenshot", &records, 5, Some("cursor"));
        assert!(results.is_empty(), "fuzzy rescue must not bypass host filter");
    }

    #[test]
    fn search_excludes_deprecated() {
        let mut record = test_tool_record("old_tool", &["legacy", "old"]);
        record.tool_flags = vec!["deprecated".to_string()];
        let records = vec![record];
        let results = search_tools("legacy old_tool", &records, 5, None);
        assert!(results.is_empty(), "deprecated tool must be excluded from search");
    }

    #[test]
    fn search_excludes_no_routing() {
        let mut record = test_tool_record("task_create", &["task", "创建"]);
        record.tool_flags = vec!["no_routing".to_string()];
        let records = vec![record];
        let results = search_tools("task_create", &records, 5, None);
        assert!(results.is_empty(), "no_routing tool must be excluded from search");
    }
}
