//! Tool routing: scoring pipeline and route_tool entry point.

use crate::scoring_config::tool_scoring_weights;
use crate::tool_types::{is_ascii_word, tokenize_text, McpToolDecision, McpToolRecord, ToolCandidate};

const DECISION_SCHEMA_VERSION: &str = "1.0.0";

/// Maximum query length in bytes to prevent abuse.
const MAX_QUERY_LEN: usize = 4096;

/// Route a natural language query to the best-matching tool.
///
/// Returns `Err` if the registry cannot be loaded (file missing, parse error, etc.).
/// Returns `Ok(None)` if the query is valid but no tool matches.
pub fn route_tool(query: &str, registry_path: &std::path::Path) -> Result<Option<McpToolDecision>, String> {
    if query.len() > MAX_QUERY_LEN {
        return Err(format!("query too long: {} bytes (max {MAX_QUERY_LEN})", query.len()));
    }
    let records = crate::tool_registry::load_tool_records(registry_path)?;
    Ok(route_tool_from_records(query, &records))
}

/// Route a query against a pre-loaded set of records.
pub fn route_tool_from_records(query: &str, records: &[McpToolRecord]) -> Option<McpToolDecision> {
    if records.is_empty() || query.trim().is_empty() || query.len() > MAX_QUERY_LEN {
        return None;
    }

    let weights = tool_scoring_weights();
    let query_lower = query.to_lowercase();
    let query_tokens = tokenize_text(&query_lower);

    let candidates: Vec<ToolCandidate> = records
        .iter()
        .map(|record| {
            let (score, reasons, matched_token_count) =
                score_tool(record, &query_lower, &query_tokens, &weights);
            ToolCandidate {
                record,
                score,
                reasons,
                matched_token_count,
            }
        })
        .collect();

    let best = candidates.into_iter().max_by(|a, b| {
        a.score
            .partial_cmp(&b.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    })?;

    if best.score <= 0.0 {
        return None;
    }

    Some(McpToolDecision {
        decision_schema_version: DECISION_SCHEMA_VERSION.to_string(),
        selected_tool: best.record.slug.clone(),
        score: best.score,
        reasons: best.reasons,
        matched_token_count: best.matched_token_count,
        dispatch_domain: best.record.dispatch_domain.clone(),
        mcp_server: best.record.mcp_server.clone(),
    })
}

/// Score a single tool against the query. Shared by routing and search.
pub(crate) fn score_tool(
    record: &McpToolRecord,
    query_lower: &str,
    query_tokens: &[String],
    weights: &crate::scoring_config::ToolScoringWeights,
) -> (f64, Vec<String>, usize) {
    let mut score = 0.0f64;
    let mut reasons = Vec::new();
    let mut matched_token_count = 0usize;

    // 1. Exact name match
    if record.slug.to_lowercase() == query_lower
        || record.display_name.to_lowercase() == query_lower
    {
        score += weights.exact_name_boost;
        reasons.push("exact_name_match".to_string());
    }

    // 2. Name token matching
    let name_match_count = query_tokens
        .iter()
        .filter(|qt| record.name_tokens.contains(qt.as_str()))
        .count();
    if name_match_count > 0 {
        score += weights.name_tokens_base + weights.name_tokens_per_token * (name_match_count as f64);
        reasons.push(format!("name_tokens:{name_match_count}"));
        matched_token_count += name_match_count;
    }

    // 3. Trigger hint matching
    // English hints: require word-boundary match (exact token in query).
    // CJK hints: substring match in query (CJK has no word boundaries).
    let trigger_match_count = record
        .trigger_hints
        .iter()
        .filter(|hint| {
            let hint_lower = hint.to_lowercase();
            if is_ascii_word(&hint_lower) {
                // English hint: match as exact token in query
                query_tokens.iter().any(|qt| qt == &hint_lower)
            } else {
                // CJK or mixed: substring match in query
                query_lower.contains(&*hint_lower)
            }
        })
        .count();
    if trigger_match_count > 0 {
        let trigger_score = weights.trigger_hint_per_match * (trigger_match_count as f64);
        score += trigger_score;
        reasons.push(format!("trigger_hints:{trigger_match_count}"));
        matched_token_count += trigger_match_count;
    }

    // 4. Keyword token matching (broader)
    let keyword_match_count = query_tokens
        .iter()
        .filter(|qt| record.keyword_tokens.contains(qt.as_str()))
        .count();
    if keyword_match_count > 0 {
        let kw_score = (weights.keyword_per_match * (keyword_match_count as f64)).min(weights.keyword_max);
        score += kw_score;
        reasons.push(format!("keywords:{keyword_match_count}"));
    }

    // 5. Description token matching (exact token match against pre-computed desc_tokens)
    let desc_match_count = query_tokens
        .iter()
        .filter(|qt| record.desc_tokens.contains(qt.as_str()))
        .count();
    if desc_match_count > 0 {
        let desc_score = (weights.description_per_match * (desc_match_count as f64)).min(weights.description_max);
        score += desc_score;
        reasons.push(format!("description:{desc_match_count}"));
    }

    // 6. Layer penalty (from externalized JSON config)
    let layer_penalty = weights.layer_penalties.get(&record.layer).copied().unwrap_or(0.0);
    if layer_penalty != 0.0 {
        score += layer_penalty;
        reasons.push(format!("layer:{layer_penalty}"));
    }

    (score, reasons, matched_token_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn test_record(slug: &str, name_tokens: &[&str], keywords: &[&str]) -> McpToolRecord {
        let mut record = McpToolRecord {
            slug: slug.to_string(),
            display_name: format!("Display {slug}"),
            description: format!("Description for {slug}"),
            layer: "builtin".to_string(),
            dispatch_domain: "composite".to_string(),
            owner: "framework".to_string(),
            gate: "none".to_string(),
            trigger_hints: keywords.iter().map(|s| s.to_string()).collect(),
            name_tokens: name_tokens.iter().map(|s| s.to_string()).collect(),
            keyword_tokens: HashSet::new(),
            desc_tokens: HashSet::new(),
            host_platforms: vec!["claude".to_string()],
            mcp_server: "router-rs".to_string(),
            tool_flags: vec![],
        };
        // Override name_tokens (they were explicitly provided), derive the rest
        record.name_tokens = name_tokens.iter().map(|s| s.to_string()).collect();
        McpToolRecord::derive_tokens(&mut record);
        record
    }

    #[test]
    fn exact_match_wins() {
        let records = vec![
            test_record("pdf_read", &["pdf", "read"], &["pdf", "PDF"]),
            test_record("browser_screenshot", &["browser", "screenshot"], &["截图", "浏览器"]),
        ];
        let decision = route_tool_from_records("pdf_read", &records);
        assert!(decision.is_some());
        assert_eq!(decision.unwrap().selected_tool, "pdf_read");
    }

    #[test]
    fn keyword_match_returns_something() {
        let records = vec![
            test_record("pdf_read", &["pdf", "read"], &["pdf", "文档"]),
            test_record("browser_screenshot", &["browser", "screenshot"], &["截图", "浏览器"]),
        ];
        let decision = route_tool_from_records("帮我处理 PDF 文档", &records);
        assert!(decision.is_some());
        let d = decision.unwrap();
        assert_eq!(d.selected_tool, "pdf_read");
    }

    #[test]
    fn empty_query_returns_none() {
        let records = vec![test_record("pdf_read", &["pdf"], &["pdf"])];
        assert!(route_tool_from_records("", &records).is_none());
    }

    #[test]
    fn no_match_returns_none() {
        let records = vec![test_record("pdf_read", &["pdf"], &["pdf"])];
        assert!(route_tool_from_records("hello world", &records).is_none());
    }
}
