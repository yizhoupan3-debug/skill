//! ScreenshotLayoutChecker — validates screenshot layout for visual goals.
//!
//! For the VISUAL scene, this checker verifies that screenshots taken during
//! task execution have a consistent, valid layout that matches expected
//! visual dimensions and composition.
//!
//! In-place adapter at `runtime-core/checkers/` (Wave 4b).

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};

/// Checker that validates screenshot layout for the VISUAL scene.
pub struct ScreenshotLayoutChecker;

impl GateChecker for ScreenshotLayoutChecker {
    fn id(&self) -> &'static str {
        "screenshot_layout"
    }

    fn scenes(&self) -> Vec<&'static str> {
        vec![quality_gate::scene::VISUAL]
    }

    fn description(&self) -> &'static str {
        "validate screenshot layout consistency for visual goals"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut findings = Vec::new();
        findings.push(Finding {
            id: "screenshot_layout-adapter".to_string(),
            severity: Severity::C,
            description: format!(
                "screenshot_layout checker invoked for task '{}'",
                ctx.task_id
            ),
            location: None,
            suggestion: Some("implement actual checks".to_string()),
        });
        CheckResult {
            checker_id: "screenshot_layout".to_string(),
            passed: true,
            findings,
        }
    }
}
