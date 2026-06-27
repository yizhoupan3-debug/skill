//! Transition validation — evidence-chain anti-fraud gate (Wave 3c-i, Phase A→C).
//!
//! Validates that task transitions (especially Complete) are backed by real
//! evidence artifacts before allowing the transition. This is the "anti-fraud"
//! gate from the v10 architecture: it prevents claiming completion without
//! verifiable evidence.
//!
//! Two integration paths:
//! - **No-goal path** (`tool_task_complete`): `validate_transition()` as a
//!   **blocking gate** — returns Err if evidence is incomplete.
//! - **Goal path** (`framework_goal_drive complete`): `validate_transition()` is the
//!   **authoritative blocking gate** (Phase B closed). `compare_old_closeout_vs_new_fraud_gate()`
//!   retains as a standalone comparison utility for stdio_dispatch.

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
}

impl TransitionVerdict {
    fn allowed(reason: impl Into<String>) -> Self {
        Self { passed: true, reason: reason.into() }
    }

    fn blocked(reason: impl Into<String>) -> Self {
        Self { passed: false, reason: reason.into() }
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
        }
        TaskTransition::Fail => {
            TransitionVerdict::allowed("fail transition — no evidence required")
        }
        TaskTransition::Complete => {
            validate_complete_transition(repo_root, task_id)
        }
    }
}

fn validate_complete_transition(repo_root: &Path, task_id: &str) -> TransitionVerdict {
    let tid = task_id.trim();
    if tid.is_empty() {
        return TransitionVerdict::blocked("task_id is empty");
    }

    let (has_evidence, evidence_ok) =
        crate::state_manager::task_evidence_artifacts_summary_for_task(repo_root, tid);

    if !has_evidence {
        return TransitionVerdict::blocked(
            format!("task '{tid}' has no evidence artifacts — cannot verify completion"),
        );
    }

    if !evidence_ok {
        return TransitionVerdict::blocked(
            format!(
                "task '{tid}' has evidence artifacts but none indicate success \
                 (no exit_code=0 or success=true)"
            ),
        );
    }

    TransitionVerdict::allowed(format!(
        "task '{tid}' has valid evidence artifacts confirming completion"
    ))
}

/// Parallel validator: runs old closeout enforcement + new fraud gate, logs
/// mismatches via `tracing::warn!`.
///
/// **Informational only** — this is a side-by-side comparison utility used by
/// stdio_dispatch for diagnostic logging. The authoritative path in both
/// `tool_task_complete` and `framework_goal_drive complete` is `validate_transition()`
/// as a blocking gate (Phase B closed).
///
/// Returns both verdicts so callers can inspect them.
pub fn compare_old_closeout_vs_new_fraud_gate(
    repo_root: &Path,
    task_id: &str,
) -> CompareResult {
    // New fraud gate verdict
    let new_verdict = validate_transition(repo_root, task_id, TaskTransition::Complete);

    // Old closeout enforcement: run the closeout validation logic.
    // The old system returns a JSON response with closeout_allowed.
    let old_verdict = run_old_closeout_check(repo_root, task_id);

    // Compare and log mismatches
    if new_verdict.passed != old_verdict.closeout_allowed {
        tracing::warn!(
            task_id = %task_id,
            new_gate_passed = %new_verdict.passed,
            old_closeout_allowed = %old_verdict.closeout_allowed,
            "compare_old_closeout_vs_new_fraud_gate: MISMATCH — diagnostic only (Phase B closed)",
        );
    }

    if new_verdict.passed {
        tracing::debug!(
            task_id = %task_id,
            "compare_old_closeout_vs_new_fraud_gate: both gates agree — transition allowed",
        );
    }

    CompareResult {
        new_gate: new_verdict,
        old_closeout: old_verdict,
    }
}

/// Summary of the old closeout enforcement check.
#[derive(Debug, Clone)]
pub struct OldCloseoutSummary {
    /// Whether the old closeout enforcement allowed the transition.
    pub closeout_allowed: bool,
    /// Number of violations found by the old system.
    pub violation_count: usize,
    /// Whether the closeout record exists.
    pub record_exists: bool,
}

impl OldCloseoutSummary {
    fn allowed() -> Self {
        Self { closeout_allowed: true, violation_count: 0, record_exists: true }
    }

    fn missing_record() -> Self {
        Self { closeout_allowed: true, violation_count: 0, record_exists: false }
    }
}

/// Result of the parallel comparison.
#[derive(Debug, Clone)]
pub struct CompareResult {
    /// Verdict from the new fraud gate.
    pub new_gate: TransitionVerdict,
    /// Summary from the old closeout enforcement.
    pub old_closeout: OldCloseoutSummary,
}

/// Run the old closeout enforcement check for comparison purposes.
///
/// Reads the closeout record file and evaluates it with evidence context.
/// Falls back to "allowed" defaults when no closeout record exists (common
/// for non-closeout workflows).
fn run_old_closeout_check(repo_root: &Path, task_id: &str) -> OldCloseoutSummary {
    use crate::closeout_validation::{
        CloseoutEvidenceContext, evaluate_closeout_record_value_with_context,
    };

    let tid = task_id.trim();
    if tid.is_empty() {
        return OldCloseoutSummary::allowed();
    }

    // Build the closeout record path
    let record_path = repo_root
        .join("artifacts/current")
        .join(tid)
        .join("CLOSEOUT_RECORD.json");

    if !record_path.is_file() {
        // No closeout record — common for goal-driven tasks that don't
        // produce closeout records. Treat as "no violations" for comparison.
        return OldCloseoutSummary::missing_record();
    }

    let raw = match std::fs::read_to_string(&record_path) {
        Ok(s) => s,
        Err(_) => return OldCloseoutSummary::allowed(),
    };

    let value: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return OldCloseoutSummary::allowed(),
    };

    // Build evidence context
    let (has_evidence, evidence_ok) =
        crate::state_manager::task_evidence_artifacts_summary_for_task(repo_root, tid);

    let ctx = CloseoutEvidenceContext {
        task_id: Some(tid.to_string()),
        has_successful_verification: has_evidence && evidence_ok,
        goal_prediction: None,
    };

    match evaluate_closeout_record_value_with_context(value, &ctx) {
        Ok(resp) => {
            let allowed = resp
                .get("closeout_allowed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let violations = resp
                .get("violations")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            OldCloseoutSummary {
                closeout_allowed: allowed,
                violation_count: violations,
                record_exists: true,
            }
        }
        Err(_) => OldCloseoutSummary::allowed(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::OnceLock;

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
    fn complete_transition_no_evidence_blocked() {
        let dir = test_dir("no-evidence");
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
        write_evidence(&dir, &[("artifact-1", true)]);
        let v = validate_transition(&dir, "test-task", TaskTransition::Complete);
        assert!(v.passed);
    }

    #[test]
    fn complete_transition_all_failed_blocked() {
        let dir = test_dir("all-failed");
        write_evidence(&dir, &[("artifact-1", false), ("artifact-2", false)]);
        let v = validate_transition(&dir, "test-task", TaskTransition::Complete);
        assert!(!v.passed);
    }

    #[test]
    fn compare_old_closeout_no_record_no_panic() {
        let dir = test_dir("compare-no-record");
        // No closeout record exists — compare should not panic
        let result = compare_old_closeout_vs_new_fraud_gate(&dir, "test-task");
        assert!(result.old_closeout.record_exists == false);
        assert!(!result.new_gate.passed); // No evidence for the task
    }
}
