//! QGEntry — two-stage exit gate for GoalEngine.
//!
//! Implements the two-stage exit gate from the v10 roadmap (§2.2):
//!
//!   Stage 1: Anti-fraud gate — verifies evidence completeness for each task.
//!   Stage 2: Quality gate — dispatches to QG Route for scene-dispatched checker evaluation.
//!
//! Called by GoalEngine after each task_complete or goal continue operation.
//! Returns a `GateVerdict`:
//!   - passed=true  → GoalEngine stays in Active (continue loop)
//!   - passed=false → GoalEngine moves to ReviewPending (agent reviews blockers)
//!
//! Wave 5a: initial standalone trigger function. Future: will become `impl QGEntry`
//! inside GoalEngine when the struct is introduced.

use std::path::Path;

use quality_gate::types::{
    CheckContext, Finding, GateVerdict, Severity,
};

use crate::qg_route::evaluate_qg_route;

/// Trigger the two-stage exit gate.
///
/// # Arguments
/// * `repo_root` — repository root path (from task context)
/// * `task_id` — current task identifier
/// * `scene` — scene constant (from GoalEngine)
/// * `goal` — goal description string
/// * `sub_scene` — optional sub-scene for checker filtering (Wave 6)
/// * `round` — current verification round (1-based)
/// * `runtime_handle` — optional tokio runtime handle for async checkers
///
/// # Returns
/// `GateVerdict` with Stage 1 (anti-fraud) and Stage 2 (quality gate) combined:
///   - Anti-fraud blocks (P0) → immediate fail, no Stage 2
///   - Quality gate verdict from checker chain
///   - Anti-fraud informational (no task, no evidence) → passes, proceeds to Stage 2
pub fn trigger(
    repo_root: &Path,
    task_id: &str,
    scene: &str,
    goal: &str,
    sub_scene: Option<&str>,
    round: u64,
    runtime_handle: Option<tokio::runtime::Handle>,
) -> GateVerdict {
    // ═══════════════════════════════════════════════════════════════════
    // Stage 1: Anti-fraud gate (evidence chain verification)
    // ═══════════════════════════════════════════════════════════════════
    let (has_evidence, evidence_ok) =
        core_state::state_manager::task_evidence_artifacts_summary_for_task(repo_root, task_id);

    if has_evidence && !evidence_ok {
        // Evidence exists but none indicates success → block.
        return GateVerdict {
            passed: false,
            checkers_ran: 0,
            blockers: vec![Finding {
                id: "fraud_gate_evidence_incomplete".to_string(),
                severity: Severity::P0,
                description: format!(
                    "evidence exists for task '{task_id}' but none indicates success"
                ),
                location: None,
                suggestion: Some(
                    "ensure at least one evidence artifact has exit_code=0 or success=true"
                        .to_string(),
                ),
            }],
            advisories: vec![],
            reason: Some("Stage 1: anti-fraud gate — evidence incomplete".to_string()),
        };
    }

    // No evidence or all evidence OK → proceed to Stage 2.

    // ═══════════════════════════════════════════════════════════════════
    // Stage 2: Quality gate (scene-dispatched checker evaluation)
    // ═══════════════════════════════════════════════════════════════════
    let ctx = CheckContext {
        scene: scene.to_string(),
        sub_scene: sub_scene.map(|s| s.to_string()),
        goal: goal.to_string(),
        round,
        repo_root: repo_root.to_path_buf(),
        task_id: task_id.to_string(),
        evidence_path: Some(repo_root.join("EVIDENCE_INDEX.json")),
        runtime_handle,
    };

    evaluate_qg_route(scene, &ctx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;
    use crate::qg_route;

    static QG_ENTRY_TEST_INIT: OnceLock<()> = OnceLock::new();

    fn ensure_init() {
        QG_ENTRY_TEST_INIT.get_or_init(|| {
            qg_route::init_qg_route();
        });
    }

    /// Trigger with a non-existent task ID (no evidence) should pass Stage 1
    /// and proceed to Stage 2.
    #[test]
    fn trigger_no_evidence_passes_stage1() {
        ensure_init();
        let verdict = trigger(
            Path::new("/nonexistent"),
            "no-such-task",
            quality_gate::scene::GENERAL,
            "test goal",
            None,
            1,
            None,
        );
        // No evidence → Stage 1 passes (D5: "空 task list = 无欺诈可能").
        // Stage 2 runs with empty repo → checkers produce findings but are advisory.
        assert!(verdict.passed);
    }

    /// Trigger with empty scene should still evaluate.
    #[test]
    fn trigger_empty_scene_falls_back_to_general() {
        ensure_init();
        let verdict = trigger(
            Path::new("/nonexistent"),
            "no-such-task",
            "",
            "test goal",
            None,
            1,
            None,
        );
        // Empty scene normalizes to "general" via scene::normalize.
        assert!(verdict.passed);
    }

    /// Verify the two-stage structure is reflected in the return.
    #[test]
    fn trigger_returns_gate_verdict() {
        ensure_init();
        let verdict = trigger(
            Path::new("/nonexistent"),
            "no-such-task",
            quality_gate::scene::RESEARCH,
            "research goal",
            None,
            2,
            None,
        );
        // Returns proper GateVerdict with all fields populated.
        assert!(verdict.passed);
        assert!(verdict.checkers_ran > 0);
    }
}
