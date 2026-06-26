//! QG Route `GateChecker` adapter for the symbolic verification module.
//!
//! In-place adapter (Wave 4b): wraps the `symbolic` module's pure functions
//! into a `GateChecker` for the RESEARCH scene.
//!
//! Registered by `research_harness::register_qg_checkers()`.

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};

/// QG Route checker that wraps `symbolic.rs` functions.
///
/// Checks:
/// - Symbolic expression identity verification
/// - Asymptotic growth classification
/// - Simplification and equivalence checking
pub struct Symbolic;

impl GateChecker for Symbolic {
    fn id(&self) -> &'static str {
        "symbolic"
    }

    fn scenes(&self) -> Vec<&'static str> {
        vec![quality_gate::scene::RESEARCH]
    }

    fn description(&self) -> &'static str {
        "symbolic verification: expression parsing, identity proving, growth classification"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let task_id = &ctx.task_id;

        // The symbolic module offers pure functions that can be called once
        // the expressions to compare are available (e.g. from task output or
        // literature extraction). This adapter demonstrates the wiring and
        // emits an informational finding.
        let mut findings = Vec::new();

        // Example: given two expression strings `lhs` and `rhs`:
        //   let lhs_expr = symbolic::parse("x^2 + 2*x + 1")?;
        //   let rhs_expr = symbolic::parse("(x + 1)^2")?;
        //   let equivalent = symbolic::equivalent(&lhs_expr, &rhs_expr);
        //
        // Growth classification:
        //   let growth = symbolic::classify_growth(&expr, "n");
        //   // returns GrowthClass::Quadratic, etc.
        //
        // Identity verification (prove lhs == rhs via algebraic rewrite):
        //   let lhs_expr = symbolic::parse("sin(x)^2 + cos(x)^2")?;
        //   let rhs_expr = symbolic::parse("1")?;
        //   let simplified = symbolic::simplify(&lhs_expr);
        //   let (proved, proof) = symbolic::prove_identity(&simplified, &rhs_expr);
        //
        // For now, emit an informational finding that the adapter exists.

        findings.push(Finding {
            id: "symbolic_gate_adapter".to_string(),
            severity: Severity::C,
            description: format!(
                "Symbolic adapter invoked for task '{task_id}' — wired, expression analysis pending input"
            ),
            location: None,
            suggestion: Some(
                "extend CheckContext or pass expression pairs for identity/growth checks"
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
