//! ScreenshotLayoutChecker — validates screenshot evidence for visual goals.
//!
//! For the VISUAL scene, this checker verifies that screenshot artifacts
//! exist and are valid PNG files (magic bytes check).
//!
//! Does not require the `image` crate — only checks file existence and
//! PNG header magic bytes (89 50 4E 47 0D 0A 1A 0A).

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};

/// PNG magic bytes.
const PNG_MAGIC: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

/// Checker that validates screenshot evidence for the VISUAL scene.
pub struct ScreenshotLayoutChecker;

impl GateChecker for ScreenshotLayoutChecker {
    fn id(&self) -> &'static str {
        "screenshot_layout"
    }

    fn description(&self) -> &'static str {
        "validate screenshot evidence exists and is a valid PNG file"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut findings = Vec::new();

        let Some(ref ev_path) = ctx.evidence_path else {
            findings.push(Finding::new("screenshot_no_evidence_path", Severity::Warning,
                "no evidence_path provided — cannot verify screenshot output")
                .with_suggestion("provide evidence_path pointing to the screenshot artifact"));
            return CheckResult {
                checker_id: self.id().to_string(),
                passed: true,
                findings,
            };
        };

        if !ev_path.is_file() {
            findings.push(Finding::new("screenshot_evidence_missing", Severity::Warning,
                format!("evidence file not found at {}", ev_path.display()))
                .with_location(ev_path.display().to_string())
                .with_suggestion("ensure the screenshot was saved before running the gate"));
            return CheckResult {
                checker_id: self.id().to_string(),
                passed: true,
                findings,
            };
        }

        // Read first 8 bytes to check PNG magic
        match std::fs::read(ev_path) {
            Ok(bytes) => {
                if bytes.len() < 8 {
                    findings.push(Finding::new("screenshot_file_too_small", Severity::B,
                        format!("evidence file is only {} bytes — not a valid image", bytes.len()))
                        .with_location(ev_path.display().to_string())
                        .with_suggestion("verify the screenshot was captured correctly"));
                } else if bytes[..8] != *PNG_MAGIC {
                    // Not PNG — could be JPEG (FF D8 FF), WEBP, etc. — flag as advisory
                    let magic_hex = bytes[..4]
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<Vec<_>>()
                        .join(" ");
                    findings.push(Finding::new("screenshot_not_png", Severity::C,
                        format!("evidence file magic bytes ({magic_hex}) are not PNG — format may not be supported by downstream tools"))
                        .with_location(ev_path.display().to_string())
                        .with_suggestion("convert screenshot to PNG format for consistent processing"));
                }
                // PNG magic matches — no findings, gate passes cleanly
            }
            Err(e) => {
                findings.push(Finding::new("screenshot_read_error", Severity::C,
                    format!("cannot read evidence file: {e}"))
                    .with_location(ev_path.display().to_string()));
            }
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
