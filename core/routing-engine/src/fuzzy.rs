//! Trigram-based fuzzy matching fallback for skill routing.
//!
//! When exact token matching yields no hit or a low-confidence hit,
//! this module computes trigram Jaccard similarity between the query
//! and each skill record's `trigger_hints` to find the closest match.

use super::text::normalize_text;
use super::types::SkillRecord;
use std::collections::HashSet;

/// Minimum trigram similarity required for a fuzzy match to be accepted.
pub const FUZZY_MIN_SIMILARITY: f64 = 0.4;

/// Extract the set of character-level trigrams from a normalized string.
///
/// Uses Unicode-safe iteration (`.chars()`). A string shorter than 3
/// characters yields a single trigram equal to the entire string so
/// that very short queries still participate in matching.
fn extract_trigrams(text: &str) -> HashSet<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() < 3 {
        let mut set = HashSet::new();
        if !text.is_empty() {
            set.insert(text.to_string());
        }
        return set;
    }
    chars
        .windows(3)
        .map(|window| window.iter().collect())
        .collect()
}

/// Compute Jaccard similarity between two trigram sets.
///
/// Returns 0.0 when both sets are empty (avoiding 0/0).
fn jaccard_similarity(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let intersection = a.intersection(b).count();
    let union = a.union(b).count();
    if union == 0 {
        return 0.0;
    }
    intersection as f64 / union as f64
}

/// Compute trigram similarity between two raw strings.
///
/// Both inputs are normalized (lowercased, whitespace-collapsed) before
/// trigram extraction. Returns a Jaccard similarity in [0.0, 1.0].
pub fn trigram_similarity(a: &str, b: &str) -> f64 {
    let norm_a = normalize_text(a);
    let norm_b = normalize_text(b);
    let trigrams_a = extract_trigrams(&norm_a);
    let trigrams_b = extract_trigrams(&norm_b);
    jaccard_similarity(&trigrams_a, &trigrams_b)
}

/// Compute the best fuzzy fallback score for a query against a single
/// skill record by comparing against all its `trigger_hints`.
///
/// Returns the maximum trigram similarity across all hints, or 0.0
/// if the record has no hints.
pub fn fuzzy_fallback_score(query: &str, record: &SkillRecord) -> f64 {
    if record.trigger_hints.is_empty() {
        return 0.0;
    }
    let normalized_query = normalize_text(query);
    let query_trigrams = extract_trigrams(&normalized_query);
    if query_trigrams.is_empty() {
        return 0.0;
    }
    record
        .trigger_hints
        .iter()
        .map(|hint| {
            let norm_hint = normalize_text(hint);
            let hint_trigrams = extract_trigrams(&norm_hint);
            jaccard_similarity(&query_trigrams, &hint_trigrams)
        })
        .fold(0.0f64, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_trigrams_basic() {
        let trigrams = extract_trigrams("hello");
        assert!(trigrams.contains("hel"));
        assert!(trigrams.contains("ell"));
        assert!(trigrams.contains("llo"));
        assert_eq!(trigrams.len(), 3);
    }

    #[test]
    fn test_extract_trigrams_short_string() {
        let trigrams = extract_trigrams("ab");
        assert_eq!(trigrams.len(), 1);
        assert!(trigrams.contains("ab"));
    }

    #[test]
    fn test_extract_trigrams_empty() {
        let trigrams = extract_trigrams("");
        assert!(trigrams.is_empty());
    }

    #[test]
    fn test_extract_trigrams_unicode() {
        let trigrams = extract_trigrams("代码审查");
        assert_eq!(trigrams.len(), 2); // "代码审", "码审查"
    }

    #[test]
    fn test_jaccard_identical() {
        let a: HashSet<String> = ["abc", "bcd"].iter().map(|s| s.to_string()).collect();
        assert_eq!(jaccard_similarity(&a, &a.clone()), 1.0);
    }

    #[test]
    fn test_jaccard_disjoint() {
        let a: HashSet<String> = ["abc"].iter().map(|s| s.to_string()).collect();
        let b: HashSet<String> = ["xyz"].iter().map(|s| s.to_string()).collect();
        assert_eq!(jaccard_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_jaccard_empty_sets() {
        let a: HashSet<String> = HashSet::new();
        assert_eq!(jaccard_similarity(&a, &a.clone()), 0.0);
    }

    #[test]
    fn test_jaccard_partial_overlap() {
        let a: HashSet<String> = ["abc", "bcd", "cde"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let b: HashSet<String> = ["bcd", "cde", "def"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        // intersection = {bcd, cde} = 2, union = {abc, bcd, cde, def} = 4
        assert_eq!(jaccard_similarity(&a, &b), 0.5);
    }

    #[test]
    fn test_trigram_similarity_identical() {
        assert_eq!(trigram_similarity("hello world", "hello world"), 1.0);
    }

    #[test]
    fn test_trigram_similarity_similar() {
        let sim = trigram_similarity("code review", "code reviewing");
        assert!(sim > 0.5, "expected >0.5, got {sim}");
    }

    #[test]
    fn test_trigram_similarity_different() {
        let sim = trigram_similarity("hello", "zzzzz");
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_fuzzy_fallback_score_with_hints() {
        let record = make_test_record(vec!["code review".to_string(), "review code".to_string()]);
        let score = fuzzy_fallback_score("help me review code", &record);
        assert!(score > 0.0, "expected positive score, got {score}");
    }

    #[test]
    fn test_fuzzy_fallback_score_no_hints() {
        let record = make_test_record(vec![]);
        assert_eq!(fuzzy_fallback_score("anything", &record), 0.0);
    }

    #[test]
    fn test_fuzzy_fallback_score_best_hint() {
        let record = make_test_record(vec![
            "completely unrelated".to_string(),
            "code review".to_string(),
        ]);
        let score = fuzzy_fallback_score("code review", &record);
        assert!(score > 0.3, "expected >0.3, got {score}");
    }

    fn make_test_record(trigger_hints: Vec<String>) -> SkillRecord {
        SkillRecord {
            slug: "test-skill".to_string(),
            skill_path: None,
            layer: "L3".to_string(),
            owner: "test".to_string(),
            gate: "none".to_string(),
            priority: "P2".to_string(),
            session_start: "n/a".to_string(),
            summary: "test skill".to_string(),
            slug_lower: "test-skill".to_string(),
            owner_lower: "test".to_string(),
            gate_lower: "none".to_string(),
            session_start_lower: "n/a".to_string(),
            gate_phrases: Vec::new(),
            trigger_hints,
            name_tokens: HashSet::new(),
            keyword_tokens: HashSet::new(),
            alias_tokens: HashSet::new(),
            do_not_use_tokens: HashSet::new(),
            framework_alias_entrypoints: Vec::new(),
            metadata_positive_triggers: Vec::new(),
            host_platforms: Vec::new(),
            record_kind: "skill".to_string(),
            primary_allowed: true,
            fallback_policy_mode: "default".to_string(),
        }
    }
}
