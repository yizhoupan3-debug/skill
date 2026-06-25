//! Thin telemetry journal emitters.
//!
//! Re-exports simple emit functions from `telemetry-emit` (L0) and keeps
//! L4+ dependent functions that need `core_state`, `routing_engine`, etc.

use core_state::goal_prediction::{GoalStatePrediction, PredictionVerification};
use framework_kernel::{PredictionOutcomeCheck, TelemetryEvent, emit_telemetry};
use serde_json::{Value, json};
use std::path::Path;
use tracing::debug;

use core_state::state_manager::{framework_goal_drive as goal_drive_inner, read_goal_state};
use routing_engine::route::RouteDecision;

// ── Re-exported from telemetry-emit (L0) ──
pub use telem::{
    emit_hook_fired,
    emit_goal_transition,
    emit_rfv_round,
    hook_action_from_output,
    hook_action_from_optional_output,
    hook_timing_action,
    emit_hook_timing_telemetry,
};

/// Emit a `RouteDecision` event (L4: requires `RouteDecision` type).
pub fn emit_route_decision(
    query: &str,
    decision: &RouteDecision,
    reroute: bool,
    latency_ms: u64,
    parity_gate: &str,
) {
    debug!(query, skill = %decision.selected_skill, score = decision.score, reroute, "route decision emitted");
    emit_telemetry(&TelemetryEvent::RouteDecision {
        task: query.to_string(),
        skill: decision.selected_skill.clone(),
        confidence: decision.score as f32,
        reroute,
        latency_ms,
        reasons: decision.reasons.clone(),
        matched_tokens: decision.matched_token_count,
        parity_gate: parity_gate.to_string(),
        candidates: vec![],
    });
}

/// Emit a `ToolCall` event with bootstrap guard.
/// Callers at L3+ should use this wrapper which calls `ensure_kernel_bootstrap()` first.
pub fn emit_tool_call(tool: &str, duration_ms: u64, success: bool) {
    crate::kernel_bootstrap::ensure_kernel_bootstrap();
    telem::emit_tool_call(tool, duration_ms, success);
}

fn prediction_checks_summary(checks: &[PredictionVerification]) -> String {
    checks
        .iter()
        .map(|c| format!("{}:{}:{}", c.rule, c.matched, c.severity))
        .collect::<Vec<_>>()
        .join("; ")
}

pub fn build_prediction_outcome_event(
    task_id: &str,
    prediction: &GoalStatePrediction,
    actual_verification_status: &str,
    checks: &[PredictionVerification],
) -> TelemetryEvent {
    let matched = !checks.is_empty() && checks.iter().all(|c| c.matched);
    TelemetryEvent::PredictionOutcome {
        task_id: task_id.to_string(),
        matched,
        predicted_verification_status: prediction.expected_verification_status.clone(),
        predicted_hypothesis: prediction.hypothesis.clone(),
        actual_verification_status: actual_verification_status.trim().to_ascii_lowercase(),
        checks_summary: prediction_checks_summary(checks),
        checks: checks
            .iter()
            .map(|c| PredictionOutcomeCheck {
                rule: c.rule.clone(),
                matched: c.matched,
                severity: c.severity.clone(),
            })
            .collect(),
    }
}

/// EV-6: journal prediction vs closeout outcome (dry-run; does not block closeout).
pub fn emit_prediction_outcome(
    task_id: &str,
    prediction: &GoalStatePrediction,
    actual_verification_status: &str,
    checks: &[PredictionVerification],
) {
    if checks.is_empty() {
        return;
    }
    crate::kernel_bootstrap::ensure_kernel_bootstrap();
    emit_telemetry(&build_prediction_outcome_event(
        task_id,
        prediction,
        actual_verification_status,
        checks,
    ));
}

/// Quality Gate stdio wrapper: ensures bootstrap and emits operation-level `hook_fired`.
pub fn framework_quality_gate(payload: Value) -> Result<Value, String> {
    crate::kernel_bootstrap::ensure_kernel_bootstrap();
    let operation = payload
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("status")
        .trim()
        .to_ascii_lowercase();
    let result = runtime_exit_gate::quality_gate::framework_quality_gate(payload)?;
    if result.get("ok") == Some(&json!(true)) {
        emit_hook_fired("quality_gate", &operation);
    }
    Ok(result)
}

fn read_goal_phase(repo_root: Option<&str>, task_id: Option<&str>) -> Option<String> {
    let repo = Path::new(repo_root?);
    let tid = task_id.filter(|s| !s.is_empty()).map(str::to_string)?;
    read_goal_state(repo, Some(&tid))
        .ok()
        .flatten()
        .and_then(|state| {
            state
                .get("active_phase")
                .and_then(|p| p.as_str())
                .map(str::to_string)
        })
}

fn maybe_emit_goal_transition(operation: &str, result: &Value, prior_phase: Option<&str>) {
    let task_id = result
        .get("task_id")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if task_id.is_empty() {
        return;
    }
    let to = result
        .get("goal_state")
        .and_then(|g| g.get("active_phase"))
        .and_then(|p| p.as_str())
        .unwrap_or(operation);
    let from = prior_phase.unwrap_or("unknown");
    emit_goal_transition(from, to, task_id);
}

/// Goal-drive stdio wrapper: emits checkpoint / phase transitions to the telemetry journal.
pub fn framework_goal_drive(payload: Value) -> Result<Value, String> {
    crate::kernel_bootstrap::ensure_kernel_bootstrap();
    let operation = payload
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("status")
        .trim()
        .to_ascii_lowercase();
    let repo_root = payload.get("repo_root").and_then(|v| v.as_str());
    let task_id = payload.get("task_id").and_then(|v| v.as_str());
    let prior_phase = if matches!(operation.as_str(), "checkpoint" | "start" | "complete") {
        read_goal_phase(repo_root, task_id)
    } else {
        None
    };
    let result = goal_drive_inner(payload)?;
    if result.get("ok") == Some(&json!(true))
        && matches!(operation.as_str(), "checkpoint" | "start" | "complete")
    {
        maybe_emit_goal_transition(&operation, &result, prior_phase.as_deref());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn prediction_mismatch_writes_prediction_outcome_journal_line() {
        use core_state::goal_prediction::GoalStatePrediction;
        use framework_kernel::{LogAggregator, TelemetryWriter};
        use std::fs;
        use std::time::SystemTime;

        let pred = GoalStatePrediction {
            expected_verification_status: Some("passed".to_string()),
            hypothesis: Some("router-rs green".to_string()),
        };
        let checks = core_state::goal_prediction::verify_prediction_against_closeout(
            &pred,
            "failed",
            "已完成 router-rs green",
        );
        assert!(
            checks
                .iter()
                .any(|c| c.rule == "prediction_verification_status_mismatch"),
            "expected status mismatch dry-run check"
        );

        let suffix = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("router-pred-outcome-{suffix}"));
        fs::create_dir_all(&dir).unwrap();
        let journal = dir.join("events.jsonl");
        let handle = LogAggregator::start(&journal);
        {
            let writer = handle.writer();
            let event = build_prediction_outcome_event("t-ev6", &pred, "failed", &checks);
            writer.write_event(&event).unwrap();
        }
        handle.shutdown();
        let raw = fs::read_to_string(&journal).unwrap();
        assert!(raw.contains("\"prediction_outcome\""));
        assert!(raw.contains("prediction_verification_status_mismatch"));
        assert!(raw.contains("\"matched\":false"));
        let _ = fs::remove_dir_all(dir);
    }
}
