//! `autoresearch` CLI — research workspace lifecycle management.
//!
//! Dissolved from `tools/autoresearch-rs/`. Calls `research_harness::*` for all business logic.

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

// ── CLI Definition ──

#[derive(Parser)]
#[command(name = "autoresearch", version, about = "Research workspace CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new research workspace
    Init {
        project: String,
        question: String,
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,
        #[arg(short, long, default_value = "quick")]
        mode: String,
    },
    /// Show workspace status
    Status {
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
    /// Show recommended next actions
    Next {
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
    /// Resume workspace (compact status)
    Resume {
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
    /// Sync workspace files
    Sync {
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
    /// Draft claims from a research question
    DraftClaims {
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
        #[arg(short, long)]
        question: Option<String>,
        #[arg(short, long, default_value_t = 5)]
        count: usize,
    },
    /// Show the current search plan
    PlanSearch {
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
    /// Research a single claim externally
    ResearchClaim {
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
        #[arg(long)]
        claim_id: Option<String>,
        #[arg(short, long)]
        query: Option<String>,
        #[arg(short, long, default_value = "all")]
        source: String,
        #[arg(short, long, default_value_t = 5)]
        limit: usize,
        #[arg(long, default_value_t = 20)]
        timeout_secs: u64,
    },
    /// Research all claims in parallel
    ResearchAll {
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
        #[arg(short, long, default_value = "all")]
        source: String,
        #[arg(short, long, default_value_t = 5)]
        limit: usize,
        #[arg(long, default_value_t = 5)]
        max_claims: usize,
        #[arg(long, default_value_t = 20)]
        timeout_secs: u64,
    },
    /// Generate novelty gate recommendation from research
    GateFromResearch {
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
        #[arg(long, default_value_t = 3)]
        min_results: usize,
        #[arg(long)]
        apply: bool,
    },
    /// Show the novelty brief for the first claim
    BriefFirstClaim {
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
    /// Add a claim comparison record
    CompareClaim {
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
        #[arg(long)]
        claim: String,
        #[arg(short, long)]
        axis: String,
        #[arg(long)]
        closest_prior_work: String,
        #[arg(short, long)]
        overlap: String,
        #[arg(short, long)]
        difference: String,
        #[arg(short, long)]
        confidence: String,
        #[arg(short, long)]
        verdict: String,
        #[arg(long)]
        claim_id: Option<String>,
    },
    /// Add a new hypothesis
    AddHypothesis {
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
        #[arg(short, long)]
        claim: String,
        #[arg(short, long)]
        prediction: Option<String>,
        #[arg(long)]
        mechanism: Option<String>,
        #[arg(long)]
        falsifiable_prediction: Option<String>,
        #[arg(long)]
        success_threshold: Option<String>,
        #[arg(long)]
        stop_condition: Option<String>,
        #[arg(long)]
        baselines: Vec<String>,
        #[arg(long)]
        confounders: Vec<String>,
        #[arg(long)]
        negative_signals: Vec<String>,
        #[arg(long)]
        minimal_test: Option<String>,
        #[arg(short = 'P', long, default_value = "medium")]
        priority: String,
        #[arg(long)]
        id: Option<String>,
    },
    /// Record an experiment run
    RecordRun {
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
        #[arg(long)]
        hypothesis_id: String,
        #[arg(short, long)]
        outcome: String,
        #[arg(short, long)]
        summary: String,
        #[arg(long)]
        metric_name: Option<String>,
        #[arg(long)]
        metric_value: Option<String>,
        #[arg(long)]
        entry_command: Option<String>,
        #[arg(long)]
        evidence_path: Option<String>,
        #[arg(long)]
        sanity_checks: Vec<String>,
        #[arg(long)]
        baseline_result: Option<String>,
        #[arg(long)]
        rules_in: Vec<String>,
        #[arg(long)]
        rules_out: Vec<String>,
        #[arg(long)]
        alternative_explanations: Vec<String>,
        #[arg(long)]
        threats: Vec<String>,
        #[arg(long)]
        interpretation: Option<String>,
        #[arg(long)]
        finding: Option<String>,
        #[arg(long)]
        decision_delta: Option<String>,
        #[arg(long)]
        reuse_note: Option<String>,
        #[arg(long)]
        applies_to: Vec<String>,
        #[arg(long)]
        does_not_apply_to: Vec<String>,
        #[arg(long)]
        override_novelty_gate: bool,
        #[arg(long)]
        override_reason: Option<String>,
    },
    /// Annotate a run with reuse metadata
    AnnotateRun {
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
        #[arg(long)]
        run_id: String,
        #[arg(long)]
        finding: Option<String>,
        #[arg(long)]
        decision_delta: Option<String>,
        #[arg(long)]
        reuse_note: Option<String>,
        #[arg(long)]
        applies_to: Vec<String>,
        #[arg(long)]
        does_not_apply_to: Vec<String>,
    },
    /// Audit reuse annotations
    AuditReuse {
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    /// Record a reflection (DEEPEN/BROADEN/PIVOT/CONCLUDE)
    Reflect {
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
        #[arg(long)]
        hypothesis_id: String,
        #[arg(short, long)]
        direction: String,
        #[arg(short, long)]
        reason: String,
        #[arg(long)]
        next_step: Option<String>,
        #[arg(long)]
        activate_hypothesis: Option<String>,
    },
    /// Set novelty gate status
    SetNoveltyGate {
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
        #[arg(short, long)]
        status: String,
        #[arg(short, long)]
        decision: Option<String>,
        #[arg(long)]
        overlap_summary: Option<String>,
        #[arg(long)]
        differentiation_strategy: Option<String>,
        #[arg(short, long)]
        claims: Vec<String>,
    },
    /// Barrier escalation (loop bridge)
    Barrier {
        #[arg(short, long)]
        workspace: Option<PathBuf>,
        #[arg(short, long)]
        problem: String,
        #[arg(long)]
        loop_id: Option<String>,
        #[arg(long)]
        run_id: Option<String>,
        #[arg(long)]
        action_id: Option<String>,
        #[arg(long, default_value_t = 3)]
        consecutive_failures: u32,
    },
}

// ── Helpers ──

fn resolve_workspace(path: &PathBuf) -> Result<PathBuf> {
    let candidate = if path.is_absolute() {
        path.clone()
    } else {
        std::env::current_dir()?.join(path)
    };
    let candidate = std::fs::canonicalize(&candidate).unwrap_or(candidate);
    if candidate.is_file() {
        Ok(candidate
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf())
    } else {
        Ok(candidate)
    }
}

fn state_path(workspace: &Path) -> PathBuf {
    workspace.join("research-state.yaml")
}

fn load_state(workspace: &Path) -> Result<Value> {
    let path = state_path(workspace);
    research_harness::state::load_state(&path)
}

fn dump_state(workspace: &Path, state: &Value) -> Result<()> {
    let path = state_path(workspace);
    research_harness::state::dump_state(&path, state)
}

fn parse_source(s: &str) -> research_harness::search::ExternalSourceArg {
    match s.to_lowercase().as_str() {
        "semantic-scholar" | "semanticscholar" | "ss" => {
            research_harness::search::ExternalSourceArg::SemanticScholar
        }
        "arxiv" => research_harness::search::ExternalSourceArg::Arxiv,
        _ => research_harness::search::ExternalSourceArg::All,
    }
}

// ── Command Implementations ──

fn cmd_init(project: &str, question: &str, dir: &Path, mode: &str) -> Result<()> {
    let workspace = research_harness::workspace::init_workspace(project, question, dir, mode)?;
    println!("Workspace initialized at: {}", workspace.display());
    Ok(())
}

fn cmd_status(workspace: &PathBuf) -> Result<()> {
    let ws = resolve_workspace(workspace)?;
    let state = load_state(&ws)?;
    // Capture provenance
    let git = research_harness::provenance::capture_git_provenance(&ws);
    let env = research_harness::provenance::capture_environment_fingerprint(&ws);
    let mut state = state;
    let state_obj = state
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("state must be an object after load"))?;
    state_obj.insert("git".into(), git);
    state_obj.insert("environment".into(), env);
    println!("{}", research_harness::render::format_status(&state));
    Ok(())
}

fn cmd_next(workspace: &PathBuf) -> Result<()> {
    let ws = resolve_workspace(workspace)?;
    let state = load_state(&ws)?;
    let actions = research_harness::claims::lifecycle::recommend_next_actions(&state);
    for action in &actions {
        println!("- {action}");
    }
    Ok(())
}

fn cmd_resume(workspace: &PathBuf) -> Result<()> {
    let ws = resolve_workspace(workspace)?;
    let state = load_state(&ws)?;
    println!("{}", research_harness::render::format_resume(&state));
    Ok(())
}

fn cmd_sync(workspace: &PathBuf) -> Result<()> {
    let ws = resolve_workspace(workspace)?;
    let state = load_state(&ws)?;
    // Sync findings
    research_harness::render::sync_managed_file(
        &ws.join("findings.md"),
        "",
        research_harness::render::FINDINGS_BLOCK_START,
        research_harness::render::FINDINGS_BLOCK_END,
        research_harness::render::render_findings_summary(&state),
    )?;
    // Sync novelty gate
    research_harness::render::sync_managed_file(
        &ws.join("literature/NOVELTY_GATE.md"),
        "# Novelty Gate\n\n",
        research_harness::render::NOVELTY_BLOCK_START,
        research_harness::render::NOVELTY_BLOCK_END,
        research_harness::render::render_novelty_gate_summary(&state),
    )?;
    println!("Workspace synced.");
    Ok(())
}

fn cmd_draft_claims(
    workspace: &PathBuf,
    question: Option<String>,
    count: usize,
) -> Result<()> {
    let ws = resolve_workspace(workspace)?;
    let state = load_state(&ws)?;
    let next = research_harness::search::research::draft_claims_from_state(
        &state,
        question.as_deref(),
        count,
    );
    dump_state(&ws, &next)?;
    let drafts = next
        .get("novelty_gate")
        .and_then(|g| g.get("draft_claims"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    println!("Drafted {} claims:", drafts.len());
    for draft in &drafts {
        println!(
            "  {}: {} [{}]",
            draft
                .get("claim_id")
                .and_then(Value::as_str)
                .unwrap_or("?"),
            draft.get("claim").and_then(Value::as_str).unwrap_or("-"),
            draft.get("axis").and_then(Value::as_str).unwrap_or("-")
        );
    }
    Ok(())
}

fn cmd_plan_search(workspace: &PathBuf) -> Result<()> {
    let ws = resolve_workspace(workspace)?;
    let state = load_state(&ws)?;
    println!(
        "{}",
        research_harness::render::render_search_plan_summary(&state)
    );
    Ok(())
}

fn cmd_research_claim(
    workspace: &PathBuf,
    claim_id: Option<String>,
    query: Option<String>,
    source: &str,
    limit: usize,
    timeout_secs: u64,
) -> Result<()> {
    let ws = resolve_workspace(workspace)?;
    let state = load_state(&ws)?;
    let source = parse_source(source);
    let result = research_harness::search::research::research_claim(
        &state,
        claim_id.as_deref(),
        query.as_deref(),
        &source,
        limit,
        timeout_secs,
    )?;
    let next = research_harness::search::research::add_external_research(&state, result.clone());
    dump_state(&ws, &next)?;
    let count = research_harness::search::research::external_research_result_count(&result);
    println!(
        "Research complete: {} results for query '{}'",
        count,
        result.get("query").and_then(Value::as_str).unwrap_or("-")
    );
    Ok(())
}

fn cmd_research_all(
    workspace: &PathBuf,
    source: &str,
    limit: usize,
    max_claims: usize,
    timeout_secs: u64,
) -> Result<()> {
    let ws = resolve_workspace(workspace)?;
    let state = load_state(&ws)?;
    let source = parse_source(source);
    let next = research_harness::search::research::research_all_claims(
        &state,
        &source,
        limit,
        max_claims,
        timeout_secs,
    )?;
    dump_state(&ws, &next)?;
    let count = next
        .get("external_research")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    println!("Batch research complete: {count} total entries.");
    Ok(())
}

fn cmd_gate_from_research(
    workspace: &PathBuf,
    min_results: usize,
    apply: bool,
) -> Result<()> {
    let ws = resolve_workspace(workspace)?;
    let state = load_state(&ws)?;
    let rec =
        research_harness::search::research::novelty_gate_recommendation_from_research(
            &state,
            min_results,
        );
    println!(
        "{}",
        research_harness::search::research::format_gate_recommendation(&rec)
    );
    if apply {
        let next =
            research_harness::search::research::apply_novelty_gate_recommendation(
                &state, &rec,
            );
        dump_state(&ws, &next)?;
        println!("\nRecommendation applied.");
    }
    Ok(())
}

fn cmd_brief_first_claim(workspace: &PathBuf) -> Result<()> {
    let ws = resolve_workspace(workspace)?;
    let state = load_state(&ws)?;
    if let Some(brief) = research_harness::search::strategy::current_brief(&state) {
        println!(
            "claim: {} — {}",
            brief
                .get("claim_id")
                .and_then(Value::as_str)
                .unwrap_or("?"),
            brief.get("claim").and_then(Value::as_str).unwrap_or("-")
        );
        println!(
            "priority: {}",
            brief
                .get("priority_label")
                .and_then(Value::as_str)
                .unwrap_or("-")
        );
        println!(
            "decision_goal: {}",
            brief
                .get("decision_goal")
                .and_then(Value::as_str)
                .unwrap_or("-")
        );
    } else {
        println!("No claims available. Run draft-claims first.");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_compare_claim(
    workspace: &PathBuf,
    claim: &str,
    axis: &str,
    closest_prior_work: &str,
    overlap: &str,
    difference: &str,
    confidence: &str,
    verdict: &str,
    claim_id: Option<String>,
) -> Result<()> {
    let ws = resolve_workspace(workspace)?;
    let state = load_state(&ws)?;
    let next = research_harness::claims::lifecycle::add_claim_comparison(
        &state,
        claim,
        axis,
        closest_prior_work,
        overlap,
        difference,
        confidence,
        verdict,
        claim_id.as_deref(),
    )?;
    dump_state(&ws, &next)?;
    println!("Claim comparison recorded.");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_add_hypothesis(
    workspace: &PathBuf,
    claim: &str,
    prediction: Option<String>,
    mechanism: Option<String>,
    falsifiable_prediction: Option<String>,
    success_threshold: Option<String>,
    stop_condition: Option<String>,
    baselines: &[String],
    confounders: &[String],
    negative_signals: &[String],
    minimal_test: Option<String>,
    priority: &str,
    id: Option<String>,
) -> Result<()> {
    let ws = resolve_workspace(workspace)?;
    let state = load_state(&ws)?;
    let next = research_harness::claims::lifecycle::add_hypothesis(
        &state,
        research_harness::claims::lifecycle::HypothesisInput {
            claim,
            prediction: prediction.as_deref(),
            mechanism: mechanism.as_deref(),
            falsifiable_prediction: falsifiable_prediction.as_deref(),
            success_threshold: success_threshold.as_deref(),
            stop_condition: stop_condition.as_deref(),
            baselines,
            confounders,
            negative_signals,
            minimal_test: minimal_test.as_deref(),
            priority,
            hypothesis_id: id.as_deref(),
        },
    )?;
    dump_state(&ws, &next)?;
    println!("Hypothesis added.");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_record_run(
    workspace: &PathBuf,
    hypothesis_id: &str,
    outcome: &str,
    summary: &str,
    metric_name: Option<String>,
    metric_value: Option<String>,
    entry_command: Option<String>,
    evidence_path: Option<String>,
    sanity_checks: &[String],
    baseline_result: Option<String>,
    rules_in: &[String],
    rules_out: &[String],
    alternative_explanations: &[String],
    threats: &[String],
    interpretation: Option<String>,
    finding: Option<String>,
    decision_delta: Option<String>,
    reuse_note: Option<String>,
    applies_to: &[String],
    does_not_apply_to: &[String],
    override_novelty_gate: bool,
    override_reason: Option<String>,
) -> Result<()> {
    let ws = resolve_workspace(workspace)?;
    let state = load_state(&ws)?;
    let next = research_harness::claims::lifecycle::record_run(
        &state,
        &research_harness::claims::lifecycle::RecordRunInput {
            hypothesis_id,
            outcome,
            summary,
            metric_name: metric_name.as_deref(),
            metric_value: metric_value.as_deref(),
            command: entry_command.as_deref(),
            evidence_path: evidence_path.as_deref(),
            sanity_checks,
            baseline_result: baseline_result.as_deref(),
            rules_in,
            rules_out,
            alternative_explanations,
            threats,
            interpretation: interpretation.as_deref(),
            finding: finding.as_deref(),
            decision_delta: decision_delta.as_deref(),
            reuse_note: reuse_note.as_deref(),
            applies_to,
            does_not_apply_to,
            override_novelty_gate,
            override_reason: override_reason.as_deref(),
        },
        &ws,
    )?;
    dump_state(&ws, &next)?;
    println!("Run recorded for hypothesis {hypothesis_id}.");
    Ok(())
}

fn cmd_annotate_run(
    workspace: &PathBuf,
    run_id: &str,
    finding: Option<String>,
    decision_delta: Option<String>,
    reuse_note: Option<String>,
    applies_to: &[String],
    does_not_apply_to: &[String],
) -> Result<()> {
    let ws = resolve_workspace(workspace)?;
    let state = load_state(&ws)?;
    let next = research_harness::claims::lifecycle::annotate_run(
        &state,
        run_id,
        research_harness::claims::lifecycle::RunAnnotationInput {
            finding: finding.as_deref(),
            decision_delta: decision_delta.as_deref(),
            reuse_note: reuse_note.as_deref(),
            applies_to,
            does_not_apply_to,
        },
    )?;
    dump_state(&ws, &next)?;
    println!("Run {run_id} annotated.");
    Ok(())
}

fn cmd_audit_reuse(workspace: &PathBuf, _apply: bool) -> Result<()> {
    let ws = resolve_workspace(workspace)?;
    let state = load_state(&ws)?;
    let audit = research_harness::claims::lifecycle::reuse_audit(&state);
    println!(
        "Total runs: {}",
        audit.get("runs").unwrap_or(&json!(0))
    );
    println!(
        "Reusable runs: {}",
        audit.get("reusable_runs").unwrap_or(&json!(0))
    );
    println!(
        "Missing annotations: {}",
        audit.get("missing_annotations").unwrap_or(&json!(0))
    );
    if let Some(missing) = audit.get("missing_runs").and_then(Value::as_array) {
        for run in missing {
            println!(
                "  - {}: {}",
                run.get("run_id").and_then(Value::as_str).unwrap_or("?"),
                run.get("summary").and_then(Value::as_str).unwrap_or("-")
            );
        }
    }
    Ok(())
}

fn cmd_reflect(
    workspace: &PathBuf,
    hypothesis_id: &str,
    direction: &str,
    reason: &str,
    next_step: Option<String>,
    activate_hypothesis: Option<String>,
) -> Result<()> {
    let ws = resolve_workspace(workspace)?;
    let state = load_state(&ws)?;
    let next = research_harness::claims::lifecycle::reflect(
        &state,
        hypothesis_id,
        direction,
        reason,
        next_step.as_deref(),
        activate_hypothesis.as_deref(),
    )?;
    dump_state(&ws, &next)?;
    println!("Reflection recorded: {direction}");
    Ok(())
}

fn cmd_set_novelty_gate(
    workspace: &PathBuf,
    status: &str,
    decision: Option<String>,
    overlap_summary: Option<String>,
    differentiation_strategy: Option<String>,
    claims: &[String],
) -> Result<()> {
    let ws = resolve_workspace(workspace)?;
    let state = load_state(&ws)?;
    let mut next = state;
    {
        let next_obj = next
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("state must be an object after load"))?;
        let gate = next_obj
            .entry("novelty_gate")
            .or_insert(json!({}));
        let gate_obj = gate.as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("novelty_gate must be an object"))?;
        gate_obj.insert("status".into(), json!(status));
        if let Some(d) = decision {
            gate_obj.insert("decision".into(), json!(d));
        }
        if let Some(os) = overlap_summary {
            gate_obj.insert("overlap_summary".into(), json!(os));
        }
        if let Some(ds) = differentiation_strategy {
            gate_obj.insert("differentiation_strategy".into(), json!(ds));
        }
        if !claims.is_empty() {
            gate_obj.insert("claims".into(), json!(claims));
        }
    }
    dump_state(&ws, &next)?;
    println!("Novelty gate set to: {status}");
    Ok(())
}

fn cmd_barrier(
    workspace: &PathBuf,
    problem: &str,
    loop_id: Option<&str>,
    run_id: Option<&str>,
    action_id: Option<&str>,
    consecutive_failures: u32,
) -> Result<()> {
    let ws = resolve_workspace(workspace)?;
    // Initialize workspace for barrier research
    let barrier_dir = ws.join("barrier-research");
    let state = research_harness::claims::lifecycle::default_state(
        "barrier",
        problem,
        "barrier",
    );
    std::fs::create_dir_all(&barrier_dir)?;
    research_harness::state::dump_state(&barrier_dir.join("research-state.yaml"), &state)?;
    // Draft claims from the barrier problem
    let with_claims =
        research_harness::search::research::draft_claims_from_state(&state, Some(problem), 3);
    research_harness::state::dump_state(
        &barrier_dir.join("research-state.yaml"),
        &with_claims,
    )?;
    // Write barrier report
    let drafted_claims = with_claims.get("novelty_gate")
        .and_then(|g| g.get("draft_claims"))
        .cloned()
        .unwrap_or(json!([]));
    let candidates: Vec<String> = drafted_claims
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|c| {
                    c.as_str()
                        .map(|s| s.to_string())
                        .or_else(|| c.get("claim").and_then(|v| v.as_str()).map(|s| s.to_string()))
                })
                .collect()
        })
        .unwrap_or_default();
    let report = json!({
        "barrier_id": format!("br-{}", chrono::Utc::now().timestamp_millis()),
        "problem": problem,
        "loop_id": loop_id,
        "run_id": run_id,
        "action_id": action_id,
        "consecutive_failures": consecutive_failures,
        "workspace": barrier_dir.display().to_string(),
        "drafted_claims": drafted_claims,
        "candidates": candidates,
        "created_at": framework_kernel::time::now_iso(),
    });
    let report_path = ws.join(format!(
        "artifacts/research-barrier/{}/BARRIER_REPORT.json",
        report.get("barrier_id").and_then(Value::as_str).unwrap_or("unknown")
    ));
    if let Some(parent) = report_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;
    println!("Barrier report written to: {}", report_path.display());
    Ok(())
}

// ── Entry Point ──

fn main() -> Result<()> {
    // Note: tempfile::NamedTempFile auto-deletes on drop, so atomic writes are safe
    // even if process is interrupted (Ctrl+C or kill)
    let cli = Cli::parse();
    match cli.command {
        Commands::Init {
            project,
            question,
            dir,
            mode,
        } => cmd_init(&project, &question, &dir, &mode)?,
        Commands::Status { workspace } => cmd_status(&workspace)?,
        Commands::Next { workspace } => cmd_next(&workspace)?,
        Commands::Resume { workspace } => cmd_resume(&workspace)?,
        Commands::Sync { workspace } => cmd_sync(&workspace)?,
        Commands::DraftClaims {
            workspace,
            question,
            count,
        } => cmd_draft_claims(&workspace, question, count)?,
        Commands::PlanSearch { workspace } => cmd_plan_search(&workspace)?,
        Commands::ResearchClaim {
            workspace,
            claim_id,
            query,
            source,
            limit,
            timeout_secs,
        } => cmd_research_claim(&workspace, claim_id, query, &source, limit, timeout_secs)?,
        Commands::ResearchAll {
            workspace,
            source,
            limit,
            max_claims,
            timeout_secs,
        } => cmd_research_all(&workspace, &source, limit, max_claims, timeout_secs)?,
        Commands::GateFromResearch {
            workspace,
            min_results,
            apply,
        } => cmd_gate_from_research(&workspace, min_results, apply)?,
        Commands::BriefFirstClaim { workspace } => cmd_brief_first_claim(&workspace)?,
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
        } => cmd_compare_claim(
            &workspace,
            &claim,
            &axis,
            &closest_prior_work,
            &overlap,
            &difference,
            &confidence,
            &verdict,
            claim_id,
        )?,
        Commands::AddHypothesis {
            workspace,
            claim,
            prediction,
            mechanism,
            falsifiable_prediction,
            success_threshold,
            stop_condition,
            baselines,
            confounders,
            negative_signals,
            minimal_test,
            priority,
            id,
        } => cmd_add_hypothesis(
            &workspace,
            &claim,
            prediction,
            mechanism,
            falsifiable_prediction,
            success_threshold,
            stop_condition,
            &baselines,
            &confounders,
            &negative_signals,
            minimal_test,
            &priority,
            id,
        )?,
        Commands::RecordRun {
            workspace,
            hypothesis_id,
            outcome,
            summary,
            metric_name,
            metric_value,
            entry_command,
            evidence_path,
            sanity_checks,
            baseline_result,
            rules_in,
            rules_out,
            alternative_explanations,
            threats,
            interpretation,
            finding,
            decision_delta,
            reuse_note,
            applies_to,
            does_not_apply_to,
            override_novelty_gate,
            override_reason,
        } => cmd_record_run(
            &workspace,
            &hypothesis_id,
            &outcome,
            &summary,
            metric_name,
            metric_value,
            entry_command,
            evidence_path,
            &sanity_checks,
            baseline_result,
            &rules_in,
            &rules_out,
            &alternative_explanations,
            &threats,
            interpretation,
            finding,
            decision_delta,
            reuse_note,
            &applies_to,
            &does_not_apply_to,
            override_novelty_gate,
            override_reason,
        )?,
        Commands::AnnotateRun {
            workspace,
            run_id,
            finding,
            decision_delta,
            reuse_note,
            applies_to,
            does_not_apply_to,
        } => cmd_annotate_run(
            &workspace,
            &run_id,
            finding,
            decision_delta,
            reuse_note,
            &applies_to,
            &does_not_apply_to,
        )?,
        Commands::AuditReuse { workspace, apply } => cmd_audit_reuse(&workspace, apply)?,
        Commands::Reflect {
            workspace,
            hypothesis_id,
            direction,
            reason,
            next_step,
            activate_hypothesis,
        } => cmd_reflect(
            &workspace,
            &hypothesis_id,
            &direction,
            &reason,
            next_step,
            activate_hypothesis,
        )?,
        Commands::SetNoveltyGate {
            workspace,
            status,
            decision,
            overlap_summary,
            differentiation_strategy,
            claims,
        } => cmd_set_novelty_gate(
            &workspace,
            &status,
            decision,
            overlap_summary,
            differentiation_strategy,
            &claims,
        )?,
        Commands::Barrier {
            workspace,
            problem,
            loop_id,
            run_id,
            action_id,
            consecutive_failures,
        } => cmd_barrier(
            &workspace.unwrap_or_else(|| PathBuf::from(".")),
            &problem,
            loop_id.as_deref(),
            run_id.as_deref(),
            action_id.as_deref(),
            consecutive_failures,
        )?,
    }
    Ok(())
}
