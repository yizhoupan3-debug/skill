//! EvidenceGate — validates that task evidence exists before allowing completion.
//!
//! Scans EVIDENCE_INDEX.json for the active task and checks that at least one
//! artifact has `exit_code == 0` or `success == true`.
//!
//! In-place adapter (Wave 4b): this module lives in checkers/ and wraps the
//! core-state evidence APIs into a `GateChecker`.

use quality_gate::checker::GateChecker;
use quality_gate::scene;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};

/// Checker that verifies task evidence exists and is valid.
pub struct EvidenceChecker;

impl GateChecker for EvidenceChecker {
    fn id(&self) -> &'static str {
        "evidence"
    }

    fn scenes(&self) -> Vec<&'static str> {
        vec![scene::GENERAL, scene::CODE_REVIEW, scene::RESEARCH]
    }

    fn description(&self) -> &'static str {
        "verify that task evidence artifacts exist and indicate success"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        // NOTE: This checker duplicates QGEntry Stage 1's anti-fraud evidence
        //       gate check (qg_entry::trigger). Stage 1 blocks on P0 when
        //       evidence exists but none indicates success. This checker runs
        //       as part of the quality-gate scene dispatch (Stage 2) and
        //       produces Warning/C findings for the same condition. Both checks
        //       are currently intentional — Stage 1 is the hard gate, this
        //       checker provides non-blocking advisory feedback for the report.
        let repo_root = &ctx.repo_root;
        let task_id = &ctx.task_id;

        let (has_evidence, evidence_ok) =
            core_state::state_manager::task_evidence_artifacts_summary_for_task(repo_root, task_id);

        let mut findings = Vec::new();

        if !has_evidence {
            findings.push(Finding {
                id: "no-evidence".to_string(),
                severity: Severity::Warning,
                description: format!(
                    "task '{task_id}' has no evidence artifacts — no evidence of completion"
                ),
                location: None,
                suggestion: Some(
                    "record evidence via append_evidence before completing".to_string(),
                ),
            });
        } else if !evidence_ok {
            findings.push(Finding {
                id: "evidence-not-ok".to_string(),
                severity: Severity::C,
                description: format!(
                    "task '{task_id}' has evidence artifacts but none indicate success"
                ),
                location: None,
                suggestion: Some(
                    "ensure at least one artifact has exit_code=0 or success=true".to_string(),
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
