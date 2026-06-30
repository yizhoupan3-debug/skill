//! QG Route `GateChecker` adapter for `inequality` module.
//!
//! Extracts LaTeX inequality expressions from `CheckContext::output_data` and
//! runs the LP feasibility solver.
//!
//! Expected `output_data` JSON:
//! ```json
//! {
//!   "inequalities": ["x + y <= 10", "x >= 0", "y >= 0"],
//!   "timeout_ms": 5000
//! }
//! ```

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};

use crate::verification::inequality::{self, FeasibilityResult, InequalitySystem};

pub struct Inequality;

impl GateChecker for Inequality {
    fn id(&self) -> &'static str {
        "inequality"
    }

    fn description(&self) -> &'static str {
        "inequality verification: LaTeX parsing, LP feasibility, system solving"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut findings = Vec::new();

        let Some(data) = ctx.output_data.as_ref() else {
            findings.push(Finding {
                id: "inequality_no_data".to_string(),
                severity: Severity::C,
                description: "No output_data provided — inequality checks skipped".to_string(),
                location: None,
                suggestion: Some(
                    "pass output_data with inequalities array to enable checks".to_string(),
                ),
            });
            return CheckResult {
                checker_id: self.id().to_string(),
                passed: true,
                findings,
            };
        };

        if !inequality::solver_available() {
            findings.push(Finding {
                id: "inequality_solver_unavailable".to_string(),
                severity: Severity::C,
                description: "minilp solver not available — inequality checks degraded".to_string(),
                location: None,
                suggestion: Some(
                    "install minilp feature to enable full LP feasibility checks".to_string(),
                ),
            });
            return CheckResult {
                checker_id: self.id().to_string(),
                passed: true,
                findings,
            };
        }

        let Some(exprs) = data.get("inequalities").and_then(|v| v.as_array()) else {
            findings.push(Finding {
                id: "inequality_no_expressions".to_string(),
                severity: Severity::C,
                description: "output_data has no inequalities array — check skipped".to_string(),
                location: None,
                suggestion: Some(
                    "add \"inequalities\": [\"x + y <= 10\", ...] to output_data".to_string(),
                ),
            });
            return CheckResult {
                checker_id: self.id().to_string(),
                passed: true,
                findings,
            };
        };

        let timeout_ms = data
            .get("timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(5000);

        // Parse all inequality expressions
        let mut parsed = Vec::new();
        for (i, expr_val) in exprs.iter().enumerate() {
            let Some(expr_str) = expr_val.as_str() else {
                continue;
            };
            match inequality::parse_inequality_latex(expr_str) {
                Ok(ineq) => parsed.push(ineq),
                Err(e) => {
                    findings.push(Finding {
                        id: format!("inequality_parse_{i}"),
                        severity: Severity::Warning,
                        description: format!("Cannot parse inequality '{expr_str}': {e}"),
                        location: Some(format!("inequalities[{i}]")),
                        suggestion: Some("check LaTeX inequality syntax".to_string()),
                    });
                }
            }
        }

        if parsed.is_empty() {
            findings.push(Finding {
                id: "inequality_no_parseable".to_string(),
                severity: Severity::C,
                description: "No parseable inequalities found — check skipped".to_string(),
                location: None,
                suggestion: None,
            });
            return CheckResult {
                checker_id: self.id().to_string(),
                passed: true,
                findings,
            };
        }

        // Solve the system
        let system = InequalitySystem::new(parsed);
        let result = inequality::solve_system(&system, Some(timeout_ms));

        match &result {
            FeasibilityResult::Feasible { model } => {
                let vars: Vec<String> = model.iter().map(|(k, v)| format!("{k}={v:.3}")).collect();
                findings.push(Finding {
                    id: "inequality_feasible".to_string(),
                    severity: Severity::C,
                    description: format!(
                        "Inequality system is feasible ({} constraints): {}",
                        system.len(),
                        vars.join(", ")
                    ),
                    location: None,
                    suggestion: None,
                });
            }
            FeasibilityResult::Infeasible { proof_certificate } => {
                findings.push(Finding {
                    id: "inequality_infeasible".to_string(),
                    severity: Severity::B,
                    description: format!(
                        "Inequality system is infeasible ({} constraints): {proof_certificate}",
                        system.len()
                    ),
                    location: None,
                    suggestion: Some(
                        "review inequality constraints for contradictions".to_string(),
                    ),
                });
            }
            FeasibilityResult::Timeout { timeout_ms: t } => {
                findings.push(Finding {
                    id: "inequality_timeout".to_string(),
                    severity: Severity::Warning,
                    description: format!("Inequality solver timed out after {t}ms"),
                    location: None,
                    suggestion: Some(
                        "increase timeout_ms or simplify the constraint system".to_string(),
                    ),
                });
            }
            FeasibilityResult::Error { message } => {
                findings.push(Finding {
                    id: "inequality_error".to_string(),
                    severity: Severity::C,
                    description: format!("Inequality solver error: {message}"),
                    location: None,
                    suggestion: None,
                });
            }
            FeasibilityResult::Warn { message } => {
                findings.push(Finding {
                    id: "inequality_warn".to_string(),
                    severity: Severity::Warning,
                    description: format!("Inequality solver warning: {message}"),
                    location: None,
                    suggestion: None,
                });
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
        let gate = Inequality;
        let result = gate.check(&ctx(None));
        assert!(result.passed);
        assert_eq!(result.findings.len(), 1);
        assert_eq!(result.findings[0].id, "inequality_no_data");
        assert!(matches!(result.findings[0].severity, Severity::C));
    }

    #[test]
    fn no_inequalities_array_returns_passed() {
        let gate = Inequality;
        let data = serde_json::json!({ "timeout_ms": 5000 });
        let result = gate.check(&ctx(Some(data)));
        assert!(result.passed);
        assert_eq!(result.findings[0].id, "inequality_no_expressions");
    }

    #[test]
    fn feasible_system_passes() {
        let gate = Inequality;
        let data = serde_json::json!({
            "inequalities": ["x <= 10", "x >= 0"]
        });
        let result = gate.check(&ctx(Some(data)));
        assert!(result.passed);
        assert!(result
            .findings
            .iter()
            .any(|f| f.id == "inequality_feasible"));
    }

    #[test]
    fn infeasible_system_fails() {
        let gate = Inequality;
        // Contradictory constraints: x >= 5 AND x <= 3
        let data = serde_json::json!({
            "inequalities": ["x >= 5", "x <= 3"]
        });
        let result = gate.check(&ctx(Some(data)));
        assert!(!result.passed);
        assert!(result
            .findings
            .iter()
            .any(|f| f.id == "inequality_infeasible"));
    }
}
