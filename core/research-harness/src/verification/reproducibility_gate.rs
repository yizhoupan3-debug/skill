//! QG Route `GateChecker` adapter for the `reproducibility` module.
//!
//! In-place adapter (Wave 5b): wraps the reproducibility module's audit
//! functions into a `GateChecker` for the RESEARCH scene.
//!
//! Registered by `research_harness::register_qg_checkers()`.

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};

use super::reproducibility;

/// QG Route checker that wraps `reproducibility.rs` audit functions.
///
/// Checks:
/// - Random seed is set and non-random (missing = P0 blocker)
/// - Deterministic rerun hash consistency (mismatch = FAIL)
/// - Environment reproducibility (lock file present and in sync)
/// - Data versioning via DVC/Git LFS (missing = WARN)
/// - Checkpoint recoverability (failure = P0 blocker)
pub struct Reproducibility;

impl GateChecker for Reproducibility {
    fn id(&self) -> &'static str {
        "reproducibility"
    }

    fn sub_scene_affinity(&self) -> Option<&'static str> {
        Some("reproducibility")
    }

    fn description(&self) -> &'static str {
        "experiment reproducibility audit: seed, determinism, environment lock, data versioning, checkpoint recovery"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut findings = Vec::new();

        // Run the full reproducibility audit using the experiment_dir from context.
        // The reproducibility module requires filesystem paths; we use the task's
        // repo_root as the experiment directory base.
        let experiment_dir = std::path::Path::new(&ctx.repo_root);

        match reproducibility::run_reproducibility_audit(experiment_dir, None) {
            Ok(report) => {
                for result in &report.checks {
                    let (severity, passed) = match &result.status {
                        reproducibility::CheckStatus::Pass => (Severity::C, true),
                        reproducibility::CheckStatus::Fail(_msg) => {
                            // Seed missing and checkpoint failure are P0 blockers
                            let sev = if result.name == "seed_set"
                                || result.name == "checkpoint_recoverable"
                            {
                                Severity::P0
                            } else {
                                Severity::B
                            };
                            (sev, false)
                        }
                        reproducibility::CheckStatus::Warn(_) => (Severity::C, true),
                        reproducibility::CheckStatus::Skip(_) => (Severity::C, true),
                    };

                    if !passed || matches!(severity, Severity::P0 | Severity::A | Severity::B) {
                        findings.push(Finding {
                            id: format!("reproducibility_{}", result.name),
                            severity,
                            description: match &result.status {
                                reproducibility::CheckStatus::Fail(msg) => {
                                    format!("{}: FAIL — {}", result.name, msg)
                                }
                                reproducibility::CheckStatus::Warn(msg) => {
                                    format!("{}: WARN — {}", result.name, msg)
                                }
                                reproducibility::CheckStatus::Skip(msg) => {
                                    format!("{}: SKIP — {}", result.name, msg)
                                }
                                reproducibility::CheckStatus::Pass => {
                                    format!("{}: PASS", result.name)
                                }
                            },
                            location: None,
                            suggestion: None,
                        });
                    }
                }
            }
            Err(e) => {
                findings.push(Finding {
                    id: "reproducibility_audit_error".to_string(),
                    severity: Severity::B,
                    description: format!("Reproducibility audit failed to run: {e}"),
                    location: None,
                    suggestion: Some(
                        "ensure experiment directory exists and is accessible".to_string(),
                    ),
                });
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
