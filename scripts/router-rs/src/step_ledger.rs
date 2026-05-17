use crate::framework_runtime::resolve_repo_root_arg;
use crate::path_guard::{safe_task_id_component, validate_task_id_component};
use crate::runtime_storage::acquire_runtime_path_lock;
use chrono::{SecondsFormat, Utc};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

pub const STEP_LEDGER_FILENAME: &str = "STEP_LEDGER.jsonl";
pub const STEP_LEDGER_ENTRY_SCHEMA_VERSION: &str = "router-rs-step-ledger-entry-v1";
pub const STEP_LEDGER_RESPONSE_SCHEMA_VERSION: &str = "router-rs-step-ledger-response-v1";
pub const STEP_LEDGER_SUMMARY_SCHEMA_VERSION: &str = "router-rs-step-ledger-summary-v1";
pub const STEP_LEDGER_AUTHORITY: &str = "rust-step-ledger";

pub fn step_ledger_path_for_task(repo_root: &Path, task_id: &str) -> PathBuf {
    let task_component = safe_task_id_component(task_id).unwrap_or("__invalid_task_id__");
    repo_root
        .join("artifacts/current")
        .join(task_component)
        .join(STEP_LEDGER_FILENAME)
}

pub fn handle_step_ledger_operation(payload: Value) -> Result<Value, String> {
    let operation = required_non_empty_string(&payload, "operation", "step ledger")?;
    match operation.as_str() {
        "append" => append_step_ledger_entry(payload),
        "summary" => summarize_step_ledger_operation(payload),
        "contract" => Ok(step_ledger_contract()),
        other => Err(format!("unknown step ledger operation: {other}")),
    }
}

pub fn step_ledger_contract() -> Value {
    json!({
        "schema_version": STEP_LEDGER_RESPONSE_SCHEMA_VERSION,
        "authority": STEP_LEDGER_AUTHORITY,
        "entry_schema_version": STEP_LEDGER_ENTRY_SCHEMA_VERSION,
        "ledger_filename": STEP_LEDGER_FILENAME,
        "canonical_role": "L2 task-scoped append-only step recovery ledger; TASK_STATE.json may project summaries only",
        "append_required_fields": ["operation", "step_id"],
        "entry_fields": [
            "schema_version",
            "authority",
            "recorded_at",
            "task_id",
            "step_id",
            "phase",
            "status",
            "input_digest",
            "retry_count",
            "side_effects",
            "evidence_ref",
            "next_resume_hint",
            "idempotency_key"
        ],
        "append_semantics": "Entries with the same idempotency_key are treated as already recorded; when no idempotency_key is supplied, router-rs derives one from task_id + step_id + input_digest when input_digest is available.",
        "model_context_policy": "Consumers should inject summaries or refs, never the full STEP_LEDGER.jsonl."
    })
}

fn append_step_ledger_entry(payload: Value) -> Result<Value, String> {
    let repo_root = resolve_repo_root_from_payload(&payload)?;
    let task_id = resolve_task_id_from_payload(&repo_root, &payload)?;
    let step_id = required_non_empty_string(&payload, "step_id", "step ledger append")?;
    let phase = optional_non_empty_string(&payload, "phase")
        .unwrap_or_else(|| "implementation".to_string());
    let status =
        optional_non_empty_string(&payload, "status").unwrap_or_else(|| "in_progress".to_string());
    let retry_count = payload
        .get("retry_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let input_digest = optional_non_empty_string(&payload, "input_digest").or_else(|| {
        optional_non_empty_string(&payload, "input_text").map(|text| sha256_text(&text))
    });
    let idempotency_key = optional_non_empty_string(&payload, "idempotency_key").or_else(|| {
        input_digest
            .as_ref()
            .map(|digest| sha256_text(&format!("{task_id}\x1e{step_id}\x1e{digest}")))
    });
    let side_effects = payload
        .get("side_effects")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let evidence_ref = payload.get("evidence_ref").cloned().unwrap_or(Value::Null);
    let next_resume_hint = payload
        .get("next_resume_hint")
        .cloned()
        .unwrap_or(Value::Null);
    let mut extra = Map::new();
    if let Some(obj) = payload.get("metadata").and_then(Value::as_object) {
        extra = obj.clone();
    }

    let mut entry = Map::new();
    entry.insert(
        "schema_version".to_string(),
        Value::String(STEP_LEDGER_ENTRY_SCHEMA_VERSION.to_string()),
    );
    entry.insert(
        "authority".to_string(),
        Value::String(STEP_LEDGER_AUTHORITY.to_string()),
    );
    entry.insert(
        "recorded_at".to_string(),
        Value::String(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)),
    );
    entry.insert("task_id".to_string(), Value::String(task_id.clone()));
    entry.insert("step_id".to_string(), Value::String(step_id));
    entry.insert("phase".to_string(), Value::String(phase));
    entry.insert("status".to_string(), Value::String(status));
    entry.insert(
        "input_digest".to_string(),
        input_digest.map(Value::String).unwrap_or(Value::Null),
    );
    entry.insert("retry_count".to_string(), json!(retry_count));
    entry.insert("side_effects".to_string(), Value::Array(side_effects));
    entry.insert("evidence_ref".to_string(), evidence_ref);
    entry.insert("next_resume_hint".to_string(), next_resume_hint);
    entry.insert(
        "idempotency_key".to_string(),
        idempotency_key
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    if !extra.is_empty() {
        entry.insert("metadata".to_string(), Value::Object(extra));
    }

    let path = step_ledger_path_for_task(&repo_root, &task_id);
    let entry_value = Value::Object(entry.clone());
    let changed = crate::task_write_lock::apply_task_ledger_mutation(&repo_root, || {
        let inner_changed = append_jsonl_entry(&path, &entry_value, idempotency_key.as_deref())?;
        if inner_changed {
            crate::task_state_aggregate::sync_task_state_aggregate_best_effort(
                &repo_root, &task_id,
            );
        }
        Ok(inner_changed)
    })?;
    Ok(json!({
        "schema_version": STEP_LEDGER_RESPONSE_SCHEMA_VERSION,
        "authority": STEP_LEDGER_AUTHORITY,
        "operation": "append",
        "changed": changed,
        "task_id": task_id,
        "path": path.display().to_string(),
        "entry": Value::Object(entry),
    }))
}

fn summarize_step_ledger_operation(payload: Value) -> Result<Value, String> {
    let repo_root = resolve_repo_root_from_payload(&payload)?;
    let task_id = resolve_task_id_from_payload(&repo_root, &payload)?;
    Ok(summarize_step_ledger_for_task(&repo_root, &task_id))
}

pub fn summarize_step_ledger_for_task(repo_root: &Path, task_id: &str) -> Value {
    let path = step_ledger_path_for_task(repo_root, task_id);
    let mut status_counts = BTreeMap::<String, u64>::new();
    let mut entry_count = 0_u64;
    let mut invalid_line_count = 0_u64;
    let mut latest_step_id = Value::Null;
    let mut latest_status = Value::Null;
    let mut latest_resume_hint = Value::Null;
    let mut latest_evidence_ref = Value::Null;

    if let Ok(file) = fs::File::open(&path) {
        for line in BufReader::new(file).lines().map_while(Result::ok) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            match serde_json::from_str::<Value>(trimmed) {
                Ok(value) => {
                    entry_count += 1;
                    let status = value
                        .get("status")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                        .to_string();
                    *status_counts.entry(status.clone()).or_insert(0) += 1;
                    latest_step_id = value.get("step_id").cloned().unwrap_or(Value::Null);
                    latest_status = Value::String(status);
                    latest_resume_hint = value
                        .get("next_resume_hint")
                        .cloned()
                        .unwrap_or(Value::Null);
                    latest_evidence_ref = value.get("evidence_ref").cloned().unwrap_or(Value::Null);
                }
                Err(_) => invalid_line_count += 1,
            }
        }
    }

    json!({
        "schema_version": STEP_LEDGER_SUMMARY_SCHEMA_VERSION,
        "authority": STEP_LEDGER_AUTHORITY,
        "task_id": task_id.trim(),
        "path": path.display().to_string(),
        "exists": path.is_file(),
        "entry_count": entry_count,
        "invalid_line_count": invalid_line_count,
        "status_counts": status_counts,
        "latest": {
            "step_id": latest_step_id,
            "status": latest_status,
            "next_resume_hint": latest_resume_hint,
            "evidence_ref": latest_evidence_ref,
        },
        "model_context_policy": "Inject this summary or specific evidence refs; do not paste the full STEP_LEDGER.jsonl."
    })
}

fn append_jsonl_entry(
    path: &Path,
    entry: &Value,
    idempotency_key: Option<&str>,
) -> Result<bool, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create step ledger parent failed: {err}"))?;
    }
    let _path_lock = acquire_runtime_path_lock(path)?;
    if let Some(key) = idempotency_key {
        if step_ledger_contains_idempotency_key(path, key)? {
            return Ok(false);
        }
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| format!("open step ledger {} failed: {err}", path.display()))?;
    let line = serde_json::to_string(entry)
        .map_err(|err| format!("serialize step ledger entry failed: {err}"))?
        + "\n";
    file.write_all(line.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|err| format!("append step ledger {} failed: {err}", path.display()))?;
    Ok(true)
}

fn step_ledger_contains_idempotency_key(
    path: &Path,
    idempotency_key: &str,
) -> Result<bool, String> {
    if idempotency_key.trim().is_empty() || !path.is_file() {
        return Ok(false);
    }
    let file = fs::File::open(path)
        .map_err(|err| format!("open step ledger {} failed: {err}", path.display()))?;
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if serde_json::from_str::<Value>(trimmed)
            .ok()
            .and_then(|value| {
                value
                    .get("idempotency_key")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .as_deref()
            == Some(idempotency_key)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn resolve_repo_root_from_payload(payload: &Value) -> Result<PathBuf, String> {
    let explicit = payload.get("repo_root").and_then(|v| {
        let s = value_text(Some(v));
        if s.is_empty() {
            None
        } else {
            Some(PathBuf::from(s))
        }
    });
    resolve_repo_root_arg(explicit.as_deref())
}

fn resolve_task_id_from_payload(repo_root: &Path, payload: &Value) -> Result<String, String> {
    let task_id = optional_non_empty_string(payload, "task_id")
        .or_else(|| crate::autopilot_goal::read_active_task_id(repo_root))
        .or_else(|| crate::autopilot_goal::read_focus_task_id(repo_root))
        .ok_or_else(|| {
            "step ledger requires task_id or active_task.json/focus_task.json".to_string()
        })?;
    validate_task_id_component(&task_id).map(str::to_string)
}

fn required_non_empty_string(payload: &Value, key: &str, context: &str) -> Result<String, String> {
    optional_non_empty_string(payload, key)
        .ok_or_else(|| format!("{context} requires non-empty {key}"))
}

fn optional_non_empty_string(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn value_text(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn sha256_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_repo(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "router-rs-step-ledger-{label}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    #[test]
    fn append_and_summary_round_trip() {
        let repo = unique_repo("roundtrip");
        let task_id = "task-a";
        let payload = json!({
            "operation": "append",
            "repo_root": repo.display().to_string(),
            "task_id": task_id,
            "step_id": "s1",
            "phase": "verify",
            "status": "pass",
            "input_text": "cargo test",
            "retry_count": 1,
            "side_effects": [],
            "evidence_ref": {"kind":"EVIDENCE_INDEX","row":0},
            "next_resume_hint": "continue at s2"
        });
        let response = handle_step_ledger_operation(payload).expect("append");
        assert_eq!(response["operation"], "append");
        let summary = summarize_step_ledger_for_task(&repo, task_id);
        assert_eq!(summary["entry_count"], json!(1));
        assert_eq!(summary["status_counts"]["pass"], json!(1));
        assert_eq!(summary["latest"]["step_id"], json!("s1"));
        assert!(repo
            .join("artifacts/current/task-a/TASK_STATE.json")
            .is_file());
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn append_rejects_task_id_path_traversal() {
        let repo = unique_repo("path-traversal");
        let payload = json!({
            "operation": "append",
            "repo_root": repo.display().to_string(),
            "task_id": "../outside",
            "step_id": "s1"
        });
        let err = handle_step_ledger_operation(payload).unwrap_err();
        assert!(err.contains("safe path component"), "{err}");
        assert!(!repo.join("artifacts/outside/STEP_LEDGER.jsonl").exists());
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn append_is_idempotent_for_same_step_and_input_digest() {
        let repo = unique_repo("idempotent");
        let payload = json!({
            "operation": "append",
            "repo_root": repo.display().to_string(),
            "task_id": "task-a",
            "step_id": "s1",
            "input_digest": "sha256:abc",
            "status": "pass"
        });
        let first = handle_step_ledger_operation(payload.clone()).expect("first append");
        let second = handle_step_ledger_operation(payload).expect("second append");
        assert_eq!(first["changed"], json!(true));
        assert_eq!(second["changed"], json!(false));
        let summary = summarize_step_ledger_for_task(&repo, "task-a");
        assert_eq!(summary["entry_count"], json!(1));
        let _ = fs::remove_dir_all(repo);
    }
}
