//! Claim drift detection.
//!
//! Detects when claims have shifted from their original formulation,
//! which may indicate scope creep or unsupported escalation.
//!
//! Drift analysis compares original vs current claims along three dimensions:
//! - Text similarity (Jaccard similarity of content words)
//! - Evidence shift (evidence anchor changes)
//! - Ceiling change (claim ceiling escalation/de-escalation)

use crate::types::Claim;
use anyhow::Result;
use std::collections::HashSet;

/// Drift analysis result for a single claim.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DriftResult {
    pub claim_id: String,
    pub drift_score: f64, // 0.0 = no drift, 1.0 = complete drift
    pub drift_type: DriftType,
    pub detail: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftType {
    None,
    ScopeExpansion,
    ScopeContraction,
    EvidenceShift,
    ToneShift,
}

/// Analyze drift between original and current claims.
///
/// Pairs claims by id; if a current claim has no matching original, it's flagged
/// as scope expansion. If an original claim has no matching current, it's flagged
/// as scope contraction.
pub fn detect_drift(original: &[Claim], current: &[Claim]) -> Result<Vec<DriftResult>> {
    let mut results = Vec::new();

    // Build lookup maps
    let orig_map: std::collections::HashMap<&str, &Claim> =
        original.iter().map(|c| (c.id.as_str(), c)).collect();
    let curr_map: std::collections::HashMap<&str, &Claim> =
        current.iter().map(|c| (c.id.as_str(), c)).collect();

    // Compare matched claims
    for (id, orig_claim) in &orig_map {
        if let Some(curr_claim) = curr_map.get(id) {
            results.push(analyze_single_drift(orig_claim, curr_claim));
        } else {
            results.push(DriftResult {
                claim_id: id.to_string(),
                drift_score: 1.0,
                drift_type: DriftType::ScopeContraction,
                detail: "claim removed in current revision".to_string(),
            });
        }
    }

    // New claims in current (not in original)
    for id in curr_map.keys() {
        if !orig_map.contains_key(id) {
            results.push(DriftResult {
                claim_id: id.to_string(),
                drift_score: 0.8,
                drift_type: DriftType::ScopeExpansion,
                detail: "new claim added in current revision".to_string(),
            });
        }
    }

    Ok(results)
}

/// Analyze drift between a pair of matched claims.
fn analyze_single_drift(orig: &Claim, curr: &Claim) -> DriftResult {
    let text_sim = jaccard_similarity(&orig.text, &curr.text);

    // Text drift: 1 - similarity
    let text_drift = 1.0 - text_sim;

    // Evidence shift: compare evidence sources
    let orig_sources: HashSet<&str> = orig.evidence.iter().map(|e| e.source.as_str()).collect();
    let curr_sources: HashSet<&str> = curr.evidence.iter().map(|e| e.source.as_str()).collect();
    let evidence_sim = if orig_sources.is_empty() && curr_sources.is_empty() {
        1.0
    } else if orig_sources.is_empty() || curr_sources.is_empty() {
        0.0
    } else {
        let intersection = orig_sources.intersection(&curr_sources).count();
        let union = orig_sources.union(&curr_sources).count();
        intersection as f64 / union as f64
    };
    let evidence_drift = 1.0 - evidence_sim;

    // Ceiling change
    let ceiling_drift = ceiling_distance(&orig.ceiling, &curr.ceiling);

    // Weighted aggregate
    let drift_score = round_to(
        0.5 * text_drift + 0.25 * evidence_drift + 0.25 * ceiling_drift,
        2,
    );

    // Determine drift type
    let drift_type = if drift_score < 0.15 {
        DriftType::None
    } else if ceiling_drift > 0.5 {
        if ceiling_value(&curr.ceiling) > ceiling_value(&orig.ceiling) {
            DriftType::ScopeExpansion
        } else {
            DriftType::ScopeContraction
        }
    } else if evidence_drift > text_drift {
        DriftType::EvidenceShift
    } else {
        DriftType::ToneShift
    };

    DriftResult {
        claim_id: orig.id.clone(),
        drift_score,
        drift_type,
        detail: format!(
            "text_drift={:.2}, evidence_drift={:.2}, ceiling_drift={:.2}",
            text_drift, evidence_drift, ceiling_drift
        ),
    }
}

/// Compute Jaccard similarity between two texts using content words.
fn jaccard_similarity(a: &str, b: &str) -> f64 {
    let words_a: HashSet<String> = extract_content_words(a);
    let words_b: HashSet<String> = extract_content_words(b);

    if words_a.is_empty() && words_b.is_empty() {
        return 1.0;
    }
    if words_a.is_empty() || words_b.is_empty() {
        return 0.0;
    }

    let intersection = words_a.intersection(&words_b).count();
    let union = words_a.union(&words_b).count();
    intersection as f64 / union as f64
}

/// Extract content words (≥3 chars, lowercased, no stopwords).
fn extract_content_words(text: &str) -> HashSet<String> {
    crate::text::extract_content_words(text)
}

/// Compute the maximum possible ceiling value across all variants.
fn ceiling_max() -> u8 {
    // Derived directly from TopVenue — if a future variant has a higher value,
    // ceiling_value(TopVenue) must also be updated, keeping this in sync.
    ceiling_value(&crate::types::ClaimCeiling::TopVenue)
}

/// Compute distance between two ceiling levels (0.0 - 1.0).
fn ceiling_distance(a: &crate::types::ClaimCeiling, b: &crate::types::ClaimCeiling) -> f64 {
    let va = ceiling_value(a);
    let vb = ceiling_value(b);
    let max_val = ceiling_max();
    if max_val == 0 {
        return 0.0;
    }
    (va as f64 - vb as f64).abs() / max_val as f64
}

/// Numeric value for ceiling comparison.
fn ceiling_value(c: &crate::types::ClaimCeiling) -> u8 {
    match c {
        crate::types::ClaimCeiling::NoClaim => 0,
        crate::types::ClaimCeiling::LocalOnly => 1,
        crate::types::ClaimCeiling::ConferenceReady => 2,
        crate::types::ClaimCeiling::TopVenue => 3,
    }
}

fn round_to(value: f64, decimals: i32) -> f64 {
    let factor = 10f64.powi(decimals);
    (value * factor).round() / factor
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::types::{ClaimCeiling, EvidenceAnchor, EvidenceStrength};

    fn make_claim(id: &str, text: &str) -> Claim {
        Claim {
            id: id.into(),
            text: text.into(),
            evidence: vec![],
            ceiling: ClaimCeiling::ConferenceReady,
        }
    }

    #[test]
    fn no_drift_identical_claims() {
        let claims = vec![make_claim("C1", "Method X improves accuracy")];
        let results = detect_drift(&claims, &claims).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].drift_score < 0.15);
        assert!(matches!(results[0].drift_type, DriftType::None));
    }

    #[test]
    fn scope_expansion_new_claim() {
        let orig = vec![make_claim("C1", "Method X improves accuracy")];
        let curr = vec![
            make_claim("C1", "Method X improves accuracy"),
            make_claim("C2", "Method X is also efficient"),
        ];
        let results = detect_drift(&orig, &curr).unwrap();
        let new_claim = results.iter().find(|r| r.claim_id == "C2").unwrap();
        assert!(matches!(new_claim.drift_type, DriftType::ScopeExpansion));
    }

    #[test]
    fn scope_contraction_removed_claim() {
        let orig = vec![make_claim("C1", "A"), make_claim("C2", "B")];
        let curr = vec![make_claim("C1", "A")];
        let results = detect_drift(&orig, &curr).unwrap();
        let removed = results.iter().find(|r| r.claim_id == "C2").unwrap();
        assert!(matches!(removed.drift_type, DriftType::ScopeContraction));
    }

    #[test]
    fn text_drift_rewording() {
        let mut orig_claim = make_claim(
            "C1",
            "The transformer model achieves state of the art results on machine translation benchmarks",
        );
        orig_claim.ceiling = ClaimCeiling::TopVenue;
        let mut curr_claim = make_claim(
            "C1",
            "Our proposed architecture demonstrates competitive performance across multiple natural language processing tasks",
        );
        curr_claim.ceiling = ClaimCeiling::TopVenue;
        let results = detect_drift(&[orig_claim], &[curr_claim]).unwrap();
        assert!(results[0].drift_score > 0.3);
    }

    #[test]
    fn evidence_shift_detected() {
        let mut orig = make_claim("C1", "same text");
        orig.evidence = vec![EvidenceAnchor {
            source: "Table 1".into(),
            location: "".into(),
            strength: EvidenceStrength::Strong,
        }];
        let mut curr = make_claim("C1", "same text");
        curr.evidence = vec![EvidenceAnchor {
            source: "Figure 5".into(),
            location: "".into(),
            strength: EvidenceStrength::Moderate,
        }];
        let results = detect_drift(&[orig], &[curr]).unwrap();
        assert!(results[0].drift_score > 0.0);
    }

    #[test]
    fn ceiling_max_matches_variants() {
        // Guard: when a new ClaimCeiling variant is added, ceiling_max and
        // ceiling_value must be updated together.
        assert_eq!(
            ceiling_max(),
            3,
            "ceiling_max must match the largest ceiling_value. If you added a variant to ClaimCeiling, update ceiling_value and ceiling_max."
        );
        assert_eq!(ceiling_value(&ClaimCeiling::TopVenue), 3);
    }
}
