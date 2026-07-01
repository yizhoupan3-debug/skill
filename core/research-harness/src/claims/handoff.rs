//! Cross-session handoff artifacts.
//!
//! Structured JSON format for resuming a research session from a prior
//! state (e.g., loaded from BARRIER_REPORT.json or a barrier handoff).
//!
//! Unlike the markdown-based [`super::ledger`], this is machine-readable
//! and designed for cross-session / cross-CLAUDE-session handoff.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

// ── Public Types ──

/// A single claim entry in the handoff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffEntry {
    pub claim_id: String,
    pub claim_text: String,
    pub evidence: Vec<EvidenceEntry>,
    pub novelty_score: Option<f64>,
    pub novelty_verdict: Option<String>,
    pub ceiling: Option<String>,
    pub status: String,
}

/// Evidence attached to a handoff entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceEntry {
    pub source: String,
    pub location: String,
    pub strength: String,
}

/// A review round snapshot for the handoff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRoundSnapshot {
    pub round: u64,
    pub dimension: String,
    pub verdict: String,
    pub finding_count: u64,
}

/// Top-level metadata for the handoff artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffMetadata {
    pub total_claims: usize,
    pub total_evidence: usize,
    pub top_ceiling: Option<String>,
    pub novelty_verdict: Option<String>,
    pub progress_summary: String,
}

/// Full handoff artifact for cross-session resumption.
///
/// Serialized to `artifacts/handoff/claim-ledger.json` by
/// [`save_handoff`] and consumed by [`load_handoff`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffArtifact {
    pub schema_version: u64,
    pub created_at: String,
    pub project: Option<String>,
    pub question: Option<String>,
    pub mode: Option<String>,
    pub entries: Vec<HandoffEntry>,
    pub review_rounds: Vec<ReviewRoundSnapshot>,
    pub metadata: HandoffMetadata,
}

// ── Build from research state ──

/// Build a [`HandoffArtifact`] from a research state JSON Value.
///
/// Extracts claims from `novelty_gate.claims`, evidence from
/// `novelty_gate.claim_records`, and review rounds from `review_history`.
/// Falls back gracefully for missing fields (backward compatible).
pub fn build_handoff(state: &Value) -> HandoffArtifact {
    let project = state
        .get("project")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let question = state
        .get("question")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let mode = state
        .get("mode")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let created_at = state
        .get("updated_at")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let entries = build_entries(state);
    let review_rounds = build_review_rounds(state);
    let total_claims = entries.len();
    let total_evidence: usize = entries.iter().map(|e| e.evidence.len()).sum();
    let top_ceiling = entries
        .iter()
        .filter_map(|e| e.ceiling.as_deref())
        .max_by(|a, b| ceiling_rank(a).cmp(&ceiling_rank(b)))
        .map(ToString::to_string);
    let novelty_verdict = state
        .get("novelty_gate")
        .and_then(|g| g.get("decision"))
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let progress_summary = format!(
        "{} claims, {} evidence items, {} review rounds",
        entries.len(),
        total_evidence,
        review_rounds.len(),
    );

    HandoffArtifact {
        schema_version: 1,
        created_at,
        project,
        question,
        mode,
        entries,
        review_rounds,
        metadata: HandoffMetadata {
            total_claims,
            total_evidence,
            top_ceiling,
            novelty_verdict,
            progress_summary,
        },
    }
}

/// Build entries from the state's `novelty_gate` section.
fn build_entries(state: &Value) -> Vec<HandoffEntry> {
    let gate = match state.get("novelty_gate") {
        Some(g) if g.is_object() => g,
        _ => return Vec::new(),
    };

    let claim_records: Vec<&Value> = gate
        .get("claim_records")
        .and_then(Value::as_array)
        .map(|a| a.iter().collect())
        .unwrap_or_default();

    let claims: Vec<&Value> = gate
        .get("claims")
        .and_then(Value::as_array)
        .map(|a| a.iter().collect())
        .unwrap_or_default();

    if claims.is_empty() && claim_records.is_empty() {
        return gate
            .get("draft_claims")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| {
                        c.as_str()
                            .map(|s| HandoffEntry {
                                claim_id: s.chars().take(12).collect(),
                                claim_text: s.to_string(),
                                evidence: Vec::new(),
                                novelty_score: None,
                                novelty_verdict: None,
                                ceiling: None,
                                status: "draft".into(),
                            })
                            .or_else(|| {
                                let claim_text = c
                                    .get("claim")
                                    .and_then(Value::as_str)
                                    .unwrap_or("unknown")
                                    .to_string();
                                Some(HandoffEntry {
                                    claim_id: c
                                        .get("claim_id")
                                        .and_then(Value::as_str)
                                        .unwrap_or("?")
                                        .to_string(),
                                    claim_text,
                                    evidence: Vec::new(),
                                    novelty_score: c.get("novelty_score").and_then(Value::as_f64),
                                    novelty_verdict: c
                                        .get("verdict")
                                        .and_then(Value::as_str)
                                        .map(ToString::to_string),
                                    ceiling: None,
                                    status: "draft".into(),
                                })
                            })
                    })
                    .collect()
            })
            .unwrap_or_default();
    }

    let mut entries: Vec<HandoffEntry> = claim_records
        .iter()
        .map(|rec| {
            let claim_id = rec
                .get("claim_id")
                .and_then(Value::as_str)
                .unwrap_or("?")
                .to_string();
            let claim_text = rec
                .get("claim")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let evidence = rec
                .get("evidence")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .map(|e| EvidenceEntry {
                            source: e
                                .get("source")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string(),
                            location: e
                                .get("location")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string(),
                            strength: e
                                .get("strength")
                                .and_then(Value::as_str)
                                .unwrap_or("missing")
                                .to_string(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            let novelty_score = rec.get("novelty_score").and_then(Value::as_f64);
            let novelty_verdict = rec
                .get("verdict")
                .and_then(Value::as_str)
                .map(ToString::to_string);
            let ceiling = rec
                .get("ceiling")
                .and_then(Value::as_str)
                .map(ToString::to_string);
            let status = rec
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("active")
                .to_string();

            HandoffEntry {
                claim_id,
                claim_text,
                evidence,
                novelty_score,
                novelty_verdict,
                ceiling,
                status,
            }
        })
        .collect();

    for claim_val in &claims {
        let name = claim_val.as_str().unwrap_or("");
        if !entries
            .iter()
            .any(|e| e.claim_text == name || e.claim_id == name)
        {
            entries.push(HandoffEntry {
                claim_id: name.chars().take(12).collect(),
                claim_text: name.to_string(),
                evidence: Vec::new(),
                novelty_score: None,
                novelty_verdict: None,
                ceiling: None,
                status: "identified".into(),
            });
        }
    }

    entries
}

fn ceiling_rank(ceiling: &str) -> u8 {
    use crate::types::ClaimCeiling;
    match ceiling {
        "top-venue" | "TopVenue" => ClaimCeiling::TopVenue.rank(),
        "conference-ready" | "ConferenceReady" => ClaimCeiling::ConferenceReady.rank(),
        "local-only" | "LocalOnly" => ClaimCeiling::LocalOnly.rank(),
        "no-claim" | "NoClaim" => ClaimCeiling::NoClaim.rank(),
        _ => 0,
    }
}

/// Build review round snapshots from `review_history` or `convergence_state`.
fn build_review_rounds(state: &Value) -> Vec<ReviewRoundSnapshot> {
    let mut rounds: Vec<ReviewRoundSnapshot> = state
        .get("review_history")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|r| ReviewRoundSnapshot {
                    round: r.get("round").and_then(Value::as_u64).unwrap_or(0),
                    dimension: r
                        .get("dimension")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    verdict: r
                        .get("verdict")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                        .to_string(),
                    finding_count: r
                        .get("findings")
                        .and_then(Value::as_array)
                        .map(|a| a.len() as u64)
                        .unwrap_or(0),
                })
                .collect()
        })
        .unwrap_or_default();

    if rounds.is_empty() {
        if let Some(cs) = state.get("convergence_state") {
            let round = cs.get("current_round").and_then(Value::as_u64).unwrap_or(0);
            if round > 0 {
                rounds.push(ReviewRoundSnapshot {
                    round,
                    dimension: "unknown".into(),
                    verdict: "in_progress".into(),
                    finding_count: 0,
                });
            }
        }
    }

    rounds
}

// ── I/O ──

/// Save a [`HandoffArtifact`] extracted from state to
/// `artifacts/handoff/claim-ledger.json` under `handoff_dir`.
pub fn save_handoff(state: &Value, handoff_dir: &Path) -> std::io::Result<PathBuf> {
    let handoff = build_handoff(state);
    let dir = handoff_dir.join("artifacts").join("handoff");
    fs::create_dir_all(&dir)?;
    let path = dir.join("claim-ledger.json");
    let json = serde_json::to_string_pretty(&handoff)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    core_state_utils::atomic_write::write_atomic_text(&path, &json)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    Ok(path)
}

/// Load a [`HandoffArtifact`] from a JSON file.
pub fn load_handoff(path: &Path) -> std::io::Result<Option<HandoffArtifact>> {
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)?;
    let handoff: HandoffArtifact = serde_json::from_str(&content)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(Some(handoff))
}

/// Load handoff from the standard path under `handoff_dir`.
pub fn load_handoff_default(handoff_dir: &Path) -> std::io::Result<Option<HandoffArtifact>> {
    let path = handoff_dir
        .join("artifacts")
        .join("handoff")
        .join("claim-ledger.json");
    load_handoff(&path)
}

// ── NL Resumption Context ──

/// Generate a natural-language resumption context from a handoff artifact.
pub fn format_resume_context(handoff: &HandoffArtifact) -> String {
    let mut lines = Vec::new();

    lines.push("# Research Session Handoff — Resumption Context\n".into());

    if let Some(ref project) = handoff.project {
        lines.push(format!("**Project**: {}", project));
    }
    if let Some(ref question) = handoff.question {
        lines.push(format!("**Question**: {}", question));
    }
    if let Some(ref mode) = handoff.mode {
        lines.push(format!("**Mode**: {}", mode));
    }
    lines.push(format!("**Created**: {}", handoff.created_at));
    lines.push(String::new());

    lines.push(format!(
        "**Progress**: {}",
        handoff.metadata.progress_summary
    ));
    if let Some(ref ceiling) = handoff.metadata.top_ceiling {
        lines.push(format!("**Top Claim Ceiling**: {}", ceiling));
    }
    if let Some(ref verdict) = handoff.metadata.novelty_verdict {
        lines.push(format!("**Novelty Verdict**: {}", verdict));
    }
    lines.push(String::new());

    if !handoff.entries.is_empty() {
        lines.push("## Claims\n".into());
        for entry in &handoff.entries {
            lines.push(format!(
                "- **{}**: {} [status: {}, ceiling: {}]",
                entry.claim_id,
                entry.claim_text,
                entry.status,
                entry.ceiling.as_deref().unwrap_or("unknown"),
            ));
            if let Some(score) = entry.novelty_score {
                let pct = (score * 100.0) as u32;
                lines.push(format!("  - Novelty score: {}%", pct));
            }
            if let Some(v) = &entry.novelty_verdict {
                lines.push(format!("  - Verdict: {}", v));
            }
            if !entry.evidence.is_empty() {
                lines.push(format!("  - Evidence ({} items):", entry.evidence.len()));
                for ev in &entry.evidence {
                    lines.push(format!(
                        "    - [{}] {} ({})",
                        ev.strength, ev.source, ev.location
                    ));
                }
            }
        }
        lines.push(String::new());
    }

    if !handoff.review_rounds.is_empty() {
        lines.push("## Review Rounds\n".into());
        for rr in &handoff.review_rounds {
            lines.push(format!(
                "- Round {} ({}) — {} ({} findings)",
                rr.round, rr.dimension, rr.verdict, rr.finding_count
            ));
        }
        lines.push(String::new());
    }

    if handoff.entries.is_empty() && handoff.review_rounds.is_empty() {
        lines.push("_No prior claims or review rounds found. Starting fresh._\n".into());
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use serde_json::json;

    #[test]
    fn build_handoff_empty_state() {
        let state = json!({});
        let handoff = build_handoff(&state);
        assert_eq!(handoff.schema_version, 1);
        assert!(handoff.entries.is_empty());
        assert!(handoff.review_rounds.is_empty());
        assert_eq!(handoff.metadata.total_claims, 0);
    }

    #[test]
    fn build_handoff_with_claims() {
        let state = json!({
            "project": "test-project",
            "question": "Is X better than Y?",
            "mode": "deep",
            "novelty_gate": {
                "claims": ["C1: X is better", "C2: Y is faster"],
                "decision": "novel"
            }
        });
        let handoff = build_handoff(&state);
        assert_eq!(handoff.project.as_deref(), Some("test-project"));
        assert_eq!(handoff.metadata.novelty_verdict.as_deref(), Some("novel"));
        assert_eq!(handoff.entries.len(), 2);
    }

    #[test]
    fn build_handoff_with_claim_records() {
        let state = json!({
            "novelty_gate": {
                "claim_records": [{
                    "claim_id": "C1",
                    "claim": "Method X achieves 95% accuracy",
                    "evidence": [{
                        "source": "Table 2",
                        "location": "results section",
                        "strength": "Strong"
                    }],
                    "novelty_score": 0.72,
                    "verdict": "Moderate",
                    "ceiling": "conference-ready",
                    "status": "active"
                }]
            }
        });
        let handoff = build_handoff(&state);
        assert_eq!(handoff.entries.len(), 1);
        let entry = &handoff.entries[0];
        assert_eq!(entry.claim_id, "C1");
        assert!((entry.novelty_score.unwrap() - 0.72).abs() < 0.01);
        assert_eq!(entry.novelty_verdict.as_deref(), Some("Moderate"));
        assert_eq!(entry.evidence.len(), 1);
        assert_eq!(entry.evidence[0].strength, "Strong");
    }

    #[test]
    fn build_handoff_with_review_rounds() {
        let state = json!({
            "review_history": [
                {"round": 1, "dimension": "逻辑与证据", "verdict": "revise", "findings": [{"id": "F1"}]},
                {"round": 2, "dimension": "最近工作与新颖性", "verdict": "accept", "findings": []}
            ]
        });
        let handoff = build_handoff(&state);
        assert_eq!(handoff.review_rounds.len(), 2);
        assert_eq!(handoff.review_rounds[0].round, 1);
        assert_eq!(handoff.review_rounds[1].round, 2);
        assert_eq!(handoff.review_rounds[0].finding_count, 1);
        assert_eq!(handoff.review_rounds[1].finding_count, 0);
    }

    #[test]
    fn build_handoff_falls_back_to_draft_claims() {
        let state = json!({
            "novelty_gate": {
                "draft_claims": [
                    {"claim_id": "d1", "claim": "Draft claim one", "novelty_score": 0.3},
                    {"claim_id": "d2", "claim": "Draft claim two"}
                ]
            }
        });
        let handoff = build_handoff(&state);
        assert_eq!(handoff.entries.len(), 2);
        let e0 = &handoff.entries[0];
        assert_eq!(e0.claim_id, "d1");
        assert!((e0.novelty_score.unwrap() - 0.3).abs() < 0.01);
    }

    #[test]
    fn save_and_load_handoff() {
        let state = json!({
            "project": "handoff-test",
            "novelty_gate": {
                "claims": ["C1: test"]
            }
        });
        let dir = tempfile::TempDir::new().unwrap();
        let path = save_handoff(&state, dir.path()).unwrap();
        assert!(path.exists());
        assert!(path.to_string_lossy().contains("claim-ledger.json"));

        let loaded = load_handoff(&path).unwrap().unwrap();
        assert_eq!(loaded.project.as_deref(), Some("handoff-test"));
        assert_eq!(loaded.entries.len(), 1);
    }

    #[test]
    fn load_handoff_missing_returns_none() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.json");
        let result = load_handoff(&path).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn load_handoff_default_finds_saved() {
        let state = json!({
            "project": "default-path-test",
            "novelty_gate": {"claims": ["C1"]}
        });
        let dir = tempfile::TempDir::new().unwrap();
        save_handoff(&state, dir.path()).unwrap();
        let loaded = load_handoff_default(dir.path()).unwrap().unwrap();
        assert_eq!(loaded.project.as_deref(), Some("default-path-test"));
    }

    #[test]
    fn format_resume_context_with_data() {
        let handoff = HandoffArtifact {
            schema_version: 1,
            created_at: "2026-06-25T12:00:00Z".into(),
            project: Some("audit".into()),
            question: Some("Is X novel?".into()),
            mode: Some("deep".into()),
            entries: vec![HandoffEntry {
                claim_id: "C1".into(),
                claim_text: "X is novel".into(),
                evidence: vec![EvidenceEntry {
                    source: "Table 2".into(),
                    location: "results".into(),
                    strength: "Strong".into(),
                }],
                novelty_score: Some(0.85),
                novelty_verdict: Some("Strong".into()),
                ceiling: Some("top-venue".into()),
                status: "active".into(),
            }],
            review_rounds: vec![ReviewRoundSnapshot {
                round: 1,
                dimension: "逻辑与证据".into(),
                verdict: "accept".into(),
                finding_count: 2,
            }],
            metadata: HandoffMetadata {
                total_claims: 1,
                total_evidence: 1,
                top_ceiling: Some("top-venue".into()),
                novelty_verdict: Some("Strong".into()),
                progress_summary: "1 claims, 1 evidence items, 1 review rounds".into(),
            },
        };
        let ctx = format_resume_context(&handoff);
        assert!(ctx.contains("C1"));
        assert!(ctx.contains("X is novel"));
        assert!(ctx.contains("85%"));
        assert!(ctx.contains("top-venue"));
        assert!(ctx.contains("逻辑与证据"));
    }

    #[test]
    fn format_resume_context_empty() {
        let handoff = HandoffArtifact {
            schema_version: 1,
            created_at: String::new(),
            project: None,
            question: None,
            mode: None,
            entries: vec![],
            review_rounds: vec![],
            metadata: HandoffMetadata {
                total_claims: 0,
                total_evidence: 0,
                top_ceiling: None,
                novelty_verdict: None,
                progress_summary: String::new(),
            },
        };
        let ctx = format_resume_context(&handoff);
        assert!(ctx.contains("Starting fresh"));
    }

    #[test]
    fn build_handoff_round_trip() {
        let state = json!({
            "project": "roundtrip",
            "question": "Q",
            "mode": "quick",
            "novelty_gate": {
                "claims": ["C1: claim A", "C2: claim B"],
                "claim_records": [{
                    "claim_id": "C1",
                    "claim": "claim A",
                    "evidence": [],
                    "novelty_score": 0.5,
                    "verdict": "Moderate",
                    "ceiling": "conference-ready",
                    "status": "active"
                }],
                "decision": "Moderate"
            },
            "review_history": [{
                "round": 1, "dimension": "E", "verdict": "revise",
                "findings": [{"id": "X"}, {"id": "Y"}]
            }]
        });
        let handoff = build_handoff(&state);
        // 3 entries: C1 from claim_records + "C1: claim A" and "C2: claim B" from flat claims list
        assert_eq!(handoff.entries.len(), 3);
        assert_eq!(handoff.review_rounds.len(), 1);

        let json = serde_json::to_string(&handoff).unwrap();
        let back: HandoffArtifact = serde_json::from_str(&json).unwrap();
        assert_eq!(back.project.as_deref(), Some("roundtrip"));
        assert_eq!(back.entries.len(), 3);
    }

    #[test]
    fn entries_claim_records_take_priority() {
        let state = json!({
            "novelty_gate": {
                "claims": ["C1: claim A"],
                "claim_records": [{
                    "claim_id": "C1",
                    "claim": "claim A from record",
                    "evidence": [],
                    "novelty_score": 0.9,
                    "verdict": "Strong",
                    "ceiling": "top-venue",
                    "status": "active"
                }]
            }
        });
        let handoff = build_handoff(&state);
        // 2 entries: C1 from record + "C1: claim A" from claims list (different text)
        assert_eq!(handoff.entries.len(), 2);
        // The claim_records entry should have the score
        let c1 = handoff.entries.iter().find(|e| e.claim_id == "C1").unwrap();
        assert!((c1.novelty_score.unwrap() - 0.9).abs() < 0.01);
    }
}
