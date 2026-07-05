//! Transition validation — evidence-chain anti-fraud gate (Wave 3c-i, Phase A→C).
//!
//! Validates that task transitions (especially Complete) are backed by real
//! evidence artifacts before allowing the transition. This is the "anti-fraud"
//! gate from the v10 architecture: it prevents claiming completion without
//! verifiable evidence.
//!
//! Integration path:
//! - **No-goal path** (`tool_task_complete`): `validate_transition()` as a
//!   **blocking gate** — returns Err if evidence is incomplete.
//! - **Goal path** (`framework_goal_drive complete`): `validate_transition()` is the
//!   **authoritative blocking gate**.

#![deny(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

/// The type of task transition being validated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskTransition {
    /// Starting a task — no evidence required.
    Start,
    /// Completing a task — requires evidence chain verification.
    Complete,
    /// Failing a task — no evidence required.
    Fail,
}

/// Result of a transition validation.
#[derive(Debug, Clone)]
pub struct TransitionVerdict {
    /// True if the transition is allowed.
    pub passed: bool,
    /// Human-readable reason for the verdict.
    pub reason: String,
    /// Number of evidence artifacts found.
    pub evidence_count: usize,
    /// The target transition type (e.g. "Start", "Complete", "Fail").
    pub transition_target: Option<String>,
}

impl TransitionVerdict {
    fn allowed(reason: impl Into<String>) -> Self {
        Self {
            passed: true,
            reason: reason.into(),
            evidence_count: 0,
            transition_target: None,
        }
    }

    fn blocked(reason: impl Into<String>) -> Self {
        Self {
            passed: false,
            reason: reason.into(),
            evidence_count: 0,
            transition_target: None,
        }
    }

    fn with_evidence_count(mut self, count: usize) -> Self {
        self.evidence_count = count;
        self
    }

    fn with_transition_target(mut self, target: Option<String>) -> Self {
        self.transition_target = target;
        self
    }
}

/// Validate a task transition against the evidence chain.
///
/// For `Complete` transitions, checks that `EVIDENCE_INDEX.json` exists and
/// contains at least one artifact with `exit_code == 0` or `success == true`.
/// `Start` and `Fail` transitions always pass.
///
/// Uses the same evidence summary function as the QG entry point
/// (`task_evidence_artifacts_summary_for_task`).
pub fn validate_transition(
    repo_root: &Path,
    task_id: &str,
    transition: TaskTransition,
) -> TransitionVerdict {
    match transition {
        TaskTransition::Start => {
            TransitionVerdict::allowed("start transition — no evidence required")
                .with_transition_target(Some("Start".to_string()))
        }
        TaskTransition::Fail => {
            TransitionVerdict::allowed("fail transition — no evidence required")
                .with_transition_target(Some("Fail".to_string()))
        }
        TaskTransition::Complete => validate_complete_transition(repo_root, task_id),
    }
}

fn count_evidence_artifacts_inner(repo_root: &Path, tid: &str) -> usize {
    let index_path = repo_root
        .join("artifacts/current")
        .join(tid)
        .join("EVIDENCE_INDEX.json");
    if !index_path.is_file() {
        return 0;
    }
    let Ok(raw) = std::fs::read_to_string(&index_path) else {
        return 0;
    };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return 0;
    };
    val.get("artifacts")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0)
}

fn validate_complete_transition(repo_root: &Path, task_id: &str) -> TransitionVerdict {
    let tid = task_id.trim();
    if tid.is_empty() {
        return TransitionVerdict::blocked("task_id is empty")
            .with_transition_target(Some("Complete".to_string()));
    }

    // P2-03: Validate task_id before path construction
    if let Err(e) = crate::utils::path_guard::validate_task_id_component(tid) {
        return TransitionVerdict::blocked(format!("invalid task_id: {e}"))
            .with_transition_target(Some("Complete".to_string()));
    }

    // Read GOAL_STATE.json to check existence
    let goal_state_path = repo_root
        .join("artifacts/current")
        .join(tid)
        .join("GOAL_STATE.json");
    let goal_state_raw = goal_state_path.is_file().then(|| {
        std::fs::read_to_string(&goal_state_path).ok()
    }).flatten();
    let has_goal_state = goal_state_raw.is_some();

    let evidence_count = count_evidence_artifacts_inner(repo_root, tid);

    if !has_goal_state {
        // D5: No GOAL_STATE.json = no active task.
        // If evidence exists for this task_id, verify it before auto-passing.
        let (has_ev, ev_ok) =
            crate::state_manager::task_evidence_artifacts_summary_for_task(repo_root, tid);
        if has_ev && !ev_ok {
            return TransitionVerdict::blocked(format!(
                "task '{tid}' has no GOAL_STATE.json but evidence exists and indicates failure"
            ))
            .with_evidence_count(evidence_count)
            .with_transition_target(Some("Complete".to_string()));
        }
        return TransitionVerdict::allowed(
            "no GOAL_STATE.json found for this task — D5 auto-pass",
        )
        .with_evidence_count(evidence_count)
        .with_transition_target(Some("Complete".to_string()));
    }

    let (has_evidence, evidence_ok) =
        crate::state_manager::task_evidence_artifacts_summary_for_task(repo_root, tid);

    if !has_evidence {
        return TransitionVerdict::blocked(format!(
            "task '{tid}' has no evidence artifacts — cannot verify completion"
        ))
        .with_evidence_count(evidence_count)
        .with_transition_target(Some("Complete".to_string()));
    }

    if !evidence_ok {
        return TransitionVerdict::blocked(format!(
            "task '{tid}' has evidence artifacts but none indicate success \
                 (no exit_code=0 or success=true)"
        ))
        .with_evidence_count(evidence_count)
        .with_transition_target(Some("Complete".to_string()));
    }

    TransitionVerdict::allowed(format!(
        "task '{tid}' has valid evidence artifacts confirming completion"
    ))
    .with_evidence_count(evidence_count)
    .with_transition_target(Some("Complete".to_string()))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::OnceLock;

    #[allow(dead_code)]
    static TEST_INIT: OnceLock<()> = OnceLock::new();

    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("transition-validation-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("artifacts/current/test-task")).expect("mkdir");
        dir
    }

    fn write_evidence(dir: &Path, artifacts: &[(&str, bool)]) {
        let ents: Vec<serde_json::Value> = artifacts
            .iter()
            .map(|(id, success)| {
                serde_json::json!({
                    "artifact_id": id,
                    "exit_code": if *success { 0 } else { 1 },
                    "success": *success,
                })
            })
            .collect();
        let index = serde_json::json!({ "artifacts": ents });
        let path = dir.join("artifacts/current/test-task/EVIDENCE_INDEX.json");
        fs::write(&path, serde_json::to_string_pretty(&index).unwrap()).expect("write evidence");
    }

    /// Write a minimal GOAL_STATE.json to simulate task existence for evidence checks.
    fn write_task_goal_state(dir: &Path) {
        let goal_state = serde_json::json!({
            "schema_version": "router-rs-goal-v1",
            "status": "running",
            "goal": "test",
        });
        let goal_dir = dir.join("artifacts/current/test-task");
        fs::create_dir_all(&goal_dir).expect("mkdir goal dir");
        let path = goal_dir.join("GOAL_STATE.json");
        fs::write(&path, serde_json::to_string_pretty(&goal_state).unwrap())
            .expect("write GOAL_STATE.json");
    }

    #[test]
    fn start_transition_always_allowed() {
        let dir = test_dir("start");
        let v = validate_transition(&dir, "test-task", TaskTransition::Start);
        assert!(v.passed);
    }

    #[test]
    fn fail_transition_always_allowed() {
        let dir = test_dir("fail");
        let v = validate_transition(&dir, "test-task", TaskTransition::Fail);
        assert!(v.passed);
    }

    #[test]
    fn complete_transition_no_task_goal_state_d5_auto_pass() {
        // D5: no GOAL_STATE.json = no active task = auto-pass.
        let dir = test_dir("d5-no-goal-state");
        let v = validate_transition(&dir, "test-task", TaskTransition::Complete);
        assert!(v.passed, "D5: no GOAL_STATE.json should auto-pass");
        assert!(v.reason.contains("D5"), "reason should mention D5: {}", v.reason);
    }

    #[test]
    fn complete_transition_has_task_but_no_evidence_blocked() {
        let dir = test_dir("has-task-no-evidence");
        write_task_goal_state(&dir);
        let v = validate_transition(&dir, "test-task", TaskTransition::Complete);
        assert!(!v.passed);
        assert!(v.reason.contains("no evidence"));
    }

    #[test]
    fn complete_transition_empty_task_id_blocked() {
        let dir = test_dir("empty-id");
        let v = validate_transition(&dir, "", TaskTransition::Complete);
        assert!(!v.passed);
    }

    #[test]
    fn complete_transition_with_successful_evidence_allowed() {
        let dir = test_dir("success-evidence");
        write_task_goal_state(&dir);
        write_evidence(&dir, &[("artifact-1", true)]);
        let v = validate_transition(&dir, "test-task", TaskTransition::Complete);
        assert!(v.passed);
    }

    #[test]
    fn complete_transition_all_failed_blocked() {
        let dir = test_dir("all-failed");
        write_task_goal_state(&dir);
        write_evidence(&dir, &[("artifact-1", false), ("artifact-2", false)]);
        let v = validate_transition(&dir, "test-task", TaskTransition::Complete);
        assert!(!v.passed);
    }
}
