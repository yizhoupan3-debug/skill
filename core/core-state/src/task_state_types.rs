//! Pure type definitions for [`ResolvedTaskView`] and [`ContinuityFrame`] — no business logic,
//! no I/O. These were originally extracted to the `core-state-types` crate (L2) and later
//! re-merged into core-state (2026-06-29).
//!
//! All types here are `serde`-annotated for serialization/deserialization,
//! with `#[serde(deny_unknown_fields)]` and `#[serde(default)]` where appropriate.

use serde::{Deserialize, Serialize};

use crate::goal_prediction::GoalStatePrediction;

/// Two-pointer snapshot (active + focus) for cross-session continuity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TaskPointers {
    #[serde(default)]
    pub active_task_id: Option<String>,
    #[serde(default)]
    pub focus_task_id: Option<String>,
}

impl Default for TaskPointers {
    fn default() -> Self {
        Self {
            active_task_id: None,
            focus_task_id: None,
        }
    }
}

/// Rolled-up depth compliance metrics for a single task (RFV rounds + GOAL checkpoints + EVIDENCE).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct DepthCompliance {
    pub goal_checkpoint_count: u64,
    pub qg_adversarial_round_count: u64,
    pub qg_falsification_test_count: u64,
    pub qg_pass_round_count: u64,
    pub qg_fail_round_count: u64,
    pub qg_skipped_round_count: u64,
    pub qg_unknown_round_count: u64,
    pub qg_pass_without_evidence_count: u64,
    pub qg_external_deep_structured_round_count: u64,
    pub qg_external_strict_ok_round_count: u64,
    pub depth_score: u8,
}

/// Single hydration bundle sent to host beforeSubmit / stop.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContinuityFrame {
    /// Pointer resolution result (active/focus/override → ResolvedTaskView with optional goal_state).
    pub pointer_view: ResolvedTaskView,
    /// Goal state determined from active→focus pointer fallback.
    /// Differs from `pointer_view.goal_state` because this uses pointer fallback
    /// rather than the resolved task_id's own GOAL.
    pub hydration_goal: Option<(serde_json::Value, String)>,
}

/// Read-model for a single task — the primary output of [`ResolvedTaskView`] resolution.
///
/// Designed for host hook consumption (beforeSubmit / stop) and depth-score computation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedTaskView {
    pub schema_version: String,
    pub task_id: Option<String>,
    pub pointers: TaskPointers,
    pub goal_state: Option<serde_json::Value>,
    pub evidence: Option<EvidenceRollup>,
    pub depth_compliance: Option<DepthCompliance>,
    pub resolution_notes: Vec<String>,
}

impl ResolvedTaskView {
    pub fn task_id(&self) -> Option<&str> { self.task_id.as_deref() }
    pub fn goal_state(&self) -> Option<&serde_json::Value> { self.goal_state.as_ref() }
    pub fn depth_compliance(&self) -> Option<&DepthCompliance> { self.depth_compliance.as_ref() }
    pub fn evidence(&self) -> Option<&EvidenceRollup> { self.evidence.as_ref() }
    pub fn pointers(&self) -> &TaskPointers { &self.pointers }
    pub fn resolution_notes(&self) -> &[String] { &self.resolution_notes }
}

/// Rollup of evidence index state for a single task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRollup {
    pub task_id: String,
    pub evidence_rows_non_empty: bool,
    pub has_successful_verification: bool,
}

/// Completion gate constraints parsed from `GOAL_STATE.completion_gates`.
/// Missing / null → no gate (equivalent to `enabled: false`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct GoalCompletionGates {
    /// Master switch. When `false`, all other fields are ignored.
    #[serde(default = "default_gate_enabled")]
    pub enabled: bool,
    /// Minimum depth score required (0–3). `None` = no constraint.
    pub min_depth_score: Option<u8>,
    /// Require at least one successful (exit_code=0) EVIDENCE_INDEX row.
    #[serde(default)]
    pub require_successful_evidence_row: bool,
    /// Minimum GOAL checkpoints. `None` = no constraint.
    pub min_goal_checkpoints: Option<u64>,
    /// Block completed if `qg_pass_without_evidence_count > 0`.
    #[serde(default)]
    pub block_on_rfv_pass_without_evidence: bool,
}

const fn default_gate_enabled() -> bool {
    true
}

impl GoalCompletionGates {
    /// Returns `true` when this gate is an unconditional pass-through (enabled=false).
    pub fn is_pass_through(&self) -> bool {
        !self.enabled
    }
}

/// Parsed extra from GOAL_STATE — optional prediction for closeout dry-run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalExtraPrediction {
    pub prediction: Option<GoalStatePrediction>,
}
