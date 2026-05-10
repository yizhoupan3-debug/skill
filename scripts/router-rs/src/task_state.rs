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

/// Aggregate "depth compliance" view (P1-A): cross-cuts RFV rounds, EVIDENCE_INDEX, and
/// goal checkpoints into a single read-only score. Consumers (closeout enforcement,
/// SessionStart digest, statusline) can inspect `depth_score` instead of re-deriving the
/// same booleans separately.
///
/// `depth_score` ∈ {0, 1, 2, 3}:
/// - 1 point: at least one RFV round with `verify_result=PASS`.
/// - 1 point: at least one successful EVIDENCE_INDEX row (`success==true` or `exit_code==0`).
/// - 1 point: at least one goal checkpoint recorded (model wrote progress at least once).
///
/// Notes:
/// - This is **advisory** — it does not gate writes. Use it as a discriminator when displaying
///   continuity status or as one of several signals in custom enforcement.
/// - `rfv_unknown_round_count` and `rfv_pass_without_evidence_count` are explicitly broken out
///   so dashboards can flag "RFV says PASS but EVIDENCE shows no successful row in the same
///   window" — the cross-check label written by `rfv_loop::cross_link_evidence`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
pub struct DepthCompliance {
    pub rfv_pass_round_count: u64,
    pub rfv_fail_round_count: u64,
    pub rfv_skipped_round_count: u64,
    pub rfv_unknown_round_count: u64,
    pub rfv_pass_without_evidence_count: u64,
    pub goal_checkpoint_count: u64,
    pub depth_score: u8,
}

fn depth_compliance_from_disk(
    goal: Option<&Value>,
    rfv: Option<&Value>,
    evidence_ok: bool,
) -> DepthCompliance {
    let mut c = DepthCompliance::default();

    if let Some(g) = goal {
        if let Some(arr) = g.get("checkpoints").and_then(Value::as_array) {
            c.goal_checkpoint_count = arr.len() as u64;
        }
    }

    if let Some(r) = rfv {
        if let Some(rounds) = r.get("rounds").and_then(Value::as_array) {
            for round in rounds {
                let vr = round
                    .get("verify_result")
                    .and_then(Value::as_str)
                    .unwrap_or("UNKNOWN")
                    .to_ascii_uppercase();
                match vr.as_str() {
                    "PASS" => c.rfv_pass_round_count += 1,
                    "FAIL" => c.rfv_fail_round_count += 1,
                    "SKIPPED" => c.rfv_skipped_round_count += 1,
                    _ => c.rfv_unknown_round_count += 1,
                }
                if vr == "PASS"
                    && round
                        .get("cross_check")
                        .and_then(Value::as_str)
                        .map(|s| s == "no_evidence_window")
                        .unwrap_or(false)
                {
                    c.rfv_pass_without_evidence_count += 1;
                }
            }
        }
    }

    let mut score: u8 = 0;
    if c.rfv_pass_round_count > 0 {
        score += 1;
    }
    if evidence_ok {
        score += 1;
    }
    if c.goal_checkpoint_count > 0 {
        score += 1;
    }
    c.depth_score = score;
    c
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
    /// Aggregate depth-compliance view (P1-A); always present alongside `evidence` for tasks
    /// with a resolved id. `None` when no task id resolves.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth_compliance: Option<DepthCompliance>,
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
            depth_compliance: None,
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

    let depth_compliance = Some(depth_compliance_from_disk(
        goal_state.as_ref(),
        rfv_loop_state.as_ref(),
        evidence_ok,
    ));

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
        depth_compliance,
        control_mode,
        resolution_notes,
    }
}

/// One-line hint for `framework refresh` / Codex SessionStart digest (`Continuity digest` prompt).
/// Omitted when no resolved `task_id` (idle). Keeps copy short for ~640-char caps.
pub fn depth_compliance_refresh_hint(view: &ResolvedTaskView) -> Option<String> {
    let tid = view.task_id.as_deref()?.trim();
    if tid.is_empty() {
        return None;
    }
    let dc = view.depth_compliance.as_ref()?;
    let mut out = format!("深度信号: d{}/3", dc.depth_score);
    if dc.rfv_pass_without_evidence_count > 0 {
        out.push_str(&format!(
            " · PASS无对照证据={}",
            dc.rfv_pass_without_evidence_count
        ));
    }
    Some(out)
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
        assert!(depth_compliance_refresh_hint(&v).is_none());
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
        let ev = v.evidence.as_ref().expect("evidence");
        assert!(!ev.evidence_rows_non_empty);
        assert!(!ev.has_successful_verification);
        let hint = depth_compliance_refresh_hint(&v).expect("hint");
        assert!(hint.contains("深度信号") && hint.contains("d0/3"));
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
