//! CLI argument definitions for autoresearch-rs.
//!
//! Extracted from main.rs for readability. All types are `pub(crate)` since
//! they're only used within this binary crate.

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use crate::{DEFAULT_EXTERNAL_TIMEOUT_SECS, DEFAULT_RESEARCH_RESULT_LIMIT};

#[derive(Parser)]
#[command(name = "autoresearch-rs")]
#[command(about = "Rust control plane for autoresearch workspaces")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    Init {
        #[arg(long)]
        project: String,
        #[arg(long)]
        question: String,
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long, value_enum, default_value_t = ModeArg::Quick)]
        mode: ModeArg,
    },
    Status {
        #[arg(long)]
        workspace: PathBuf,
    },
    Next {
        #[arg(long)]
        workspace: PathBuf,
    },
    Resume {
        #[arg(long)]
        workspace: PathBuf,
    },
    Sync {
        #[arg(long)]
        workspace: PathBuf,
    },
    DraftClaims {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long)]
        question: Option<String>,
        #[arg(long, default_value_t = 4)]
        count: usize,
    },
    PlanSearch {
        #[arg(long)]
        workspace: PathBuf,
    },
    ResearchClaim {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long = "claim-id")]
        claim_id: Option<String>,
        #[arg(long)]
        query: Option<String>,
        #[arg(long, value_enum, default_value_t = ExternalSourceArg::All)]
        source: ExternalSourceArg,
        #[arg(long, default_value_t = DEFAULT_RESEARCH_RESULT_LIMIT)]
        limit: usize,
        #[arg(long = "timeout-secs", default_value_t = DEFAULT_EXTERNAL_TIMEOUT_SECS)]
        timeout_secs: u64,
    },
    ResearchAll {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long, value_enum, default_value_t = ExternalSourceArg::All)]
        source: ExternalSourceArg,
        #[arg(long, default_value_t = DEFAULT_RESEARCH_RESULT_LIMIT)]
        limit: usize,
        #[arg(long = "max-claims", default_value_t = 3)]
        max_claims: usize,
        #[arg(long = "timeout-secs", default_value_t = DEFAULT_EXTERNAL_TIMEOUT_SECS)]
        timeout_secs: u64,
    },
    GateFromResearch {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long = "min-results", default_value_t = 1)]
        min_results: usize,
        #[arg(long = "apply")]
        apply: bool,
    },
    BriefFirstClaim {
        #[arg(long)]
        workspace: PathBuf,
    },
    CompareClaim {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long)]
        claim: String,
        #[arg(long)]
        axis: String,
        #[arg(long = "closest-prior-work")]
        closest_prior_work: String,
        #[arg(long, value_enum)]
        overlap: OverlapArg,
        #[arg(long)]
        difference: String,
        #[arg(long, value_enum)]
        confidence: ConfidenceArg,
        #[arg(long, value_enum)]
        verdict: VerdictArg,
        #[arg(long = "claim-id")]
        claim_id: Option<String>,
    },
    AddHypothesis {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long)]
        claim: String,
        #[arg(long)]
        prediction: Option<String>,
        #[arg(long)]
        mechanism: Option<String>,
        #[arg(long = "falsifiable-prediction")]
        falsifiable_prediction: Option<String>,
        #[arg(long = "success-threshold")]
        success_threshold: Option<String>,
        #[arg(long = "stop-condition")]
        stop_condition: Option<String>,
        #[arg(long = "baseline")]
        baselines: Vec<String>,
        #[arg(long = "confounder")]
        confounders: Vec<String>,
        #[arg(long = "negative-signal")]
        negative_signals: Vec<String>,
        #[arg(long = "minimal-test")]
        minimal_test: Option<String>,
        #[arg(long, value_enum, default_value_t = PriorityArg::Medium)]
        priority: PriorityArg,
        #[arg(long = "id")]
        id: Option<String>,
    },
    RecordRun {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long = "hypothesis-id")]
        hypothesis_id: String,
        #[arg(long, value_enum)]
        outcome: OutcomeArg,
        #[arg(long)]
        summary: String,
        #[arg(long = "metric-name")]
        metric_name: Option<String>,
        #[arg(long = "metric-value")]
        metric_value: Option<String>,
        #[arg(long = "command")]
        entry_command: Option<String>,
        #[arg(long = "evidence-path")]
        evidence_path: Option<String>,
        #[arg(long = "sanity-check")]
        sanity_checks: Vec<String>,
        #[arg(long = "baseline-result")]
        baseline_result: Option<String>,
        #[arg(long = "rules-in")]
        rules_in: Vec<String>,
        #[arg(long = "rules-out")]
        rules_out: Vec<String>,
        #[arg(long = "alternative-explanation")]
        alternative_explanations: Vec<String>,
        #[arg(long = "threat")]
        threats: Vec<String>,
        #[arg(long = "interpretation")]
        interpretation: Option<String>,
        #[arg(long = "finding")]
        finding: Option<String>,
        #[arg(long = "decision-delta")]
        decision_delta: Option<String>,
        #[arg(long = "reuse-note")]
        reuse_note: Option<String>,
        #[arg(long = "applies-to")]
        applies_to: Vec<String>,
        #[arg(long = "does-not-apply-to")]
        does_not_apply_to: Vec<String>,
        #[arg(long = "override-novelty-gate")]
        override_novelty_gate: bool,
        #[arg(long = "override-reason")]
        override_reason: Option<String>,
    },
    AnnotateRun {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long = "run-id")]
        run_id: String,
        #[arg(long = "finding")]
        finding: Option<String>,
        #[arg(long = "decision-delta")]
        decision_delta: Option<String>,
        #[arg(long = "reuse-note")]
        reuse_note: Option<String>,
        #[arg(long = "applies-to")]
        applies_to: Vec<String>,
        #[arg(long = "does-not-apply-to")]
        does_not_apply_to: Vec<String>,
    },
    AuditReuse {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long = "apply")]
        apply: bool,
    },
    Reflect {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long = "hypothesis-id")]
        hypothesis_id: String,
        #[arg(long, value_enum)]
        direction: DirectionArg,
        #[arg(long)]
        reason: String,
        #[arg(long = "next-step")]
        next_step: Option<String>,
        #[arg(long = "activate-hypothesis")]
        activate_hypothesis: Option<String>,
    },
    SetNoveltyGate {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long, value_enum)]
        status: GateStatusArg,
        #[arg(long)]
        decision: Option<String>,
        #[arg(long = "overlap-summary")]
        overlap_summary: Option<String>,
        #[arg(long = "differentiation-strategy")]
        differentiation_strategy: Option<String>,
        #[arg(long = "claim")]
        claims: Vec<String>,
    },
    /// Loop barrier escalation: init workspace → research → BARRIER_REPORT.json
    Barrier {
        /// The hard barrier problem description (from loop runner or manual)
        #[arg(long)]
        problem: String,
        /// Optional workspace dir (created if not exists)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// Loop ID context (from loop runner)
        #[arg(long = "loop-id")]
        loop_id: Option<String>,
        /// Run ID context (from loop runner)
        #[arg(long = "run-id")]
        run_id: Option<String>,
        /// Action ID context (from loop runner)
        #[arg(long = "action-id")]
        action_id: Option<String>,
        /// Consecutive failure count (from loop runner)
        #[arg(long = "consecutive-failures", default_value_t = 0)]
        consecutive_failures: u32,
    },
    /// Record a structured research log entry (text layer + SQLite FTS5)
    LogRecord {
        #[arg(long)]
        workspace: PathBuf,
        /// Research direction / project name
        #[arg(long)]
        direction: String,
        /// Research question or problem
        #[arg(long)]
        question: String,
        /// Entry point: manual | barrier_escalation | loop
        #[arg(long, default_value = "manual")]
        entry_point: String,
        /// Barrier ID if escalation-triggered
        #[arg(long)]
        barrier_id: Option<String>,
    },
    /// Full-text search across all research logs
    LogSearch {
        #[arg(long)]
        workspace: PathBuf,
        /// FTS5 query string
        #[arg(long)]
        query: String,
        /// Max results
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Add an insight to an existing log entry
    LogInsight {
        #[arg(long)]
        workspace: PathBuf,
        /// Log entry ID (UUID)
        #[arg(long = "log-id")]
        log_id: String,
        /// Insight text
        #[arg(long)]
        text: String,
        /// Confidence: high | medium | low
        #[arg(long, default_value = "medium")]
        confidence: String,
    },
    /// Connect two log entries (cross-reference)
    LogConnect {
        #[arg(long)]
        workspace: PathBuf,
        /// First log entry ID
        #[arg(long = "log-id-a")]
        log_id_a: String,
        /// Second log entry ID
        #[arg(long = "log-id-b")]
        log_id_b: String,
        /// Relationship description
        #[arg(long)]
        relation: Option<String>,
    },
    /// Show neighbors of a research log entry
    LogNeighbors {
        #[arg(long)]
        workspace: PathBuf,
        /// Entry ID
        #[arg(long = "entry-id")]
        entry_id: String,
        /// Relation filter: extends,contradicts,supports,supersedes
        #[arg(long)]
        relation: Option<String>,
        /// Max results
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// Visualize research knowledge graph
    LogViz {
        #[arg(long)]
        workspace: PathBuf,
        /// Center entry ID (full graph if omitted)
        #[arg(long = "entry-id")]
        entry_id: Option<String>,
        /// Max depth
        #[arg(long, default_value_t = 2)]
        max_depth: usize,
        /// Output format: text | dot
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Trace research path from a barrier
    LogRoute {
        #[arg(long)]
        workspace: PathBuf,
        /// Barrier ID
        #[arg(long = "barrier-id")]
        barrier_id: String,
        /// Max depth
        #[arg(long, default_value_t = 3)]
        max_depth: usize,
    },
    /// Auto-extract entities from a log entry
    LogExtract {
        #[arg(long)]
        workspace: PathBuf,
        /// Log entry ID
        #[arg(long = "entry-id")]
        entry_id: String,
    },
    /// Search entities via FTS5
    LogSearchEntities {
        #[arg(long)]
        workspace: PathBuf,
        /// FTS5 query string
        #[arg(long)]
        query: String,
        /// Max results
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
}

#[derive(Clone, ValueEnum)]
pub(crate) enum ModeArg {
    Quick,
    Full,
}

#[derive(Clone, ValueEnum)]
pub(crate) enum PriorityArg {
    High,
    Medium,
    Low,
}

#[derive(Clone, ValueEnum)]
pub(crate) enum OutcomeArg {
    Confirmatory,
    Exploratory,
    Failed,
    Ambiguous,
}

#[derive(Clone, ValueEnum)]
pub(crate) enum DirectionArg {
    #[value(alias = "DEEPEN")]
    Deepen,
    #[value(alias = "BROADEN")]
    Broaden,
    #[value(alias = "PIVOT")]
    Pivot,
    #[value(alias = "CONCLUDE")]
    Conclude,
}

#[derive(Clone, ValueEnum)]
pub(crate) enum GateStatusArg {
    Pending,
    Passed,
    Pivot,
}

#[derive(Clone, ValueEnum)]
pub(crate) enum ExternalSourceArg {
    All,
    #[value(name = "semantic-scholar")]
    SemanticScholar,
    Arxiv,
}

#[derive(Clone, ValueEnum)]
pub(crate) enum OverlapArg {
    Low,
    Medium,
    High,
}

#[derive(Clone, ValueEnum)]
pub(crate) enum ConfidenceArg {
    Low,
    Medium,
    High,
}

#[derive(Clone, ValueEnum)]
pub(crate) enum VerdictArg {
    Novel,
    Defensible,
    Risky,
    #[value(name = "not-novel")]
    NotNovel,
}
