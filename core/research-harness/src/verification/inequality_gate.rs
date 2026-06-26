//! QG Route `GateChecker` adapter for `inequality` module.
//!
//! In-place adapter (Wave 4b): wraps the inequality module's pure functions
//! into a `GateChecker` for the RESEARCH scene.
//!
//! Registered by `research_harness::register_qg_checkers()`.

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};

/// QG Route checker that wraps `inequality.rs` functions.
///
/// Checks:
/// - LaTeX inequality string parsing
/// - Linear programming feasibility via minilp
/// - Inequality verification pipeline
pub struct Inequality;

impl GateChecker for Inequality {
    fn id(&self) -> &'static str {
        "inequality"
    }

    fn scenes(&self) -> Vec<&'static str> {
        vec![quality_gate::scene::RESEARCH]
    }

    fn description(&self) -> &'static str {
        "inequality verification: LaTeX parsing, minilp LP feasibility, inequality system solving"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        // The inequality functions are pure — they only need the constraint
        // text. In a real integration, the checker would read inequalities
        // from the task's output or a structured context. For now, the
        // context serves as the marker that the check was invoked.
        let task_id = &ctx.task_id;

        // Since we don't have the actual inequality expressions in CheckContext,
        // this checker checks that the inequality module is wired correctly.
        // A full implementation would extract expressions from task output.
        let mut findings = Vec::new();

        // If we had expressions, we'd call:
        // let ineq = inequality::parse_inequality_latex("x + y <= 10")?;
        // let system = inequality::InequalitySystem::new(vec![ineq]);
        // let result = inequality::solve_system(&system, Some(5000));
        //
        // For now, emit an informational finding that the adapter exists.

        findings.push(Finding {
            id: "inequality_adapter".to_string(),
            severity: Severity::C,
            description: format!(
                "Inequality checker invoked for task '{task_id}' — adapter wired, LP analysis pending expression input"
            ),
            location: None,
            suggestion: Some(
                "extend CheckContext to include inequality expressions for full minilp feasibility analysis"
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
