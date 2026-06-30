//! Claim ledger management.
//!
//! Manages the CLAIM_LEDGER.md file — tracks claims, their evidence anchors,
//! and ceiling levels throughout the revision process.
//!
//! Format (CLAIM_LEDGER.md):
//! ```markdown
//! # Claim Ledger
//!
//! ## C1: claim text
//! - Evidence: [Strong/Moderate/Weak/Missing] source (location)
//! - Ceiling: top-venue / conference-ready / local-only / no-claim
//! ```

use crate::types::{Claim, ClaimCeiling, EvidenceAnchor, EvidenceStrength};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Load claims from a CLAIM_LEDGER.md file.
///
/// Parses the markdown format into Claim structs. Returns empty Vec if file doesn't exist.
pub fn load_ledger(path: &Path) -> Result<Vec<Claim>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read claim ledger: {}", path.display()))?;
    parse_ledger_markdown(&content)
}

/// Save claims to a CLAIM_LEDGER.md file.
pub fn save_ledger(path: &Path, claims: &[Claim]) -> Result<()> {
    let markdown = render_ledger_markdown(claims);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    core_state_utils::atomic_write::write_atomic_text(path, &markdown)
        .with_context(|| format!("failed to write claim ledger: {}", path.display()))?;
    Ok(())
}

/// Compute the overall claim ceiling for a set of claims.
///
/// Returns the ceiling of the strongest claim (TopVenue > ConferenceReady > LocalOnly > NoClaim).
pub fn compute_ceiling(claims: &[Claim]) -> ClaimCeiling {
    if claims.is_empty() {
        return ClaimCeiling::NoClaim;
    }

    let has_top = claims
        .iter()
        .any(|c| matches!(c.ceiling, ClaimCeiling::TopVenue));
    let has_conf = claims
        .iter()
        .any(|c| matches!(c.ceiling, ClaimCeiling::ConferenceReady));
    let has_local = claims
        .iter()
        .any(|c| matches!(c.ceiling, ClaimCeiling::LocalOnly));

    if has_top {
        ClaimCeiling::TopVenue
    } else if has_conf {
        ClaimCeiling::ConferenceReady
    } else if has_local {
        ClaimCeiling::LocalOnly
    } else {
        ClaimCeiling::NoClaim
    }
}

/// Parse CLAIM_LEDGER.md content into Claim structs.
fn parse_ledger_markdown(content: &str) -> Result<Vec<Claim>> {
    let mut claims = Vec::new();
    let mut current_claim: Option<Claim> = None;

    for line in content.lines() {
        let trimmed = line.trim();

        // ## C1: claim text
        if let Some(rest) = trimmed.strip_prefix("## ") {
            if let Some(colon_pos) = rest.find(": ") {
                let id = rest[..colon_pos].trim().to_string();
                let text = rest[colon_pos + 2..].trim().to_string();
                if let Some(claim) = current_claim.take() {
                    claims.push(claim);
                }
                current_claim = Some(Claim {
                    id,
                    text,
                    evidence: Vec::new(),
                    ceiling: ClaimCeiling::NoClaim,
                });
            }
        }

        // - Evidence: [Strong] source (location)
        if let Some(rest) = trimmed.strip_prefix("- Evidence:") {
            if let Some(ref mut claim) = current_claim {
                let rest = rest.trim();
                let (strength, after_bracket) = if let Some(end) = rest.find(']') {
                    let bracket_content = rest.trim_start_matches('[').trim();
                    let bracket_end = bracket_content.find(']').unwrap_or(0);
                    let strength_str = &bracket_content[..bracket_end];
                    let strength = match strength_str {
                        "Strong" => EvidenceStrength::Strong,
                        "Moderate" => EvidenceStrength::Moderate,
                        "Weak" => EvidenceStrength::Weak,
                        _ => EvidenceStrength::Missing,
                    };
                    (strength, &rest[end + 1..])
                } else {
                    (EvidenceStrength::Missing, rest)
                };

                let source_loc = after_bracket.trim();
                let (source, location) = if let Some(paren_start) = source_loc.rfind('(') {
                    // Use rfind to handle source names that contain parentheses:
                    // only the LAST parenthetical is the location.
                    let source = source_loc[..paren_start].trim().to_string();
                    let location = source_loc[paren_start + 1..]
                        .trim_end_matches(')')
                        .to_string();
                    (source, location)
                } else {
                    (source_loc.to_string(), String::new())
                };

                claim.evidence.push(EvidenceAnchor {
                    source,
                    location,
                    strength,
                });
            }
        }

        // - Ceiling: top-venue / conference-ready / local-only / no-claim
        if let Some(rest) = trimmed.strip_prefix("- Ceiling:") {
            if let Some(ref mut claim) = current_claim {
                claim.ceiling = match rest.trim() {
                    "top-venue" => ClaimCeiling::TopVenue,
                    "conference-ready" => ClaimCeiling::ConferenceReady,
                    "local-only" => ClaimCeiling::LocalOnly,
                    _ => ClaimCeiling::NoClaim,
                };
            }
        }
    }

    if let Some(claim) = current_claim {
        claims.push(claim);
    }

    Ok(claims)
}

/// Render claims to CLAIM_LEDGER.md format.
fn render_ledger_markdown(claims: &[Claim]) -> String {
    let mut lines = vec!["# Claim Ledger".to_string(), String::new()];

    for claim in claims {
        lines.push(format!("## {}: {}", claim.id, claim.text));

        for evidence in &claim.evidence {
            let strength = match evidence.strength {
                EvidenceStrength::Strong => "Strong",
                EvidenceStrength::Moderate => "Moderate",
                EvidenceStrength::Weak => "Weak",
                EvidenceStrength::Missing => "Missing",
            };
            if evidence.location.is_empty() {
                lines.push(format!("- Evidence: [{}] {}", strength, evidence.source));
            } else {
                lines.push(format!(
                    "- Evidence: [{}] {} ({})",
                    strength, evidence.source, evidence.location
                ));
            }
        }

        let ceiling = match claim.ceiling {
            ClaimCeiling::TopVenue => "top-venue",
            ClaimCeiling::ConferenceReady => "conference-ready",
            ClaimCeiling::LocalOnly => "local-only",
            ClaimCeiling::NoClaim => "no-claim",
        };
        lines.push(format!("- Ceiling: {ceiling}"));
        lines.push(String::new());
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn round_trip() {
        let claims = vec![
            Claim {
                id: "C1".into(),
                text: "Method X improves accuracy by 5%".into(),
                evidence: vec![
                    EvidenceAnchor {
                        source: "Table 2".into(),
                        location: "main results".into(),
                        strength: EvidenceStrength::Strong,
                    },
                    EvidenceAnchor {
                        source: "Figure 3".into(),
                        location: "ablation".into(),
                        strength: EvidenceStrength::Moderate,
                    },
                ],
                ceiling: ClaimCeiling::ConferenceReady,
            },
            Claim {
                id: "C2".into(),
                text: "Method X is efficient".into(),
                evidence: vec![EvidenceAnchor {
                    source: "pending".into(),
                    location: String::new(),
                    strength: EvidenceStrength::Missing,
                }],
                ceiling: ClaimCeiling::NoClaim,
            },
        ];

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("CLAIM_LEDGER.md");

        save_ledger(&path, &claims).unwrap();
        let loaded = load_ledger(&path).unwrap();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].id, "C1");
        assert_eq!(loaded[0].text, "Method X improves accuracy by 5%");
        assert_eq!(loaded[0].evidence.len(), 2);
        assert!(matches!(
            loaded[0].evidence[0].strength,
            EvidenceStrength::Strong
        ));
        assert!(matches!(loaded[0].ceiling, ClaimCeiling::ConferenceReady));
        assert!(matches!(loaded[1].ceiling, ClaimCeiling::NoClaim));
    }

    #[test]
    fn compute_ceiling_top_venue() {
        let claims = vec![Claim {
            id: "C1".into(),
            text: "test".into(),
            evidence: vec![],
            ceiling: ClaimCeiling::TopVenue,
        }];
        assert!(matches!(compute_ceiling(&claims), ClaimCeiling::TopVenue));
    }

    #[test]
    fn compute_ceiling_empty() {
        assert!(matches!(compute_ceiling(&[]), ClaimCeiling::NoClaim));
    }

    #[test]
    fn load_nonexistent_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.md");
        assert!(load_ledger(&path).unwrap().is_empty());
    }
}
