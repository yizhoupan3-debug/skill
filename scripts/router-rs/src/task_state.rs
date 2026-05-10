//! Unified **read model** for per-task continuity artifacts (L2) consumed by L3.
//!
//! Design: `docs/task_state_unified_resolve.md`. **Writes** serialize via `task_write_lock`
//! (phase 2); this module only aggregates read models (`ResolvedTaskView`, `CursorContinuityFrame`).

use crate::autopilot_goal::{
    goal_state_requests_continuation, read_active_task_id, read_focus_task_id, read_goal_state,
    task_evidence_artifacts_summary_for_task,
};
use crate::rfv_loop::read_rfv_loop_state;
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

pub const RESOLVED_TASK_VIEW_SCHEMA_VERSION: &str = "router-rs-resolved-task-view-v1";

/// Single disk snapshot for Cursor **beforeSubmit** / **stop** continuity + gate hydrate.
///
/// - `pointer_view`：`resolve_task_view`（override 无 → active → focus），供续跑合并时与 `active_task` 对齐缓存。
/// - `hydration_goal`：与 `read_goal_state_for_hydration` 一致（含 orphan 扫盘），供 `AG_FOLLOWUP` hydrate。
#[derive(Debug, Clone)]
pub struct CursorContinuityFrame {
    pub pointer_view: ResolvedTaskView,
    pub hydration_goal: Option<(serde_json::Value, String)>,
}

/// beforeSubmit / Stop 入口：一次构建指针视图 + hydration 目标对。
pub fn resolve_cursor_continuity_frame(repo_root: &Path) -> CursorContinuityFrame {
    let pointer_view = resolve_task_view(repo_root, None);
    let hydration_goal = crate::autopilot_goal::read_goal_state_for_hydration(repo_root)
        .ok()
        .flatten();
    CursorContinuityFrame {
        pointer_view,
        hydration_goal,
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TaskPointers {
    pub active_task_id: Option<String>,
    pub focus_task_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EvidenceRollup {
    pub task_id: String,
    pub evidence_rows_non_empty: bool,
    pub has_successful_verification: bool,
}

/// High-level macro-controller mode for the resolved task id.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskControlMode {
    Idle,
    Autopilot,
    RfvLoop,
    Conflict { reason: String },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ResolvedTaskView {
    pub schema_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    pub pointers: TaskPointers,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal_state: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rfv_loop_state: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<EvidenceRollup>,
    pub control_mode: TaskControlMode,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub resolution_notes: Vec<String>,
}

fn rfv_loop_active(state: &Value) -> bool {
    state
        .get("loop_status")
        .and_then(Value::as_str)
        .is_some_and(|s| s.eq_ignore_ascii_case("active"))
}

fn classify_control_mode(
    goal: Option<&Value>,
    rfv: Option<&Value>,
    notes: &mut Vec<String>,
) -> TaskControlMode {
    let g_on = goal.is_some_and(goal_state_requests_continuation);
    let r_on = rfv.is_some_and(rfv_loop_active);
    match (g_on, r_on) {
        (true, true) => {
            notes.push(
                "goal macro (drive_until_done+running) and RFV loop_status=active both true; invariant violation expected to be prevented by writers"
                    .to_string(),
            );
            TaskControlMode::Conflict {
                reason: "autopilot_goal_and_rfv_loop_both_active".to_string(),
            }
        }
        (true, false) => TaskControlMode::Autopilot,
        (false, true) => TaskControlMode::RfvLoop,
        (false, false) => TaskControlMode::Idle,
    }
}

/// Resolve a single view for continuity debugging and future hook consumption.
///
/// `task_id` resolution: `task_id_override` (non-empty) > `active_task.json` > `focus_task.json`.
/// Does **not** scan `**/GOAL_STATE.json` by mtime (see design doc).
pub fn resolve_task_view(repo_root: &Path, task_id_override: Option<&str>) -> ResolvedTaskView {
    let pointers = TaskPointers {
        active_task_id: read_active_task_id(repo_root),
        focus_task_id: read_focus_task_id(repo_root),
    };

    let tid = task_id_override
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| pointers.active_task_id.clone())
        .or_else(|| pointers.focus_task_id.clone());

    let mut resolution_notes: Vec<String> = Vec::new();

    let Some(ref task_id) = tid else {
        return ResolvedTaskView {
            schema_version: RESOLVED_TASK_VIEW_SCHEMA_VERSION.to_string(),
            task_id: None,
            pointers,
            goal_state: None,
            rfv_loop_state: None,
            evidence: None,
            control_mode: TaskControlMode::Idle,
            resolution_notes: vec![
                "no_task_id: override empty and no active/focus pointer".to_string()
            ],
        };
    };

    let goal_state = read_goal_state(repo_root, Some(task_id.as_str())).unwrap_or(None);
    let rfv_loop_state = read_rfv_loop_state(repo_root, Some(task_id.as_str())).unwrap_or(None);

    let (evidence_rows, evidence_ok) =
        task_evidence_artifacts_summary_for_task(repo_root, task_id.as_str());
    let evidence = Some(EvidenceRollup {
        task_id: task_id.clone(),
        evidence_rows_non_empty: evidence_rows,
        has_successful_verification: evidence_ok,
    });

    let control_mode = classify_control_mode(
        goal_state.as_ref(),
        rfv_loop_state.as_ref(),
        &mut resolution_notes,
    );

    ResolvedTaskView {
        schema_version: RESOLVED_TASK_VIEW_SCHEMA_VERSION.to_string(),
        task_id: Some(task_id.clone()),
        pointers,
        goal_state,
        rfv_loop_state,
        evidence,
        control_mode,
        resolution_notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_repo(label: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("router-rs-task-state-{label}-{nonce}"))
    }

    fn write_active(tmp: &Path, id: &str) {
        let p = tmp.join("artifacts/current/active_task.json");
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, format!(r#"{{"task_id":"{id}"}}"#)).unwrap();
    }

    #[test]
    fn no_pointer_idle() {
        let tmp = unique_repo("no-ptr");
        fs::create_dir_all(&tmp).unwrap();
        let v = resolve_task_view(&tmp, None);
        assert_eq!(v.task_id, None);
        assert!(matches!(v.control_mode, TaskControlMode::Idle));
        assert!(!v.resolution_notes.is_empty());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn active_only_goal_autopilot() {
        let tmp = unique_repo("goal");
        let tid = "t1";
        write_active(&tmp, tid);
        let task_dir = tmp.join("artifacts/current").join(tid);
        fs::create_dir_all(&task_dir).unwrap();
        fs::write(
            task_dir.join("GOAL_STATE.json"),
            serde_json::to_string_pretty(&json!({
                "schema_version": "router-rs-autopilot-goal-v1",
                "drive_until_done": true,
                "status": "running",
                "goal": "ship feature",
                "non_goals": [],
                "done_when": [],
                "validation_commands": [],
                "current_horizon": "",
                "checkpoints": [],
                "blocker": null,
                "updated_at": "2026-01-01T00:00:00Z"
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            task_dir.join("EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[]}"#,
        )
        .unwrap();

        let v = resolve_task_view(&tmp, None);
        assert_eq!(v.task_id.as_deref(), Some(tid));
        assert!(matches!(v.control_mode, TaskControlMode::Autopilot));
        let ev = v.evidence.expect("evidence");
        assert!(!ev.evidence_rows_non_empty);
        assert!(!ev.has_successful_verification);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn conflict_when_goal_and_rfv_active() {
        let tmp = unique_repo("conflict");
        let tid = "t2";
        write_active(&tmp, tid);
        let task_dir = tmp.join("artifacts/current").join(tid);
        fs::create_dir_all(&task_dir).unwrap();
        fs::write(
            task_dir.join("GOAL_STATE.json"),
            serde_json::to_string_pretty(&json!({
                "drive_until_done": true,
                "status": "running",
                "goal": "g"
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            task_dir.join("RFV_LOOP_STATE.json"),
            serde_json::to_string_pretty(&json!({
                "schema_version": "router-rs-rfv-loop-v1",
                "loop_status": "active",
                "goal": "g",
                "max_rounds": 3,
                "current_round": 0,
                "rounds": []
            }))
            .unwrap(),
        )
        .unwrap();

        let v = resolve_task_view(&tmp, None);
        assert!(matches!(v.control_mode, TaskControlMode::Conflict { .. }));
        assert!(!v.resolution_notes.is_empty());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn override_wins_over_active() {
        let tmp = unique_repo("override");
        write_active(&tmp, "active-id");
        let other = "other-id";
        let task_dir = tmp.join("artifacts/current").join(other);
        fs::create_dir_all(&task_dir).unwrap();
        fs::write(
            task_dir.join("GOAL_STATE.json"),
            serde_json::to_string_pretty(&json!({
                "drive_until_done": false,
                "status": "completed",
                "goal": "done"
            }))
            .unwrap(),
        )
        .unwrap();

        let v = resolve_task_view(&tmp, Some(other));
        assert_eq!(v.task_id.as_deref(), Some(other));
        assert!(matches!(v.control_mode, TaskControlMode::Idle));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn continuity_frame_hydration_finds_orphan_goal_without_active_pointer() {
        let tmp = unique_repo("orphan-hydr");
        let tid = "t-orph";
        let task_dir = tmp.join("artifacts/current").join(tid);
        fs::create_dir_all(&task_dir).unwrap();
        fs::write(
            task_dir.join("GOAL_STATE.json"),
            r#"{"goal":"orphan","status":"running","drive_until_done":true}"#,
        )
        .unwrap();
        let frame = resolve_cursor_continuity_frame(&tmp);
        assert!(frame.pointer_view.task_id.is_none());
        let (g, id) = frame.hydration_goal.expect("hydration");
        assert_eq!(id, tid);
        assert_eq!(g.get("goal").and_then(Value::as_str), Some("orphan"));
        let _ = fs::remove_dir_all(&tmp);
    }
}
