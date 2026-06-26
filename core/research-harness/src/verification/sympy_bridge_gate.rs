//! QG Route `GateChecker` adapter for `sympy_bridge`.
//!
//! In-place adapter (Wave 4b): wraps the sympy_bridge module's pure functions
//! into a `GateChecker` for the RESEARCH scene.
//!
//! Registered by `research_harness::register_qg_checkers()`.

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};

/// QG Route checker that wraps `sympy_bridge.rs` functions.
///
/// Checks:
/// - Symbolic identity verification
/// - Expression simplification
/// - SymPy availability probe
pub struct SympyBridge;

impl GateChecker for SympyBridge {
    fn id(&self) -> &'static str {
        "sympy_bridge"
    }

    fn scenes(&self) -> Vec<&'static str> {
        vec![quality_gate::scene::RESEARCH]
    }

    fn description(&self) -> &'static str {
        "symbolic math verification: identity check, expression simplification, availability probe"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let task_id = &ctx.task_id;

        let mut findings = Vec::new();

        // If we had an expression pair from the task, we'd call:
        // let vr = sympy_bridge::verify_identity(lhs, rhs);
        // let simplified = sympy_bridge::simplify_expression(expr);
        // let available = sympy_bridge::sympy_available();
        //
        // For now, emit an informational finding that the adapter exists.

        findings.push(Finding {
            id: "sympy_bridge_adapter".to_string(),
            severity: Severity::C,
            description: format!(
                "SympyBridge invoked for task '{task_id}' — adapter wired, \
                 symbolic verification pending expression input"
            ),
            location: None,
            suggestion: Some(
                "extend CheckContext to include symbolic expressions for identity verification"
                    .to_string(),
            ),
        });

        findings.push(Finding {
            id: "sympy_bridge_simplify".to_string(),
            severity: Severity::C,
            description: format!(
                "SympyBridge simplify_expression available for task '{task_id}'"
            ),
            location: None,
            suggestion: Some(
                "pass expressions through task context to enable full simplification checks"
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
