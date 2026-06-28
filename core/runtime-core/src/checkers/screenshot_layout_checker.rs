//! ScreenshotLayoutChecker — validates screenshot layout for visual goals.
//!
//! For the VISUAL scene, this checker verifies that screenshots taken during
//! task execution have a consistent, valid layout that matches expected
//! visual dimensions and composition.
//!
//! **Status: C-level stub** — requires the `image` crate and a reference
//! template system to compare layouts.
//!
//! **Implementation path:**
//! - Load screenshot PNG from task artifacts via `ctx.evidence_path`
//! - Parse image dimensions and basic layout regions (header, body, footer)
//! - Compare against reference templates or expected aspect ratios
//! - Check for common issues: text overflow, clipping, aspect ratio distortion
//! - Emit B-level findings for layout inconsistencies

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult};

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
        let _ = ctx; // unused while checker is a stub
        CheckResult {
            checker_id: "screenshot_layout".to_string(),
            passed: true,
            findings: Vec::new(),
        }
    }
}
