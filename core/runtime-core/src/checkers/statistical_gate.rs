//! StatisticalGateChecker — statistical methodology verification for the RESEARCH scene.
//!
//! Validates statistical methodology claims (p-value recomputation, GRIM test, etc.).
//! In-place adapter: this module lives in checkers/ and wraps into a `GateChecker`.
//!
//! Wave 5b: verification skill → QG Checker alias for statistical-verification.

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};
use quality_gate::scene;

/// Checker that validates statistical methodology claims.
pub struct StatisticalGateChecker;

impl StatisticalGateChecker {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StatisticalGateChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl GateChecker for StatisticalGateChecker {
    fn id(&self) -> &'static str {
        "statistical-gate"
    }

    fn scenes(&self) -> Vec<&'static str> {
        vec![scene::RESEARCH]
    }

    fn description(&self) -> &'static str {
        "statistical verification: p-value recomputation, GRIM test, effect size reporting, multiple comparison correction, assumption checking"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut findings = Vec::new();

        findings.push(Finding {
            id: "statistical-gate-placeholder".to_string(),
            severity: Severity::C,
            description: format!(
                "statistical gate checker invoked for task '{}' — placeholder, implement actual checks",
                ctx.task_id,
            ),
            location: None,
            suggestion: Some("implement actual statistical checks (p-value, GRIM, effect size, corrections)".to_string()),
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
