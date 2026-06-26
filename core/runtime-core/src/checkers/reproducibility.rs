//! ReproducibilityChecker — reproducibility verification for the RESEARCH scene.
//!
//! Validates whether results can be reproduced (seeds, determinism, environment,
//! data versioning, checkpoint recovery).
//! In-place adapter: this module lives in checkers/ and wraps into a `GateChecker`.
//!
//! Wave 5b: verification skill → QG Checker alias for reproducibility-verification.

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};
use quality_gate::scene;

/// Checker that validates experimental reproducibility.
pub struct ReproducibilityChecker;

impl ReproducibilityChecker {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReproducibilityChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl GateChecker for ReproducibilityChecker {
    fn id(&self) -> &'static str {
        "reproducibility"
    }

    fn scenes(&self) -> Vec<&'static str> {
        vec![scene::RESEARCH]
    }

    fn description(&self) -> &'static str {
        "reproducibility verification: seed locking, deterministic rerun, environment pinning, data versioning, checkpoint recovery"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut findings = Vec::new();

        findings.push(Finding {
            id: "reproducibility-placeholder".to_string(),
            severity: Severity::C,
            description: format!(
                "reproducibility checker invoked for task '{}' — placeholder, implement actual checks",
                ctx.task_id,
            ),
            location: None,
            suggestion: Some("implement actual reproducibility checks (seeds, determinism, environment, data)".to_string()),
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
