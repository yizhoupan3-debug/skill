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
