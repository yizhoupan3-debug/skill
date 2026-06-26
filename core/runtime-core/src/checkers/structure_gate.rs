//! StructureGateChecker — document structure verification for the RESEARCH scene.
//!
//! Validates document structure against standards (LaTeX, cross-references, format).
//! In-place adapter: this module lives in checkers/ and wraps into a `GateChecker`.
//!
//! Wave 5b: verification skill → QG Checker alias for structure-verification.

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};
use quality_gate::scene;

/// Checker that validates document structure.
pub struct StructureGateChecker;

impl StructureGateChecker {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StructureGateChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl GateChecker for StructureGateChecker {
    fn id(&self) -> &'static str {
        "structure-gate"
    }

    fn scenes(&self) -> Vec<&'static str> {
        vec![scene::RESEARCH]
    }

    fn description(&self) -> &'static str {
        "structure verification: LaTeX compilation, cross-reference consistency, claim-evidence alignment, format compliance, notation consistency, equation numbering"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut findings = Vec::new();

        findings.push(Finding {
            id: "structure-gate-placeholder".to_string(),
            severity: Severity::C,
            description: format!(
                "structure gate checker invoked for task '{}' — placeholder, implement actual checks",
                ctx.task_id,
            ),
            location: None,
            suggestion: Some("implement actual structure checks (LaTeX, cross-references, format, notation)".to_string()),
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
