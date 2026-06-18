use anyhow::{Context, Result};
use clap::Parser;
use regex::Regex;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

mod arg_impls;
mod cli;
use cli::*;
mod helpers;
use helpers::*;
mod text;
use text::*;
mod state;
use state::*;
mod search;
use search::*;
mod provenance;
use provenance::*;
mod research;
use research::*;
mod claims;
use claims::*;
mod render;
use render::*;
mod workspace;
use workspace::*;

const SCHEMA_VERSION: i64 = 4;
const STAGE_BOOTSTRAP: &str = "bootstrap";
const STAGE_INNER_LOOP: &str = "inner-loop";
const STAGE_OUTER_LOOP: &str = "outer-loop";
const STAGE_FINALIZE: &str = "finalize";
const STALE_STATE_DAYS: i64 = 10;
const RECENT_ACTIVITY_DAYS: i64 = 14;
const FALLBACK_ACTIVITY_LIMIT: usize = 3;
const TEMPLATES_RELATIVE: &str = "tools/autoresearch-rs/templates";
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

static ARXIV_ENTRY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<entry>(.*?)</entry>").expect("arxiv entry regex"));
static ARXIV_AUTHOR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)<author>.*?<name>(.*?)</name>.*?</author>").expect("arxiv author regex")
});
const CONTEXT_BLOCK_START: &str = "<!-- autoresearch:context:start -->";
const CONTEXT_BLOCK_END: &str = "<!-- autoresearch:context:end -->";
const REUSE_INDEX_BLOCK_START: &str = "<!-- autoresearch:reuse-index:start -->";
const REUSE_INDEX_BLOCK_END: &str = "<!-- autoresearch:reuse-index:end -->";

fn cmd_init(project: &str, question: &str, dir: &Path, mode: &str) -> Result<()> {
    let root = init_workspace(project, question, dir, mode)?;
    append_ledger_event(
        &root,
        "workspace.initialized",
        json!({ "project": project, "question": question, "mode": mode }),
    )?;
    println!("Initialized autoresearch workspace at {}", root.display());
    Ok(())
}

fn cmd_status(workspace: &Path) -> Result<()> {
    let (workspace, state_path) = ensure_workspace(workspace)?;
    let mut state = load_state(&state_path)?;
    set_key(
        &mut state,
        "environment",
        capture_environment_fingerprint(&workspace),
    );
    set_key(&mut state, "git", capture_git_provenance(&workspace));
    println!("{}", format_status(&state));
    Ok(())
}

fn cmd_next(workspace: &Path) -> Result<()> {
    let (_, state_path) = ensure_workspace(workspace)?;
    let state = load_state(&state_path)?;
    for action in recommend_next_actions(&state) {
        println!("- {action}");
    }
    Ok(())
}

fn cmd_resume(workspace: &Path) -> Result<()> {
    let (workspace, state_path) = ensure_workspace(workspace)?;
    let mut state = load_state(&state_path)?;
    set_key(
        &mut state,
        "environment",
        capture_environment_fingerprint(&workspace),
    );
    set_key(&mut state, "git", capture_git_provenance(&workspace));
    println!("{}", format_resume(&state));
    Ok(())
}

fn cmd_sync(workspace: &Path) -> Result<()> {
    let (workspace, state_path) = ensure_workspace(workspace)?;
    let state = load_state(&state_path)?;
    sync_workspace_files(&workspace, &state)?;
    append_ledger_event(
        &workspace,
        "workspace.synced",
        json!({ "runs": arr(&state, "run_history").len() }),
    )?;
    println!("Synchronized workspace files for {}", workspace.display());
    Ok(())
}

fn cmd_draft_claims(workspace: &Path, question: Option<String>, count: usize) -> Result<()> {
    let (workspace, state_path) = ensure_workspace(workspace)?;
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
    Ok(())
}

fn cmd_plan_search(workspace: &Path) -> Result<()> {
    let (workspace, state_path) = ensure_workspace(workspace)?;
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
    Ok(())
}

fn cmd_research_claim(
    workspace: &Path,
    claim_id: Option<String>,
    query: Option<String>,
    source: &ExternalSourceArg,
    limit: usize,
    timeout_secs: u64,
) -> Result<()> {
    let (workspace, state_path) = ensure_workspace(workspace)?;
    let state = load_state(&state_path)?;
    let research = research_claim(
        &state,
        claim_id.as_deref(),
        query.as_deref(),
        source,
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
    Ok(())
}

fn cmd_research_all(
    workspace: &Path,
    source: &ExternalSourceArg,
    limit: usize,
    max_claims: usize,
    timeout_secs: u64,
) -> Result<()> {
    let (workspace, state_path) = ensure_workspace(workspace)?;
    let state = load_state(&state_path)?;
    let updated = research_all_claims(&state, source, limit, max_claims, timeout_secs)?;
    let added = arr(&updated, "external_research")
        .len()
        .saturating_sub(arr(&ensure_state_defaults(&state), "external_research").len());
    dump_state(&state_path, &updated)?;
    sync_workspace_files(&workspace, &updated)?;
    append_research_log(
        &workspace,
        "External research batch recorded",
        vec![
            format!("claims searched: {added}"),
            format!("source: {}", source.as_str()),
        ],
    )?;
    append_ledger_event(
        &workspace,
        "external_research.batch_recorded",
        json!({ "claims": added, "source": source.as_str() }),
    )?;
    println!(
        "Recorded {added} external research entries for {}",
        workspace.display()
    );
    Ok(())
}

fn cmd_gate_from_research(workspace: &Path, min_results: usize, apply: bool) -> Result<()> {
    let (workspace, state_path) = ensure_workspace(workspace)?;
    let state = load_state(&state_path)?;
    let recommendation = novelty_gate_recommendation_from_research(&state, min_results);
    if apply {
        let updated = apply_novelty_gate_recommendation(&state, &recommendation);
        dump_state(&state_path, &updated)?;
        sync_workspace_files(&workspace, &updated)?;
        append_research_log(
            &workspace,
            "Novelty gate recommendation applied",
            vec![
                format!(
                    "status: {}",
                    str_field(&recommendation, "recommended_status")
                ),
                format!("decision: {}", str_field(&recommendation, "decision")),
            ],
        )?;
        append_ledger_event(
            &workspace,
            "novelty_gate.recommended_from_external_research",
            recommendation.clone(),
        )?;
        println!("{}", format_gate_recommendation(&recommendation));
    } else {
        println!("{}", format_gate_recommendation(&recommendation));
    }
    Ok(())
}

fn cmd_brief_first_claim(workspace: &Path) -> Result<()> {
    let (workspace, state_path) = ensure_workspace(workspace)?;
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
    Ok(())
}

fn cmd_compare_claim(
    workspace: &Path,
    claim: &str,
    axis: &str,
    closest_prior_work: &str,
    overlap: &OverlapArg,
    difference: &str,
    confidence: &ConfidenceArg,
    verdict: &VerdictArg,
    claim_id: Option<String>,
) -> Result<()> {
    let (workspace, state_path) = ensure_workspace(workspace)?;
    let state = load_state(&state_path)?;
    let updated = add_claim_comparison(
        &state,
        claim,
        axis,
        closest_prior_work,
        overlap.as_str(),
        difference,
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
    Ok(())
}

fn cmd_add_hypothesis(
    workspace: &Path,
    claim: &str,
    prediction: Option<String>,
    mechanism: Option<String>,
    falsifiable_prediction: Option<String>,
    success_threshold: Option<String>,
    stop_condition: Option<String>,
    baselines: &Vec<String>,
    confounders: &Vec<String>,
    negative_signals: &Vec<String>,
    minimal_test: Option<String>,
    priority: &PriorityArg,
    id: Option<String>,
) -> Result<()> {
    let (workspace, state_path) = ensure_workspace(workspace)?;
    let state = load_state(&state_path)?;
    let updated = add_hypothesis(
        &state,
        HypothesisInput {
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
            priority: priority.as_str(),
            hypothesis_id: id.as_deref(),
        },
    )?;
    dump_state(&state_path, &updated)?;
    sync_workspace_files(&workspace, &updated)?;
    let resolved_id = id.unwrap_or_else(|| slugify(claim).chars().take(40).collect());
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
    Ok(())
}

fn cmd_record_run(
    workspace: &Path,
    hypothesis_id: &str,
    outcome: &OutcomeArg,
    summary: &str,
    metric_name: Option<String>,
    metric_value: Option<String>,
    entry_command: Option<String>,
    evidence_path: Option<String>,
    sanity_checks: &Vec<String>,
    baseline_result: Option<String>,
    rules_in: &Vec<String>,
    rules_out: &Vec<String>,
    alternative_explanations: &Vec<String>,
    threats: &Vec<String>,
    interpretation: Option<String>,
    finding: Option<String>,
    decision_delta: Option<String>,
    reuse_note: Option<String>,
    applies_to: &Vec<String>,
    does_not_apply_to: &Vec<String>,
    override_novelty_gate: bool,
    override_reason: Option<String>,
) -> Result<()> {
    let (workspace, state_path) = ensure_workspace(workspace)?;
    let state = load_state(&state_path)?;
    let updated = record_run(
        &state,
        &RecordRunInput {
            hypothesis_id,
            outcome: outcome.as_str(),
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
        &workspace,
    )?;
    dump_state(&state_path, &updated)?;
    sync_workspace_files(&workspace, &updated)?;
    if let Some(record) = latest_run_for_hypothesis(&updated, hypothesis_id) {
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
                "sanity_checks": record.get("sanity_checks").cloned().unwrap_or(Value::Null),
                "baseline_result": record.get("baseline_result").cloned().unwrap_or(Value::Null),
                "rules_in": record.get("rules_in").cloned().unwrap_or(Value::Null),
                "rules_out": record.get("rules_out").cloned().unwrap_or(Value::Null),
                "alternative_explanations": record.get("alternative_explanations").cloned().unwrap_or(Value::Null),
                "threats": record.get("threats").cloned().unwrap_or(Value::Null),
                "interpretation": record.get("interpretation").cloned().unwrap_or(Value::Null),
                "finding": record.get("finding").cloned().unwrap_or(Value::Null),
                "decision_delta": record.get("decision_delta").cloned().unwrap_or(Value::Null),
                "reuse_note": record.get("reuse_note").cloned().unwrap_or(Value::Null),
                "applies_to": record.get("applies_to").cloned().unwrap_or(Value::Null),
                "does_not_apply_to": record.get("does_not_apply_to").cloned().unwrap_or(Value::Null),
                "novelty_gate_status_at_run": record.get("novelty_gate_status_at_run").cloned().unwrap_or(Value::Null),
                "novelty_gate_override": record.get("novelty_gate_override").cloned().unwrap_or(Value::Null),
                "override_reason": record.get("override_reason").cloned().unwrap_or(Value::Null),
                "environment_fingerprint": record.get("environment_fingerprint").cloned().unwrap_or(Value::Null),
                "git_provenance": record.get("git_provenance").cloned().unwrap_or(Value::Null),
            }),
        )?;
    }
    if let Some(hypothesis) = find_hypothesis(&updated, hypothesis_id) {
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
    Ok(())
}

fn cmd_annotate_run(
    workspace: &Path,
    run_id: &str,
    finding: Option<String>,
    decision_delta: Option<String>,
    reuse_note: Option<String>,
    applies_to: &Vec<String>,
    does_not_apply_to: &Vec<String>,
) -> Result<()> {
    let (workspace, state_path) = ensure_workspace(workspace)?;
    let state = load_state(&state_path)?;
    let updated = annotate_run(
        &state,
        run_id,
        RunAnnotationInput {
            finding: finding.as_deref(),
            decision_delta: decision_delta.as_deref(),
            reuse_note: reuse_note.as_deref(),
            applies_to,
            does_not_apply_to,
        },
    )?;
    dump_state(&state_path, &updated)?;
    sync_workspace_files(&workspace, &updated)?;
    append_research_log(
        &workspace,
        &format!("Run annotated for reuse ({run_id})"),
        vec![
            format!("finding: {}", finding.unwrap_or_else(|| "-".into())),
            format!(
                "decision_delta: {}",
                decision_delta.unwrap_or_else(|| "-".into())
            ),
        ],
    )?;
    append_ledger_event(
        &workspace,
        "run.annotated",
        json!({
            "run_id": run_id,
            "finding": latest_run_by_id(&updated, run_id).and_then(|run| run.get("finding")).cloned().unwrap_or(Value::Null),
            "decision_delta": latest_run_by_id(&updated, run_id).and_then(|run| run.get("decision_delta")).cloned().unwrap_or(Value::Null),
            "reuse_note": latest_run_by_id(&updated, run_id).and_then(|run| run.get("reuse_note")).cloned().unwrap_or(Value::Null),
        }),
    )?;
    println!("Annotated {run_id} for reuse");
    Ok(())
}

fn cmd_audit_reuse(workspace: &Path, apply: bool) -> Result<()> {
    let (workspace, state_path) = ensure_workspace(workspace)?;
    let state = load_state(&state_path)?;
    let audit = reuse_audit(&state);
    if apply {
        sync_managed_file(
            &workspace.join("findings-reuse-index.md"),
            "# Findings Reuse Index\n\n",
            REUSE_INDEX_BLOCK_START,
            REUSE_INDEX_BLOCK_END,
            render_reuse_index_summary(&state),
        )?;
        append_research_log(
            &workspace,
            "Reuse audit refreshed",
            vec![
                format!("reusable: {}", audit["reusable_runs"]),
                format!("missing: {}", audit["missing_annotations"]),
            ],
        )?;
        append_ledger_event(&workspace, "reuse.audit_refreshed", audit.clone())?;
        dump_state(&state_path, &state)?;
    }
    println!("{}", format_reuse_audit(&audit));
    Ok(())
}

fn cmd_reflect(
    workspace: &Path,
    hypothesis_id: &str,
    direction: &DirectionArg,
    reason: &str,
    next_step: Option<String>,
    activate_hypothesis: Option<String>,
) -> Result<()> {
    let (workspace, state_path) = ensure_workspace(workspace)?;
    let state = load_state(&state_path)?;
    let updated = reflect(
        &state,
        hypothesis_id,
        direction.as_str(),
        reason,
        next_step.as_deref(),
        activate_hypothesis.as_deref(),
    )?;
    dump_state(&state_path, &updated)?;
    sync_workspace_files(&workspace, &updated)?;
    if let Some(decision) = latest_decision_for_hypothesis(&updated, hypothesis_id) {
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
    if let Some(hypothesis) = find_hypothesis(&updated, hypothesis_id) {
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
    Ok(())
}

fn cmd_barrier(
    workspace: &Path,
    problem: &str,
    loop_id: Option<&str>,
    run_id: Option<&str>,
    action_id: Option<&str>,
    consecutive_failures: u32,
) -> Result<()> {
    let barrier_id = format!("br-{}", chrono::Utc::now().format("%Y%m%d%H%M%S"));
    let repo_root = repo_root()?;
    let barrier_root = if workspace.as_os_str() == "." {
        repo_root.join("artifacts").join("research-barrier").join(&barrier_id)
    } else {
        workspace.to_path_buf()
    };
    fs::create_dir_all(&barrier_root)
        .with_context(|| format!("create barrier dir: {}", barrier_root.display()))?;

    let project_name = loop_id.unwrap_or("barrier-research");
    let ws_root = init_workspace(project_name, problem, &barrier_root, "full")
        .with_context(|| "barrier workspace init failed")?;
    let state_path = ws_root.join("research-state.yaml");

    // Load state and set current_direction
    let mut state = load_state(&state_path)?;
    set_key(&mut state, "current_direction", json!({
        "original_question": problem,
        "last_reaffirmed": now_iso(),
        "deviation_warning_count": 0,
    }));
    state = ensure_state_defaults(&state);
    dump_state(&state_path, &state)?;

    // Draft claims from barrier problem
    let updated = draft_claims_from_state(&state, Some(problem), 3);
    dump_state(&state_path, &updated)?;

    // Quick external research on top draft claims
    for claim in novelty_arr(&updated, "draft_claims").iter().take(2) {
        let claim_text = str_field(claim, "claim");
        if let Ok(research) = research_claim(
            &updated, None, Some(&claim_text), &crate::ExternalSourceArg::All, 3, 20,
        ) {
            let _ = add_external_research(&updated, research);
        }
    }
    // Persist external research results into state before building report
    dump_state(&state_path, &updated)?;

    // Build candidates with evidence from external research
    let draft_claims = novelty_arr(&updated, "draft_claims");
    let candidates: Vec<Value> = draft_claims.iter().map(|c| {
        let claim_id = c.get("id").and_then(Value::as_str).unwrap_or("");
        let claim_text = str_field(c, "claim");

        // Gather evidence from external research entries for this claim
        let research_entries = external_research_entries_for_claim(&updated, claim_id);
        let mut evidence: Vec<Value> = Vec::new();
        for entry in research_entries {
            let results = entry.get("results").and_then(Value::as_array);
            if let Some(papers) = results {
                for paper in papers.iter().take(5) {
                    let title = str_field(paper, "title");
                    let url = str_field(paper, "url");
                    let authors = str_field(paper, "authors");
                    let source = str_field(entry, "source");
                    evidence.push(json!({
                        "title": if title.is_empty() { "_untitled_" } else { &title },
                        "url": url,
                        "authors": authors,
                        "source": source,
                    }));
                }
            }
        }

        json!({
            "id": c.get("id"),
            "hypothesis": claim_text,
            "confidence": c.get("confidence").or(Some(&json!("medium"))),
            "evidence": evidence,
            "evidence_count": evidence.len(),
            "expected_effort": "unknown",
            "risk": "unknown",
        })
    }).collect();

    let barrier_report = json!({
        "schema_version": "barrier-report-v1",
        "barrier_id": barrier_id,
        "barrier": problem,
        "context": {
            "loop_id": loop_id,
            "run_id": run_id,
            "action_id": action_id,
            "consecutive_failures": consecutive_failures,
        },
        "candidates": candidates,
        "recommended": novelty_arr(&updated, "draft_claims").first()
            .and_then(|c| c.get("id")),
        "generated_at": now_iso(),
    });

    let report_path = barrier_root.join("BARRIER_REPORT.json");
    fs::write(&report_path, serde_json::to_string_pretty(&barrier_report)?)
        .with_context(|| format!("write BARRIER_REPORT: {}", report_path.display()))?;

    sync_workspace_files(&ws_root, &updated)?;

    append_ledger_event(&ws_root, "barrier.escalated", json!({
        "barrier_id": barrier_id,
        "problem": problem,
        "loop_id": loop_id,
        "run_id": run_id,
        "action_id": action_id,
        "consecutive_failures": consecutive_failures,
    }))?;

    // Output barrier report JSON to stdout for loop runner
    println!("{}", serde_json::to_string_pretty(&barrier_report)?);
    eprintln!("BARRIER_REPORT: {}", report_path.display());
    Ok(())
}

fn cmd_log_record(
    workspace: &Path,
    direction: &str,
    question: &str,
    entry_point: &str,
    barrier_id: Option<&str>,
) -> Result<()> {
    let (workspace, _) = ensure_workspace(workspace)?;
    // Use workspace-local research-log directory
    let log_root = workspace.join("research-log");

    // Ensure log workspace exists
    research_log_rs::init_log_workspace(&log_root)
        .context("failed to init research-log workspace")?;

    let now = chrono::Utc::now();
    let log_id = format!("rl-{}", now.format("%Y%m%d%H%M%S"));

    // DB layer
    let db_path = log_root.join("research-log.db");
    let conn = research_log_rs::db::init_database(&db_path)?;

    let entry = research_log_rs::models::Entry {
        id: log_id.clone(),
        direction: direction.to_string(),
        question: question.to_string(),
        context: None,
        entry_point: entry_point.to_string(),
        barrier_id: barrier_id.map(String::from),
        importance: 0,
        status: research_log_rs::models::STATUS_ACTIVE.to_string(),
        created_at: now.to_rfc3339(),
        updated_at: now.to_rfc3339(),
    };
    research_log_rs::db::insert_entry(&conn, &entry)?;

    // Also write to legacy research-log.md for backward compatibility
    append_research_log(
        &workspace,
        &format!("Log recorded: {}", direction),
        vec![
            format!("id: {}", log_id),
            format!("question: {}", question),
        ],
    )?;

    println!("Recorded research log entry: {}", log_id);
    Ok(())
}

fn cmd_log_search(workspace: &Path, query: &str, limit: usize) -> Result<()> {
    let (workspace, _) = ensure_workspace(workspace)?;
    let log_root = workspace.join("research-log");
    let db_path = log_root.join("research-log.db");

    if !db_path.exists() {
        println!("No research log database found at {}. Run `log:record` first.", db_path.display());
        return Ok(());
    }

    let conn = research_log_rs::db::init_database(&db_path)?;
    let results = research_log_rs::db::search_entries(&conn, query, None, None, None, None, limit)?;

    if results.is_empty() {
        println!("No results for: {}", query);
        return Ok(());
    }

    println!("Search results for \"{}\" ({} found):", query, results.len());
    for r in &results {
        println!("  [{:.30}] {}: {} (score: {:.2})", r.id, r.direction, r.snippet, r.score);
    }
    Ok(())
}

fn cmd_log_insight(workspace: &Path, log_id: &str, text: &str, confidence: &str) -> Result<()> {
    let (workspace, _) = ensure_workspace(workspace)?;
    let log_root = workspace.join("research-log");
    let db_path = log_root.join("research-log.db");
    let conn = research_log_rs::db::init_database(&db_path)?;

    let confidence_val: f64 = match confidence {
        "high" => 0.9,
        "medium" => 0.6,
        "low" => 0.3,
        _ => 0.5,
    };

    let finding = research_log_rs::models::Finding {
        id: 0,
        entry_id: log_id.to_string(),
        kind: research_log_rs::models::FINDING_KIND_INSIGHT.to_string(),
        content: text.to_string(),
        confidence: Some(confidence_val),
        metadata: None,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    research_log_rs::db::insert_finding(&conn, &finding)?;

    // Also append to legacy log
    append_research_log(
        &workspace,
        &format!("Insight for {}", log_id),
        vec![format!("confidence: {}", confidence), text.to_string()],
    )?;

    println!("Added insight to log entry: {}", log_id);
    Ok(())
}

fn cmd_log_connect(workspace: &Path, log_id_a: &str, log_id_b: &str, relation: Option<String>) -> Result<()> {
    let (workspace, _) = ensure_workspace(workspace)?;
    let log_root = workspace.join("research-log");
    let db_path = log_root.join("research-log.db");
    let conn = research_log_rs::db::init_database(&db_path)?;

    let log_conn = research_log_rs::models::LogConnection {
        id: 0,
        entry_id_a: log_id_a.to_string(),
        entry_id_b: log_id_b.to_string(),
        relation,
        weight: 1.0,
        confidence: None,
        notes: None,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    research_log_rs::db::insert_connection(&conn, &log_conn)?;
    println!("Connected log entries: {} <-> {}", log_id_a, log_id_b);
    Ok(())
}

fn cmd_log_neighbors(workspace: &Path, entry_id: &str, relation_filter: Option<&str>, limit: usize) -> Result<()> {
    let (workspace, _) = ensure_workspace(workspace)?;
    let log_root = workspace.join("research-log");
    let db_path = log_root.join("research-log.db");
    let conn = research_log_rs::db::init_database(&db_path)?;
    let g = research_log_rs::graph::load_full_graph(&conn)?;

    match research_log_rs::db::get_entry(&conn, entry_id)? {
        Some(ref entry) => {
            let filter: Option<Vec<&str>> = relation_filter
                .map(|r| r.split(',').map(|s| s.trim()).collect());
            let neighbors = research_log_rs::graph::get_neighbors(&g, entry_id, filter.as_deref());
            println!("Neighbors of [{}] {}: {}", entry.id, entry.direction, entry.question);
            println!("  ({} connection(s))", neighbors.len());
            for (nid, rel, _w, conf) in neighbors.iter().take(limit) {
                match research_log_rs::db::get_entry(&conn, nid)? {
                    Some(e) => println!("  {} --[{}{}]--> [{}] {}",
                        entry_id,
                        rel.unwrap_or("related"),
                        conf.map(|c| format!(" conf={}", c)).unwrap_or_default(),
                        e.id,
                        e.question.chars().take(60).collect::<String>(),
                    ),
                    None => println!("  {} --[{}]--> {}", entry_id, rel.unwrap_or("related"), nid),
                }
            }
        }
        None => println!("Entry not found: {}", entry_id),
    }
    Ok(())
}

fn cmd_log_viz(workspace: &Path, entry_id: Option<&str>, max_depth: usize, format: &str) -> Result<()> {
    if format != "text" {
        eprintln!("Warning: only 'text' format is supported in autoresearch viz; defaulting to ASCII output");
    }
    let (workspace, _) = ensure_workspace(workspace)?;
    let log_root = workspace.join("research-log");
    let db_path = log_root.join("research-log.db");
    let conn = research_log_rs::db::init_database(&db_path)?;
    let g = match entry_id {
        Some(eid) => research_log_rs::graph::load_subgraph(&conn, eid, max_depth)?,
        None => research_log_rs::graph::load_full_graph(&conn)?,
    };

    let mut labels = std::collections::HashMap::new();
    for node in &g.nodes {
        if let Some(entry) = research_log_rs::db::get_entry(&conn, node)? {
            labels.insert(node.clone(), format!("{}:{}", entry.direction, entry.question.chars().take(40).collect::<String>()));
        }
    }

    let stats = research_log_rs::graph::get_graph_stats(&g);
    println!("Knowledge Graph: {} nodes, {} edges, density {:.4}", stats.node_count, stats.edge_count, stats.density);
    println!();

    // Deduplicate edges for ASCII (each connection appears twice in adjacency)
    let mut seen_edges = std::collections::HashSet::new();
    for node in &g.nodes {
        let label = labels.get(node).map(|s| s.as_str()).unwrap_or(node);
        println!("  [{}] {}", node, label);
        if let Some(edges) = g.adjacency.get(node) {
            for (nbor, rel, w, _) in edges {
                let edge_key = if node.as_str() < nbor.as_str() {
                    format!("{}->{}", node, nbor)
                } else {
                    format!("{}->{}", nbor, node)
                };
                if !seen_edges.insert(edge_key) {
                    continue;
                }
                if g.nodes.contains(nbor) {
                    println!("   └──[{} w={:.1}]──> [{}]", rel.as_deref().unwrap_or("related"), w, nbor);
                }
            }
        }
        println!();
    }
    Ok(())
}

fn cmd_log_route(workspace: &Path, barrier_id: &str, max_depth: usize) -> Result<()> {
    let (workspace, _) = ensure_workspace(workspace)?;
    let log_root = workspace.join("research-log");
    let db_path = log_root.join("research-log.db");
    let conn = research_log_rs::db::init_database(&db_path)?;
    let route = research_log_rs::graph::trace_barrier_route(&conn, barrier_id, max_depth)?;

    println!("Barrier Route: {}", route.barrier.barrier_id);
    println!("  Loop: {:?}, Created: {}", route.barrier.loop_id, route.barrier.created_at);
    for ewf in &route.root_entries {
        println!("  Entry: [{}] {}", ewf.entry.id, ewf.entry.question);
    }
    let stats = research_log_rs::graph::get_graph_stats(&route.subgraph);
    println!("  Subgraph: {} nodes, {} edges", stats.node_count, stats.edge_count);
    Ok(())
}

fn cmd_log_extract(workspace: &Path, entry_id: &str) -> Result<()> {
    let (workspace, _) = ensure_workspace(workspace)?;
    let log_root = workspace.join("research-log");
    let db_path = log_root.join("research-log.db");
    let conn = research_log_rs::db::init_database(&db_path)?;

    let entry = match research_log_rs::db::get_entry(&conn, entry_id)? {
        Some(e) => e,
        None => {
            println!("Entry not found: {}", entry_id);
            return Ok(());
        }
    };

    let mut text = entry.question.clone();
    for finding in research_log_rs::db::get_findings(&conn, entry_id)? {
        text.push(' ');
        text.push_str(&finding.content);
    }
    for tag in research_log_rs::db::get_tags(&conn, entry_id)? {
        text.push(' ');
        text.push_str(&tag);
    }

    let found = research_log_rs::extract::extract_entities_from_text(&text);
    if found.is_empty() {
        println!("No entities found in entry [{}].", entry_id);
        return Ok(());
    }
    for (name, kind) in &found {
        let eid = research_log_rs::db::upsert_entity(&conn, name, kind, None, None)?;
        research_log_rs::db::insert_entry_entity(&conn, entry_id, eid, research_log_rs::models::ENTRY_ENTITY_ROLE_MENTIONED)?;
    }
    println!("Extracted {} entities from [{}]:", found.len(), entry_id);
    for (name, kind) in &found {
        println!("  [{}] {}", kind, name);
    }
    Ok(())
}

fn cmd_log_search_entities(workspace: &Path, query: &str, limit: usize) -> Result<()> {
    let (workspace, _) = ensure_workspace(workspace)?;
    let log_root = workspace.join("research-log");
    let db_path = log_root.join("research-log.db");
    let conn = research_log_rs::db::init_database(&db_path)?;
    let results = research_log_rs::db::search_entities(&conn, query, limit)?;

    println!("Entity search results for \"{}\" ({} found):", query, results.len());
    for e in &results {
        println!("  [{}] {} (id={})", e.kind, e.name, e.id);
    }
    Ok(())
}

fn cmd_set_novelty_gate(
    workspace: &Path,
    status: &GateStatusArg,
    decision: Option<String>,
    overlap_summary: Option<String>,
    differentiation_strategy: Option<String>,
    claims: &Vec<String>,
) -> Result<()> {
    let (workspace, state_path) = ensure_workspace(workspace)?;
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
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init {
            project,
            question,
            dir,
            mode,
        } => cmd_init(&project, &question, &dir, mode.as_str())?,
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
        Commands::LogRecord {
            workspace,
            direction,
            question,
            entry_point,
            barrier_id,
        } => cmd_log_record(
            &workspace,
            &direction,
            &question,
            &entry_point,
            barrier_id.as_deref(),
        )?,
        Commands::LogSearch {
            workspace,
            query,
            limit,
        } => cmd_log_search(&workspace, &query, limit)?,
        Commands::LogInsight {
            workspace,
            log_id,
            text,
            confidence,
        } => cmd_log_insight(&workspace, &log_id, &text, &confidence)?,
        Commands::LogConnect {
            workspace,
            log_id_a,
            log_id_b,
            relation,
        } => cmd_log_connect(&workspace, &log_id_a, &log_id_b, relation)?,
        Commands::LogNeighbors {
            workspace,
            entry_id,
            relation,
            limit,
        } => cmd_log_neighbors(&workspace, &entry_id, relation.as_deref(), limit)?,
        Commands::LogViz {
            workspace,
            entry_id,
            max_depth,
            format,
        } => cmd_log_viz(&workspace, entry_id.as_deref(), max_depth, &format)?,
        Commands::LogRoute {
            workspace,
            barrier_id,
            max_depth,
        } => cmd_log_route(&workspace, &barrier_id, max_depth)?,
        Commands::LogExtract {
            workspace,
            entry_id,
        } => cmd_log_extract(&workspace, &entry_id)?,
        Commands::LogSearchEntities {
            workspace,
            query,
            limit,
        } => cmd_log_search_entities(&workspace, &query, limit)?,
    }
    Ok(())
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

fn dump_state(path: &Path, state: &Value) -> Result<()> {
    let mut state_to_write = ensure_state_defaults(state);
    refresh_novelty_views(&mut state_to_write);
    set_key(&mut state_to_write, "schema_version", json!(SCHEMA_VERSION));
    set_key(&mut state_to_write, "updated_at", json!(now_iso()));
    let actions = recommend_next_actions(&state_to_write);
    set_key(&mut state_to_write, "next_actions", json!(actions));
    let rendered = serde_yml::to_string(&state_to_write)?;
    fs::write(path, rendered)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── helpers ──────────────────────────────────────────────────────────

    fn minimal_state() -> Value {
        default_state("test-project", "Does X improve Y?", "quick")
    }

    fn state_with_gate_passed() -> Value {
        let state = minimal_state();
        let state = draft_claims_from_state(&state, None, 3);
        let state = add_claim_comparison(
            &state,
            "claim-1",
            "method",
            "prior-work",
            "low",
            "different enough",
            "high",
            "novel",
            Some("C1"),
        );
        let mut state = add_claim_comparison(
            &state,
            "claim-2",
            "task",
            "prior-work-2",
            "medium",
            "different scope",
            "medium",
            "defensible",
            Some("C2"),
        );
        // Ensure the gate is explicitly passed
        novelty_gate_mut(&mut state).insert("status".into(), json!("passed"));
        state
    }

    fn state_with_hypothesis_and_run() -> (Value, tempfile::TempDir) {
        let state = state_with_gate_passed();
        let state = add_hypothesis(
            &state,
            HypothesisInput {
                claim: "c",
                prediction: None,
                mechanism: None,
                falsifiable_prediction: None,
                success_threshold: None,
                stop_condition: None,
                baselines: &[],
                confounders: &[],
                negative_signals: &[],
                minimal_test: None,
                priority: "medium",
                hypothesis_id: Some("h1"),
            },
        )
        .unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let state = record_run(
            &state,
            &RecordRunInput {
                hypothesis_id: "h1",
                outcome: "confirmatory",
                summary: "test run",
                metric_name: None,
                metric_value: None,
                command: None,
                evidence_path: None,
                sanity_checks: &[],
                baseline_result: None,
                rules_in: &[],
                rules_out: &[],
                alternative_explanations: &[],
                threats: &[],
                interpretation: None,
                finding: None,
                decision_delta: None,
                reuse_note: None,
                applies_to: &[],
                does_not_apply_to: &[],
                override_novelty_gate: false,
                override_reason: None,
            },
            tmp.path(),
        )
        .unwrap();
        (state, tmp)
    }

    // ── slugify ──────────────────────────────────────────────────────────

    #[test]
    fn slugify_basic_text() {
        assert_eq!(slugify("Hello World"), "hello-world");
    }

    #[test]
    fn slugify_special_characters() {
        assert_eq!(slugify("What is AI?"), "what-is-ai");
    }

    #[test]
    fn slugify_empty_input() {
        assert_eq!(slugify(""), "hypothesis");
        assert_eq!(slugify("   "), "hypothesis");
    }

    #[test]
    fn slugify_consecutive_special_chars() {
        assert_eq!(slugify("foo---bar"), "foo-bar");
    }

    #[test]
    fn slugify_leading_trailing_dashes() {
        assert_eq!(slugify("--hello--"), "hello");
    }

    // ── cleanup_question_text ────────────────────────────────────────────

    #[test]
    fn cleanup_removes_leading_auxiliary_verbs() {
        let result = cleanup_question_text("Can retrieval augmented generation improve citations?");
        assert!(!result.starts_with("Can "));
    }

    #[test]
    fn cleanup_removes_trailing_punctuation() {
        let result = cleanup_question_text("What is the answer?");
        assert!(!result.ends_with('?'));
    }

    #[test]
    fn cleanup_preserves_non_question_form() {
        let result = cleanup_question_text("retrieval augmented generation");
        assert_eq!(result, "retrieval augmented generation");
    }

    // ── extract_question_parts ───────────────────────────────────────────

    #[test]
    fn extract_question_parts_with_improves_pattern() {
        let (focus, target, effect) =
            extract_question_parts("Does method X improve accuracy on dataset Y?");
        assert!(!focus.is_empty());
        assert!(!target.is_empty());
        assert!(effect.contains("improve"));
    }

    #[test]
    fn extract_question_parts_with_using_pattern() {
        let (focus, _target, effect) =
            extract_question_parts("How using transformers for text classification works?");
        assert!(!focus.is_empty());
        assert!(!effect.is_empty());
    }

    #[test]
    fn extract_question_parts_fallback_to_keywords() {
        let (focus, _target, _effect) =
            extract_question_parts("neural architecture search optimization");
        assert!(!focus.is_empty());
    }

    // ── compact_words ────────────────────────────────────────────────────

    #[test]
    fn compact_words_filters_stopwords() {
        let words = compact_words("the quick brown fox is a good animal", 10);
        assert!(!words.contains(&"the".to_string()));
        assert!(!words.contains(&"is".to_string()));
        assert!(!words.contains(&"a".to_string()));
        assert!(words.contains(&"quick".to_string()));
    }

    #[test]
    fn compact_words_respects_limit() {
        let words = compact_words("alpha beta gamma delta epsilon zeta eta theta", 3);
        assert_eq!(words.len(), 3);
    }

    #[test]
    fn compact_words_deduplicates() {
        let words = compact_words("hello hello hello world", 10);
        assert_eq!(words.iter().filter(|w| *w == "hello").count(), 1);
    }

    #[test]
    fn compact_words_filters_short_words() {
        let words = compact_words("ab cd efgh", 10);
        assert!(!words.contains(&"ab".to_string()));
        assert!(words.contains(&"efgh".to_string()));
    }

    // ── XML parsing ──────────────────────────────────────────────────────

    #[test]
    fn xml_text_between_extracts_content() {
        let raw = "<title>  Hello World  </title>";
        assert_eq!(xml_text_between(raw, "title"), Some("Hello World".into()));
    }

    #[test]
    fn xml_text_between_missing_tag() {
        assert_eq!(xml_text_between("<other>data</other>", "title"), None);
    }

    #[test]
    fn decode_xml_entities_all_types() {
        let result = decode_xml_entities("&lt;b&gt;bold &amp; &quot;quoted&quot; &apos;text&apos;");
        assert_eq!(result, "<b>bold & \"quoted\" 'text'");
    }

    #[test]
    fn decode_xml_entities_collapses_whitespace() {
        let result = decode_xml_entities("hello   world");
        assert_eq!(result, "hello world");
    }

    // ── markdown_link ────────────────────────────────────────────────────

    #[test]
    fn markdown_link_some() {
        assert_eq!(
            markdown_link(Some("https://example.com")),
            "[link](https://example.com)"
        );
    }

    #[test]
    fn markdown_link_none() {
        assert_eq!(markdown_link(None), "-");
    }

    #[test]
    fn markdown_link_empty() {
        assert_eq!(markdown_link(Some("  ")), "-");
    }

    // ── replace_placeholders ─────────────────────────────────────────────

    #[test]
    fn replace_placeholders_basic() {
        let result = replace_placeholders("Hello {name}!", &[("name", "World")]);
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn replace_placeholders_multiple() {
        let result = replace_placeholders("{a} and {b}", &[("a", "X"), ("b", "Y")]);
        assert_eq!(result, "X and Y");
    }

    #[test]
    fn replace_placeholders_no_match() {
        let result = replace_placeholders("no placeholders", &[("key", "val")]);
        assert_eq!(result, "no placeholders");
    }

    // ── escape_table_cell ────────────────────────────────────────────────

    #[test]
    fn escape_table_cell_replaces_pipe() {
        assert_eq!(escape_table_cell("foo|bar"), "foo/bar");
    }

    #[test]
    fn escape_table_cell_no_pipe() {
        assert_eq!(escape_table_cell("normal"), "normal");
    }

    // ── format_overlap_risk ──────────────────────────────────────────────

    #[test]
    fn format_overlap_risk_all_levels() {
        assert!(format_overlap_risk("low").contains("low"));
        assert!(format_overlap_risk("medium").contains("medium"));
        assert!(format_overlap_risk("high").contains("high"));
        assert_eq!(format_overlap_risk("unknown"), "unknown");
    }

    // ── value_to_string ──────────────────────────────────────────────────

    #[test]
    fn value_to_string_conversions() {
        assert_eq!(value_to_string(&json!("hello")), "hello");
        assert_eq!(value_to_string(&Value::Null), "-");
        assert_eq!(value_to_string(&json!(42)), "42");
    }

    // ── join_string_array ────────────────────────────────────────────────

    #[test]
    fn join_string_array_normal() {
        let arr = json!(["a", "b", "c"]);
        assert_eq!(join_string_array(arr.as_array().unwrap()), "a, b, c");
    }

    #[test]
    fn join_string_array_empty() {
        let arr = json!([]);
        assert_eq!(join_string_array(arr.as_array().unwrap()), "_none_");
    }

    // ── upsert_managed_block ─────────────────────────────────────────────

    #[test]
    fn upsert_managed_block_inserts_new() {
        let result =
            upsert_managed_block("existing content", "<!--START-->", "<!--END-->", "new data");
        assert!(result.contains("<!--START-->"));
        assert!(result.contains("new data"));
        assert!(result.contains("<!--END-->"));
        assert!(result.contains("existing content"));
    }

    #[test]
    fn upsert_managed_block_replaces_existing() {
        let text = "before\n<!--START-->\nold data\n<!--END-->\nafter";
        let result = upsert_managed_block(text, "<!--START-->", "<!--END-->", "updated");
        assert!(result.contains("updated"));
        assert!(!result.contains("old data"));
        assert!(result.contains("before"));
        assert!(result.contains("after"));
    }

    // ── dedupe_research_results ──────────────────────────────────────────

    #[test]
    fn dedupe_research_results_removes_duplicates() {
        let results = vec![
            json!({"source": "arXiv", "title": "Paper A"}),
            json!({"source": "arXiv", "title": "Paper A"}),
            json!({"source": "Semantic Scholar", "title": "Paper A"}),
        ];
        let deduped = dedupe_research_results(results);
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn dedupe_research_results_case_insensitive() {
        let results = vec![
            json!({"source": "arXiv", "title": "Paper A"}),
            json!({"source": "arxiv", "title": "paper a"}),
        ];
        let deduped = dedupe_research_results(results);
        assert_eq!(deduped.len(), 1);
    }

    // ── source_covers ────────────────────────────────────────────────────

    #[test]
    fn source_covers_all_covers_specific() {
        assert!(source_covers("all", &ExternalSourceArg::Arxiv));
        assert!(source_covers("all", &ExternalSourceArg::SemanticScholar));
    }

    #[test]
    fn source_covers_specific_does_not_cover_other() {
        assert!(!source_covers("arxiv", &ExternalSourceArg::SemanticScholar));
    }

    #[test]
    fn source_covers_exact_match() {
        assert!(source_covers("arxiv", &ExternalSourceArg::Arxiv));
        assert!(source_covers(
            "semantic-scholar",
            &ExternalSourceArg::SemanticScholar
        ));
        assert!(source_covers("all", &ExternalSourceArg::All));
    }

    // ── normalize_limit ──────────────────────────────────────────────────

    #[test]
    fn normalize_limit_clamping() {
        assert_eq!(normalize_limit(0), 1);
        assert_eq!(normalize_limit(5), 5);
        assert_eq!(normalize_limit(50), 20);
        assert_eq!(normalize_limit(1), 1);
        assert_eq!(normalize_limit(20), 20);
    }

    // ── string_vec / optional_string ─────────────────────────────────────

    #[test]
    fn string_vec_trims_and_filters() {
        let result = string_vec(&[" hello ".into(), "".into(), "world".into()]);
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0].as_str().unwrap(), "hello");
    }

    #[test]
    fn optional_string_variants() {
        assert_eq!(optional_string(Some("hello")), json!("hello"));
        assert_eq!(optional_string(Some("  ")), Value::Null);
        assert_eq!(optional_string(None), Value::Null);
    }

    // ── default_state ────────────────────────────────────────────────────

    #[test]
    fn default_state_has_all_required_keys() {
        let state = default_state("proj", "question?", "quick");
        for key in [
            "schema_version",
            "project",
            "question",
            "mode",
            "status",
            "stage",
            "hypotheses",
            "run_history",
            "external_research",
            "novelty_gate",
            "next_actions",
            "created_at",
            "updated_at",
        ] {
            assert!(state.get(key).is_some(), "missing key: {key}");
        }
        assert_eq!(state.get("project").and_then(Value::as_str), Some("proj"));
        assert_eq!(state.get("mode").and_then(Value::as_str), Some("quick"));
    }

    // ── migrate_state ────────────────────────────────────────────────────

    #[test]
    fn migrate_state_upgrades_version() {
        let old = json!({
            "schema_version": 2,
            "hypotheses": [{"id": "h1", "status": "active"}],
            "run_history": [],
            "decisions": [],
            "updated_at": "2026-01-01T00:00:00Z"
        });
        let migrated = migrate_state(&old);
        assert_eq!(
            migrated.get("schema_version").and_then(Value::as_i64),
            Some(SCHEMA_VERSION)
        );
        assert!(migrated.get("external_research").is_some());
    }

    #[test]
    fn migrate_state_already_current_is_noop() {
        let state = default_state("p", "q", "quick");
        let migrated = migrate_state(&state);
        assert_eq!(
            migrated.get("schema_version").and_then(Value::as_i64),
            Some(SCHEMA_VERSION)
        );
    }

    // ── ensure_state_defaults ────────────────────────────────────────────

    #[test]
    fn ensure_state_defaults_fills_missing_fields() {
        let sparse = json!({"project": "p", "question": "q"});
        let filled = ensure_state_defaults(&sparse);
        assert!(filled.get("hypotheses").is_some());
        assert!(filled.get("run_history").is_some());
        assert!(filled.get("novelty_gate").is_some());
        assert_eq!(
            filled.get("schema_version").and_then(Value::as_i64),
            Some(SCHEMA_VERSION)
        );
    }

    #[test]
    fn ensure_state_defaults_preserves_existing() {
        let state = default_state("proj", "question", "full");
        let ensured = ensure_state_defaults(&state);
        assert_eq!(ensured.get("project").and_then(Value::as_str), Some("proj"));
        assert_eq!(ensured.get("mode").and_then(Value::as_str), Some("full"));
    }

    // ── axis_weights ─────────────────────────────────────────────────────

    #[test]
    fn axis_weights_known_axes() {
        let (n, c, r) = axis_weights("method");
        assert_eq!((n, c, r), (5, 2, 3));
        let (n, c, r) = axis_weights("task");
        assert_eq!((n, c, r), (4, 3, 4));
    }

    #[test]
    fn axis_weights_unknown_defaults() {
        let (n, c, r) = axis_weights("unknown");
        assert_eq!((n, c, r), (3, 2, 3));
    }

    // ── default_required_evidence ────────────────────────────────────────

    #[test]
    fn default_required_evidence_method() {
        let evidence = default_required_evidence("method");
        assert_eq!(evidence.len(), 3);
        assert!(evidence[0].to_lowercase().contains("overlap"));
    }

    #[test]
    fn default_required_evidence_unknown_axis() {
        let evidence = default_required_evidence("random");
        assert_eq!(evidence.len(), 3);
    }

    // ── expected_baselines_for_axis ──────────────────────────────────────

    #[test]
    fn expected_baselines_for_axis_all_variants() {
        for axis in [
            "method",
            "task",
            "setting",
            "comparison",
            "framing",
            "unknown",
        ] {
            let baselines = expected_baselines_for_axis(axis);
            assert_eq!(baselines.len(), 3, "axis={axis} should have 3 baselines");
        }
    }

    // ── propose_claims_from_question ─────────────────────────────────────

    #[test]
    fn propose_claims_from_question_generates_correct_count() {
        let claims = propose_claims_from_question("Does method X improve accuracy?", 3);
        assert_eq!(claims.len(), 3);
    }

    #[test]
    fn propose_claims_from_question_clamps_count() {
        let claims = propose_claims_from_question("test question", 0);
        assert_eq!(claims.len(), 1);
        let claims = propose_claims_from_question("test question", 100);
        assert!(claims.len() <= 5);
    }

    #[test]
    fn propose_claims_has_claim_ids() {
        let claims = propose_claims_from_question("Does X improve Y?", 2);
        assert_eq!(
            claims[0].get("claim_id").and_then(Value::as_str),
            Some("C1")
        );
        assert_eq!(
            claims[1].get("claim_id").and_then(Value::as_str),
            Some("C2")
        );
    }

    // ── draft_claims_from_state ──────────────────────────────────────────

    #[test]
    fn draft_claims_from_state_populates_gate() {
        let state = minimal_state();
        let updated = draft_claims_from_state(&state, Some("Does X improve Y?"), 3);
        let drafts = novelty_arr(&updated, "draft_claims");
        assert_eq!(drafts.len(), 3);
        let claims = novelty_arr(&updated, "claims");
        assert_eq!(claims.len(), 3);
    }

    #[test]
    fn draft_claims_from_state_uses_question_override() {
        let state = minimal_state();
        let updated = draft_claims_from_state(&state, Some("Custom question about Z?"), 2);
        let drafts = novelty_arr(&updated, "draft_claims");
        assert!(!drafts.is_empty());
    }

    // ── add_claim_comparison ─────────────────────────────────────────────

    #[test]
    fn add_claim_comparison_creates_record() {
        let state = minimal_state();
        let updated = add_claim_comparison(
            &state,
            "my claim",
            "method",
            "prior work",
            "low",
            "different approach",
            "high",
            "novel",
            Some("C1"),
        );
        let records = novelty_arr(&updated, "claim_records");
        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].get("claim_id").and_then(Value::as_str),
            Some("C1")
        );
        assert_eq!(
            records[0].get("verdict").and_then(Value::as_str),
            Some("novel")
        );
    }

    #[test]
    fn add_claim_comparison_updates_existing() {
        let state = minimal_state();
        let updated = add_claim_comparison(
            &state,
            "first",
            "method",
            "pw",
            "low",
            "diff",
            "high",
            "novel",
            Some("C1"),
        );
        let updated = add_claim_comparison(
            &updated,
            "updated",
            "task",
            "pw2",
            "high",
            "diff2",
            "low",
            "risky",
            Some("C1"),
        );
        let records = novelty_arr(&updated, "claim_records");
        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].get("claim").and_then(Value::as_str),
            Some("updated")
        );
    }

    #[test]
    fn add_claim_comparison_auto_ids() {
        let state = minimal_state();
        let updated = add_claim_comparison(
            &state, "a", "method", "pw", "low", "diff", "high", "novel", None,
        );
        let updated = add_claim_comparison(
            &updated, "b", "task", "pw2", "low", "diff2", "high", "novel", None,
        );
        let records = novelty_arr(&updated, "claim_records");
        assert_eq!(records.len(), 2);
        let ids: Vec<&str> = records
            .iter()
            .filter_map(|r| r.get("claim_id").and_then(Value::as_str))
            .collect();
        assert!(ids.contains(&"C1"));
        assert!(ids.contains(&"C2"));
    }

    // ── score_claim_priority ─────────────────────────────────────────────

    #[test]
    fn score_claim_priority_novel_low_overlap() {
        let record = json!({
            "axis": "method",
            "overlap": "low",
            "confidence": "high",
            "verdict": "novel"
        });
        let scored = score_claim_priority(&record);
        let score = scored
            .get("priority_score")
            .and_then(Value::as_i64)
            .unwrap();
        assert!(score > 15, "expected high score, got {score}");
        assert_eq!(
            scored.get("priority_label").and_then(Value::as_str),
            Some("first")
        );
    }

    #[test]
    fn score_claim_priority_not_novel_high_overlap() {
        let record = json!({
            "axis": "method",
            "overlap": "high",
            "confidence": "low",
            "verdict": "not-novel"
        });
        let scored = score_claim_priority(&record);
        let score = scored
            .get("priority_score")
            .and_then(Value::as_i64)
            .unwrap();
        assert!(score < 13, "expected low score, got {score}");
    }

    // ── prioritize_claims ────────────────────────────────────────────────

    #[test]
    fn prioritize_claims_sorts_by_score() {
        let claims = vec![
            json!({"claim_id": "C2", "axis": "method", "overlap": "high", "verdict": "not-novel", "confidence": "low"}),
            json!({"claim_id": "C1", "axis": "method", "overlap": "low", "verdict": "novel", "confidence": "high"}),
        ];
        let prioritized = prioritize_claims(&claims);
        assert_eq!(
            prioritized[0].get("claim_id").and_then(Value::as_str),
            Some("C1")
        );
        assert_eq!(
            prioritized[0]
                .get("recommended_order")
                .and_then(Value::as_i64),
            Some(1)
        );
    }

    // ── overall_novelty_assessment ───────────────────────────────────────

    #[test]
    fn overall_novelty_assessment_empty() {
        let state = minimal_state();
        assert_eq!(overall_novelty_assessment(&state), "insufficient");
    }

    #[test]
    fn overall_novelty_assessment_strong() {
        let state = state_with_gate_passed();
        // gate_passed has C1=novel, C2=defensible -> "strong" (has at least one novel)
        assert_eq!(overall_novelty_assessment(&state), "strong");
    }

    #[test]
    fn overall_novelty_assessment_weak() {
        let state = minimal_state();
        let state = add_claim_comparison(
            &state,
            "a",
            "method",
            "pw",
            "low",
            "d",
            "high",
            "not-novel",
            Some("C1"),
        );
        let state = add_claim_comparison(
            &state,
            "b",
            "task",
            "pw",
            "low",
            "d",
            "high",
            "not-novel",
            Some("C2"),
        );
        assert_eq!(overall_novelty_assessment(&state), "weak");
    }

    // ── find_hypothesis ──────────────────────────────────────────────────

    #[test]
    fn find_hypothesis_found() {
        let mut state = minimal_state();
        arr_mut(&mut state, "hypotheses").push(json!({"id": "h1", "claim": "test"}));
        assert!(find_hypothesis(&state, "h1").is_some());
    }

    #[test]
    fn find_hypothesis_not_found() {
        let state = minimal_state();
        assert!(find_hypothesis(&state, "nonexistent").is_none());
    }

    // ── next_run_id ──────────────────────────────────────────────────────

    #[test]
    fn next_run_id_sequential() {
        let state = minimal_state();
        assert_eq!(next_run_id(&state), "run-001");
    }

    #[test]
    fn next_run_id_after_runs() {
        let mut state = minimal_state();
        arr_mut(&mut state, "run_history").push(json!({"run_id": "run-001"}));
        arr_mut(&mut state, "run_history").push(json!({"run_id": "run-002"}));
        assert_eq!(next_run_id(&state), "run-003");
    }

    // ── add_hypothesis ───────────────────────────────────────────────────

    #[test]
    fn add_hypothesis_basic() {
        let state = state_with_gate_passed();
        let result = add_hypothesis(
            &state,
            HypothesisInput {
                claim: "test claim",
                prediction: Some("prediction"),
                mechanism: Some("mechanism"),
                falsifiable_prediction: None,
                success_threshold: None,
                stop_condition: None,
                baselines: &[],
                confounders: &[],
                negative_signals: &[],
                minimal_test: None,
                priority: "high",
                hypothesis_id: Some("h-1"),
            },
        );
        let updated = result.unwrap();
        assert_eq!(arr(&updated, "hypotheses").len(), 1);
        assert_eq!(
            find_hypothesis(&updated, "h-1")
                .unwrap()
                .get("claim")
                .and_then(Value::as_str),
            Some("test claim")
        );
    }

    #[test]
    fn add_hypothesis_duplicate_rejected() {
        let state = state_with_gate_passed();
        let state = add_hypothesis(
            &state,
            HypothesisInput {
                claim: "c",
                prediction: None,
                mechanism: None,
                falsifiable_prediction: None,
                success_threshold: None,
                stop_condition: None,
                baselines: &[],
                confounders: &[],
                negative_signals: &[],
                minimal_test: None,
                priority: "medium",
                hypothesis_id: Some("dup"),
            },
        )
        .unwrap();
        let result = add_hypothesis(
            &state,
            HypothesisInput {
                claim: "c2",
                prediction: None,
                mechanism: None,
                falsifiable_prediction: None,
                success_threshold: None,
                stop_condition: None,
                baselines: &[],
                confounders: &[],
                negative_signals: &[],
                minimal_test: None,
                priority: "medium",
                hypothesis_id: Some("dup"),
            },
        );
        assert!(result.is_err());
    }

    // ── transition_hypothesis ────────────────────────────────────────────

    #[test]
    fn transition_hypothesis_valid() {
        let mut state = state_with_gate_passed();
        arr_mut(&mut state, "hypotheses").push(json!({
            "id": "h1", "claim": "test", "status": "queued",
            "status_reason": null, "status_updated_at": now_iso(), "created_at": now_iso()
        }));
        let index = find_hypothesis_index(&state, "h1").unwrap();
        let result = transition_hypothesis(&mut state, index, "active", Some("activated"));
        assert!(result.is_ok());
        assert_eq!(
            find_hypothesis(&state, "h1")
                .unwrap()
                .get("status")
                .and_then(Value::as_str),
            Some("active")
        );
    }

    #[test]
    fn transition_hypothesis_invalid() {
        let mut state = state_with_gate_passed();
        arr_mut(&mut state, "hypotheses").push(json!({
            "id": "h1", "claim": "test", "status": "concluded",
            "status_reason": null, "status_updated_at": now_iso(), "created_at": now_iso()
        }));
        let index = find_hypothesis_index(&state, "h1").unwrap();
        // concluded -> active is not allowed (concluded is terminal)
        let result = transition_hypothesis(&mut state, index, "active", None);
        assert!(result.is_err());
    }

    // ── format_string_list ───────────────────────────────────────────────

    #[test]
    fn format_string_list_normal() {
        let result = format_string_list(&["a".into(), "b".into()], "empty");
        assert!(result.contains("- a"));
        assert!(result.contains("- b"));
    }

    #[test]
    fn format_string_list_empty() {
        assert_eq!(format_string_list(&[], "empty"), "empty");
    }

    // ── format_status ────────────────────────────────────────────────────

    #[test]
    fn format_status_contains_key_info() {
        let state = minimal_state();
        let status = format_status(&state);
        assert!(status.contains("test-project"));
        assert!(status.contains("bootstrap"));
        assert!(status.contains("active"));
        assert!(status.contains("next_actions:"));
    }

    // ── parse_iso_timestamp ──────────────────────────────────────────────

    #[test]
    fn parse_iso_timestamp_valid() {
        let ts = parse_iso_timestamp("2026-01-15T10:30:00Z");
        assert!(ts.is_some());
    }

    #[test]
    fn parse_iso_timestamp_invalid() {
        assert!(parse_iso_timestamp("not-a-date").is_none());
        assert!(parse_iso_timestamp("").is_none());
        assert!(parse_iso_timestamp("  ").is_none());
    }

    // ── default_research_query ───────────────────────────────────────────

    #[test]
    fn default_research_query_explicit_wins() {
        let result = default_research_query(None, Some("custom query"));
        assert_eq!(result.unwrap(), "custom query");
    }

    #[test]
    fn default_research_query_fails_without_claim_or_explicit() {
        let result = default_research_query(None, None);
        assert!(result.is_err());
    }

    #[test]
    fn default_research_query_from_claim() {
        let record = json!({
            "claim": "neural architecture search improves efficiency",
            "axis": "method"
        });
        let result = default_research_query(Some(&record), None).unwrap();
        assert!(!result.is_empty());
    }

    // ── external_research_result_count ───────────────────────────────────

    #[test]
    fn external_research_result_count_with_results() {
        let entry = json!({"results": [{"title": "A"}, {"title": "B"}]});
        assert_eq!(external_research_result_count(&entry), 2);
    }

    #[test]
    fn external_research_result_count_no_results() {
        let entry = json!({});
        assert_eq!(external_research_result_count(&entry), 0);
    }

    // ── add_external_research ────────────────────────────────────────────

    #[test]
    fn add_external_research_appends() {
        let state = minimal_state();
        let research = json!({
            "research_id": "ext-123",
            "query": "test query",
            "source": "all",
            "results": [],
            "errors": [],
            "created_at": "2026-01-01T00:00:00Z"
        });
        let updated = add_external_research(&state, research);
        assert_eq!(arr(&updated, "external_research").len(), 1);
    }

    // ── record_run and reflect (integration) ─────────────────────────────

    #[test]
    fn record_run_requires_passed_gate() {
        let state = state_with_gate_passed();
        let state = add_hypothesis(
            &state,
            HypothesisInput {
                claim: "c",
                prediction: None,
                mechanism: None,
                falsifiable_prediction: None,
                success_threshold: None,
                stop_condition: None,
                baselines: &[],
                confounders: &[],
                negative_signals: &[],
                minimal_test: None,
                priority: "medium",
                hypothesis_id: Some("h1"),
            },
        )
        .unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let result = record_run(
            &state,
            &RecordRunInput {
                hypothesis_id: "h1",
                outcome: "confirmatory",
                summary: "test summary",
                metric_name: None,
                metric_value: None,
                command: None,
                evidence_path: None,
                sanity_checks: &[],
                baseline_result: None,
                rules_in: &[],
                rules_out: &[],
                alternative_explanations: &[],
                threats: &[],
                interpretation: None,
                finding: None,
                decision_delta: None,
                reuse_note: None,
                applies_to: &[],
                does_not_apply_to: &[],
                override_novelty_gate: false,
                override_reason: None,
            },
            tmp.path(),
        );
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(arr(&updated, "run_history").len(), 1);
    }

    #[test]
    fn record_run_rejects_unknown_hypothesis() {
        let state = state_with_gate_passed();
        let tmp = tempfile::tempdir().unwrap();
        let result = record_run(
            &state,
            &RecordRunInput {
                hypothesis_id: "nonexistent",
                outcome: "confirmatory",
                summary: "summary",
                metric_name: None,
                metric_value: None,
                command: None,
                evidence_path: None,
                sanity_checks: &[],
                baseline_result: None,
                rules_in: &[],
                rules_out: &[],
                alternative_explanations: &[],
                threats: &[],
                interpretation: None,
                finding: None,
                decision_delta: None,
                reuse_note: None,
                applies_to: &[],
                does_not_apply_to: &[],
                override_novelty_gate: false,
                override_reason: None,
            },
            tmp.path(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn reflect_transitions_hypothesis() {
        let (state, _tmp) = state_with_hypothesis_and_run();
        // Reflect with DEEPEN
        let result = reflect(&state, "h1", "DEEPEN", "interesting pattern", None, None);
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(
            updated.get("current_direction").and_then(Value::as_str),
            Some("DEEPEN")
        );
        assert_eq!(
            find_hypothesis(&updated, "h1")
                .unwrap()
                .get("status")
                .and_then(Value::as_str),
            Some("active")
        );
    }

    #[test]
    fn reflect_conclude_sets_status() {
        let (state, _tmp) = state_with_hypothesis_and_run();
        let updated = reflect(&state, "h1", "CONCLUDE", "done", None, None).unwrap();
        assert_eq!(
            updated.get("status").and_then(Value::as_str),
            Some("concluded")
        );
    }

    // ── annotate_run ─────────────────────────────────────────────────────

    #[test]
    fn annotate_run_adds_finding() {
        let (state, _tmp) = state_with_hypothesis_and_run();
        let run_id = str_field(arr(&state, "run_history").last().unwrap(), "run_id");
        let updated = annotate_run(
            &state,
            &run_id,
            RunAnnotationInput {
                finding: Some("reusable finding"),
                decision_delta: Some("changed decision"),
                reuse_note: Some("note"),
                applies_to: &["scope-a".into()],
                does_not_apply_to: &[],
            },
        )
        .unwrap();
        let run = latest_run_by_id(&updated, &run_id).unwrap();
        assert_eq!(
            run.get("finding").and_then(Value::as_str),
            Some("reusable finding")
        );
    }

    #[test]
    fn annotate_run_rejects_unknown_id() {
        let state = minimal_state();
        let result = annotate_run(
            &state,
            "nonexistent",
            RunAnnotationInput {
                finding: None,
                decision_delta: None,
                reuse_note: None,
                applies_to: &[],
                does_not_apply_to: &[],
            },
        );
        assert!(result.is_err());
    }

    // ── reuse_audit ──────────────────────────────────────────────────────

    #[test]
    fn reuse_audit_counts_correctly() {
        let state = minimal_state();
        let audit = reuse_audit(&state);
        assert_eq!(audit.get("runs").and_then(Value::as_u64), Some(0));
        assert_eq!(audit.get("reusable_runs").and_then(Value::as_u64), Some(0));
    }

    // ── build_search_queries ─────────────────────────────────────────────

    #[test]
    fn build_search_queries_returns_four() {
        let queries = build_search_queries("neural architecture search efficiency", "method");
        assert_eq!(queries.len(), 4);
        let labels: Vec<&str> = queries
            .iter()
            .filter_map(|q| q.get("label").and_then(Value::as_str))
            .collect();
        assert!(labels.contains(&"broad"));
        assert!(labels.contains(&"focused"));
        assert!(labels.contains(&"recent"));
        assert!(labels.contains(&"combination"));
    }

    #[test]
    fn build_search_queries_empty_claim() {
        let queries = build_search_queries("", "");
        assert_eq!(queries.len(), 4);
    }

    // ── verification_standard_for_priority ───────────────────────────────

    #[test]
    fn verification_standard_for_priority_all_labels() {
        assert!(!verification_standard_for_priority("first").is_empty());
        assert!(!verification_standard_for_priority("next").is_empty());
        assert!(!verification_standard_for_priority("later").is_empty());
    }

    // ── value_as_string_list ─────────────────────────────────────────────

    #[test]
    fn value_as_string_list_from_array() {
        let value = json!({"tags": ["a", "b", "", "c"]});
        let list = value_as_string_list(&value, "tags");
        assert_eq!(list, vec!["a", "b", "c"]);
    }

    #[test]
    fn value_as_string_list_missing_key() {
        let value = json!({});
        let list = value_as_string_list(&value, "missing");
        assert!(list.is_empty());
    }

    // ── default_run_record_path / default_reflection_path ────────────────

    #[test]
    fn default_run_record_path_format() {
        assert_eq!(
            default_run_record_path("h1", "run-001"),
            "experiments/h1/run-001.md"
        );
    }

    #[test]
    fn default_reflection_path_format() {
        assert_eq!(
            default_reflection_path("h1", Some("run-001")),
            "experiments/h1/run-001-reflection.md"
        );
        assert_eq!(
            default_reflection_path("h1", None),
            "experiments/h1/reflection-reflection.md"
        );
    }
}
