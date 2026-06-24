//! Fuzzy rescue: trigram-based Jaccard similarity for tools with zero scored hits.
//!
//! Wraps `routing-core::fuzzy` primitives with tool-specific thresholds.
//! The minimum similarity threshold is loaded from the externalized
//! `tool_scoring_weights.json` configuration at runtime.
//! When the primary scoring pipeline produces no match (all records score ≤ 0.0),
//! this module provides a fallback by comparing the query against each record's
//! trigger hints using character-level trigram overlap.

/// Maximum fuzzy score returned (avoids fuzzy outscoring exact matches).
pub const FUZZY_MAX_SCORE: f64 = 90.0;

/// Re-export core trigram primitives from routing-core.
pub use routing_core::fuzzy::{extract_trigrams, jaccard_similarity};

/// Compute the best fuzzy score for a query against a list of trigger hint strings.
/// Uses the configured threshold from `tool_scoring_weights.json`.
/// Returns `Some(score)` when the best similarity meets or exceeds the configured threshold.
/// `score` is scaled to [0, 100] from the [0, 1] Jaccard range, capped at `FUZZY_MAX_SCORE`.
pub fn best_fuzzy_score(query: &str, hints: &[String]) -> Option<f64> {
    let min_similarity = crate::scoring_config::tool_scoring_weights().fuzzy_min_similarity;
    routing_core::fuzzy::best_fuzzy_jaccard(query, hints).and_then(|raw_score| {
        if raw_score >= min_similarity {
            Some((raw_score * 100.0).min(FUZZY_MAX_SCORE))
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_trigrams_basic() {
        let trigrams = extract_trigrams("hello");
        assert!(trigrams.contains("hel"));
        assert!(trigrams.contains("ell"));
        assert!(trigrams.contains("llo"));
        assert_eq!(trigrams.len(), 3);
    }

    #[test]
    fn extract_trigrams_short() {
        let trigrams = extract_trigrams("ab");
        assert_eq!(trigrams.len(), 1);
        assert!(trigrams.contains("ab"));
    }

    #[test]
    fn jaccard_identical() {
        let a = ["hel", "ell", "llo"].iter().map(|s| s.to_string()).collect();
        let b = ["hel", "ell", "llo"].iter().map(|s| s.to_string()).collect();
        assert!((jaccard_similarity(&a, &b) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn fuzzy_match_handles_typo() {
        let hints = vec!["screenshot".to_string(), "浏览器截图".to_string()];
        let score = best_fuzzy_score("screeenshot", &hints);
        assert!(score.is_some(), "typo should fuzzy-match");
        assert!(score.unwrap() > 50.0);
    }

    #[test]
    fn fuzzy_no_match_for_irrelevant() {
        let hints = vec!["screenshot".to_string()];
        let score = best_fuzzy_score("量子计算论文", &hints);
        assert!(score.is_none());
    }
}
