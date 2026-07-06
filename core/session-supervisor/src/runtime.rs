use chrono::{DateTime, TimeDelta, Utc};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

pub use framework_core::json_value::{
    optional_bool, optional_non_empty_string, required_non_empty_string,
};

use core_errors::FrameworkError;

use crate::types::{
    SESSION_SUPERVISOR_STORE_SCHEMA_VERSION, SessionSupervisorStore, WorkerEvent,
    WorkerSessionRecord,
};

/// Maximum number of events kept per worker. Oldest events are truncated
/// when this limit is reached, preventing unbounded Vec growth (P1 leak).
const MAX_WORKER_EVENTS: usize = 50;

/// Maximum retention for terminated worker records (seconds). Workers in
/// terminal states (interrupted, failed, completed) older than this are
/// removed from the store to prevent unbounded Vec + state.json growth.
const TERMINATED_WORKER_RETENTION_SECS: i64 = 3600; // 1 hour

pub fn load_store(path: &Path) -> Result<SessionSupervisorStore, FrameworkError> {
    if !path.is_file() {
        return Ok(SessionSupervisorStore {
            schema_version: SESSION_SUPERVISOR_STORE_SCHEMA_VERSION.to_string(),
            version: 1,
            workers: Vec::new(),
        });
    }
    let payload: SessionSupervisorStore = serde_json::from_str(&fs::read_to_string(path)?)?;
    Ok(payload)
}

pub fn save_store(path: &Path, store: &SessionSupervisorStore) -> Result<(), FrameworkError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let payload = serde_json::to_string_pretty(store)? + "\n";
    core_state_utils::atomic_write::write_atomic_text(path, &payload)
}

/// Insert or update a worker record in the supervisor store.
///
/// # Concurrency safety
/// This function takes `&mut SessionSupervisorStore` (exclusive access to the in-memory store).
/// All callers in `handle_session_supervisor_operation` acquire a POSIX `flock`
/// (`acquire_runtime_path_lock`) before calling this, ensuring cross-process
/// mutual exclusion on the backing file. Callers outside that path must provide
/// their own synchronization.
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

pub fn resolve_state_path(payload: &Value) -> Result<PathBuf, FrameworkError> {
    let cwd = std::env::current_dir()?;
    let default = cwd.join("artifacts/session_supervisor/state.json");
    if let Some(path) = optional_non_empty_string(payload, "state_path") {
        let pb = PathBuf::from(&path);
        let candidate = if pb.is_absolute() {
            pb
        } else {
            cwd.join(&path)
        };
        let temp = std::env::temp_dir();

        // Security: verify the write target resolves within allowed directories.
        // Canonicalize the parent directory (which must exist) to catch symlink
        // escapes. On macOS /var → /private/var, so both sides need resolving.
        let parent = candidate.parent().unwrap_or(&candidate);
        let resolved_parent = std::fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
        let resolved_cwd = std::fs::canonicalize(&cwd).unwrap_or_else(|_| cwd.clone());
        let resolved_temp = std::fs::canonicalize(&temp).unwrap_or_else(|_| temp.clone());

        let file_name = candidate
            .file_name()
            .map(|n| std::path::Path::new(n))
            .unwrap_or(std::path::Path::new(""));
        let under_cwd = resolved_parent.starts_with(&resolved_cwd);
        let under_tmp = resolved_parent.starts_with(&resolved_temp);
        if !under_cwd && !under_tmp {
            return Err(FrameworkError::validation(format!(
                "state_path must be under cwd {} or system temp {}",
                cwd.display(),
                temp.display()
            )));
        }
        Ok(candidate)
    } else {
        Ok(default)
    }
}

pub fn now_from_payload(payload: &Value) -> Result<String, FrameworkError> {
    if let Some(now) = optional_non_empty_string(payload, "now") {
        parse_rfc3339(&now)?;
        return Ok(now);
    }
    Ok(framework_core::time::now_iso())
}

pub fn add_seconds_rfc3339(now: &str, seconds: i64) -> Result<String, FrameworkError> {
    let dt = parse_rfc3339(now)?;
    Ok((dt + TimeDelta::seconds(seconds)).to_rfc3339())
}

pub fn parse_rfc3339(value: &str) -> Result<DateTime<Utc>, FrameworkError> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|err| {
            FrameworkError::validation(format!("invalid RFC3339 timestamp {value:?}: {err}"))
        })
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
    // Cap at MAX_WORKER_EVENTS to prevent unbounded Vec growth.
    // Workers persist in the store indefinitely (never removed), so without
    // this cap, every lifecycle transition accumulates a permanent event.
    if worker.events.len() > MAX_WORKER_EVENTS {
        let excess = worker.events.len() - MAX_WORKER_EVENTS;
        worker.events.drain(..excess);
    }
}

/// Remove workers from the store that have been in a terminal state
/// (interrupted, completed, failed) longer than `TERMINATED_WORKER_RETENTION_SECS`.
/// Prevents unbounded Vec and state.json growth (P2).
pub fn compact_terminated_workers(workers: &mut Vec<WorkerSessionRecord>, now: &str) {
    let now_dt = match parse_rfc3339(now) {
        Ok(dt) => dt,
        Err(e) => {
            tracing::warn!("compact_terminated_workers: invalid timestamp ({e})");
            return;
        }
    };
    let terminal_statuses = ["interrupted", "completed", "failed"];
    workers.retain(|w| {
        if !terminal_statuses.contains(&w.status.as_str()) {
            return true; // not terminal — keep
        }
        let updated = match parse_rfc3339(&w.updated_at) {
            Ok(dt) => dt,
            Err(_) => return true, // unparseable — keep to be safe
        };
        let age_secs = now_dt.signed_duration_since(updated).num_seconds();
        age_secs < TERMINATED_WORKER_RETENTION_SECS
    });
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
