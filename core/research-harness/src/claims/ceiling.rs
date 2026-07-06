//! Claim ceiling computation.
//!
//! Determines how strong a claim can be given the available evidence.
//! Uses a weighted confidence model instead of hard thresholds:
//! - Strong = 1.0, Moderate = 0.5, Weak = 0.2
//! - confidence = weight / (weight + 1.0)  [sigmoid, bounded [0, 1))
//! - TopVenue ≥ 0.6, ConferenceReady ≥ 0.3, > 0 → LocalOnly

use crate::types::{Claim, ClaimCeiling};

/// Compute the appropriate ceiling for a claim given its evidence strength.
pub fn compute_claim_ceiling(claim: &Claim) -> ClaimCeiling {
    // Evidence with Missing strength is a placeholder (not yet filled);
    // it neither proves nor disproves the claim — skip it from ceiling logic.
    let real_evidence: Vec<_> = claim
        .evidence
        .iter()
        .filter(|e| !matches!(e.strength, crate::types::EvidenceStrength::Missing))
        .collect();

    if real_evidence.is_empty() {
        return ClaimCeiling::NoClaim;
    }

    // Weighted confidence model
    let total_weight: f64 = real_evidence
        .iter()
        .map(|e| match e.strength {
            crate::types::EvidenceStrength::Strong => 1.0,
            crate::types::EvidenceStrength::Moderate => 0.5,
            crate::types::EvidenceStrength::Weak => 0.2,
            crate::types::EvidenceStrength::Missing => 0.0, // filtered out above
        })
        .sum();

    // Sigmoid: confidence in (0, 1), approaches 1.0 asymptotically
    // With 1 strong: 1.0/2.0 = 0.50 (ConferenceReady)
    // With 2 strong: 2.0/3.0 = 0.67 (TopVenue)
    // With 1 moderate: 0.5/1.5 = 0.33 (ConferenceReady)
    let confidence = total_weight / (total_weight + 1.0);

    if total_weight == 0.0 {
        // Only Weak evidence — insufficient for publication
        ClaimCeiling::LocalOnly
    } else if confidence >= 0.6 {
        ClaimCeiling::TopVenue
    } else if confidence >= 0.3 {
        ClaimCeiling::ConferenceReady
    } else {
        ClaimCeiling::LocalOnly
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{EvidenceAnchor, EvidenceStrength};

    fn make_claim(evidence: Vec<EvidenceAnchor>) -> Claim {
        Claim {
            id: "C1".into(),
            text: "test claim".into(),
            evidence,
            ceiling: ClaimCeiling::NoClaim,
        }
    }

    fn anchor(strength: EvidenceStrength) -> EvidenceAnchor {
        EvidenceAnchor {
            source: "paper".into(),
            location: "table 1".into(),
            strength,
        }
    }

    #[test]
    fn ceiling_no_evidence() {
        let claim = make_claim(vec![]);
        assert!(matches!(
            compute_claim_ceiling(&claim),
            ClaimCeiling::NoClaim
        ));
    }

    #[test]
    fn ceiling_missing_evidence_ignored_when_strong_present() {
        // Missing evidence is a placeholder (not yet filled).
        // When Strong evidence exists, Missing should not lower the ceiling.
        let claim = make_claim(vec![
            anchor(EvidenceStrength::Strong),
            anchor(EvidenceStrength::Missing),
        ]);
        assert!(matches!(
            compute_claim_ceiling(&claim),
            ClaimCeiling::ConferenceReady
        ));
    }

    #[test]
    fn ceiling_moderate_only() {
        let claim = make_claim(vec![anchor(EvidenceStrength::Moderate)]);
        assert!(matches!(
            compute_claim_ceiling(&claim),
            ClaimCeiling::ConferenceReady
        ));
    }

    #[test]
    fn ceiling_strong_single() {
        let claim = make_claim(vec![anchor(EvidenceStrength::Strong)]);
        assert!(matches!(
            compute_claim_ceiling(&claim),
            ClaimCeiling::ConferenceReady
        ));
    }

    #[test]
    fn ceiling_strong_three_plus() {
        let claim = make_claim(vec![
            anchor(EvidenceStrength::Strong),
            anchor(EvidenceStrength::Strong),
            anchor(EvidenceStrength::Strong),
        ]);
        assert!(matches!(
            compute_claim_ceiling(&claim),
            ClaimCeiling::TopVenue
        ));
    }

    #[test]
    fn ceiling_strong_and_moderate_mixed() {
        // 1 strong + 1 moderate: weight=1.5, confidence=1.5/2.5=0.6 → TopVenue
        let claim = make_claim(vec![
            anchor(EvidenceStrength::Strong),
            anchor(EvidenceStrength::Moderate),
        ]);
        assert!(matches!(
            compute_claim_ceiling(&claim),
            ClaimCeiling::TopVenue
        ));
    }

    #[test]
    fn ceiling_two_strong_reaches_top_venue() {
        // 2 strong: weight=2.0, confidence=2.0/3.0=0.67 → TopVenue
        let claim = make_claim(vec![
            anchor(EvidenceStrength::Strong),
            anchor(EvidenceStrength::Strong),
        ]);
        assert!(matches!(
            compute_claim_ceiling(&claim),
            ClaimCeiling::TopVenue
        ));
    }

    #[test]
    fn ceiling_weak_only_stays_local() {
        let claim = make_claim(vec![
            anchor(EvidenceStrength::Weak),
            anchor(EvidenceStrength::Weak),
        ]);
        assert!(matches!(
            compute_claim_ceiling(&claim),
            ClaimCeiling::LocalOnly
        ));
    }
}
