//! QG Route `GateChecker` adapter for `DimensionalConsistency`.
//!
//! Extracts equation strings from `CheckContext::output_data` and calls
//! `formal::check_dimensional_consistency` for each.
//!
//! Expected `output_data` JSON:
//! ```json
//! { "equations": ["F = ma", "E = mc^2"] }
//! ```

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};

use crate::verification::formal;

pub struct DimensionalConsistency;

impl GateChecker for DimensionalConsistency {
    fn id(&self) -> &'static str {
        "dimensional_consistency"
    }

    fn description(&self) -> &'static str {
        "dimensional consistency: SI unit matching, equation chain validation"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut findings = Vec::new();

        let Some(data) = ctx.output_data.as_ref() else {
            findings.push(Finding {
                id: "formal_no_data".to_string(),
                severity: Severity::C,
                description: "No output_data provided — dimensional consistency checks skipped"
                    .to_string(),
                location: None,
                suggestion: Some(
                    "pass output_data with equations array to enable checks".to_string(),
                ),
            });
            return CheckResult {
                checker_id: self.id().to_string(),
                passed: true,
                findings,
            };
        };

        let Some(equations) = data.get("equations").and_then(|v| v.as_array()) else {
            findings.push(Finding {
                id: "formal_no_equations".to_string(),
                severity: Severity::C,
                description: "output_data has no equations array — dimensional check skipped"
                    .to_string(),
                location: None,
                suggestion: Some("add \"equations\": [\"F = ma\", ...] to output_data".to_string()),
            });
            return CheckResult {
                checker_id: self.id().to_string(),
                passed: true,
                findings,
            };
        };

        for (i, eq_val) in equations.iter().enumerate() {
            let Some(eq_str) = eq_val.as_str() else {
                continue;
            };
            match formal::check_dimensional_consistency(eq_str) {
                Ok(consistent) => {
                    findings.push(Finding {
                        id: format!("dimensional_eq_{i}"),
                        severity: if consistent { Severity::C } else { Severity::B },
                        description: format!(
                            "Equation '{eq_str}': {}",
                            if consistent {
                                "dimensionally consistent"
                            } else {
                                "dimensional inconsistency detected"
                            }
                        ),
                        location: Some(format!("equations[{i}]")),
                        suggestion: if consistent {
                            None
                        } else {
                            Some("check unit symbols and dimensions in equation".to_string())
                        },
                    });
                }
                Err(e) => {
                    findings.push(Finding {
                        id: format!("dimensional_eq_{i}_error"),
                        severity: Severity::C,
                        description: format!("Equation '{eq_str}' analysis error: {e}"),
                        location: Some(format!("equations[{i}]")),
                        suggestion: None,
                    });
                }
            }
        }

        let passed = findings
            .iter()
            .all(|f| matches!(f.severity, Severity::C | Severity::Warning));
        CheckResult {
            checker_id: self.id().to_string(),
            passed,
            findings,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use quality_gate::checker::GateChecker;
    use quality_gate::types::CheckContext;

    fn ctx(output_data: Option<serde_json::Value>) -> CheckContext {
        CheckContext {
            scene: "test".into(),
            sub_scene: None,
            goal: "test".into(),
            round: 1,
            repo_root: std::path::PathBuf::from("."),
            task_id: "t1".into(),
            evidence_path: None,
            runtime_handle: None,
            output_data,
            evaluated_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn no_data_returns_passed() {
        let gate = DimensionalConsistency;
        let result = gate.check(&ctx(None));
        assert!(result.passed);
        assert_eq!(result.findings.len(), 1);
        assert_eq!(result.findings[0].id, "formal_no_data");
        assert!(matches!(result.findings[0].severity, Severity::C));
    }

    #[test]
    fn no_equations_array_returns_passed() {
        let gate = DimensionalConsistency;
        let data = serde_json::json!({ "other_key": "value" });
        let result = gate.check(&ctx(Some(data)));
        assert!(result.passed);
        assert_eq!(result.findings[0].id, "formal_no_equations");
    }

    #[test]
    fn consistent_equation_passes() {
        let gate = DimensionalConsistency;
        // An equation with matching dimension annotations on both sides
        let data = serde_json::json!({
            "equations": ["[L] = [L]"]
        });
        let result = gate.check(&ctx(Some(data)));
        assert!(result.passed);
        let eq_finding = result.findings.iter().find(|f| f.id == "dimensional_eq_0");
        assert!(eq_finding.is_some());
        assert!(matches!(eq_finding.unwrap().severity, Severity::C));
    }

    #[test]
    fn inconsistent_equation_fails() {
        let gate = DimensionalConsistency;
        // Mismatched dimension annotations: [L] on left, [L][T] on right
        let data = serde_json::json!({
            "equations": ["[L] = [L][T]"]
        });
        let result = gate.check(&ctx(Some(data)));
        assert!(!result.passed);
        let eq_finding = result.findings.iter().find(|f| f.id == "dimensional_eq_0");
        assert!(matches!(eq_finding.unwrap().severity, Severity::B));
    }
}
