//! Pure type definitions for task state resolution — no business logic, no IO.

use serde::Serialize;
use serde_json::Value;

/// A continuity frame bundles a resolved pointer view with the hydration goal.
pub struct ContinuityFrame {
    pub pointer_view: ResolvedTaskView,
    pub hydration_goal: Option<(Value, String)>,
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
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
pub struct DepthCompliance {
    pub qg_pass_round_count: u64,
    pub qg_fail_round_count: u64,
    pub qg_skipped_round_count: u64,
    pub qg_unknown_round_count: u64,
    pub qg_pass_without_evidence_count: u64,
    pub qg_adversarial_round_count: u64,
    pub qg_falsification_test_count: u64,
    pub qg_external_deep_structured_round_count: u64,
    pub qg_external_strict_ok_round_count: u64,
    pub goal_checkpoint_count: u64,
    pub depth_score: u8,
}

/// High-level macro-controller mode for the resolved task id.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskControlMode {
    Idle,
    GoalDrive,
    QualityGate,
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

/// Parsed `GOAL_STATE.completion_gates` — controls hard-gate enforcement on Goal complete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GoalCompletionGates {
    pub enabled: bool,
    pub min_depth_score: Option<u8>,
    pub require_successful_evidence_row: bool,
    pub min_goal_checkpoints: Option<u64>,
    pub block_on_rfv_pass_without_evidence: bool,
}

/// Goal execution type: determines the goal lifecycle strategy.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum GoalType {
    /// Linear goal: plan → execute → review. Default when absent.
    #[default]
    Linear,
    /// Loop goal: review → implement (with task splitting), uses loop engine.
    Loop,
}

