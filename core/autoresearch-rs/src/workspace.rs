//! Workspace file management: write_if_missing, sync_workspace_files,
//! format_status, format_resume.

use anyhow::Result;
use serde_json::Value;
use std::fs;
use std::path::Path;

use crate::*;

pub(super) fn write_if_missing(path: &Path, content: String) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if !path.exists() {
        fs::write(path, content)?;
    }
    Ok(())
}

pub(super) fn sync_workspace_files(workspace: &Path, state: &Value) -> Result<()> {
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
        &workspace.join("findings-reuse-index.md"),
        "# Findings Reuse Index\n\n",
        REUSE_INDEX_BLOCK_START,
        REUSE_INDEX_BLOCK_END,
        render_reuse_index_summary(state),
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

pub(super) fn format_status(state: &Value) -> String {
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
        format!("reusable_runs: {}", reusable_runs(state).len()),
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

pub(super) fn format_resume(state: &Value) -> String {
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
            "latest_interpretation: {}",
            str_field_default(run, "interpretation", "-")
        ));
        lines.push(format!(
            "latest_finding: {}",
            str_field_default(run, "finding", "-")
        ));
        lines.push(format!(
            "latest_decision_delta: {}",
            str_field_default(run, "decision_delta", "-")
        ));
        let applies_to = value_as_string_list(run, "applies_to");
        if !applies_to.is_empty() {
            lines.push(format!("latest_applies_to: {}", applies_to.join("; ")));
        }
        let rules_out = value_as_string_list(run, "rules_out");
        if !rules_out.is_empty() {
            lines.push(format!("latest_rules_out: {}", rules_out.join("; ")));
        }
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
    lines.push(format!("reusable_runs: {}", reusable_runs(state).len()));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn write_if_missing_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.md");
        write_if_missing(&path, "content".to_string()).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "content");
    }

    #[test]
    fn write_if_missing_does_not_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.md");
        fs::write(&path, "original").unwrap();
        write_if_missing(&path, "new".to_string()).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "original");
    }

    #[test]
    fn write_if_missing_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/b/c/test.md");
        write_if_missing(&path, "deep".to_string()).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "deep");
    }
}
