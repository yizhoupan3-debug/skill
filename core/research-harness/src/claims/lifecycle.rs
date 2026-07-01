//! Hypothesis / Run / Reflect 生命周期状态机。
//!
//! 管理 research workspace 中的 hypothesis 创建、状态转换、run 记录、
//! 方向反思、run 注解和 claim 比较。所有函数操作 serde_json::Value 状态对象。

use anyhow::{Result, anyhow, bail};
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use std::path::Path;

use crate::util::{
    arr, arr_mut, novelty_arr, novelty_gate_mut, novelty_str, set_key, str_field,
    str_field_default, value_as_string_list,
};

// ── 自包含辅助函数 ──

fn parse_iso_timestamp(ts: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(ts)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn days_since(ts: &str) -> Option<i64> {
    parse_iso_timestamp(ts).map(|dt| (Utc::now() - dt).num_days())
}

fn optional_string(value: Option<&str>) -> Value {
    match value.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => json!(s),
        None => Value::Null,
    }
}

fn string_vec(items: &[String]) -> Value {
    json!(
        items
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
    )
}

fn merge_string_array(existing: &Value, additions: &[String]) -> Value {
    let mut merged: Vec<String> = existing
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();
    for item in additions.iter().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        if !merged.iter().any(|e| e == item) {
            merged.push(item.to_string());
        }
    }
    json!(merged)
}

fn ensure_state_defaults(state: &Value) -> Result<Value> {
    // Propagate hydration errors: silent fallback would lose data (A2).
    crate::state::hydrate_state(state).map_err(|e| {
        anyhow::anyhow!(
            "ensure_state_defaults: hydrate_state failed: {e}. \
             State may be corrupt (non-object in hypotheses/run_history/etc)."
        )
    })
}

// ── 常量 ──

const STAGE_BOOTSTRAP: &str = "bootstrap";
const STAGE_INNER_LOOP: &str = "inner-loop";
const STAGE_OUTER_LOOP: &str = "outer-loop";
const STAGE_FINALIZE: &str = "finalize";
const STALE_STATE_DAYS: i64 = 10;
const RECENT_ACTIVITY_DAYS: i64 = 14;
const FALLBACK_ACTIVITY_LIMIT: usize = 3;
const SCHEMA_VERSION: i64 = 4;

// ── 入口管理 ──

pub fn sort_entries_by_recency(entries: &[Value], timestamp_field: &str) -> Vec<Value> {
    let mut sorted = entries.to_vec();
    sorted.sort_by(|a, b| {
        let ta =
            parse_iso_timestamp(str_field(a, timestamp_field)).unwrap_or(DateTime::<Utc>::MIN_UTC);
        let tb =
            parse_iso_timestamp(str_field(b, timestamp_field)).unwrap_or(DateTime::<Utc>::MIN_UTC);
        tb.cmp(&ta)
    });
    sorted
}

pub fn current_context_runs(state: &Value) -> Vec<Value> {
    let runs = arr(state, "run_history");
    let active_id = state.get("active_hypothesis").and_then(Value::as_str);
    if let Some(active_id) = active_id {
        let recent: Vec<_> = sort_entries_by_recency(runs, "recorded_at")
            .into_iter()
            .filter(|r| r.get("hypothesis_id").and_then(Value::as_str) == Some(active_id))
            .filter(|r| {
                days_since(str_field(r, "recorded_at")).is_some_and(|d| d <= RECENT_ACTIVITY_DAYS)
            })
            .take(FALLBACK_ACTIVITY_LIMIT)
            .collect();
        if !recent.is_empty() {
            return recent;
        }
    }
    sort_entries_by_recency(runs, "recorded_at")
        .into_iter()
        .take(FALLBACK_ACTIVITY_LIMIT)
        .collect()
}

pub fn reusable_runs(state: &Value) -> Vec<Value> {
    sort_entries_by_recency(arr(state, "run_history"), "recorded_at")
        .into_iter()
        .filter(|r| {
            !str_field_default(r, "finding", "").is_empty()
                || !str_field_default(r, "decision_delta", "").is_empty()
                || !str_field_default(r, "reuse_note", "").is_empty()
                || !value_as_string_list(r, "applies_to").is_empty()
                || !value_as_string_list(r, "does_not_apply_to").is_empty()
        })
        .collect()
}

pub fn reuse_audit(state: &Value) -> Value {
    let reusable = reusable_runs(state);
    let missing: Vec<_> = sort_entries_by_recency(arr(state, "run_history"), "recorded_at")
        .into_iter()
        .filter(|r| {
            str_field_default(r, "finding", "").is_empty()
                || str_field_default(r, "decision_delta", "").is_empty()
                || str_field_default(r, "reuse_note", "").is_empty()
        })
        .take(20)
        .map(|r| {
            json!({
                "run_id": str_field(&r, "run_id"),
                "summary": str_field_default(&r, "summary", "-"),
            })
        })
        .collect();
    json!({
        "runs": arr(state, "run_history").len(),
        "reusable_runs": reusable.len(),
        "missing_annotations": missing.len(),
        "missing_runs": missing,
        "generated_at": framework_core::time::now_iso(),
    })
}

// ── Hypothesis 管理 ──

pub fn find_hypothesis<'a>(state: &'a Value, hypothesis_id: &str) -> Option<&'a Value> {
    arr(state, "hypotheses")
        .iter()
        .find(|h| h.get("id").and_then(Value::as_str) == Some(hypothesis_id))
}

pub fn find_hypothesis_index(state: &Value, hypothesis_id: &str) -> Option<usize> {
    arr(state, "hypotheses")
        .iter()
        .position(|h| h.get("id").and_then(Value::as_str) == Some(hypothesis_id))
}

pub fn next_run_id(state: &Value) -> String {
    format!("run-{:03}", arr(state, "run_history").len() + 1)
}

pub fn transition_hypothesis(
    state: &mut Value,
    index: usize,
    new_status: &str,
    reason: Option<&str>,
) -> Result<()> {
    let hypotheses = arr_mut(state, "hypotheses")?;
    let h = hypotheses
        .get_mut(index)
        .ok_or_else(|| anyhow!("Missing hypothesis index"))?;
    let previous = h.get("status").and_then(Value::as_str);
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
            "Invalid hypothesis transition: {} -> {}",
            previous.unwrap_or("none"),
            new_status
        );
    }
    let hid = str_field(h, "id").to_string();
    let obj = h
        .as_object_mut()
        .ok_or_else(|| anyhow!("hypothesis must be object"))?;
    obj.insert("status".into(), json!(new_status));
    obj.insert(
        "status_reason".into(),
        reason.map(Value::from).unwrap_or(Value::Null),
    );
    obj.insert(
        "status_updated_at".into(),
        json!(framework_core::time::now_iso()),
    );
    let backlog = arr_mut(state, "hypothesis_backlog")?;
    if new_status == "queued" {
        if !backlog.iter().any(|v| v.as_str() == Some(&hid)) {
            backlog.push(json!(hid));
        }
    } else if let Some(pos) = backlog.iter().position(|v| v.as_str() == Some(&hid)) {
        backlog.remove(pos);
    }
    Ok(())
}

pub struct HypothesisInput<'a> {
    pub claim: &'a str,
    pub prediction: Option<&'a str>,
    pub mechanism: Option<&'a str>,
    pub falsifiable_prediction: Option<&'a str>,
    pub success_threshold: Option<&'a str>,
    pub stop_condition: Option<&'a str>,
    pub baselines: &'a [String],
    pub confounders: &'a [String],
    pub negative_signals: &'a [String],
    pub minimal_test: Option<&'a str>,
    pub priority: &'a str,
    pub hypothesis_id: Option<&'a str>,
}

pub fn add_hypothesis(state: &Value, input: HypothesisInput<'_>) -> Result<Value> {
    let mut next = ensure_state_defaults(state)?;
    let id = input
        .hypothesis_id
        .map(ToString::to_string)
        .unwrap_or_else(|| {
            let slug = crate::text::slugify(input.claim);
            if slug.len() <= 40 {
                slug
            } else {
                // Add 8-char hash suffix to avoid collisions when truncated
                use sha2::{Digest, Sha256};
                let hash = Sha256::digest(input.claim.as_bytes());
                let suffix: String = hash.iter().take(4).map(|b| format!("{:02x}", b)).collect();
                format!("{}-{}", slug.chars().take(31).collect::<String>(), suffix)
            }
        });
    if find_hypothesis(&next, &id).is_some() {
        bail!("Hypothesis already exists: {id}");
    }
    let entry = json!({
        "id": id, "claim": input.claim, "prediction": input.prediction,
        "mechanism": optional_string(input.mechanism),
        "falsifiable_prediction": optional_string(input.falsifiable_prediction.or(input.prediction)),
        "success_threshold": optional_string(input.success_threshold),
        "stop_condition": optional_string(input.stop_condition),
        "baselines": string_vec(input.baselines), "confounders": string_vec(input.confounders),
        "negative_signals": string_vec(input.negative_signals),
        "minimal_test": optional_string(input.minimal_test),
        "priority": input.priority, "status": "queued", "status_reason": Value::Null,
        "status_updated_at": framework_core::time::now_iso(), "created_at": framework_core::time::now_iso(),
    });
    arr_mut(&mut next, "hypotheses")?.push(entry);
    let backlog = arr_mut(&mut next, "hypothesis_backlog")?;
    if !backlog.iter().any(|v| v.as_str() == Some(&id)) {
        backlog.push(json!(id.clone()));
    }
    if next
        .get("active_hypothesis")
        .and_then(Value::as_str)
        .is_none()
        && novelty_str(&next, "status", "pending") == "passed"
    {
        set_key(&mut next, "active_hypothesis", json!(id.clone()));
        let idx = find_hypothesis_index(&next, &id)
            .ok_or_else(|| anyhow!("hypothesis just inserted must exist"))?;
        transition_hypothesis(
            &mut next,
            idx,
            "active",
            Some("first active hypothesis after novelty gate passed"),
        )?;
    }
    Ok(next)
}

// ── Run 记录 ──

pub struct RecordRunInput<'a> {
    pub hypothesis_id: &'a str,
    pub outcome: &'a str,
    pub summary: &'a str,
    pub metric_name: Option<&'a str>,
    pub metric_value: Option<&'a str>,
    pub command: Option<&'a str>,
    pub evidence_path: Option<&'a str>,
    pub sanity_checks: &'a [String],
    pub baseline_result: Option<&'a str>,
    pub rules_in: &'a [String],
    pub rules_out: &'a [String],
    pub alternative_explanations: &'a [String],
    pub threats: &'a [String],
    pub interpretation: Option<&'a str>,
    pub finding: Option<&'a str>,
    pub decision_delta: Option<&'a str>,
    pub reuse_note: Option<&'a str>,
    pub applies_to: &'a [String],
    pub does_not_apply_to: &'a [String],
    pub override_novelty_gate: bool,
    pub override_reason: Option<&'a str>,
}

pub fn record_run(state: &Value, input: &RecordRunInput<'_>, _workspace: &Path) -> Result<Value> {
    let mut next = ensure_state_defaults(state)?;
    let Some(index) = find_hypothesis_index(&next, input.hypothesis_id) else {
        bail!("Unknown hypothesis: {}", input.hypothesis_id);
    };
    let gate_status = novelty_str(&next, "status", "pending").to_string();
    if gate_status != "passed" {
        if !input.override_novelty_gate {
            bail!("Novelty gate must pass before recording runs (current: {gate_status})");
        }
        if input.override_reason.unwrap_or("").trim().is_empty() {
            bail!("Novelty gate override requires --override-reason");
        }
    }
    // Reuse the index from find_hypothesis_index (avoids a second O(n) scan).
    let current_status = {
        let hypotheses = arr(&next, "hypotheses");
        let h = hypotheses.get(index).ok_or_else(|| {
            anyhow!(
                "Hypothesis {} missing at index {}",
                input.hypothesis_id,
                index
            )
        })?;
        h.get("status")
            .and_then(Value::as_str)
            .unwrap_or("queued")
            .to_string()
    };
    if !["active", "queued"].contains(&current_status.as_str()) {
        bail!(
            "Hypothesis {} must be active or queued, current: {current_status}",
            input.hypothesis_id
        );
    }
    if current_status == "queued" {
        transition_hypothesis(
            &mut next,
            index,
            "active",
            Some("activated by first recorded run"),
        )?;
    }
    let run_id = next_run_id(&next);
    set_key(&mut next, "stage", json!(STAGE_OUTER_LOOP));
    set_key(&mut next, "active_hypothesis", json!(input.hypothesis_id));
    let record = json!({
        "run_id": run_id, "hypothesis_id": input.hypothesis_id, "outcome": input.outcome,
        "summary": input.summary, "metric_name": input.metric_name, "metric_value": input.metric_value,
        "command": input.command, "evidence_path": input.evidence_path,
        "sanity_checks": string_vec(input.sanity_checks), "baseline_result": optional_string(input.baseline_result),
        "rules_in": string_vec(input.rules_in), "rules_out": string_vec(input.rules_out),
        "alternative_explanations": string_vec(input.alternative_explanations), "threats": string_vec(input.threats),
        "interpretation": optional_string(input.interpretation), "finding": optional_string(input.finding),
        "decision_delta": optional_string(input.decision_delta), "reuse_note": optional_string(input.reuse_note),
        "applies_to": string_vec(input.applies_to), "does_not_apply_to": string_vec(input.does_not_apply_to),
        "novelty_gate_status_at_run": gate_status, "novelty_gate_override": input.override_novelty_gate,
        "override_reason": input.override_reason,
        "recorded_at": framework_core::time::now_iso(),
    });
    arr_mut(&mut next, "run_history")?.push(record);
    transition_hypothesis(
        &mut next,
        index,
        "needs_reflection",
        Some(&format!("{run_id} recorded")),
    )?;
    Ok(next)
}

pub fn latest_run_for_hypothesis<'a>(state: &'a Value, hypothesis_id: &str) -> Option<&'a Value> {
    arr(state, "run_history")
        .iter()
        .rev()
        .find(|r| r.get("hypothesis_id").and_then(Value::as_str) == Some(hypothesis_id))
}

// ── Run 注解 ──

pub struct RunAnnotationInput<'a> {
    pub finding: Option<&'a str>,
    pub decision_delta: Option<&'a str>,
    pub reuse_note: Option<&'a str>,
    pub applies_to: &'a [String],
    pub does_not_apply_to: &'a [String],
}

pub fn annotate_run(state: &Value, run_id: &str, input: RunAnnotationInput<'_>) -> Result<Value> {
    let mut next = ensure_state_defaults(state)?;
    let Some(record) = arr_mut(&mut next, "run_history")?
        .iter_mut()
        .find(|r| r.get("run_id").and_then(Value::as_str) == Some(run_id))
    else {
        bail!("Unknown run id: {run_id}");
    };
    let obj = record
        .as_object_mut()
        .ok_or_else(|| anyhow!("run record must be object"))?;
    if let Some(v) = input.finding {
        let v = optional_string(Some(v));
        if !v.is_null() {
            obj.insert("finding".into(), v);
        }
    }
    if let Some(v) = input.decision_delta {
        let v = optional_string(Some(v));
        if !v.is_null() {
            obj.insert("decision_delta".into(), v);
        }
    }
    if let Some(v) = input.reuse_note {
        let v = optional_string(Some(v));
        if !v.is_null() {
            obj.insert("reuse_note".into(), v);
        }
    }
    obj.insert(
        "applies_to".into(),
        merge_string_array(
            obj.get("applies_to").unwrap_or(&Value::Null),
            input.applies_to,
        ),
    );
    obj.insert(
        "does_not_apply_to".into(),
        merge_string_array(
            obj.get("does_not_apply_to").unwrap_or(&Value::Null),
            input.does_not_apply_to,
        ),
    );
    obj.insert(
        "reuse_annotated_at".into(),
        json!(framework_core::time::now_iso()),
    );
    Ok(next)
}

// ── 反思 ──

pub fn latest_decision_for_hypothesis<'a>(
    state: &'a Value,
    hypothesis_id: &str,
) -> Option<&'a Value> {
    arr(state, "decisions")
        .iter()
        .rev()
        .find(|d| d.get("hypothesis_id").and_then(Value::as_str) == Some(hypothesis_id))
}

pub fn reflect(
    state: &Value,
    hypothesis_id: &str,
    direction: &str,
    reason: &str,
    next_step: Option<&str>,
    activate_hypothesis: Option<&str>,
) -> Result<Value> {
    let mut next = ensure_state_defaults(state)?;
    let Some(index) = find_hypothesis_index(&next, hypothesis_id) else {
        bail!("Unknown hypothesis: {hypothesis_id}");
    };
    let status = find_hypothesis(&next, hypothesis_id)
        .and_then(|h| h.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("-");
    if status != "needs_reflection" {
        bail!("Hypothesis {hypothesis_id} must be in needs_reflection, current: {status}");
    }
    let latest_run = latest_run_for_hypothesis(&next, hypothesis_id)
        .ok_or_else(|| anyhow!("Cannot reflect without a recorded run"))?;
    if let Some(latest_decision) = latest_decision_for_hypothesis(&next, hypothesis_id) {
        if latest_decision.get("run_id") == latest_run.get("run_id") {
            bail!(
                "Run {} already has a reflection",
                str_field(latest_run, "run_id")
            );
        }
    }
    let run_id = str_field(latest_run, "run_id").to_string();
    let decision = json!({
        "hypothesis_id": hypothesis_id, "run_id": run_id, "direction": direction,
        "reason": reason, "interpretation": reason, "next_step": next_step,
        "recorded_at": framework_core::time::now_iso(),
    });
    arr_mut(&mut next, "decisions")?.push(decision);
    set_key(&mut next, "current_direction", json!(direction));
    match direction {
        "CONCLUDE" => {
            transition_hypothesis(&mut next, index, "concluded", Some(reason))?;
            // Only set global status if ALL hypotheses are concluded
            let all_concluded = arr(&next, "hypotheses")
                .iter()
                .all(|h| h.get("status").and_then(Value::as_str) == Some("concluded"));
            if all_concluded {
                set_key(&mut next, "status", json!("concluded"));
                set_key(&mut next, "stage", json!(STAGE_FINALIZE));
            }
        }
        "PIVOT" => {
            set_key(&mut next, "stage", json!(STAGE_INNER_LOOP));
            transition_hypothesis(&mut next, index, "parked", Some(reason))?;
        }
        "DEEPEN" | "BROADEN" => {
            set_key(&mut next, "stage", json!(STAGE_INNER_LOOP));
            transition_hypothesis(&mut next, index, "active", Some(reason))?;
        }
        _ => {
            return Err(anyhow::anyhow!(
                "unknown reflect direction: '{direction}'. \
                 Expected DEEPEN, BROADEN, PIVOT, or CONCLUDE."
            ));
        }
    }
    if let Some(target_id) = activate_hypothesis {
        let Some(target_index) = find_hypothesis_index(&next, target_id) else {
            bail!("Unknown activate_hypothesis: {target_id}");
        };
        set_key(&mut next, "active_hypothesis", json!(target_id));
        let target_status = find_hypothesis(&next, target_id)
            .and_then(|h| h.get("status"))
            .and_then(Value::as_str)
            .unwrap_or("-");
        if target_status == "queued" || target_status == "parked" {
            transition_hypothesis(
                &mut next,
                target_index,
                "active",
                Some("activated after pivot"),
            )?;
        }
    } else if direction != "CONCLUDE" {
        set_key(&mut next, "active_hypothesis", json!(hypothesis_id));
    }
    Ok(next)
}

// ── Claim 比较 ──

#[allow(clippy::too_many_arguments)]
pub fn add_claim_comparison(
    state: &Value,
    claim: &str,
    axis: &str,
    closest_prior_work: &str,
    overlap: &str,
    difference: &str,
    confidence: &str,
    verdict: &str,
    claim_id: Option<&str>,
) -> Result<Value> {
    let mut next = ensure_state_defaults(state)?;
    let id = claim_id
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("C{}", novelty_arr(&next, "claim_records").len() + 1));
    let record = json!({
        "claim_id": id, "claim": claim, "axis": axis, "closest_prior_work": closest_prior_work,
        "overlap": overlap, "difference": difference, "confidence": confidence,
        "verdict": verdict, "recorded_at": framework_core::time::now_iso(),
    });
    let gate = novelty_gate_mut(&mut next);
    let claims_list;
    {
        let records = gate
            .entry("claim_records".to_string())
            .or_insert(json!([]))
            .as_array_mut()
            .ok_or_else(|| anyhow!("claim_records must be an array"))?;
        if let Some(pos) = records
            .iter()
            .position(|r| r.get("claim_id").and_then(Value::as_str) == Some(&id))
        {
            records[pos] = record;
        } else {
            records.push(record);
        }
        claims_list = records
            .iter()
            .map(|r| str_field(r, "claim").to_string())
            .collect::<Vec<_>>();
    }
    gate.insert("claims".into(), json!(claims_list));
    Ok(next)
}

// ── 默认状态 ──

pub fn default_state(project: &str, question: &str, mode: &str) -> Value {
    let state = json!({
        "schema_version": SCHEMA_VERSION, "project": project, "question": question, "mode": mode,
        "status": "active", "stage": STAGE_BOOTSTRAP,
        "current_direction": Value::Null, "active_hypothesis": Value::Null,
        "novelty_gate": { "status": "pending", "claims": [], "claim_records": [], "draft_claims": [] },
        "hypotheses": [], "hypothesis_backlog": [], "run_history": [], "external_research": [],
        "evidence_index": [], "blockers": [], "decisions": [], "next_actions": [],
        "created_at": framework_core::time::now_iso(), "updated_at": framework_core::time::now_iso(),
    });
    state
}

// ── 推荐下一步 ──

pub fn recommend_next_actions(state: &Value) -> Vec<String> {
    let updated_days = days_since(str_field(state, "updated_at"));
    let stale = updated_days.is_some_and(|d| d > STALE_STATE_DAYS);
    if stale || (!arr(state, "run_history").is_empty() && current_context_runs(state).is_empty()) {
        return vec![
            "先刷新当前上下文：确认 active hypothesis 和当前目标。".into(),
            "重查当前代码和最新实验输出，再决定方向。".into(),
        ];
    }
    if state.get("status").and_then(Value::as_str) == Some("concluded") {
        return vec![
            "Freeze final narrative in findings.md.".into(),
            "Archive winning evidence path.".into(),
        ];
    }
    let gate_status = novelty_str(state, "status", "pending");
    if gate_status != "passed" {
        let mut actions = vec!["先完成 novelty gate，再启动高成本实验。".into()];
        if novelty_arr(state, "claims").is_empty() {
            actions.push("提炼 3-5 条 novelty claims。".into());
        }
        return actions;
    }
    let hypotheses = arr(state, "hypotheses");
    if hypotheses.is_empty() {
        return vec!["补 3 条可比较的 hypothesis。".into()];
    }
    let active = state
        .get("active_hypothesis")
        .and_then(Value::as_str)
        .and_then(|id| find_hypothesis(state, id));
    let Some(active) = active else {
        return vec!["指定一个 active hypothesis。".into()];
    };
    let active_id = str_field(active, "id");
    if latest_run_for_hypothesis(state, active_id).is_none() {
        return vec![format!(
            "先为 {active_id} 写 protocol，再做第一轮 bounded run。"
        )];
    }
    vec![format!(
        "对 {active_id} 做 reflection，选 DEEPEN/BROADEN/PIVOT/CONCLUDE。"
    )]
}

// ── State freshness ──

/// Freshness analysis result for a research state.
pub struct StateFreshness {
    pub stale: bool,
    pub history_bias_risk: bool,
    pub recent_runs: Vec<Value>,
    pub recent_decisions: Vec<Value>,
}

/// Analyze the freshness of a research state.
///
/// Returns staleness indicator, history bias risk, and recent activity windows.
pub fn state_freshness(state: &Value) -> StateFreshness {
    let updated_at = str_field(state, "updated_at");
    let stale = days_since(updated_at)
        .map(|d| d > STALE_STATE_DAYS)
        .unwrap_or(true);

    let active_id = state.get("active_hypothesis").and_then(Value::as_str);

    // Sort once and reuse — avoids cloning the entire array 3+ times in separate
    // calls to sort_entries_by_recency within this function.
    fn sort_by_ts<'a>(entries: &[&'a Value], field: &str) -> Vec<&'a Value> {
        let mut sorted: Vec<&Value> = entries.iter().copied().collect();
        sorted.sort_by(|a, b| {
            let ta = parse_iso_timestamp(str_field(a, field))
                .unwrap_or(DateTime::<Utc>::MIN_UTC);
            let tb = parse_iso_timestamp(str_field(b, field))
                .unwrap_or(DateTime::<Utc>::MIN_UTC);
            tb.cmp(&ta)
        });
        sorted
    }

    let all_runs: Vec<&Value> = arr(state, "run_history").iter().collect();
    let all_decisions: Vec<&Value> = arr(state, "decisions").iter().collect();
    let sorted_runs = sort_by_ts(&all_runs, "recorded_at");
    let sorted_decisions = sort_by_ts(&all_decisions, "recorded_at");

    // Recent runs: prefer active hypothesis, fall back to chronological
    let recent_runs: Vec<&Value> = if let Some(active_id) = active_id {
        let filtered: Vec<_> = sorted_runs
            .iter()
            .copied()
            .filter(|r| r.get("hypothesis_id").and_then(Value::as_str) == Some(active_id))
            .filter(|r| {
                days_since(str_field(r, "recorded_at"))
                    .is_some_and(|d| d <= RECENT_ACTIVITY_DAYS)
            })
            .take(FALLBACK_ACTIVITY_LIMIT)
            .collect();
        if !filtered.is_empty() {
            filtered
        } else {
            sorted_runs
                .iter()
                .copied()
                .take(FALLBACK_ACTIVITY_LIMIT)
                .collect()
        }
    } else {
        sorted_runs
            .iter()
            .copied()
            .take(FALLBACK_ACTIVITY_LIMIT)
            .collect()
    };

    // Recent decisions: same logic
    let recent_decisions: Vec<&Value> = if let Some(active_id) = active_id {
        let filtered: Vec<_> = sorted_decisions
            .iter()
            .copied()
            .filter(|d| d.get("hypothesis_id").and_then(Value::as_str) == Some(active_id))
            .filter(|d| {
                days_since(str_field(d, "recorded_at"))
                    .is_some_and(|d| d <= RECENT_ACTIVITY_DAYS)
            })
            .take(FALLBACK_ACTIVITY_LIMIT)
            .collect();
        if !filtered.is_empty() {
            filtered
        } else {
            sorted_decisions
                .iter()
                .copied()
                .take(FALLBACK_ACTIVITY_LIMIT)
                .collect()
        }
    } else {
        sorted_decisions
            .iter()
            .copied()
            .take(FALLBACK_ACTIVITY_LIMIT)
            .collect()
    };

    let history_bias_risk = stale
        || (recent_runs.is_empty() && !all_runs.is_empty())
        || (recent_decisions.is_empty() && !all_decisions.is_empty());

    StateFreshness {
        stale,
        history_bias_risk,
        recent_runs: recent_runs.into_iter().cloned().collect(),
        recent_decisions: recent_decisions.into_iter().cloned().collect(),
    }
}

/// Find runs that are missing reuse annotations (finding, decision_delta, reuse_note).
pub fn missing_reuse_annotation_runs(state: &Value) -> Vec<Value> {
    sort_entries_by_recency(arr(state, "run_history"), "recorded_at")
        .into_iter()
        .filter(|r| {
            str_field_default(r, "finding", "").is_empty()
                || str_field_default(r, "decision_delta", "").is_empty()
                || str_field_default(r, "reuse_note", "").is_empty()
        })
        .collect()
}

// ── 测试 ──

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn minimal_state() -> Value {
        default_state("test-project", "Does X improve Y?", "quick")
    }

    fn gate_passed_state() -> Value {
        let s = minimal_state();
        let s2 = add_claim_comparison(
            &s,
            "c1",
            "method",
            "pw",
            "low",
            "diff",
            "high",
            "novel",
            Some("C1"),
        )
        .unwrap();
        let mut s3 = add_claim_comparison(
            &s2,
            "c2",
            "task",
            "pw2",
            "medium",
            "diff2",
            "medium",
            "defensible",
            Some("C2"),
        )
        .unwrap();
        novelty_gate_mut(&mut s3).insert("status".into(), json!("passed"));
        s3
    }

    #[test]
    fn add_hypothesis_basic() {
        let state = gate_passed_state();
        let result = add_hypothesis(
            &state,
            HypothesisInput {
                claim: "test claim",
                prediction: Some("p"),
                mechanism: None,
                falsifiable_prediction: None,
                success_threshold: None,
                stop_condition: None,
                baselines: &[],
                confounders: &[],
                negative_signals: &[],
                minimal_test: None,
                priority: "high",
                hypothesis_id: Some("h1"),
            },
        )
        .unwrap();
        assert_eq!(arr(&result, "hypotheses").len(), 1);
        assert_eq!(
            find_hypothesis(&result, "h1")
                .unwrap()
                .get("claim")
                .and_then(Value::as_str),
            Some("test claim")
        );
    }

    #[test]
    fn add_hypothesis_duplicate_rejected() {
        let state = gate_passed_state();
        let s = add_hypothesis(
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
            &s,
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

    #[test]
    fn transition_hypothesis_valid() {
        let mut s = gate_passed_state();
        arr_mut(&mut s, "hypotheses").unwrap().push(json!({"id": "h1", "claim": "c", "status": "queued", "status_updated_at": framework_core::time::now_iso(), "created_at": framework_core::time::now_iso()}));
        let idx = find_hypothesis_index(&s, "h1").unwrap();
        transition_hypothesis(&mut s, idx, "active", Some("activated")).unwrap();
        assert_eq!(
            find_hypothesis(&s, "h1")
                .unwrap()
                .get("status")
                .and_then(Value::as_str),
            Some("active")
        );
    }

    #[test]
    fn transition_hypothesis_invalid() {
        let mut s = gate_passed_state();
        arr_mut(&mut s, "hypotheses").unwrap().push(json!({"id": "h1", "claim": "c", "status": "concluded", "status_updated_at": framework_core::time::now_iso(), "created_at": framework_core::time::now_iso()}));
        let idx = find_hypothesis_index(&s, "h1").unwrap();
        assert!(transition_hypothesis(&mut s, idx, "active", None).is_err());
    }

    #[test]
    fn record_run_basic() {
        let s = gate_passed_state();
        let s2 = add_hypothesis(
            &s,
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
            &s2,
            &RecordRunInput {
                hypothesis_id: "h1",
                outcome: "confirmatory",
                summary: "test",
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
        assert_eq!(arr(&result, "run_history").len(), 1);
    }

    #[test]
    fn record_run_rejects_unknown() {
        let s = gate_passed_state();
        let tmp = tempfile::tempdir().unwrap();
        let result = record_run(
            &s,
            &RecordRunInput {
                hypothesis_id: "nonexistent",
                outcome: "confirmatory",
                summary: "s",
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
        let s = gate_passed_state();
        let s2 = add_hypothesis(
            &s,
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
        let s3 = record_run(
            &s2,
            &RecordRunInput {
                hypothesis_id: "h1",
                outcome: "confirmatory",
                summary: "s",
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
        let result = reflect(&s3, "h1", "DEEPEN", "interesting", None, None).unwrap();
        assert_eq!(
            result.get("current_direction").and_then(Value::as_str),
            Some("DEEPEN")
        );
        assert_eq!(
            find_hypothesis(&result, "h1")
                .unwrap()
                .get("status")
                .and_then(Value::as_str),
            Some("active")
        );
    }

    #[test]
    fn reflect_conclude() {
        let s = gate_passed_state();
        let s2 = add_hypothesis(
            &s,
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
        let s3 = record_run(
            &s2,
            &RecordRunInput {
                hypothesis_id: "h1",
                outcome: "confirmatory",
                summary: "s",
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
        let result = reflect(&s3, "h1", "CONCLUDE", "done", None, None).unwrap();
        assert_eq!(
            result.get("status").and_then(Value::as_str),
            Some("concluded")
        );
    }

    #[test]
    fn recommend_actions_initial() {
        let s = minimal_state();
        let actions = recommend_next_actions(&s);
        assert!(!actions.is_empty());
    }

    #[test]
    fn add_claim_comparison_creates_record() {
        let s = minimal_state();
        let updated = add_claim_comparison(
            &s,
            "my claim",
            "method",
            "pw",
            "low",
            "diff",
            "high",
            "novel",
            Some("C1"),
        )
        .unwrap();
        let records = novelty_arr(&updated, "claim_records");
        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].get("claim_id").and_then(Value::as_str),
            Some("C1")
        );
    }

    #[test]
    fn state_freshness_empty_state() {
        let s = minimal_state();
        let freshness = state_freshness(&s);
        // A freshly created state should not be stale
        assert!(!freshness.stale);
        assert!(freshness.recent_runs.is_empty());
        assert!(freshness.recent_decisions.is_empty());
    }

    #[test]
    fn state_freshness_with_runs() {
        let s = gate_passed_state();
        let s2 = add_hypothesis(
            &s,
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
        let s3 = record_run(
            &s2,
            &RecordRunInput {
                hypothesis_id: "h1",
                outcome: "confirmatory",
                summary: "s",
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
        let freshness = state_freshness(&s3);
        assert_eq!(freshness.recent_runs.len(), 1);
    }

    #[test]
    fn missing_reuse_annotation_runs_finds_incomplete() {
        let s = gate_passed_state();
        let s2 = add_hypothesis(
            &s,
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
        let s3 = record_run(
            &s2,
            &RecordRunInput {
                hypothesis_id: "h1",
                outcome: "confirmatory",
                summary: "s",
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
        let missing = missing_reuse_annotation_runs(&s3);
        assert_eq!(missing.len(), 1); // run has empty finding/decision_delta/reuse_note
    }
}
