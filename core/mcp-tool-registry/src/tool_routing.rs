//! Tool routing: 8-step scoring pipeline, host filtering, and fuzzy rescue.
//!
//! Checked against skill routing's 16-step pipeline. Steps exclusive to skill
//! routing that are intentionally omitted here (not applicable to tools):
//! - NL route adjustments / framework alias pre/post rules
//! - Gate / overlay / owner competition (tools have single dispatch_domain)
//! - Session-start signals (tools are stateless)
//! - Design/paper/agent-swarm domain signals
//! - Parallel review / code-review-deep / visual-review / codegraph signals

use crate::fuzzy::best_fuzzy_score;
use crate::scoring_config::tool_scoring_weights;
use crate::tool_types::{
    is_ascii_word, tokenize_text, McpToolDecision, McpToolRecord, ToolCandidate,
};

const DECISION_SCHEMA_VERSION: &str = "1.0.0";

/// Maximum query length in bytes to prevent abuse.
const MAX_QUERY_LEN: usize = 4096;

/// Route a natural language query to the best-matching tool.
///
/// `host_id` is optional — when provided, records with non-empty `host_platforms`
/// are filtered to only those matching the host.
pub fn route_tool(
    query: &str,
    registry_path: &std::path::Path,
    host_id: Option<&str>,
) -> Result<Option<McpToolDecision>, String> {
    if query.len() > MAX_QUERY_LEN {
        return Err(format!(
            "query too long: {} bytes (max {MAX_QUERY_LEN})",
            query.len()
        ));
    }
    let records = crate::tool_registry::load_tool_records(registry_path)?;
    Ok(route_tool_from_records(query, &records, host_id))
}

/// Route a query against a pre-loaded set of records, with optional host filtering.
pub fn route_tool_from_records(
    query: &str,
    records: &[McpToolRecord],
    host_id: Option<&str>,
) -> Option<McpToolDecision> {
    if records.is_empty() || query.trim().is_empty() || query.len() > MAX_QUERY_LEN {
        return None;
    }

    let weights = tool_scoring_weights();
    let query_lower = query.to_lowercase();
    let query_tokens = tokenize_text(&query_lower);

    // Step 1-6: score all records
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

    // Step 7: host filtering — apply penalty for host mismatch
    let filtered: Vec<ToolCandidate> = if let Some(hid) = host_id {
        let hid_lower = hid.to_lowercase();
        candidates
            .into_iter()
            .map(|c| {
                if c.record.host_platforms.is_empty() {
                    c // empty host_platforms = all hosts
                } else if c.record.host_platforms.iter().any(|p| p.to_lowercase() == hid_lower) {
                    c // host matches
                } else {
                    // Penalize rather than exclude — allows fallback
                    let mut c = c;
                    c.score = 0.0;
                    c.reasons.push(format!("host_filter: excluded ({hid})"));
                    c
                }
            })
            .collect()
    } else {
        candidates
    };

    // Pick best among scored candidates above zero
    let best = filtered.iter().max_by(|a, b| {
        a.score
            .partial_cmp(&b.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    })?;

    if best.score > 0.0 {
        return Some(McpToolDecision {
            decision_schema_version: DECISION_SCHEMA_VERSION.to_string(),
            selected_tool: best.record.slug.clone(),
            score: best.score,
            reasons: best.reasons.clone(),
            matched_token_count: best.matched_token_count,
            dispatch_domain: best.record.dispatch_domain.clone(),
            mcp_server: best.record.mcp_server.clone(),
            fuzzy_match: false,
        });
    }

    // Step 8: fuzzy rescue — try trigram matching against trigger hints
    let mut fuzzy_candidates: Vec<(f64, &McpToolRecord)> = Vec::new();
    for record in records {
        if let Some(fuzzy_score) = best_fuzzy_score(&query_lower, &record.trigger_hints) {
            fuzzy_candidates.push((fuzzy_score, record));
        }
    }
    fuzzy_candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let (fuzzy_score, fuzzy_record) = fuzzy_candidates.into_iter().next()?;

    Some(McpToolDecision {
        decision_schema_version: DECISION_SCHEMA_VERSION.to_string(),
        selected_tool: fuzzy_record.slug.clone(),
        score: fuzzy_score,
        reasons: vec![format!("fuzzy_rescue: trigram similarity {fuzzy_score:.1}")],
        matched_token_count: 0,
        dispatch_domain: fuzzy_record.dispatch_domain.clone(),
        mcp_server: fuzzy_record.mcp_server.clone(),
        fuzzy_match: true,
    })
}

/// 8-step scoring pipeline. Steps 1-6 produce the primary score;
/// step 7 (do-not-use) and step 8 (layer penalty) are applied as adjustments.
pub(crate) fn score_tool(
    record: &McpToolRecord,
    query_lower: &str,
    query_tokens: &[String],
    weights: &crate::scoring_config::ToolScoringWeights,
) -> (f64, Vec<String>, usize) {
    let mut score = 0.0f64;
    let mut reasons = Vec::new();
    let mut matched_token_count = 0usize;

    // Step 1: Exact name match (slug or display_name)
    if record.slug_lower == query_lower || record.display_name_lower == query_lower {
        score += weights.exact_name_boost;
        reasons.push("exact_name_match".to_string());
    }

    // Step 2: Name token matching
    let name_match_count = query_tokens
        .iter()
        .filter(|qt| record.name_tokens.contains(qt.as_str()))
        .count();
    if name_match_count > 0 {
        score += weights.name_tokens_base + weights.name_tokens_per_token * (name_match_count as f64);
        reasons.push(format!("name_tokens:{name_match_count}"));
        matched_token_count += name_match_count;
    }

    // Step 3: Display name matching (tool-specific: Chinese display names like "PDF 文本提取")
    let display_match_count = query_tokens
        .iter()
        .filter(|qt| record.alias_tokens.contains(qt.as_str()))
        .count();
    if display_match_count > 0 {
        score += weights.display_name_per_match * (display_match_count as f64);
        reasons.push(format!("display_name:{display_match_count}"));
        matched_token_count += display_match_count;
    }

    // Step 4: Trigger hint matching
    let trigger_match_count = record
        .trigger_hints
        .iter()
        .filter(|hint| {
            let hint_lower = hint.to_lowercase();
            if is_ascii_word(&hint_lower) {
                query_tokens.iter().any(|qt| qt == &hint_lower)
            } else {
                query_lower.contains(&hint_lower)
            }
        })
        .count();
    if trigger_match_count > 0 {
        score += weights.trigger_hint_per_match * (trigger_match_count as f64);
        reasons.push(format!("trigger_hints:{trigger_match_count}"));
        matched_token_count += trigger_match_count;
    }

    // Step 5: Keyword + Alias token matching
    let keyword_match_count = query_tokens
        .iter()
        .filter(|qt| record.keyword_tokens.contains(qt.as_str()))
        .count();
    if keyword_match_count > 0 {
        let kw_score =
            (weights.keyword_per_keyword * (keyword_match_count as f64)).min(weights.keyword_max);
        score += kw_score;
        reasons.push(format!("keywords:{keyword_match_count}"));
        matched_token_count += keyword_match_count;
    }

    let alias_match_count = query_tokens
        .iter()
        .filter(|qt| record.alias_tokens.contains(qt.as_str()))
        .count();
    if alias_match_count > 0 && keyword_match_count == 0 {
        // Only add alias score when keywords didn't fire (avoid double-count)
        let alias_score = weights.alias_hits_base + weights.alias_hits_per_hit * (alias_match_count as f64);
        score += alias_score;
        reasons.push(format!("alias_tokens:{alias_match_count}"));
        matched_token_count += alias_match_count;
    }

    // Step 6: Description token matching
    let desc_match_count = query_tokens
        .iter()
        .filter(|qt| record.desc_tokens.contains(qt.as_str()))
        .count();
    if desc_match_count > 0 {
        let desc_score =
            (weights.description_per_match * (desc_match_count as f64)).min(weights.description_max);
        score += desc_score;
        reasons.push(format!("description:{desc_match_count}"));
    }

    // Step 7: do-not-use penalty (for deprecated tools)
    if !record.do_not_use_tokens.is_empty() && score > 0.0 {
        let negative_hits: Vec<&str> = query_tokens
            .iter()
            .filter(|qt| record.do_not_use_tokens.contains(qt.as_str()))
            .map(|s| s.as_str())
            .collect();
        if !negative_hits.is_empty() {
            let penalty = f64::min(
                score * weights.do_not_use_penalty_max_ratio,
                (negative_hits.len() as f64) * weights.do_not_use_penalty_per_hit,
            );
            score = f64::max(0.0, score - penalty);
            reasons.push(format!("do_not_use_penalty:{penalty:.1}"));
        }
    }

    // Step 8: Layer penalty (from externalized JSON config)
    let layer_penalty = weights
        .layer_penalties
        .get(&record.layer)
        .copied()
        .unwrap_or(0.0);
    if layer_penalty != 0.0 {
        score += layer_penalty;
        if score <= 0.0 {
            score = 0.0;
        }
        reasons.push(format!("layer:{layer_penalty}"));
    }

    (score, reasons, matched_token_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool_types::McpToolRecord;
    use std::collections::HashSet;

    fn test_record(slug: &str, name_tokens: &[&str], keywords: &[&str]) -> McpToolRecord {
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
            name_tokens: name_tokens.iter().map(|s| s.to_string()).collect(),
            keyword_tokens: HashSet::new(),
            desc_tokens: HashSet::new(),
            alias_tokens: HashSet::new(),
            do_not_use_tokens: HashSet::new(),
            host_platforms: vec!["claude".to_string()],
            mcp_server: "router-rs".to_string(),
            tool_flags: vec![],
            input_schema_json: None,
        };
        record.name_tokens = name_tokens.iter().map(|s| s.to_string()).collect();
        McpToolRecord::derive_tokens(&mut record);
        record
    }

    #[test]
    fn exact_match_wins() {
        let records = vec![
            test_record("pdf_read", &["pdf", "read"], &["pdf", "PDF"]),
            test_record(
                "browser_screenshot",
                &["browser", "screenshot"],
                &["截图", "浏览器"],
            ),
        ];
        let decision = route_tool_from_records("pdf_read", &records, None);
        assert!(decision.is_some());
        assert_eq!(decision.unwrap().selected_tool, "pdf_read");
    }

    #[test]
    fn keyword_match_returns_something() {
        let records = vec![
            test_record("pdf_read", &["pdf", "read"], &["pdf", "文档"]),
            test_record(
                "browser_screenshot",
                &["browser", "screenshot"],
                &["截图", "浏览器"],
            ),
        ];
        let decision = route_tool_from_records("帮我处理 PDF 文档", &records, None);
        assert!(decision.is_some());
        let d = decision.unwrap();
        assert_eq!(d.selected_tool, "pdf_read");
    }

    #[test]
    fn empty_query_returns_none() {
        let records = vec![test_record("pdf_read", &["pdf"], &["pdf"])];
        assert!(route_tool_from_records("", &records, None).is_none());
    }

    #[test]
    fn no_match_returns_none() {
        let records = vec![test_record("pdf_read", &["pdf"], &["pdf"])];
        assert!(route_tool_from_records("hello world", &records, None).is_none());
    }

    #[test]
    fn host_filter_penalizes_mismatch() {
        let records = vec![
            test_record("pdf_read", &["pdf"], &["pdf"]),
            test_record(
                "browser_screenshot",
                &["browser", "screenshot"],
                &["截图", "浏览器"],
            ),
        ];
        // Query matches "screenshot" but host is "cursor" — pdf_read has host_platforms=["claude"]
        // Both should be penalized
        let decision = route_tool_from_records("screenshot", &records, Some("cursor"));
        assert!(decision.is_none()); // No match after host filter penalty
    }

    #[test]
    fn fuzzy_rescue_handles_typo() {
        let records = vec![test_record(
            "browser_screenshot",
            &["browser", "screenshot"],
            &["截图", "screenshot"],
        )];
        // "screeenshot" is a typo of the trigger hint "screenshot"
        let decision = route_tool_from_records("screeenshot", &records, None);
        assert!(decision.is_some(), "typo should fuzzy-match");
        let d = decision.unwrap();
        assert!(d.fuzzy_match, "should be flagged as fuzzy match");
        assert_eq!(d.selected_tool, "browser_screenshot");
    }

    #[test]
    fn display_name_matching_works() {
        let mut r = test_record("pdf_read", &["pdf", "read"], &["pdf"]);
        r.display_name = "PDF 文本提取".to_string();
        r.display_name_lower = r.display_name.to_lowercase();
        r.alias_tokens = tokenize_text(&r.display_name_lower).into_iter().collect();
        let records = vec![r];
        // Query matches the Chinese display name
        let decision = route_tool_from_records("文本提取", &records, None);
        assert!(decision.is_some(), "display name should match");
        if let Some(d) = decision {
            assert!(d.reasons.iter().any(|r| r.contains("display_name")));
        }
    }
}
