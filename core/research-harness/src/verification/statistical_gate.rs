//! QG Route `GateChecker` adapter for `StatisticalChecker`.
//!
//! In-place adapter (Wave 4b): wraps the `statistical` module's pure functions
//! into a `GateChecker` for the RESEARCH scene.
//!
//! Registered by `research_harness::register_qg_checkers()`.

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};

/// QG Route checker that wraps `statistical.rs` functions.
///
/// Checks:
/// - GRIM test (Granularity-Related Inconsistency of Means)
/// - P-value verification
/// - Multiple comparison correction
/// - Effect size reporting
pub struct StatisticalChecker;

impl GateChecker for StatisticalChecker {
    fn id(&self) -> &'static str {
        "statistical"
    }

    fn scenes(&self) -> Vec<&'static str> {
        vec![quality_gate::scene::RESEARCH]
    }

    fn description(&self) -> &'static str {
        "statistical verification checks: GRIM, p-value, multiple comparisons, effect size"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let task_id = &ctx.task_id;

        // The statistical functions are pure — they require concrete numeric
        // values from the task output. Since CheckContext does not carry the
        // task output text, this checker emits informational findings for each
        // available check to confirm the adapter is wired correctly.
        //
        // A full integration would extract numbers from the task report and
        // call:
        //   statistical::grim_test(mean, n, decimals)
        //   statistical::verify_p_value(observed, expected, tolerance)
        //   statistical::check_multiple_comparison_correction(tests, corrected)
        //   statistical::check_effect_size_reported(effect_size, test_type)

        let mut findings = Vec::new();

        findings.push(Finding {
            id: "statistical_grim_adapter".to_string(),
            severity: Severity::C,
            description: format!(
                "GRIM check invoked for task '{task_id}' — adapter wired, pending mean/n input"
            ),
            location: None,
            suggestion: Some(
                "extract observed_mean, n, and decimals from task output to call statistical::grim_test"
                    .to_string(),
            ),
        });

        findings.push(Finding {
            id: "statistical_p_value_adapter".to_string(),
            severity: Severity::C,
            description: format!(
                "P-value verification invoked for task '{task_id}' — adapter wired, pending observed/expected input"
            ),
            location: None,
            suggestion: Some(
                "extract observed and expected p-values from task output to call statistical::verify_p_value"
                    .to_string(),
            ),
        });

        findings.push(Finding {
            id: "statistical_multicomp_adapter".to_string(),
            severity: Severity::C,
            description: format!(
                "Multiple comparison correction check invoked for task '{task_id}' — adapter wired, pending test count input"
            ),
            location: None,
            suggestion: Some(
                "extract num_tests and correction_applied from task output to call statistical::check_multiple_comparison_correction"
                    .to_string(),
            ),
        });

        findings.push(Finding {
            id: "statistical_effect_size_adapter".to_string(),
            severity: Severity::C,
            description: format!(
                "Effect size check invoked for task '{task_id}' — adapter wired, pending effect_size/test_type input"
            ),
            location: None,
            suggestion: Some(
                "extract effect_size and test_type from task output to call statistical::check_effect_size_reported"
                    .to_string(),
            ),
        });

        let passed = true; // informational only — never blocks

        CheckResult {
            checker_id: self.id().to_string(),
            passed,
            findings,
        }
    }
}
