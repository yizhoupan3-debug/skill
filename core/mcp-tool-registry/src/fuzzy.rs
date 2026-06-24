//! Fuzzy rescue: trigram-based Jaccard similarity for tools with zero scored hits.
//!
//! When the primary scoring pipeline produces no match (all records score ≤ 0.0),
//! this module provides a fallback by comparing the query against each record's
//! trigger hints using character-level trigram overlap.

use std::collections::HashSet;

/// Minimum Jaccard similarity for a fuzzy match to be accepted.
/// Slightly lower than skill routing's 0.4 because tool vocabulary is shorter.
pub const FUZZY_MIN_SIMILARITY: f64 = 0.35;

/// Maximum fuzzy score returned (avoids fuzzy outscoring exact matches).
pub const FUZZY_MAX_SCORE: f64 = 90.0;

/// Extract character trigrams (3-grams) from text.
/// Both ASCII and CJK characters produce trigrams — CJK characters are treated
/// as individual tokens for trigram purposes (each CJK char is a "trigram").
pub fn extract_trigrams(text: &str) -> HashSet<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= 3 {
        let mut set = HashSet::new();
        set.insert(text.to_lowercase());
        return set;
    }
    chars
        .windows(3)
        .map(|w| w.iter().collect::<String>().to_lowercase())
        .collect()
}

/// Jaccard similarity between two trigram sets.
pub fn jaccard_similarity(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let intersection = a.intersection(b).count();
    let union = a.union(b).count();
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

/// Compute the best fuzzy score for a query against a list of trigger hint strings.
/// Returns `Some(score)` when the best similarity meets or exceeds `FUZZY_MIN_SIMILARITY`.
/// `score` is scaled to [0, 100] from the [0, 1] Jaccard range, capped at `FUZZY_MAX_SCORE`.
pub fn best_fuzzy_score(query: &str, hints: &[String]) -> Option<f64> {
    if query.is_empty() || hints.is_empty() {
        return None;
    }
    let query_trigrams = extract_trigrams(query);
    if query_trigrams.is_empty() {
        return None;
    }

    let mut best = 0.0f64;
    for hint in hints {
        let hint_trigrams = extract_trigrams(hint);
        let sim = jaccard_similarity(&query_trigrams, &hint_trigrams);
        if sim > best {
            best = sim;
        }
    }

    if best >= FUZZY_MIN_SIMILARITY {
        Some((best * 100.0).min(FUZZY_MAX_SCORE))
    } else {
        None
    }
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
    fn jaccard_disjoint() {
        let a = ["hel", "ell", "llo"].iter().map(|s| s.to_string()).collect();
        let b = ["wor", "orl", "rld"].iter().map(|s| s.to_string()).collect();
        assert!((jaccard_similarity(&a, &b) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn fuzzy_match_handles_typo() {
        // "screeenshot" vs "screenshot" should fuzzy-match
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
