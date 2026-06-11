//! Entries management (recent/context/reuse), hypothesis/claim lifecycle:
//! find, transition, add, record_run, reflect, add_claim_comparison.

use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;

use crate::*;

pub(super) fn sort_entries_by_recency(entries: &[Value], timestamp_field: &str) -> Vec<Value> {
    let mut sorted = entries.to_vec();
    sorted.sort_by(|a, b| {
        let ta =
            parse_iso_timestamp(&str_field(a, timestamp_field)).unwrap_or(DateTime::<Utc>::MIN_UTC);
        let tb =
            parse_iso_timestamp(&str_field(b, timestamp_field)).unwrap_or(DateTime::<Utc>::MIN_UTC);
        tb.cmp(&ta)
    });
    sorted
}

pub(super) fn recent_entries(
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

pub(super) fn current_context_runs(state: &Value) -> Vec<Value> {
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

pub(super) fn current_context_decisions(state: &Value) -> Vec<Value> {
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

pub(super) fn reusable_runs(state: &Value) -> Vec<Value> {
    sort_entries_by_recency(arr(state, "run_history"), "recorded_at")
        .into_iter()
        .filter(|record| {
            !str_field_default(record, "finding", "").is_empty()
                || !str_field_default(record, "decision_delta", "").is_empty()
                || !str_field_default(record, "reuse_note", "").is_empty()
                || !value_as_string_list(record, "applies_to").is_empty()
                || !value_as_string_list(record, "does_not_apply_to").is_empty()
        })
        .collect()
}

pub(super) fn missing_reuse_annotation_runs(state: &Value) -> Vec<Value> {
    sort_entries_by_recency(arr(state, "run_history"), "recorded_at")
        .into_iter()
        .filter(|record| {
            str_field_default(record, "finding", "").is_empty()
                || str_field_default(record, "decision_delta", "").is_empty()
                || str_field_default(record, "reuse_note", "").is_empty()
        })
        .collect()
}

pub(super) fn reuse_audit(state: &Value) -> Value {
    let reusable = reusable_runs(state);
    let missing = missing_reuse_annotation_runs(state);
    json!({
        "runs": arr(state, "run_history").len(),
        "reusable_runs": reusable.len(),
        "missing_annotations": missing.len(),
        "missing_runs": missing
            .iter()
            .take(20)
            .map(|record| {
                let missing_fields = [
                    ("finding", str_field_default(record, "finding", "").is_empty()),
                    (
                        "decision_delta",
                        str_field_default(record, "decision_delta", "").is_empty(),
                    ),
                    (
                        "reuse_note",
                        str_field_default(record, "reuse_note", "").is_empty(),
                    ),
                ]
                .into_iter()
                .filter_map(|(field, missing)| missing.then_some(field))
                .collect::<Vec<_>>();
                json!({
                    "run_id": str_field(record, "run_id"),
                    "summary": str_field_default(record, "summary", "-"),
                    "missing": missing_fields,
                })
            })
            .collect::<Vec<_>>(),
        "generated_at": now_iso(),
    })
}

pub(super) fn format_reuse_audit(audit: &Value) -> String {
    let mut lines = vec![
        format!(
            "runs: {}",
            audit
                .get("runs")
                .map(value_to_string)
                .unwrap_or_else(|| "0".into())
        ),
        format!(
            "reusable_runs: {}",
            audit
                .get("reusable_runs")
                .map(value_to_string)
                .unwrap_or_else(|| "0".into())
        ),
        format!(
            "missing_annotations: {}",
            audit
                .get("missing_annotations")
                .map(value_to_string)
                .unwrap_or_else(|| "0".into())
        ),
        "missing_runs:".into(),
    ];
    for item in audit
        .get("missing_runs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let missing = item
            .get("missing")
            .and_then(Value::as_array)
            .map(|fields| join_string_array(fields))
            .unwrap_or_else(|| "_none_".into());
        lines.push(format!(
            "- {}: missing {}",
            str_field_default(item, "run_id", "?"),
            missing
        ));
    }
    lines.join("\n")
}

pub(super) struct Freshness {
    pub(super) stale: bool,
    pub(super) history_bias_risk: bool,
    pub(super) recent_runs: Vec<Value>,
    pub(super) recent_decisions: Vec<Value>,
}

pub(super) fn state_freshness(state: &Value) -> Freshness {
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

pub(super) fn recommend_next_actions(state: &Value) -> Vec<String> {
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
        actions.push("多个 claim 时直接跑 research-all，再用 gate-from-research 看缺口。".into());
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
    if str_field_default(latest_run, "finding", "").is_empty()
        || str_field_default(latest_run, "decision_delta", "").is_empty()
        || str_field_default(latest_run, "reuse_note", "").is_empty()
    {
        return vec![
            format!(
                "先给 {} 补 reusable finding、decision delta 和 reuse note，避免只留下流水账。",
                str_field(latest_run, "run_id")
            ),
            format!(
                "用 annotate-run --run-id {} 补齐 applies-to / does-not-apply-to。",
                str_field(latest_run, "run_id")
            ),
        ];
    }
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

pub(super) fn find_hypothesis<'a>(state: &'a Value, hypothesis_id: &str) -> Option<&'a Value> {
    arr(state, "hypotheses")
        .iter()
        .find(|item| item.get("id").and_then(Value::as_str) == Some(hypothesis_id))
}

pub(super) fn find_hypothesis_index(state: &Value, hypothesis_id: &str) -> Option<usize> {
    arr(state, "hypotheses")
        .iter()
        .position(|item| item.get("id").and_then(Value::as_str) == Some(hypothesis_id))
}

pub(super) fn choose_backlog_hypothesis(state: &Value) -> Option<&Value> {
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

pub(super) fn latest_run_for_hypothesis<'a>(state: &'a Value, hypothesis_id: &str) -> Option<&'a Value> {
    arr(state, "run_history")
        .iter()
        .rev()
        .find(|item| item.get("hypothesis_id").and_then(Value::as_str) == Some(hypothesis_id))
}

pub(super) fn latest_run_by_id<'a>(state: &'a Value, run_id: &str) -> Option<&'a Value> {
    arr(state, "run_history")
        .iter()
        .rev()
        .find(|item| item.get("run_id").and_then(Value::as_str) == Some(run_id))
}

pub(super) fn latest_decision_for_hypothesis<'a>(state: &'a Value, hypothesis_id: &str) -> Option<&'a Value> {
    arr(state, "decisions")
        .iter()
        .rev()
        .find(|item| item.get("hypothesis_id").and_then(Value::as_str) == Some(hypothesis_id))
}

pub(super) fn next_run_id(state: &Value) -> String {
    format!("run-{:03}", arr(state, "run_history").len() + 1)
}

pub(super) fn default_run_record_path(hypothesis_id: &str, run_id: &str) -> String {
    format!("experiments/{hypothesis_id}/{run_id}.md")
}

pub(super) fn default_reflection_path(hypothesis_id: &str, run_id: Option<&str>) -> String {
    format!(
        "experiments/{hypothesis_id}/{}-reflection.md",
        run_id.unwrap_or("reflection")
    )
}

pub(super) fn transition_hypothesis(
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

pub(super) struct HypothesisInput<'a> {
    pub(super) claim: &'a str,
    pub(super) prediction: Option<&'a str>,
    pub(super) mechanism: Option<&'a str>,
    pub(super) falsifiable_prediction: Option<&'a str>,
    pub(super) success_threshold: Option<&'a str>,
    pub(super) stop_condition: Option<&'a str>,
    pub(super) baselines: &'a [String],
    pub(super) confounders: &'a [String],
    pub(super) negative_signals: &'a [String],
    pub(super) minimal_test: Option<&'a str>,
    pub(super) priority: &'a str,
    pub(super) hypothesis_id: Option<&'a str>,
}

pub(super) fn add_hypothesis(state: &Value, input: HypothesisInput<'_>) -> Result<Value> {
    let mut next_state = ensure_state_defaults(state);
    let resolved_id = input
        .hypothesis_id
        .map(ToString::to_string)
        .unwrap_or_else(|| slugify(input.claim).chars().take(40).collect());
    if find_hypothesis(&next_state, &resolved_id).is_some() {
        bail!("Hypothesis already exists: {resolved_id}");
    }
    let entry = json!({
        "id": resolved_id,
        "claim": input.claim,
        "prediction": input.prediction,
        "mechanism": optional_string(input.mechanism),
        "falsifiable_prediction": optional_string(input.falsifiable_prediction.or(input.prediction)),
        "success_threshold": optional_string(input.success_threshold),
        "stop_condition": optional_string(input.stop_condition),
        "baselines": string_vec(input.baselines),
        "confounders": string_vec(input.confounders),
        "negative_signals": string_vec(input.negative_signals),
        "minimal_test": optional_string(input.minimal_test),
        "priority": input.priority,
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

pub(super) struct RunAnnotationInput<'a> {
    pub(super) finding: Option<&'a str>,
    pub(super) decision_delta: Option<&'a str>,
    pub(super) reuse_note: Option<&'a str>,
    pub(super) applies_to: &'a [String],
    pub(super) does_not_apply_to: &'a [String],
}

pub(super) fn merge_string_array(existing: &Value, additions: &[String]) -> Value {
    let mut merged = existing
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .filter(|item| !item.trim().is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for item in additions
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
    {
        if !merged.iter().any(|existing| existing == item) {
            merged.push(item.to_string());
        }
    }
    json!(merged)
}

pub(super) fn annotate_run(state: &Value, run_id: &str, input: RunAnnotationInput<'_>) -> Result<Value> {
    let mut next_state = ensure_state_defaults(state);
    let Some(record) = arr_mut(&mut next_state, "run_history")
        .iter_mut()
        .find(|item| item.get("run_id").and_then(Value::as_str) == Some(run_id))
    else {
        bail!("Unknown run id: {run_id}");
    };
    let item = record.as_object_mut().expect("run record must be object");
    if let Some(value) = input.finding {
        let value = optional_string(Some(value));
        if !value.is_null() {
            item.insert("finding".into(), value);
        }
    }
    if let Some(value) = input.decision_delta {
        let value = optional_string(Some(value));
        if !value.is_null() {
            item.insert("decision_delta".into(), value);
        }
    }
    if let Some(value) = input.reuse_note {
        let value = optional_string(Some(value));
        if !value.is_null() {
            item.insert("reuse_note".into(), value);
        }
    }
    let applies_to = merge_string_array(
        item.get("applies_to").unwrap_or(&Value::Null),
        input.applies_to,
    );
    item.insert("applies_to".into(), applies_to);
    let does_not_apply_to = merge_string_array(
        item.get("does_not_apply_to").unwrap_or(&Value::Null),
        input.does_not_apply_to,
    );
    item.insert("does_not_apply_to".into(), does_not_apply_to);
    item.insert("reuse_annotated_at".into(), json!(now_iso()));
    Ok(next_state)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn record_run(
    state: &Value,
    hypothesis_id: &str,
    outcome: &str,
    summary: &str,
    metric_name: Option<&str>,
    metric_value: Option<&str>,
    command: Option<&str>,
    evidence_path: Option<&str>,
    sanity_checks: &[String],
    baseline_result: Option<&str>,
    rules_in: &[String],
    rules_out: &[String],
    alternative_explanations: &[String],
    threats: &[String],
    interpretation: Option<&str>,
    finding: Option<&str>,
    decision_delta: Option<&str>,
    reuse_note: Option<&str>,
    applies_to: &[String],
    does_not_apply_to: &[String],
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
        "sanity_checks": string_vec(sanity_checks),
        "baseline_result": optional_string(baseline_result),
        "rules_in": string_vec(rules_in),
        "rules_out": string_vec(rules_out),
        "alternative_explanations": string_vec(alternative_explanations),
        "threats": string_vec(threats),
        "interpretation": optional_string(interpretation),
        "finding": optional_string(finding),
        "decision_delta": optional_string(decision_delta),
        "reuse_note": optional_string(reuse_note),
        "applies_to": string_vec(applies_to),
        "does_not_apply_to": string_vec(does_not_apply_to),
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

pub(super) fn reflect(
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
pub(super) fn add_claim_comparison(
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

