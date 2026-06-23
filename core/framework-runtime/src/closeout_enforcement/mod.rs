// ── Submodules ──
pub mod types;
pub mod evaluation;
pub mod contract;
#[cfg(test)]
mod tests;

// ── Module-private helpers shared by submodules ──

/// Allowed verification status values for closeout records.
const ALLOWED_VERIFICATION_STATUSES: &[&str] = &["passed", "failed", "partial", "not_run"];

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

// ── Re-exports: preserve all original pub paths ──
pub use types::*;
pub use evaluation::*;
pub use contract::*;
