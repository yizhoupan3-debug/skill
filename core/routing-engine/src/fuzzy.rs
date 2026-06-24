//! Trigram-based fuzzy matching fallback for skill routing.
//!
//! Wraps `routing-core` trigram primitives with skill-record-specific logic.
//! The core trigram extraction and Jaccard similarity live in `routing-core`;
//! this module adds CJK-aware normalization and the `SkillRecord`-specific
//! `fuzzy_fallback_score` function.

use super::text::normalize_text;
use super::types::SkillRecord;

/// Minimum trigram similarity required for a fuzzy match to be accepted.
pub const FUZZY_MIN_SIMILARITY: f64 = 0.4;

/// Re-export core trigram primitives from routing-core.
pub use routing_core::fuzzy::{extract_trigrams, jaccard_similarity, trigram_similarity};

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
    use std::collections::HashSet;

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
            skill_flags: Vec::new(),
        }
    }
}
