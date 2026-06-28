//! Closeout record validation — R1–R8 rules from the deleted
//! `fr-contracts/closeout_enforcement/` module, migrated to core-state (Wave 3c-i).
//!
//! These functions validate a closeout record against structural and semantic
//! rules, returning a JSON response that consumers use to decide whether
//! closeout is allowed.

use core_errors::FrameworkError;
use serde_json::{Value, json};

type Result<T> = std::result::Result<T, FrameworkError>;

pub const CLOSEOUT_RECORD_SCHEMA_VERSION: &str = "closeout-record-v1";
const CLOSEOUT_ENFORCEMENT_RESPONSE_SCHEMA_VERSION: &str =
    "router-rs-closeout-enforcement-response-v1";
const CLOSEOUT_ENFORCEMENT_AUTHORITY: &str = "core-state-closeout-validation";

const ALLOWED_VERIFICATION_STATUSES: &[&str] = &["passed", "failed", "partial", "not_run"];

/// Evidence context for context-aware closeout evaluation (R8 cross-checks
/// against EVIDENCE_INDEX.json).
#[derive(Debug, Clone, Default)]
pub struct CloseoutEvidenceContext {
    pub task_id: Option<String>,
    pub has_successful_verification: bool,
    pub goal_prediction: Option<core_state_types::goal_prediction::GoalStatePrediction>,
}

// ── Rule helpers ──────────────────────────────────────────────────────

fn rule_category(rule: &str) -> &'static str {
    match rule {
        "schema_version_mismatch"
        | "task_id_missing"
        | "summary_missing"
        | "verification_status_missing"
        | "verification_status_invalid"
        | "task_id_context_mismatch"
        | "parse_error"
        | "invalid_command_evidence" => "hard",
        "claimed_done_without_evidence"
        | "changed_files_without_command_or_risk"
        | "verification_passed_with_failed_command"
        | "verification_passed_with_missing_artifact"
        | "not_run_without_blockers_or_risks"
        | "claimed_done_with_failed_verification"
        | "claimed_passed_without_evidence"
        | "claimed_passed_without_evidence_index_rows" => "soft",
        _ => "hard",
    }
}

// ── Public entry points ───────────────────────────────────────────────

/// Evaluate a closeout record value against R1–R7 rules.
/// Returns a JSON response with `closeout_allowed`, `violations`, etc.
pub fn evaluate_closeout_record_value(payload: Value) -> Result<Value> {
    let raw_shape = raw_shape_violations(&payload, None);
    if raw_shape.iter().any(|v| v.severity == "block") {
        return blocked_response(payload, raw_shape);
    }
    let task_id = payload
        .get("task_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    match serde_json::from_value::<CloseoutRecord>(payload) {
        Ok(record) => {
            let response = evaluate_record(&record);
            Ok(serde_json::to_value(response)?)
        }
        Err(err) => parse_error_response(&task_id, raw_shape, &err.to_string()),
    }
}

/// Evaluate a closeout record value with evidence context (includes R8).
pub fn evaluate_closeout_record_value_with_context(
    payload: Value,
    ctx: &CloseoutEvidenceContext,
) -> Result<Value> {
    let raw_shape = raw_shape_violations(&payload, ctx.task_id.as_deref());
    if raw_shape.iter().any(|v| v.severity == "block") {
        return blocked_response(payload, raw_shape);
    }
    let task_id = payload
        .get("task_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    match serde_json::from_value::<CloseoutRecord>(payload) {
        Ok(record) => {
            let mut response = evaluate_record(&record);
            // R8: cross-check EVIDENCE_INDEX
            let has_success = ctx.has_successful_verification;
            let status = record.verification_status.trim().to_ascii_lowercase();
            if status == "passed"
                && record.commands_run.is_empty()
                && !has_success
                && !response
                    .violations
                    .iter()
                    .any(|v| v.rule == "claimed_passed_without_evidence")
            {
                response.violations.push(CloseoutViolation::new(
                    "claimed_passed_without_evidence_index_rows", "block",
                    "verification_status=passed and commands_run is empty, and EVIDENCE_INDEX.json has no successful rows",
                ));
                response
                    .missing_evidence
                    .push("evidence_index_successful_row".into());
                response.closeout_allowed = false;
            }
            let has_hard = response.violations.iter().any(|v| v.category == "hard");
            response.can_proceed = !has_hard;
            Ok(serde_json::to_value(response)?)
        }
        Err(err) => parse_error_response(&task_id, raw_shape, &err.to_string()),
    }
}

/// Return the closeout enforcement contract as JSON (schema + rule enumeration).
pub fn closeout_enforcement_contract() -> Value {
    json!({
        "schema_version": CLOSEOUT_ENFORCEMENT_RESPONSE_SCHEMA_VERSION,
        "authority": CLOSEOUT_ENFORCEMENT_AUTHORITY,
        "record_schema_version": CLOSEOUT_RECORD_SCHEMA_VERSION,
        "allowed_verification_statuses": ALLOWED_VERIFICATION_STATUSES,
        "rules": [
            "schema_version_mismatch", "task_id_context_mismatch", "task_id_missing",
            "summary_missing", "verification_status_missing", "verification_status_invalid",
            "claimed_done_without_evidence", "changed_files_without_command_or_risk",
            "verification_passed_with_failed_command", "invalid_command_evidence",
            "verification_passed_with_missing_artifact", "not_run_without_blockers_or_risks",
            "claimed_done_with_failed_verification", "claimed_passed_without_evidence",
            "claimed_passed_without_evidence_index_rows", "parse_error",
        ],
    })
}

/// Return the closeout record schema version constant for use in schema drift checks.
pub fn closeout_record_schema_version() -> &'static str {
    CLOSEOUT_RECORD_SCHEMA_VERSION
}

// ── Internal types (serialization-only) ───────────────────────────────

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct CloseoutRecord {
    #[serde(default)]
    schema_version: String,
    #[serde(default)]
    task_id: String,
    #[serde(default)]
    changed_files: Vec<String>,
    #[serde(default)]
    commands_run: Vec<CloseoutCommand>,
    #[serde(default)]
    artifacts_checked: Vec<CloseoutArtifact>,
    #[serde(default)]
    verification_status: String,
    #[serde(default)]
    blockers: Vec<String>,
    #[serde(default)]
    risks: Vec<String>,
    #[serde(default)]
    summary: String,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct CloseoutCommand {
    #[serde(default)]
    command: String,
    #[serde(default)]
    exit_code: i64,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct CloseoutArtifact {
    #[serde(default)]
    path: String,
    #[serde(default)]
    exists: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
struct CloseoutViolation {
    rule: String,
    severity: String,
    category: String,
    detail: String,
}

impl CloseoutViolation {
    fn new(
        rule: impl Into<String>,
        severity: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        let rule = rule.into();
        let category = rule_category(&rule).to_string();
        Self {
            rule,
            severity: severity.into(),
            category,
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct CloseoutResponse {
    schema_version: String,
    authority: String,
    task_id: String,
    closeout_allowed: bool,
    can_proceed: bool,
    claimed_completion: bool,
    violations: Vec<CloseoutViolation>,
    missing_evidence: Vec<String>,
    verification_status: String,
}

// ── Core evaluation (R1–R7) ───────────────────────────────────────────

fn evaluate_record(record: &CloseoutRecord) -> CloseoutResponse {
    let mut violations = Vec::new();
    let mut missing = Vec::new();

    // Schema version
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
    // Task id
    if record.task_id.trim().is_empty() {
        violations.push(CloseoutViolation::new(
            "task_id_missing",
            "block",
            "task_id must be non-empty",
        ));
        missing.push("task_id".into());
    }
    // Summary
    let summary_trimmed = record.summary.trim();
    if summary_trimmed.is_empty() {
        violations.push(CloseoutViolation::new(
            "summary_missing",
            "block",
            "summary must be non-empty",
        ));
        missing.push("summary".into());
    }
    let claimed_completion = summary_claims_completion(summary_trimmed);
    // Verification status
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
        missing.push("verification_status".into());
    }

    // R1: Claimed done without evidence
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
        missing.push("validation_command_or_risk_acknowledgement".into());
    }
    // R2: Changed files without command
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
        missing.push("validation_command".into());
    }
    // R3: Verification passed with failed command
    if status_lower == "passed"
        && let Some(failed) = record.commands_run.iter().find(|c| c.exit_code != 0)
    {
        violations.push(CloseoutViolation::new(
            "verification_passed_with_failed_command",
            "block",
            format!(
                "verification_status=passed but command exited {}: {}",
                failed.exit_code, failed.command
            ),
        ));
    }
    // R3b: Invalid command evidence
    for cmd in &record.commands_run {
        if cmd.command.trim().is_empty() {
            violations.push(CloseoutViolation::new(
                "invalid_command_evidence",
                "block",
                format!(
                    "commands_run contains a row without a non-empty command; exit_code={}",
                    cmd.exit_code
                ),
            ));
            missing.push("command".into());
        }
    }
    // R4: Verification passed with missing artifact
    if status_lower == "passed"
        && let Some(ma) = record.artifacts_checked.iter().find(|a| !a.exists)
    {
        violations.push(CloseoutViolation::new(
            "verification_passed_with_missing_artifact",
            "block",
            format!(
                "verification_status=passed but artifact does not exist: {}",
                ma.path
            ),
        ));
    }
    // R5: Not run without blockers or risks
    if status_lower == "not_run" && record.blockers.is_empty() && record.risks.is_empty() {
        let already = violations
            .iter()
            .any(|v| v.rule == "claimed_done_without_evidence");
        if !already {
            violations.push(CloseoutViolation::new(
                "not_run_without_blockers_or_risks",
                "block",
                "verification_status=not_run requires at least one blocker or risk",
            ));
            missing.push("blocker_or_risk".into());
        }
    }
    // R6: Claimed done with failed verification
    if status_lower == "failed"
        && claimed_completion
        && record.risks.is_empty()
        && record.blockers.is_empty()
    {
        violations.push(CloseoutViolation::new("claimed_done_with_failed_verification", "block",
            "summary claims completion but verification_status=failed without recorded risks or blockers"));
    }
    // R7: Claimed passed without evidence
    if status_lower == "passed"
        && record.commands_run.is_empty()
        && record.artifacts_checked.is_empty()
        && record.risks.is_empty()
        && record.blockers.is_empty()
    {
        violations.push(CloseoutViolation::new("claimed_passed_without_evidence", "block",
            "verification_status=passed but commands_run/artifacts_checked/risks/blockers all empty"));
        missing.push("evidence_or_acknowledgement".into());
    }

    let blocking = violations.iter().any(|v| v.severity == "block");
    let has_hard = violations.iter().any(|v| v.category == "hard");
    CloseoutResponse {
        schema_version: CLOSEOUT_ENFORCEMENT_RESPONSE_SCHEMA_VERSION.into(),
        authority: CLOSEOUT_ENFORCEMENT_AUTHORITY.into(),
        task_id: record.task_id.clone(),
        closeout_allowed: !blocking,
        can_proceed: !has_hard,
        claimed_completion,
        violations,
        missing_evidence: missing,
        verification_status: status_lower,
    }
}

// ── Helpers ───────────────────────────────────────────────────────────

fn raw_shape_violations(payload: &Value, expected_tid: Option<&str>) -> Vec<CloseoutViolation> {
    let mut violations = Vec::new();
    if payload
        .get("schema_version")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        violations.push(CloseoutViolation::new("schema_version_mismatch", "block",
            format!("expected schema_version={CLOSEOUT_RECORD_SCHEMA_VERSION}, got missing or empty value")));
    }
    if let Some(expected) = expected_tid.map(str::trim).filter(|s| !s.is_empty()) {
        let actual = payload
            .get("task_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        if actual != expected {
            violations.push(CloseoutViolation::new("task_id_context_mismatch", "block",
                format!("closeout record task_id {actual:?} does not match evaluation context {expected:?}")));
        }
    }
    if let Some(commands) = payload.get("commands_run").and_then(Value::as_array) {
        for (idx, cmd) in commands.iter().enumerate() {
            let ct = cmd
                .get("command")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or("");
            let has_ec = cmd.get("exit_code").and_then(Value::as_i64).is_some();
            if ct.is_empty() || !has_ec {
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

fn blocked_response(payload: Value, violations: Vec<CloseoutViolation>) -> Result<Value> {
    let tid = payload.get("task_id").and_then(Value::as_str).unwrap_or("");
    let mut resp = base_response(tid);
    for v in violations {
        if !resp
            .violations
            .iter()
            .any(|e| e.rule == v.rule && e.detail == v.detail)
        {
            resp.violations.push(v);
        }
    }
    Ok(serde_json::to_value(resp)?)
}

fn parse_error_response(
    tid: &str,
    _violations: Vec<CloseoutViolation>,
    err: &str,
) -> Result<Value> {
    let mut resp = base_response(tid);
    resp.violations.push(CloseoutViolation::new(
        "parse_error",
        "block",
        format!("parse closeout record failed: {err}"),
    ));
    Ok(serde_json::to_value(resp)?)
}

fn base_response(tid: &str) -> CloseoutResponse {
    CloseoutResponse {
        schema_version: CLOSEOUT_ENFORCEMENT_RESPONSE_SCHEMA_VERSION.into(),
        authority: CLOSEOUT_ENFORCEMENT_AUTHORITY.into(),
        task_id: tid.into(),
        closeout_allowed: false,
        can_proceed: false,
        claimed_completion: false,
        violations: Vec::new(),
        missing_evidence: Vec::new(),
        verification_status: String::new(),
    }
}

fn summary_claims_completion(summary: &str) -> bool {
    core_policy::hook_common::contains_completion_claim_token(summary)
}
