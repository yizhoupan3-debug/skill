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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use quality_gate::checker::GateChecker;
    use quality_gate::types::CheckContext;

    fn ctx_with_repo(repo_root: std::path::PathBuf) -> CheckContext {
        CheckContext {
            scene: "test".into(),
            sub_scene: None,
            goal: "test".into(),
            round: 1,
            repo_root,
            task_id: "t1".into(),
            evidence_path: None,
            runtime_handle: None,
            output_data: None,
            evaluated_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn empty_dir_runs_audit() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let gate = Reproducibility;
        let result = gate.check(&ctx_with_repo(dir.path().to_path_buf()));
        // The audit runs; seed_set will FAIL since no code files exist
        assert!(!result.passed);
        let seed_finding = result
            .findings
            .iter()
            .find(|f| f.id == "reproducibility_seed_set");
        assert!(seed_finding.is_some());
        assert!(matches!(seed_finding.unwrap().severity, Severity::P0));
    }

    #[test]
    fn seed_present_passes_gate() {
        let dir = tempfile::tempdir().expect("create tempdir");
        // Create a Python file with a seed setting
        std::fs::write(dir.path().join("train.py"), "import torch\ntorch.manual_seed(42)\n")
            .expect("write py file");
        let gate = Reproducibility;
        let result = gate.check(&ctx_with_repo(dir.path().to_path_buf()));
        // seed_set should PASS; other checks may vary
        let seed_finding = result
            .findings
            .iter()
            .find(|f| f.id == "reproducibility_seed_set");
        assert!(seed_finding.is_none());
    }

    #[test]
    fn findings_severity_mapping() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let gate = Reproducibility;
        let result = gate.check(&ctx_with_repo(dir.path().to_path_buf()));
        // Verify that seed_set failure maps to P0 severity
        for finding in &result.findings {
            if finding.id == "reproducibility_seed_set" {
                assert!(matches!(finding.severity, Severity::P0));
            }
        }
    }
}
