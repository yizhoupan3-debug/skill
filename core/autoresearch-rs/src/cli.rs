use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use crate::constants::*;

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

impl ModeArg {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            ModeArg::Quick => "quick",
            ModeArg::Full => "full",
        }
    }
}

impl PriorityArg {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            PriorityArg::High => "high",
            PriorityArg::Medium => "medium",
            PriorityArg::Low => "low",
        }
    }
}

impl OutcomeArg {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            OutcomeArg::Confirmatory => "confirmatory",
            OutcomeArg::Exploratory => "exploratory",
            OutcomeArg::Failed => "failed",
            OutcomeArg::Ambiguous => "ambiguous",
        }
    }
}

impl DirectionArg {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            DirectionArg::Deepen => "DEEPEN",
            DirectionArg::Broaden => "BROADEN",
            DirectionArg::Pivot => "PIVOT",
            DirectionArg::Conclude => "CONCLUDE",
        }
    }
}

impl GateStatusArg {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            GateStatusArg::Pending => "pending",
            GateStatusArg::Passed => "passed",
            GateStatusArg::Pivot => "pivot",
        }
    }
}

impl ExternalSourceArg {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            ExternalSourceArg::All => "all",
            ExternalSourceArg::SemanticScholar => "semantic-scholar",
            ExternalSourceArg::Arxiv => "arxiv",
        }
    }
}

impl OverlapArg {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            OverlapArg::Low => "low",
            OverlapArg::Medium => "medium",
            OverlapArg::High => "high",
        }
    }
}

impl ConfidenceArg {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            ConfidenceArg::Low => "low",
            ConfidenceArg::Medium => "medium",
            ConfidenceArg::High => "high",
        }
    }
}

impl VerdictArg {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            VerdictArg::Novel => "novel",
            VerdictArg::Defensible => "defensible",
            VerdictArg::Risky => "risky",
            VerdictArg::NotNovel => "not-novel",
        }
    }
}
