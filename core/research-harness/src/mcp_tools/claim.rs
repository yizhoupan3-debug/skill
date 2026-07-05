//! Claim drift detection tool + parsing helpers.
//!
//! # Functions
//! - `tool_research_claim_drift` — detect drift between original and current claims
//! - `parse_claim_ceiling` — parse a ceiling string into `ClaimCeiling`
//! - `parse_evidence_anchors` — parse an evidence array into `Vec<EvidenceAnchor>`

use core_errors::FrameworkError;
use serde_json::{Value, json};

/// Maximum number of elements in any array-type parameter to a research tool.
/// Imported from parent module where `MAX_ARRAY_ELEMENTS` is defined as `pub(super)`.
use super::MAX_ARRAY_ELEMENTS;

/// Parse a `ceiling` string value into `ClaimCeiling`.
fn parse_claim_ceiling(s: Option<&str>) -> crate::types::ClaimCeiling {
    use crate::types::ClaimCeiling;
    match s {
        Some("no-claim") => ClaimCeiling::NoClaim,
        Some("local-only") => ClaimCeiling::LocalOnly,
        Some("conference-ready") | Some("conference_ready") => ClaimCeiling::ConferenceReady,
        Some("top-venue") | Some("top_venue") => ClaimCeiling::TopVenue,
        _ => ClaimCeiling::ConferenceReady,
    }
}

/// Parse an optional `evidence` array into `Vec<EvidenceAnchor>`.
fn parse_evidence_anchors(arr: Option<&[Value]>) -> Vec<crate::types::EvidenceAnchor> {
    use crate::types::{EvidenceAnchor, EvidenceStrength};
    arr.map(|items| {
        items
            .iter()
            .filter_map(|v| {
                let source = v.get("source").and_then(Value::as_str)?;
                Some(EvidenceAnchor {
                    source: source.to_string(),
                    location: v
                        .get("location")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    strength: match v.get("strength").and_then(Value::as_str) {
                        Some("strong") => EvidenceStrength::Strong,
                        Some("moderate") => EvidenceStrength::Moderate,
                        Some("weak") => EvidenceStrength::Weak,
                        _ => EvidenceStrength::Missing,
                    },
                })
            })
            .collect()
    })
    .unwrap_or_default()
}

/// Detect claim drift between original and current claims.
pub(super) fn tool_research_claim_drift(arguments: &Value) -> Result<String, FrameworkError> {
    let original_claims_val = arguments
        .get("original_claims")
        .and_then(Value::as_array)
        .ok_or(FrameworkError::validation(
            "research_claim_drift requires 'original_claims' array",
        ))?;
    let current_claims_val = arguments
        .get("current_claims")
        .and_then(Value::as_array)
        .ok_or(FrameworkError::validation(
            "research_claim_drift requires 'current_claims' array",
        ))?;

    // Enforce array size limits
    if original_claims_val.len() > MAX_ARRAY_ELEMENTS {
        return Err(FrameworkError::validation(format!(
            "original_claims array too large: {} elements (max {MAX_ARRAY_ELEMENTS})",
            original_claims_val.len()
        )));
    }
    if current_claims_val.len() > MAX_ARRAY_ELEMENTS {
        return Err(FrameworkError::validation(format!(
            "current_claims array too large: {} elements (max {MAX_ARRAY_ELEMENTS})",
            current_claims_val.len()
        )));
    }

    // Clone to satisfy borrow checker
    let original_claims = original_claims_val.clone();
    let current_claims = current_claims_val.clone();

    let parse_claims = |arr: &[Value]| -> Vec<crate::types::Claim> {
        arr.iter()
            .map(|v| {
                let id = v
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let text = v
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let ceiling = parse_claim_ceiling(v.get("ceiling").and_then(Value::as_str));
                let evidence = parse_evidence_anchors(
                    v.get("evidence")
                        .and_then(Value::as_array)
                        .map(|v| v.as_slice()),
                );
                crate::types::Claim {
                    id,
                    text,
                    evidence,
                    ceiling,
                }
            })
            .collect()
    };

    let orig = parse_claims(&original_claims);
    let curr = parse_claims(&current_claims);

    let results = crate::claims::drift::detect_drift(&orig, &curr)
        .map_err(|e| FrameworkError::validation(e.to_string()))?;

    serde_json::to_string_pretty(&json!({
        "drift_results": results,
        "total_claims_analyzed": results.len(),
    }))
    .map_err(FrameworkError::Json)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::super::handle_research_tool;
    use serde_json::{Value, json};

    #[test]
    fn research_claim_drift_missing_required() {
        let result = handle_research_tool("research_claim_drift", &json!({}));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("requires 'original_claims'")
        );
    }

    #[test]
    fn research_claim_drift_basic() {
        let result = handle_research_tool(
            "research_claim_drift",
            &json!({
                "original_claims": [{"id": "c1", "text": "Method A achieves 95% accuracy."}],
                "current_claims": [{"id": "c1", "text": "Method A achieves 92% accuracy on the test set."}],
            }),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn research_claim_drift_with_ceiling_and_evidence() {
        let result = handle_research_tool(
            "research_claim_drift",
            &json!({
                "original_claims": [{
                    "id": "c1",
                    "text": "Our approach outperforms all baselines.",
                    "ceiling": "top-venue",
                    "evidence": [{"source": "Table 2", "location": "p.5", "strength": "strong"}],
                }],
                "current_claims": [{
                    "id": "c1",
                    "text": "Our approach outperforms existing methods.",
                    "ceiling": "conference-ready",
                    "evidence": [{"source": "Table 2", "location": "p.5", "strength": "moderate"}],
                }],
            }),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn research_claim_drift_empty_arrays() {
        let result = handle_research_tool(
            "research_claim_drift",
            &json!({"original_claims": [], "current_claims": []}),
        );
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(
            parsed.get("total_claims_analyzed").and_then(Value::as_u64),
            Some(0)
        );
    }

    #[test]
    fn parse_claim_ceiling_variants() {
        use crate::types::ClaimCeiling;
        assert_eq!(super::parse_claim_ceiling(Some("no-claim")), ClaimCeiling::NoClaim);
        assert_eq!(
            super::parse_claim_ceiling(Some("local-only")),
            ClaimCeiling::LocalOnly
        );
        assert_eq!(
            super::parse_claim_ceiling(Some("conference-ready")),
            ClaimCeiling::ConferenceReady
        );
        assert_eq!(
            super::parse_claim_ceiling(Some("conference_ready")),
            ClaimCeiling::ConferenceReady
        );
        assert_eq!(
            super::parse_claim_ceiling(Some("top-venue")),
            ClaimCeiling::TopVenue
        );
        assert_eq!(
            super::parse_claim_ceiling(Some("top_venue")),
            ClaimCeiling::TopVenue
        );
        assert_eq!(super::parse_claim_ceiling(None), ClaimCeiling::ConferenceReady);
        assert_eq!(
            super::parse_claim_ceiling(Some("unknown")),
            ClaimCeiling::ConferenceReady
        );
    }

    #[test]
    fn parse_evidence_anchors_empty() {
        assert!(super::parse_evidence_anchors(None).is_empty());
    }

    #[test]
    fn parse_evidence_anchors_basic() {
        use crate::types::EvidenceStrength;
        let input = json!([
            {"source": "Table 1", "location": "p.3", "strength": "strong"},
            {"source": "Figure 2", "strength": "weak"},
        ]);
        let anchors = super::parse_evidence_anchors(Some(input.as_array().unwrap()));
        assert_eq!(anchors.len(), 2);
        assert_eq!(anchors[0].source, "Table 1");
        assert_eq!(anchors[0].strength, EvidenceStrength::Strong);
        assert_eq!(anchors[1].source, "Figure 2");
        assert_eq!(anchors[1].strength, EvidenceStrength::Weak);
    }
}
