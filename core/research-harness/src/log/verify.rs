// Migrated from tools/research-log-rs/src/verify.rs

//! Schema validation for research harness state files.
//!
//! Validates `research-state.yaml` files against the expected schema,
//! reporting errors and warnings for structural issues.

use anyhow::Result;
use serde_json::Value;
use std::path::Path;

/// A validation issue found during schema validation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ValidationIssue {
    /// JSON Pointer-like path to the issue location.
    pub path: String,
    /// "error" | "warning"
    pub severity: String,
    /// Human-readable description.
    pub message: String,
}

/// Validate a `research-state.yaml` file against the expected schema.
/// Returns a list of issues (empty = valid).
pub fn validate_research_state(path: &Path) -> Result<Vec<ValidationIssue>> {
    let content = std::fs::read_to_string(path)?;
    let value: Value = serde_yml::from_str(&content)?;
    let mut issues = Vec::new();

    // Required top-level keys
    let required = ["schema_version", "project", "question", "status", "stage"];
    for key in &required {
        if value.get(*key).is_none() {
            issues.push(ValidationIssue {
                path: format!("/{}", key),
                severity: "error".into(),
                message: format!("Missing required key: {}", key),
            });
        }
    }

    // schema_version must be a positive integer
    if let Some(sv) = value.get("schema_version") {
        if !sv.is_number() || sv.as_i64().unwrap_or(0) < 1 {
            issues.push(ValidationIssue {
                path: "/schema_version".into(),
                severity: "error".into(),
                message: format!("schema_version must be a positive integer, got {:?}", sv),
            });
        }
    }

    // status must be one of known values
    if let Some(status) = value.get("status").and_then(Value::as_str) {
        if !["active", "concluded", "archived"].contains(&status) {
            issues.push(ValidationIssue {
                path: "/status".into(),
                severity: "warning".into(),
                message: format!(
                    "Unknown status '{}'; expected active|concluded|archived",
                    status
                ),
            });
        }
    }

    // stage lifecycle validation
    if let Some(stage) = value.get("stage").and_then(Value::as_str) {
        let known_stages = [
            "bootstrap",
            "inner-loop", "inner_loop",
            "inner_loop_design",
            "inner_loop_code",
            "inner_loop_eval",
            "outer-loop", "outer_loop",
            "barrier_escalation",
            "finalize",
            "reflect",
        ];
        if !known_stages.contains(&stage) {
            issues.push(ValidationIssue {
                path: "/stage".into(),
                severity: "warning".into(),
                message: format!("Unknown stage '{}'", stage),
            });
        }
    }

    // hypotheses should be an array
    if let Some(hypotheses) = value.get("hypotheses") {
        if !hypotheses.is_array() {
            issues.push(ValidationIssue {
                path: "/hypotheses".into(),
                severity: "error".into(),
                message: "hypotheses must be an array".into(),
            });
        }
    }

    // run_history items should have required fields
    if let Some(runs) = value.get("run_history").and_then(Value::as_array) {
        for (i, run) in runs.iter().enumerate() {
            if run.get("run_id").is_none() {
                issues.push(ValidationIssue {
                    path: format!("/run_history/{}/run_id", i),
                    severity: "error".into(),
                    message: "run record missing run_id".into(),
                });
            }
            if run.get("outcome").is_none() {
                issues.push(ValidationIssue {
                    path: format!("/run_history/{}/outcome", i),
                    severity: "warning".into(),
                    message: "run record missing outcome".into(),
                });
            }
        }
    }

    // novelty_gate sub-object check
    if let Some(gate) = value.get("novelty_gate") {
        if gate.get("status").is_none() {
            issues.push(ValidationIssue {
                path: "/novelty_gate/status".into(),
                severity: "warning".into(),
                message: "novelty_gate missing status".into(),
            });
        }
    }

    Ok(issues)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("research-state.yaml");
        std::fs::write(
            &path,
            r#"
schema_version: 4
project: test-project
question: test question
status: active
stage: bootstrap
hypotheses: []
run_history: []
novelty_gate:
  status: pending
"#,
        )
        .unwrap();
        let issues = validate_research_state(&path).unwrap();
        assert!(
            issues.is_empty(),
            "valid state should have no issues: {:?}",
            issues
        );
    }

    #[test]
    fn test_validate_missing_required() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("research-state.yaml");
        std::fs::write(&path, r#"schema_version: 4"#).unwrap();
        let issues = validate_research_state(&path).unwrap();
        let errors: Vec<_> = issues.iter().filter(|i| i.severity == "error").collect();
        assert!(errors.len() >= 3, "should have errors for missing required keys");
    }

    #[test]
    fn test_validate_bad_schema_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("research-state.yaml");
        std::fs::write(
            &path,
            r#"
schema_version: 0
project: test
question: test
status: active
stage: bootstrap
"#,
        )
        .unwrap();
        let issues = validate_research_state(&path).unwrap();
        assert!(issues.iter().any(|i| i.path == "/schema_version"));
    }

    #[test]
    fn test_validate_unknown_status() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("research-state.yaml");
        std::fs::write(
            &path,
            r#"
schema_version: 4
project: test
question: test
status: invalid_status
stage: bootstrap
"#,
        )
        .unwrap();
        let issues = validate_research_state(&path).unwrap();
        assert!(issues
            .iter()
            .any(|i| i.message.contains("Unknown status")));
    }
}
