//! Formatting and rendering: tables, findings summary, novelty gate summary,
//! search plan, external research, claims, hypothesis cards, protocols,
//! run records, reflections, and managed file syncing.

use serde_json::Value;
use std::fs;
use std::path::Path;

use crate::*;

pub(super) fn escape_table_cell(value: &str) -> String {
    value.replace('|', "/")
}

pub(super) fn format_overlap_risk(overlap: &str) -> String {
    match overlap {
        "low" => "🟢 low".into(),
        "medium" => "🟡 medium".into(),
        "high" => "🔴 high".into(),
        _ => overlap.into(),
    }
}

pub(super) fn summarize_rules_in(state: &Value) -> Vec<String> {
    let mut lines = Vec::new();
    for record in current_context_runs(state) {
        let run_id = str_field(&record, "run_id");
        let finding = str_field_default(&record, "finding", "");
        if !finding.is_empty() {
            lines.push(format!("{run_id}: {finding}"));
            continue;
        }
        let rules = value_as_string_list(&record, "rules_in");
        if rules.is_empty() {
            lines.push(format!(
                "{}: {}",
                run_id,
                str_field_default(&record, "summary", "_No summary_")
            ));
        } else {
            for item in rules {
                lines.push(format!("{run_id}: {item}"));
            }
        }
    }
    if lines.is_empty() {
        vec!["_No run-backed support recorded yet._".into()]
    } else {
        lines
    }
}

pub(super) fn summarize_rules_out(state: &Value) -> Vec<String> {
    let mut lines = Vec::new();
    for record in current_context_runs(state) {
        let run_id = str_field(&record, "run_id");
        let rules = value_as_string_list(&record, "rules_out");
        if rules.is_empty() {
            if ["failed", "ambiguous"]
                .contains(&record.get("outcome").and_then(Value::as_str).unwrap_or(""))
            {
                lines.push(format!(
                    "{}: {}",
                    run_id,
                    str_field_default(&record, "summary", "_No summary_")
                ));
            }
        } else {
            for item in rules {
                lines.push(format!("{run_id}: {item}"));
            }
        }
    }
    if lines.is_empty() {
        vec!["_No ruled-out branch has been recorded yet._".into()]
    } else {
        lines
    }
}

pub(super) fn summarize_remaining_risks(state: &Value) -> Vec<String> {
    let blockers = arr(state, "blockers");
    if !blockers.is_empty() {
        return blockers
            .iter()
            .take(3)
            .map(|item| item.as_str().unwrap_or(&item.to_string()).to_string())
            .collect();
    }
    let mut risks = Vec::new();
    for record in current_context_runs(state) {
        let run_id = str_field(&record, "run_id");
        for item in value_as_string_list(&record, "does_not_apply_to") {
            risks.push(format!("{run_id} outside scope: {item}"));
        }
        for item in value_as_string_list(&record, "threats") {
            risks.push(format!("{run_id}: {item}"));
        }
    }
    if !risks.is_empty() {
        return risks.into_iter().take(5).collect();
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

pub(super) fn render_findings_summary(state: &Value) -> String {
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
    let reusable = current_context_runs(state)
        .into_iter()
        .filter(|record| {
            !str_field_default(record, "reuse_note", "").is_empty()
                || !str_field_default(record, "decision_delta", "").is_empty()
                || !value_as_string_list(record, "applies_to").is_empty()
        })
        .collect::<Vec<_>>();
    if !reusable.is_empty() {
        lines.extend([String::new(), "### Reuse Notes".into()]);
        for record in reusable.into_iter().take(5) {
            let run_id = str_field(&record, "run_id");
            let note = str_field_default(&record, "reuse_note", "");
            let delta = str_field_default(&record, "decision_delta", "");
            if !note.is_empty() {
                lines.push(format!("- {run_id}: {note}"));
            }
            if !delta.is_empty() {
                lines.push(format!("- {run_id} decision: {delta}"));
            }
            let applies_to = value_as_string_list(&record, "applies_to");
            if !applies_to.is_empty() {
                lines.push(format!("- {run_id} applies to: {}", applies_to.join("; ")));
            }
        }
    }
    let alternative_explanations = current_context_runs(state)
        .into_iter()
        .flat_map(|record| value_as_string_list(&record, "alternative_explanations"))
        .collect::<Vec<_>>();
    if !alternative_explanations.is_empty() {
        lines.extend([
            String::new(),
            "### Alternative Explanations To Clear".into(),
        ]);
        for item in alternative_explanations.into_iter().take(5) {
            lines.push(format!("- {item}"));
        }
    }
    lines.join("\n")
}

pub(super) fn render_reuse_index_summary(state: &Value) -> String {
    let runs = reusable_runs(state);
    let mut lines = vec![
        "## Managed Reuse Index".into(),
        String::new(),
        "- purpose: find portable results without rereading chronological logs".into(),
        format!("- reusable runs: {}", runs.len()),
        String::new(),
        "| Run | Finding | Decision Delta | Applies To | Does Not Apply To | Reuse Note |".into(),
        "|---|---|---|---|---|---|".into(),
    ];
    if runs.is_empty() {
        lines.push("| _none yet_ | - | - | - | - | - |".into());
    } else {
        for record in runs.iter().take(20) {
            lines.push(format!(
                "| {} | {} | {} | {} | {} | {} |",
                escape_table_cell(&str_field_default(record, "run_id", "-")),
                escape_table_cell(&str_field_default(record, "finding", "-")),
                escape_table_cell(&str_field_default(record, "decision_delta", "-")),
                escape_table_cell(&value_as_string_list(record, "applies_to").join("; ")),
                escape_table_cell(&value_as_string_list(record, "does_not_apply_to").join("; ")),
                escape_table_cell(&str_field_default(record, "reuse_note", "-")),
            ));
        }
    }
    lines.extend([
        String::new(),
        "## Missing Reuse Annotations".into(),
        String::new(),
    ]);
    let missing = missing_reuse_annotation_runs(state)
        .into_iter()
        .take(10)
        .collect::<Vec<_>>();
    if missing.is_empty() {
        lines.push(
            "- _All recorded runs have reusable finding, decision delta, and reuse note._".into(),
        );
    } else {
        for record in missing {
            lines.push(format!(
                "- {}: run `annotate-run --run-id {}` before treating this as reusable evidence.",
                str_field(&record, "run_id"),
                str_field(&record, "run_id")
            ));
        }
    }
    lines.join("\n")
}

pub(super) fn render_novelty_gate_summary(state: &Value) -> String {
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

pub(super) fn render_search_plan_summary(state: &Value) -> String {
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

pub(super) fn render_external_research_summary(state: &Value) -> String {
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
            "_No external research recorded yet. Run `research-claim` for one claim or `research-all` for a batch after drafting claims._".into(),
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

pub(super) fn render_claims_summary(state: &Value) -> String {
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

pub(super) fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => "-".into(),
        other => other.to_string(),
    }
}

pub(super) fn join_string_array(values: &[Value]) -> String {
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

pub(super) fn format_string_list(values: &[String], empty: &str) -> String {
    if values.is_empty() {
        return empty.to_string();
    }
    values
        .iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn render_current_context_summary(state: &Value) -> String {
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
            let display = str_field_default(record, "finding", "");
            let display = if display.is_empty() {
                str_field_default(record, "summary", "_No summary_")
            } else {
                display
            };
            lines.push(format!(
                "- {}: {}",
                str_field_default(record, "run_id", "-"),
                display
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
    let reusable = reusable_runs(state);
    lines.extend([
        String::new(),
        "### Reusable Evidence".into(),
        format!("- indexed reusable runs: {}", reusable.len()),
    ]);
    for record in reusable.iter().take(3) {
        lines.push(format!(
            "- {}: {}",
            str_field(record, "run_id"),
            str_field_default(record, "finding", "_No finding_")
        ));
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

pub(super) fn upsert_managed_block(text: &str, start_marker: &str, end_marker: &str, content: &str) -> String {
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

pub(super) fn sync_managed_file(
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

pub(super) fn format_hypothesis_card(hypothesis: &Value) -> String {
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
        "## Mechanism",
        "",
        &str_field_default(
            hypothesis,
            "mechanism",
            "_Why should this work, beyond changing a parameter?_",
        ),
        "",
        "## Prediction",
        "",
        &str_field_default(
            hypothesis,
            "prediction",
            "_Add the expected observable change._",
        ),
        "",
        "## Falsifiable Prediction",
        "",
        &str_field_default(
            hypothesis,
            "falsifiable_prediction",
            "_What observation would make this hypothesis weaker?_",
        ),
        "",
        "## Priority",
        "",
        &format!("`{}`", str_field_default(hypothesis, "priority", "medium")),
        "",
        "## Baselines / Controls",
        "",
        &format_string_list(
            &value_as_string_list(hypothesis, "baselines"),
            "_Closest simple baseline, ablation, or control._",
        ),
        "",
        "## Confounders",
        "",
        &format_string_list(
            &value_as_string_list(hypothesis, "confounders"),
            "_What could explain the result besides the proposed mechanism?_",
        ),
        "",
        "## Negative Signals",
        "",
        &format_string_list(
            &value_as_string_list(hypothesis, "negative_signals"),
            "_Early observations that should stop or reframe this branch._",
        ),
        "",
        "## Success Threshold",
        "",
        &str_field_default(
            hypothesis,
            "success_threshold",
            "_What metric or observation counts as a win?_",
        ),
        "",
        "## Minimal Decisive Test",
        "",
        &str_field_default(
            hypothesis,
            "minimal_test",
            "_Smallest test that can change the decision._",
        ),
        "",
        "## Stop Condition",
        "",
        &str_field_default(
            hypothesis,
            "stop_condition",
            "_When do we stop spending more budget on this branch?_",
        ),
        "",
    ]
    .join("\n")
}

pub(super) fn format_protocol(hypothesis: &Value) -> String {
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
        "## Proposed Mechanism",
        "",
        &str_field_default(
            hypothesis,
            "mechanism",
            "_Why should the change cause the predicted result?_",
        ),
        "",
        "## Prediction",
        "",
        &str_field_default(hypothesis, "prediction", "_What outcome do you expect?_"),
        "",
        "## Falsifiable Prediction",
        "",
        &str_field_default(
            hypothesis,
            "falsifiable_prediction",
            "_What observation would weaken the hypothesis?_",
        ),
        "",
        "## Metric",
        "",
        "_Primary metric plus sanity checks._",
        "",
        "## Baselines / Controls",
        "",
        &format_string_list(
            &value_as_string_list(hypothesis, "baselines"),
            "_Closest simple baseline, ablation, or control._",
        ),
        "",
        "## Confounders",
        "",
        &format_string_list(
            &value_as_string_list(hypothesis, "confounders"),
            "_What else could explain the result?_",
        ),
        "",
        "## Success Threshold",
        "",
        &str_field_default(
            hypothesis,
            "success_threshold",
            "_What result counts as success?_",
        ),
        "",
        "## Negative Signals",
        "",
        &format_string_list(
            &value_as_string_list(hypothesis, "negative_signals"),
            "_What result should stop or reframe the branch?_",
        ),
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
        "## Minimal Decisive Test",
        "",
        &str_field_default(
            hypothesis,
            "minimal_test",
            "_Smallest run that can update the decision._",
        ),
        "",
        "## Stop Condition",
        "",
        &str_field_default(
            hypothesis,
            "stop_condition",
            "_When do you stop this line?_",
        ),
        "",
    ]
    .join("\n")
}

pub(super) fn format_analysis_stub(hypothesis: &Value) -> String {
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
        "## Alternative Explanations",
        "",
        "_What else could explain the observed pattern?_",
        "",
        "## Baseline / Ablation Read",
        "",
        "_Did the result beat the right simple baseline or only tune around it?_",
        "",
        "## Open Questions",
        "",
        "_What still needs to be disambiguated?_",
        "",
    ]
    .join("\n")
}

pub(super) fn format_run_record(record: &Value) -> String {
    let metric_name = str_field_default(record, "metric_name", "metric");
    let metric_value = str_field_default(record, "metric_value", "value");
    let command = str_field_default(record, "command", "_not recorded_");
    let artifact_path = str_field_default(record, "evidence_path", "_not recorded_");
    let sanity_checks = value_as_string_list(record, "sanity_checks");
    let rules_in = value_as_string_list(record, "rules_in");
    let rules_out = value_as_string_list(record, "rules_out");
    let alternative_explanations = value_as_string_list(record, "alternative_explanations");
    let threats = value_as_string_list(record, "threats");
    let applies_to = value_as_string_list(record, "applies_to");
    let does_not_apply_to = value_as_string_list(record, "does_not_apply_to");
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
        "## Reusable Finding",
        "",
        &str_field_default(
            record,
            "finding",
            "_One reusable sentence: under what condition, what changed, and why it matters._",
        ),
        "",
        "## Decision Delta",
        "",
        &str_field_default(
            record,
            "decision_delta",
            "_What future decision should change because of this run?_",
        ),
        "",
        "## Reuse Scope",
        "",
        "### Applies To",
        "",
        &format_string_list(&applies_to, "_Where this result can be reused._"),
        "",
        "### Does Not Apply To",
        "",
        &format_string_list(&does_not_apply_to, "_Boundary conditions for reuse._"),
        "",
        "### Reuse Note",
        "",
        &str_field_default(
            record,
            "reuse_note",
            "_How to use this result later without rereading the whole log._",
        ),
        "",
        "## Metric Snapshot",
        "",
        &format!("- metric: {metric_name}"),
        &format!("- value: {metric_value}"),
        "",
        "## Sanity Checks",
        "",
        &format_string_list(&sanity_checks, "_Fill in sanity checks here._"),
        "",
        "## Baseline / Control Result",
        "",
        &str_field_default(
            record,
            "baseline_result",
            "_Compare against the simplest meaningful baseline or ablation._",
        ),
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
        "### Rules In",
        "",
        &format_string_list(&rules_in, "_What did this result support?_"),
        "",
        "### Rules Out",
        "",
        &format_string_list(&rules_out, "_What did this result eliminate?_"),
        "",
        "## Alternative Explanations",
        "",
        &format_string_list(
            &alternative_explanations,
            "_What else could explain the result?_",
        ),
        "",
        "## Threats To Interpretation",
        "",
        &format_string_list(&threats, "_What could make this conclusion misleading?_"),
        "",
        "## Interpretation",
        "",
        &str_field_default(
            record,
            "interpretation",
            "_Mechanistic interpretation, not just metric narration._",
        ),
        "",
    ]
    .join("\n")
}

pub(super) fn format_reflection_note(decision: &Value) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn escape_table_cell_pipe() {
        assert_eq!(escape_table_cell("a|b|c"), "a/b/c");
    }

    #[test]
    fn escape_table_cell_no_pipe() {
        assert_eq!(escape_table_cell("hello"), "hello");
    }

    #[test]
    fn format_overlap_risk_low() {
        assert_eq!(format_overlap_risk("low"), "🟢 low");
    }

    #[test]
    fn format_overlap_risk_high() {
        assert_eq!(format_overlap_risk("high"), "🔴 high");
    }

    #[test]
    fn format_overlap_risk_unknown_passthrough() {
        assert_eq!(format_overlap_risk("unknown"), "unknown");
    }

    #[test]
    fn summarize_rules_in_empty() {
        let state = json!({"rules": []});
        assert!(summarize_rules_in(&state).is_empty());
    }

    #[test]
    fn summarize_rules_in_with_rules() {
        let state = json!({"rules": [
            {"rule_id": "R1", "description": "Check A"},
            {"rule_id": "R2", "description": "Check B"},
        ]});
        let lines = summarize_rules_in(&state);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("R1"));
    }
}
