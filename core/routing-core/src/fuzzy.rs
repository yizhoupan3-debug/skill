//! Trigram-based fuzzy matching — shared between skill routing and tool routing.
//!
//! Provides character-level trigram Jaccard similarity for fuzzy fallback
//! when token-based scoring produces no match.

use std::collections::HashSet;

/// Extract character trigrams (3-grams) from text.
///
/// Both ASCII and CJK characters produce trigrams.
/// A string shorter than 3 characters yields a single trigram equal to the
/// entire string so that very short queries still participate in matching.
pub fn extract_trigrams(text: &str) -> HashSet<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return HashSet::new();
    }
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
///
/// Returns a value in [0.0, 1.0]. Returns 1.0 when both sets are empty
/// (they are trivially identical).
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

/// Compute the best fuzzy score for a query against a list of candidate strings.
///
/// Returns the maximum Jaccard similarity across all candidates, or `None`
/// if none meet the minimum threshold.
/// The returned score is in [0.0, 1.0] (raw Jaccard).
pub fn best_fuzzy_jaccard(query: &str, candidates: &[String]) -> Option<f64> {
    if query.is_empty() || candidates.is_empty() {
        return None;
    }
    let query_trigrams = extract_trigrams(query);
    if query_trigrams.is_empty() {
        return None;
    }

    let mut best = 0.0f64;
    for candidate in candidates {
        let candidate_trigrams = extract_trigrams(candidate);
        let sim = jaccard_similarity(&query_trigrams, &candidate_trigrams);
        if sim > best {
            best = sim;
        }
    }

    if best > 0.0 { Some(best) } else { None }
}

/// Compute trigram similarity between two raw strings (convenience wrapper).
pub fn trigram_similarity(a: &str, b: &str) -> f64 {
    let trigrams_a = extract_trigrams(a);
    let trigrams_b = extract_trigrams(b);
    jaccard_similarity(&trigrams_a, &trigrams_b)
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
    fn extract_trigrams_empty() {
        let trigrams = extract_trigrams("");
        assert!(trigrams.is_empty());
    }

    #[test]
    fn extract_trigrams_unicode() {
        let trigrams = extract_trigrams("代码审查");
        assert_eq!(trigrams.len(), 2);
        assert!(trigrams.contains("代码审"));
        assert!(trigrams.contains("码审查"));
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
    fn jaccard_empty_sets() {
        let a: HashSet<String> = HashSet::new();
        let b: HashSet<String> = HashSet::new();
        assert!((jaccard_similarity(&a, &b) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn jaccard_partial_overlap() {
        let a = ["abc", "bcd", "cde"].iter().map(|s| s.to_string()).collect();
        let b = ["bcd", "cde", "def"].iter().map(|s| s.to_string()).collect();
        assert!((jaccard_similarity(&a, &b) - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn best_fuzzy_with_hints() {
        let hints = vec!["code review".to_string(), "review code".to_string()];
        let score = best_fuzzy_jaccard("help me review code", &hints);
        assert!(score.is_some());
        assert!(score.unwrap() > 0.0);
    }

    #[test]
    fn best_fuzzy_no_hints() {
        let hints: Vec<String> = vec![];
        assert!(best_fuzzy_jaccard("anything", &hints).is_none());
    }

    #[test]
    fn trigram_similarity_identical() {
        assert!((trigram_similarity("hello world", "hello world") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn trigram_similarity_different() {
        assert!((trigram_similarity("hello", "zzzzz") - 0.0).abs() < f64::EPSILON);
    }
}
