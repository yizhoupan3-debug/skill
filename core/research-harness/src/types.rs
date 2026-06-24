//! Core types shared across the research harness.

use serde::{Deserialize, Serialize};

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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaperSource {
    SemanticScholar,
    ArXiv,
    PubMed,
    Manual,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VerificationStatus {
    Pass,
    Fail,
    Warn,
    Skip,
}

// ── Convergence ──

/// Convergence state for the review loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvergenceState {
    pub min_rounds: u64,
    pub consecutive_stable_required: u64,
    pub consecutive_stable_count: u64,
    pub max_rounds: u64,
    pub current_round: u64,
}

impl ConvergenceState {
    /// Check if the loop has converged.
    pub fn is_converged(&self) -> bool {
        self.current_round >= self.min_rounds
            && self.consecutive_stable_count >= self.consecutive_stable_required
    }

    /// Check if the loop has hit the hard ceiling.
    pub fn is_at_ceiling(&self) -> bool {
        self.current_round >= self.max_rounds
    }

    /// Record a round result and update stable count.
    pub fn record_round(&mut self, has_blocking_findings: bool) {
        if has_blocking_findings {
            self.consecutive_stable_count = 0;
        } else {
            self.consecutive_stable_count += 1;
        }
        self.current_round += 1;
    }
}
