//! State management: schema defaults, migration, and loading.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

use crate::*;

pub(super) fn ensure_state_defaults(state: &Value) -> Value {
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
        item.entry("mechanism").or_insert(Value::Null);
        item.entry("falsifiable_prediction").or_insert(Value::Null);
        item.entry("success_threshold").or_insert(Value::Null);
        item.entry("stop_condition").or_insert(Value::Null);
        item.entry("baselines").or_insert(json!([]));
        item.entry("confounders").or_insert(json!([]));
        item.entry("negative_signals").or_insert(json!([]));
        item.entry("minimal_test").or_insert(Value::Null);
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
        item.entry("sanity_checks").or_insert(json!([]));
        item.entry("baseline_result").or_insert(Value::Null);
        item.entry("rules_in").or_insert(json!([]));
        item.entry("rules_out").or_insert(json!([]));
        item.entry("alternative_explanations").or_insert(json!([]));
        item.entry("threats").or_insert(json!([]));
        item.entry("interpretation").or_insert(Value::Null);
        item.entry("finding").or_insert(Value::Null);
        item.entry("decision_delta").or_insert(Value::Null);
        item.entry("reuse_note").or_insert(Value::Null);
        item.entry("applies_to").or_insert(json!([]));
        item.entry("does_not_apply_to").or_insert(json!([]));
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

pub(super) fn load_state(path: &Path) -> Result<Value> {
    let raw = fs::read_to_string(path)?;
    let data: Value = serde_yml::from_str(&raw)
        .or_else(|_| serde_json::from_str(&raw))
        .with_context(|| format!("State file must be YAML/JSON: {}", path.display()))?;
    if !data.is_object() {
        bail!("State file must be a mapping: {}", path.display());
    }
    Ok(ensure_state_defaults(&migrate_state(&data)))
}

pub(super) fn migrate_state(state: &Value) -> Value {
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
        let Some(item) = hypothesis.as_object_mut() else {
            continue;
        };
        item.entry("mechanism").or_insert(Value::Null);
        item.entry("falsifiable_prediction").or_insert(Value::Null);
        item.entry("success_threshold").or_insert(Value::Null);
        item.entry("stop_condition").or_insert(Value::Null);
        item.entry("baselines").or_insert(json!([]));
        item.entry("confounders").or_insert(json!([]));
        item.entry("negative_signals").or_insert(json!([]));
        item.entry("minimal_test").or_insert(Value::Null);
        if status == "active"
            && latest_run.is_some()
            && (latest_decision.is_none()
                || latest_decision.and_then(|item| item.get("run_id"))
                    != latest_run.and_then(|item| item.get("run_id")))
        {
            status = "needs_reflection".to_string();
        }
        item.insert("status".into(), json!(status));
        item.entry("status_reason").or_insert(Value::Null);
        let status_updated_at = item
            .get("created_at")
            .cloned()
            .unwrap_or_else(|| json!(updated_at.clone()));
        item.entry("status_updated_at").or_insert(status_updated_at);
    }
    for record in arr_mut(&mut migrated, "run_history") {
        let Some(item) = record.as_object_mut() else {
            continue;
        };
        item.entry("novelty_gate_status_at_run")
            .or_insert(Value::Null);
        item.entry("novelty_gate_override").or_insert(json!(false));
        item.entry("override_reason").or_insert(Value::Null);
        item.entry("environment_fingerprint").or_insert(Value::Null);
        item.entry("git_provenance").or_insert(Value::Null);
        item.entry("sanity_checks").or_insert(json!([]));
        item.entry("baseline_result").or_insert(Value::Null);
        item.entry("rules_in").or_insert(json!([]));
        item.entry("rules_out").or_insert(json!([]));
        item.entry("alternative_explanations").or_insert(json!([]));
        item.entry("threats").or_insert(json!([]));
        item.entry("interpretation").or_insert(Value::Null);
        item.entry("finding").or_insert(Value::Null);
        item.entry("decision_delta").or_insert(Value::Null);
        item.entry("reuse_note").or_insert(Value::Null);
        item.entry("applies_to").or_insert(json!([]));
        item.entry("does_not_apply_to").or_insert(json!([]));
    }
    obj_mut(&mut migrated)
        .entry("external_research")
        .or_insert(json!([]));
    set_key(&mut migrated, "schema_version", json!(SCHEMA_VERSION));
    migrated
}
