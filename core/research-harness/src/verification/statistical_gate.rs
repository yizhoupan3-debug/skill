//! QG Route `GateChecker` adapter for `StatisticalChecker`.
//!
//! Extracts structured statistical data from `CheckContext::output_data` and calls
//! the underlying pure functions in `verification::statistical`.
//!
//! Expected `output_data` JSON (all fields optional):
//! ```json
//! {
//!   "grim": { "mean": 3.50, "n": 100, "decimals": 2 },
//!   "p_value": { "observed": 0.03, "expected": 0.05, "tolerance": 0.01 },
//!   "multiple_comparison": { "num_tests": 5, "correction_applied": true },
//!   "effect_size": { "effect_size": 0.5, "test_type": "t-test" }
//! }
//! ```

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};

use crate::verification::statistical;

pub struct StatisticalChecker;

impl GateChecker for StatisticalChecker {
    fn id(&self) -> &'static str {
        "statistical"
    }

    fn description(&self) -> &'static str {
        "statistical verification: GRIM, p-value, multiple comparisons, effect size"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut findings = Vec::new();

        let Some(data) = ctx.output_data.as_ref() else {
            findings.push(Finding {
                id: "statistical_no_data".to_string(),
                severity: Severity::C,
                description: "No output_data provided — statistical checks skipped".to_string(),
                location: None,
                suggestion: Some(
                    "pass output_data with statistical keys (grim, p_value, etc.) to enable checks"
                        .to_string(),
                ),
            });
            return CheckResult {
                checker_id: self.id().to_string(),
                passed: true,
                findings,
            };
        };

        // GRIM test
        if let Some(grim) = data.get("grim") {
            let mean = grim.get("mean").and_then(|v| v.as_f64());
            let n = grim.get("n").and_then(|v| v.as_u64());
            let decimals = grim.get("decimals").and_then(|v| v.as_u64()).unwrap_or(2) as usize;

            if mean.is_none() {
                findings.push(Finding {
                    id: "statistical_grim_missing_mean".to_string(),
                    severity: Severity::Warning,
                    description: "GRIM test skipped: 'mean' field missing in output_data.grim".to_string(),
                    location: None,
                    suggestion: Some("provide 'mean' as a float in output_data.grim".to_string()),
                });
            } else if n.is_none() {
                findings.push(Finding {
                    id: "statistical_grim_missing_n".to_string(),
                    severity: Severity::Warning,
                    description: "GRIM test skipped: 'n' field missing in output_data.grim".to_string(),
                    location: None,
                    suggestion: Some("provide 'n' as an integer in output_data.grim".to_string()),
                });
            } else if n == Some(0) {
                findings.push(Finding {
                    id: "statistical_grim_zero_n".to_string(),
                    severity: Severity::Warning,
                    description: "GRIM test skipped: n=0 is invalid".to_string(),
                    location: None,
                    suggestion: Some("provide a positive integer for 'n' in output_data.grim".to_string()),
                });
            } else {
                let mean = mean.unwrap();
                let n = n.unwrap() as usize;
                match statistical::grim_test(mean, n, decimals) {
                    Ok(passed) => {
                        findings.push(Finding {
                            id: "statistical_grim".to_string(),
                            severity: if passed { Severity::C } else { Severity::B },
                            description: format!(
                                "GRIM test for mean={mean}, n={n}, decimals={decimals}: {}",
                                if passed { "consistent" } else { "inconsistent" }
                            ),
                            location: None,
                            suggestion: if passed {
                                None
                            } else {
                                Some(
                                    "mean is not granular-compatible with sample size — check data"
                                        .to_string(),
                                )
                            },
                        });
                    }
                    Err(e) => {
                        findings.push(Finding {
                            id: "statistical_grim_error".to_string(),
                            severity: Severity::C,
                            description: format!("GRIM test error: {e}"),
                            location: None,
                            suggestion: None,
                        });
                    }
                }
            }
        }

        // P-value verification
        if let Some(pv) = data.get("p_value") {
            let observed = pv.get("observed").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let expected = pv.get("expected").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let tolerance = pv.get("tolerance").and_then(|v| v.as_f64()).unwrap_or(0.01);
            let passed = statistical::verify_p_value(observed, expected, tolerance);
            findings.push(Finding {
                id: "statistical_p_value".to_string(),
                severity: if passed { Severity::C } else { Severity::B },
                description: format!(
                    "P-value verification: observed={observed}, expected={expected}, tolerance={tolerance}: {}",
                    if passed { "consistent" } else { "inconsistent" }
                ),
                location: None,
                suggestion: if passed { None } else {
                    Some("reported p-value does not match expected value within tolerance".to_string())
                },
            });
        }

        // Multiple comparison correction
        if let Some(mc) = data.get("multiple_comparison") {
            let num_tests = mc.get("num_tests").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let correction_applied = mc
                .get("correction_applied")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let passed =
                statistical::check_multiple_comparison_correction(num_tests, correction_applied);
            findings.push(Finding {
                id: "statistical_multicomp".to_string(),
                severity: if passed { Severity::C } else { Severity::Warning },
                description: format!(
                    "Multiple comparison: num_tests={num_tests}, correction_applied={correction_applied}: {}",
                    if passed { "ok" } else { "missing correction" }
                ),
                location: None,
                suggestion: if passed { None } else {
                    Some("apply Bonferroni or FDR correction for multiple tests".to_string())
                },
            });
        }

        // Effect size reporting
        if let Some(es) = data.get("effect_size") {
            let effect_size = es.get("effect_size").and_then(|v| v.as_f64());
            let test_type = es
                .get("test_type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let reported = statistical::check_effect_size_reported(effect_size, test_type);
            findings.push(Finding {
                id: "statistical_effect_size".to_string(),
                severity: if reported {
                    Severity::C
                } else {
                    Severity::Warning
                },
                description: format!(
                    "Effect size for {test_type}: {}",
                    if reported { "reported" } else { "missing" }
                ),
                location: None,
                suggestion: if reported {
                    None
                } else {
                    Some(format!("report effect size for {test_type} test"))
                },
            });
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
        let gate = StatisticalChecker;
        let result = gate.check(&ctx(None));
        assert!(result.passed);
        assert_eq!(result.findings.len(), 1);
        assert_eq!(result.findings[0].id, "statistical_no_data");
        assert!(matches!(result.findings[0].severity, Severity::C));
    }

    #[test]
    fn grim_consistent_passes() {
        let gate = StatisticalChecker;
        // mean=3.5, n=100, decimals=2 → sum=350, round(350)=350, reconstructed=3.50 → consistent
        let data = serde_json::json!({
            "grim": { "mean": 3.50, "n": 100, "decimals": 2 }
        });
        let result = gate.check(&ctx(Some(data)));
        assert!(result.passed);
        let grim = result.findings.iter().find(|f| f.id == "statistical_grim");
        assert!(matches!(grim.unwrap().severity, Severity::C));
    }

    #[test]
    fn p_value_consistent_passes() {
        let gate = StatisticalChecker;
        let data = serde_json::json!({
            "p_value": { "observed": 0.05, "expected": 0.05, "tolerance": 0.01 }
        });
        let result = gate.check(&ctx(Some(data)));
        assert!(result.passed);
    }

    #[test]
    fn p_value_inconsistent_fails() {
        let gate = StatisticalChecker;
        let data = serde_json::json!({
            "p_value": { "observed": 0.03, "expected": 0.95, "tolerance": 0.01 }
        });
        let result = gate.check(&ctx(Some(data)));
        assert!(!result.passed);
        let pv = result.findings.iter().find(|f| f.id == "statistical_p_value");
        assert!(matches!(pv.unwrap().severity, Severity::B));
    }
}
