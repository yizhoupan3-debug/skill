//! Fuzzy rescue: delegates to shared `routing_core::scoring::best_fuzzy_score`.
//!
//! Only retains tool-specific threshold loading; the n-gram computation
//! is shared via `routing-core`.

/// Maximum fuzzy score returned (avoids fuzzy outscoring exact matches).
pub const FUZZY_MAX_SCORE: f64 = 90.0;

/// Compute the best fuzzy score for a query against a list of trigger hint strings.
/// Uses weighted character n-gram cosine similarity, loaded threshold from config.
pub fn best_fuzzy_score(query: &str, hints: &[String]) -> Option<f64> {
    let min_similarity = crate::scoring_config::tool_scoring_weights().fuzzy_min_similarity;
    routing_core::scoring::best_fuzzy_score(query, hints, min_similarity)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn extract_trigrams_basic() {
        let trigrams = routing_core::fuzzy::extract_trigrams("hello");
        assert!(trigrams.contains("hel"));
        assert!(trigrams.contains("ell"));
        assert!(trigrams.contains("llo"));
        assert_eq!(trigrams.len(), 3);
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
