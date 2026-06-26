//! ProseQcChecker — text quality gate checker for the RESEARCH scene.
//!
//! Validates text quality, clarity, grammar, and structure of research prose.
//! In-place adapter: this module lives in checkers/ and wraps into a `GateChecker`.
//!
//! Wave 5b: verification skill → QG Checker alias for prose-verification.

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};
use quality_gate::scene;

/// Checker that validates prose quality (terminology, style, claim drift, etc.).
pub struct ProseQcChecker;

impl ProseQcChecker {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ProseQcChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl GateChecker for ProseQcChecker {
    fn id(&self) -> &'static str {
        "prose-qc"
    }

    fn scenes(&self) -> Vec<&'static str> {
        vec![scene::RESEARCH]
    }

    fn description(&self) -> &'static str {
        "text quality checks: terminology consistency, style guide compliance, claim drift detection, language register, hedging appropriateness"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut findings = Vec::new();

        findings.push(Finding {
            id: "prose-qc-placeholder".to_string(),
            severity: Severity::C,
            description: format!(
                "prose quality checker invoked for task '{}' — placeholder, implement actual checks",
                ctx.task_id,
            ),
            location: None,
            suggestion: Some("implement actual prose quality checks (terminology, style, claim drift, register, hedging)".to_string()),
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
