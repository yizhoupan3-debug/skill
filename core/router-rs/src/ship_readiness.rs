//! Single disk-first evaluator for Cursor goal / Stop `AG_FOLLOWUP` (ShipReadiness).
//! Chat supplements contract only when no readable `GOAL_STATE` is on the hydration pointer.

use std::path::Path;

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GoalReadiness {
    pub contract: bool,
    pub progress: bool,
    pub verification: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalPrimaryFix {
    GoalContract,
    CheckpointProgress,
    VerificationOrBlocker,
}

impl GoalPrimaryFix {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GoalContract => "write_goal_state_or_framework_goal_drive_start",
            Self::CheckpointProgress => "framework_goal_drive_checkpoint_or_evidence",
            Self::VerificationOrBlocker => "run_validation_commands_or_append_evidence_index",
        }
    }
}

fn goal_state_list_any_nonempty_string(goal: &Value, key: &str) -> bool {
    match goal.get(key) {
        None => false,
        Some(Value::Array(items)) => items
            .iter()
            .any(|v| v.as_str().map(|s| !s.trim().is_empty()).unwrap_or(false)),
        Some(Value::String(s)) => !s.trim().is_empty(),
        _ => false,
    }
}

/// Disk-only readiness from `GOAL_STATE` + `EVIDENCE_INDEX` (no chat keywords).
pub fn evaluate_goal_readiness_from_disk(
    repo_root: &Path,
    goal: &Value,
    task_id: &str,
) -> GoalReadiness {
    let gtext = goal
        .get("goal")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    let has_goal_text = !gtext.is_empty();
    let validation_nonempty = goal_state_list_any_nonempty_string(goal, "validation_commands");
    let non_goals_nonempty = goal_state_list_any_nonempty_string(goal, "non_goals");
    let done_when_items = goal
        .get("done_when")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .count()
        })
        .unwrap_or(0);

    let contract =
        has_goal_text && non_goals_nonempty && validation_nonempty && done_when_items >= 2;

    let checkpointed = goal
        .get("checkpoints")
        .and_then(Value::as_array)
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    let (evidence_rows, evidence_ok) =
        crate::autopilot_goal::task_evidence_artifacts_summary_for_task(repo_root, task_id);

    let st_lc = goal
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let blocker = goal
        .get("blocker")
        .and_then(Value::as_str)
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    let progress = checkpointed || evidence_rows;

    let drive = goal.get("drive_until_done").and_then(Value::as_bool).unwrap_or(false);
    let mut verification = matches!(st_lc.as_str(), "blocked" | "completed" | "paused")
        || blocker
        || evidence_ok
        || checkpointed;
    if !drive && matches!(st_lc.as_str(), "planned" | "draft") {
        verification = true;
    }

    GoalReadiness {
        contract,
        progress,
        verification,
    }
}

#[allow(dead_code)]
pub fn goal_is_satisfied_flags(contract: bool, progress: bool, verification: bool) -> bool {
    contract && progress && verification
}

pub fn missing_parts_flags(contract: bool, progress: bool, verification: bool) -> Vec<&'static str> {
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
    missing
}

pub fn primary_fix_for_missing_flags(
    contract: bool,
    progress: bool,
    verification: bool,
) -> Option<GoalPrimaryFix> {
    if !contract {
        return Some(GoalPrimaryFix::GoalContract);
    }
    if !progress {
        return Some(GoalPrimaryFix::CheckpointProgress);
    }
    if !verification {
        return Some(GoalPrimaryFix::VerificationOrBlocker);
    }
    None
}

/// Stop hard line: `router-rs AG_FOLLOWUP missing_parts=… primary_fix=…`
pub fn goal_stop_followup_line(
    contract: bool,
    progress: bool,
    verification: bool,
    goal_followup_count: u32,
) -> String {
    let parts = missing_parts_flags(contract, progress, verification);
    let joined = parts.join(",");
    let mut line = format!("router-rs AG_FOLLOWUP missing_parts={joined}");
    if let Some(fix) = primary_fix_for_missing_flags(contract, progress, verification) {
        line.push_str(" primary_fix=");
        line.push_str(fix.as_str());
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
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_repo() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("ship-readiness-test-{nonce}"));
        let task_id = "t-ship";
        fs::create_dir_all(root.join("artifacts/current").join(task_id)).expect("mkdir");
        fs::write(
            root.join("artifacts/current/active_task.json"),
            format!(r#"{{"task_id":"{task_id}"}}"#),
        )
        .expect("active");
        root
    }

    #[test]
    fn goal_is_satisfied_flags_requires_all_three() {
        assert!(!goal_is_satisfied_flags(false, true, true));
        assert!(goal_is_satisfied_flags(true, true, true));
    }

    #[test]
    fn missing_parts_and_primary_fix_order() {
        assert_eq!(
            missing_parts_flags(false, false, false),
            vec!["goal_contract", "checkpoint_progress", "verification_or_blocker"]
        );
        assert_eq!(
            primary_fix_for_missing_flags(true, false, true),
            Some(GoalPrimaryFix::CheckpointProgress)
        );
    }

    #[test]
    fn evaluate_goal_readiness_from_disk_requires_contract_and_evidence() {
        let repo = temp_repo();
        let task_id = "t-ship";
        let goal = json!({
            "goal": "ship harness",
            "non_goals": ["scope creep"],
            "done_when": ["a", "b"],
            "validation_commands": ["cargo test"],
            "status": "running",
            "drive_until_done": true,
            "checkpoints": []
        });
        let r = evaluate_goal_readiness_from_disk(&repo, &goal, task_id);
        assert!(r.contract);
        assert!(!r.progress);
        assert!(!r.verification);
        let line = goal_stop_followup_line(r.contract, r.progress, r.verification, 0);
        assert!(line.starts_with("router-rs AG_FOLLOWUP"));
        assert!(line.contains("primary_fix="));
    }

    #[test]
    fn evaluate_goal_readiness_planned_without_drive_skips_verify_block() {
        let repo = temp_repo();
        let goal = json!({
            "goal": "plan only",
            "non_goals": ["n"],
            "done_when": ["a", "b"],
            "validation_commands": ["echo ok"],
            "status": "planned",
            "drive_until_done": false
        });
        let r = evaluate_goal_readiness_from_disk(&repo, &goal, "t-ship");
        assert!(r.contract);
        assert!(r.verification);
    }
}
