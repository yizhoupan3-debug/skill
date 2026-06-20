//! AIGC scorer — combine detection signals into a composite 0-100 score.

use crate::types::AigcDetectionResult;

/// Compute a weighted composite AIGC score (0–100) from per-segment detection results.
///
/// Weights:
/// - n-gram anomaly: 0.3
/// - burstiness (low CV = AI): 0.4
/// - syntactic patterns: 0.3
///
/// If no results are provided, returns 0.
pub fn score(results: &[AigcDetectionResult]) -> u32 {
    if results.is_empty() {
        return 0;
    }

    let total_weight = 0.3 + 0.4 + 0.3; // = 1.0
    let mut weighted_sum = 0.0f64;
    let mut count = 0.0f64;

    for result in results {
        let mut ngram = 0.0f64;
        let mut burst = 0.0f64;
        let mut pattern = 0.0f64;

        for signal in &result.signals {
            match signal.signal_type {
                crate::types::AigcSignalType::NGramAnomaly => ngram = signal.value,
                crate::types::AigcSignalType::LowBurstiness => burst = signal.value,
                crate::types::AigcSignalType::SyntacticPattern => pattern = signal.value,
                _ => {}
            }
        }

        let segment_score = (ngram * 0.3 + burst * 0.4 + pattern * 0.3) / total_weight;
        weighted_sum += segment_score;
        count += 1.0;
    }

    if count == 0.0 {
        return 0;
    }

    let avg = weighted_sum / count;
    // Convert from 0.0–1.0 to 0–100 and clamp.
    ((avg * 100.0).round() as u32).min(100)
}

/// Compute per-segment scores (0–100) alongside the composite.
pub fn score_per_segment(results: &[AigcDetectionResult]) -> Vec<(String, u32)> {
    results
        .iter()
        .map(|r| {
            let seg_score = score(std::slice::from_ref(r));
            (r.segment_id.clone(), seg_score)
        })
        .collect()
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AigcSignal, AigcSignalType};

    fn make_result(
        segment_id: &str,
        ngram: f64,
        burst: f64,
        pattern: f64,
    ) -> AigcDetectionResult {
        AigcDetectionResult {
            segment_id: segment_id.to_string(),
            ai_probability: 0.0, // overridden by scorer
            score: 0,
            signals: vec![
                AigcSignal {
                    signal_type: AigcSignalType::NGramAnomaly,
                    value: ngram,
                    detail: String::new(),
                },
                AigcSignal {
                    signal_type: AigcSignalType::LowBurstiness,
                    value: burst,
                    detail: String::new(),
                },
                AigcSignal {
                    signal_type: AigcSignalType::SyntacticPattern,
                    value: pattern,
                    detail: String::new(),
                },
            ],
        }
    }

    #[test]
    fn test_score_range_0_to_100() {
        // All zeros → 0
        let low = vec![make_result("a", 0.0, 0.0, 0.0)];
        assert_eq!(score(&low), 0);

        // All ones → 100
        let high = vec![make_result("a", 1.0, 1.0, 1.0)];
        assert_eq!(score(&high), 100);

        // Mid range
        let mid = vec![make_result("a", 0.5, 0.5, 0.5)];
        let s = score(&mid);
        assert!((40..=60).contains(&s), "mid-range score should be ~50, got {s}");
    }

    #[test]
    fn test_empty_results_returns_zero() {
        assert_eq!(score(&[]), 0);
    }

    #[test]
    fn test_multiple_segments_averaged() {
        let r1 = make_result("seg-0", 1.0, 1.0, 1.0); // → 100
        let r2 = make_result("seg-1", 0.0, 0.0, 0.0); // → 0
        let combined = score(&[r1, r2]);
        assert_eq!(combined, 50);
    }

    #[test]
    fn test_per_segment_scoring() {
        let r1 = make_result("seg-0", 1.0, 1.0, 1.0);
        let r2 = make_result("seg-1", 0.0, 0.0, 0.0);
        let per_seg = score_per_segment(&[r1, r2]);
        assert_eq!(per_seg.len(), 2);
        assert_eq!(per_seg[0].1, 100);
        assert_eq!(per_seg[1].1, 0);
    }
}
