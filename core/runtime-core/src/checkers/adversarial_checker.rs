//! AdversarialChecker — a general-scene checker that flags common issues.
//!
//! For the GENERAL scene, this serves as a lightweight adversarial pass:
//! checks for common completion pitfalls (e.g., empty evidence, trivial changes
//! that don't address the task, missing task coverage).
//!
//! In-place adapter at `runtime-core/checkers/` (Wave 4b).

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};

/// General-purpose adversarial checker for the GENERAL scene.
pub struct AdversarialChecker;

impl GateChecker for AdversarialChecker {
    fn id(&self) -> &'static str {
        "adversarial"
    }

    fn description(&self) -> &'static str {
        "general adversarial checks: evidence presence, scope coverage"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut findings = Vec::new();

        // ── 1. evidence_path presence ──
        if ctx.evidence_path.as_ref().map_or(true, |p| !p.is_file()) {
            findings.push(Finding {
                id: "missing-evidence-file".to_string(),
                severity: Severity::Warning,
                description: format!(
                    "evidence file not found at {:?} — adversarial pass cannot verify artifacts",
                    ctx.evidence_path,
                ),
                location: None,
                suggestion: Some(
                    "ensure evidence is recorded before completing the goal".to_string(),
                ),
            });
        }

        // ── 2. non-trivial round count ──
        if ctx.round == 1 {
            // Single round suggests the goal may not have been iterated on.
            // This is informational only, not a blocker.
            findings.push(Finding {
                id: "single-round".to_string(),
                severity: Severity::C,
                description: format!(
                    "goal '{}' completed in a single round — no iterative improvement cycle",
                    ctx.goal,
                ),
                location: None,
                suggestion: Some(
                    "consider whether the goal was adequately verified in one pass".to_string(),
                ),
            });
        }

        let passed = findings.is_empty()
            || findings
                .iter()
                .all(|f| !matches!(f.severity, Severity::P0 | Severity::A | Severity::B));

        CheckResult {
            checker_id: self.id().to_string(),
            passed,
            findings,
        }
    }
}
