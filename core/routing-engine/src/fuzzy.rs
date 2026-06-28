//! N-gram fuzzy matching fallback for skill routing.
//!
//! Wraps `routing-core` character n-gram primitives with skill-record-specific logic.
//! Uses weighted character n-gram cosine similarity (unigram+bigram+trigram)
//! instead of trigram Jaccard for better CJK cross-language matching.
//!
//! The core scoring pipeline has its own n-gram step (via `NgramCache` in
//! `route::ngram`); this module is the **fuzzy rescue** path, invoked when
//! no skill scored above zero.  The `fuzzy_fallback_score` function reuses
//! `NgramCache` so that the weight formula (0.5 unigram + 0.3 bigram + 0.2
//! trigram) is defined in a single location.

use super::text::normalize_text;
use super::types::SkillRecord;
use crate::route::ngram::NgramCache;

/// Minimum n-gram similarity required for a fuzzy match to be accepted.
/// Lower than the old trigram Jaccard threshold (0.4) because n-gram
/// vectors are higher-dimensional and cosine similarity produces smaller
/// raw values for partial matches.
pub const FUZZY_MIN_SIMILARITY: f64 = 0.25;

/// Compute the best fuzzy fallback score for a query against a single
/// skill record by comparing against all its `trigger_hints`.
///
/// Reuses `NgramCache` (same weighted formula as the pipeline n-gram step)
/// to avoid code duplication.  Returns the maximum similarity across all
/// hints, or 0.0 if the record has no hints.
pub fn fuzzy_fallback_score(query: &str, record: &SkillRecord) -> f64 {
    if record.trigger_hints.is_empty() {
        return 0.0;
    }
    let cache = NgramCache::new(query);
    record
        .trigger_hints
        .iter()
        .map(|hint| cache.similarity_to(&normalize_text(hint)))
        .fold(0.0f64, f64::max)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_fuzzy_fallback_score_with_hints() {
        let record = make_test_record(vec!["code review".to_string(), "review code".to_string()]);
        let score = fuzzy_fallback_score("help me review code", &record);
        assert!(
            score > 0.0,
            "expected positive n-gram score for 'help me review code' vs 'code review', got {score}"
        );
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
        assert!(
            score > 0.2,
            "expected >0.2 weighted n-gram similarity for exact match 'code review', got {score}"
        );
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
