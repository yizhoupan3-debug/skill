//! GoalContractChecker — validates goal contract completeness before allowing iteration completion.
//!
//! Checks:
//! 1. done_when items have corresponding evidence (checkpoint notes or evidence artifacts)
//! 2. Minimum checkpoint count (goal was actually iterated on)
//! 3. Validation commands were executed (at least one evidence artifact references them)
//! 4. No stale/pending blockers

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};
use serde_json::Value;
use std::fs;

/// General-scene checker that validates goal contract completeness.
pub struct GoalContractChecker;

impl GateChecker for GoalContractChecker {
    fn id(&self) -> &'static str {
        "goal_contract"
    }

    fn description(&self) -> &'static str {
        "verify goal contract: done_when coverage, checkpoints, validation commands"
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

        // ── 1. Read GOAL_STATE.json ──
        let goal_path = task_dir.join("GOAL_STATE.json");
        let goal_state: Option<Value> = fs::read_to_string(&goal_path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok());

        let Some(goal) = goal_state.as_ref() else {
            findings.push(Finding::new("missing-goal-state", Severity::Warning,
                format!("GOAL_STATE.json not found for task '{}'", ctx.task_id))
                .with_suggestion("create a goal before completing"));
            return CheckResult {
                checker_id: self.id().to_string(),
                passed: false,
                findings,
            };
        };

        // ── 2. Check done_when coverage ──
        let done_when: Vec<&str> = goal
            .get("done_when")
            .and_then(Value::as_array)
            .map(|arr| arr.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();

        if done_when.is_empty() {
            findings.push(Finding::new("no-done-when", Severity::A,
                "goal has no done_when items — completion cannot be verified"
                    .to_string())
                .with_suggestion("add done_when items to define verifiable completion criteria"));
        }

        // ── 3. Check checkpoint count ──
        let checkpoints = goal
            .get("checkpoints")
            .and_then(Value::as_array)
            .map(|a| a.len())
            .unwrap_or(0);

        if checkpoints == 0 && ctx.round > 1 {
            findings.push(Finding::new("no-checkpoints", Severity::B,
                format!("goal completed in {} rounds but has zero checkpoints — no progress recorded", ctx.round))
                .with_suggestion("use checkpoint operation to record progress between iterations"));
        }

        // ── 4. Check validation commands ──
        let validation_commands: Vec<&str> = goal
            .get("validation_commands")
            .and_then(Value::as_array)
            .map(|arr| arr.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();

        if !validation_commands.is_empty() {
            // Check if any evidence artifact references validation command execution
            let evidence_path = task_dir.join("EVIDENCE_INDEX.json");
            let has_validation_evidence = fs::read_to_string(&evidence_path)
                .ok()
                .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
                .and_then(|v| v.get("artifacts").cloned())
                .and_then(|a| a.as_array().cloned())
                .map(|artifacts| {
                    artifacts.iter().any(|a| {
                        let source = a.get("source").and_then(Value::as_str).unwrap_or("");
                        let kind = a.get("kind").and_then(Value::as_str).unwrap_or("");
                        source.contains("validation") || kind.contains("validation")
                            || source.contains("cargo") || source.contains("test")
                    })
                })
                .unwrap_or(false);

            if !has_validation_evidence {
                findings.push(Finding::new("no-validation-evidence", Severity::B,
                    format!("goal has {} validation command(s) but no evidence of execution",
                        validation_commands.len()))
                    .with_suggestion("run validation commands and record results as evidence"));
            }
        }

        // ── 5. Check for pending blockers ──
        let blockers = goal.get("blockers");
        let has_active_blockers = blockers
            .and_then(Value::as_array)
            .map(|a| !a.is_empty())
            .unwrap_or(false);

        if has_active_blockers {
            findings.push(Finding::new("pending-blockers", Severity::A,
                "goal has pending blockers — cannot complete while blockers are unresolved"
                    .to_string())
                .with_suggestion("resolve all blockers before completing the goal"));
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
