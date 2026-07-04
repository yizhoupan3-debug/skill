//! AdversarialChecker — a general-scene checker that flags common issues.
//!
//! For the GENERAL scene, this serves as a lightweight adversarial pass:
//! checks for common completion pitfalls (e.g., empty evidence, trivial changes
//! that don't address the task, missing task coverage).
//!
//! In-place adapter at `runtime-core/checkers/` (Wave 4b).

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};
use serde_json::Value;
use std::fs;

/// General-purpose adversarial checker for the GENERAL scene.
pub struct AdversarialChecker;

impl GateChecker for AdversarialChecker {
    fn id(&self) -> &'static str {
        "adversarial"
    }

    fn description(&self) -> &'static str {
        "general adversarial checks: evidence presence, scope coverage, build/test results"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut findings = Vec::new();

        // Defense-in-depth: validate task_id before filesystem access
        if core_state_utils::path_guard::safe_task_id_component(&ctx.task_id).is_none() {
            findings.push(Finding::new("invalid-task-id", Severity::P0,
                format!("task_id '{}' contains unsafe characters", ctx.task_id)));
            return CheckResult {
                checker_id: self.id().to_string(),
                passed: false,
                findings,
            };
        }

        let task_dir = ctx.repo_root.join("artifacts/current").join(&ctx.task_id);

        // ── 1. evidence_path presence ──
        if ctx.evidence_path.as_ref().map_or(true, |p| !p.is_file()) {
            findings.push(Finding::new("missing-evidence-file", Severity::Warning,
                format!("evidence file not found at {:?} — adversarial pass cannot verify artifacts", ctx.evidence_path))
                .with_suggestion("ensure evidence is recorded before completing the goal"));
        }

        // ── 2. non-trivial round count ──
        if ctx.round == 1 {
            findings.push(Finding::new("single-round", Severity::C,
                format!("goal '{}' completed in a single round — no iterative improvement cycle", ctx.goal))
                .with_suggestion("consider whether the goal was adequately verified in one pass"));
        }

        // ── 3. Check for build/test evidence ──
        let has_build_evidence = fs::read_to_string(task_dir.join("EVIDENCE_INDEX.json"))
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            .and_then(|v| v.get("artifacts").cloned())
            .and_then(|a| a.as_array().cloned())
            .map(|artifacts| {
                artifacts.iter().any(|a| {
                    let source = a.get("source").and_then(Value::as_str).unwrap_or("");
                    let kind = a.get("kind").and_then(Value::as_str).unwrap_or("");
                    source.contains("cargo") || source.contains("test") || source.contains("build")
                        || kind.contains("test") || kind.contains("build")
                        || source.contains("npm") || source.contains("pytest")
                })
            })
            .unwrap_or(false);

        if !has_build_evidence && ctx.round > 1 {
            findings.push(Finding::new("no-build-evidence", Severity::C,
                format!("goal '{}' completed in {} rounds but no build/test evidence recorded", ctx.goal, ctx.round))
                .with_suggestion("run tests/build and record results as evidence before completing"));
        }

        // ── 4. Check TASK_OUTPUT.json for structured results ──
        let task_output_path = task_dir.join("TASK_OUTPUT.json");
        let has_structured_output = fs::read_to_string(&task_output_path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            .map(|v| {
                v.get("results").and_then(Value::as_array).map_or(false, |a| !a.is_empty())
                    || v.get("outputs").and_then(Value::as_array).map_or(false, |a| !a.is_empty())
            })
            .unwrap_or(false);

        if !has_structured_output && ctx.round > 2 {
            findings.push(Finding::new("no-structured-output", Severity::C,
                format!("goal '{}' has {} rounds but TASK_OUTPUT.json has no structured results", ctx.goal, ctx.round))
                .with_suggestion("use task_output_write to record structured results"));
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
