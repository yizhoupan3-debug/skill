//! Research state persistence: load, save, migrate, and hydrate state from disk.
//!
//! Migrated from `tools/autoresearch-rs/src/state.rs`.
//!
//! This module handles schema versioning, YAML/JSON file I/O, and the full
//! state hydration pipeline (filling in defaults for all nested structures).

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

use crate::util::{obj_mut, arr_mut, set_key, now_iso, novelty_gate_mut, str_field};

// ── Local helpers ──

// ── Constants ──

const SCHEMA_VERSION: i64 = 4;

const VALID_HYPOTHESIS_STATUSES: &[&str] = &[
    "queued",
    "active",
    "needs_reflection",
    "parked",
    "concluded",
];

// ── Hypothesis defaults ──

fn ensure_hypothesis_defaults(item: &mut serde_json::Map<String, Value>, updated_at: &str) {
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
    if !VALID_HYPOTHESIS_STATUSES.contains(&status.as_str()) {
        item.insert("status".into(), json!("queued"));
    } else {
        item.entry("status").or_insert(json!(status));
    }
    item.entry("status_reason").or_insert(Value::Null);
    let status_updated_at = item
        .get("created_at")
        .cloned()
        .unwrap_or_else(|| json!(updated_at));
    item.entry("status_updated_at").or_insert(status_updated_at);
}

// ── Run record defaults ──

fn ensure_run_record_defaults(item: &mut serde_json::Map<String, Value>) {
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

// ── Schema migration ──

/// Migrate state from older schema versions to the current version.
pub fn migrate_state(state: &Value) -> Result<Value> {
    let mut migrated = state.clone();
    let version = migrated
        .get("schema_version")
        .and_then(Value::as_i64)
        .unwrap_or(2);
    if version >= SCHEMA_VERSION {
        return Ok(migrated);
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
        let hypothesis_id = str_field(hypothesis, "id").to_string();
        let mut status = hypothesis
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("queued")
            .to_string();
        if !VALID_HYPOTHESIS_STATUSES.contains(&status.as_str()) {
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
        ensure_hypothesis_defaults(item, &updated_at);
        if status == "active"
            && latest_run.is_some()
            && (latest_decision.is_none()
                || latest_decision.and_then(|item| item.get("run_id"))
                    != latest_run.and_then(|item| item.get("run_id")))
        {
            status = "needs_reflection".to_string();
        }
        item.insert("status".into(), json!(status));
    }
    for record in arr_mut(&mut migrated, "run_history") {
        let Some(item) = record.as_object_mut() else {
            continue;
        };
        ensure_run_record_defaults(item);
    }
    obj_mut(&mut migrated)
        .entry("external_research")
        .or_insert(json!([]));
    set_key(&mut migrated, "schema_version", json!(SCHEMA_VERSION));
    Ok(migrated)
}

// ── Hydrate state ──

/// Fully hydrate a state value with all defaults for arrays, novelty_gate,
/// hypothesis fields, run record fields, and external_research records.
///
/// This is the comprehensive version of `ensure_state_defaults` that includes
/// field-level defaults for nested structures (hypotheses, runs, external research).
pub fn hydrate_state(state: &Value) -> Result<Value> {
    let mut hydrated = state.clone();
    {
        let root = obj_mut(&mut hydrated);
        root.entry("schema_version")
            .or_insert(json!(SCHEMA_VERSION));
        root.entry("status").or_insert(json!("active"));
        root.entry("stage").or_insert(json!("bootstrap"));
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
    let updated_at = str_field(&hydrated, "updated_at").to_string();
    for hypothesis in arr_mut(&mut hydrated, "hypotheses") {
        let item = hypothesis
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("hypothesis must be object"))?;
        ensure_hypothesis_defaults(item, &updated_at);
    }
    for record in arr_mut(&mut hydrated, "run_history") {
        let item = record
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("run record must be object"))?;
        ensure_run_record_defaults(item);
    }
    for record in arr_mut(&mut hydrated, "external_research") {
        let item = record
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("external research record must be object"))?;
        item.entry("claim_id").or_insert(Value::Null);
        item.entry("source").or_insert(json!("all"));
        item.entry("results").or_insert(json!([]));
        item.entry("errors").or_insert(json!([]));
        item.entry("created_at")
            .or_insert_with(|| json!(updated_at.clone()));
    }
    set_key(&mut hydrated, "schema_version", json!(SCHEMA_VERSION));
    Ok(hydrated)
}

// ── Load / Save ──

/// Load research state from a YAML or JSON file, applying migration and defaults.
pub fn load_state(path: &Path) -> Result<Value> {
    let raw = fs::read_to_string(path)?;
    let data: Value = serde_yml::from_str(&raw)
        .or_else(|_| serde_json::from_str(&raw))
        .with_context(|| format!("State file must be YAML/JSON: {}", path.display()))?;
    if !data.is_object() {
        anyhow::bail!("State file must be a mapping: {}", path.display());
    }
    hydrate_state(&migrate_state(&data)?)
}

/// Save state to disk with refreshed novelty views and updated timestamps.
///
/// Uses write-to-tempfile-then-rename for crash safety: if the process is
/// killed mid-write, the original file remains intact (POSIX rename is atomic).
pub fn dump_state(path: &Path, state: &Value) -> Result<()> {
    let mut state_to_write = hydrate_state(state)?;
    set_key(&mut state_to_write, "schema_version", json!(SCHEMA_VERSION));
    set_key(&mut state_to_write, "updated_at", json!(now_iso()));
    let actions = crate::claims::lifecycle::recommend_next_actions(&state_to_write);
    set_key(&mut state_to_write, "next_actions", json!(actions));
    let rendered = serde_yml::to_string(&state_to_write)?;

    // Atomic write via core-standard write_atomic_text (POSIX rename for crash safety).
    core_state::utils::atomic_write::write_atomic_text(path, &rendered)
        .map_err(|e| anyhow::anyhow!("atomic write failed for {}: {e}", path.display()))?;
    Ok(())
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hydrate_state_fills_all_arrays() {
        let state = json!({});
        let hydrated = hydrate_state(&state).unwrap();
        assert!(hydrated.get("hypotheses").unwrap().is_array());
        assert!(hydrated.get("run_history").unwrap().is_array());
        assert!(hydrated.get("decisions").unwrap().is_array());
        assert!(hydrated.get("external_research").unwrap().is_array());
        assert!(hydrated.get("blockers").unwrap().is_array());
    }

    #[test]
    fn hydrate_state_fills_novelty_gate() {
        let state = json!({});
        let hydrated = hydrate_state(&state).unwrap();
        let gate = hydrated.get("novelty_gate").unwrap();
        assert_eq!(gate.get("status").unwrap(), "pending");
        assert!(gate.get("claims").unwrap().is_array());
        assert!(gate.get("claim_records").unwrap().is_array());
        assert!(gate.get("draft_claims").unwrap().is_array());
    }

    #[test]
    fn hydrate_state_fills_hypothesis_defaults() {
        let state = json!({
            "hypotheses": [{"id": "h1", "claim": "test"}]
        });
        let hydrated = hydrate_state(&state).unwrap();
        let h = &hydrated["hypotheses"][0];
        assert_eq!(h["status"], "queued");
        assert!(h["mechanism"].is_null());
        assert!(h["baselines"].is_array());
        assert!(h["confounders"].is_array());
        assert!(h["negative_signals"].is_array());
    }

    #[test]
    fn hydrate_state_fills_run_record_defaults() {
        let state = json!({
            "run_history": [{"run_id": "run-001", "hypothesis_id": "h1", "outcome": "confirmatory"}]
        });
        let hydrated = hydrate_state(&state).unwrap();
        let r = &hydrated["run_history"][0];
        assert!(r["finding"].is_null());
        assert!(r["decision_delta"].is_null());
        assert!(r["reuse_note"].is_null());
        assert!(r["sanity_checks"].is_array());
        assert!(r["rules_in"].is_array());
        assert!(r["rules_out"].is_array());
    }

    #[test]
    fn hydrate_state_fills_external_research_defaults() {
        let state = json!({
            "external_research": [{"query": "test"}]
        });
        let hydrated = hydrate_state(&state).unwrap();
        let r = &hydrated["external_research"][0];
        assert_eq!(r["source"], "all");
        assert!(r["results"].is_array());
        assert!(r["errors"].is_array());
        assert!(r["claim_id"].is_null());
    }

    #[test]
    fn hydrate_state_preserves_existing_values() {
        let state = json!({
            "project": "my-project",
            "status": "concluded",
            "hypotheses": [{"id": "h1", "claim": "c", "status": "active", "mechanism": "m"}]
        });
        let hydrated = hydrate_state(&state).unwrap();
        assert_eq!(hydrated["project"], "my-project");
        assert_eq!(hydrated["status"], "concluded");
        assert_eq!(hydrated["hypotheses"][0]["mechanism"], "m");
    }

    #[test]
    fn migrate_state_upgrades_version() {
        let state = json!({"schema_version": 2, "hypotheses": [], "run_history": []});
        let migrated = migrate_state(&state).unwrap();
        assert_eq!(migrated["schema_version"], SCHEMA_VERSION);
    }

    #[test]
    fn migrate_state_noop_for_current() {
        let state = json!({"schema_version": SCHEMA_VERSION});
        let migrated = migrate_state(&state).unwrap();
        assert_eq!(migrated["schema_version"], SCHEMA_VERSION);
    }

    #[test]
    fn load_state_from_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.yaml");
        fs::write(&path, "project: test\nquestion: q\nmode: quick\n").unwrap();
        let state = load_state(&path).unwrap();
        assert_eq!(state["project"], "test");
        assert_eq!(state["question"], "q");
    }

    #[test]
    fn load_state_from_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        fs::write(&path, r#"{"project": "test", "question": "q"}"#).unwrap();
        let state = load_state(&path).unwrap();
        assert_eq!(state["project"], "test");
    }

    #[test]
    fn load_state_rejects_non_object() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.yaml");
        fs::write(&path, "- item1\n- item2\n").unwrap();
        assert!(load_state(&path).is_err());
    }

    #[test]
    fn dump_state_writes_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.yaml");
        let state = json!({"project": "test", "question": "q", "mode": "quick"});
        dump_state(&path, &state).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("project"));
        assert!(raw.contains("test"));
    }

    #[test]
    fn dump_state_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.yaml");
        let state = json!({"project": "test", "question": "q", "mode": "quick"});
        dump_state(&path, &state).unwrap();
        let loaded = load_state(&path).unwrap();
        assert_eq!(loaded["project"], "test");
        assert_eq!(loaded["question"], "q");
    }
}
