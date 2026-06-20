//! Claim ceiling computation.
//!
//! Determines how strong a claim can be given the available evidence.

use crate::types::{Claim, ClaimCeiling};

/// Compute the appropriate ceiling for a claim given its evidence strength.
pub fn compute_claim_ceiling(claim: &Claim) -> ClaimCeiling {
    let has_strong = claim.evidence.iter().any(|e| {
        matches!(e.strength, crate::types::EvidenceStrength::Strong)
    });
    let has_moderate = claim.evidence.iter().any(|e| {
        matches!(e.strength, crate::types::EvidenceStrength::Moderate)
    });
    let has_missing = claim.evidence.iter().any(|e| {
        matches!(e.strength, crate::types::EvidenceStrength::Missing)
    });

    if has_missing {
        ClaimCeiling::NoClaim
    } else if !has_strong && !has_moderate {
        ClaimCeiling::NoClaim
    } else if has_strong && claim.evidence.len() >= 3 {
        ClaimCeiling::TopVenue
    } else if has_strong || has_moderate {
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
    fn ceiling_missing_evidence_blocks() {
        let claim = make_claim(vec![
            anchor(EvidenceStrength::Strong),
            anchor(EvidenceStrength::Missing),
        ]);
        assert!(matches!(
            compute_claim_ceiling(&claim),
            ClaimCeiling::NoClaim
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
        let claim = make_claim(vec![
            anchor(EvidenceStrength::Strong),
            anchor(EvidenceStrength::Moderate),
        ]);
        assert!(matches!(
            compute_claim_ceiling(&claim),
            ClaimCeiling::ConferenceReady
        ));
    }
}
