use chrono::{DateTime, Duration, Utc};
use serde_json::{Value, json};
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::types::{
    SESSION_SUPERVISOR_STORE_SCHEMA_VERSION, SessionSupervisorStore, WorkerEvent,
    WorkerSessionRecord,
};

pub fn worker_log_path(state_path: &Path, worker_id: &str) -> PathBuf {
    let state_dir = state_path.parent().unwrap_or_else(|| Path::new("."));
    state_dir
        .join("logs")
        .join(format!("{}.log", sanitize_segment(worker_id)))
}

pub fn load_store(path: &Path) -> Result<SessionSupervisorStore, String> {
    if !path.is_file() {
        return Ok(SessionSupervisorStore {
            schema_version: SESSION_SUPERVISOR_STORE_SCHEMA_VERSION.to_string(),
            version: 1,
            workers: Vec::new(),
        });
    }
    let payload: SessionSupervisorStore = serde_json::from_str(
        &fs::read_to_string(path).map_err(|err| format!("read supervisor store failed: {err}"))?,
    )
    .map_err(|err| format!("parse supervisor store failed: {err}"))?;
    Ok(payload)
}

pub fn save_store(path: &Path, store: &SessionSupervisorStore) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create supervisor state dir failed: {err}"))?;
    }
    let payload = serde_json::to_string_pretty(store)
        .map_err(|err| format!("serialize supervisor store failed: {err}"))?
        + "\n";
    let parent = path.parent().ok_or_else(|| {
        format!(
            "supervisor state path {} has no parent directory",
            path.display()
        )
    })?;
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("supervisor_state");
    let tmp_path = parent.join(format!(".router-rs.{file_name}.{}.tmp", std::process::id()));
    {
        let mut tmp_file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp_path)
            .map_err(|err| {
                format!(
                    "create supervisor state temp file {} failed: {err}",
                    tmp_path.display()
                )
            })?;
        tmp_file
            .write_all(payload.as_bytes())
            .and_then(|_| tmp_file.sync_all())
            .map_err(|err| {
                let _ = fs::remove_file(&tmp_path);
                format!(
                    "write supervisor state temp payload failed for {}: {err}",
                    tmp_path.display()
                )
            })?;
    }
    fs::rename(&tmp_path, path).map_err(|err| {
        let _ = fs::remove_file(&tmp_path);
        format!(
            "replace supervisor state failed for {}: {err}",
            path.display()
        )
    })?;
    Ok(())
}

pub fn upsert_worker(store: &mut SessionSupervisorStore, worker: WorkerSessionRecord) {
    if let Some(existing) = store
        .workers
        .iter_mut()
        .find(|existing| existing.worker_id == worker.worker_id)
    {
        *existing = worker;
    } else {
        store.workers.push(worker);
    }
    store.version += 1;
}

pub fn resolve_state_path(payload: &Value) -> Result<PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|err| format!("read current_dir failed: {err}"))?;
    let default = cwd.join("artifacts/session_supervisor/state.json");
    if let Some(path) = optional_non_empty_string(payload, "state_path") {
        let pb = PathBuf::from(&path);
        let candidate = if pb.is_absolute() {
            pb
        } else {
            cwd.join(&path)
        };
        let temp = std::env::temp_dir();
        let under_cwd = candidate.strip_prefix(&cwd).is_ok();
        let under_tmp = candidate.strip_prefix(&temp).is_ok();
        if !under_cwd && !under_tmp {
            return Err(format!(
                "state_path must be under cwd {} or system temp {}",
                cwd.display(),
                temp.display()
            ));
        }
        Ok(candidate)
    } else {
        Ok(default)
    }
}

pub fn now_from_payload(payload: &Value) -> Result<String, String> {
    if let Some(now) = optional_non_empty_string(payload, "now") {
        parse_rfc3339(&now)?;
        return Ok(now);
    }
    Ok(Utc::now().to_rfc3339())
}

pub fn add_seconds_rfc3339(now: &str, seconds: i64) -> Result<String, String> {
    let dt = parse_rfc3339(now)?;
    Ok((dt + Duration::seconds(seconds)).to_rfc3339())
}

pub fn parse_rfc3339(value: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|err| format!("invalid RFC3339 timestamp {value:?}: {err}"))
}

pub fn required_non_empty_string(
    payload: &Value,
    key: &str,
    context: &str,
) -> Result<String, String> {
    optional_non_empty_string(payload, key)
        .ok_or_else(|| format!("{context} requires a non-empty {key}"))
}

pub fn optional_non_empty_string(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .and_then(|value| {
            if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        })
}

pub fn optional_bool(payload: &Value, key: &str) -> Option<bool> {
    payload.get(key).and_then(Value::as_bool)
}

pub fn optional_i64(payload: &Value, key: &str) -> Option<i64> {
    payload.get(key).and_then(Value::as_i64)
}

pub fn push_event(
    worker: &mut WorkerSessionRecord,
    event: &str,
    status: &str,
    timestamp: &str,
    detail: Option<String>,
) {
    worker.events.push(WorkerEvent {
        event: event.to_string(),
        status: status.to_string(),
        timestamp: timestamp.to_string(),
        detail,
    });
}

pub fn sanitize_segment(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "worker".to_string()
    } else {
        slug
    }
}

pub fn ensure_lane_contract_metadata(
    metadata: Value,
    worker_id: &str,
    host: &str,
    cwd: &str,
    prompt: Option<&str>,
    lane_contract: Option<Value>,
) -> Value {
    let mut object = metadata.as_object().cloned().unwrap_or_default();
    let existing_lane_contract = object.remove("lane_contract").or(lane_contract);
    object.insert(
        "lane_contract".to_string(),
        merge_lane_contract_defaults(
            existing_lane_contract,
            worker_id,
            host,
            cwd,
            prompt.unwrap_or("bounded worker lane"),
        ),
    );
    Value::Object(object)
}

fn merge_lane_contract_defaults(
    lane_contract: Option<Value>,
    worker_id: &str,
    host: &str,
    cwd: &str,
    lane_goal: &str,
) -> Value {
    let defaults = json!({
        "lane_id": worker_id,
        "lane_owner": host,
        "lane_goal": lane_goal,
        "goal": lane_goal,
        "bounded_scope": cwd,
        "forbidden_scope": "outside assigned lane-local scope",
        "verification_required": true,
        "expected_output": {
            "changed_files": [],
            "evidence": [],
            "verification": [],
            "risk": null,
            "next_action": null
        },
        "final_digest": null,
        "evidence_ref": null,
        "integration_status": "planned",
        "verification_status": "not-started",
        "recovery_anchor": worker_id
    });
    let mut merged = defaults.as_object().cloned().unwrap_or_default();
    if let Some(Value::Object(provided)) = lane_contract {
        for (key, value) in provided {
            if key == "expected_output" {
                let mut expected = merged
                    .get("expected_output")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                if let Value::Object(provided_expected) = value {
                    for (nested_key, nested_value) in provided_expected {
                        expected.insert(nested_key, nested_value);
                    }
                    merged.insert(key, Value::Object(expected));
                } else {
                    merged.insert(key, value);
                }
            } else {
                merged.insert(key, value);
            }
        }
    }
    Value::Object(merged)
}
