use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const CLOSEOUT_RECORD_SCHEMA_VERSION: &str = "closeout-record-v1";
pub const CLOSEOUT_ENFORCEMENT_RESPONSE_SCHEMA_VERSION: &str =
    "router-rs-closeout-enforcement-response-v1";
pub const CLOSEOUT_ENFORCEMENT_AUTHORITY: &str = "rust-closeout-enforcement";

const COMPLETION_KEYWORDS: &[&str] = &[
    "done",
    "finished",
    "completed",
    "succeeded",
    "passed",
    "已完成",
    "完成",
    "通过",
    "搞定",
];

const ALLOWED_VERIFICATION_STATUSES: &[&str] = &["passed", "failed", "partial", "not_run"];

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub struct CloseoutCommandRecord {
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub exit_code: i64,
    #[serde(default)]
    pub duration_ms: Option<i64>,
    #[serde(default)]
    pub stdout_summary: Option<String>,
    #[serde(default)]
    pub stderr_summary: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub struct CloseoutArtifactRecord {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub exists: bool,
    #[serde(default)]
    pub size_bytes: Option<i64>,
    #[serde(default)]
    pub checks: Vec<String>,
}

/// Deny unknown fields so that typos like `verification_state` (instead of
/// `verification_status`) or `commands_ran` (instead of `commands_run`) fail
/// loud with a `parse closeout record failed` error rather than being
/// silently ignored by serde defaults. The schema is closed; new fields must
/// be added in lockstep with `configs/framework/CLOSEOUT_RECORD_SCHEMA.json`.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub struct CloseoutRecord {
    #[serde(default)]
    pub schema_version: String,
    #[serde(default)]
    pub task_id: String,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
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
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CloseoutViolation {
    pub rule: String,
    pub severity: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CloseoutEnforcementResponse {
    pub schema_version: String,
    pub authority: String,
    pub task_id: String,
    pub closeout_allowed: bool,
    pub claimed_completion: bool,
    pub violations: Vec<CloseoutViolation>,
    pub missing_evidence: Vec<String>,
    pub verification_status: String,
}

pub fn evaluate_closeout_record_value(payload: Value) -> Result<Value, String> {
    let raw_shape_violations = raw_closeout_record_shape_violations(&payload, None);
    let record: CloseoutRecord = serde_json::from_value(payload)
        .map_err(|err| format!("parse closeout record failed: {err}"))?;
    let mut response = evaluate_closeout_record(&record);
    append_closeout_violations(&mut response, raw_shape_violations);
    serde_json::to_value(response).map_err(|err| format!("serialize closeout response: {err}"))
}

pub fn evaluate_closeout_record(record: &CloseoutRecord) -> CloseoutEnforcementResponse {
    let mut violations: Vec<CloseoutViolation> = Vec::new();
    let mut missing: Vec<String> = Vec::new();

    // 0. schema_version sanity (block: refuse evaluation of missing or unknown shape).
    if record.schema_version.trim().is_empty()
        || record.schema_version != CLOSEOUT_RECORD_SCHEMA_VERSION
    {
        violations.push(CloseoutViolation {
            rule: "schema_version_mismatch".to_string(),
            severity: "block".to_string(),
            detail: format!(
                "expected schema_version={CLOSEOUT_RECORD_SCHEMA_VERSION}, got {:?}",
                record.schema_version
            ),
        });
    }

    if record.task_id.trim().is_empty() {
        violations.push(CloseoutViolation {
            rule: "task_id_missing".to_string(),
            severity: "block".to_string(),
            detail: "task_id must be non-empty".to_string(),
        });
        missing.push("task_id".to_string());
    }

    let summary_trimmed = record.summary.trim();
    if summary_trimmed.is_empty() {
        violations.push(CloseoutViolation {
            rule: "summary_missing".to_string(),
            severity: "block".to_string(),
            detail: "summary must be non-empty".to_string(),
        });
        missing.push("summary".to_string());
    }

    let claimed_completion = summary_claims_completion(summary_trimmed);

    let status_lower = record.verification_status.trim().to_ascii_lowercase();
    let status_recognized = ALLOWED_VERIFICATION_STATUSES.contains(&status_lower.as_str());
    if !status_lower.is_empty() && !status_recognized {
        violations.push(CloseoutViolation {
            rule: "verification_status_invalid".to_string(),
            severity: "block".to_string(),
            detail: format!(
                "verification_status must be one of {:?}, got {:?}",
                ALLOWED_VERIFICATION_STATUSES, record.verification_status
            ),
        });
    }
    if status_lower.is_empty() {
        violations.push(CloseoutViolation {
            rule: "verification_status_missing".to_string(),
            severity: "block".to_string(),
            detail: "verification_status must be one of passed|failed|partial|not_run".to_string(),
        });
        missing.push("verification_status".to_string());
    }

    // R1: claimed completion but no evidence and no acknowledged risk.
    if claimed_completion
        && status_lower == "not_run"
        && record.risks.is_empty()
        && record.blockers.is_empty()
    {
        violations.push(CloseoutViolation {
            rule: "claimed_done_without_evidence".to_string(),
            severity: "block".to_string(),
            detail: "summary claims completion but verification_status=not_run with no risks or blockers".to_string(),
        });
        missing.push("validation_command_or_risk_acknowledgement".to_string());
    }

    // R2: changed files but no commands_run and no risks recorded.
    if !record.changed_files.is_empty() && record.commands_run.is_empty() && record.risks.is_empty()
    {
        violations.push(CloseoutViolation {
            rule: "changed_files_without_command_or_risk".to_string(),
            severity: "block".to_string(),
            detail: format!(
                "{} changed file(s) recorded but no commands_run and no risks declared",
                record.changed_files.len()
            ),
        });
        missing.push("validation_command".to_string());
    }

    // R3: verification_status=passed but a command failed.
    if status_lower == "passed" {
        if let Some(failed) = record.commands_run.iter().find(|c| c.exit_code != 0) {
            violations.push(CloseoutViolation {
                rule: "verification_passed_with_failed_command".to_string(),
                severity: "block".to_string(),
                detail: format!(
                    "verification_status=passed but command exited {}: {}",
                    failed.exit_code, failed.command
                ),
            });
        }
    }

    // R3b: command evidence must be auditable; serde defaults must not turn `{}` into success.
    if let Some(invalid) = record
        .commands_run
        .iter()
        .find(|c| c.command.trim().is_empty())
    {
        violations.push(CloseoutViolation {
            rule: "invalid_command_evidence".to_string(),
            severity: "block".to_string(),
            detail: format!(
                "commands_run contains a row without a non-empty command; exit_code={}",
                invalid.exit_code
            ),
        });
        missing.push("command".to_string());
    }

    // R4: verification_status=passed but artifact missing.
    if status_lower == "passed" {
        if let Some(missing_artifact) = record.artifacts_checked.iter().find(|a| !a.exists) {
            violations.push(CloseoutViolation {
                rule: "verification_passed_with_missing_artifact".to_string(),
                severity: "block".to_string(),
                detail: format!(
                    "verification_status=passed but artifact does not exist: {}",
                    missing_artifact.path
                ),
            });
        }
    }

    // R5: not_run without blockers or risks.
    if status_lower == "not_run" && record.blockers.is_empty() && record.risks.is_empty() {
        // Only emit if not already covered by R1.
        let already_covered = violations
            .iter()
            .any(|v| v.rule == "claimed_done_without_evidence");
        if !already_covered {
            violations.push(CloseoutViolation {
                rule: "not_run_without_blockers_or_risks".to_string(),
                severity: "block".to_string(),
                detail: "verification_status=not_run requires at least one blocker or risk"
                    .to_string(),
            });
            missing.push("blocker_or_risk".to_string());
        }
    }

    // R6: failed verification but summary still claims completion without acknowledged risks.
    if status_lower == "failed"
        && claimed_completion
        && record.risks.is_empty()
        && record.blockers.is_empty()
    {
        violations.push(CloseoutViolation {
            rule: "claimed_done_with_failed_verification".to_string(),
            severity: "block".to_string(),
            detail: "summary claims completion but verification_status=failed without recorded risks or blockers".to_string(),
        });
    }

    // R9 (task-scoped depth / `GOAL_STATE.completion_gates` alignment): **deferred** — needs
    // `task_id`→ledger policy resolution and `CLOSEOUT_RECORD` serde/schema lockstep; use GOAL
    // `complete` + RFV `append_round` close_gates (explicit close **and** max_rounds cap close)
    // instead — contract: `docs/references/rfv-loop/reasoning-depth-contract.md`; ADR:
    // `docs/plans/ADR_rfv_close_gates_max_rounds.md`.

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
        violations.push(CloseoutViolation {
            rule: "claimed_passed_without_evidence".to_string(),
            severity: "block".to_string(),
            detail: "verification_status=passed but commands_run/artifacts_checked/risks/blockers all empty — supply at least one command, artifact check, risk, or blocker to back the claim".to_string(),
        });
        missing.push("evidence_or_acknowledgement".to_string());
    }

    let blocking = violations.iter().any(|v| v.severity == "block");

    CloseoutEnforcementResponse {
        schema_version: CLOSEOUT_ENFORCEMENT_RESPONSE_SCHEMA_VERSION.to_string(),
        authority: CLOSEOUT_ENFORCEMENT_AUTHORITY.to_string(),
        task_id: record.task_id.clone(),
        closeout_allowed: !blocking,
        claimed_completion,
        violations,
        missing_evidence: missing,
        verification_status: status_lower,
    }
}

pub fn closeout_enforcement_contract() -> Value {
    json!({
        "schema_version": CLOSEOUT_ENFORCEMENT_RESPONSE_SCHEMA_VERSION,
        "authority": CLOSEOUT_ENFORCEMENT_AUTHORITY,
        "record_schema_version": CLOSEOUT_RECORD_SCHEMA_VERSION,
        "allowed_verification_statuses": ALLOWED_VERIFICATION_STATUSES,
        "completion_keywords": COMPLETION_KEYWORDS,
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
            "claimed_passed_without_evidence_index_rows"
        ]
    })
}

/// Optional context for context-aware closeout evaluation. When supplied, R8 also
/// cross-checks against `EVIDENCE_INDEX.json` rows for the task: `verification_status=passed`
/// with empty `commands_run` AND zero successful EVIDENCE_INDEX rows is blocked even when
/// `artifacts_checked` is non-empty (artifact existence ≠ executable verification).
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct CloseoutEvidenceContext {
    /// Expected task id for task-scoped evaluation.
    pub task_id: Option<String>,
    /// Whether the task's `EVIDENCE_INDEX.json` `artifacts` array is non-empty.
    /// (Reserved: future R-rules may want to flag "rows present but none successful".)
    pub evidence_rows_non_empty: bool,
    /// Whether the task's `EVIDENCE_INDEX.json` has at least one row with
    /// `success==true` or `exit_code==0`.
    pub has_successful_verification: bool,
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
            response.violations.push(CloseoutViolation {
                rule: "task_id_context_mismatch".to_string(),
                severity: "block".to_string(),
                detail: format!(
                    "closeout record task_id {:?} does not match evaluation context {:?}",
                    record.task_id, expected
                ),
            });
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
        response.violations.push(CloseoutViolation {
            rule: "claimed_passed_without_evidence_index_rows".to_string(),
            severity: "block".to_string(),
            detail: "verification_status=passed and commands_run is empty, and EVIDENCE_INDEX.json has no successful rows — record at least one verifier command (or run a verifier so PostTool hooks append to EVIDENCE_INDEX)".to_string(),
        });
        response
            .missing_evidence
            .push("evidence_index_successful_row".to_string());
        response.closeout_allowed = false;
    }
    response
}

/// Convenience JSON wrapper mirroring [`evaluate_closeout_record_value`] but with context.
pub fn evaluate_closeout_record_value_with_context(
    payload: Value,
    ctx: &CloseoutEvidenceContext,
) -> Result<Value, String> {
    let raw_shape_violations =
        raw_closeout_record_shape_violations(&payload, ctx.task_id.as_deref());
    let record: CloseoutRecord = serde_json::from_value(payload)
        .map_err(|err| format!("parse closeout record failed: {err}"))?;
    let mut response = evaluate_closeout_record_with_context(&record, ctx);
    append_closeout_violations(&mut response, raw_shape_violations);
    serde_json::to_value(response).map_err(|err| format!("serialize closeout response: {err}"))
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
        violations.push(CloseoutViolation {
            rule: "schema_version_mismatch".to_string(),
            severity: "block".to_string(),
            detail: format!("expected schema_version={CLOSEOUT_RECORD_SCHEMA_VERSION}, got missing or empty value"),
        });
    }
    if let Some(expected) = expected_task_id.map(str::trim).filter(|s| !s.is_empty()) {
        let actual = payload
            .get("task_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        if actual != expected {
            violations.push(CloseoutViolation {
                rule: "task_id_context_mismatch".to_string(),
                severity: "block".to_string(),
                detail: format!("closeout record task_id {actual:?} does not match evaluation context {expected:?}"),
            });
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
                violations.push(CloseoutViolation {
                    rule: "invalid_command_evidence".to_string(),
                    severity: "block".to_string(),
                    detail: format!(
                        "commands_run[{idx}] must include non-empty command and integer exit_code"
                    ),
                });
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
    if summary.is_empty() {
        return false;
    }
    let lower = summary.to_ascii_lowercase();
    COMPLETION_KEYWORDS
        .iter()
        .any(|kw| lower.contains(&kw.to_ascii_lowercase()))
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
        assert!(response["violations"]
            .as_array()
            .expect("violations")
            .iter()
            .any(|v| v["rule"] == "schema_version_mismatch"));
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
        assert!(response["violations"]
            .as_array()
            .expect("violations")
            .iter()
            .any(|v| v["rule"] == "invalid_command_evidence"));
    }

    #[test]
    fn context_task_id_mismatch_blocks_value_evaluator() {
        let ctx = CloseoutEvidenceContext {
            task_id: Some("expected-task".to_string()),
            evidence_rows_non_empty: true,
            has_successful_verification: true,
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
        assert!(response["violations"]
            .as_array()
            .expect("violations")
            .iter()
            .any(|v| v["rule"] == "task_id_context_mismatch"));
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
        assert!(rules
            .iter()
            .any(|v| v == "verification_passed_with_missing_artifact"));
        assert!(rules.iter().any(|v| v == "claimed_passed_without_evidence"));
        assert!(rules
            .iter()
            .any(|v| v == "claimed_passed_without_evidence_index_rows"));
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
        };
        let resp = evaluate_closeout_record_with_context(&record, &ctx);
        assert!(!resp.closeout_allowed, "violations: {:?}", resp.violations);
        assert!(has_rule(
            &resp,
            "claimed_passed_without_evidence_index_rows"
        ));
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
        };
        let resp = evaluate_closeout_record_with_context(&record, &ctx);
        assert!(resp.closeout_allowed, "violations: {:?}", resp.violations);
        assert!(!has_rule(
            &resp,
            "claimed_passed_without_evidence_index_rows"
        ));
    }

    /// P0-F-style invariant: typo'd field rejected by deny_unknown_fields, not silently ignored.
    #[test]
    fn unknown_field_in_record_is_rejected_at_parse() {
        let bad = json!({
            "schema_version": CLOSEOUT_RECORD_SCHEMA_VERSION,
            "task_id": "t",
            "summary": "ok",
            "verification_state": "passed"
        });
        let err = evaluate_closeout_record_value(bad).expect_err("typo must fail parse");
        assert!(
            err.contains("parse closeout record failed"),
            "unexpected error: {err}"
        );
    }
}
