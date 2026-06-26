//! QG Route `GateChecker` adapter for the `asymptotic` module.
//!
//! In-place adapter (Wave 4b): wraps the asymptotic module's pure functions
//! into a `GateChecker` for the RESEARCH scene.
//!
//! Registered by `research_harness::register_qg_checkers()`.

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};

/// QG Route checker that wraps `asymptotic.rs` functions.
///
/// Checks:
/// - Asymptotic chain composition (mixed-chain detection, transitivity)
/// - Magnitude / leading-term estimates (expression ~ leading term as var->oo)
/// - Individual asymptotic claims (f relation g holds or not)
pub struct Asymptotic;

impl GateChecker for Asymptotic {
    fn id(&self) -> &'static str {
        "asymptotic"
    }

    fn scenes(&self) -> Vec<&'static str> {
        vec![quality_gate::scene::RESEARCH]
    }

    fn description(&self) -> &'static str {
        "asymptotic analysis checks: chain composition, magnitude estimation, claim verification"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        // The asymptotic functions are pure and require task-specific
        // expressions / chains.  In a full integration the checker would
        // extract those from the task context.  For now, the context serves
        // as the marker that the check was invoked.
        let task_id = &ctx.task_id;

        // Since we don't have the actual asymptotic expressions in
        // CheckContext, this checker confirms the adapter is wired.
        // A full implementation would read the chain / expression data
        // from the task payload and call into the asymptotic module.
        let mut findings = Vec::new();

        // If we had chain data, we'd call:
        // let comp = asymptotic::compose_asymptotic_chain(&chain);
        // if comp.mixed_chain_warning.is_some() { ... }
        //
        // If we had a claim:
        // let vr = asymptotic::check_asymptotic_claim(f, g, &OrderRelation::MuchLess, var, "oo");
        //
        // If we wanted magnitude:
        // let vr = asymptotic::magnitude_estimate("n^2 + n", "n", "oo");
        //
        // For now, emit an informational finding that the adapter exists.

        findings.push(Finding {
            id: "asymptotic_gate_adapter".to_string(),
            severity: Severity::C,
            description: format!(
                "Asymptotic checker invoked for task '{task_id}' — adapter wired, analysis pending chain/expression input"
            ),
            location: None,
            suggestion: Some(
                "extend CheckContext to include asymptotic chain / expression data for full analysis"
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
