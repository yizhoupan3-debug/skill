//! Shared token-scoring pipeline for both skill routing and tool routing.
//!
//! Extracts 5 common dimensions that both engines implement identically:
//! exact name → name tokens → trigger hints → keywords → aliases.
//! Each dimension uses dedup tracking to prevent double-counting.
//!
//! Engine-specific logic (gate, owner, overlay, session-start, visual-review,
//! description matching, layer penalty, NL adjustments) stays in each engine.

use std::collections::HashSet;

pub use core_state_utils::text_utils::tokenize_cjk_aware;

// ---------------------------------------------------------------------------
// Weights
// ---------------------------------------------------------------------------

/// Tuneable weights for the 5 shared token-scoring dimensions.
#[derive(Debug, Clone)]
pub struct TokenScoreWeights {
    pub exact_name_boost: f64,
    pub name_tokens_base: f64,
    pub name_tokens_per_token: f64,
    pub trigger_hint_per_match: f64,
    pub keyword_per_keyword: f64,
    pub keyword_max: f64,
    pub alias_hits_base: f64,
    pub alias_hits_per_hit: f64,
}

impl TokenScoreWeights {
    /// Construct from explicit values (used by tool routing).
    pub fn from_tool_weights(
        exact_name_boost: f64,
        name_tokens_base: f64,
        name_tokens_per_token: f64,
        trigger_hint_per_match: f64,
        keyword_per_keyword: f64,
        keyword_max: f64,
        alias_hits_base: f64,
        alias_hits_per_hit: f64,
    ) -> Self {
        Self {
            exact_name_boost,
            name_tokens_base,
            name_tokens_per_token,
            trigger_hint_per_match,
            keyword_per_keyword,
            keyword_max,
            alias_hits_base,
            alias_hits_per_hit,
        }
    }

    /// Construct from JSON configuration value (used by skill routing).
    pub fn from_skill_weights(
        exact_name_boost: f64,
        name_tokens_base: f64,
        name_tokens_per_token: f64,
        trigger_hint_per_match: f64,
        keywords_per_keyword: f64,
        keywords_max: f64,
        alias_hits_base: f64,
        alias_hits_per_hit: f64,
    ) -> Self {
        Self {
            exact_name_boost,
            name_tokens_base,
            name_tokens_per_token,
            trigger_hint_per_match,
            keyword_per_keyword: keywords_per_keyword,
            keyword_max: keywords_max,
            alias_hits_base,
            alias_hits_per_hit,
        }
    }
}

// ---------------------------------------------------------------------------
// Result
// ---------------------------------------------------------------------------

/// Result of the shared token-scoring pipeline.
#[derive(Debug, Clone)]
pub struct TokenScoreResult {
    pub score: f64,
    pub reasons: Vec<String>,
    pub matched_token_count: usize,
    /// Tokens that were matched in the shared pipeline.
    /// Consumers can use this set to add domain-specific scoring steps
    /// (e.g., description matching) without double-counting.
    pub matched_tokens: Vec<String>,
}

// ---------------------------------------------------------------------------
// Shared pipeline
// ---------------------------------------------------------------------------

/// Run the 5-step shared token-scoring pipeline with dedup tracking.
///
/// Steps:
/// 1. Exact name match (slug or display_name == query_lower)
/// 2. Name token matching (slug split by `-_`, dedup'd)
/// 3. Trigger hint matching (single-word exact, multi-word via `contains`)
/// 4. Keyword token matching (dedup'd, capped at `keyword_max`)
/// 5. Alias token matching (dedup'd)
///
/// Each step registers matched tokens in an internal `unique_matched` set
/// so that no token is counted more than once across all steps.
pub fn score_shared_token_matches(
    query_lower: &str,
    query_tokens: &[String],
    slug_lower: &str,
    display_name_lower: &str,
    name_tokens: &HashSet<String>,
    trigger_hints: &[String],
    keyword_tokens: &HashSet<String>,
    alias_tokens: &HashSet<String>,
    weights: &TokenScoreWeights,
) -> TokenScoreResult {
    let mut score = 0.0f64;
    let mut reasons = Vec::new();
    let mut matched_token_count = 0usize;
    let mut unique_matched: HashSet<&str> = HashSet::new();

    // Step 1: Exact name match (slug or display_name)
    if slug_lower == query_lower || display_name_lower == query_lower {
        score += weights.exact_name_boost;
        reasons.push("exact_name_match".to_string());
    }

    // Step 2: Name token matching (dedup against unique_matched)
    let name_match_count = query_tokens
        .iter()
        .filter(|qt| name_tokens.contains(qt.as_str()) && !unique_matched.contains(qt.as_str()))
        .count();
    if name_match_count > 0 {
        score +=
            weights.name_tokens_base + weights.name_tokens_per_token * (name_match_count as f64);
        reasons.push(format!("name_tokens:{name_match_count}"));
        matched_token_count += name_match_count;
        unique_matched.extend(
            query_tokens
                .iter()
                .filter(|qt| name_tokens.contains(qt.as_str()))
                .map(|s| s.as_str()),
        );
    }

    // Step 3: Trigger hint matching
    let trigger_match_count = trigger_hints
        .iter()
        .filter(|hint| {
            let hint_lower = hint.to_lowercase();
            if hint_lower.is_empty() {
                return false;
            }
            if core_state_utils::text_utils::is_ascii_word(&hint_lower) {
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
        // Register matched ASCII single-word trigger hints in unique_matched
        // to prevent re-scoring in keyword/alias steps.
        unique_matched.extend(
            trigger_hints
                .iter()
                .filter(|h| {
                    let lc = h.to_lowercase();
                    !lc.is_empty()
                        && core_state_utils::text_utils::is_ascii_word(&lc)
                })
                .flat_map(|hint| {
                    query_tokens
                        .iter()
                        .filter(move |qt| qt.as_str() == hint)
                })
                .map(|s| s.as_str()),
        );
    }

    // Step 4: Keyword token matching (dedup against unique_matched)
    let keyword_match_count = query_tokens
        .iter()
        .filter(|qt| keyword_tokens.contains(qt.as_str()) && !unique_matched.contains(qt.as_str()))
        .count();
    if keyword_match_count > 0 {
        let kw_score = (weights.keyword_per_keyword * (keyword_match_count as f64))
            .min(weights.keyword_max);
        score += kw_score;
        reasons.push(format!("keywords:{keyword_match_count}"));
        matched_token_count += keyword_match_count;
        unique_matched.extend(
            query_tokens
                .iter()
                .filter(|qt| keyword_tokens.contains(qt.as_str()))
                .map(|s| s.as_str()),
        );
    }

    // Step 5: Alias token matching (dedup against unique_matched)
    let alias_match_count = query_tokens
        .iter()
        .filter(|qt| alias_tokens.contains(qt.as_str()) && !unique_matched.contains(qt.as_str()))
        .count();
    if alias_match_count > 0 {
        let alias_score =
            weights.alias_hits_base + weights.alias_hits_per_hit * (alias_match_count as f64);
        score += alias_score;
        reasons.push(format!("alias_tokens:{alias_match_count}"));
        matched_token_count += alias_match_count;
        unique_matched.extend(
            query_tokens
                .iter()
                .filter(|qt| alias_tokens.contains(qt.as_str()))
                .map(|s| s.as_str()),
        );
    }

    TokenScoreResult {
        score,
        reasons,
        matched_token_count,
        matched_tokens: unique_matched.into_iter().map(|s| s.to_string()).collect(),
    }
}

// ---------------------------------------------------------------------------
// Fuzzy rescue (shared)
// ---------------------------------------------------------------------------

/// Maximum fuzzy score returned (avoids fuzzy outscoring exact matches).
const FUZZY_MAX_SCORE: f64 = 90.0;

/// Compute the best fuzzy score against a list of trigger hints.
/// Uses weighted character n-gram cosine similarity.
/// Returns `Some(score)` scaled to [0, 100] when similarity >= `min_similarity`.
pub fn best_fuzzy_score(query: &str, hints: &[String], min_similarity: f64) -> Option<f64> {
    if query.is_empty() || hints.is_empty() {
        return None;
    }
    let best = hints
        .iter()
        .map(|hint| crate::fuzzy::weighted_ngram_similarity(query, hint))
        .fold(0.0f64, f64::max);
    if best >= min_similarity {
        Some((best * 100.0).min(FUZZY_MAX_SCORE))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn test_weights() -> TokenScoreWeights {
        TokenScoreWeights {
            exact_name_boost: 50.0,
            name_tokens_base: 14.0,
            name_tokens_per_token: 4.0,
            trigger_hint_per_match: 20.0,
            keyword_per_keyword: 3.0,
            keyword_max: 24.0,
            alias_hits_base: 12.0,
            alias_hits_per_hit: 4.0,
        }
    }

    fn token_set(strings: &[&str]) -> HashSet<String> {
        strings.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn exact_name_match_scores_name_boost() {
        let result = score_shared_token_matches(
            "pdf_read",
            &["pdf_read".to_string()],
            "pdf_read",
            "PDF Reader",
            &token_set(&[]),
            &[],
            &token_set(&[]),
            &token_set(&[]),
            &test_weights(),
        );
        assert_eq!(result.score, 50.0);
        assert!(result.reasons.iter().any(|r| r == "exact_name_match"));
    }

    #[test]
    fn name_tokens_score_correctly() {
        let result = score_shared_token_matches(
            "read pdf",
            &["read".to_string(), "pdf".to_string()],
            "pdf_read",
            "PDF Reader",
            &token_set(&["pdf", "read"]),
            &[],
            &token_set(&[]),
            &token_set(&[]),
            &test_weights(),
        );
        // base(14) + 2*per_token(4) = 22
        assert!((result.score - 22.0).abs() < 0.001);
    }

    #[test]
    fn trigger_hint_ascii_word_matches_via_token() {
        let result = score_shared_token_matches(
            "screenshot",
            &["screenshot".to_string()],
            "browser",
            "Browser",
            &token_set(&["browser"]),
            &["screenshot".to_string()],
            &token_set(&[]),
            &token_set(&[]),
            &test_weights(),
        );
        assert!((result.score - 20.0).abs() < 0.001); // trigger hint only
    }

    #[test]
    fn dedup_prevents_double_counting() {
        let result = score_shared_token_matches(
            "pdf",
            &["pdf".to_string()],
            "pdf_read",
            "PDF Reader",
            &token_set(&["pdf", "read"]), // "pdf" in name_tokens
            &["pdf".to_string()], // "pdf" also in trigger_hints
            &token_set(&["pdf"]),  // "pdf" also in keywords
            &token_set(&["pdf"]),  // "pdf" also in aliases
            &test_weights(),
        );
        // name_tokens: base(14) + per_token(4) = 18
        // trigger_hints: "pdf" also matches as trigger hint (not dedup'd) = 20
        // keywords: "pdf" already in unique_matched from trigger_hints → skip
        // aliases: "pdf" already in unique_matched → skip
        // total: 18 + 20 = 38, matched_token_count: 2
        assert!((result.score - 38.0).abs() < 0.001);
        assert_eq!(result.matched_token_count, 2);
    }

    #[test]
    fn trigger_hint_cjk_contains() {
        let query = "帮我处理 PDF 文档";
        let lowered = query.to_lowercase();
        let query_tokens: Vec<String> = tokenize_cjk_aware(&lowered);
        let result = score_shared_token_matches(
            &lowered,
            &query_tokens,
            "pdf_read",
            "PDF Reader",
            &token_set(&["pdf"]),
            &["PDF 文档".to_string()],
            &token_set(&[]),
            &token_set(&[]),
            &test_weights(),
        );
        // name_tokens: "pdf" matches (14 + 4 = 18)
        // trigger_hints: "PDF 文档" lowered to "pdf 文档", contains match = 20
        // total = 38
        assert!((result.score - 38.0).abs() < 0.001, "expected 38, got {}", result.score);
    }

    #[test]
    fn keyword_capped_at_max() {
        let mut weights = test_weights();
        weights.keyword_max = 5.0;
        let tokens: Vec<String> = (0..10).map(|i| format!("token{i}")).collect();
        let kw_set: HashSet<String> = tokens.iter().cloned().collect();
        let result = score_shared_token_matches(
            "token0 token1 token2 token3 token4 token5 token6 token7 token8 token9",
            &tokens,
            "test",
            "Test",
            &token_set(&[]),
            &[],
            &kw_set,
            &token_set(&[]),
            &weights,
        );
        // 10 unique keyword hits * 3.0 = 30, capped at 5.0
        assert!((result.score - 5.0).abs() < 0.001);
    }

    #[test]
    fn alias_adds_base_and_per_hit() {
        let result = score_shared_token_matches(
            "text extraction",
            &["text".to_string(), "extraction".to_string()],
            "pdf_read",
            "",
            &token_set(&[]),
            &[],
            &token_set(&[]),
            &token_set(&["text", "extraction"]),
            &test_weights(),
        );
        // base(12) + 2*per_hit(4) = 20
        assert!((result.score - 20.0).abs() < 0.001);
    }

    #[test]
    fn fuzzy_scoring_handles_typo() {
        let hints = vec!["screenshot".to_string(), "浏览器截图".to_string()];
        let score = best_fuzzy_score("screeenshot", &hints, 0.25);
        assert!(score.is_some());
        assert!(score.unwrap() > 50.0);
    }

    #[test]
    fn fuzzy_no_match_for_irrelevant() {
        let hints = vec!["screenshot".to_string()];
        let score = best_fuzzy_score("量子计算论文", &hints, 0.25);
        assert!(score.is_none());
    }
}
