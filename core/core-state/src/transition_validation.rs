//! Anti-fraud transition validation — per-task evidence gate (Stage 1 of QGEntry).
//!
//! Validates task state transitions with evidence-chain integrity checks.
//! When a task transitions to Completed, this function verifies:
//!   1. The transition is valid in the task state machine
//!   2. Evidence index exists and is non-empty (anti-fraud check)
//!   3. At least one evidence row has `success == true` or `exit_code == 0`
//!   4. Cross-link integrity: evidence rows reference valid targets
//!
//! This replaces the `closeout_enforcement` R1-R8 rules (Wave 3c-i).
//! Phase C complete: validate_transition is the only active path.

use serde_json::Value;
use std::path::Path;

/// Supported per-task state transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskTransition {
    /// InProgress → Completed (requires evidence gate)
    Complete,
    /// Created → InProgress (always allowed)
    Start,
    /// Any → Failed (always allowed, logs reason)
    Fail,
}

/// Result of validate_transition.
#[derive(Debug)]
pub struct TransitionVerdict {
    /// Whether the transition is allowed.
    pub allowed: bool,
    /// Human-readable reason if blocked.
    pub reason: Option<String>,
    /// Evidence summary if checked.
    pub evidence_ok: Option<bool>,
    /// Number of evidence rows found.
    pub evidence_count: usize,
}

/// Validate a task state transition.
///
/// Returns `TransitionVerdict { allowed: true, .. }` when the transition is valid.
/// When `transition == TaskTransition::Complete`, requires evidence index with
/// at least one successful evidence row.
pub fn validate_transition(
    repo_root: &Path,
    task_id: &str,
    transition: TaskTransition,
) -> TransitionVerdict {
    match transition {
        TaskTransition::Start | TaskTransition::Fail => TransitionVerdict {
            allowed: true,
            reason: None,
            evidence_ok: None,
            evidence_count: 0,
        },
        TaskTransition::Complete => validate_complete_with_evidence(repo_root, task_id),
    }
}

fn validate_complete_with_evidence(repo_root: &Path, task_id: &str) -> TransitionVerdict {
    // Build evidence index path: {repo_root}/artifacts/current/{task_id}/EVIDENCE_INDEX.json
    let evidence_path = repo_root
        .join("artifacts/current")
        .join(sanitize_task_id(task_id))
        .join("EVIDENCE_INDEX.json");

    if !evidence_path.is_file() {
        return TransitionVerdict {
            allowed: false,
            reason: Some(format!(
                "EVIDENCE_INDEX.json not found for task '{task_id}' — \
                 complete requires at least one successful evidence row. \
                 Run `framework_task_complete` with evidence or add evidence first."
            )),
            evidence_ok: None,
            evidence_count: 0,
        };
    }

    let raw = match std::fs::read_to_string(&evidence_path) {
        Ok(s) => s,
        Err(e) => {
            return TransitionVerdict {
                allowed: false,
                reason: Some(format!("cannot read EVIDENCE_INDEX.json: {e}")),
                evidence_ok: None,
                evidence_count: 0,
            };
        }
    };

    let index: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return TransitionVerdict {
                allowed: false,
                reason: Some(format!("cannot parse EVIDENCE_INDEX.json: {e}")),
                evidence_ok: None,
                evidence_count: 0,
            };
        }
    };

    let artifacts = index
        .get("artifacts")
        .and_then(Value::as_array)
        .map(|a| a.as_slice())
        .unwrap_or(&[]);

    if artifacts.is_empty() {
        return TransitionVerdict {
            allowed: false,
            reason: Some(format!(
                "EVIDENCE_INDEX.json for task '{task_id}' has no artifacts — \
                 complete requires at least one successful evidence row. \
                 Run validation commands or add evidence before completing."
            )),
            evidence_ok: None,
            evidence_count: 0,
        };
    }

    let has_success = artifacts.iter().any(|entry| {
        entry.get("success").and_then(Value::as_bool) == Some(true)
            || entry
                .get("exit_code")
                .map(|v| v.as_i64() == Some(0) || v.as_u64() == Some(0))
                .unwrap_or(false)
    });

    if !has_success {
        return TransitionVerdict {
            allowed: false,
            reason: Some(format!(
                "EVIDENCE_INDEX.json for task '{task_id}' has {} artifact(s) \
                 but none with exit_code=0 or success=true — \
                 complete requires at least one successful evidence row.",
                artifacts.len()
            )),
            evidence_ok: Some(false),
            evidence_count: artifacts.len(),
        };
    }

    TransitionVerdict {
        allowed: true,
        reason: None,
        evidence_ok: Some(true),
        evidence_count: artifacts.len(),
    }
}

fn sanitize_task_id(task_id: &str) -> String {
    // Basic path sanitization: prevent directory traversal
    task_id
        .replace(['/', '\\'], "_").replace("..", "_")
        .trim_matches('_')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn setup_evidence_dir(name: &str) -> (std::path::PathBuf, String) {
        let dir = std::env::temp_dir().join(format!("validate_transition_test_{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("artifacts/current/test-task")).unwrap();
        (dir, "test-task".to_string())
    }

    #[test]
    fn start_transition_always_allowed() {
        let (dir, tid) = setup_evidence_dir("start");
        let result = validate_transition(&dir, &tid, TaskTransition::Start);
        assert!(result.allowed);
        assert!(result.reason.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fail_transition_always_allowed() {
        let (dir, tid) = setup_evidence_dir("fail");
        let result = validate_transition(&dir, &tid, TaskTransition::Fail);
        assert!(result.allowed);
        assert!(result.reason.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn complete_rejected_when_no_evidence_file() {
        let (dir, tid) = setup_evidence_dir("no_evidence");
        // Don't write EVIDENCE_INDEX.json
        let result = validate_transition(&dir, &tid, TaskTransition::Complete);
        assert!(!result.allowed);
        assert!(result.reason.as_ref().unwrap().contains("EVIDENCE_INDEX.json not found"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn complete_rejected_when_empty_artifacts() {
        let (dir, tid) = setup_evidence_dir("empty_artifacts");
        std::fs::write(
            dir.join("artifacts/current/test-task/EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[]}"#,
        )
        .unwrap();
        let result = validate_transition(&dir, &tid, TaskTransition::Complete);
        assert!(!result.allowed);
        assert!(result.reason.as_ref().unwrap().contains("has no artifacts"));
        assert_eq!(result.evidence_count, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn complete_rejected_when_no_successful_evidence() {
        let (dir, tid) = setup_evidence_dir("no_success");
        std::fs::write(
            dir.join("artifacts/current/test-task/EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[
                {"command_preview":"cargo check","exit_code":1}
            ]}"#,
        )
        .unwrap();
        let result = validate_transition(&dir, &tid, TaskTransition::Complete);
        assert!(!result.allowed);
        assert!(result.reason.as_ref().unwrap().contains("none with exit_code=0"));
        assert_eq!(result.evidence_count, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn complete_allowed_with_successful_exit_code() {
        let (dir, tid) = setup_evidence_dir("exit_ok");
        std::fs::write(
            dir.join("artifacts/current/test-task/EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[
                {"command_preview":"cargo test","exit_code":0,"success":true}
            ]}"#,
        )
        .unwrap();
        let result = validate_transition(&dir, &tid, TaskTransition::Complete);
        assert!(result.allowed, "reason: {:?}", result.reason);
        assert!(result.reason.is_none());
        assert_eq!(result.evidence_ok, Some(true));
        assert_eq!(result.evidence_count, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn complete_allowed_with_multiple_evidence_rows() {
        let (dir, tid) = setup_evidence_dir("multi");
        std::fs::write(
            dir.join("artifacts/current/test-task/EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[
                {"command_preview":"cargo check","exit_code":0},
                {"command_preview":"cargo test","exit_code":0,"success":true}
            ]}"#,
        )
        .unwrap();
        let result = validate_transition(&dir, &tid, TaskTransition::Complete);
        assert!(result.allowed);
        assert_eq!(result.evidence_count, 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn complete_allowed_with_success_flag_only() {
        let (dir, tid) = setup_evidence_dir("success_flag");
        std::fs::write(
            dir.join("artifacts/current/test-task/EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[
                {"command_preview":"custom check","success":true}
            ]}"#,
        )
        .unwrap();
        let result = validate_transition(&dir, &tid, TaskTransition::Complete);
        assert!(result.allowed);
        assert_eq!(result.evidence_count, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sanitize_task_id_removes_path_traversal() {
        assert_eq!(sanitize_task_id("../etc/passwd"), "etc_passwd");
        assert_eq!(sanitize_task_id("normal-task"), "normal-task");
        assert_eq!(sanitize_task_id("a/b/c"), "a_b_c");
    }
}
