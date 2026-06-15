use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::debug;

pub const CLOSEOUT_RECORD_SCHEMA_VERSION: &str = "closeout-record-v1";
pub const CLOSEOUT_ENFORCEMENT_RESPONSE_SCHEMA_VERSION: &str =
    "router-rs-closeout-enforcement-response-v1";
pub const CLOSEOUT_ENFORCEMENT_AUTHORITY: &str = "rust-closeout-enforcement";

const ALLOWED_VERIFICATION_STATUSES: &[&str] = &["passed", "failed", "partial", "not_run"];

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CloseoutCommandRecord {
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub exit_code: i64,
    #[serde(default)]
    #[allow(dead_code)] // CLOSEOUT_RECORD schema - reserved for future use.
    pub duration_ms: Option<i64>,
    #[serde(default)]
    #[allow(dead_code)] // CLOSEOUT_RECORD schema - reserved for future use.
    pub stdout_summary: Option<String>,
    #[serde(default)]
    #[allow(dead_code)] // CLOSEOUT_RECORD schema - reserved for future use.
    pub stderr_summary: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CloseoutArtifactRecord {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub exists: bool,
    #[serde(default)]
    #[allow(dead_code)] // CLOSEOUT_RECORD schema - reserved for future use.
    pub size_bytes: Option<i64>,
    #[serde(default)]
    #[allow(dead_code)] // CLOSEOUT_RECORD schema - reserved for future use.
    pub checks: Vec<String>,
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
    #[allow(dead_code)] // CLOSEOUT_RECORD schema - reserved for future use.
    pub started_at: Option<String>,
    #[serde(default)]
    #[allow(dead_code)] // CLOSEOUT_RECORD schema - reserved for future use.
    pub ended_at: Option<String>,
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
    #[allow(dead_code)] // CLOSEOUT_RECORD schema - reserved for future use.
    pub notes: Option<String>,
}

/// Classify a rule name as "hard" (must fix before complete) or "soft" (advisory).
/// Returns `"hard"` by default for unknown rules (fail-safe).
fn closeout_rule_category(rule: &str) -> &'static str {
    match rule {
        // hard: structural/schema errors that make the record unreliable
        "schema_version_mismatch"
        | "task_id_missing"
        | "summary_missing"
        | "verification_status_missing"
        | "verification_status_invalid"
        | "task_id_context_mismatch"
        | "parse_error"
        | "invalid_command_evidence" => "hard",
        // soft: advisory — evidence/consistency warnings
        "claimed_done_without_evidence"
        | "changed_files_without_command_or_risk"
        | "verification_passed_with_failed_command"
        | "verification_passed_with_missing_artifact"
        | "not_run_without_blockers_or_risks"
        | "claimed_done_with_failed_verification"
        | "claimed_passed_without_evidence"
        | "claimed_passed_without_evidence_index_rows" => "soft",
        // Prediction verification rules are always advisory (warn-level).
        "prediction_verification_status_mismatch"
        | "prediction_hypothesis_not_reflected"
        | "prediction_verification_status_match"
        | "prediction_hypothesis_reflected" => "soft",
        // Unknown rule: fail-safe to hard.
        _ => "hard",
    }
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

#[tracing::instrument(level = "debug", skip_all)]
pub fn evaluate_closeout_record_value(payload: Value) -> Result<Value, String> {
    // Check raw shape violations FIRST: critical issues like missing schema_version
    // are more actionable than serde's deny_unknown_fields errors, which can mask
    // the real problem when the record shape is fundamentally broken.
    let raw_shape_violations = raw_closeout_record_shape_violations(&payload, None);
    if raw_shape_violations.iter().any(|v| v.severity == "block") {
        let mut response = CloseoutEnforcementResponse {
            schema_version: CLOSEOUT_ENFORCEMENT_RESPONSE_SCHEMA_VERSION.to_string(),
            authority: CLOSEOUT_ENFORCEMENT_AUTHORITY.to_string(),
            task_id: payload
                .get("task_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            closeout_allowed: false,
            can_proceed: false,
            claimed_completion: false,
            violations: Vec::new(),
            missing_evidence: Vec::new(),
            verification_status: String::new(),
            prediction_verification: Vec::new(),
        };
        append_closeout_violations(&mut response, raw_shape_violations);
        return serde_json::to_value(response)
            .map_err(|err| format!("serialize closeout response: {err}"));
    }
    match serde_json::from_value::<CloseoutRecord>(payload.clone()) {
        Ok(record) => {
            let mut response = evaluate_closeout_record(&record);
            append_closeout_violations(&mut response, raw_shape_violations);
            serde_json::to_value(response)
                .map_err(|err| format!("serialize closeout response: {err}"))
        }
        Err(err) => {
            let mut response = CloseoutEnforcementResponse {
                schema_version: CLOSEOUT_ENFORCEMENT_RESPONSE_SCHEMA_VERSION.to_string(),
                authority: CLOSEOUT_ENFORCEMENT_AUTHORITY.to_string(),
                task_id: payload
                    .get("task_id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                closeout_allowed: false,
                can_proceed: false,
                claimed_completion: false,
                violations: Vec::new(),
                missing_evidence: Vec::new(),
                verification_status: String::new(),
                prediction_verification: Vec::new(),
            };
            append_closeout_violations(&mut response, raw_shape_violations);
            response.violations.push(CloseoutViolation::new(
                "parse_error",
                "block",
                format!("parse closeout record failed: {err}"),
            ));
            serde_json::to_value(response)
                .map_err(|err| format!("serialize closeout response: {err}"))
        }
    }
}

pub fn evaluate_closeout_record(record: &CloseoutRecord) -> CloseoutEnforcementResponse {
    let mut violations: Vec<CloseoutViolation> = Vec::new();
    let mut missing: Vec<String> = Vec::new();

    // 0. schema_version sanity (block: refuse evaluation of missing or unknown shape).
    if record.schema_version.trim().is_empty()
        || record.schema_version != CLOSEOUT_RECORD_SCHEMA_VERSION
    {
        violations.push(CloseoutViolation::new(
            "schema_version_mismatch",
            "block",
            format!(
                "expected schema_version={CLOSEOUT_RECORD_SCHEMA_VERSION}, got {:?}",
                record.schema_version
            ),
        ));
    }

    if record.task_id.trim().is_empty() {
        violations.push(CloseoutViolation::new(
            "task_id_missing",
            "block",
            "task_id must be non-empty",
        ));
        missing.push("task_id".to_string());
    }

    let summary_trimmed = record.summary.trim();
    if summary_trimmed.is_empty() {
        violations.push(CloseoutViolation::new(
            "summary_missing",
            "block",
            "summary must be non-empty",
        ));
        missing.push("summary".to_string());
    }

    let claimed_completion = summary_claims_completion(summary_trimmed);

    let status_lower = record.verification_status.trim().to_ascii_lowercase();
    let status_recognized = ALLOWED_VERIFICATION_STATUSES.contains(&status_lower.as_str());
    if !status_lower.is_empty() && !status_recognized {
        violations.push(CloseoutViolation::new(
            "verification_status_invalid",
            "block",
            format!(
                "verification_status must be one of {:?}, got {:?}",
                ALLOWED_VERIFICATION_STATUSES, record.verification_status
            ),
        ));
    }
    if status_lower.is_empty() {
        violations.push(CloseoutViolation::new(
            "verification_status_missing",
            "block",
            "verification_status must be one of passed|failed|partial|not_run",
        ));
        missing.push("verification_status".to_string());
    }

    // R1: claimed completion but no evidence and no acknowledged risk.
    if claimed_completion
        && status_lower == "not_run"
        && record.risks.is_empty()
        && record.blockers.is_empty()
    {
        violations.push(CloseoutViolation::new(
            "claimed_done_without_evidence",
            "block",
            "summary claims completion but verification_status=not_run with no risks or blockers",
        ));
        missing.push("validation_command_or_risk_acknowledgement".to_string());
    }

    // R2: changed files but no commands_run and no risks recorded.
    if !record.changed_files.is_empty() && record.commands_run.is_empty() && record.risks.is_empty()
    {
        violations.push(CloseoutViolation::new(
            "changed_files_without_command_or_risk",
            "block",
            format!(
                "{} changed file(s) recorded but no commands_run and no risks declared",
                record.changed_files.len()
            ),
        ));
        missing.push("validation_command".to_string());
    }

    // R3: verification_status=passed but a command failed.
    if status_lower == "passed" {
        if let Some(failed) = record.commands_run.iter().find(|c| c.exit_code != 0) {
            violations.push(CloseoutViolation::new(
                "verification_passed_with_failed_command",
                "block",
                format!(
                    "verification_status=passed but command exited {}: {}",
                    failed.exit_code, failed.command
                ),
            ));
        }
    }

    // R3b: command evidence must be auditable; serde defaults must not turn `{}` into success.
    if let Some(invalid) = record
        .commands_run
        .iter()
        .find(|c| c.command.trim().is_empty())
    {
        violations.push(CloseoutViolation::new(
            "invalid_command_evidence",
            "block",
            format!(
                "commands_run contains a row without a non-empty command; exit_code={}",
                invalid.exit_code
            ),
        ));
        missing.push("command".to_string());
    }

    // R4: verification_status=passed but artifact missing.
    if status_lower == "passed" {
        if let Some(missing_artifact) = record.artifacts_checked.iter().find(|a| !a.exists) {
            violations.push(CloseoutViolation::new(
                "verification_passed_with_missing_artifact",
                "block",
                format!(
                    "verification_status=passed but artifact does not exist: {}",
                    missing_artifact.path
                ),
            ));
        }
    }

    // R5: not_run without blockers or risks.
    if status_lower == "not_run" && record.blockers.is_empty() && record.risks.is_empty() {
        // Only emit if not already covered by R1.
        let already_covered = violations
            .iter()
            .any(|v| v.rule == "claimed_done_without_evidence");
        if !already_covered {
            violations.push(CloseoutViolation::new(
                "not_run_without_blockers_or_risks",
                "block",
                "verification_status=not_run requires at least one blocker or risk",
            ));
            missing.push("blocker_or_risk".to_string());
        }
    }

    // R6: failed verification but summary still claims completion without acknowledged risks.
    if status_lower == "failed"
        && claimed_completion
        && record.risks.is_empty()
        && record.blockers.is_empty()
    {
        violations.push(CloseoutViolation::new(
            "claimed_done_with_failed_verification",
            "block",
            "summary claims completion but verification_status=failed without recorded risks or blockers",
        ));
    }

    // TODO: task-scoped depth / `GOAL_STATE.completion_gates` alignment
    // Phase 3 pointer consolidation (3B/3C) completed 2026-06-02.
    // Next: re-evaluate closeout vs completion_gates alignment — state model simplified
    //   (5 files → 2 control-plane anchors), closeout gate should align with
    //   GOAL_STATE.completion_gates.min_depth_score against depth_compliance_aggregate output.

    // R7 (depth review P0-B): verification_status=passed but record carries no command evidence
    // and the optional EvidenceContext (when supplied by orchestrator) shows no successful
    // EVIDENCE_INDEX rows either. Pure self-attestation should not be enough to claim "passed".
    // The context-aware overload `evaluate_closeout_record_with_context` enforces this; here we
    // emit only the record-internal half so the rule is documented and `commands_run`-empty
    // claims at least surface a violation when no risks are acknowledged.
    if status_lower == "passed"
        && record.commands_run.is_empty()
        && record.artifacts_checked.is_empty()
        && record.risks.is_empty()
        && record.blockers.is_empty()
    {
        violations.push(CloseoutViolation::new(
            "claimed_passed_without_evidence",
            "block",
            "verification_status=passed but commands_run/artifacts_checked/risks/blockers all empty — supply at least one command, artifact check, risk, or blocker to back the claim",
        ));
        missing.push("evidence_or_acknowledgement".to_string());
    }

    let blocking = violations.iter().any(|v| v.severity == "block");
    let has_hard_blocker = violations.iter().any(|v| v.category == "hard");

    CloseoutEnforcementResponse {
        schema_version: CLOSEOUT_ENFORCEMENT_RESPONSE_SCHEMA_VERSION.to_string(),
        authority: CLOSEOUT_ENFORCEMENT_AUTHORITY.to_string(),
        task_id: record.task_id.clone(),
        closeout_allowed: !blocking,
        can_proceed: !has_hard_blocker,
        claimed_completion,
        violations,
        missing_evidence: missing,
        verification_status: status_lower,
        prediction_verification: Vec::new(),
    }
}

pub fn closeout_enforcement_contract() -> Value {
    json!({
        "schema_version": CLOSEOUT_ENFORCEMENT_RESPONSE_SCHEMA_VERSION,
        "authority": CLOSEOUT_ENFORCEMENT_AUTHORITY,
        "record_schema_version": CLOSEOUT_RECORD_SCHEMA_VERSION,
        "allowed_verification_statuses": ALLOWED_VERIFICATION_STATUSES,
        "completion_keywords": crate::hook_common::completion_claim_keywords_export(),
        "rules": [
            "schema_version_mismatch",
            "task_id_context_mismatch",
            "task_id_missing",
            "summary_missing",
            "verification_status_missing",
            "verification_status_invalid",
            "claimed_done_without_evidence",
            "changed_files_without_command_or_risk",
            "verification_passed_with_failed_command",
            "invalid_command_evidence",
            "verification_passed_with_missing_artifact",
            "not_run_without_blockers_or_risks",
            "claimed_done_with_failed_verification",
            "claimed_passed_without_evidence",
            "claimed_passed_without_evidence_index_rows",
            "prediction_verification_status_mismatch",
            "prediction_hypothesis_not_reflected",
            "parse_error"
        ],
        "rule_categories": {
            "hard": [
                "schema_version_mismatch",
                "task_id_missing",
                "summary_missing",
                "verification_status_missing",
                "verification_status_invalid",
                "task_id_context_mismatch",
                "parse_error",
                "invalid_command_evidence"
            ],
            "soft": [
                "claimed_done_without_evidence",
                "changed_files_without_command_or_risk",
                "verification_passed_with_failed_command",
                "verification_passed_with_missing_artifact",
                "not_run_without_blockers_or_risks",
                "claimed_done_with_failed_verification",
                "claimed_passed_without_evidence",
                "claimed_passed_without_evidence_index_rows"
            ]
        },
        "prediction_verification_rules": [
            "prediction_verification_status_match",
            "prediction_verification_status_mismatch",
            "prediction_hypothesis_reflected",
            "prediction_hypothesis_not_reflected"
        ]
    })
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
    #[allow(dead_code)] // CLOSEOUT_RECORD schema - reserved for future use.
    pub evidence_rows_non_empty: bool,
    /// Whether the task's `EVIDENCE_INDEX.json` has at least one row with
    /// `success==true` or `exit_code==0`.
    pub has_successful_verification: bool,
    /// Optional `GOAL_STATE.extra.prediction` for EV-6 dry-run verification.
    pub goal_prediction: Option<core_state::goal_prediction::GoalStatePrediction>,
}

/// Like [`evaluate_closeout_record`] but also runs R8 against an external evidence rollup.
pub fn evaluate_closeout_record_with_context(
    record: &CloseoutRecord,
    ctx: &CloseoutEvidenceContext,
) -> CloseoutEnforcementResponse {
    let mut response = evaluate_closeout_record(record);
    if let Some(expected) = ctx
        .task_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if record.task_id.trim() != expected {
            response.violations.push(CloseoutViolation::new(
                "task_id_context_mismatch",
                "block",
                format!(
                    "closeout record task_id {:?} does not match evaluation context {:?}",
                    record.task_id, expected
                ),
            ));
            response
                .missing_evidence
                .push("matching_task_id".to_string());
            response.closeout_allowed = false;
        }
    }
    let status_lower = record.verification_status.trim().to_ascii_lowercase();
    if status_lower == "passed"
        && record.commands_run.is_empty()
        && !ctx.has_successful_verification
        && !response
            .violations
            .iter()
            .any(|v| v.rule == "claimed_passed_without_evidence")
    {
        response.violations.push(CloseoutViolation::new(
            "claimed_passed_without_evidence_index_rows",
            "block",
            "verification_status=passed and commands_run is empty, and EVIDENCE_INDEX.json has no successful rows — record at least one verifier command (or run a verifier so PostTool hooks append to EVIDENCE_INDEX)",
        ));
        response
            .missing_evidence
            .push("evidence_index_successful_row".to_string());
        response.closeout_allowed = false;
    }
    if let Some(prediction) = ctx.goal_prediction.as_ref() {
        append_prediction_verification(
            &mut response,
            prediction,
            &record.verification_status,
            &record.summary,
        );
    }

    // Recompute can_proceed after all violations are collected.
    let has_hard_blocker = response.violations.iter().any(|v| v.category == "hard");
    response.can_proceed = !has_hard_blocker;

    response
}

fn append_prediction_verification(
    response: &mut CloseoutEnforcementResponse,
    prediction: &core_state::goal_prediction::GoalStatePrediction,
    verification_status: &str,
    summary: &str,
) {
    let checks = core_state::goal_prediction::verify_prediction_against_closeout(
        prediction,
        verification_status,
        summary,
    );
    for check in &checks {
        if check.severity == "warn" {
            response.violations.push(CloseoutViolation::new(
                &check.rule,
                &check.severity,
                &check.detail,
            ));
        }
        response
            .prediction_verification
            .push(PredictionVerificationReport {
                matched: check.matched,
                rule: check.rule.clone(),
                detail: check.detail.clone(),
                severity: check.severity.clone(),
            });
    }
    if !checks.is_empty() {
        crate::telemetry_emit::emit_prediction_outcome(
            &response.task_id,
            prediction,
            verification_status,
            &checks,
        );
    }
}

/// Convenience JSON wrapper mirroring [`evaluate_closeout_record_value`] but with context.
pub fn evaluate_closeout_record_value_with_context(
    payload: Value,
    ctx: &CloseoutEvidenceContext,
) -> Result<Value, String> {
    // Check raw shape violations FIRST (same rationale as evaluate_closeout_record_value).
    let raw_shape_violations =
        raw_closeout_record_shape_violations(&payload, ctx.task_id.as_deref());
    if raw_shape_violations.iter().any(|v| v.severity == "block") {
        let mut response = CloseoutEnforcementResponse {
            schema_version: CLOSEOUT_ENFORCEMENT_RESPONSE_SCHEMA_VERSION.to_string(),
            authority: CLOSEOUT_ENFORCEMENT_AUTHORITY.to_string(),
            task_id: payload
                .get("task_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            closeout_allowed: false,
            can_proceed: false,
            claimed_completion: false,
            violations: Vec::new(),
            missing_evidence: Vec::new(),
            verification_status: String::new(),
            prediction_verification: Vec::new(),
        };
        append_closeout_violations(&mut response, raw_shape_violations);
        return serde_json::to_value(response)
            .map_err(|err| format!("serialize closeout response: {err}"));
    }
    match serde_json::from_value::<CloseoutRecord>(payload.clone()) {
        Ok(record) => {
            let mut response = evaluate_closeout_record_with_context(&record, ctx);
            append_closeout_violations(&mut response, raw_shape_violations);
            serde_json::to_value(response)
                .map_err(|err| format!("serialize closeout response: {err}"))
        }
        Err(err) => {
            let mut response = CloseoutEnforcementResponse {
                schema_version: CLOSEOUT_ENFORCEMENT_RESPONSE_SCHEMA_VERSION.to_string(),
                authority: CLOSEOUT_ENFORCEMENT_AUTHORITY.to_string(),
                task_id: payload
                    .get("task_id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                closeout_allowed: false,
                can_proceed: false,
                claimed_completion: false,
                violations: Vec::new(),
                missing_evidence: Vec::new(),
                verification_status: String::new(),
                prediction_verification: Vec::new(),
            };
            append_closeout_violations(&mut response, raw_shape_violations);
            response.violations.push(CloseoutViolation::new(
                "parse_error",
                "block",
                format!("parse closeout record failed: {err}"),
            ));
            serde_json::to_value(response)
                .map_err(|err| format!("serialize closeout response: {err}"))
        }
    }
}

fn raw_closeout_record_shape_violations(
    payload: &Value,
    expected_task_id: Option<&str>,
) -> Vec<CloseoutViolation> {
    let mut violations = Vec::new();
    if payload
        .get("schema_version")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        violations.push(CloseoutViolation::new(
            "schema_version_mismatch",
            "block",
            format!("expected schema_version={CLOSEOUT_RECORD_SCHEMA_VERSION}, got missing or empty value"),
        ));
    }
    if let Some(expected) = expected_task_id.map(str::trim).filter(|s| !s.is_empty()) {
        let actual = payload
            .get("task_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        if actual != expected {
            violations.push(CloseoutViolation::new(
                "task_id_context_mismatch",
                "block",
                format!("closeout record task_id {actual:?} does not match evaluation context {expected:?}"),
            ));
        }
    }
    if let Some(commands) = payload.get("commands_run").and_then(Value::as_array) {
        for (idx, command) in commands.iter().enumerate() {
            let command_text = command
                .get("command")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or("");
            let has_exit_code = command.get("exit_code").and_then(Value::as_i64).is_some();
            if command_text.is_empty() || !has_exit_code {
                violations.push(CloseoutViolation::new(
                    "invalid_command_evidence",
                    "block",
                    format!(
                        "commands_run[{idx}] must include non-empty command and integer exit_code"
                    ),
                ));
            }
        }
    }
    violations
}

fn append_closeout_violations(
    response: &mut CloseoutEnforcementResponse,
    violations: Vec<CloseoutViolation>,
) {
    for violation in violations {
        if response
            .violations
            .iter()
            .any(|existing| existing.rule == violation.rule && existing.detail == violation.detail)
        {
            continue;
        }
        if violation.severity == "block" {
            response.closeout_allowed = false;
        }
        response.violations.push(violation);
    }
}

fn summary_claims_completion(summary: &str) -> bool {
    crate::hook_common::contains_completion_claim_token(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record_with(summary: &str, status: &str) -> CloseoutRecord {
        CloseoutRecord {
            schema_version: CLOSEOUT_RECORD_SCHEMA_VERSION.to_string(),
            task_id: "t-1".to_string(),
            summary: summary.to_string(),
            verification_status: status.to_string(),
            ..Default::default()
        }
    }

    fn has_rule(resp: &CloseoutEnforcementResponse, rule: &str) -> bool {
        resp.violations.iter().any(|v| v.rule == rule)
    }

    #[test]
    fn passed_clean_record_is_allowed() {
        let mut record = record_with("已完成 deck rebuild and verified output", "passed");
        record.commands_run.push(CloseoutCommandRecord {
            command: "python build_deck.py".to_string(),
            exit_code: 0,
            ..Default::default()
        });
        record.artifacts_checked.push(CloseoutArtifactRecord {
            path: "ppt/deck_v3.pptx".to_string(),
            exists: true,
            ..Default::default()
        });
        record.changed_files.push("ppt/build_deck.py".to_string());
        let resp = evaluate_closeout_record(&record);
        assert!(
            resp.closeout_allowed,
            "expected allowed, got {:?}",
            resp.violations
        );
        assert!(resp.claimed_completion);
    }

    #[test]
    fn claimed_done_without_evidence_is_blocked() {
        let record = record_with("已完成", "not_run");
        let resp = evaluate_closeout_record(&record);
        assert!(!resp.closeout_allowed);
        assert!(has_rule(&resp, "claimed_done_without_evidence"));
    }

    #[test]
    fn changed_files_without_command_or_risk_is_blocked() {
        let mut record = record_with("Refactored builder", "partial");
        record.changed_files.push("ppt/build_deck.py".to_string());
        let resp = evaluate_closeout_record(&record);
        assert!(!resp.closeout_allowed);
        assert!(has_rule(&resp, "changed_files_without_command_or_risk"));
    }

    #[test]
    fn changed_files_with_risk_is_allowed() {
        let mut record = record_with("Refactored builder; tests not run", "partial");
        record.changed_files.push("ppt/build_deck.py".to_string());
        record
            .risks
            .push("did not execute python build_deck.py because PIL missing".to_string());
        let resp = evaluate_closeout_record(&record);
        assert!(
            resp.closeout_allowed,
            "expected allowed, violations: {:?}",
            resp.violations
        );
    }

    #[test]
    fn verification_passed_with_failed_command_is_blocked() {
        let mut record = record_with("done", "passed");
        record.commands_run.push(CloseoutCommandRecord {
            command: "pytest".to_string(),
            exit_code: 1,
            ..Default::default()
        });
        let resp = evaluate_closeout_record(&record);
        assert!(!resp.closeout_allowed);
        assert!(has_rule(&resp, "verification_passed_with_failed_command"));
    }

    #[test]
    fn verification_passed_with_missing_artifact_is_blocked() {
        let mut record = record_with("done", "passed");
        record.artifacts_checked.push(CloseoutArtifactRecord {
            path: "ppt/deck_v3.pptx".to_string(),
            exists: false,
            ..Default::default()
        });
        let resp = evaluate_closeout_record(&record);
        assert!(!resp.closeout_allowed);
        assert!(has_rule(&resp, "verification_passed_with_missing_artifact"));
    }

    #[test]
    fn not_run_without_blockers_is_blocked() {
        let record = record_with("Investigating but not yet done", "not_run");
        let resp = evaluate_closeout_record(&record);
        assert!(!resp.closeout_allowed);
        assert!(
            has_rule(&resp, "not_run_without_blockers_or_risks")
                || has_rule(&resp, "claimed_done_without_evidence")
        );
    }

    #[test]
    fn not_run_with_blocker_is_allowed() {
        let mut record = record_with("Paused awaiting user", "not_run");
        record
            .blockers
            .push("Need user to approve schema migration".to_string());
        let resp = evaluate_closeout_record(&record);
        assert!(resp.closeout_allowed, "violations: {:?}", resp.violations);
    }

    #[test]
    fn failed_status_with_done_summary_is_blocked() {
        let mut record = record_with("已完成", "failed");
        // No risks/blockers.
        record.commands_run.push(CloseoutCommandRecord {
            command: "pytest".to_string(),
            exit_code: 2,
            ..Default::default()
        });
        let resp = evaluate_closeout_record(&record);
        assert!(!resp.closeout_allowed);
        assert!(has_rule(&resp, "claimed_done_with_failed_verification"));
    }

    #[test]
    fn empty_summary_and_status_blocks() {
        let record = CloseoutRecord {
            schema_version: CLOSEOUT_RECORD_SCHEMA_VERSION.to_string(),
            task_id: "t-empty".to_string(),
            ..Default::default()
        };
        let resp = evaluate_closeout_record(&record);
        assert!(!resp.closeout_allowed);
        assert!(has_rule(&resp, "summary_missing"));
        assert!(has_rule(&resp, "verification_status_missing"));
    }

    #[test]
    fn schema_version_mismatch_blocks() {
        let record = CloseoutRecord {
            schema_version: "wrong-v0".to_string(),
            task_id: "t".to_string(),
            summary: "ok".to_string(),
            verification_status: "partial".to_string(),
            ..Default::default()
        };
        let resp = evaluate_closeout_record(&record);
        assert!(!resp.closeout_allowed);
        assert!(has_rule(&resp, "schema_version_mismatch"));
    }

    #[test]
    fn schema_version_missing_blocks_value_evaluator() {
        let response = evaluate_closeout_record_value(json!({
            "task_id": "t",
            "summary": "ok",
            "verification_status": "partial"
        }))
        .expect("evaluate closeout");
        assert_eq!(response["closeout_allowed"], json!(false));
        assert!(
            response["violations"]
                .as_array()
                .expect("violations")
                .iter()
                .any(|v| v["rule"] == "schema_version_mismatch")
        );
    }

    #[test]
    fn empty_command_record_does_not_count_as_success_evidence() {
        let response = evaluate_closeout_record_value(json!({
            "schema_version": CLOSEOUT_RECORD_SCHEMA_VERSION,
            "task_id": "t",
            "summary": "done",
            "verification_status": "passed",
            "commands_run": [{}]
        }))
        .expect("evaluate closeout");
        assert_eq!(response["closeout_allowed"], json!(false));
        assert!(
            response["violations"]
                .as_array()
                .expect("violations")
                .iter()
                .any(|v| v["rule"] == "invalid_command_evidence")
        );
    }

    #[test]
    fn context_task_id_mismatch_blocks_value_evaluator() {
        let ctx = CloseoutEvidenceContext {
            task_id: Some("expected-task".to_string()),
            evidence_rows_non_empty: true,
            has_successful_verification: true,
            ..Default::default()
        };
        let response = evaluate_closeout_record_value_with_context(
            json!({
                "schema_version": CLOSEOUT_RECORD_SCHEMA_VERSION,
                "task_id": "other-task",
                "summary": "done",
                "verification_status": "passed",
                "commands_run": [{"command": "cargo test", "exit_code": 0}]
            }),
            &ctx,
        )
        .expect("evaluate closeout");
        assert_eq!(response["closeout_allowed"], json!(false));
        assert!(
            response["violations"]
                .as_array()
                .expect("violations")
                .iter()
                .any(|v| v["rule"] == "task_id_context_mismatch")
        );
    }

    #[test]
    fn invalid_status_is_blocked() {
        let record = record_with("ok", "maybe");
        let resp = evaluate_closeout_record(&record);
        assert!(!resp.closeout_allowed);
        assert!(has_rule(&resp, "verification_status_invalid"));
    }

    #[test]
    fn contract_payload_lists_rules() {
        let payload = closeout_enforcement_contract();
        let rules = payload["rules"].as_array().expect("rules array");
        assert!(rules.iter().any(|v| v == "claimed_done_without_evidence"));
        assert!(
            rules
                .iter()
                .any(|v| v == "verification_passed_with_missing_artifact")
        );
        assert!(rules.iter().any(|v| v == "claimed_passed_without_evidence"));
        assert!(
            rules
                .iter()
                .any(|v| v == "claimed_passed_without_evidence_index_rows")
        );
        assert_eq!(
            payload["record_schema_version"],
            CLOSEOUT_RECORD_SCHEMA_VERSION
        );
    }

    /// P0-B / R7: passed + nothing else recorded → block.
    #[test]
    fn passed_with_no_evidence_or_acknowledgement_is_blocked() {
        let record = record_with("已完成 verification skipped", "passed");
        let resp = evaluate_closeout_record(&record);
        assert!(!resp.closeout_allowed, "violations: {:?}", resp.violations);
        assert!(has_rule(&resp, "claimed_passed_without_evidence"));
    }

    /// R7 must not fire when an artifact-existence check is recorded (R4 still owns missing-artifact case).
    #[test]
    fn passed_with_only_artifact_check_is_allowed_by_r7() {
        let mut record = record_with("done", "passed");
        record.artifacts_checked.push(CloseoutArtifactRecord {
            path: "out/release.tar.gz".to_string(),
            exists: true,
            ..Default::default()
        });
        let resp = evaluate_closeout_record(&record);
        // R7 scope = empty {commands_run, artifacts_checked, risks, blockers}; artifact present → R7 silent.
        assert!(!has_rule(&resp, "claimed_passed_without_evidence"));
    }

    /// R8: passed + commands_run empty + EVIDENCE rollup empty → block by context-aware path.
    #[test]
    fn r8_blocks_passed_when_evidence_rollup_empty() {
        let mut record = record_with("done", "passed");
        // Acknowledge a risk so R7 stays silent and we isolate R8.
        record
            .risks
            .push("did not run verifier locally".to_string());
        let ctx = CloseoutEvidenceContext {
            task_id: Some(record.task_id.clone()),
            evidence_rows_non_empty: false,
            has_successful_verification: false,
            ..Default::default()
        };
        let resp = evaluate_closeout_record_with_context(&record, &ctx);
        assert!(!resp.closeout_allowed, "violations: {:?}", resp.violations);
        assert!(has_rule(
            &resp,
            "claimed_passed_without_evidence_index_rows"
        ));
    }

    #[test]
    fn prediction_mismatch_is_warn_not_block() {
        use core_state::goal_prediction::GoalStatePrediction;
        let mut record = record_with("已完成 router-rs green", "failed");
        record.commands_run.push(CloseoutCommandRecord {
            command: "cargo test -p router-rs".to_string(),
            exit_code: 1,
            ..Default::default()
        });
        record.risks.push("tests failed unexpectedly".to_string());
        let ctx = CloseoutEvidenceContext {
            goal_prediction: Some(GoalStatePrediction {
                expected_verification_status: Some("passed".to_string()),
                hypothesis: Some("router-rs green".to_string()),
            }),
            ..Default::default()
        };
        let resp = evaluate_closeout_record_with_context(&record, &ctx);
        assert!(
            resp.closeout_allowed,
            "prediction dry-run must not block closeout: {:?}",
            resp.violations
        );
        assert!(has_rule(&resp, "prediction_verification_status_mismatch"));
        assert!(
            resp.prediction_verification
                .iter()
                .any(|p| p.rule == "prediction_hypothesis_reflected")
        );
    }

    /// R8 silent when EVIDENCE rollup has at least one successful row.
    #[test]
    fn r8_allows_passed_when_evidence_has_successful_row() {
        let mut record = record_with("done", "passed");
        record.risks.push(
            "commands_run intentionally empty; relying on hook-appended evidence".to_string(),
        );
        let ctx = CloseoutEvidenceContext {
            task_id: Some(record.task_id.clone()),
            evidence_rows_non_empty: true,
            has_successful_verification: true,
            ..Default::default()
        };
        let resp = evaluate_closeout_record_with_context(&record, &ctx);
        assert!(resp.closeout_allowed, "violations: {:?}", resp.violations);
        assert!(!has_rule(
            &resp,
            "claimed_passed_without_evidence_index_rows"
        ));
    }

    /// P0-F-style invariant: typo'd field rejected by deny_unknown_fields, not silently ignored.
    /// After fix: serde parse failure returns Ok(response) with closeout_allowed=false and
    /// a parse_error violation (plus any raw_shape_violations), rather than Err(String).
    #[test]
    fn unknown_field_in_record_is_rejected_at_parse() {
        let bad = json!({
            "schema_version": CLOSEOUT_RECORD_SCHEMA_VERSION,
            "task_id": "t",
            "summary": "ok",
            "verification_state": "passed"
        });
        let response = evaluate_closeout_record_value(bad)
            .expect("should return enforcement response, not Err");
        assert_eq!(response["closeout_allowed"], json!(false));
        let violations = response["violations"].as_array().expect("violations");
        assert!(
            violations.iter().any(|v| v["rule"] == "parse_error"),
            "should have parse_error violation: {:?}",
            violations
        );
        let parse_detail = violations
            .iter()
            .find(|v| v["rule"] == "parse_error")
            .and_then(|v| v["detail"].as_str())
            .unwrap_or("");
        assert!(
            parse_detail.contains("unknown field"),
            "parse_error detail should mention unknown field: {}",
            parse_detail
        );
    }
}
