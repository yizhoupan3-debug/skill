//! Markdown 渲染管线：findings 摘要、novelty gate、search plan、external research、
//! claims、current context、reuse index、hypothesis card、protocol、run record、
//! reflection note、managed file sync。
//!
//! 从 `tools/autoresearch-rs/src/render.rs` 完整迁入。

use anyhow::Result;
use serde_json::Value;
use std::fs;
use std::path::Path;

use crate::util::{
    arr, novelty_gate, str_field, str_field_default, value_as_string_list, value_to_string,
};

// ── 自包含辅助函数 ──

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

fn novelty_str<'a>(state: &'a Value, key: &str, default: &'a str) -> &'a str {
    novelty_gate(state)
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
}

fn novelty_arr<'a>(state: &'a Value, key: &str) -> &'a [Value] {
    novelty_gate(state)
        .get(key)
        .and_then(Value::as_array)
        .map(|a| a.as_slice())
        .unwrap_or(&[])
}

fn format_string_list(values: &[String], empty: &str) -> String {
    if values.is_empty() {
        return empty.to_string();
    }
    values
        .iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n")
}

// ── Managed block 常量 ──

pub const FINDINGS_BLOCK_START: &str = "<!-- autoresearch:findings:start -->";
pub const FINDINGS_BLOCK_END: &str = "<!-- autoresearch:findings:end -->";
pub const NOVELTY_BLOCK_START: &str = "<!-- autoresearch:novelty:start -->";
pub const NOVELTY_BLOCK_END: &str = "<!-- autoresearch:novelty:end -->";
pub const SEARCH_PLAN_BLOCK_START: &str = "<!-- autoresearch:search-plan:start -->";
pub const SEARCH_PLAN_BLOCK_END: &str = "<!-- autoresearch:search-plan:end -->";
pub const EXTERNAL_RESEARCH_BLOCK_START: &str = "<!-- autoresearch:external-research:start -->";
pub const EXTERNAL_RESEARCH_BLOCK_END: &str = "<!-- autoresearch:external-research:end -->";
pub const CLAIMS_BLOCK_START: &str = "<!-- autoresearch:claims:start -->";
pub const CLAIMS_BLOCK_END: &str = "<!-- autoresearch:claims:end -->";
pub const CONTEXT_BLOCK_START: &str = "<!-- autoresearch:context:start -->";
pub const CONTEXT_BLOCK_END: &str = "<!-- autoresearch:context:end -->";
pub const REUSE_INDEX_BLOCK_START: &str = "<!-- autoresearch:reuse-index:start -->";
pub const REUSE_INDEX_BLOCK_END: &str = "<!-- autoresearch:reuse-index:end -->";

// ── 基础格式化 ──

pub fn escape_table_cell(value: &str) -> String {
    value.replace('|', "/")
}

pub fn format_overlap_risk(overlap: &str) -> String {
    match overlap {
        "low" => "🟢 low".into(),
        "medium" => "🟡 medium".into(),
        "high" => "🔴 high".into(),
        _ => overlap.into(),
    }
}

// ── Findings 摘要 ──

fn summarize_rules_in(state: &Value) -> Vec<String> {
    let mut lines = Vec::new();
    for record in crate::claims::lifecycle::current_context_runs(state) {
        let run_id = str_field(&record, "run_id");
        let finding = str_field_default(&record, "finding", "");
        if !finding.is_empty() {
            lines.push(format!("{run_id}: {finding}"));
            continue;
        }
        let rules = value_as_string_list(&record, "rules_in");
        if rules.is_empty() {
            lines.push(format!(
                "{run_id}: {}",
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

fn summarize_rules_out(state: &Value) -> Vec<String> {
    let mut lines = Vec::new();
    for record in crate::claims::lifecycle::current_context_runs(state) {
        let run_id = str_field(&record, "run_id");
        let rules = value_as_string_list(&record, "rules_out");
        if rules.is_empty() {
            if ["failed", "ambiguous"].contains(
                &record
                    .get("outcome")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
            ) {
                lines.push(format!(
                    "{run_id}: {}",
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

fn summarize_remaining_risks(state: &Value) -> Vec<String> {
    let blockers = arr(state, "blockers");
    if !blockers.is_empty() {
        return blockers
            .iter()
            .take(3)
            .map(|item| item.as_str().unwrap_or(&item.to_string()).to_string())
            .collect();
    }
    let mut risks = Vec::new();
    for record in crate::claims::lifecycle::current_context_runs(state) {
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

pub fn render_findings_summary(state: &Value) -> String {
    let mut lines = vec!["## Managed Summary".into(), String::new()];
    lines.push(format!(
        "- strongest current claim: {}",
        crate::search::research::strongest_current_claim(state)
    ));
    lines.push(String::new());
    lines.push("### What The Evidence Rules In".into());
    for item in summarize_rules_in(state) {
        lines.push(format!("- {item}"));
    }
    lines.push(String::new());
    lines.push("### What The Evidence Rules Out".into());
    for item in summarize_rules_out(state) {
        lines.push(format!("- {item}"));
    }
    lines.push(String::new());
    lines.push("### Remaining Risks".into());
    for item in summarize_remaining_risks(state) {
        lines.push(format!("- {item}"));
    }
    lines.push(String::new());
    lines.push("### Positioning Strategy".into());
    lines.push(format!(
        "- {}",
        novelty_gate(state)
            .get("differentiation_strategy")
            .and_then(Value::as_str)
            .unwrap_or("_Not recorded yet._")
    ));
    // Latest external research
    if let Some(entry) = crate::search::research::latest_external_research(state) {
        lines.push(String::new());
        lines.push("### Latest External Research".into());
        lines.push(format!("- query: {}", str_field_default(entry, "query", "-")));
        lines.push(format!(
            "- results: {}",
            crate::search::research::external_research_result_count(entry)
        ));
    }
    // Reuse notes
    let reusable: Vec<_> = crate::claims::lifecycle::current_context_runs(state)
        .into_iter()
        .filter(|record| {
            !str_field_default(record, "reuse_note", "").is_empty()
                || !str_field_default(record, "decision_delta", "").is_empty()
                || !value_as_string_list(record, "applies_to").is_empty()
        })
        .collect();
    if !reusable.is_empty() {
        lines.push(String::new());
        lines.push("### Reuse Notes".into());
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
    lines.join("\n")
}

// ── Novelty Gate 摘要 ──

pub fn render_novelty_gate_summary(state: &Value) -> String {
    let records = novelty_arr(state, "claim_records");
    let mut lines = vec![
        "## Managed Summary".into(),
        String::new(),
        format!(
            "- status: {}",
            novelty_str(state, "status", "pending")
        ),
        format!(
            "- overall novelty assessment: {}",
            crate::search::research::overall_novelty_assessment(state)
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
        "| Claim | Axis | Closest Prior Work | Overlap | Difference | Confidence | Verdict |".into(),
        "|---|---|---|---|---|---|---|".into(),
    ];
    if records.is_empty() {
        lines.push("| _none yet_ | - | - | - | - | - | - |".into());
    } else {
        for record in records {
            lines.push(format!(
                "| {} | {} | {} | {} | {} | {} | {} |",
                escape_table_cell(str_field_default(record, "claim", "_missing_")),
                escape_table_cell(str_field_default(record, "axis", "-")),
                escape_table_cell(str_field_default(record, "closest_prior_work", "-")),
                format_overlap_risk(str_field_default(record, "overlap", "-")),
                escape_table_cell(str_field_default(record, "difference", "-")),
                str_field_default(record, "confidence", "-"),
                str_field_default(record, "verdict", "-")
            ));
        }
    }
    lines.push(String::new());
    lines.push("## Differentiation Strategy".into());
    lines.push(String::new());
    lines.push(
        novelty_str(state, "differentiation_strategy", "_Not recorded yet._").to_string(),
    );
    lines.join("\n")
}

// ── Reuse Index 摘要 ──

pub fn render_reuse_index_summary(state: &Value) -> String {
    let runs = crate::claims::lifecycle::reusable_runs(state);
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
                escape_table_cell(str_field_default(record, "run_id", "-")),
                escape_table_cell(str_field_default(record, "finding", "-")),
                escape_table_cell(str_field_default(record, "decision_delta", "-")),
                escape_table_cell(&value_as_string_list(record, "applies_to").join("; ")),
                escape_table_cell(
                    &value_as_string_list(record, "does_not_apply_to").join("; ")
                ),
                escape_table_cell(str_field_default(record, "reuse_note", "-")),
            ));
        }
    }
    lines.push(String::new());
    lines.push("## Missing Reuse Annotations".into());
    lines.push(String::new());
    let missing = crate::claims::lifecycle::missing_reuse_annotation_runs(state);
    if missing.is_empty() {
        lines.push(
            "- _All recorded runs have reusable finding, decision delta, and reuse note._".into(),
        );
    } else {
        for record in missing.iter().take(10) {
            let run_id = str_field(record, "run_id");
            lines.push(format!(
                "- {run_id}: run `annotate-run --run-id {run_id}` before treating this as reusable evidence."
            ));
        }
    }
    lines.join("\n")
}

// ── Search Plan 摘要 ──

pub fn render_search_plan_summary(state: &Value) -> String {
    let plan = crate::search::strategy::current_search_plan(state);
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
        lines.push(format!(
            "### {} — {}",
            str_field_default(&entry, "claim_id", "C?"),
            str_field_default(&entry, "claim", "_missing_")
        ));
        lines.push(String::new());
        lines.push(format!("- axis: {}", str_field_default(&entry, "axis", "-")));
        lines.push(format!(
            "- recommended order: {}",
            entry
                .get("recommended_order")
                .map(value_to_string)
                .unwrap_or_else(|| "-".into())
        ));
        lines.push(format!(
            "- priority: {} ({})",
            str_field_default(&entry, "priority_label", "-"),
            entry
                .get("priority_score")
                .map(value_to_string)
                .unwrap_or_else(|| "-".into())
        ));
        lines.push(format!(
            "- why first or later: {}",
            str_field_default(&entry, "priority_reason", "-")
        ));
        lines.push(format!(
            "- keywords: {}",
            entry
                .get("keywords")
                .and_then(Value::as_array)
                .map(|values| join_string_array(values))
                .unwrap_or_else(|| "_none_".into())
        ));
        lines.push(format!(
            "- sources: {}",
            entry
                .get("sources")
                .and_then(Value::as_array)
                .map(|values| join_string_array(values))
                .unwrap_or_default()
        ));
        lines.push(String::new());
        lines.push("#### Query Ladder".into());
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
        lines.push(String::new());
        lines.push("#### Required Evidence".into());
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

// ── External Research 摘要 ──

pub fn render_external_research_summary(state: &Value) -> String {
    let entries = arr(state, "external_research");
    let mut lines = vec![
        "## Managed External Research".into(),
        String::new(),
        format!("- recorded searches: {}", entries.len()),
        "- sources: Semantic Scholar, arXiv".into(),
        String::new(),
    ];
    if entries.is_empty() {
        lines.push("_No external research recorded yet. Run `research-claim` for one claim or `research-all` for a batch after drafting claims._".into());
        return lines.join("\n");
    }
    for entry in entries.iter().rev().take(5) {
        lines.push(format!(
            "### {} — {}",
            str_field_default(entry, "research_id", "ext-?"),
            str_field_default(entry, "query", "-")
        ));
        lines.push(String::new());
        lines.push(format!(
            "- claim: {}",
            str_field_default(entry, "claim_id", "custom")
        ));
        lines.push(format!(
            "- source mode: {}",
            str_field_default(entry, "source", "all")
        ));
        lines.push(format!(
            "- captured at: {}",
            str_field_default(entry, "created_at", "-")
        ));
        lines.push(format!(
            "- result count: {}",
            crate::search::research::external_research_result_count(entry)
        ));
        lines.push(String::new());
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
                crate::text::markdown_link(result.get("url").and_then(Value::as_str))
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

// ── Claims 摘要 ──

pub fn render_claims_summary(state: &Value) -> String {
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
        lines.push(format!("### {}", str_field_default(draft, "claim_id", "C?")));
        lines.push(String::new());
        lines.push(format!(
            "- axis: {}",
            str_field_default(draft, "axis", "-")
        ));
        lines.push(format!(
            "- specificity: {}",
            str_field_default(draft, "specificity", "-")
        ));
        lines.push(format!(
            "- recommended order: {}",
            draft
                .get("recommended_order")
                .map(value_to_string)
                .unwrap_or_else(|| "-".into())
        ));
        lines.push(format!(
            "- priority: {} ({})",
            str_field_default(draft, "priority_label", "-"),
            draft
                .get("priority_score")
                .map(value_to_string)
                .unwrap_or_else(|| "-".into())
        ));
        lines.push(format!(
            "- why first or later: {}",
            str_field_default(draft, "priority_reason", "-")
        ));
        lines.push(format!(
            "- claim: {}",
            str_field_default(draft, "claim", "-")
        ));
        lines.push(String::new());
        lines.push("#### Required Evidence".into());
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

// ── Current Context 摘要 ──

pub fn render_current_context_summary(state: &Value) -> String {
    let freshness = crate::claims::lifecycle::state_freshness(state);
    let brief = crate::search::strategy::current_brief(state);
    let mut lines = vec![
        "## Managed Current Context".into(),
        String::new(),
        "- source of truth: `research-state.yaml`".into(),
        format!("- state updated_at: {}", str_field(state, "updated_at")),
        format!(
            "- freshness: {}",
            if freshness.stale { "stale" } else { "fresh" }
        ),
        format!(
            "- history bias risk: {}",
            if freshness.history_bias_risk {
                "high"
            } else {
                "low"
            }
        ),
        format!(
            "- active hypothesis: {}",
            state
                .get("active_hypothesis")
                .and_then(Value::as_str)
                .unwrap_or("-")
        ),
        format!(
            "- recommended focus: {}",
            crate::search::strategy::current_recommended_focus(state)
                .unwrap_or_else(|| "-".into())
        ),
        "- guardrail: treat `research-log.md` and older notes as background only unless they reappear in the current context window.".into(),
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
    lines.push(String::new());
    lines.push("### Recent Decisions".into());
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
    let reusable = crate::claims::lifecycle::reusable_runs(state);
    lines.push(String::new());
    lines.push("### Reusable Evidence".into());
    lines.push(format!("- indexed reusable runs: {}", reusable.len()));
    for record in reusable.iter().take(3) {
        lines.push(format!(
            "- {}: {}",
            str_field(record, "run_id"),
            str_field_default(record, "finding", "_No finding_")
        ));
    }
    if let Some(brief) = brief {
        lines.push(String::new());
        lines.push("### Active Novelty Brief".into());
        lines.push(format!(
            "- claim: {} — {}",
            str_field(&brief, "claim_id"),
            str_field(&brief, "claim")
        ));
        lines.push(format!(
            "- decision goal: {}",
            str_field(&brief, "decision_goal")
        ));
        lines.push(format!(
            "- verification standard: {}",
            str_field(&brief, "verification_standard")
        ));
        lines.push("- expected baselines:".into());
        for baseline in brief
            .get("expected_baselines")
            .and_then(Value::as_array)
            .unwrap_or(&Vec::new())
        {
            lines.push(format!("- {}", baseline.as_str().unwrap_or("")));
        }
    }
    if freshness.history_bias_risk {
        lines.push(String::new());
        lines.push("### Reconcile First".into());
        lines.push("- Confirm the active hypothesis is still the real target before trusting old notes.".into());
        lines.push("- Re-check live data, code, or current artifacts before extending any older conclusion.".into());
    }
    lines.join("\n")
}

// ── Hypothesis Card ──

pub fn format_hypothesis_card(hypothesis: &Value) -> String {
    [
        "# Hypothesis Card",
        "",
        "## Hypothesis ID",
        "",
        &format!("`{}`", str_field(hypothesis, "id")),
        "",
        "## Claim",
        "",
        str_field_default(hypothesis, "claim", "_TBD_"),
        "",
        "## Mechanism",
        "",
        str_field_default(
            hypothesis,
            "mechanism",
            "_Why should this work, beyond changing a parameter?_",
        ),
        "",
        "## Prediction",
        "",
        str_field_default(
            hypothesis,
            "prediction",
            "_Add the expected observable change._",
        ),
        "",
        "## Falsifiable Prediction",
        "",
        str_field_default(
            hypothesis,
            "falsifiable_prediction",
            "_What observation would make this hypothesis weaker?_",
        ),
        "",
        "## Priority",
        "",
        &format!(
            "`{}`",
            str_field_default(hypothesis, "priority", "medium")
        ),
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
        str_field_default(
            hypothesis,
            "success_threshold",
            "_What metric or observation counts as a win?_",
        ),
        "",
        "## Minimal Decisive Test",
        "",
        str_field_default(
            hypothesis,
            "minimal_test",
            "_Smallest test that can change the decision._",
        ),
        "",
        "## Stop Condition",
        "",
        str_field_default(
            hypothesis,
            "stop_condition",
            "_When do we stop spending more budget on this branch?_",
        ),
        "",
    ]
    .join("\n")
}

// ── Protocol ──

pub fn format_protocol(hypothesis: &Value) -> String {
    [
        "# Experiment Protocol",
        "",
        "## Hypothesis",
        "",
        str_field_default(hypothesis, "claim", "_Which hypothesis is being tested?_"),
        "",
        "## What Change",
        "",
        "_What changes in this run?_",
        "",
        "## Proposed Mechanism",
        "",
        str_field_default(
            hypothesis,
            "mechanism",
            "_Why should the change cause the predicted result?_",
        ),
        "",
        "## Prediction",
        "",
        str_field_default(hypothesis, "prediction", "_What outcome do you expect?_"),
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
        str_field_default(
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
        "## Minimal Decisive Test",
        "",
        str_field_default(
            hypothesis,
            "minimal_test",
            "_Smallest run that can update the decision._",
        ),
        "",
    ]
    .join("\n")
}

// ── Analysis Stub ──

pub fn format_analysis_stub(hypothesis: &Value) -> String {
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

// ── Run Record ──

pub fn format_run_record(record: &Value) -> String {
    let metric_name = str_field_default(record, "metric_name", "metric");
    let metric_value = str_field_default(record, "metric_value", "value");
    let command = str_field_default(record, "command", "_not recorded_");
    let artifact_path = str_field_default(record, "evidence_path", "_not recorded_");
    let sanity_checks = value_as_string_list(record, "sanity_checks");
    let rules_in = value_as_string_list(record, "rules_in");
    let rules_out = value_as_string_list(record, "rules_out");
    let alternative_explanations = value_as_string_list(record, "alternative_explanations");
    let threats = value_as_string_list(record, "threats");
    let _applies_to = value_as_string_list(record, "applies_to");
    let _does_not_apply_to = value_as_string_list(record, "does_not_apply_to");
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
        str_field_default(record, "summary", "_No summary recorded._"),
        "",
        "## Reusable Finding",
        "",
        str_field_default(
            record,
            "finding",
            "_One reusable sentence: under what condition, what changed, and why it matters._",
        ),
        "",
        "## Decision Delta",
        "",
        str_field_default(
            record,
            "decision_delta",
            "_What future decision should change because of this run?_",
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
        str_field_default(
            record,
            "interpretation",
            "_Mechanistic interpretation, not just metric narration._",
        ),
        "",
    ]
    .join("\n")
}

// ── Reflection Note ──

pub fn format_reflection_note(decision: &Value) -> String {
    [
        "# Reflection Note",
        "",
        "## Run",
        "",
        &format!(
            "`{}`",
            str_field_default(decision, "run_id", "run-xxx")
        ),
        "",
        "## What Happened",
        "",
        str_field_default(decision, "reason", "_Summarize the observed pattern._"),
        "",
        "## Why It Probably Happened",
        "",
        str_field_default(
            decision,
            "reason",
            "_Mechanistic explanation or best current guess._",
        ),
        "",
        "## Direction",
        "",
        &format!(
            "`{}`",
            str_field_default(decision, "direction", "DEEPEN")
        ),
        "",
        "## Next Step",
        "",
        str_field_default(decision, "next_step", "_One concrete next move only._"),
        "",
    ]
    .join("\n")
}

// ── Resume ──

pub fn format_resume(state: &Value) -> String {
    let freshness = crate::claims::lifecycle::state_freshness(state);
    let brief = crate::search::strategy::current_brief(state);
    let mut lines = vec![
        format!("question: {}", str_field(state, "question")),
        format!("stage: {}", str_field(state, "stage")),
        format!("novelty_gate: {}", novelty_str(state, "status", "-")),
        format!(
            "novelty_assessment: {}",
            crate::search::research::overall_novelty_assessment(state)
        ),
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
            crate::search::strategy::current_recommended_focus(state)
                .unwrap_or_else(|| "-".into())
        ),
        format!(
            "novelty_brief_claim: {}",
            brief
                .as_ref()
                .and_then(|item| item.get("claim_id"))
                .and_then(Value::as_str)
                .unwrap_or("-")
        ),
        format!(
            "active_hypothesis: {}",
            state
                .get("active_hypothesis")
                .and_then(Value::as_str)
                .unwrap_or("-")
        ),
    ];
    if let Some(run) = freshness.recent_runs.first() {
        lines.push(format!(
            "latest_run: {} ({})",
            str_field(run, "run_id"),
            str_field(run, "outcome")
        ));
        lines.push(format!(
            "latest_finding: {}",
            str_field_default(run, "finding", "-")
        ));
    }
    if let Some(decision) = freshness.recent_decisions.first() {
        lines.push(format!(
            "latest_direction: {}",
            str_field_default(decision, "direction", "-")
        ));
    }
    lines.push(format!(
        "draft_claims: {}",
        novelty_arr(state, "draft_claims").len()
    ));
    lines.push(format!(
        "search_plan_entries: {}",
        crate::search::strategy::current_search_plan(state).len()
    ));
    lines.push(format!(
        "reusable_runs: {}",
        crate::claims::lifecycle::reusable_runs(state).len()
    ));
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

// ── Status ──

pub fn format_status(state: &Value) -> String {
    let mut lines = vec![
        format!("project: {}", str_field(state, "project")),
        format!("stage: {}", str_field(state, "stage")),
        format!("status: {}", str_field(state, "status")),
        format!("mode: {}", str_field(state, "mode")),
        format!(
            "active_hypothesis: {}",
            state
                .get("active_hypothesis")
                .and_then(Value::as_str)
                .unwrap_or("-")
        ),
        format!("novelty_gate: {}", novelty_str(state, "status", "-")),
        format!(
            "novelty_assessment: {}",
            crate::search::research::overall_novelty_assessment(state)
        ),
        format!("hypotheses: {}", arr(state, "hypotheses").len()),
        format!("runs: {}", arr(state, "run_history").len()),
        format!(
            "reusable_runs: {}",
            crate::claims::lifecycle::reusable_runs(state).len()
        ),
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

// ── Managed File Sync ──

pub fn upsert_managed_block(
    text: &str,
    block_start: &str,
    block_end: &str,
    content: &str,
) -> String {
    let managed = format!("{block_start}\n{}\n{block_end}", content.trim_end());
    if let Some(start_pos) = text.find(block_start) {
        if let Some(end_pos) = text.find(block_end) {
            let end_with_marker = end_pos + block_end.len();
            let mut result = String::new();
            result.push_str(&text[..start_pos]);
            result.push_str(&managed);
            result.push_str(&text[end_with_marker..]);
            return result;
        }
    }
    if text.is_empty() {
        managed
    } else {
        format!("{text}\n\n{managed}")
    }
}

pub fn sync_managed_file(
    path: &Path,
    header: &str,
    block_start: &str,
    block_end: &str,
    content: String,
) -> Result<()> {
    let existing = fs::read_to_string(path).unwrap_or_default();
    let updated = upsert_managed_block(&existing, block_start, block_end, &content);
    let final_content = if existing.is_empty() {
        format!("{header}{updated}")
    } else {
        updated
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, final_content)?;
    Ok(())
}

// ── 测试 ──

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn escape_table_cell_replaces_pipe() {
        assert_eq!(escape_table_cell("foo|bar"), "foo/bar");
    }

    #[test]
    fn format_overlap_risk_all_levels() {
        assert!(format_overlap_risk("low").contains("low"));
        assert!(format_overlap_risk("medium").contains("medium"));
        assert!(format_overlap_risk("high").contains("high"));
    }

    #[test]
    fn upsert_managed_block_inserts_new() {
        let result = upsert_managed_block("existing", "<!--S-->", "<!--E-->", "new data");
        assert!(result.contains("new data"));
        assert!(result.contains("existing"));
    }

    #[test]
    fn upsert_managed_block_replaces_existing() {
        let text = "before\n<!--S-->\nold\n<!--E-->\nafter";
        let result = upsert_managed_block(text, "<!--S-->", "<!--E-->", "updated");
        assert!(result.contains("updated"));
        assert!(!result.contains("old"));
    }

    #[test]
    fn render_findings_summary_basic() {
        let state = json!({"run_history": [], "blockers": [], "novelty_gate": {"claims": ["c1"]}});
        let summary = render_findings_summary(&state);
        assert!(summary.contains("Managed Summary"));
    }

    #[test]
    fn render_novelty_gate_summary_basic() {
        let state = json!({"novelty_gate": {"status": "passed", "claim_records": [{"claim": "c1", "axis": "method", "closest_prior_work": "pw", "overlap": "low", "difference": "d", "confidence": "high", "verdict": "novel"}]}});
        let summary = render_novelty_gate_summary(&state);
        assert!(summary.contains("passed"));
        assert!(summary.contains("c1"));
    }

    #[test]
    fn render_reuse_index_summary_empty() {
        let state = json!({"run_history": []});
        let summary = render_reuse_index_summary(&state);
        assert!(summary.contains("Reuse Index"));
        assert!(summary.contains("none yet"));
    }

    #[test]
    fn render_search_plan_summary_empty() {
        let state = json!({"novelty_gate": {"claim_records": [], "draft_claims": []}});
        let summary = render_search_plan_summary(&state);
        assert!(summary.contains("Search Plan"));
    }

    #[test]
    fn render_claims_summary_basic() {
        let state = json!({"novelty_gate": {"draft_claims": [{"claim_id": "C1", "axis": "method", "claim": "test"}]}});
        let summary = render_claims_summary(&state);
        assert!(summary.contains("C1"));
    }

    #[test]
    fn render_current_context_summary_basic() {
        let state = json!({"run_history": [], "decisions": [], "next_actions": []});
        let summary = render_current_context_summary(&state);
        assert!(summary.contains("Current Context"));
    }

    #[test]
    fn format_hypothesis_card_basic() {
        let h = json!({"id": "h1", "claim": "test claim", "priority": "high"});
        let card = format_hypothesis_card(&h);
        assert!(card.contains("Hypothesis Card"));
        assert!(card.contains("h1"));
        assert!(card.contains("test claim"));
    }

    #[test]
    fn format_protocol_basic() {
        let h = json!({"id": "h1", "claim": "test"});
        let protocol = format_protocol(&h);
        assert!(protocol.contains("Experiment Protocol"));
    }

    #[test]
    fn format_run_record_basic() {
        let record = json!({"run_id": "run-001", "hypothesis_id": "h1", "outcome": "confirmatory", "summary": "test"});
        let rr = format_run_record(&record);
        assert!(rr.contains("Run Record"));
        assert!(rr.contains("run-001"));
    }

    #[test]
    fn format_reflection_note_basic() {
        let decision = json!({"run_id": "run-001", "direction": "DEEPEN", "reason": "interesting"});
        let note = format_reflection_note(&decision);
        assert!(note.contains("Reflection Note"));
        assert!(note.contains("DEEPEN"));
    }

    #[test]
    fn format_status_basic() {
        let state = json!({
            "project": "test", "stage": "bootstrap", "status": "active",
            "mode": "quick", "active_hypothesis": null, "novelty_gate": {"status": "pending"},
            "hypotheses": [], "run_history": [], "decisions": [],
            "next_actions": ["action1"]
        });
        let status = format_status(&state);
        assert!(status.contains("test"));
        assert!(status.contains("action1"));
    }

    #[test]
    fn format_resume_basic() {
        let state = json!({
            "question": "q?", "stage": "bootstrap", "novelty_gate": {"status": "pending"},
            "run_history": [], "decisions": [], "next_actions": []
        });
        let resume = format_resume(&state);
        assert!(resume.contains("q?"));
    }

    #[test]
    fn sync_managed_file_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.md");
        sync_managed_file(&path, "# Header\n\n", "<!--S-->", "<!--E-->", "content".into())
            .unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("# Header"));
        assert!(content.contains("content"));
    }
}
