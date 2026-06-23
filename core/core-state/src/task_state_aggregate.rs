//! Phase 3: optional **aggregate projection** `artifacts/current/<task_id>/TASK_STATE.json`.
//!
//! Canonical source remains `GOAL_STATE.json`, `RFV_LOOP_STATE.json`, `EVIDENCE_INDEX.json`,
//! and task-scoped append logs such as `STEP_LEDGER.jsonl`.
//! This file is refreshed after ledger mutations so humans/tools can open one JSON for goal+rfv+evidence rollup.
//!
//! Design: see `task_state.rs` §5 阶段 3.

use crate::state_manager::read_quality_gate_state;
use crate::state_manager::{read_goal_state, task_evidence_artifacts_summary_for_task};
use crate::utils::atomic_write::write_atomic_json;
use crate::utils::path_guard::{safe_task_id_component, validate_task_id_component};

use serde_json::json;
use std::path::{Path, PathBuf};

pub const TASK_STATE_AGGREGATE_FILENAME: &str = "TASK_STATE.json";
pub const TASK_STATE_AGGREGATE_SCHEMA_VERSION: &str = "router-rs-task-state-aggregate-v1";

pub fn task_state_aggregate_path(repo_root: &Path, task_id: &str) -> PathBuf {
    let task_component = safe_task_id_component(task_id).unwrap_or("__invalid_task_id__");
    repo_root
        .join("artifacts/current")
        .join(task_component)
        .join(TASK_STATE_AGGREGATE_FILENAME)
}

/// Refresh `TASK_STATE.json` from canonical per-task files (does **not** acquire `task_write_lock` —
/// callers must invoke under the same outer serialization as other ledger writes, or single-threaded repair).
pub fn sync_task_state_aggregate(repo_root: &Path, task_id: &str) -> Result<(), String> {
    let tid = task_id.trim();
    if tid.is_empty() {
        return Ok(());
    }
    validate_task_id_component(tid)?;
    let goal_state = read_goal_state(repo_root, Some(tid)).unwrap_or(None);
    let rfv_loop_state = read_quality_gate_state(repo_root, Some(tid)).unwrap_or(None);
    let (evidence_rows, evidence_ok) = task_evidence_artifacts_summary_for_task(repo_root, tid);
    let step_ledger = crate::step_ledger::summarize_step_ledger_for_task(repo_root, tid);

    // Read TASK_LEDGER.jsonl to find the highest seq number
    let last_seq = crate::task_state::read_task_ledger_transactions(repo_root, tid)
        .iter()
        .filter_map(|tx| tx.seq)
        .max();

    let payload = json!({
        "schema_version": TASK_STATE_AGGREGATE_SCHEMA_VERSION,
        "task_id": tid,
        "synced_at": framework_kernel::time::now_iso(),
        "goal_state": goal_state,
        "rfv_loop_state": rfv_loop_state,
        "evidence": {
            "evidence_rows_non_empty": evidence_rows,
            "has_successful_verification": evidence_ok,
        },
        "step_ledger": step_ledger,
        "last_seq": last_seq,
        "note": "Projection only; canonical GOAL_STATE.json / RFV_LOOP_STATE.json / EVIDENCE_INDEX.json / STEP_LEDGER.jsonl remain authoritative."
    });
    let path = task_state_aggregate_path(repo_root, tid);
    write_atomic_json(&path, &payload)
}

pub fn sync_task_state_aggregate_best_effort(repo_root: &Path, task_id: &str) {
    if task_id.trim().is_empty() {
        return;
    }
    if let Err(e) = sync_task_state_aggregate(repo_root, task_id) {
        tracing::warn!(task_id = %task_id.trim(), error = %e, "TASK_STATE_AGGREGATE_SYNC_FAILED");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_manager::framework_goal_drive;
    use serde_json::{Value, json};
    use std::fs;

    #[test]
    fn sync_writes_after_goal_start() {
        let _env = ();
        let prev = std::env::var_os("ROUTER_RS_TASK_STATE_AGGREGATE_AUTO");
        unsafe { std::env::set_var("ROUTER_RS_TASK_STATE_AGGREGATE_AUTO", "1") };
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
        framework_goal_drive(json!({
            "repo_root": tmp.display().to_string(),
            "operation": "start",
            "task_id": "t-agg",
            "goal": "phase3 aggregate",
            "non_goals": ["n"],
            "done_when": ["d1", "d2"],
            "validation_commands": ["cargo test -q"],
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
        match prev {
            Some(v) => unsafe { std::env::set_var("ROUTER_RS_TASK_STATE_AGGREGATE_AUTO", v) },
            None => unsafe { std::env::remove_var("ROUTER_RS_TASK_STATE_AGGREGATE_AUTO") },
        }
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn sync_rejects_task_id_path_traversal() {
        let tmp = std::env::temp_dir().join(format!(
            "router-rs-task-agg-traversal-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = fs::remove_dir_all(&tmp);
        let err = sync_task_state_aggregate(&tmp, "../outside").unwrap_err();
        assert!(err.contains("safe path component"), "{err}");
        assert!(!tmp.join("artifacts/outside/TASK_STATE.json").exists());
        let _ = fs::remove_dir_all(&tmp);
    }
}
