use serde_json::Value;

use super::types::*;
use super::ALLOWED_VERIFICATION_STATUSES;

/// Shared helper: build a blocked closeout response with the given violations.
/// Used by both `evaluate_closeout_record_value` and its `_with_context` variant.
fn make_blocked_closeout_response(
    task_id: &str,
    violations: Vec<CloseoutViolation>,
) -> Result<Value, String> {
    let mut response = CloseoutEnforcementResponse {
        schema_version: CLOSEOUT_ENFORCEMENT_RESPONSE_SCHEMA_VERSION.to_string(),
        authority: CLOSEOUT_ENFORCEMENT_AUTHORITY.to_string(),
        task_id: task_id.to_string(),
        closeout_allowed: false,
        can_proceed: false,
        claimed_completion: false,
        violations: Vec::new(),
        missing_evidence: Vec::new(),
        verification_status: String::new(),
        prediction_verification: Vec::new(),
    };
    append_closeout_violations(&mut response, violations);
    serde_json::to_value(response).map_err(|err| format!("serialize closeout response: {err}"))
}

/// Shared helper: build a parse-error closeout response.
fn make_parse_error_closeout_response(
    task_id: &str,
    violations: Vec<CloseoutViolation>,
    parse_error: String,
) -> Result<Value, String> {
    let mut response = CloseoutEnforcementResponse {
        schema_version: CLOSEOUT_ENFORCEMENT_RESPONSE_SCHEMA_VERSION.to_string(),
        authority: CLOSEOUT_ENFORCEMENT_AUTHORITY.to_string(),
        task_id: task_id.to_string(),
        closeout_allowed: false,
        can_proceed: false,
        claimed_completion: false,
        violations: Vec::new(),
        missing_evidence: Vec::new(),
        verification_status: String::new(),
        prediction_verification: Vec::new(),
    };
    append_closeout_violations(&mut response, violations);
    response.violations.push(CloseoutViolation::new(
        "parse_error",
        "block",
        format!("parse closeout record failed: {parse_error}"),
    ));
    serde_json::to_value(response).map_err(|err| format!("serialize closeout response: {err}"))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn evaluate_closeout_record_value(payload: Value) -> Result<Value, String> {
    // Check raw shape violations FIRST: critical issues like missing schema_version
    // are more actionable than serde's deny_unknown_fields errors, which can mask
    // the real problem when the record shape is fundamentally broken.
    let raw_shape_violations = raw_closeout_record_shape_violations(&payload, None);
    if raw_shape_violations.iter().any(|v| v.severity == "block") {
        let task_id = payload
            .get("task_id")
            .and_then(Value::as_str)
            .unwrap_or("");
        return make_blocked_closeout_response(task_id, raw_shape_violations);
    }
    let task_id = payload
        .get("task_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    match serde_json::from_value::<CloseoutRecord>(payload) {
        Ok(record) => {
            let mut response = evaluate_closeout_record(&record);
            append_closeout_violations(&mut response, raw_shape_violations);
            serde_json::to_value(response)
                .map_err(|err| format!("serialize closeout response: {err}"))
        }
        Err(err) => make_parse_error_closeout_response(&task_id, raw_shape_violations, err.to_string()),
    }
}

pub fn evaluate_closeout_record(record: &CloseoutRecord) -> CloseoutEnforcementResponse {
    let mut violations: Vec<CloseoutViolation> = Vec::new();
    let mut missing: Vec<String> = Vec::new();

    validate_schema_version(record, &mut violations, &mut missing);
    validate_task_id(record, &mut violations, &mut missing);
    let (_summary_trimmed, claimed_completion) =
        validate_summary(record, &mut violations, &mut missing);
    let (status_lower, _status_recognized) =
        validate_verification_status(record, &mut violations, &mut missing);

    validate_r1_claimed_done_without_evidence(
        record,
        &status_lower,
        claimed_completion,
        &mut violations,
        &mut missing,
    );
    validate_r2_changed_files_without_command(record, &mut violations, &mut missing);
    validate_r3_verification_passed_with_failed_command(record, &status_lower, &mut violations);
    validate_r3b_invalid_command_evidence(record, &mut violations, &mut missing);
    validate_r4_verification_passed_with_missing_artifact(record, &status_lower, &mut violations);
    validate_r5_not_run_without_blockers_or_risks(record, &status_lower, &mut violations, &mut missing);
    validate_r6_claimed_done_with_failed_verification(
        record,
        &status_lower,
        claimed_completion,
        &mut violations,
    );

    // @later(v7.2): task-scoped depth / `GOAL_STATE.completion_gates` alignment
    // Phase 3 pointer consolidation (3B/3C) completed 2026-06-02.
    // Next: re-evaluate closeout vs completion_gates alignment -- state model simplified
    //   (5 files -> 2 control-plane anchors), closeout gate should align with
    //   GOAL_STATE.completion_gates.min_depth_score against depth_compliance_aggregate output.

    // R7 (depth review P0-B): verification_status=passed but record carries no command evidence
    // and the optional EvidenceContext (when supplied by orchestrator) shows no successful
    // EVIDENCE_INDEX rows either. Pure self-attestation should not be enough to claim "passed".
    // The context-aware overload `evaluate_closeout_record_with_context` enforces this; here we
    // emit only the record-internal half so the rule is documented and `commands_run`-empty
    // claims at least surface a violation when no risks are acknowledged.
    validate_r7_claimed_passed_without_evidence(record, &status_lower, &mut violations, &mut missing);

    build_closeout_response(record, &status_lower, claimed_completion, violations, missing)
}

fn validate_schema_version(
    record: &CloseoutRecord,
    violations: &mut Vec<CloseoutViolation>,
    _missing: &mut Vec<String>,
) {
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
}

fn validate_task_id(
    record: &CloseoutRecord,
    violations: &mut Vec<CloseoutViolation>,
    missing: &mut Vec<String>,
) {
    if record.task_id.trim().is_empty() {
        violations.push(CloseoutViolation::new(
            "task_id_missing",
            "block",
            "task_id must be non-empty",
        ));
        missing.push("task_id".to_string());
    }
}

fn validate_summary(
    record: &CloseoutRecord,
    violations: &mut Vec<CloseoutViolation>,
    missing: &mut Vec<String>,
) -> (String, bool) {
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
    (summary_trimmed.to_string(), claimed_completion)
}

fn validate_verification_status(
    record: &CloseoutRecord,
    violations: &mut Vec<CloseoutViolation>,
    missing: &mut Vec<String>,
) -> (String, bool) {
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
    (status_lower, status_recognized)
}

fn validate_r1_claimed_done_without_evidence(
    record: &CloseoutRecord,
    status_lower: &str,
    claimed_completion: bool,
    violations: &mut Vec<CloseoutViolation>,
    missing: &mut Vec<String>,
) {
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
}

fn validate_r2_changed_files_without_command(
    record: &CloseoutRecord,
    violations: &mut Vec<CloseoutViolation>,
    missing: &mut Vec<String>,
) {
    if !record.changed_files.is_empty() && record.commands_run.is_empty() && record.risks.is_empty() {
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
}

fn validate_r3_verification_passed_with_failed_command(
    record: &CloseoutRecord,
    status_lower: &str,
    violations: &mut Vec<CloseoutViolation>,
) {
    if status_lower == "passed"
        && let Some(failed) = record.commands_run.iter().find(|c| c.exit_code != 0) {
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

fn validate_r3b_invalid_command_evidence(
    record: &CloseoutRecord,
    violations: &mut Vec<CloseoutViolation>,
    missing: &mut Vec<String>,
) {
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
}

fn validate_r4_verification_passed_with_missing_artifact(
    record: &CloseoutRecord,
    status_lower: &str,
    violations: &mut Vec<CloseoutViolation>,
) {
    if status_lower == "passed"
        && let Some(missing_artifact) = record.artifacts_checked.iter().find(|a| !a.exists) {
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

fn validate_r5_not_run_without_blockers_or_risks(
    record: &CloseoutRecord,
    status_lower: &str,
    violations: &mut Vec<CloseoutViolation>,
    missing: &mut Vec<String>,
) {
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
}

fn validate_r6_claimed_done_with_failed_verification(
    record: &CloseoutRecord,
    status_lower: &str,
    claimed_completion: bool,
    violations: &mut Vec<CloseoutViolation>,
) {
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
}

fn validate_r7_claimed_passed_without_evidence(
    record: &CloseoutRecord,
    status_lower: &str,
    violations: &mut Vec<CloseoutViolation>,
    missing: &mut Vec<String>,
) {
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
}

fn build_closeout_response(
    record: &CloseoutRecord,
    status_lower: &str,
    claimed_completion: bool,
    violations: Vec<CloseoutViolation>,
    missing: Vec<String>,
) -> CloseoutEnforcementResponse {
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
        verification_status: status_lower.to_string(),
        prediction_verification: Vec::new(),
    }
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
        && record.task_id.trim() != expected {
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
    prediction: &core_state_types::goal_prediction::GoalStatePrediction,
    verification_status: &str,
    summary: &str,
) {
    let checks = core_state_types::goal_prediction::verify_prediction_against_closeout(
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
        framework_runtime_hooks::hooks().emit_prediction_outcome(
            &response.task_id,
            &format!("prediction: {} => {}", prediction.expected_verification_status.as_deref().unwrap_or("?"), prediction.hypothesis.as_deref().unwrap_or("?")),
            verification_status,
            checks.len(),
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
        let task_id = payload
            .get("task_id")
            .and_then(Value::as_str)
            .unwrap_or("");
        return make_blocked_closeout_response(task_id, raw_shape_violations);
    }
    let task_id = payload
        .get("task_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    match serde_json::from_value::<CloseoutRecord>(payload) {
        Ok(record) => {
            let mut response = evaluate_closeout_record_with_context(&record, ctx);
            append_closeout_violations(&mut response, raw_shape_violations);
            serde_json::to_value(response)
                .map_err(|err| format!("serialize closeout response: {err}"))
        }
        Err(err) => make_parse_error_closeout_response(&task_id, raw_shape_violations, err.to_string()),
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
    core_policy::hook_common::contains_completion_claim_token(summary)
}
