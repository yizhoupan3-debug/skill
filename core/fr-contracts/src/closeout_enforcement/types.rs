use serde::{Deserialize, Serialize};

use super::closeout_rule_category;

pub const CLOSEOUT_RECORD_SCHEMA_VERSION: &str = "closeout-record-v1";
pub const CLOSEOUT_ENFORCEMENT_RESPONSE_SCHEMA_VERSION: &str =
    "router-rs-closeout-enforcement-response-v1";
pub const CLOSEOUT_ENFORCEMENT_AUTHORITY: &str = "rust-closeout-enforcement";

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CloseoutCommandRecord {
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub exit_code: i64,
    #[serde(default)]
    #[serde(rename = "duration_ms")]
    pub _duration_ms: Option<i64>,
    #[serde(default)]
    #[serde(rename = "stdout_summary")]
    pub _stdout_summary: Option<String>,
    #[serde(default)]
    #[serde(rename = "stderr_summary")]
    pub _stderr_summary: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CloseoutArtifactRecord {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub exists: bool,
    #[serde(default)]
    #[serde(rename = "size_bytes")]
    pub _size_bytes: Option<i64>,
    #[serde(default)]
    #[serde(rename = "checks")]
    pub _checks: Vec<String>,
}

/// Deny unknown fields so that typos like `verification_state` (instead of
/// `verification_status`) or `commands_ran` (instead of `commands_run`) fail
/// loud with a `parse closeout record failed` error rather than being
/// silently ignored by serde defaults. The schema is closed; new fields must
/// be added in lockstep with `configs/framework/CLOSEOUT_RECORD_SCHEMA.json`.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CloseoutRecord {
    #[serde(default)]
    pub schema_version: String,
    #[serde(default)]
    pub task_id: String,
    #[serde(default)]
    #[serde(rename = "started_at")]
    pub _started_at: Option<String>,
    #[serde(default)]
    #[serde(rename = "ended_at")]
    pub _ended_at: Option<String>,
    #[serde(default)]
    pub changed_files: Vec<String>,
    #[serde(default)]
    pub commands_run: Vec<CloseoutCommandRecord>,
    #[serde(default)]
    pub artifacts_checked: Vec<CloseoutArtifactRecord>,
    #[serde(default)]
    pub verification_status: String,
    #[serde(default)]
    pub blockers: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    #[serde(rename = "notes")]
    pub _notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CloseoutViolation {
    pub rule: String,
    pub severity: String,
    pub category: String,
    pub detail: String,
}

impl CloseoutViolation {
    /// Create a violation with `category` automatically derived from the rule name.
    pub fn new(
        rule: impl Into<String>,
        severity: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        let rule = rule.into();
        let category = closeout_rule_category(&rule).to_string();
        Self {
            rule,
            severity: severity.into(),
            category,
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PredictionVerificationReport {
    pub matched: bool,
    pub rule: String,
    pub detail: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CloseoutEnforcementResponse {
    pub schema_version: String,
    pub authority: String,
    pub task_id: String,
    pub closeout_allowed: bool,
    pub can_proceed: bool,
    pub claimed_completion: bool,
    pub violations: Vec<CloseoutViolation>,
    pub missing_evidence: Vec<String>,
    pub verification_status: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub prediction_verification: Vec<PredictionVerificationReport>,
}

/// Optional context for context-aware closeout evaluation. When supplied, R8 also
/// cross-checks against `EVIDENCE_INDEX.json` rows for the task: `verification_status=passed`
/// with empty `commands_run` AND zero successful EVIDENCE_INDEX rows is blocked even when
/// `artifacts_checked` is non-empty (artifact existence ≠ executable verification).
#[derive(Debug, Clone, Default)]
pub struct CloseoutEvidenceContext {
    /// Expected task id for task-scoped evaluation.
    pub task_id: Option<String>,
    /// Whether the task's `EVIDENCE_INDEX.json` `artifacts` array is non-empty.
    /// (Reserved: future R-rules may want to flag "rows present but none successful".)
    pub _evidence_rows_non_empty: bool,
    /// Whether the task's `EVIDENCE_INDEX.json` has at least one row with
    /// `success==true` or `exit_code==0`.
    pub has_successful_verification: bool,
    /// Optional `GOAL_STATE.extra.prediction` for EV-6 dry-run verification.
    pub goal_prediction: Option<core_state_types::goal_prediction::GoalStatePrediction>,
}
