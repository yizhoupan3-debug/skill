//! Loop-level anti-drift check.
//!
//! Every N review cycles (default 3), compares the current goal text
//! against the original snapshot to detect scope creep or direction shift.
//! Uses Jaccard similarity on content words (lightweight, standalone).

use crate::types::{AntiDriftState, DriftCheckResult};
use std::collections::HashSet;

/// Drift detection threshold: score above this indicates goal drift has occurred.
const DRIFT_DETECTION_THRESHOLD: f64 = 0.3;
/// Scope expansion threshold: score above this indicates significant scope expansion.
const SCOPE_EXPANSION_THRESHOLD: f64 = 0.6;

/// Perform a drift check: compare original goal snapshot to current goal text.
pub fn perform_drift_check(
    anti_drift: &mut AntiDriftState,
    current_goal_text: &str,
) -> DriftCheckResult {
    let original = anti_drift.original_goal_snapshot.as_deref().unwrap_or("");

    let drift_score = jaccard_drift(original, current_goal_text);
    let drift_detected = drift_score > DRIFT_DETECTION_THRESHOLD;
    let drift_type = if !drift_detected {
        "none".to_string()
    } else if drift_score > SCOPE_EXPANSION_THRESHOLD {
        "scope_expansion".to_string()
    } else {
        "evidence_shift".to_string()
    };

    DriftCheckResult {
        checked_at: framework_core::time::now_iso(),
        review_cycle: anti_drift.review_cycle_count,
        drift_detected,
        drift_score,
        drift_type,
        detail: format!(
            "jaccard_drift={:.2} original_len={} current_len={}",
            drift_score,
            original.len(),
            current_goal_text.len()
        ),
    }
}

/// Check if a drift check should fire based on cycle count.
pub fn should_check_drift(anti_drift: &AntiDriftState) -> bool {
    anti_drift.check_interval > 0
        && anti_drift.review_cycle_count > 0
        && anti_drift
            .review_cycle_count
            .is_multiple_of(anti_drift.check_interval)
}

/// Lightweight Jaccard drift: 1.0 - jaccard_similarity.
fn jaccard_drift(a: &str, b: &str) -> f64 {
    let words_a = extract_words(a);
    let words_b = extract_words(b);
    if words_a.is_empty() && words_b.is_empty() {
        return 0.0;
    }
    if words_a.is_empty() || words_b.is_empty() {
        return 1.0;
    }
    let intersection = words_a.intersection(&words_b).count();
    let union_len = words_a.len() + words_b.len() - intersection;
    if union_len == 0 {
        0.0
    } else {
        1.0 - (intersection as f64 / union_len as f64)
    }
}

fn extract_words(text: &str) -> HashSet<String> {
    let mut words = HashSet::new();

    // Split on non-alphanumeric boundaries (works for English, code, etc.)
    for token in text.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-') {
        let lower = token.to_ascii_lowercase();
        if lower.len() >= 3 && !STOPWORDS.contains(&lower.as_str()) {
            words.insert(lower);
        }
    }

    // Chinese bigram extraction: slide a 2-char window over CJK characters.
    // This gives word-level granularity for Chinese text (no spaces in Chinese).
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i + 1 < chars.len() {
        let c0 = chars[i];
        let c1 = chars[i + 1];
        if is_cjk(c0) && is_cjk(c1) {
            let bigram = format!("{c0}{c1}");
            if !STOPWORDS.contains(&bigram.as_str()) {
                words.insert(bigram);
            }
            i += 1; // slide by 1 to get overlapping bigrams
        } else {
            i += 1;
        }
    }

    words
}

/// Check if a character is CJK (Chinese/Japanese/Korean).
fn is_cjk(c: char) -> bool {
    ('\u{4E00}'..='\u{9FFF}').contains(&c)   // CJK Unified Ideographs
        || ('\u{3400}'..='\u{4DBF}').contains(&c) // CJK Extension A
        || ('\u{F900}'..='\u{FAFF}').contains(&c) // CJK Compatibility Ideographs
}

static STOPWORDS: &[&str] = &[
    "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
    "do", "does", "did", "will", "would", "could", "should", "may", "might", "shall", "can",
    "need", "dare", "ought", "used", "to", "of", "in", "for", "on", "with", "at", "by", "from",
    "as", "into", "through", "during", "before", "after", "above", "below", "between", "out",
    "off", "over", "under", "again", "further", "then", "once", "and", "but", "or", "nor", "not",
    "so", "yet", "both", "this", "that", "these", "those", "it", "its", "we", "our", "they",
    "their", "的", "了", "在", "是", "我", "有", "和", "就", "不", "人", "都", "一", "一个", "上",
    "也", "很", "到", "说", "要", "去", "你", "会", "着", "没有", "看", "好", "自己", "这",
];

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn test_no_drift_identical() {
        let mut state = AntiDriftState {
            original_goal_snapshot: Some("修复认证 bug".to_string()),
            ..Default::default()
        };
        let result = perform_drift_check(&mut state, "修复认证 bug");
        assert!(!result.drift_detected);
        assert!(result.drift_score < 0.1);
    }

    #[test]
    fn test_drift_detected() {
        let mut state = AntiDriftState {
            original_goal_snapshot: Some("修复认证 bug".to_string()),
            ..Default::default()
        };
        let result = perform_drift_check(&mut state, "重写前端 UI 组件库并优化性能");
        assert!(result.drift_detected);
        assert!(result.drift_score > 0.5);
    }

    #[test]
    fn test_should_check_drift_at_interval() {
        let state_1 = AntiDriftState {
            review_cycle_count: 1,
            check_interval: 3,
            ..Default::default()
        };
        assert!(!should_check_drift(&state_1));
        let state_3 = AntiDriftState {
            review_cycle_count: 3,
            check_interval: 3,
            ..Default::default()
        };
        assert!(should_check_drift(&state_3));
        let state_6 = AntiDriftState {
            review_cycle_count: 6,
            check_interval: 3,
            ..Default::default()
        };
        assert!(should_check_drift(&state_6));
        let state_4 = AntiDriftState {
            review_cycle_count: 4,
            check_interval: 3,
            ..Default::default()
        };
        assert!(!should_check_drift(&state_4));
    }
}
