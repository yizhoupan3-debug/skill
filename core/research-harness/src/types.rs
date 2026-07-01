//! Core types shared across the research harness.

use serde::{Deserialize, Serialize};
use std::fmt;

// ── Review ──

/// Severity level for review findings: P0=一票否决, A=核心硬伤, B=需补充, C=打磨, Warning=隐晦警告.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Severity {
    /// 一票否决 — data integrity, academic integrity, hard theory errors
    P0,
    /// 核心硬伤 — logic/method/evidence core defects
    A,
    /// 需补充 — missing data, experiments, baselines, statistics
    B,
    /// 隐晦警告 — subtle omissions, undeclared boundaries
    Warning,
    /// 打磨 — prose polish, style, formatting
    C,
}

impl Severity {
    /// Whether this severity blocks convergence (P0, A, B block; Warning, C do not).
    pub fn blocks_convergence(&self) -> bool {
        matches!(self, Severity::P0 | Severity::A | Severity::B)
    }
}

/// A single review finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub severity: Severity,
    pub dimension: String,
    pub location: String,
    pub description: String,
    pub suggestion: Option<String>,
}

/// Verdict from a review round.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReviewVerdict {
    Accept,
    Revise,
    Reject,
}

/// Output of a single review round.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRoundOutput {
    pub round: u64,
    pub dimension: String,
    pub verdict: ReviewVerdict,
    pub findings: Vec<Finding>,
    pub summary: String,
}

// ── Review Dimensions ──

/// Progressive disclosure dimensions for paper review (7 dimensions).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDimension {
    /// R1: Claim ceiling, evidence coverage, ablation, comparison fairness
    LogicAndEvidence,
    /// R2: Closest prior work, novelty positioning, venue calibration
    NoveltyAndPositioning,
    /// R3: Equation closure, symbol uniqueness, derivation gaps
    MathAndNotation,
    /// R4: Figure rendering, caption self-containment, table density
    FiguresAndReadability,
    /// R5: Terminology density, defensive tone, EN slop / ZH 套话
    LanguageAndTone,
    /// R6: Page pressure, hidden evidence, appendix routing
    LengthAndAppendix,
    /// R7+: All dimensions, regression check on previous fixes
    FullRegression,
}

impl ReviewDimension {
    /// Get the dimension for a given round number (1-indexed, wraps at 7+).
    pub fn for_round(round: u64) -> Self {
        match round {
            1 => Self::LogicAndEvidence,
            2 => Self::NoveltyAndPositioning,
            3 => Self::MathAndNotation,
            4 => Self::FiguresAndReadability,
            5 => Self::LanguageAndTone,
            6 => Self::LengthAndAppendix,
            _ => Self::FullRegression,
        }
    }

    /// Human-readable name for prompts.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::LogicAndEvidence => "逻辑与证据",
            Self::NoveltyAndPositioning => "最近工作与新颖性",
            Self::MathAndNotation => "数学与符号",
            Self::FiguresAndReadability => "图表与可读性",
            Self::LanguageAndTone => "语言与防御性",
            Self::LengthAndAppendix => "长度与附录路由",
            Self::FullRegression => "全面重审",
        }
    }
}

// ── Claims ──

/// A single claim in the claim ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    pub id: String,
    pub text: String,
    pub evidence: Vec<EvidenceAnchor>,
    pub ceiling: ClaimCeiling,
}

/// Evidence supporting a claim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceAnchor {
    pub source: String,
    pub location: String,
    pub strength: EvidenceStrength,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvidenceStrength {
    Strong,
    Moderate,
    Weak,
    Missing,
}

/// How far a claim can go (no-claim, local-only, top-venue).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClaimCeiling {
    NoClaim,
    LocalOnly,
    ConferenceReady,
    TopVenue,
}

impl ClaimCeiling {
    /// Returns a numeric rank for ordering (higher = stronger claim).
    pub fn rank(&self) -> u8 {
        match self {
            ClaimCeiling::NoClaim => 0,
            ClaimCeiling::LocalOnly => 1,
            ClaimCeiling::ConferenceReady => 2,
            ClaimCeiling::TopVenue => 3,
        }
    }
}

// ── AIGC ──

/// AIGC detection result for a text segment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AigcDetectionResult {
    pub segment_id: String,
    pub ai_probability: f64, // 0.0 - 1.0
    pub score: u32,          // 0 - 100
    pub signals: Vec<AigcSignal>,
}

/// Individual detection signal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AigcSignal {
    pub signal_type: AigcSignalType,
    pub value: f64,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AigcSignalType {
    NGramAnomaly,
    LowBurstiness,
    SyntacticPattern,
    VocabularyRepetition,
    SentenceLengthUniformity,
}

/// Humanization strategy applied to a text segment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanizeResult {
    pub original: String,
    pub rewritten: String,
    pub strategies_applied: Vec<String>,
    pub estimated_score_improvement: f64,
}

// ── Search ──

/// A paper found through literature search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paper {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub abstract_text: String,
    pub year: Option<u32>,
    pub venue: Option<String>,
    pub doi: Option<String>,
    pub url: Option<String>,
    pub source: PaperSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaperSource {
    SemanticScholar,
    ArXiv,
    PubMed,
    Manual,
}

impl fmt::Display for PaperSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PaperSource::SemanticScholar => write!(f, "semantic_scholar"),
            PaperSource::ArXiv => write!(f, "arxiv"),
            PaperSource::PubMed => write!(f, "pubmed"),
            PaperSource::Manual => write!(f, "manual"),
        }
    }
}

// ── Verification ──

/// Result of a verification check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub check_name: String,
    pub status: VerificationStatus,
    pub details: String,
    pub evidence_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VerificationStatus {
    Pass,
    Fail,
    Warn,
    Skip,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use serde_json;

    #[test]
    fn severity_blocks_convergence() {
        assert!(Severity::P0.blocks_convergence());
        assert!(Severity::A.blocks_convergence());
        assert!(Severity::B.blocks_convergence());
        assert!(!Severity::Warning.blocks_convergence());
        assert!(!Severity::C.blocks_convergence());
    }

    #[test]
    fn review_dimension_for_round() {
        assert_eq!(
            ReviewDimension::for_round(0),
            ReviewDimension::FullRegression
        );
        assert_eq!(
            ReviewDimension::for_round(1),
            ReviewDimension::LogicAndEvidence
        );
        assert_eq!(
            ReviewDimension::for_round(2),
            ReviewDimension::NoveltyAndPositioning
        );
        assert_eq!(
            ReviewDimension::for_round(3),
            ReviewDimension::MathAndNotation
        );
        assert_eq!(
            ReviewDimension::for_round(4),
            ReviewDimension::FiguresAndReadability
        );
        assert_eq!(
            ReviewDimension::for_round(5),
            ReviewDimension::LanguageAndTone
        );
        assert_eq!(
            ReviewDimension::for_round(6),
            ReviewDimension::LengthAndAppendix
        );
        assert_eq!(
            ReviewDimension::for_round(7),
            ReviewDimension::FullRegression
        );
        assert_eq!(
            ReviewDimension::for_round(100),
            ReviewDimension::FullRegression
        );
    }

    #[test]
    fn review_dimension_display_name() {
        assert_eq!(
            ReviewDimension::LogicAndEvidence.display_name(),
            "逻辑与证据"
        );
        assert_eq!(
            ReviewDimension::NoveltyAndPositioning.display_name(),
            "最近工作与新颖性"
        );
        assert_eq!(
            ReviewDimension::MathAndNotation.display_name(),
            "数学与符号"
        );
        assert_eq!(
            ReviewDimension::FiguresAndReadability.display_name(),
            "图表与可读性"
        );
        assert_eq!(
            ReviewDimension::LanguageAndTone.display_name(),
            "语言与防御性"
        );
        assert_eq!(
            ReviewDimension::LengthAndAppendix.display_name(),
            "长度与附录路由"
        );
        assert_eq!(ReviewDimension::FullRegression.display_name(), "全面重审");
    }

    #[test]
    fn severity_serde_roundtrip() {
        for variant in &[
            Severity::P0,
            Severity::A,
            Severity::B,
            Severity::Warning,
            Severity::C,
        ] {
            let json = serde_json::to_value(variant).unwrap();
            let back: Severity = serde_json::from_value(json).unwrap();
            assert_eq!(*variant, back);
        }
    }

    #[test]
    fn review_verdict_serde_roundtrip() {
        for variant in &[
            ReviewVerdict::Accept,
            ReviewVerdict::Revise,
            ReviewVerdict::Reject,
        ] {
            let json = serde_json::to_value(variant).unwrap();
            let back: ReviewVerdict = serde_json::from_value(json).unwrap();
            assert_eq!(*variant, back);
        }
    }

    #[test]
    fn claim_ceiling_serde_kebab() {
        let json = serde_json::to_value(ClaimCeiling::ConferenceReady).unwrap();
        assert_eq!(json, serde_json::json!("conference-ready"));
        let back: ClaimCeiling = serde_json::from_value(json).unwrap();
        assert_eq!(back, ClaimCeiling::ConferenceReady);
    }

    #[test]
    fn evidence_strength_serde_lowercase() {
        let json = serde_json::to_value(EvidenceStrength::Strong).unwrap();
        assert_eq!(json, serde_json::json!("strong"));
    }

    #[test]
    fn claim_default_construction() {
        let c = Claim {
            id: "C1".into(),
            text: "test".into(),
            evidence: vec![],
            ceiling: ClaimCeiling::NoClaim,
        };
        assert_eq!(c.id, "C1");
        assert!(c.evidence.is_empty());
    }

    #[test]
    fn paper_source_display() {
        assert_eq!(PaperSource::SemanticScholar.to_string(), "semantic_scholar");
        assert_eq!(PaperSource::ArXiv.to_string(), "arxiv");
        assert_eq!(PaperSource::PubMed.to_string(), "pubmed");
        assert_eq!(PaperSource::Manual.to_string(), "manual");
    }

    #[test]
    fn aigc_detection_bounds() {
        let result = AigcDetectionResult {
            segment_id: "seg1".into(),
            ai_probability: 1.0,
            score: 100,
            signals: vec![],
        };
        assert_eq!(result.score, 100);
        assert_eq!(result.ai_probability, 1.0);
    }

    #[test]
    fn finding_optional_suggestion() {
        let with = Finding {
            id: "F1".into(),
            severity: Severity::B,
            dimension: "logic".into(),
            location: "sec:3".into(),
            description: "missing baseline".into(),
            suggestion: Some("add baseline".into()),
        };
        assert!(with.suggestion.is_some());

        let without = Finding {
            id: "F2".into(),
            severity: Severity::C,
            dimension: "style".into(),
            location: "sec:5".into(),
            description: "typo".into(),
            suggestion: None,
        };
        assert!(without.suggestion.is_none());
    }

    #[test]
    fn verification_status_serde_roundtrip() {
        for variant in &[
            VerificationStatus::Pass,
            VerificationStatus::Fail,
            VerificationStatus::Warn,
            VerificationStatus::Skip,
        ] {
            let json = serde_json::to_value(variant).unwrap();
            let back: VerificationStatus = serde_json::from_value(json).unwrap();
            assert_eq!(*variant, back);
        }
    }
}
