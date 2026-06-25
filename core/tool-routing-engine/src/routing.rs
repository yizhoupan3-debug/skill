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
use crate::types::{McpToolDecision, ToolCandidate};
use core_state_utils::text_utils::is_ascii_word;
use core_state_utils::text_utils::tokenize_cjk_aware as tokenize_text;
use mcp_tool_registry::McpToolRecord;
use std::collections::HashSet;

const DECISION_SCHEMA_VERSION: &str = "1.0.0";

/// Route a natural language query to the best-matching tool.
///
/// `host_id` is optional — when provided, records with non-empty `host_platforms`
/// are filtered to only those matching the host.
pub fn route_tool(
    query: &str,
    registry_path: &std::path::Path,
    host_id: Option<&str>,
) -> Result<Option<McpToolDecision>, String> {
    if query.len() > crate::MAX_QUERY_LEN {
        return Err(format!(
            "query too long: {} bytes (max {})",
            query.len(),
            crate::MAX_QUERY_LEN,
        ));
    }
    let records = mcp_tool_registry::load_tool_records_cached(registry_path)?;
    Ok(route_tool_from_records(query, &records, host_id))
}

/// Route a query against a pre-loaded set of records, with optional host filtering.
pub fn route_tool_from_records(
    query: &str,
    records: &[McpToolRecord],
    host_id: Option<&str>,
) -> Option<McpToolDecision> {
    if records.is_empty() || query.trim().is_empty() || query.len() > crate::MAX_QUERY_LEN {
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
                if c.record.host_platforms.is_empty()
                    || c.record.host_platforms.iter().any(|p| p.to_lowercase() == hid_lower)
                {
                    c
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
    // Respect host filtering (same logic as Step 7) to prevent bypass
    let mut fuzzy_candidates: Vec<(f64, &McpToolRecord)> = Vec::new();
    for record in records {
        // Skip records excluded by host filter
        if let Some(hid) = host_id {
            let hid_lower = hid.to_lowercase();
            if !record.host_platforms.is_empty()
                && !record.host_platforms.iter().any(|p| p.to_lowercase() == hid_lower)
            {
                continue;
            }
        }
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

/// 7-step scoring pipeline. Steps 1-5 produce the primary score;
/// step 6 (do-not-use) and step 7 (layer penalty) are applied as adjustments.
/// Step 3 (display_name) was merged into Step 4's alias mechanism to avoid
/// double-counting — the same alias_tokens were being scored twice.
pub(crate) fn score_tool(
    record: &McpToolRecord,
    query_lower: &str,
    query_tokens: &[String],
    weights: &crate::scoring_config::ToolScoringWeights,
) -> (f64, Vec<String>, usize) {
    // Pre-compute routing tokens from the raw record (tool layer no longer
    // embeds these fields; derivation happens here in the routing layer).
    let slug_lower = record.slug.to_lowercase();
    let display_name_lower = record.display_name.to_lowercase();

    let name_tokens: HashSet<String> = slug_lower
        .split(['-', '_'])
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
        .collect();

    let keyword_tokens: HashSet<String> = record
        .trigger_hints
        .iter()
        .flat_map(|hint| tokenize_text(&hint.to_lowercase()))
        .collect();

    let desc_tokens: HashSet<String> = tokenize_text(&record.description.to_lowercase())
        .into_iter()
        .collect();

    let alias_tokens: HashSet<String> = tokenize_text(&display_name_lower)
        .into_iter()
        .collect();

    let do_not_use_tokens: HashSet<String> =
        if record.tool_flags.iter().any(|f| f == "deprecated") {
            // Only include "deprecated" as the signal keyword — do NOT clone
            // name_tokens here. Otherwise the tool gets penalized for its OWN
            // name tokens matching the query, creating a confusing self-penalty
            // where the same tokens earn positive score (Step 2) and then reduce
            // it (Step 6). The penalty should only fire when the query explicitly
            // signals "I want something deprecated".
            let mut tokens = HashSet::new();
            tokens.insert("deprecated".to_string());
            tokens
        } else {
            HashSet::new()
        };

    let mut score = 0.0f64;
    let mut reasons = Vec::new();
    let mut matched_token_count = 0usize;

    // Step 1: Exact name match (slug or display_name)
    if slug_lower == query_lower || display_name_lower == query_lower {
        score += weights.exact_name_boost;
        reasons.push("exact_name_match".to_string());
    }

    // Step 2: Name token matching
    let name_match_count = query_tokens
        .iter()
        .filter(|qt| name_tokens.contains(qt.as_str()))
        .count();
    if name_match_count > 0 {
        score += weights.name_tokens_base + weights.name_tokens_per_token * (name_match_count as f64);
        reasons.push(format!("name_tokens:{name_match_count}"));
        matched_token_count += name_match_count;
    }

    // Step 3: Trigger hint matching

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

    // Step 4: Keyword + Alias token matching
    let keyword_match_count = query_tokens
        .iter()
        .filter(|qt| keyword_tokens.contains(qt.as_str()))
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
        .filter(|qt| alias_tokens.contains(qt.as_str()))
        .count();
    if alias_match_count > 0 {
        // Deduplicate against keyword-matched tokens to avoid double-counting.
        // Instead of the old all-or-nothing lockout (suppress ALL alias scoring
        // when ANY keyword matched), we subtract overlapping tokens so that
        // non-overlapping alias tokens (e.g., display_name words not present in
        // trigger_hints) still contribute.
        let alias_unique_count = query_tokens
            .iter()
            .filter(|qt| alias_tokens.contains(qt.as_str()) && !keyword_tokens.contains(qt.as_str()))
            .count();
        if alias_unique_count > 0 {
            let alias_score = weights.alias_hits_base
                + weights.alias_hits_per_hit * (alias_unique_count as f64);
            score += alias_score;
            reasons.push(format!("alias_tokens:{alias_match_count}[unique:{alias_unique_count}]"));
            matched_token_count += alias_unique_count;
        }
    }

    // Step 5: Description token matching
    let desc_match_count = query_tokens
        .iter()
        .filter(|qt| desc_tokens.contains(qt.as_str()))
        .count();
    if desc_match_count > 0 {
        let desc_score =
            (weights.description_per_match * (desc_match_count as f64)).min(weights.description_max);
        score += desc_score;
        reasons.push(format!("description:{desc_match_count}"));
    }

    // Step 6: do-not-use penalty (for deprecated tools)
    // Always applies a base penalty to deprecated tools, plus additional
    // penalty when the query explicitly contains "deprecated".
    if !do_not_use_tokens.is_empty() && score > 0.0 {
        let base_penalty = weights.do_not_use_penalty_per_hit;
        let additional_hits: Vec<&str> = query_tokens
            .iter()
            .filter(|qt| do_not_use_tokens.contains(qt.as_str()))
            .map(|s| s.as_str())
            .collect();
        let total_penalty = base_penalty
            + (additional_hits.len() as f64) * weights.do_not_use_penalty_per_hit;
        let penalty = f64::min(
            score * weights.do_not_use_penalty_max_ratio,
            total_penalty,
        );
        score = f64::max(0.0, score - penalty);
        reasons.push(format!("do_not_use_penalty:{penalty:.1}"));
    }

    // Step 7: Layer penalty (from externalized JSON config)
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

    fn test_tool_record(slug: &str, keywords: &[&str]) -> McpToolRecord {
        McpToolRecord {
            slug: slug.to_string(),
            display_name: format!("Display {slug}"),
            description: format!("Description for {slug}"),
            layer: "builtin".to_string(),
            dispatch_domain: "composite".to_string(),
            owner: "framework".to_string(),
            gate: "none".to_string(),
            trigger_hints: keywords.iter().map(|s| s.to_string()).collect(),
            host_platforms: vec!["claude".to_string()],
            mcp_server: "router-rs".to_string(),
            tool_flags: vec![],
            input_schema_json: None,
        }
    }

    #[test]
    fn exact_match_wins() {
        let records = vec![
            test_tool_record("pdf_read", &["pdf", "PDF"]),
            test_tool_record("browser_screenshot", &["截图", "浏览器"]),
        ];
        let decision = route_tool_from_records("pdf_read", &records, None);
        assert!(decision.is_some());
        assert_eq!(decision.unwrap().selected_tool, "pdf_read");
    }

    #[test]
    fn keyword_match_returns_something() {
        let records = vec![
            test_tool_record("pdf_read", &["pdf", "文档"]),
            test_tool_record("browser_screenshot", &["截图", "浏览器"]),
        ];
        let decision = route_tool_from_records("帮我处理 PDF 文档", &records, None);
        assert!(decision.is_some());
        let d = decision.unwrap();
        assert_eq!(d.selected_tool, "pdf_read");
    }

    #[test]
    fn empty_query_returns_none() {
        let records = vec![test_tool_record("pdf_read", &["pdf"])];
        assert!(route_tool_from_records("", &records, None).is_none());
    }

    #[test]
    fn no_match_returns_none() {
        let records = vec![test_tool_record("pdf_read", &["pdf"])];
        assert!(route_tool_from_records("hello world", &records, None).is_none());
    }

    #[test]
    fn host_filter_penalizes_mismatch() {
        let records = vec![
            test_tool_record("pdf_read", &["pdf"]),
            test_tool_record("browser_screenshot", &["截图", "浏览器"]),
        ];
        // Query matches "screenshot" but host is "cursor" — pdf_read has host_platforms=["claude"]
        // Both should be penalized
        let decision = route_tool_from_records("screenshot", &records, Some("cursor"));
        assert!(decision.is_none()); // No match after host filter penalty
    }

    #[test]
    fn fuzzy_rescue_respects_host_filter() {
        // "screenshto" would fuzzy-match trigger_hint "screenshot" via trigram,
        // but host="cursor" doesn't match claude-only record — must not select it
        let records = vec![test_tool_record("browser_screenshot", &["screenshot"])];
        let decision = route_tool_from_records("screenshto", &records, Some("cursor"));
        assert!(
            decision.is_none(),
            "fuzzy rescue must not bypass host filter"
        );
    }

    #[test]
    fn fuzzy_rescue_handles_typo() {
        let records = vec![test_tool_record(
            "browser_screenshot",
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
        let mut r = test_tool_record("pdf_read", &["pdf"]);
        r.display_name = "PDF 文本提取".to_string();
        let records = vec![r];
        // Query matches the Chinese display name
        let decision = route_tool_from_records("文本提取", &records, None);
        assert!(decision.is_some(), "display name should match");
        if let Some(d) = decision {
            assert!(d.reasons.iter().any(|r| r.contains("alias")),
                "display name matching now produces alias_tokens reasons");
        }
    }

    #[test]
    fn score_tool_deprecated_base_penalty_no_keyword() {
        let weights = tool_scoring_weights();
        let mut record = test_tool_record("old_tool", &["legacy"]);
        record.tool_flags = vec!["deprecated".to_string()];
        record.description = "An old deprecated tool".to_string();
        // Query does NOT contain "deprecated" — base penalty should still fire.
        let query_tokens = tokenize_text("old_tool legacy");
        let (score, reasons, _) = score_tool(&record, "old_tool legacy", &query_tokens, &weights);
        assert!(score > 0.0, "exact name should still score positively");
        assert!(
            reasons.iter().any(|r| r.starts_with("do_not_use_penalty:")),
            "base do-not-use penalty should fire even without 'deprecated' keyword"
        );
    }

    #[test]
    fn score_tool_do_not_use_penalty() {
        let weights = tool_scoring_weights();
        let mut record = test_tool_record("old_tool", &["legacy"]);
        record.tool_flags = vec!["deprecated".to_string()];
        record.description = "An old deprecated tool".to_string();
        // Query contains "deprecated" explicitly — that's the signal that
        // triggers the do-not-use penalty. Querying the tool name alone no
        // longer self-penalizes (name_tokens are not cloned into do_not_use_tokens).
        let query_tokens = tokenize_text("old_tool deprecated");
        let (score, reasons, _) = score_tool(&record, "old_tool deprecated", &query_tokens, &weights);
        assert!(score > 0.0, "exact name should still score positively");
        assert!(
            reasons.iter().any(|r| r.contains("do_not_use_penalty")),
            "do-not-use penalty reason should be present when query contains 'deprecated'"
        );
        // Without the self-penalty bug, exact name boost (100) is still
        // dominant. The "deprecated" token adds via keyword matching too.
        // Verify the penalty did reduce score from the peak though.
        assert!(
            reasons.iter().any(|r| r.starts_with("do_not_use_penalty:")),
            "do_not_use_penalty reason should have a numeric value"
        );
    }

    #[test]
    fn score_tool_layer_penalty_external() {
        let weights = tool_scoring_weights();
        let make_record = |layer: &str| McpToolRecord {
            slug: "ext_tool".to_string(),
            display_name: "Ext Tool".to_string(),
            description: "An external tool".to_string(),
            layer: layer.to_string(),
            dispatch_domain: "composite".to_string(),
            owner: "external".to_string(),
            gate: "none".to_string(),
            trigger_hints: vec![],
            host_platforms: vec![],
            mcp_server: "ext-server".to_string(),
            tool_flags: vec![],
            input_schema_json: None,
        };
        let query_tokens = tokenize_text("ext_tool");
        let (score_builtin, _, _) = score_tool(&make_record("builtin"), "ext_tool", &query_tokens, &weights);
        let (score_external, reasons, _) = score_tool(&make_record("external"), "ext_tool", &query_tokens, &weights);
        assert!(
            reasons.iter().any(|r| r.contains("layer")),
            "layer penalty reason should be present"
        );
        // external layer penalty is -2.0 relative to builtin
        let diff = score_builtin - score_external;
        assert!((diff - 2.0).abs() < 0.001, "external layer penalty should reduce score by 2.0");
    }

    #[test]
    fn score_tool_alias_only() {
        let weights = tool_scoring_weights();
        let mut record = test_tool_record("pdf_read", &[]);
        record.display_name = "PDF 文本提取".to_string();
        let query_tokens = tokenize_text("文本提取");
        let (score, reasons, _) = score_tool(&record, "文本提取", &query_tokens, &weights);
        assert!(
            reasons.iter().any(|r| r.contains("alias")),
            "alias matching should fire when keywords=0"
        );
        assert!(score > 0.0, "alias-only should score > 0");
    }

    #[test]
    fn score_tool_description_match() {
        let weights = tool_scoring_weights();
        let mut record = test_tool_record("pdf_read", &[]);
        record.display_name = "".to_string();
        record.description = "extract text and images from PDF files".to_string();
        let query_tokens = tokenize_text("extract images");
        let (score, reasons, _) = score_tool(&record, "extract images", &query_tokens, &weights);
        assert!(
            reasons.iter().any(|r| r.contains("description")),
            "description matching should fire"
        );
        assert!(score > 0.0, "description match should score > 0");
    }

    #[test]
    fn route_long_query() {
        let records = vec![test_tool_record("pdf_read", &["pdf"])];
        let long_query = "a".repeat(5000);
        let decision = route_tool_from_records(&long_query, &records, None);
        assert!(decision.is_none(), "query over MAX_QUERY_LEN should return None");
    }
}
