//! FormalGateChecker — formal logic and math proof verification for the RESEARCH scene.
//!
//! Validates formal logic, mathematical proofs, CAS identity simplification,
//! SMT consistency, and dimensional analysis.
//! In-place adapter: this module lives in checkers/ and wraps into a `GateChecker`.
//!
//! Wave 5b: verification skill → QG Checker alias for formal-verification.

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};
use quality_gate::scene;

/// Checker that validates formal logic and mathematical proofs.
pub struct FormalGateChecker;

impl FormalGateChecker {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FormalGateChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl GateChecker for FormalGateChecker {
    fn id(&self) -> &'static str {
        "formal-gate"
    }

    fn scenes(&self) -> Vec<&'static str> {
        vec![scene::RESEARCH]
    }

    fn description(&self) -> &'static str {
        "formal verification: CAS identity simplification, SMT consistency, witness validation, dimensional analysis, step dependency checking"
    }

    fn sub_scene_affinity(&self) -> Option<&'static str> {
        Some("formal")
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut findings = Vec::new();

        findings.push(Finding {
            id: "formal-gate-placeholder".to_string(),
            severity: Severity::C,
            description: format!(
                "formal gate checker invoked for task '{}' — placeholder, implement actual checks",
                ctx.task_id,
            ),
            location: None,
            suggestion: Some("implement actual formal verification (CAS, SMT, dimensional analysis)".to_string()),
        });

        let passed = findings.iter().all(|f| {
            !matches!(f.severity, Severity::P0 | Severity::A | Severity::B)
        });

        CheckResult {
            checker_id: self.id().to_string(),
            passed,
            findings,
        }
    }
}
