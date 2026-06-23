use serde_json::{Value, json};

use super::types::*;
use super::ALLOWED_VERIFICATION_STATUSES;

pub fn closeout_enforcement_contract() -> Value {
    json!({
        "schema_version": CLOSEOUT_ENFORCEMENT_RESPONSE_SCHEMA_VERSION,
        "authority": CLOSEOUT_ENFORCEMENT_AUTHORITY,
        "record_schema_version": CLOSEOUT_RECORD_SCHEMA_VERSION,
        "allowed_verification_statuses": ALLOWED_VERIFICATION_STATUSES,
        "completion_keywords": core_policy::hook_common::completion_claim_keywords_export(),
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
