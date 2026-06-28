//! Character n-gram semantic similarity scoring for skill routing.
//!
//! Provides [`NgramCache`] for pre-computing n-gram vectors per query,
//! and [`score_ngram_signal`] for comparing a query against a skill record's
//! trigger hints using weighted unigram+bigram+trigram cosine similarity.
//!
//! This signal is designed to catch semantic overlap that token-based
//! keyword matching misses, especially for CJK cross-language queries
//! where the same concept is expressed in different writing systems.

use routing_core::fuzzy::{character_ngrams, cosine_similarity};
use std::collections::HashMap;

use super::scoring_config::ScoringWeights;
use super::text::normalize_text;
use super::types::SkillRecord;

/// Pre-computed n-gram vectors for a single query.
///
/// Created once per `route_task()` call and reused across all skill records.
/// Avoids repeated n-gram extraction for the same query.
pub(crate) struct NgramCache {
    query_uni: HashMap<String, usize>,
    query_bi: HashMap<String, usize>,
    query_tri: HashMap<String, usize>,
}

impl NgramCache {
    pub(crate) fn new(query_text: &str) -> Self {
        let normalized = normalize_text(query_text);
        Self {
            query_uni: character_ngrams(&normalized, 1, 1),
            query_bi: character_ngrams(&normalized, 2, 2),
            query_tri: character_ngrams(&normalized, 3, 3),
        }
    }

    /// Weighted cosine similarity against a target string.
    pub(crate) fn similarity_to(&self, target: &str) -> f64 {
        let target_uni = character_ngrams(target, 1, 1);
        if target_uni.is_empty() || self.query_uni.is_empty() {
            return 0.0;
        }
        let cos_uni = cosine_similarity(&self.query_uni, &target_uni);

        let target_bi = character_ngrams(target, 2, 2);
        let cos_bi = if self.query_bi.is_empty() || target_bi.is_empty() {
            0.0
        } else {
            cosine_similarity(&self.query_bi, &target_bi)
        };

        let target_tri = character_ngrams(target, 3, 3);
        let cos_tri = if self.query_tri.is_empty() || target_tri.is_empty() {
            0.0
        } else {
            cosine_similarity(&self.query_tri, &target_tri)
        };

        0.5 * cos_uni + 0.3 * cos_bi + 0.2 * cos_tri
    }
}

/// Score the n-gram similarity signal for a single skill record.
///
/// Compares the query (via pre-computed `NgramCache`) against all of the
/// record's `trigger_hints` and returns the maximum weighted similarity,
/// clamped to `[0.0, w.ngram_similarity_max]`.
pub(crate) fn score_ngram_signal(
    cache: &NgramCache,
    record: &SkillRecord,
    w: &ScoringWeights,
) -> f64 {
    if record.trigger_hints.is_empty() {
        return 0.0;
    }
    let best = record
        .trigger_hints
        .iter()
        .map(|hint| cache.similarity_to(&normalize_text(hint)))
        .fold(0.0f64, f64::max);
    best.min(w.ngram_similarity_max)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn ngram_cache_similar_english() {
        let cache = NgramCache::new("code review");
        let sim = cache.similarity_to("review code");
        assert!(
            sim > 0.3,
            "expected >0.3 for 'review code' vs 'code review', got {sim}"
        );
    }

    #[test]
    fn ngram_cache_cjk_partial() {
        let cache = NgramCache::new("提交代码");
        let sim = cache.similarity_to("代码提交");
        assert!(sim > 0.3, "expected >0.3 for CJK reordering, got {sim}");
    }

    #[test]
    fn ngram_cache_unrelated() {
        let cache = NgramCache::new("quantum physics");
        let sim = cache.similarity_to("pizza delivery");
        assert!(sim < 0.6, "expected <0.6 for unrelated, got {sim}");
    }

    #[test]
    fn score_ngram_no_hints() {
        let cache = NgramCache::new("anything");
        let record = make_record(vec![]);
        let w = make_weights();
        assert_eq!(score_ngram_signal(&cache, &record, &w), 0.0);
    }

    #[test]
    fn score_ngram_with_hints() {
        let cache = NgramCache::new("git 提交代码");
        let record = make_record(vec!["提交代码".to_string(), "code review".to_string()]);
        let w = make_weights();
        let score = score_ngram_signal(&cache, &record, &w);
        assert!(score > 0.0, "expected positive n-gram score, got {score}");
        assert!(score <= 15.0, "expected clamped to max, got {score}");
    }

    fn make_record(trigger_hints: Vec<String>) -> SkillRecord {
        use std::collections::HashSet;
        SkillRecord {
            slug: "test".to_string(),
            skill_path: None,
            layer: "L3".to_string(),
            owner: "test".to_string(),
            gate: "none".to_string(),
            priority: "P2".to_string(),
            session_start: "n/a".to_string(),
            summary: "test".to_string(),
            slug_lower: "test".to_string(),
            owner_lower: "test".to_string(),
            gate_lower: "none".to_string(),
            session_start_lower: "n/a".to_string(),
            gate_phrases: vec![],
            trigger_hints,
            name_tokens: HashSet::new(),
            keyword_tokens: HashSet::new(),
            alias_tokens: HashSet::new(),
            do_not_use_tokens: HashSet::new(),
            framework_alias_entrypoints: vec![],
            metadata_positive_triggers: vec![],
            host_platforms: vec![],
            record_kind: "skill".to_string(),
            primary_allowed: true,
            fallback_policy_mode: "default".to_string(),
            skill_flags: vec![],
        }
    }

    fn make_weights() -> ScoringWeights {
        ScoringWeights::default()
    }
}
