//! Fuzzy matching primitives — trigram Jaccard (legacy) and n-gram cosine (new).
//!
//! Provides two families of fuzzy string matching:
//!
//! - **Trigram Jaccard** (`extract_trigrams`, `jaccard_similarity`): used by
//!   legacy consumers and by the `best_fuzzy_jaccard` helper.
//! - **Character n-gram cosine** (`character_ngrams`, `cosine_similarity`,
//!   `weighted_ngram_similarity`): weighted unigram+bigram+trigram cosine
//!   similarity for better CJK cross-language matching. This is the
//!   recommended family for new code.

use std::collections::HashMap;
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

/// Extract character n-gram count vectors (1- to N-grams) from text.
///
/// Returns a `HashMap` mapping each n-gram substring to its frequency.
/// Supports n-gram orders from `min_n` to `max_n` inclusive.
/// Unlike `extract_trigrams` (which uses `HashSet` for Jaccard), this preserves
/// frequency counts for cosine similarity.
pub fn character_ngrams(text: &str, min_n: usize, max_n: usize) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    let chars: Vec<char> = text.chars().collect();
    for n in min_n..=max_n {
        if chars.len() < n {
            continue;
        }
        // For short text below n, emit the whole string as a single n-gram
        if chars.len() == n {
            *map.entry(text.to_lowercase()).or_insert(0) += 1;
        } else {
            for window in chars.windows(n) {
                let gram: String = window.iter().collect();
                *map.entry(gram.to_lowercase()).or_insert(0) += 1;
            }
        }
    }
    map
}

/// Cosine similarity between two n-gram count vectors.
///
/// Returns a value in [0.0, 1.0]. Both vectors are `HashMap<String, usize>`
/// where the key is an n-gram and the value is its frequency.
/// Returns 0.0 if either vector is empty.
pub fn cosine_similarity(a: &HashMap<String, usize>, b: &HashMap<String, usize>) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let mut dot_product = 0usize;
    for (key, count_a) in a {
        if let Some(count_b) = b.get(key) {
            dot_product += count_a * count_b;
        }
    }
    let mag_a: usize = a.values().map(|v| v * v).sum();
    let mag_b: usize = b.values().map(|v| v * v).sum();
    let denominator = (mag_a as f64).sqrt() * (mag_b as f64).sqrt();
    if denominator == 0.0 {
        0.0
    } else {
        dot_product as f64 / denominator
    }
}

/// Weighted character n-gram similarity between two strings.
///
/// Combines unigram (weight 0.5), bigram (weight 0.3), and trigram (weight 0.2)
/// cosine similarities. The weighting favors character-level overlap while still
/// capturing local ordering via higher-order n-grams.
pub fn weighted_ngram_similarity(a: &str, b: &str) -> f64 {
    let uni_a = character_ngrams(a, 1, 1);
    let uni_b = character_ngrams(b, 1, 1);
    if uni_a.is_empty() || uni_b.is_empty() {
        return 0.0;
    }
    let cos_uni = cosine_similarity(&uni_a, &uni_b);
    let bi_a = character_ngrams(a, 2, 2);
    let bi_b = character_ngrams(b, 2, 2);
    let cos_bi = if bi_a.is_empty() || bi_b.is_empty() {
        0.0
    } else {
        cosine_similarity(&bi_a, &bi_b)
    };
    let tri_a = character_ngrams(a, 3, 3);
    let tri_b = character_ngrams(b, 3, 3);
    let cos_tri = if tri_a.is_empty() || tri_b.is_empty() {
        0.0
    } else {
        cosine_similarity(&tri_a, &tri_b)
    };
    0.5 * cos_uni + 0.3 * cos_bi + 0.2 * cos_tri
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
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
        let a = ["hel", "ell", "llo"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let b = ["hel", "ell", "llo"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!((jaccard_similarity(&a, &b) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn jaccard_disjoint() {
        let a = ["hel", "ell", "llo"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let b = ["wor", "orl", "rld"]
            .iter()
            .map(|s| s.to_string())
            .collect();
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
        let a = ["abc", "bcd", "cde"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let b = ["bcd", "cde", "def"]
            .iter()
            .map(|s| s.to_string())
            .collect();
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

    // ── character n-gram cosine similarity tests ──

    #[test]
    fn character_ngrams_basic() {
        let map = character_ngrams("abc", 1, 2);
        // unigrams: a, b, c; bigrams: ab, bc
        assert_eq!(map.get("a"), Some(&1));
        assert_eq!(map.get("b"), Some(&1));
        assert_eq!(map.get("ab"), Some(&1));
        assert_eq!(map.get("bc"), Some(&1));
        assert_eq!(map.len(), 5);
    }

    #[test]
    fn character_ngrams_cjk() {
        let map = character_ngrams("代码", 1, 2);
        assert_eq!(map.get("代"), Some(&1));
        assert_eq!(map.get("码"), Some(&1));
        assert_eq!(map.get("代码"), Some(&1));
    }

    #[test]
    fn character_ngrams_empty() {
        assert!(character_ngrams("", 1, 3).is_empty());
    }

    #[test]
    fn cosine_identical() {
        let a = character_ngrams("hello world", 1, 2);
        let b = character_ngrams("hello world", 1, 2);
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn cosine_disjoint() {
        let a = character_ngrams("aaaaa", 1, 1);
        let b = character_ngrams("bbbbb", 1, 1);
        assert!((cosine_similarity(&a, &b) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn cosine_empty() {
        let a = character_ngrams("", 1, 1);
        let b = character_ngrams("hello", 1, 1);
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn weighted_ngram_cjk_rewording() {
        // CJK reordering (code review → review code) should share most characters
        let sim = weighted_ngram_similarity("代码审查", "审查代码");
        assert!(
            sim > 0.5,
            "expected high similarity for CJK reordering, got {sim}"
        );
    }

    #[test]
    fn weighted_ngram_mixed_language() {
        // Mixed language: shares "提交" character and ASCII letters
        let sim = weighted_ngram_similarity("git 提交", "gitx 提交");
        assert!(
            sim > 0.3,
            "expected moderate similarity for mixed language, got {sim}"
        );
    }

    #[test]
    fn weighted_ngram_no_relation() {
        let sim = weighted_ngram_similarity("量子物理", "pizza delivery");
        assert!(
            sim < 0.3,
            "expected low similarity for unrelated text, got {sim}"
        );
    }
}
