//! Ship-readiness implementation: evaluates goal readiness from disk artifacts.
//!
//! Reads `GOAL_STATE.json` and `EVIDENCE_INDEX.json` under
//! `artifacts/current/<task_id>/` to compute `GoalReadiness` flags
//! (contract, progress, verification).

use std::path::Path;
use serde_json::Value;

/// Evaluate goal readiness by inspecting GOAL_STATE.json and EVIDENCE_INDEX.json
/// on disk.
pub fn evaluate_goal_readiness(repo_root: &Path, goal: &Value, task_id: &str) -> host_projection::hooks::GoalReadiness {
    // ── contract: goal document is well-formed ──
    let has_goal_text = goal
        .get("goal")
        .and_then(Value::as_str)
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let has_non_goals = goal
        .get("non_goals")
        .and_then(Value::as_array)
        .map(|a| a.iter().any(|v| v.as_str().is_some_and(|s| !s.trim().is_empty())))
        .unwrap_or(false);
    let has_validation = goal
        .get("validation_commands")
        .and_then(Value::as_array)
        .map(|a| a.iter().any(|v| v.as_str().is_some_and(|s| !s.trim().is_empty())))
        .unwrap_or(false);
    let done_when_count = goal
        .get("done_when")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter(|v| v.as_str().is_some_and(|s| !s.trim().is_empty()))
                .count()
        })
        .unwrap_or(0);
    let contract = has_goal_text && has_non_goals && has_validation && done_when_count >= 2;

    // ── progress: checkpoints or evidence exist ──
    let has_checkpoints = goal
        .get("checkpoints")
        .and_then(Value::as_array)
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    let evidence_path = repo_root
        .join("artifacts/current")
        .join(task_id)
        .join("EVIDENCE_INDEX.json");
    let has_evidence = if evidence_path.is_file() {
        std::fs::read_to_string(&evidence_path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            .and_then(|v| v.get("artifacts").and_then(Value::as_array).cloned())
            .map(|a| !a.is_empty())
            .unwrap_or(false)
    } else {
        false
    };
    let status = goal.get("status").and_then(Value::as_str).unwrap_or("");
    let progress = has_checkpoints || has_evidence || status == "completed";

    // ── verification: evidence or completed status ──
    let verification = has_evidence
        || status == "completed"
        || (has_checkpoints && status == "running");

    host_projection::hooks::GoalReadiness {
        contract,
        progress,
        verification,
    }
}

/// Generate a human-readable followup line listing missing readiness parts.
pub fn goal_stop_followup(
    contract: bool,
    progress: bool,
    verification: bool,
    goal_followup_count: u32,
) -> String {
    let mut missing = Vec::new();
    if !contract {
        missing.push("goal_contract");
    }
    if !progress {
        missing.push("checkpoint_progress");
    }
    if !verification {
        missing.push("verification_or_blocker");
    }
    let joined = missing.join(",");
    let mut line = format!("router-rs AG_FOLLOWUP missing_parts={joined}");
    if !contract {
        line.push_str(" primary_fix=goal_contract");
    } else if !progress {
        line.push_str(" primary_fix=checkpoint_progress");
    } else if !verification {
        line.push_str(" primary_fix=verification_or_blocker");
    }
    if goal_followup_count >= 3 {
        line.push_str(" | 已连续多轮 Stop 未满足门控；若确为小任务请直接单独一行 small_task");
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn evaluate_returns_default_when_no_goal_state() {
        let tmp = TempDir::new().unwrap();
        let goal = json!({"goal": "test", "non_goals": ["ng"], "validation_commands": ["vc"], "done_when": ["a", "b"]});
        let r = evaluate_goal_readiness(tmp.path(), &goal, "t1");
        // contract = true (all fields present), progress = false (no checkpoints/evidence), verification = false
        assert!(r.contract);
        assert!(!r.progress);
        assert!(!r.verification);
    }

    #[test]
    fn evaluate_with_evidence_file() {
        let tmp = TempDir::new().unwrap();
        let task_dir = tmp.path().join("artifacts/current/t1");
        fs::create_dir_all(&task_dir).unwrap();
        let evidence = json!({"artifacts": [{"tool": "test", "command": "cargo test", "success": true}]});
        fs::write(task_dir.join("EVIDENCE_INDEX.json"), evidence.to_string()).unwrap();
        let goal = json!({"goal": "g", "non_goals": ["ng"], "validation_commands": ["vc"], "done_when": ["a", "b"], "status": "running"});
        let r = evaluate_goal_readiness(tmp.path(), &goal, "t1");
        assert!(r.contract);
        assert!(r.progress); // has evidence
        assert!(r.verification); // has evidence
    }

    #[test]
    fn evaluate_completed_status() {
        let tmp = TempDir::new().unwrap();
        let goal = json!({"goal": "g", "non_goals": ["ng"], "validation_commands": ["vc"], "done_when": ["a", "b"], "status": "completed"});
        let r = evaluate_goal_readiness(tmp.path(), &goal, "t1");
        assert!(r.contract);
        assert!(r.progress);
        assert!(r.verification);
    }

    #[test]
    fn followup_reports_missing_parts() {
        let line = goal_stop_followup(false, false, false, 0);
        assert!(line.contains("goal_contract"));
        assert!(line.contains("checkpoint_progress"));
        assert!(line.contains("verification_or_blocker"));
        assert!(line.contains("primary_fix=goal_contract"));
    }

    #[test]
    fn followup_escalation_after_3_rounds() {
        let line = goal_stop_followup(true, false, false, 3);
        assert!(line.contains("已连续多轮 Stop"));
    }
}
