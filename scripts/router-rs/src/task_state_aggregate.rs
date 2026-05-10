//! Phase 3: optional **aggregate projection** `artifacts/current/<task_id>/TASK_STATE.json`.
//!
//! Canonical source remains `GOAL_STATE.json`, `RFV_LOOP_STATE.json`, and `EVIDENCE_INDEX.json`.
//! This file is refreshed after ledger mutations so humans/tools can open one JSON for goal+rfv+evidence rollup.
//!
//! Design: `docs/task_state_unified_resolve.md` §5 阶段 3.

use crate::autopilot_goal::{read_goal_state, task_evidence_artifacts_summary_for_task};
use crate::rfv_loop::read_rfv_loop_state;
use chrono::Utc;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub const TASK_STATE_AGGREGATE_FILENAME: &str = "TASK_STATE.json";
pub const TASK_STATE_AGGREGATE_SCHEMA_VERSION: &str = "router-rs-task-state-aggregate-v1";

pub fn task_state_aggregate_path(repo_root: &Path, task_id: &str) -> PathBuf {
    repo_root
        .join("artifacts/current")
        .join(task_id.trim())
        .join(TASK_STATE_AGGREGATE_FILENAME)
}

fn write_atomic_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create parent directory failed: {err}"))?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|err| format!("serialize TASK_STATE aggregate failed: {err}"))?;
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, text)
        .map_err(|err| format!("write temp file failed for {}: {err}", tmp_path.display()))?;
    fs::rename(&tmp_path, path).map_err(|err| {
        let _ = fs::remove_file(&tmp_path);
        format!(
            "rename temp file failed {} -> {}: {err}",
            tmp_path.display(),
            path.display()
        )
    })?;
    Ok(())
}

/// Refresh `TASK_STATE.json` from canonical per-task files (does **not** acquire `task_write_lock` —
/// callers must invoke under the same outer serialization as other ledger writes, or single-threaded repair).
pub fn sync_task_state_aggregate(repo_root: &Path, task_id: &str) -> Result<(), String> {
    let tid = task_id.trim();
    if tid.is_empty() {
        return Ok(());
    }
    let goal_state = read_goal_state(repo_root, Some(tid)).unwrap_or(None);
    let rfv_loop_state = read_rfv_loop_state(repo_root, Some(tid)).unwrap_or(None);
    let (evidence_rows, evidence_ok) = task_evidence_artifacts_summary_for_task(repo_root, tid);

    let payload = json!({
        "schema_version": TASK_STATE_AGGREGATE_SCHEMA_VERSION,
        "task_id": tid,
        "synced_at": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "goal_state": goal_state,
        "rfv_loop_state": rfv_loop_state,
        "evidence": {
            "evidence_rows_non_empty": evidence_rows,
            "has_successful_verification": evidence_ok,
        },
        "note": "Projection only; canonical GOAL_STATE.json / RFV_LOOP_STATE.json / EVIDENCE_INDEX.json remain authoritative."
    });
    let path = task_state_aggregate_path(repo_root, tid);
    write_atomic_json(&path, &payload)
}

pub(crate) fn sync_task_state_aggregate_best_effort(repo_root: &Path, task_id: &str) {
    if task_id.trim().is_empty() {
        return;
    }
    if let Err(e) = sync_task_state_aggregate(repo_root, task_id) {
        eprintln!(
            "[router-rs] TASK_STATE.json aggregate sync failed (task_id={}): {}",
            task_id.trim(),
            e
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autopilot_goal::framework_autopilot_goal;
    use serde_json::json;
    use std::fs;

    #[test]
    fn sync_writes_after_goal_start() {
        let tmp = std::env::temp_dir().join(format!(
            "router-rs-task-agg-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("artifacts/current/t-agg")).expect("mkdir");
        fs::write(
            tmp.join("artifacts/current/active_task.json"),
            r#"{"task_id":"t-agg"}"#,
        )
        .expect("active");
        framework_autopilot_goal(json!({
            "repo_root": tmp.display().to_string(),
            "operation": "start",
            "task_id": "t-agg",
            "goal": "phase3 aggregate",
            "drive_until_done": true,
        }))
        .expect("start");
        let p = task_state_aggregate_path(&tmp, "t-agg");
        assert!(p.is_file(), "TASK_STATE.json missing at {}", p.display());
        let raw = fs::read_to_string(&p).expect("read");
        let v: Value = serde_json::from_str(&raw).expect("json");
        assert_eq!(
            v.get("schema_version").and_then(Value::as_str),
            Some(TASK_STATE_AGGREGATE_SCHEMA_VERSION)
        );
        assert_eq!(v.get("task_id").and_then(Value::as_str), Some("t-agg"));
        assert!(v.get("goal_state").is_some());
        let _ = fs::remove_dir_all(&tmp);
    }
}
