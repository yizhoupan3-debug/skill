use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Local, Utc};
use clap::{Parser, Subcommand, ValueEnum};
use regex::Regex;
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, USER_AGENT};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use uuid::Uuid;

const SCHEMA_VERSION: i64 = 3;
const STAGE_BOOTSTRAP: &str = "bootstrap";
const STAGE_INNER_LOOP: &str = "inner-loop";
const STAGE_OUTER_LOOP: &str = "outer-loop";
const STAGE_FINALIZE: &str = "finalize";
const STALE_STATE_DAYS: i64 = 10;
const RECENT_ACTIVITY_DAYS: i64 = 14;
const FALLBACK_ACTIVITY_LIMIT: usize = 3;
const TEMPLATES_DIR: &str = "skills/autoresearch/templates";
const DEFAULT_RESEARCH_RESULT_LIMIT: usize = 5;
const DEFAULT_EXTERNAL_TIMEOUT_SECS: u64 = 20;
const SEMANTIC_SCHOLAR_BASE_URL: &str = "https://api.semanticscholar.org/graph/v1/paper/search";
const ARXIV_BASE_URL: &str = "https://export.arxiv.org/api/query";

const FINDINGS_BLOCK_START: &str = "<!-- autoresearch:findings:start -->";
const FINDINGS_BLOCK_END: &str = "<!-- autoresearch:findings:end -->";
const NOVELTY_BLOCK_START: &str = "<!-- autoresearch:novelty:start -->";
const NOVELTY_BLOCK_END: &str = "<!-- autoresearch:novelty:end -->";
const SEARCH_PLAN_BLOCK_START: &str = "<!-- autoresearch:search-plan:start -->";
const SEARCH_PLAN_BLOCK_END: &str = "<!-- autoresearch:search-plan:end -->";
const EXTERNAL_RESEARCH_BLOCK_START: &str = "<!-- autoresearch:external-research:start -->";
const EXTERNAL_RESEARCH_BLOCK_END: &str = "<!-- autoresearch:external-research:end -->";
const CLAIMS_BLOCK_START: &str = "<!-- autoresearch:claims:start -->";
const CLAIMS_BLOCK_END: &str = "<!-- autoresearch:claims:end -->";
const CONTEXT_BLOCK_START: &str = "<!-- autoresearch:context:start -->";
const CONTEXT_BLOCK_END: &str = "<!-- autoresearch:context:end -->";

#[derive(Parser)]
#[command(name = "autoresearch-rs")]
#[command(about = "Rust control plane for autoresearch workspaces")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
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
        #[arg(long = "override-novelty-gate")]
        override_novelty_gate: bool,
        #[arg(long = "override-reason")]
        override_reason: Option<String>,
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
enum ModeArg {
    Quick,
    Full,
}

#[derive(Clone, ValueEnum)]
enum PriorityArg {
    High,
    Medium,
    Low,
}

#[derive(Clone, ValueEnum)]
enum OutcomeArg {
    Confirmatory,
    Exploratory,
    Failed,
    Ambiguous,
}

#[derive(Clone, ValueEnum)]
enum DirectionArg {
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
enum GateStatusArg {
    Pending,
    Passed,
    Pivot,
}

#[derive(Clone, ValueEnum)]
enum ExternalSourceArg {
    All,
    #[value(name = "semantic-scholar")]
    SemanticScholar,
    Arxiv,
}

#[derive(Clone, ValueEnum)]
enum OverlapArg {
    Low,
    Medium,
    High,
}

#[derive(Clone, ValueEnum)]
enum ConfidenceArg {
    Low,
    Medium,
    High,
}

#[derive(Clone, ValueEnum)]
enum VerdictArg {
    Novel,
    Defensible,
    Risky,
    #[value(name = "not-novel")]
    NotNovel,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init {
            project,
            question,
            dir,
            mode,
        } => {
            let root = init_workspace(&project, &question, &dir, mode.as_str())?;
            append_ledger_event(
                &root,
                "workspace.initialized",
                json!({ "project": project, "question": question, "mode": mode.as_str() }),
            )?;
            println!("Initialized autoresearch workspace at {}", root.display());
        }
        Commands::Status { workspace } => {
            let (workspace, state_path) = ensure_workspace(&workspace)?;
            let mut state = load_state(&state_path)?;
            set_key(
                &mut state,
                "environment",
                capture_environment_fingerprint(&workspace),
            );
            set_key(&mut state, "git", capture_git_provenance(&workspace));
            println!("{}", format_status(&state));
        }
        Commands::Next { workspace } => {
            let (_, state_path) = ensure_workspace(&workspace)?;
            let state = load_state(&state_path)?;
            for action in recommend_next_actions(&state) {
                println!("- {action}");
            }
        }
        Commands::Resume { workspace } => {
            let (workspace, state_path) = ensure_workspace(&workspace)?;
            let mut state = load_state(&state_path)?;
            set_key(
                &mut state,
                "environment",
                capture_environment_fingerprint(&workspace),
            );
            set_key(&mut state, "git", capture_git_provenance(&workspace));
            println!("{}", format_resume(&state));
        }
        Commands::Sync { workspace } => {
            let (workspace, state_path) = ensure_workspace(&workspace)?;
            let state = load_state(&state_path)?;
            sync_workspace_files(&workspace, &state)?;
            append_ledger_event(
                &workspace,
                "workspace.synced",
                json!({ "runs": arr(&state, "run_history").len() }),
            )?;
            println!("Synchronized workspace files for {}", workspace.display());
        }
        Commands::DraftClaims {
            workspace,
            question,
            count,
        } => {
            let (workspace, state_path) = ensure_workspace(&workspace)?;
            let state = load_state(&state_path)?;
            let updated = draft_claims_from_state(&state, question.as_deref(), count);
            dump_state(&state_path, &updated)?;
            sync_workspace_files(&workspace, &updated)?;
            append_research_log(
                &workspace,
                "Draft claims generated",
                vec![
                    format!("claims: {}", novelty_arr(&updated, "draft_claims").len()),
                    format!(
                        "question: {}",
                        question.unwrap_or_else(|| str_key(&updated, "question"))
                    ),
                ],
            )?;
            append_ledger_event(
                &workspace,
                "novelty_gate.draft_claims",
                json!({ "count": novelty_arr(&updated, "draft_claims").len() }),
            )?;
            println!("Generated draft claims for {}", workspace.display());
        }
        Commands::PlanSearch { workspace } => {
            let (workspace, state_path) = ensure_workspace(&workspace)?;
            let state = load_state(&state_path)?;
            let updated = ensure_state_defaults(&state);
            dump_state(&state_path, &updated)?;
            sync_workspace_files(&workspace, &updated)?;
            append_research_log(
                &workspace,
                "Novelty search view refreshed",
                vec![
                    format!("entries: {}", current_search_plan(&updated).len()),
                    "source priority: Semantic Scholar -> arXiv -> Google Scholar".to_string(),
                ],
            )?;
            append_ledger_event(
                &workspace,
                "novelty_gate.search_plan_refreshed",
                json!({ "entries": current_search_plan(&updated).len() }),
            )?;
            println!("Refreshed novelty search plan for {}", workspace.display());
        }
        Commands::ResearchClaim {
            workspace,
            claim_id,
            query,
            source,
            limit,
            timeout_secs,
        } => {
            let (workspace, state_path) = ensure_workspace(&workspace)?;
            let state = load_state(&state_path)?;
            let research = research_claim(
                &state,
                claim_id.as_deref(),
                query.as_deref(),
                &source,
                limit,
                timeout_secs,
            )?;
            let updated = add_external_research(&state, research);
            dump_state(&state_path, &updated)?;
            sync_workspace_files(&workspace, &updated)?;
            if let Some(entry) = latest_external_research(&updated) {
                append_research_log(
                    &workspace,
                    &format!(
                        "External research recorded ({})",
                        str_field(entry, "research_id")
                    ),
                    vec![
                        format!("claim: {}", str_field_default(entry, "claim_id", "custom")),
                        format!("query: {}", str_field(entry, "query")),
                        format!("results: {}", external_research_result_count(entry)),
                    ],
                )?;
                append_ledger_event(
                    &workspace,
                    "external_research.recorded",
                    json!({
                        "research_id": entry.get("research_id").cloned().unwrap_or(Value::Null),
                        "claim_id": entry.get("claim_id").cloned().unwrap_or(Value::Null),
                        "query": entry.get("query").cloned().unwrap_or(Value::Null),
                        "source": entry.get("source").cloned().unwrap_or(Value::Null),
                        "results": external_research_result_count(entry),
                    }),
                )?;
            }
            println!("Recorded external research for {}", workspace.display());
        }
        Commands::BriefFirstClaim { workspace } => {
            let (workspace, state_path) = ensure_workspace(&workspace)?;
            let state = load_state(&state_path)?;
            let updated = ensure_state_defaults(&state);
            dump_state(&state_path, &updated)?;
            sync_workspace_files(&workspace, &updated)?;
            let brief = current_brief(&updated);
            append_research_log(
                &workspace,
                "Novelty brief refreshed",
                vec![
                    format!(
                        "claim: {}",
                        brief
                            .as_ref()
                            .and_then(|item| item.get("claim_id"))
                            .and_then(Value::as_str)
                            .unwrap_or("_not set_")
                    ),
                    "scope: top-priority novelty claim".to_string(),
                ],
            )?;
            append_ledger_event(
                &workspace,
                "novelty_gate.brief_refreshed",
                json!({ "claim_id": brief.and_then(|item| item.get("claim_id").cloned()) }),
            )?;
            println!("Refreshed novelty brief for {}", workspace.display());
        }
        Commands::CompareClaim {
            workspace,
            claim,
            axis,
            closest_prior_work,
            overlap,
            difference,
            confidence,
            verdict,
            claim_id,
        } => {
            let (workspace, state_path) = ensure_workspace(&workspace)?;
            let state = load_state(&state_path)?;
            let updated = add_claim_comparison(
                &state,
                &claim,
                &axis,
                &closest_prior_work,
                overlap.as_str(),
                &difference,
                confidence.as_str(),
                verdict.as_str(),
                claim_id.as_deref(),
            );
            dump_state(&state_path, &updated)?;
            sync_workspace_files(&workspace, &updated)?;
            append_research_log(
                &workspace,
                &format!(
                    "Novelty claim compared ({})",
                    claim_id.as_deref().unwrap_or("auto")
                ),
                vec![
                    format!("claim: {claim}"),
                    format!("overlap: {}", overlap.as_str()),
                    format!("verdict: {}", verdict.as_str()),
                ],
            )?;
            append_ledger_event(
                &workspace,
                "novelty_gate.updated",
                json!({
                    "claim_id": claim_id,
                    "claim": claim,
                    "overlap": overlap.as_str(),
                    "verdict": verdict.as_str(),
                }),
            )?;
            println!(
                "Recorded novelty claim comparison for {}",
                workspace.display()
            );
        }
        Commands::AddHypothesis {
            workspace,
            claim,
            prediction,
            priority,
            id,
        } => {
            let (workspace, state_path) = ensure_workspace(&workspace)?;
            let state = load_state(&state_path)?;
            let updated = add_hypothesis(
                &state,
                &claim,
                prediction.as_deref(),
                priority.as_str(),
                id.as_deref(),
            )?;
            dump_state(&state_path, &updated)?;
            sync_workspace_files(&workspace, &updated)?;
            let resolved_id = id.unwrap_or_else(|| slugify(&claim).chars().take(40).collect());
            if let Some(hypothesis) = find_hypothesis(&updated, &resolved_id) {
                append_research_log(
                    &workspace,
                    &format!("Hypothesis added ({resolved_id})"),
                    vec![
                        format!("claim: {}", str_field(hypothesis, "claim")),
                        format!("priority: {}", str_field(hypothesis, "priority")),
                    ],
                )?;
                append_ledger_event(
                    &workspace,
                    "hypothesis.added",
                    json!({
                        "hypothesis_id": resolved_id,
                        "status": hypothesis.get("status").cloned().unwrap_or(Value::Null),
                        "priority": hypothesis.get("priority").cloned().unwrap_or(Value::Null),
                    }),
                )?;
            }
            println!("Added hypothesis in {}", workspace.display());
        }
        Commands::RecordRun {
            workspace,
            hypothesis_id,
            outcome,
            summary,
            metric_name,
            metric_value,
            entry_command,
            evidence_path,
            override_novelty_gate,
            override_reason,
        } => {
            let (workspace, state_path) = ensure_workspace(&workspace)?;
            let state = load_state(&state_path)?;
            let updated = record_run(
                &state,
                &hypothesis_id,
                outcome.as_str(),
                &summary,
                metric_name.as_deref(),
                metric_value.as_deref(),
                entry_command.as_deref(),
                evidence_path.as_deref(),
                override_novelty_gate,
                override_reason.as_deref(),
                &workspace,
            )?;
            dump_state(&state_path, &updated)?;
            sync_workspace_files(&workspace, &updated)?;
            if let Some(record) = latest_run_for_hypothesis(&updated, &hypothesis_id) {
                append_research_log(
                    &workspace,
                    &format!("Run recorded ({})", str_field(record, "run_id")),
                    vec![
                        format!("hypothesis: {}", str_field(record, "hypothesis_id")),
                        format!("outcome: {}", str_field(record, "outcome")),
                        format!("summary: {}", str_field(record, "summary")),
                    ],
                )?;
                append_ledger_event(
                    &workspace,
                    "run.recorded",
                    json!({
                        "run_id": record.get("run_id").cloned().unwrap_or(Value::Null),
                        "hypothesis_id": record.get("hypothesis_id").cloned().unwrap_or(Value::Null),
                        "outcome": record.get("outcome").cloned().unwrap_or(Value::Null),
                        "metric_name": record.get("metric_name").cloned().unwrap_or(Value::Null),
                        "metric_value": record.get("metric_value").cloned().unwrap_or(Value::Null),
                        "command": record.get("command").cloned().unwrap_or(Value::Null),
                        "evidence_path": record.get("evidence_path").cloned().unwrap_or(Value::Null),
                        "novelty_gate_status_at_run": record.get("novelty_gate_status_at_run").cloned().unwrap_or(Value::Null),
                        "novelty_gate_override": record.get("novelty_gate_override").cloned().unwrap_or(Value::Null),
                        "override_reason": record.get("override_reason").cloned().unwrap_or(Value::Null),
                        "environment_fingerprint": record.get("environment_fingerprint").cloned().unwrap_or(Value::Null),
                        "git_provenance": record.get("git_provenance").cloned().unwrap_or(Value::Null),
                    }),
                )?;
            }
            if let Some(hypothesis) = find_hypothesis(&updated, &hypothesis_id) {
                append_ledger_event(
                    &workspace,
                    "hypothesis.status_changed",
                    json!({
                        "hypothesis_id": hypothesis_id,
                        "status": hypothesis.get("status").cloned().unwrap_or(Value::Null),
                        "reason": hypothesis.get("status_reason").cloned().unwrap_or(Value::Null),
                    }),
                )?;
            }
            println!("Recorded run for {hypothesis_id}");
        }
        Commands::Reflect {
            workspace,
            hypothesis_id,
            direction,
            reason,
            next_step,
            activate_hypothesis,
        } => {
            let (workspace, state_path) = ensure_workspace(&workspace)?;
            let state = load_state(&state_path)?;
            let updated = reflect(
                &state,
                &hypothesis_id,
                direction.as_str(),
                &reason,
                next_step.as_deref(),
                activate_hypothesis.as_deref(),
            )?;
            dump_state(&state_path, &updated)?;
            sync_workspace_files(&workspace, &updated)?;
            if let Some(decision) = latest_decision_for_hypothesis(&updated, &hypothesis_id) {
                append_research_log(
                    &workspace,
                    &format!(
                        "Reflection recorded ({})",
                        str_field_default(decision, "run_id", "no-run")
                    ),
                    vec![
                        format!("hypothesis: {}", str_field(decision, "hypothesis_id")),
                        format!("direction: {}", str_field(decision, "direction")),
                        format!("reason: {}", str_field(decision, "reason")),
                    ],
                )?;
                append_ledger_event(
                    &workspace,
                    "reflection.recorded",
                    json!({
                        "hypothesis_id": decision.get("hypothesis_id").cloned().unwrap_or(Value::Null),
                        "run_id": decision.get("run_id").cloned().unwrap_or(Value::Null),
                        "direction": decision.get("direction").cloned().unwrap_or(Value::Null),
                        "reason": decision.get("reason").cloned().unwrap_or(Value::Null),
                    }),
                )?;
            }
            if let Some(hypothesis) = find_hypothesis(&updated, &hypothesis_id) {
                append_ledger_event(
                    &workspace,
                    "hypothesis.status_changed",
                    json!({
                        "hypothesis_id": hypothesis_id,
                        "status": hypothesis.get("status").cloned().unwrap_or(Value::Null),
                        "reason": hypothesis.get("status_reason").cloned().unwrap_or(Value::Null),
                    }),
                )?;
            }
            println!("Recorded reflection for {hypothesis_id}");
        }
        Commands::SetNoveltyGate {
            workspace,
            status,
            decision,
            overlap_summary,
            differentiation_strategy,
            claims,
        } => {
            let (workspace, state_path) = ensure_workspace(&workspace)?;
            let mut state = load_state(&state_path)?;
            let gate = novelty_gate_mut(&mut state);
            gate.insert("status".to_string(), json!(status.as_str()));
            if let Some(decision) = decision {
                gate.insert("decision".to_string(), json!(decision));
            }
            if let Some(overlap_summary) = overlap_summary {
                gate.insert("overlap_summary".to_string(), json!(overlap_summary));
            }
            if let Some(strategy) = differentiation_strategy {
                gate.insert("differentiation_strategy".to_string(), json!(strategy));
            }
            if !claims.is_empty() {
                gate.insert("claims".to_string(), json!(claims));
            }
            dump_state(&state_path, &state)?;
            sync_workspace_files(&workspace, &state)?;
            append_research_log(
                &workspace,
                "Novelty gate updated",
                vec![
                    format!("status: {}", novelty_str(&state, "status", "pending")),
                    format!("decision: {}", novelty_str(&state, "decision", "_not set_")),
                ],
            )?;
            append_ledger_event(
                &workspace,
                "novelty_gate.updated",
                json!({
                    "status": novelty_str(&state, "status", "pending"),
                    "decision": novelty_value(&state, "decision"),
                }),
            )?;
            println!("Updated novelty gate for {}", workspace.display());
        }
    }
    Ok(())
}

impl ModeArg {
    fn as_str(&self) -> &'static str {
        match self {
            ModeArg::Quick => "quick",
            ModeArg::Full => "full",
        }
    }
}

impl PriorityArg {
    fn as_str(&self) -> &'static str {
        match self {
            PriorityArg::High => "high",
            PriorityArg::Medium => "medium",
            PriorityArg::Low => "low",
        }
    }
}

impl OutcomeArg {
    fn as_str(&self) -> &'static str {
        match self {
            OutcomeArg::Confirmatory => "confirmatory",
            OutcomeArg::Exploratory => "exploratory",
            OutcomeArg::Failed => "failed",
            OutcomeArg::Ambiguous => "ambiguous",
        }
    }
}

impl DirectionArg {
    fn as_str(&self) -> &'static str {
        match self {
            DirectionArg::Deepen => "DEEPEN",
            DirectionArg::Broaden => "BROADEN",
            DirectionArg::Pivot => "PIVOT",
            DirectionArg::Conclude => "CONCLUDE",
        }
    }
}

impl GateStatusArg {
    fn as_str(&self) -> &'static str {
        match self {
            GateStatusArg::Pending => "pending",
            GateStatusArg::Passed => "passed",
            GateStatusArg::Pivot => "pivot",
        }
    }
}

impl ExternalSourceArg {
    fn as_str(&self) -> &'static str {
        match self {
            ExternalSourceArg::All => "all",
            ExternalSourceArg::SemanticScholar => "semantic-scholar",
            ExternalSourceArg::Arxiv => "arxiv",
        }
    }
}

impl OverlapArg {
    fn as_str(&self) -> &'static str {
        match self {
            OverlapArg::Low => "low",
            OverlapArg::Medium => "medium",
            OverlapArg::High => "high",
        }
    }
}

impl ConfidenceArg {
    fn as_str(&self) -> &'static str {
        match self {
            ConfidenceArg::Low => "low",
            ConfidenceArg::Medium => "medium",
            ConfidenceArg::High => "high",
        }
    }
}

impl VerdictArg {
    fn as_str(&self) -> &'static str {
        match self {
            VerdictArg::Novel => "novel",
            VerdictArg::Defensible => "defensible",
            VerdictArg::Risky => "risky",
            VerdictArg::NotNovel => "not-novel",
        }
    }
}

fn now_iso() -> String {
    Utc::now().with_nanosecond_zero().to_rfc3339()
}

trait NanosecondZero {
    fn with_nanosecond_zero(self) -> Self;
}

impl NanosecondZero for DateTime<Utc> {
    fn with_nanosecond_zero(self) -> Self {
        self.with_nanosecond(0).unwrap_or(self)
    }
}

use chrono::Timelike;

fn parse_iso_timestamp(value: &str) -> Option<DateTime<Utc>> {
    if value.trim().is_empty() {
        return None;
    }
    DateTime::parse_from_rfc3339(&value.replace('Z', "+00:00"))
        .ok()
        .map(|ts| ts.with_timezone(&Utc))
}

fn days_since(value: &str) -> Option<i64> {
    parse_iso_timestamp(value).map(|ts| (Utc::now() - ts).num_days().max(0))
}

fn repo_root() -> Result<PathBuf> {
    if let Ok(root) = std::env::var("CARGO_MANIFEST_DIR") {
        return Ok(PathBuf::from(root)
            .parent()
            .and_then(Path::parent)
            .unwrap_or(Path::new("."))
            .to_path_buf());
    }
    let current = std::env::current_dir()?;
    for candidate in current.ancestors() {
        if candidate.join("AGENT.md").exists() && candidate.join("skills").exists() {
            return Ok(candidate.to_path_buf());
        }
    }
    Ok(current)
}

fn template_path(name: &str) -> Result<PathBuf> {
    Ok(repo_root()?.join(TEMPLATES_DIR).join(name))
}

fn load_template(name: &str) -> Result<String> {
    let path = template_path(name)?;
    fs::read_to_string(&path).with_context(|| format!("Missing template: {}", path.display()))
}

fn replace_placeholders(template: &str, pairs: &[(&str, &str)]) -> String {
    let mut rendered = template.to_string();
    for (key, value) in pairs {
        rendered = rendered.replace(&format!("{{{key}}}"), value);
    }
    rendered
}

fn slugify(text: &str) -> String {
    let lowered = text.trim().to_lowercase();
    let cleaned = Regex::new(r"[^a-z0-9]+")
        .unwrap()
        .replace_all(&lowered, "-")
        .to_string();
    let collapsed = Regex::new(r"-+")
        .unwrap()
        .replace_all(&cleaned, "-")
        .trim_matches('-')
        .to_string();
    if collapsed.is_empty() {
        "hypothesis".to_string()
    } else {
        collapsed
    }
}

fn obj_mut(value: &mut Value) -> &mut Map<String, Value> {
    value.as_object_mut().expect("state must be an object")
}

fn arr<'a>(value: &'a Value, key: &str) -> &'a Vec<Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .expect("expected array after defaults")
}

fn arr_mut<'a>(value: &'a mut Value, key: &str) -> &'a mut Vec<Value> {
    obj_mut(value)
        .entry(key.to_string())
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .expect("expected array")
}

fn str_key(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("-")
        .to_string()
}

fn str_field(value: &Value, key: &str) -> String {
    str_field_default(value, key, "-")
}

fn str_field_default(value: &Value, key: &str, default: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

fn set_key(value: &mut Value, key: &str, child: Value) {
    obj_mut(value).insert(key.to_string(), child);
}

fn novelty_gate(value: &Value) -> &Map<String, Value> {
    value
        .get("novelty_gate")
        .and_then(Value::as_object)
        .expect("novelty gate defaults must exist")
}

fn novelty_gate_mut(value: &mut Value) -> &mut Map<String, Value> {
    obj_mut(value)
        .entry("novelty_gate".to_string())
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .expect("novelty_gate must be object")
}

fn novelty_arr<'a>(value: &'a Value, key: &str) -> &'a Vec<Value> {
    novelty_gate(value)
        .get(key)
        .and_then(Value::as_array)
        .expect("novelty array default missing")
}

fn novelty_value(value: &Value, key: &str) -> Value {
    novelty_gate(value).get(key).cloned().unwrap_or(Value::Null)
}

fn novelty_str(value: &Value, key: &str, default: &str) -> String {
    novelty_gate(value)
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

fn default_state(project: &str, question: &str, mode: &str) -> Value {
    let timestamp = now_iso();
    let mut state = json!({
        "schema_version": SCHEMA_VERSION,
        "project": project,
        "question": question,
        "mode": mode,
        "status": "active",
        "stage": STAGE_BOOTSTRAP,
        "current_direction": Value::Null,
        "active_hypothesis": Value::Null,
        "novelty_gate": {
            "status": "pending",
            "claims": [],
            "claim_records": [],
            "draft_claims": [],
            "overlap_summary": Value::Null,
            "differentiation_strategy": Value::Null,
            "decision": Value::Null
        },
        "hypotheses": [],
        "hypothesis_backlog": [],
        "run_history": [],
        "external_research": [],
        "evidence_index": [],
        "blockers": [],
        "decisions": [],
        "environment": Value::Null,
        "git": Value::Null,
        "next_actions": [],
        "created_at": timestamp,
        "updated_at": timestamp
    });
    let actions = recommend_next_actions(&state);
    set_key(&mut state, "next_actions", json!(actions));
    state
}

fn ensure_state_defaults(state: &Value) -> Value {
    let mut hydrated = state.clone();
    {
        let root = obj_mut(&mut hydrated);
        root.entry("schema_version")
            .or_insert(json!(SCHEMA_VERSION));
        root.entry("status").or_insert(json!("active"));
        root.entry("stage").or_insert(json!(STAGE_BOOTSTRAP));
        root.entry("mode").or_insert(json!("quick"));
        root.entry("current_direction").or_insert(Value::Null);
        root.entry("active_hypothesis").or_insert(Value::Null);
        root.entry("hypotheses").or_insert(json!([]));
        root.entry("hypothesis_backlog").or_insert(json!([]));
        root.entry("run_history").or_insert(json!([]));
        root.entry("external_research").or_insert(json!([]));
        root.entry("evidence_index").or_insert(json!([]));
        root.entry("blockers").or_insert(json!([]));
        root.entry("decisions").or_insert(json!([]));
        root.entry("environment").or_insert(Value::Null);
        root.entry("git").or_insert(Value::Null);
        root.entry("next_actions").or_insert(json!([]));
        let created_at = root
            .entry("created_at")
            .or_insert_with(|| json!(now_iso()))
            .clone();
        root.entry("updated_at").or_insert(created_at);
    }
    {
        let gate = novelty_gate_mut(&mut hydrated);
        gate.entry("status").or_insert(json!("pending"));
        gate.entry("claims").or_insert(json!([]));
        gate.entry("claim_records").or_insert(json!([]));
        gate.entry("draft_claims").or_insert(json!([]));
        gate.entry("overlap_summary").or_insert(Value::Null);
        gate.entry("differentiation_strategy")
            .or_insert(Value::Null);
        gate.entry("decision").or_insert(Value::Null);
    }
    let updated_at = str_key(&hydrated, "updated_at");
    for hypothesis in arr_mut(&mut hydrated, "hypotheses") {
        let item = hypothesis
            .as_object_mut()
            .expect("hypothesis must be object");
        let status = item
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("queued")
            .to_string();
        let valid = [
            "queued",
            "active",
            "needs_reflection",
            "parked",
            "concluded",
        ];
        if !valid.contains(&status.as_str()) {
            item.insert("status".into(), json!("queued"));
        } else {
            item.entry("status").or_insert(json!(status));
        }
        item.entry("status_reason").or_insert(Value::Null);
        let status_updated_at = item
            .get("created_at")
            .cloned()
            .unwrap_or_else(|| json!(updated_at.clone()));
        item.entry("status_updated_at").or_insert(status_updated_at);
    }
    for record in arr_mut(&mut hydrated, "run_history") {
        let item = record.as_object_mut().expect("run record must be object");
        item.entry("novelty_gate_status_at_run")
            .or_insert(Value::Null);
        item.entry("novelty_gate_override").or_insert(json!(false));
        item.entry("override_reason").or_insert(Value::Null);
        item.entry("environment_fingerprint").or_insert(Value::Null);
        item.entry("git_provenance").or_insert(Value::Null);
    }
    for record in arr_mut(&mut hydrated, "external_research") {
        let item = record
            .as_object_mut()
            .expect("external research record must be object");
        item.entry("claim_id").or_insert(Value::Null);
        item.entry("source").or_insert(json!("all"));
        item.entry("results").or_insert(json!([]));
        item.entry("errors").or_insert(json!([]));
        item.entry("created_at")
            .or_insert_with(|| json!(updated_at.clone()));
    }
    set_key(&mut hydrated, "schema_version", json!(SCHEMA_VERSION));
    hydrated
}

fn load_state(path: &Path) -> Result<Value> {
    let raw = fs::read_to_string(path)?;
    let data: Value = serde_yaml::from_str(&raw)
        .or_else(|_| serde_json::from_str(&raw))
        .with_context(|| format!("State file must be YAML/JSON: {}", path.display()))?;
    if !data.is_object() {
        bail!("State file must be a mapping: {}", path.display());
    }
    Ok(ensure_state_defaults(&migrate_state(&data)))
}

fn migrate_state(state: &Value) -> Value {
    let mut migrated = state.clone();
    let version = migrated
        .get("schema_version")
        .and_then(Value::as_i64)
        .unwrap_or(2);
    if version >= SCHEMA_VERSION {
        return migrated;
    }
    let run_history = migrated
        .get("run_history")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let decisions = migrated
        .get("decisions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let updated_at = migrated
        .get("updated_at")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    for hypothesis in arr_mut(&mut migrated, "hypotheses") {
        let hypothesis_id = str_field(hypothesis, "id");
        let mut status = hypothesis
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("queued")
            .to_string();
        if ![
            "queued",
            "active",
            "needs_reflection",
            "parked",
            "concluded",
        ]
        .contains(&status.as_str())
        {
            status = "queued".to_string();
        }
        let latest_run = run_history.iter().rev().find(|item| {
            item.get("hypothesis_id").and_then(Value::as_str) == Some(hypothesis_id.as_str())
        });
        let latest_decision = decisions.iter().rev().find(|item| {
            item.get("hypothesis_id").and_then(Value::as_str) == Some(hypothesis_id.as_str())
        });
        if status == "active"
            && latest_run.is_some()
            && (latest_decision.is_none()
                || latest_decision.and_then(|item| item.get("run_id"))
                    != latest_run.and_then(|item| item.get("run_id")))
        {
            status = "needs_reflection".to_string();
        }
        let item = hypothesis.as_object_mut().unwrap();
        item.insert("status".into(), json!(status));
        item.entry("status_reason").or_insert(Value::Null);
        let status_updated_at = item
            .get("created_at")
            .cloned()
            .unwrap_or_else(|| json!(updated_at.clone()));
        item.entry("status_updated_at").or_insert(status_updated_at);
    }
    for record in arr_mut(&mut migrated, "run_history") {
        let item = record.as_object_mut().unwrap();
        item.entry("novelty_gate_status_at_run")
            .or_insert(Value::Null);
        item.entry("novelty_gate_override").or_insert(json!(false));
        item.entry("override_reason").or_insert(Value::Null);
        item.entry("environment_fingerprint").or_insert(Value::Null);
        item.entry("git_provenance").or_insert(Value::Null);
    }
    obj_mut(&mut migrated)
        .entry("external_research")
        .or_insert(json!([]));
    set_key(&mut migrated, "schema_version", json!(SCHEMA_VERSION));
    migrated
}

fn dump_state(path: &Path, state: &Value) -> Result<()> {
    let mut state_to_write = ensure_state_defaults(state);
    refresh_novelty_views(&mut state_to_write);
    set_key(&mut state_to_write, "schema_version", json!(SCHEMA_VERSION));
    set_key(&mut state_to_write, "updated_at", json!(now_iso()));
    let actions = recommend_next_actions(&state_to_write);
    set_key(&mut state_to_write, "next_actions", json!(actions));
    let rendered = serde_yaml::to_string(&state_to_write)?;
    fs::write(path, rendered)?;
    Ok(())
}

fn resolve_workspace(path: &Path) -> Result<PathBuf> {
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let candidate = candidate.canonicalize().unwrap_or(candidate);
    if candidate.is_file() {
        if candidate.file_name().and_then(|name| name.to_str()) != Some("research-state.yaml") {
            bail!("Workspace path must be a project directory or research-state.yaml");
        }
        Ok(candidate.parent().unwrap_or(Path::new(".")).to_path_buf())
    } else {
        Ok(candidate)
    }
}

fn ensure_workspace(path: &Path) -> Result<(PathBuf, PathBuf)> {
    let workspace = resolve_workspace(path)?;
    let state_path = workspace.join("research-state.yaml");
    if !state_path.exists() {
        bail!("Missing state file: {}", state_path.display());
    }
    Ok((workspace, state_path))
}

fn init_workspace(project: &str, question: &str, base_dir: &Path, mode: &str) -> Result<PathBuf> {
    let base = if base_dir.is_absolute() {
        base_dir.to_path_buf()
    } else {
        std::env::current_dir()?.join(base_dir)
    };
    let root = base.join(project);
    for directory in [
        root.clone(),
        root.join("literature"),
        root.join("src"),
        root.join("data"),
        root.join("experiments"),
        root.join("experiments/_templates"),
        root.join("to_human"),
        root.join("paper"),
    ] {
        fs::create_dir_all(directory)?;
    }
    let state_path = root.join("research-state.yaml");
    if state_path.exists() {
        bail!(
            "Refusing to overwrite existing workspace: {}",
            root.display()
        );
    }
    let state = default_state(project, question, mode);
    dump_state(&state_path, &state)?;
    let date = Local::now().format("%Y-%m-%d").to_string();
    fs::write(
        root.join("research-log.md"),
        replace_placeholders(
            &load_template("research-log.md")?,
            &[
                ("project", project),
                ("question", question),
                ("date", &date),
            ],
        ),
    )?;
    fs::write(
        root.join("findings.md"),
        replace_placeholders(
            &load_template("findings.md")?,
            &[("project", project), ("question", question)],
        ),
    )?;
    fs::write(
        root.join("BOOTSTRAP_BRIEF.md"),
        replace_placeholders(
            &load_template("bootstrap-brief.md")?,
            &[("project", project), ("question", question)],
        ),
    )?;
    fs::write(
        root.join("literature/NOVELTY_GATE.md"),
        replace_placeholders(
            &load_template("novelty-gate.md")?,
            &[("project", project), ("question", question)],
        ),
    )?;
    fs::write(
        root.join("experiments/README.md"),
        replace_placeholders(
            &load_template("experiments-readme.md")?,
            &[("project", project)],
        ),
    )?;
    for (output_name, template_name) in [
        ("HYPOTHESIS_CARD.md", "hypothesis-card.md"),
        ("PROTOCOL_TEMPLATE.md", "protocol-template.md"),
        ("RUN_RECORD_TEMPLATE.md", "run-record-template.md"),
        ("REFLECTION_TEMPLATE.md", "reflection-template.md"),
    ] {
        fs::write(
            root.join("experiments/_templates").join(output_name),
            load_template(template_name)?,
        )?;
    }
    sync_workspace_files(&root, &state)?;
    Ok(root)
}

fn command_output(args: &[&str], cwd: &Path) -> Option<String> {
    let (program, rest) = args.split_first()?;
    let output = Command::new(program)
        .args(rest)
        .current_dir(cwd)
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

fn capture_environment_fingerprint(workspace: &Path) -> Value {
    json!({
        "rust_version": command_output(&["rustc", "--version"], workspace).unwrap_or_else(|| "unknown".to_string()),
        "platform": std::env::consts::OS,
        "machine": std::env::consts::ARCH,
        "yaml_available": true,
        "external_research_http": true,
        "workspace": workspace.display().to_string(),
    })
}

fn capture_git_provenance(workspace: &Path) -> Value {
    let head = command_output(&["git", "rev-parse", "HEAD"], workspace);
    if head.is_none() {
        return json!({
            "available": false,
            "workspace": workspace.display().to_string(),
            "head": Value::Null,
            "branch": Value::Null,
            "dirty": Value::Null,
            "tracked_changes": 0,
            "untracked_changes": 0,
        });
    }
    let inherited = fs::read_to_string(workspace.join("research-state.yaml"))
        .ok()
        .and_then(|raw| serde_yaml::from_str::<Value>(&raw).ok())
        .and_then(|state| state.get("git").cloned())
        .filter(|git| {
            git.get("available")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        });
    if let Some(inherited) = inherited {
        return inherited;
    }
    let branch = command_output(&["git", "rev-parse", "--abbrev-ref", "HEAD"], workspace);
    let status = command_output(&["git", "status", "--porcelain"], workspace).unwrap_or_default();
    let mut tracked_changes = 0;
    let mut untracked_changes = 0;
    let mut dirty = false;
    for line in status.lines().filter(|line| !line.trim().is_empty()) {
        dirty = true;
        if line.starts_with("??") {
            untracked_changes += 1;
        } else {
            tracked_changes += 1;
        }
    }
    json!({
        "available": true,
        "workspace": workspace.display().to_string(),
        "head": head,
        "branch": branch,
        "dirty": dirty,
        "tracked_changes": tracked_changes,
        "untracked_changes": untracked_changes,
    })
}

fn summarize_environment_fingerprint(fingerprint: Option<&Value>) -> String {
    let Some(fingerprint) = fingerprint else {
        return "rust=- platform=- machine=-".to_string();
    };
    let runtime_version = fingerprint
        .get("rust_version")
        .and_then(Value::as_str)
        .unwrap_or("-");
    format!(
        "rust={} platform={} machine={}",
        runtime_version,
        fingerprint
            .get("platform")
            .and_then(Value::as_str)
            .unwrap_or("-"),
        fingerprint
            .get("machine")
            .and_then(Value::as_str)
            .unwrap_or("-")
    )
}

fn summarize_git_provenance(provenance: Option<&Value>) -> String {
    let Some(provenance) = provenance else {
        return "unavailable".to_string();
    };
    if !provenance
        .get("available")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return "unavailable".to_string();
    }
    let head = provenance
        .get("head")
        .and_then(Value::as_str)
        .unwrap_or("-");
    let short_head = head.chars().take(7).collect::<String>();
    let branch = provenance
        .get("branch")
        .and_then(Value::as_str)
        .unwrap_or("-");
    let dirty = if provenance
        .get("dirty")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        "dirty"
    } else {
        "clean"
    };
    format!(
        "{} {} {} tracked={} untracked={}",
        short_head,
        branch,
        dirty,
        provenance
            .get("tracked_changes")
            .and_then(Value::as_i64)
            .unwrap_or(0),
        provenance
            .get("untracked_changes")
            .and_then(Value::as_i64)
            .unwrap_or(0)
    )
}

fn append_ledger_event(workspace: &Path, kind: &str, payload: Value) -> Result<()> {
    let event = json!({
        "schema_version": "autoresearch-ledger-v1",
        "event_id": format!("evt_{}", Uuid::new_v4().simple().to_string().chars().take(12).collect::<String>()),
        "ts": now_iso(),
        "kind": kind,
        "workspace": workspace.display().to_string(),
        "project": workspace.file_name().and_then(|n| n.to_str()).unwrap_or("-"),
        "payload": payload,
    });
    let target = workspace.join("run-ledger.jsonl");
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut handle = OpenOptions::new().create(true).append(true).open(target)?;
    writeln!(handle, "{}", serde_json::to_string(&event)?)?;
    Ok(())
}

fn append_research_log(workspace: &Path, heading: &str, bullets: Vec<String>) -> Result<()> {
    let log_path = workspace.join("research-log.md");
    let mut lines = vec![
        String::new(),
        format!("## {} — {}", Local::now().format("%Y-%m-%d"), heading),
        String::new(),
    ];
    for bullet in bullets {
        lines.push(format!("- {bullet}"));
    }
    lines.push(String::new());
    let mut handle = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    write!(handle, "{}", lines.join("\n"))?;
    Ok(())
}

fn stopwords() -> HashSet<&'static str> {
    [
        "a", "an", "and", "are", "as", "at", "be", "by", "can", "for", "from", "in", "into", "is",
        "it", "of", "on", "or", "reduce", "research", "that", "the", "this", "to", "use", "using",
        "with",
    ]
    .into_iter()
    .collect()
}

fn compact_words(text: &str, limit: usize) -> Vec<String> {
    let re = Regex::new(r"[A-Za-z0-9][A-Za-z0-9_-]*").unwrap();
    let stops = stopwords();
    let mut filtered = Vec::new();
    for cap in re.find_iter(&text.to_lowercase()) {
        let word = cap.as_str();
        if word.len() <= 2 || stops.contains(word) {
            continue;
        }
        if !filtered.iter().any(|item| item == word) {
            filtered.push(word.to_string());
        }
        if filtered.len() >= limit {
            break;
        }
    }
    filtered
}

fn default_required_evidence(axis: &str) -> Vec<String> {
    let axis_lower = axis.to_lowercase();
    if axis_lower.contains("method") || axis_lower.contains("workflow") {
        return vec![
            "Direct overlap papers using the same mechanism".into(),
            "Nearest baseline implementations or orchestration frameworks".into(),
            "Claims about what is structurally different".into(),
        ];
    }
    if axis_lower.contains("setting")
        || axis_lower.contains("domain")
        || axis_lower.contains("task")
    {
        return vec![
            "Prior work in the same domain or task".into(),
            "Recent competitors in the last 3 years".into(),
            "Evidence that the constraint or setting is materially different".into(),
        ];
    }
    if axis_lower.contains("combination") {
        return vec![
            "Papers combining the same building blocks".into(),
            "Closest papers combining two of the three components".into(),
            "Evidence that the composition order or objective is different".into(),
        ];
    }
    vec![
        "Closest prior work for the same core claim".into(),
        "Recent competitors from the last 3 years".into(),
        "Evidence for the exact differentiation sentence".into(),
    ]
}

fn build_search_queries(claim: &str, axis: &str) -> Vec<Value> {
    let keywords = compact_words(claim, 6);
    let broad_terms = if keywords.is_empty() {
        claim.to_string()
    } else {
        keywords
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let focused_terms = if keywords.is_empty() {
        claim.to_string()
    } else {
        keywords
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let combination_terms = if keywords.len() >= 4 {
        vec![
            keywords[0].clone(),
            keywords[1].clone(),
            keywords[keywords.len() - 2].clone(),
            keywords[keywords.len() - 1].clone(),
        ]
        .join(" ")
    } else {
        focused_terms.clone()
    };
    let axis_hint = if axis.trim().is_empty() {
        "claim".to_string()
    } else {
        axis.trim().to_lowercase()
    };
    vec![
        json!({"label": "broad", "query": broad_terms}),
        json!({"label": "focused", "query": format!("{focused_terms} {axis_hint}").trim().to_string()}),
        json!({"label": "recent", "query": format!("{focused_terms} last 3 years").trim().to_string()}),
        json!({"label": "combination", "query": combination_terms}),
    ]
}

fn axis_weights(axis: &str) -> (i64, i64, i64) {
    match axis {
        "method" => (5, 2, 3),
        "workflow" => (5, 2, 3),
        "task" => (4, 3, 4),
        "comparison" => (4, 1, 5),
        "setting" => (3, 4, 2),
        "framing" => (2, 1, 4),
        _ => (3, 2, 3),
    }
}

fn score_claim_priority(record: &Value) -> Value {
    let axis = str_field_default(record, "axis", "claim").to_lowercase();
    let (mut novelty, mut cost, mut reviewer) = axis_weights(&axis);
    match record.get("overlap").and_then(Value::as_str) {
        Some("low") => novelty += 2,
        Some("medium") => novelty += 1,
        Some("high") => {
            novelty -= 1;
            reviewer += 1;
        }
        _ => {}
    }
    match record.get("confidence").and_then(Value::as_str) {
        Some("high") => cost -= 1,
        Some("low") => cost += 1,
        _ => {}
    }
    match record.get("verdict").and_then(Value::as_str) {
        Some("novel") => novelty += 2,
        Some("defensible") => novelty += 1,
        Some("risky") => {
            reviewer += 1;
            cost += 1;
        }
        Some("not-novel") => {
            novelty -= 2;
            cost += 1;
        }
        _ => {}
    }
    let specificity = str_field(record, "specificity").to_lowercase();
    if specificity.contains("testable") {
        cost -= 1;
    }
    if specificity.contains("paper-facing") {
        reviewer += 1;
    }
    let score = novelty * 3 + reviewer * 2 - cost * 2;
    let label = if score >= 18 {
        "first"
    } else if score >= 13 {
        "next"
    } else {
        "later"
    };
    let reason = if novelty >= reviewer && cost <= 2 {
        "high novelty upside with relatively cheap verification"
    } else if reviewer >= novelty && cost <= 3 {
        "reviewer pressure is high, so checking this early reduces risk"
    } else if cost >= 4 {
        "potentially useful, but verification is expensive"
    } else {
        "worth checking, but not the best first search target"
    };
    let mut out = record.clone();
    let map = out.as_object_mut().expect("claim record must be object");
    map.insert("priority_score".into(), json!(score));
    map.insert("priority_label".into(), json!(label));
    map.insert("priority_reason".into(), json!(reason));
    out
}

fn prioritize_claims(claims: &[Value]) -> Vec<Value> {
    let mut scored: Vec<Value> = claims.iter().map(score_claim_priority).collect();
    scored.sort_by(|a, b| {
        let score_a = a.get("priority_score").and_then(Value::as_i64).unwrap_or(0);
        let score_b = b.get("priority_score").and_then(Value::as_i64).unwrap_or(0);
        score_b
            .cmp(&score_a)
            .then_with(|| str_field(a, "claim_id").cmp(&str_field(b, "claim_id")))
    });
    for (index, item) in scored.iter_mut().enumerate() {
        item.as_object_mut()
            .unwrap()
            .insert("recommended_order".into(), json!(index + 1));
    }
    scored
}

fn top_priority_claim(state: &Value) -> Option<Value> {
    for key in ["claim_records", "draft_claims"] {
        let entries = novelty_gate(state)
            .get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if entries.is_empty() {
            continue;
        }
        let mut ranked = entries;
        ranked.sort_by(|a, b| {
            let order_a = a
                .get("recommended_order")
                .and_then(Value::as_i64)
                .unwrap_or(999);
            let order_b = b
                .get("recommended_order")
                .and_then(Value::as_i64)
                .unwrap_or(999);
            let score_a = a.get("priority_score").and_then(Value::as_i64).unwrap_or(0);
            let score_b = b.get("priority_score").and_then(Value::as_i64).unwrap_or(0);
            order_a
                .cmp(&order_b)
                .then_with(|| score_b.cmp(&score_a))
                .then_with(|| str_field(a, "claim_id").cmp(&str_field(b, "claim_id")))
        });
        return ranked.into_iter().next();
    }
    None
}

fn current_recommended_focus(state: &Value) -> Option<String> {
    let claim = top_priority_claim(state)?;
    Some(format!(
        "{}: {}",
        str_field_default(&claim, "claim_id", "C?"),
        str_field_default(&claim, "claim", "_No claim recorded._")
    ))
}

fn build_search_plan_entry(record: &Value) -> Value {
    let claim = str_field(record, "claim");
    let axis = str_field_default(record, "axis", "claim");
    json!({
        "claim_id": str_field_default(record, "claim_id", "C?"),
        "claim": claim,
        "axis": axis,
        "priority_score": record.get("priority_score").cloned().unwrap_or(Value::Null),
        "priority_label": record.get("priority_label").cloned().unwrap_or(Value::Null),
        "priority_reason": record.get("priority_reason").cloned().unwrap_or(Value::Null),
        "recommended_order": record.get("recommended_order").cloned().unwrap_or(Value::Null),
        "keywords": compact_words(&claim, 6),
        "queries": build_search_queries(&claim, &axis),
        "sources": ["Semantic Scholar", "arXiv", "Google Scholar"],
        "required_evidence": default_required_evidence(&axis),
    })
}

fn current_search_plan(state: &Value) -> Vec<Value> {
    let source_records = if !novelty_arr(state, "claim_records").is_empty() {
        novelty_arr(state, "claim_records").clone()
    } else {
        novelty_arr(state, "draft_claims").clone()
    };
    let mut plan: Vec<Value> = source_records.iter().map(build_search_plan_entry).collect();
    plan.sort_by(|a, b| {
        let order_a = a
            .get("recommended_order")
            .and_then(Value::as_i64)
            .unwrap_or(999);
        let order_b = b
            .get("recommended_order")
            .and_then(Value::as_i64)
            .unwrap_or(999);
        let score_a = a.get("priority_score").and_then(Value::as_i64).unwrap_or(0);
        let score_b = b.get("priority_score").and_then(Value::as_i64).unwrap_or(0);
        order_a
            .cmp(&order_b)
            .then_with(|| score_b.cmp(&score_a))
            .then_with(|| str_field(a, "claim_id").cmp(&str_field(b, "claim_id")))
    });
    plan
}

fn refresh_novelty_views(state: &mut Value) {
    let search_plan = current_search_plan(state);
    let recommended_focus = current_recommended_focus(state);
    let brief = current_brief(state);
    let gate = novelty_gate_mut(state);
    gate.insert("search_plan".into(), json!(search_plan));
    gate.insert(
        "recommended_focus".into(),
        recommended_focus.map_or(Value::Null, Value::String),
    );
    gate.insert("brief".into(), brief.unwrap_or(Value::Null));
}

fn expected_baselines_for_axis(axis: &str) -> Vec<String> {
    match axis.to_lowercase().as_str() {
        "method" | "workflow" => vec![
            "Closest simple baseline implementation".into(),
            "Nearest orchestration or workflow framework baseline".into(),
            "A stripped-down version without the claimed mechanism".into(),
        ],
        "task" => vec![
            "Closest task-specific prior method".into(),
            "Simple transfer baseline without the claimed novelty".into(),
            "Recent strongest competitor from the last 3 years".into(),
        ],
        "setting" => vec![
            "Same method in an adjacent setting".into(),
            "Simple baseline in the same constraint".into(),
            "Closest unconstrained baseline to show what the setting changes".into(),
        ],
        "comparison" => vec![
            "Closest simple baseline the reviewer will ask about first".into(),
            "A stronger but obvious comparator".into(),
            "An ablated version removing the claimed differentiator".into(),
        ],
        "framing" => vec![
            "Closest paper making a similar framing claim".into(),
            "A simpler framing that could explain the same result".into(),
            "The baseline narrative a reviewer would default to".into(),
        ],
        _ => vec![
            "Closest prior work the reviewer will expect".into(),
            "A simple baseline explanation".into(),
            "The strongest recent competitor in the same area".into(),
        ],
    }
}

fn verification_standard_for_priority(label: &str) -> &'static str {
    match label {
        "first" => "You should be able to decide proceed vs reframe after one focused search pass.",
        "next" => "This should be checked after the first claim is clarified, not before.",
        _ => "Useful later, but not strong enough to spend the first search budget on.",
    }
}

fn current_brief(state: &Value) -> Option<Value> {
    let top = top_priority_claim(state)?;
    let plan = current_search_plan(state);
    let matching = plan
        .iter()
        .find(|entry| entry.get("claim_id") == top.get("claim_id"));
    let axis = str_field_default(&top, "axis", "claim");
    Some(json!({
        "claim_id": str_field_default(&top, "claim_id", "C?"),
        "claim": str_field_default(&top, "claim", "_No claim recorded._"),
        "axis": axis,
        "priority_label": str_field_default(&top, "priority_label", "later"),
        "priority_score": top.get("priority_score").cloned().unwrap_or(json!(0)),
        "priority_reason": str_field_default(&top, "priority_reason", "_No reason recorded._"),
        "decision_goal": "Decide whether this claim is safe to keep, should be reframed, or should be dropped.",
        "verification_standard": verification_standard_for_priority(&str_field_default(&top, "priority_label", "later")),
        "sources": matching.and_then(|item| item.get("sources")).cloned().unwrap_or(json!(["Semantic Scholar", "arXiv", "Google Scholar"])),
        "queries": matching.and_then(|item| item.get("queries")).cloned().unwrap_or_else(|| json!(build_search_queries(&str_field(&top, "claim"), &axis))),
        "required_evidence": matching.and_then(|item| item.get("required_evidence")).cloned().unwrap_or_else(|| json!(default_required_evidence(&axis))),
        "expected_baselines": expected_baselines_for_axis(&axis),
    }))
}

fn normalize_limit(limit: usize) -> usize {
    limit.clamp(1, 20)
}

fn xml_text_between(raw: &str, tag: &str) -> Option<String> {
    let pattern = Regex::new(&format!(r"(?s)<{tag}(?:\s[^>]*)?>(.*?)</{tag}>")).ok()?;
    let captures = pattern.captures(raw)?;
    Some(decode_xml_entities(captures.get(1)?.as_str().trim()))
}

fn decode_xml_entities(raw: &str) -> String {
    raw.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn markdown_link(value: Option<&str>) -> String {
    value
        .filter(|item| !item.trim().is_empty())
        .map(|item| format!("[link]({})", item.trim()))
        .unwrap_or_else(|| "-".into())
}

fn http_client(timeout_secs: u64) -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(timeout_secs.clamp(3, 120)))
        .build()
        .context("failed to build HTTP client")
}

fn fetch_semantic_scholar(client: &Client, query: &str, limit: usize) -> Result<Vec<Value>> {
    let response: Value = client
        .get(SEMANTIC_SCHOLAR_BASE_URL)
        .header(USER_AGENT, "autoresearch-rs/0.1")
        .header(ACCEPT, "application/json")
        .query(&[
            ("query", query),
            (
                "fields",
                "title,authors,year,venue,url,abstract,citationCount,externalIds",
            ),
            ("limit", &normalize_limit(limit).to_string()),
        ])
        .send()
        .context("Semantic Scholar request failed")?
        .error_for_status()
        .context("Semantic Scholar returned an error")?
        .json()
        .context("Semantic Scholar returned invalid JSON")?;
    let mut results = Vec::new();
    for item in response
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    {
        let authors = item
            .get("authors")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(|author| author.get("name").and_then(Value::as_str))
                    .take(4)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        results.push(json!({
            "source": "Semantic Scholar",
            "title": str_field_default(&item, "title", "_untitled_"),
            "authors": authors,
            "year": item.get("year").cloned().unwrap_or(Value::Null),
            "venue": item.get("venue").cloned().unwrap_or(Value::Null),
            "url": item.get("url").cloned().unwrap_or(Value::Null),
            "abstract": item.get("abstract").cloned().unwrap_or(Value::Null),
            "citation_count": item.get("citationCount").cloned().unwrap_or(Value::Null),
            "external_ids": item.get("externalIds").cloned().unwrap_or(Value::Null),
        }));
    }
    Ok(results)
}

fn fetch_arxiv(client: &Client, query: &str, limit: usize) -> Result<Vec<Value>> {
    let raw = client
        .get(ARXIV_BASE_URL)
        .header(USER_AGENT, "autoresearch-rs/0.1")
        .query(&[
            ("search_query", format!("all:{query}")),
            ("start", "0".to_string()),
            ("max_results", normalize_limit(limit).to_string()),
            ("sortBy", "relevance".to_string()),
            ("sortOrder", "descending".to_string()),
        ])
        .send()
        .context("arXiv request failed")?
        .error_for_status()
        .context("arXiv returned an error")?
        .text()
        .context("arXiv returned invalid text")?;
    let entry_re = Regex::new(r"(?s)<entry>(.*?)</entry>").unwrap();
    let author_re = Regex::new(r"(?s)<author>.*?<name>(.*?)</name>.*?</author>").unwrap();
    let mut results = Vec::new();
    for entry in entry_re.captures_iter(&raw) {
        let entry_raw = entry.get(1).map(|item| item.as_str()).unwrap_or("");
        let authors = author_re
            .captures_iter(entry_raw)
            .filter_map(|cap| cap.get(1).map(|item| decode_xml_entities(item.as_str())))
            .take(4)
            .collect::<Vec<_>>()
            .join(", ");
        results.push(json!({
            "source": "arXiv",
            "title": xml_text_between(entry_raw, "title").unwrap_or_else(|| "_untitled_".into()),
            "authors": authors,
            "year": xml_text_between(entry_raw, "published").map(|date| date.chars().take(4).collect::<String>()).unwrap_or_default(),
            "venue": "arXiv",
            "url": xml_text_between(entry_raw, "id").unwrap_or_default(),
            "abstract": xml_text_between(entry_raw, "summary").unwrap_or_default(),
            "citation_count": Value::Null,
            "external_ids": Value::Null,
        }));
    }
    Ok(results)
}

fn dedupe_research_results(results: Vec<Value>) -> Vec<Value> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for result in results {
        let key = format!(
            "{}::{}",
            str_field_default(&result, "source", "-").to_lowercase(),
            str_field_default(&result, "title", "-").to_lowercase()
        );
        if seen.insert(key) {
            deduped.push(result);
        }
    }
    deduped
}

fn claim_record_for_research(state: &Value, claim_id: Option<&str>) -> Option<Value> {
    if let Some(desired) = claim_id {
        for key in ["claim_records", "draft_claims"] {
            if let Some(record) = novelty_gate(state)
                .get(key)
                .and_then(Value::as_array)
                .and_then(|items| {
                    items
                        .iter()
                        .find(|item| item.get("claim_id").and_then(Value::as_str) == Some(desired))
                })
            {
                return Some(record.clone());
            }
        }
    }
    top_priority_claim(state)
}

fn default_research_query(record: Option<&Value>, explicit_query: Option<&str>) -> Result<String> {
    if let Some(query) = explicit_query
        .map(str::trim)
        .filter(|query| !query.is_empty())
    {
        return Ok(query.to_string());
    }
    if let Some(record) = record {
        if let Some(query) = build_search_queries(
            &str_field_default(record, "claim", ""),
            &str_field_default(record, "axis", "claim"),
        )
        .into_iter()
        .find(|item| item.get("label").and_then(Value::as_str) == Some("focused"))
        .and_then(|item| {
            item.get("query")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        }) {
            return Ok(query);
        }
    }
    bail!("No query available. Run draft-claims first or pass --query.");
}

fn research_claim(
    state: &Value,
    claim_id: Option<&str>,
    explicit_query: Option<&str>,
    source: &ExternalSourceArg,
    limit: usize,
    timeout_secs: u64,
) -> Result<Value> {
    let source_record = claim_record_for_research(state, claim_id);
    if claim_id.is_some() && source_record.is_none() {
        bail!("Unknown claim id: {}", claim_id.unwrap());
    }
    let query = default_research_query(source_record.as_ref(), explicit_query)?;
    let client = http_client(timeout_secs)?;
    let mut results = Vec::new();
    let mut errors = Vec::new();
    if matches!(
        source,
        ExternalSourceArg::All | ExternalSourceArg::SemanticScholar
    ) {
        match fetch_semantic_scholar(&client, &query, limit) {
            Ok(items) => results.extend(items),
            Err(err) => errors.push(format!("semantic-scholar: {err}")),
        }
    }
    if matches!(source, ExternalSourceArg::All | ExternalSourceArg::Arxiv) {
        match fetch_arxiv(&client, &query, limit) {
            Ok(items) => results.extend(items),
            Err(err) => errors.push(format!("arxiv: {err}")),
        }
    }
    if results.is_empty() && !errors.is_empty() {
        bail!("External research failed: {}", errors.join("; "));
    }
    Ok(json!({
        "research_id": format!("ext-{}", Uuid::new_v4().simple().to_string().chars().take(8).collect::<String>()),
        "claim_id": source_record
            .as_ref()
            .and_then(|item| item.get("claim_id"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        "claim": source_record
            .as_ref()
            .map(|item| str_field_default(item, "claim", "-"))
            .unwrap_or_else(|| explicit_query.unwrap_or("-").to_string()),
        "query": query,
        "source": source.as_str(),
        "results": dedupe_research_results(results),
        "errors": errors,
        "created_at": now_iso(),
    }))
}

fn add_external_research(state: &Value, research: Value) -> Value {
    let mut next_state = ensure_state_defaults(state);
    arr_mut(&mut next_state, "external_research").push(research);
    next_state
}

fn latest_external_research(state: &Value) -> Option<&Value> {
    arr(state, "external_research").last()
}

fn external_research_result_count(entry: &Value) -> usize {
    entry
        .get("results")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

fn cleanup_question_text(question: &str) -> String {
    let trimmed = question.trim().trim_end_matches(['?', '.', '!']);
    let re = Regex::new(
        r"(?i)^(can|could|does|do|did|is|are|should|would|will|how|why|what|whether)\s+",
    )
    .unwrap();
    let cleaned = re.replace(trimmed, "").trim().to_string();
    if cleaned.is_empty() {
        question.trim().to_string()
    } else {
        cleaned
    }
}

fn extract_question_parts(question: &str) -> (String, String, String) {
    let cleaned = cleanup_question_text(question);
    let lowered = cleaned.to_lowercase();
    let mut focus = cleaned.clone();
    let mut target = "the stated task or setting".to_string();
    let mut effect = "a meaningful measurable improvement".to_string();
    let main_re = Regex::new(
        r"(.+?)\s+(improve|improves|reduce|reduces|increase|increases|enable|enables)\s+(.+)",
    )
    .unwrap();
    if let Some(caps) = main_re.captures(&lowered) {
        focus = caps[1].trim().to_string();
        target = caps[3].trim().to_string();
        effect = format!("{} {}", caps[2].trim(), caps[3].trim());
    } else {
        let using_re = Regex::new(r"using\s+(.+?)\s+for\s+(.+)").unwrap();
        if let Some(caps) = using_re.captures(&lowered) {
            focus = caps[1].trim().to_string();
            target = caps[2].trim().to_string();
            effect = format!("improve {target}");
        } else {
            let keywords = compact_words(&cleaned, 8);
            if !keywords.is_empty() {
                focus = keywords
                    .iter()
                    .take(4)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" ");
                if keywords.len() >= 6 {
                    target = keywords
                        .iter()
                        .skip(4)
                        .take(4)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(" ");
                    effect = format!("improve {target}");
                }
            }
        }
    }
    (focus, target, effect)
}

fn default_draft_claim_evidence(axis: &str, focus: &str, target: &str) -> Vec<String> {
    match axis.to_lowercase().as_str() {
        "method" => vec![
            format!("Closest papers using {focus}"),
            "Nearest mechanism-level baselines".into(),
            "Evidence that the mechanism meaningfully differs".into(),
        ],
        "task" => vec![
            format!("Prior work on {focus} for {target}"),
            "Recent task/domain competitors from the last 3 years".into(),
            "Evidence that the task framing is not already saturated".into(),
        ],
        "setting" => vec![
            format!("Papers in the same constrained setting as {target}"),
            "Evidence that the constraint changes the problem materially".into(),
            "Comparable results in adjacent settings".into(),
        ],
        _ => vec![
            "Papers making a similar contribution claim".into(),
            "Reviewer-expected baseline papers".into(),
            "Evidence for the exact differentiation sentence".into(),
        ],
    }
}

fn propose_claims_from_question(question: &str, count: usize) -> Vec<Value> {
    let (focus, target, effect) = extract_question_parts(question);
    let templates = vec![
        (
            "method",
            "testable hypothesis",
            format!("Using {focus} is itself a defensible mechanism-level contribution."),
        ),
        (
            "task",
            "direction with concrete benchmark target",
            format!("Applying {focus} to {target} is novel enough to justify a focused study."),
        ),
        (
            "setting",
            "testable hypothesis",
            format!("The value of {focus} depends on the specific setting or constraint around {target}."),
        ),
        (
            "framing",
            "paper-facing positioning claim",
            format!("The strongest paper-facing claim is that {focus} can {effect}, not that it is universally better."),
        ),
        (
            "comparison",
            "reviewer-facing claim",
            format!("The core reviewer question is whether {focus} beats the closest simple baseline for {target}."),
        ),
    ];
    templates
        .into_iter()
        .take(count.clamp(1, 5))
        .enumerate()
        .map(|(index, (axis, specificity, claim))| {
            json!({
                "claim_id": format!("C{}", index + 1),
                "axis": axis,
                "specificity": specificity,
                "claim": claim,
                "required_evidence": default_draft_claim_evidence(axis, &focus, &target),
            })
        })
        .collect()
}

fn draft_claims_from_state(state: &Value, question_override: Option<&str>, count: usize) -> Value {
    let mut next_state = ensure_state_defaults(state);
    let question = question_override
        .map(ToString::to_string)
        .unwrap_or_else(|| str_key(&next_state, "question"));
    let drafts = prioritize_claims(&propose_claims_from_question(&question, count));
    let gate = novelty_gate_mut(&mut next_state);
    gate.insert("draft_claims".into(), json!(drafts));
    let claims = gate
        .get("draft_claims")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .map(|draft| str_field(draft, "claim"))
        .collect::<Vec<_>>();
    gate.insert("claims".into(), json!(claims));
    next_state
}

fn overall_novelty_assessment(state: &Value) -> &'static str {
    let records = novelty_arr(state, "claim_records");
    if records.is_empty() {
        return "insufficient";
    }
    if records
        .iter()
        .all(|record| record.get("verdict").and_then(Value::as_str) == Some("novel"))
    {
        return "strong";
    }
    let not_novel_count = records
        .iter()
        .filter(|record| record.get("verdict").and_then(Value::as_str) == Some("not-novel"))
        .count();
    if not_novel_count > 0 {
        return if not_novel_count >= 2 {
            "weak"
        } else {
            "moderate"
        };
    }
    if records
        .iter()
        .any(|record| record.get("verdict").and_then(Value::as_str) == Some("risky"))
    {
        return "moderate";
    }
    if records
        .iter()
        .any(|record| record.get("verdict").and_then(Value::as_str) == Some("novel"))
    {
        "strong"
    } else {
        "moderate"
    }
}

fn strongest_current_claim(state: &Value) -> String {
    let records = novelty_arr(state, "claim_records");
    if !records.is_empty() {
        let verdict_order = HashMap::from([
            ("novel", 0),
            ("defensible", 1),
            ("risky", 2),
            ("not-novel", 3),
        ]);
        let confidence_order = HashMap::from([("high", 0), ("medium", 1), ("low", 2)]);
        let mut ranked = records.clone();
        ranked.sort_by(|a, b| {
            let va = verdict_order
                .get(a.get("verdict").and_then(Value::as_str).unwrap_or("risky"))
                .unwrap_or(&9);
            let vb = verdict_order
                .get(b.get("verdict").and_then(Value::as_str).unwrap_or("risky"))
                .unwrap_or(&9);
            let ca = confidence_order
                .get(
                    a.get("confidence")
                        .and_then(Value::as_str)
                        .unwrap_or("medium"),
                )
                .unwrap_or(&1);
            let cb = confidence_order
                .get(
                    b.get("confidence")
                        .and_then(Value::as_str)
                        .unwrap_or("medium"),
                )
                .unwrap_or(&1);
            va.cmp(vb)
                .then_with(|| ca.cmp(cb))
                .then_with(|| str_field(a, "claim_id").cmp(&str_field(b, "claim_id")))
        });
        return str_field_default(&ranked[0], "claim", "_No strong claim recorded yet._");
    }
    if let Some(active_id) = state.get("active_hypothesis").and_then(Value::as_str) {
        if let Some(active) = find_hypothesis(state, active_id) {
            return str_field_default(active, "claim", "_No strong claim recorded yet._");
        }
    }
    "_No strong claim recorded yet._".to_string()
}

fn sort_entries_by_recency(entries: &[Value], timestamp_field: &str) -> Vec<Value> {
    let mut sorted = entries.to_vec();
    sorted.sort_by(|a, b| {
        let ta = parse_iso_timestamp(&str_field(a, timestamp_field))
            .unwrap_or_else(|| DateTime::<Utc>::MIN_UTC);
        let tb = parse_iso_timestamp(&str_field(b, timestamp_field))
            .unwrap_or_else(|| DateTime::<Utc>::MIN_UTC);
        tb.cmp(&ta)
    });
    sorted
}

fn recent_entries(
    entries: &[Value],
    timestamp_field: &str,
    max_age_days: i64,
    limit: usize,
    hypothesis_id: Option<&str>,
) -> Vec<Value> {
    let mut filtered = Vec::new();
    for entry in sort_entries_by_recency(entries, timestamp_field) {
        if let Some(hypothesis_id) = hypothesis_id {
            if entry.get("hypothesis_id").and_then(Value::as_str) != Some(hypothesis_id) {
                continue;
            }
        }
        let age = days_since(&str_field(&entry, timestamp_field));
        if age.is_none() || age.unwrap() > max_age_days {
            continue;
        }
        filtered.push(entry);
        if filtered.len() >= limit {
            break;
        }
    }
    filtered
}

fn current_context_runs(state: &Value) -> Vec<Value> {
    let runs = arr(state, "run_history");
    let active_id = state.get("active_hypothesis").and_then(Value::as_str);
    if let Some(active_id) = active_id {
        let active_recent = recent_entries(
            runs,
            "recorded_at",
            RECENT_ACTIVITY_DAYS,
            FALLBACK_ACTIVITY_LIMIT,
            Some(active_id),
        );
        if !active_recent.is_empty() {
            return active_recent;
        }
    }
    let global_recent = recent_entries(
        runs,
        "recorded_at",
        RECENT_ACTIVITY_DAYS,
        FALLBACK_ACTIVITY_LIMIT,
        None,
    );
    if !global_recent.is_empty() {
        return global_recent;
    }
    sort_entries_by_recency(runs, "recorded_at")
        .into_iter()
        .take(FALLBACK_ACTIVITY_LIMIT)
        .collect()
}

fn current_context_decisions(state: &Value) -> Vec<Value> {
    let decisions = arr(state, "decisions");
    let active_id = state.get("active_hypothesis").and_then(Value::as_str);
    if let Some(active_id) = active_id {
        let active_recent = recent_entries(
            decisions,
            "recorded_at",
            RECENT_ACTIVITY_DAYS,
            FALLBACK_ACTIVITY_LIMIT,
            Some(active_id),
        );
        if !active_recent.is_empty() {
            return active_recent;
        }
    }
    let global_recent = recent_entries(
        decisions,
        "recorded_at",
        RECENT_ACTIVITY_DAYS,
        FALLBACK_ACTIVITY_LIMIT,
        None,
    );
    if !global_recent.is_empty() {
        return global_recent;
    }
    sort_entries_by_recency(decisions, "recorded_at")
        .into_iter()
        .take(FALLBACK_ACTIVITY_LIMIT)
        .collect()
}

struct Freshness {
    stale: bool,
    history_bias_risk: bool,
    recent_runs: Vec<Value>,
    recent_decisions: Vec<Value>,
}

fn state_freshness(state: &Value) -> Freshness {
    let updated_days = days_since(&str_key(state, "updated_at"));
    let recent_runs = current_context_runs(state);
    let recent_decisions = current_context_decisions(state);
    let stale = updated_days.is_some_and(|days| days > STALE_STATE_DAYS);
    let history_bias_risk = stale
        || ((!arr(state, "run_history").is_empty() || !arr(state, "decisions").is_empty())
            && recent_runs.is_empty()
            && recent_decisions.is_empty());
    Freshness {
        stale,
        history_bias_risk,
        recent_runs,
        recent_decisions,
    }
}

fn recommend_next_actions(state: &Value) -> Vec<String> {
    let freshness = state_freshness(state);
    if freshness.history_bias_risk {
        return vec![
            "先刷新当前上下文：确认 active hypothesis 和当前目标，旧日志只当背景。".into(),
            "先看 CURRENT_CONTEXT.md 和 research-state.yaml，不要直接沿用更早的 findings 或 research-log 结论。".into(),
            "重查一遍当前代码、数据或最新实验输出，再决定要不要继续旧方向。".into(),
        ];
    }
    if state.get("status").and_then(Value::as_str) == Some("concluded") {
        return vec![
            "Freeze the final narrative in findings.md and to_human/.".into(),
            "Archive the winning evidence path and keep experiment folders append-only.".into(),
        ];
    }
    let gate_status = novelty_str(state, "status", "pending");
    let claims = novelty_arr(state, "claims");
    if gate_status != "passed" {
        let mut actions = Vec::new();
        if claims.is_empty() {
            actions
                .push("提炼 3 到 5 条 novelty claims，先写进 literature/NOVELTY_GATE.md。".into());
        }
        actions.push(
            "用 research-claim 做一轮外部检索，把最近论文证据写进 EXTERNAL_RESEARCH.md。".into(),
        );
        actions.push("先完成 novelty gate，再启动高成本实验。".into());
        if gate_status == "pending" {
            actions.push("给每条 claim 标注 overlap level，并写 differentiation strategy。".into());
        }
        return actions;
    }
    let hypotheses = arr(state, "hypotheses");
    if hypotheses.is_empty() {
        return vec![
            "补 3 条可比较的 hypothesis，并为每条写 prediction 和 success threshold。".into(),
            "从最高优先级 hypothesis 开始，不要并发改同一份研究状态。".into(),
        ];
    }
    let active = state
        .get("active_hypothesis")
        .and_then(Value::as_str)
        .and_then(|id| find_hypothesis(state, id));
    if active.is_none() {
        if let Some(candidate) = choose_backlog_hypothesis(state) {
            let candidate_id = str_field(candidate, "id");
            return vec![
                format!("把 {candidate_id} 设为 active hypothesis，并先写协议。"),
                format!("在 experiments/{candidate_id}/ 下落 protocol 和 run record。"),
            ];
        }
        return vec!["清理 hypothesis 列表，重新指定一个 active hypothesis。".into()];
    }
    let active = active.unwrap();
    let active_id = str_field(active, "id");
    let latest_run = latest_run_for_hypothesis(state, &active_id);
    if latest_run.is_none() {
        return vec![
            format!("先为 {active_id} 写 protocol，再做第一轮 bounded run。"),
            "跑完立刻记录 metric、sanity check 和 rules in / rules out。".into(),
        ];
    }
    let latest_run = latest_run.unwrap();
    let latest_decision = latest_decision_for_hypothesis(state, &active_id);
    if latest_decision.is_none()
        || latest_decision.unwrap().get("run_id") != latest_run.get("run_id")
    {
        return vec![
            format!(
                "对 {} 做 reflection，并明确选 DEEPEN/BROADEN/PIVOT/CONCLUDE。",
                str_field(latest_run, "run_id")
            ),
            "把结果写回 findings.md，而不只是留在聊天里。".into(),
        ];
    }
    match latest_decision
        .unwrap()
        .get("direction")
        .and_then(Value::as_str)
    {
        Some("DEEPEN") => vec![
            format!("围绕 {active_id} 收紧变量，再做一个更小更干净的验证实验。"),
            "只改一个关键因素，避免把因果解释搅混。".into(),
        ],
        Some("BROADEN") => vec![
            format!("把 {active_id} 的结论扩到第二个 setting 或 baseline。"),
            "保持协议不变，只扩数据面或比较面。".into(),
        ],
        Some("PIVOT") => {
            if let Some(candidate) = choose_backlog_hypothesis(state) {
                let candidate_id = str_field(candidate, "id");
                if candidate_id != active_id {
                    return vec![
                        format!("停止继续堆 {active_id}，切到 {candidate_id} 开新 protocol。"),
                        "把旧方向失败原因写清楚，避免重复试错。".into(),
                    ];
                }
            }
            vec![
                "当前方向该 pivot，但还缺新的候选 hypothesis。".into(),
                "先补 hypothesis backlog，再选新的 active hypothesis。".into(),
            ]
        }
        _ => vec!["进入 finalize，把 strongest claim、证据链和未解决风险收束成 handoff。".into()],
    }
}

fn find_hypothesis<'a>(state: &'a Value, hypothesis_id: &str) -> Option<&'a Value> {
    arr(state, "hypotheses")
        .iter()
        .find(|item| item.get("id").and_then(Value::as_str) == Some(hypothesis_id))
}

fn find_hypothesis_index(state: &Value, hypothesis_id: &str) -> Option<usize> {
    arr(state, "hypotheses")
        .iter()
        .position(|item| item.get("id").and_then(Value::as_str) == Some(hypothesis_id))
}

fn choose_backlog_hypothesis(state: &Value) -> Option<&Value> {
    if arr(state, "hypotheses").is_empty() {
        return None;
    }
    for id in arr(state, "hypothesis_backlog")
        .iter()
        .filter_map(Value::as_str)
    {
        if let Some(candidate) = find_hypothesis(state, id) {
            if candidate.get("status").and_then(Value::as_str) != Some("concluded") {
                return Some(candidate);
            }
        }
    }
    let priority_order = HashMap::from([("high", 0), ("medium", 1), ("low", 2)]);
    let mut ranked = arr(state, "hypotheses").iter().collect::<Vec<_>>();
    ranked.sort_by(|a, b| {
        let pa = priority_order
            .get(
                a.get("priority")
                    .and_then(Value::as_str)
                    .unwrap_or("medium"),
            )
            .unwrap_or(&1);
        let pb = priority_order
            .get(
                b.get("priority")
                    .and_then(Value::as_str)
                    .unwrap_or("medium"),
            )
            .unwrap_or(&1);
        pa.cmp(pb)
            .then_with(|| str_field(a, "id").cmp(&str_field(b, "id")))
    });
    ranked.into_iter().next()
}

fn latest_run_for_hypothesis<'a>(state: &'a Value, hypothesis_id: &str) -> Option<&'a Value> {
    arr(state, "run_history")
        .iter()
        .rev()
        .find(|item| item.get("hypothesis_id").and_then(Value::as_str) == Some(hypothesis_id))
}

fn latest_decision_for_hypothesis<'a>(state: &'a Value, hypothesis_id: &str) -> Option<&'a Value> {
    arr(state, "decisions")
        .iter()
        .rev()
        .find(|item| item.get("hypothesis_id").and_then(Value::as_str) == Some(hypothesis_id))
}

fn next_run_id(state: &Value) -> String {
    format!("run-{:03}", arr(state, "run_history").len() + 1)
}

fn default_run_record_path(hypothesis_id: &str, run_id: &str) -> String {
    format!("experiments/{hypothesis_id}/{run_id}.md")
}

fn default_reflection_path(hypothesis_id: &str, run_id: Option<&str>) -> String {
    format!(
        "experiments/{hypothesis_id}/{}-reflection.md",
        run_id.unwrap_or("reflection")
    )
}

fn transition_hypothesis(
    state: &mut Value,
    hypothesis_index: usize,
    new_status: &str,
    reason: Option<&str>,
) -> Result<()> {
    let hypotheses = arr_mut(state, "hypotheses");
    let hypothesis = hypotheses
        .get_mut(hypothesis_index)
        .ok_or_else(|| anyhow!("Missing hypothesis index"))?;
    let previous = hypothesis.get("status").and_then(Value::as_str);
    let allowed = match previous {
        None => vec![
            "queued",
            "active",
            "needs_reflection",
            "parked",
            "concluded",
        ],
        Some("queued") => vec!["queued", "active", "parked", "concluded"],
        Some("active") => vec!["active", "needs_reflection", "parked", "concluded"],
        Some("needs_reflection") => vec!["needs_reflection", "active", "parked", "concluded"],
        Some("parked") => vec!["parked", "queued", "active", "concluded"],
        Some("concluded") => vec!["concluded"],
        _ => vec![],
    };
    if !allowed.contains(&new_status) {
        bail!(
            "Invalid hypothesis transition for {}: {} -> {}",
            str_field_default(hypothesis, "id", "?"),
            previous.unwrap_or("none"),
            new_status
        );
    }
    let hypothesis_id = str_field(hypothesis, "id");
    let item = hypothesis.as_object_mut().unwrap();
    item.insert("status".into(), json!(new_status));
    item.insert(
        "status_reason".into(),
        reason.map(Value::from).unwrap_or(Value::Null),
    );
    item.insert("status_updated_at".into(), json!(now_iso()));
    let backlog = arr_mut(state, "hypothesis_backlog");
    if new_status == "queued" {
        if !backlog
            .iter()
            .any(|item| item.as_str() == Some(&hypothesis_id))
        {
            backlog.push(json!(hypothesis_id));
        }
    } else if let Some(index) = backlog
        .iter()
        .position(|item| item.as_str() == Some(&hypothesis_id))
    {
        backlog.remove(index);
    }
    Ok(())
}

fn add_hypothesis(
    state: &Value,
    claim: &str,
    prediction: Option<&str>,
    priority: &str,
    hypothesis_id: Option<&str>,
) -> Result<Value> {
    let mut next_state = ensure_state_defaults(state);
    let resolved_id = hypothesis_id
        .map(ToString::to_string)
        .unwrap_or_else(|| slugify(claim).chars().take(40).collect());
    if find_hypothesis(&next_state, &resolved_id).is_some() {
        bail!("Hypothesis already exists: {resolved_id}");
    }
    let entry = json!({
        "id": resolved_id,
        "claim": claim,
        "prediction": prediction,
        "priority": priority,
        "status": "queued",
        "status_reason": Value::Null,
        "status_updated_at": now_iso(),
        "created_at": now_iso(),
    });
    arr_mut(&mut next_state, "hypotheses").push(entry);
    let backlog = arr_mut(&mut next_state, "hypothesis_backlog");
    if !backlog
        .iter()
        .any(|item| item.as_str() == Some(&resolved_id))
    {
        backlog.push(json!(resolved_id.clone()));
    }
    if next_state
        .get("active_hypothesis")
        .and_then(Value::as_str)
        .is_none()
        && novelty_str(&next_state, "status", "pending") == "passed"
    {
        set_key(
            &mut next_state,
            "active_hypothesis",
            json!(resolved_id.clone()),
        );
        let index = find_hypothesis_index(&next_state, &resolved_id).unwrap();
        transition_hypothesis(
            &mut next_state,
            index,
            "active",
            Some("first active hypothesis after novelty gate passed"),
        )?;
    }
    Ok(next_state)
}

#[allow(clippy::too_many_arguments)]
fn record_run(
    state: &Value,
    hypothesis_id: &str,
    outcome: &str,
    summary: &str,
    metric_name: Option<&str>,
    metric_value: Option<&str>,
    command: Option<&str>,
    evidence_path: Option<&str>,
    override_novelty_gate: bool,
    override_reason: Option<&str>,
    workspace: &Path,
) -> Result<Value> {
    let mut next_state = ensure_state_defaults(state);
    let Some(index) = find_hypothesis_index(&next_state, hypothesis_id) else {
        bail!("Unknown hypothesis: {hypothesis_id}");
    };
    let gate_status = novelty_str(&next_state, "status", "pending");
    if gate_status != "passed" {
        if !override_novelty_gate {
            bail!("Novelty gate must pass before recording runs (current: {gate_status})");
        }
        if override_reason.unwrap_or("").trim().is_empty() {
            bail!("Novelty gate override requires --override-reason");
        }
    }
    let current_status = find_hypothesis(&next_state, hypothesis_id)
        .and_then(|item| item.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("queued")
        .to_string();
    if !["active", "queued"].contains(&current_status.as_str()) {
        bail!("Hypothesis {hypothesis_id} must be active or queued before a run, current status: {current_status}");
    }
    if current_status == "queued" {
        transition_hypothesis(
            &mut next_state,
            index,
            "active",
            Some("activated by first recorded run"),
        )?;
    }
    let run_id = next_run_id(&next_state);
    set_key(&mut next_state, "stage", json!(STAGE_OUTER_LOOP));
    set_key(&mut next_state, "active_hypothesis", json!(hypothesis_id));
    let environment = if workspace.join("research-state.yaml").exists() {
        next_state
            .get("environment")
            .filter(|value| !value.is_null())
            .cloned()
            .unwrap_or_else(|| capture_environment_fingerprint(workspace))
    } else {
        capture_environment_fingerprint(workspace)
    };
    let provenance = if workspace.join("research-state.yaml").exists() {
        next_state
            .get("git")
            .filter(|value| !value.is_null())
            .cloned()
            .unwrap_or_else(|| capture_git_provenance(workspace))
    } else {
        capture_git_provenance(workspace)
    };
    set_key(&mut next_state, "environment", environment.clone());
    set_key(&mut next_state, "git", provenance.clone());
    let record = json!({
        "run_id": run_id,
        "hypothesis_id": hypothesis_id,
        "outcome": outcome,
        "summary": summary,
        "metric_name": metric_name,
        "metric_value": metric_value,
        "command": command,
        "evidence_path": evidence_path.map(ToString::to_string).unwrap_or_else(|| default_run_record_path(hypothesis_id, &run_id)),
        "novelty_gate_status_at_run": gate_status,
        "novelty_gate_override": override_novelty_gate,
        "override_reason": override_reason,
        "environment_fingerprint": environment,
        "git_provenance": provenance,
        "recorded_at": now_iso(),
    });
    arr_mut(&mut next_state, "run_history").push(record.clone());
    transition_hypothesis(
        &mut next_state,
        index,
        "needs_reflection",
        Some(&format!("{run_id} recorded")),
    )?;
    arr_mut(&mut next_state, "evidence_index").push(json!({
        "run_id": run_id,
        "path": record.get("evidence_path").cloned().unwrap_or(Value::Null),
        "added_at": now_iso(),
    }));
    Ok(next_state)
}

fn reflect(
    state: &Value,
    hypothesis_id: &str,
    direction: &str,
    reason: &str,
    next_step: Option<&str>,
    activate_hypothesis: Option<&str>,
) -> Result<Value> {
    let mut next_state = ensure_state_defaults(state);
    let Some(index) = find_hypothesis_index(&next_state, hypothesis_id) else {
        bail!("Unknown hypothesis: {hypothesis_id}");
    };
    let status = find_hypothesis(&next_state, hypothesis_id)
        .and_then(|item| item.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("-");
    if status != "needs_reflection" {
        bail!("Hypothesis {hypothesis_id} must be in needs_reflection before reflect, current status: {status}");
    }
    let latest_run = latest_run_for_hypothesis(&next_state, hypothesis_id)
        .ok_or_else(|| anyhow!("Cannot reflect without a recorded run for {hypothesis_id}"))?;
    if let Some(latest_decision) = latest_decision_for_hypothesis(&next_state, hypothesis_id) {
        if latest_decision.get("run_id") == latest_run.get("run_id") {
            bail!(
                "Run {} already has a reflection",
                str_field(latest_run, "run_id")
            );
        }
    }
    let run_id = str_field(latest_run, "run_id");
    let decision = json!({
        "hypothesis_id": hypothesis_id,
        "run_id": run_id,
        "direction": direction,
        "reason": reason,
        "next_step": next_step,
        "note_path": default_reflection_path(hypothesis_id, Some(&run_id)),
        "recorded_at": now_iso(),
    });
    arr_mut(&mut next_state, "decisions").push(decision);
    set_key(&mut next_state, "current_direction", json!(direction));
    match direction {
        "CONCLUDE" => {
            set_key(&mut next_state, "status", json!("concluded"));
            set_key(&mut next_state, "stage", json!(STAGE_FINALIZE));
            transition_hypothesis(&mut next_state, index, "concluded", Some(reason))?;
        }
        "PIVOT" => {
            set_key(&mut next_state, "stage", json!(STAGE_INNER_LOOP));
            transition_hypothesis(&mut next_state, index, "parked", Some(reason))?;
        }
        _ => {
            set_key(&mut next_state, "stage", json!(STAGE_INNER_LOOP));
            transition_hypothesis(&mut next_state, index, "active", Some(reason))?;
        }
    }
    if let Some(target_id) = activate_hypothesis {
        let Some(target_index) = find_hypothesis_index(&next_state, target_id) else {
            bail!("Unknown activate_hypothesis: {target_id}");
        };
        set_key(&mut next_state, "active_hypothesis", json!(target_id));
        let target_status = find_hypothesis(&next_state, target_id)
            .and_then(|item| item.get("status"))
            .and_then(Value::as_str)
            .unwrap_or("-");
        if target_status == "queued" {
            transition_hypothesis(
                &mut next_state,
                target_index,
                "active",
                Some("activated after pivot"),
            )?;
        } else if target_status == "parked" {
            transition_hypothesis(
                &mut next_state,
                target_index,
                "active",
                Some("reactivated after pivot"),
            )?;
        }
    } else if direction != "CONCLUDE" {
        set_key(&mut next_state, "active_hypothesis", json!(hypothesis_id));
    }
    Ok(next_state)
}

#[allow(clippy::too_many_arguments)]
fn add_claim_comparison(
    state: &Value,
    claim: &str,
    axis: &str,
    closest_prior_work: &str,
    overlap: &str,
    difference: &str,
    confidence: &str,
    verdict: &str,
    claim_id: Option<&str>,
) -> Value {
    let mut next_state = ensure_state_defaults(state);
    let resolved_id = claim_id
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("C{}", novelty_arr(&next_state, "claim_records").len() + 1));
    let record = json!({
        "claim_id": resolved_id,
        "claim": claim,
        "axis": axis,
        "closest_prior_work": closest_prior_work,
        "overlap": overlap,
        "difference": difference,
        "confidence": confidence,
        "verdict": verdict,
        "recorded_at": now_iso(),
    });
    let gate = novelty_gate_mut(&mut next_state);
    let records = gate
        .entry("claim_records")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .unwrap();
    if let Some(index) = records
        .iter()
        .position(|item| item.get("claim_id").and_then(Value::as_str) == Some(&resolved_id))
    {
        records[index] = record;
    } else {
        records.push(record);
    }
    let prioritized = prioritize_claims(records);
    gate.insert("claim_records".into(), json!(prioritized.clone()));
    gate.insert(
        "claims".into(),
        json!(prioritized
            .iter()
            .map(|item| str_field(item, "claim"))
            .collect::<Vec<_>>()),
    );
    gate.insert(
        "overlap_summary".into(),
        json!(prioritized
            .iter()
            .map(|item| format!(
                "{}={}",
                str_field(item, "claim_id"),
                str_field(item, "overlap")
            ))
            .collect::<Vec<_>>()
            .join(", ")),
    );
    next_state
}

fn escape_table_cell(value: &str) -> String {
    value.replace('|', "/")
}

fn format_overlap_risk(overlap: &str) -> String {
    match overlap {
        "low" => "🟢 low".into(),
        "medium" => "🟡 medium".into(),
        "high" => "🔴 high".into(),
        _ => overlap.into(),
    }
}

fn summarize_rules_in(state: &Value) -> Vec<String> {
    let lines = current_context_runs(state)
        .into_iter()
        .map(|record| {
            format!(
                "{}: {}",
                str_field(&record, "run_id"),
                str_field_default(&record, "summary", "_No summary_")
            )
        })
        .collect::<Vec<_>>();
    if lines.is_empty() {
        vec!["_No run-backed support recorded yet._".into()]
    } else {
        lines
    }
}

fn summarize_rules_out(state: &Value) -> Vec<String> {
    let lines = current_context_runs(state)
        .into_iter()
        .filter(|record| {
            ["failed", "ambiguous"]
                .contains(&record.get("outcome").and_then(Value::as_str).unwrap_or(""))
        })
        .map(|record| {
            format!(
                "{}: {}",
                str_field(&record, "run_id"),
                str_field_default(&record, "summary", "_No summary_")
            )
        })
        .collect::<Vec<_>>();
    if lines.is_empty() {
        vec!["_No ruled-out branch has been recorded yet._".into()]
    } else {
        lines
    }
}

fn summarize_remaining_risks(state: &Value) -> Vec<String> {
    let blockers = arr(state, "blockers");
    if !blockers.is_empty() {
        return blockers
            .iter()
            .take(3)
            .map(|item| item.as_str().unwrap_or(&item.to_string()).to_string())
            .collect();
    }
    let actions = state
        .get("next_actions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if actions.is_empty() {
        vec!["_No explicit remaining risk recorded._".into()]
    } else {
        actions
            .iter()
            .take(3)
            .map(|item| item.as_str().unwrap_or("").to_string())
            .collect()
    }
}

fn render_findings_summary(state: &Value) -> String {
    let mut lines = vec![
        "## Managed Summary".into(),
        String::new(),
        format!(
            "- strongest current claim: {}",
            strongest_current_claim(state)
        ),
        String::new(),
        "### What The Evidence Rules In".into(),
    ];
    for item in summarize_rules_in(state) {
        lines.push(format!("- {item}"));
    }
    lines.extend([String::new(), "### What The Evidence Rules Out".into()]);
    for item in summarize_rules_out(state) {
        lines.push(format!("- {item}"));
    }
    lines.extend([String::new(), "### Remaining Risks".into()]);
    for item in summarize_remaining_risks(state) {
        lines.push(format!("- {item}"));
    }
    lines.extend([
        String::new(),
        "### Positioning Strategy".into(),
        format!(
            "- {}",
            novelty_gate(state)
                .get("differentiation_strategy")
                .and_then(Value::as_str)
                .unwrap_or("_Not recorded yet._")
        ),
    ]);
    if let Some(entry) = latest_external_research(state) {
        lines.extend([
            String::new(),
            "### Latest External Research".into(),
            format!("- query: {}", str_field_default(entry, "query", "-")),
            format!("- results: {}", external_research_result_count(entry)),
        ]);
    }
    lines.join("\n")
}

fn render_novelty_gate_summary(state: &Value) -> String {
    let records = novelty_arr(state, "claim_records");
    let mut lines = vec![
        "## Managed Summary".into(),
        String::new(),
        format!("- status: {}", novelty_str(state, "status", "pending")),
        format!(
            "- overall novelty assessment: {}",
            overall_novelty_assessment(state)
        ),
        format!(
            "- decision: {}",
            novelty_str(state, "decision", "_Not recorded yet._")
        ),
        format!(
            "- overlap summary: {}",
            novelty_str(state, "overlap_summary", "_Not recorded yet._")
        ),
        String::new(),
        "## Claim Comparison Matrix".into(),
        String::new(),
        "| Claim | Axis | Closest Prior Work | Overlap | Difference | Confidence | Verdict |"
            .into(),
        "|---|---|---|---|---|---|---|".into(),
    ];
    if records.is_empty() {
        lines.push("| _none yet_ | - | - | - | - | - | - |".into());
    } else {
        for record in records {
            lines.push(format!(
                "| {} | {} | {} | {} | {} | {} | {} |",
                escape_table_cell(&str_field_default(record, "claim", "_missing_")),
                escape_table_cell(&str_field_default(record, "axis", "-")),
                escape_table_cell(&str_field_default(record, "closest_prior_work", "-")),
                format_overlap_risk(&str_field_default(record, "overlap", "-")),
                escape_table_cell(&str_field_default(record, "difference", "-")),
                str_field_default(record, "confidence", "-"),
                str_field_default(record, "verdict", "-")
            ));
        }
    }
    lines.extend([
        String::new(),
        "## Differentiation Strategy".into(),
        String::new(),
        novelty_str(state, "differentiation_strategy", "_Not recorded yet._"),
    ]);
    lines.join("\n")
}

fn render_search_plan_summary(state: &Value) -> String {
    let plan = current_search_plan(state);
    let top = plan
        .iter()
        .find(|entry| entry.get("recommended_order").and_then(Value::as_i64) == Some(1));
    let mut lines = vec![
        "## Managed Search Plan".into(),
        String::new(),
        format!("- generated entries: {}", plan.len()),
        "- source priority: Semantic Scholar -> arXiv -> Google Scholar".into(),
        top.map(|entry| {
            format!(
                "- recommended first search target: {} ({})",
                str_field(entry, "claim_id"),
                str_field(entry, "priority_label")
            )
        })
        .unwrap_or_else(|| "- recommended first search target: _not set_".into()),
        String::new(),
    ];
    if plan.is_empty() {
        lines.push("_No search plan has been generated yet._".into());
        return lines.join("\n");
    }
    for entry in plan {
        lines.extend([
            format!(
                "### {} — {}",
                str_field_default(&entry, "claim_id", "C?"),
                str_field_default(&entry, "claim", "_missing_")
            ),
            String::new(),
            format!("- axis: {}", str_field_default(&entry, "axis", "-")),
            format!(
                "- recommended order: {}",
                entry
                    .get("recommended_order")
                    .map(value_to_string)
                    .unwrap_or_else(|| "-".into())
            ),
            format!(
                "- priority: {} ({})",
                str_field_default(&entry, "priority_label", "-"),
                entry
                    .get("priority_score")
                    .map(value_to_string)
                    .unwrap_or_else(|| "-".into())
            ),
            format!(
                "- why first or later: {}",
                str_field_default(&entry, "priority_reason", "-")
            ),
            format!(
                "- keywords: {}",
                entry
                    .get("keywords")
                    .and_then(Value::as_array)
                    .map(|values| join_string_array(values))
                    .unwrap_or_else(|| "_none_".into())
            ),
            format!(
                "- sources: {}",
                entry
                    .get("sources")
                    .and_then(Value::as_array)
                    .map(|values| join_string_array(values))
                    .unwrap_or_default()
            ),
            String::new(),
            "#### Query Ladder".into(),
        ]);
        for query in entry
            .get("queries")
            .and_then(Value::as_array)
            .unwrap_or(&Vec::new())
        {
            lines.push(format!(
                "- {}: `{}`",
                str_field_default(query, "label", "query"),
                str_field(query, "query")
            ));
        }
        lines.extend([String::new(), "#### Required Evidence".into()]);
        for item in entry
            .get("required_evidence")
            .and_then(Value::as_array)
            .unwrap_or(&Vec::new())
        {
            lines.push(format!("- {}", item.as_str().unwrap_or("")));
        }
        lines.push(String::new());
    }
    lines.join("\n").trim_end().to_string()
}

fn render_external_research_summary(state: &Value) -> String {
    let entries = arr(state, "external_research");
    let mut lines = vec![
        "## Managed External Research".into(),
        String::new(),
        format!("- recorded searches: {}", entries.len()),
        "- sources: Semantic Scholar, arXiv".into(),
        String::new(),
    ];
    if entries.is_empty() {
        lines.push(
            "_No external research recorded yet. Run `research-claim` after drafting claims._"
                .into(),
        );
        return lines.join("\n");
    }
    for entry in entries.iter().rev().take(5) {
        lines.extend([
            format!(
                "### {} — {}",
                str_field_default(entry, "research_id", "ext-?"),
                str_field_default(entry, "query", "-")
            ),
            String::new(),
            format!(
                "- claim: {}",
                str_field_default(entry, "claim_id", "custom")
            ),
            format!(
                "- source mode: {}",
                str_field_default(entry, "source", "all")
            ),
            format!(
                "- captured at: {}",
                str_field_default(entry, "created_at", "-")
            ),
            format!("- result count: {}", external_research_result_count(entry)),
            String::new(),
        ]);
        for result in entry
            .get("results")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .take(8)
        {
            let year = result
                .get("year")
                .map(value_to_string)
                .filter(|value| !value.is_empty() && value != "null")
                .unwrap_or_else(|| "-".into());
            lines.push(format!(
                "- {} ({}, {}): {}",
                str_field_default(result, "title", "_untitled_"),
                year,
                str_field_default(result, "source", "-"),
                markdown_link(result.get("url").and_then(Value::as_str))
            ));
        }
        let errors = entry
            .get("errors")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .unwrap_or_default();
        if !errors.is_empty() {
            lines.push(format!("- source errors: {errors}"));
        }
        lines.push(String::new());
    }
    lines.join("\n").trim_end().to_string()
}

fn render_claims_summary(state: &Value) -> String {
    let drafts = novelty_arr(state, "draft_claims");
    let top = drafts
        .iter()
        .find(|draft| draft.get("recommended_order").and_then(Value::as_i64) == Some(1));
    let mut lines = vec![
        "## Managed Claim Extraction".into(),
        String::new(),
        format!("- generated claims: {}", drafts.len()),
        top.map(|draft| {
            format!(
                "- recommended first claim: {} ({})",
                str_field(draft, "claim_id"),
                str_field(draft, "priority_label")
            )
        })
        .unwrap_or_else(|| "- recommended first claim: _not set_".into()),
        String::new(),
    ];
    if drafts.is_empty() {
        lines.push("_No draft claims have been generated yet._".into());
        return lines.join("\n");
    }
    for draft in drafts {
        lines.extend([
            format!("### {}", str_field_default(draft, "claim_id", "C?")),
            String::new(),
            format!("- axis: {}", str_field_default(draft, "axis", "-")),
            format!(
                "- specificity: {}",
                str_field_default(draft, "specificity", "-")
            ),
            format!(
                "- recommended order: {}",
                draft
                    .get("recommended_order")
                    .map(value_to_string)
                    .unwrap_or_else(|| "-".into())
            ),
            format!(
                "- priority: {} ({})",
                str_field_default(draft, "priority_label", "-"),
                draft
                    .get("priority_score")
                    .map(value_to_string)
                    .unwrap_or_else(|| "-".into())
            ),
            format!(
                "- why first or later: {}",
                str_field_default(draft, "priority_reason", "-")
            ),
            format!("- claim: {}", str_field_default(draft, "claim", "-")),
            String::new(),
            "#### Required Evidence".into(),
        ]);
        for item in draft
            .get("required_evidence")
            .and_then(Value::as_array)
            .unwrap_or(&Vec::new())
        {
            lines.push(format!("- {}", item.as_str().unwrap_or("")));
        }
        lines.push(String::new());
    }
    lines.join("\n").trim_end().to_string()
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => "-".into(),
        other => other.to_string(),
    }
}

fn join_string_array(values: &[Value]) -> String {
    let joined = values
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    if joined.is_empty() {
        "_none_".into()
    } else {
        joined
    }
}

fn render_current_context_summary(state: &Value) -> String {
    let freshness = state_freshness(state);
    let brief = current_brief(state);
    let mut lines = vec![
        "## Managed Current Context".into(),
        String::new(),
        "- source of truth: `research-state.yaml`".into(),
        format!("- state updated_at: {}", str_key(state, "updated_at")),
        format!("- freshness: {}", if freshness.stale { "stale" } else { "fresh" }),
        format!(
            "- history bias risk: {}",
            if freshness.history_bias_risk { "high" } else { "low" }
        ),
        format!("- active hypothesis: {}", state.get("active_hypothesis").and_then(Value::as_str).unwrap_or("-")),
        format!("- recommended focus: {}", current_recommended_focus(state).unwrap_or_else(|| "-".into())),
        "- guardrail: treat `research-log.md` and older notes as background only unless they reappear in the current context window.".into(),
        String::new(),
        "### Recent Activity Window".into(),
        format!("- window policy: prefer the active hypothesis and the last {RECENT_ACTIVITY_DAYS} days; otherwise fall back to the latest few entries."),
        String::new(),
        "### Recent Runs".into(),
    ];
    if freshness.recent_runs.is_empty() {
        lines.push("- _No recent runs in the current context window._".into());
    } else {
        for record in &freshness.recent_runs {
            lines.push(format!(
                "- {}: {}",
                str_field_default(record, "run_id", "-"),
                str_field_default(record, "summary", "_No summary_")
            ));
        }
    }
    lines.extend([String::new(), "### Recent Decisions".into()]);
    if freshness.recent_decisions.is_empty() {
        lines.push("- _No recent decisions in the current context window._".into());
    } else {
        for decision in &freshness.recent_decisions {
            lines.push(format!(
                "- {}: {} because {}",
                decision
                    .get("run_id")
                    .and_then(Value::as_str)
                    .unwrap_or("no-run"),
                str_field_default(decision, "direction", "-"),
                str_field_default(decision, "reason", "_No reason_")
            ));
        }
    }
    if let Some(brief) = brief {
        lines.extend([
            String::new(),
            "### Active Novelty Brief".into(),
            format!(
                "- claim: {} — {}",
                str_field(&brief, "claim_id"),
                str_field(&brief, "claim")
            ),
            format!("- decision goal: {}", str_field(&brief, "decision_goal")),
            format!(
                "- verification standard: {}",
                str_field(&brief, "verification_standard")
            ),
            "- expected baselines:".into(),
        ]);
        for baseline in brief
            .get("expected_baselines")
            .and_then(Value::as_array)
            .unwrap_or(&Vec::new())
        {
            lines.push(format!("- {}", baseline.as_str().unwrap_or("")));
        }
    }
    if freshness.history_bias_risk {
        lines.extend([
            String::new(),
            "### Reconcile First".into(),
            "- Confirm the active hypothesis is still the real target before trusting old notes.".into(),
            "- Re-check live data, code, or current artifacts before extending any older conclusion.".into(),
        ]);
    }
    lines.join("\n")
}

fn upsert_managed_block(text: &str, start_marker: &str, end_marker: &str, content: &str) -> String {
    let managed = format!("{start_marker}\n{}\n{end_marker}", content.trim_end());
    if text.contains(start_marker) && text.contains(end_marker) {
        let pattern = Regex::new(&format!(
            "(?s){}.*?{}",
            regex::escape(start_marker),
            regex::escape(end_marker)
        ))
        .unwrap();
        pattern.replace(text, managed).to_string()
    } else {
        let stripped = text.trim_end();
        if stripped.is_empty() {
            format!("{managed}\n")
        } else {
            format!("{managed}\n\n{stripped}\n")
        }
    }
}

fn sync_managed_file(
    path: &Path,
    fallback: &str,
    start: &str,
    end: &str,
    content: String,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = fs::read_to_string(path).unwrap_or_else(|_| fallback.to_string());
    let updated = upsert_managed_block(&text, start, end, &content);
    fs::write(path, updated)?;
    Ok(())
}

fn format_hypothesis_card(hypothesis: &Value) -> String {
    [
        "# Hypothesis Card",
        "",
        "## Hypothesis ID",
        "",
        &format!("`{}`", str_field(hypothesis, "id")),
        "",
        "## Claim",
        "",
        &str_field_default(hypothesis, "claim", "_TBD_"),
        "",
        "## Prediction",
        "",
        &str_field_default(
            hypothesis,
            "prediction",
            "_Add the expected observable change._",
        ),
        "",
        "## Priority",
        "",
        &format!("`{}`", str_field_default(hypothesis, "priority", "medium")),
        "",
        "## Success Threshold",
        "",
        "_What metric or observation counts as a win?_",
        "",
        "## Stop Condition",
        "",
        "_When do we stop spending more budget on this branch?_",
        "",
    ]
    .join("\n")
}

fn format_protocol(hypothesis: &Value) -> String {
    [
        "# Experiment Protocol",
        "",
        "## Hypothesis",
        "",
        &str_field_default(hypothesis, "claim", "_Which hypothesis is being tested?_"),
        "",
        "## What Change",
        "",
        "_What changes in this run?_",
        "",
        "## Prediction",
        "",
        &str_field_default(hypothesis, "prediction", "_What outcome do you expect?_"),
        "",
        "## Metric",
        "",
        "_Primary metric plus sanity checks._",
        "",
        "## Success Threshold",
        "",
        "_What result counts as success?_",
        "",
        "## Command / Entry Point",
        "",
        "```bash",
        "# put the exact command here",
        "```",
        "",
        "## Seed / Environment",
        "",
        "_Record what is needed for reproducibility._",
        "",
        "## Stop Condition",
        "",
        "_When do you stop this line?_",
        "",
    ]
    .join("\n")
}

fn format_analysis_stub(hypothesis: &Value) -> String {
    [
        &format!("# Analysis — {}", str_field(hypothesis, "id")),
        "",
        "## Current Pattern",
        "",
        "_Summarize what repeated runs are saying._",
        "",
        "## What This Probably Means",
        "",
        "_Prefer mechanism over raw metric narration._",
        "",
        "## Open Questions",
        "",
        "_What still needs to be disambiguated?_",
        "",
    ]
    .join("\n")
}

fn format_run_record(record: &Value) -> String {
    let metric_name = str_field_default(record, "metric_name", "metric");
    let metric_value = str_field_default(record, "metric_value", "value");
    let command = str_field_default(record, "command", "_not recorded_");
    let artifact_path = str_field_default(record, "evidence_path", "_not recorded_");
    let override_used = if record
        .get("novelty_gate_override")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        "yes"
    } else {
        "no"
    };
    let override_reason = str_field_default(record, "override_reason", "_not used_");
    let environment = summarize_environment_fingerprint(record.get("environment_fingerprint"));
    let provenance = summarize_git_provenance(record.get("git_provenance"));
    [
        "# Run Record",
        "",
        "## Run ID",
        "",
        &format!("`{}`", str_field(record, "run_id")),
        "",
        "## Hypothesis",
        "",
        &format!("`{}`", str_field(record, "hypothesis_id")),
        "",
        "## Outcome",
        "",
        &format!("`{}`", str_field(record, "outcome")),
        "",
        "## Summary",
        "",
        &str_field_default(record, "summary", "_No summary recorded._"),
        "",
        "## Metric Snapshot",
        "",
        &format!("- metric: {metric_name}"),
        &format!("- value: {metric_value}"),
        "- sanity: _fill in sanity checks here_",
        "",
        "## Evidence",
        "",
        &format!("- command: {command}"),
        &format!("- artifact path: {artifact_path}"),
        &format!(
            "- novelty gate at run: {}",
            str_field_default(record, "novelty_gate_status_at_run", "-")
        ),
        &format!("- novelty override used: {override_used}"),
        &format!("- override reason: {override_reason}"),
        &format!("- environment: {environment}"),
        &format!("- git: {provenance}"),
        "",
        "## Rules In / Rules Out",
        "",
        "_What changed in your belief after this run?_",
        "",
    ]
    .join("\n")
}

fn format_reflection_note(decision: &Value) -> String {
    [
        "# Reflection Note",
        "",
        "## Run",
        "",
        &format!("`{}`", str_field_default(decision, "run_id", "run-xxx")),
        "",
        "## What Happened",
        "",
        &str_field_default(decision, "reason", "_Summarize the observed pattern._"),
        "",
        "## Why It Probably Happened",
        "",
        &str_field_default(
            decision,
            "reason",
            "_Mechanistic explanation or best current guess._",
        ),
        "",
        "## Rules In / Rules Out",
        "",
        "_What did this result actually eliminate or support?_",
        "",
        "## Direction",
        "",
        &format!("`{}`", str_field_default(decision, "direction", "DEEPEN")),
        "",
        "## Next Step",
        "",
        &str_field_default(decision, "next_step", "_One concrete next move only._"),
        "",
    ]
    .join("\n")
}

fn write_if_missing(path: &Path, content: String) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if !path.exists() {
        fs::write(path, content)?;
    }
    Ok(())
}

fn sync_workspace_files(workspace: &Path, state: &Value) -> Result<()> {
    let legacy_brief = workspace.join("literature/NOVELTY_BRIEF.md");
    if legacy_brief.exists() {
        fs::remove_file(legacy_brief)?;
    }
    for hypothesis in arr(state, "hypotheses") {
        let id = str_field(hypothesis, "id");
        let dir = workspace.join("experiments").join(&id);
        fs::create_dir_all(&dir)?;
        write_if_missing(
            &dir.join("HYPOTHESIS_CARD.md"),
            format_hypothesis_card(hypothesis),
        )?;
        write_if_missing(&dir.join("protocol.md"), format_protocol(hypothesis))?;
        write_if_missing(&dir.join("analysis.md"), format_analysis_stub(hypothesis))?;
    }
    for record in arr(state, "run_history") {
        if let Some(path) = record.get("evidence_path").and_then(Value::as_str) {
            write_if_missing(&workspace.join(path), format_run_record(record))?;
        }
    }
    for decision in arr(state, "decisions") {
        if let Some(path) = decision.get("note_path").and_then(Value::as_str) {
            write_if_missing(&workspace.join(path), format_reflection_note(decision))?;
        }
    }
    sync_managed_file(
        &workspace.join("literature/NOVELTY_GATE.md"),
        "",
        NOVELTY_BLOCK_START,
        NOVELTY_BLOCK_END,
        render_novelty_gate_summary(state),
    )?;
    sync_managed_file(
        &workspace.join("literature/NOVELTY_CLAIMS.md"),
        "# Novelty Claims\n\n",
        CLAIMS_BLOCK_START,
        CLAIMS_BLOCK_END,
        render_claims_summary(state),
    )?;
    sync_managed_file(
        &workspace.join("literature/NOVELTY_SEARCH_PLAN.md"),
        "# Novelty Search Plan\n\n",
        SEARCH_PLAN_BLOCK_START,
        SEARCH_PLAN_BLOCK_END,
        render_search_plan_summary(state),
    )?;
    sync_managed_file(
        &workspace.join("literature/EXTERNAL_RESEARCH.md"),
        "# External Research\n\n",
        EXTERNAL_RESEARCH_BLOCK_START,
        EXTERNAL_RESEARCH_BLOCK_END,
        render_external_research_summary(state),
    )?;
    sync_managed_file(
        &workspace.join("CURRENT_CONTEXT.md"),
        "# Current Context\n\n",
        CONTEXT_BLOCK_START,
        CONTEXT_BLOCK_END,
        render_current_context_summary(state),
    )?;
    sync_managed_file(
        &workspace.join("findings.md"),
        "",
        FINDINGS_BLOCK_START,
        FINDINGS_BLOCK_END,
        render_findings_summary(state),
    )?;
    Ok(())
}

fn format_status(state: &Value) -> String {
    let mut lines = vec![
        format!("project: {}", str_key(state, "project")),
        format!("stage: {}", str_key(state, "stage")),
        format!("status: {}", str_key(state, "status")),
        format!("mode: {}", str_key(state, "mode")),
        format!(
            "active_hypothesis: {}",
            state
                .get("active_hypothesis")
                .and_then(Value::as_str)
                .unwrap_or("-")
        ),
        format!("novelty_gate: {}", novelty_str(state, "status", "-")),
        format!("git: {}", summarize_git_provenance(state.get("git"))),
        format!(
            "environment: {}",
            summarize_environment_fingerprint(state.get("environment"))
        ),
        format!("hypotheses: {}", arr(state, "hypotheses").len()),
        format!("runs: {}", arr(state, "run_history").len()),
        format!(
            "external_research: {}",
            arr(state, "external_research").len()
        ),
        format!("blockers: {}", arr(state, "blockers").len()),
        "next_actions:".into(),
    ];
    for action in state
        .get("next_actions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .take(4)
    {
        lines.push(format!("- {}", action.as_str().unwrap_or("")));
    }
    lines.join("\n")
}

fn format_resume(state: &Value) -> String {
    let active_id = state.get("active_hypothesis").and_then(Value::as_str);
    let freshness = state_freshness(state);
    let latest_run = freshness.recent_runs.first();
    let latest_decision = freshness.recent_decisions.first();
    let brief = current_brief(state);
    let mut lines = vec![
        format!("question: {}", str_key(state, "question")),
        format!("stage: {}", str_key(state, "stage")),
        format!("novelty_gate: {}", novelty_str(state, "status", "-")),
        format!("novelty_assessment: {}", overall_novelty_assessment(state)),
        format!(
            "freshness: {}",
            if freshness.stale { "stale" } else { "fresh" }
        ),
        format!(
            "history_bias_risk: {}",
            if freshness.history_bias_risk {
                "high"
            } else {
                "low"
            }
        ),
        format!(
            "recommended_focus: {}",
            current_recommended_focus(state).unwrap_or_else(|| "-".into())
        ),
        format!(
            "novelty_brief_claim: {}",
            brief
                .as_ref()
                .and_then(|item| item.get("claim_id"))
                .and_then(Value::as_str)
                .unwrap_or("-")
        ),
        format!("active_hypothesis: {}", active_id.unwrap_or("-")),
        format!("git: {}", summarize_git_provenance(state.get("git"))),
        format!(
            "environment: {}",
            summarize_environment_fingerprint(state.get("environment"))
        ),
    ];
    if let Some(active_id) = active_id {
        if let Some(hypothesis) = find_hypothesis(state, active_id) {
            lines.push(format!(
                "active_claim: {}",
                str_field_default(hypothesis, "claim", "-")
            ));
        }
    }
    if let Some(run) = latest_run {
        lines.push(format!(
            "latest_run: {} ({})",
            str_field(run, "run_id"),
            str_field(run, "outcome")
        ));
        lines.push(format!(
            "latest_summary: {}",
            str_field_default(run, "summary", "-")
        ));
        lines.push(format!(
            "latest_run_git: {}",
            summarize_git_provenance(run.get("git_provenance"))
        ));
        lines.push(format!(
            "latest_run_env: {}",
            summarize_environment_fingerprint(run.get("environment_fingerprint"))
        ));
    }
    if let Some(decision) = latest_decision {
        lines.push(format!(
            "latest_direction: {}",
            str_field_default(decision, "direction", "-")
        ));
        lines.push(format!(
            "latest_reason: {}",
            str_field_default(decision, "reason", "-")
        ));
    }
    lines.push(format!(
        "draft_claims: {}",
        novelty_arr(state, "draft_claims").len()
    ));
    lines.push(format!(
        "search_plan_entries: {}",
        current_search_plan(state).len()
    ));
    lines.push(format!(
        "external_research_entries: {}",
        arr(state, "external_research").len()
    ));
    lines.push("guardrail: trust CURRENT_CONTEXT.md and research-state.yaml first; treat older logs as background.".into());
    lines.push("next_actions:".into());
    for action in state
        .get("next_actions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .take(3)
    {
        lines.push(format!("- {}", action.as_str().unwrap_or("")));
    }
    lines.join("\n")
}
