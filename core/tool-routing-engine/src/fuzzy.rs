//! Fuzzy rescue: character n-gram cosine similarity for tools with zero scored hits.
//!
//! Wraps `routing-core::fuzzy` n-gram primitives with tool-specific thresholds.
//! The minimum similarity threshold is loaded from the externalized
//! `tool_scoring_weights.json` configuration at runtime.
//! When the primary scoring pipeline produces no match (all records score ≤ 0.0),
//! this module provides a fallback by comparing the query against each record's
//! trigger hints using weighted character n-gram cosine similarity
//! (unigram 0.5 + bigram 0.3 + trigram 0.2).

/// Maximum fuzzy score returned (avoids fuzzy outscoring exact matches).
pub const FUZZY_MAX_SCORE: f64 = 90.0;

/// Compute the best fuzzy score for a query against a list of trigger hint strings.
/// Uses weighted character n-gram cosine similarity for better CJK cross-language
/// matching than the old trigram Jaccard.
/// Returns `Some(score)` when the best similarity meets or exceeds the configured threshold.
/// `score` is scaled to [0, 100] from the [0, 1] n-gram range, capped at `FUZZY_MAX_SCORE`.
pub fn best_fuzzy_score(query: &str, hints: &[String]) -> Option<f64> {
    if query.is_empty() || hints.is_empty() {
        return None;
    }
    let min_similarity = crate::scoring_config::tool_scoring_weights().fuzzy_min_similarity;
    let best = hints
        .iter()
        .map(|hint| routing_core::fuzzy::weighted_ngram_similarity(query, hint))
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

    #[test]
    fn extract_trigrams_basic() {
        // Verify routing_core re-exports still work for downstream consumers
        let trigrams = routing_core::fuzzy::extract_trigrams("hello");
        assert!(trigrams.contains("hel"));
        assert!(trigrams.contains("ell"));
        assert!(trigrams.contains("llo"));
        assert_eq!(trigrams.len(), 3);
    }

    #[test]
    fn extract_trigrams_short() {
        let trigrams = routing_core::fuzzy::extract_trigrams("ab");
        assert_eq!(trigrams.len(), 1);
        assert!(trigrams.contains("ab"));
    }

    #[test]
    fn jaccard_identical() {
        let a = ["hel", "ell", "llo"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let b = ["hel", "ell", "llo"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!((routing_core::fuzzy::jaccard_similarity(&a, &b) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn fuzzy_match_handles_typo() {
        let hints = vec!["screenshot".to_string(), "浏览器截图".to_string()];
        let score = best_fuzzy_score("screeenshot", &hints);
        assert!(score.is_some(), "typo should fuzzy-match via n-gram");
        assert!(score.unwrap() > 50.0);
    }

    #[test]
    fn fuzzy_no_match_for_irrelevant() {
        let hints = vec!["screenshot".to_string()];
        let score = best_fuzzy_score("量子计算论文", &hints);
        assert!(score.is_none());
    }
}
