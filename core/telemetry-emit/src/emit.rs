//! Simple emit functions — moved from `runtime-infra::telemetry_emit`.
//! These depend only on `framework-kernel` and `telemetry-types` (L0).

use crate::TelemetryEvent;
use serde_json::Value;

/// Emit a `HookFired` event.
pub fn emit_hook_fired(hook_name: &str, action: &str) {
    crate::emit_telemetry(&TelemetryEvent::HookFired {
        hook_name: hook_name.to_string(),
        action: action.to_string(),
    });
}

/// Emit a `GoalTransition` event.
pub fn emit_goal_transition(from: &str, to: &str, task_id: &str) {
    crate::emit_telemetry(&TelemetryEvent::GoalTransition {
        from: from.to_string(),
        to: to.to_string(),
        task_id: task_id.to_string(),
    });
}

/// Emit an `RfvRound` event.
pub fn emit_rfv_round(round: u32, verdict: &str) {
    crate::emit_telemetry(&TelemetryEvent::RfvRound {
        round,
        verdict: verdict.to_string(),
    });
}

/// Emit a `ToolCall` event (raw — no bootstrap guard).
/// Callers at L4+ should use `runtime-infra::telemetry_emit::emit_tool_call`
/// which calls `ensure_kernel_bootstrap()` first.
pub fn emit_tool_call(tool: &str, duration_ms: u64, success: bool) {
    crate::emit_telemetry(&TelemetryEvent::ToolCall {
        tool: tool.to_string(),
        duration_ms,
        success,
    });
}

// ── Hook action helpers (pure functions, no deps) ──

/// Determine hook action from a serde_json output Value.
pub fn hook_action_from_output(output: &Value) -> &'static str {
    if output.get("continue").and_then(Value::as_bool) == Some(false) {
        return "block";
    }
    if output.get("decision").and_then(Value::as_str) == Some("block") {
        return "block";
    }
    if output.get("permission").and_then(Value::as_str) == Some("deny") {
        return "deny";
    }
    if output.get("permission").and_then(Value::as_str) == Some("allow") {
        return "allow";
    }
    if output.get("suppressOutput").and_then(Value::as_bool) == Some(true) {
        return "silent";
    }
    "allow"
}

/// Determine hook action from an optional output Value.
pub fn hook_action_from_optional_output(output: Option<&Value>) -> &'static str {
    match output {
        None => "silent",
        Some(value) if value.as_object().is_some_and(|map| map.is_empty()) => "silent",
        Some(value) => hook_action_from_output(value),
    }
}

/// Format hook timing durations into an action string (for `HookFired` action field).
pub fn hook_timing_action(duration_ms: u64, lock_wait_ms: u64, cargo_check_ms: u64) -> String {
    format!("timing:{duration_ms}ms:lock={lock_wait_ms}:cargo={cargo_check_ms}")
}

/// Emit a hook timing telemetry event (calls `emit_hook_fired` under the hood).
pub fn emit_hook_timing_telemetry(
    event: &str,
    duration_ms: u64,
    lock_wait_ms: u64,
    cargo_check_ms: u64,
) {
    emit_hook_fired(
        event,
        &hook_timing_action(duration_ms, lock_wait_ms, cargo_check_ms),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TelemetryEvent;
    use serde_json::json;

    #[test]
    fn hook_action_from_stop_output() {
        let blocked = json!({"continue": false});
        assert_eq!(hook_action_from_output(&blocked), "block");
        let allowed = json!({"permission": "allow"});
        assert_eq!(hook_action_from_output(&allowed), "allow");
        let codex_block = json!({"decision": "block"});
        assert_eq!(hook_action_from_output(&codex_block), "block");
        let claude_silent = json!({"suppressOutput": true});
        assert_eq!(hook_action_from_output(&claude_silent), "silent");
        assert_eq!(hook_action_from_optional_output(None), "silent");
    }

    #[test]
    fn hook_timing_action_encodes_durations() {
        let action = hook_timing_action(42, 3, 7);
        assert_eq!(action, "timing:42ms:lock=3:cargo=7");
    }

    #[test]
    fn metric_counter_basics() {
        let counter = crate::MetricCounter::new("test_metric")
            .with_label("env", "test");
        // just verify it doesn't panic and labels are stored
        assert_eq!(counter.name(), "test_metric");
        assert_eq!(counter.label_value("env"), Some("test"));
    }

    #[test]
    fn metric_counter_emit_creates_valid_metric_event() {
        let metric = crate::MetricCounter::new("test_emit")
            .with_label("tool", "my_tool");
        let event = metric.build_event(42.0);
        match &event {
            TelemetryEvent::MetricEvent { metric_name, value, labels } => {
                assert_eq!(metric_name, "test_emit");
                assert!((*value - 42.0).abs() < f64::EPSILON);
                assert_eq!(labels.get("tool"), Some(&"my_tool".to_string()));
            }
            _ => panic!("expected MetricEvent variant"),
        }
    }
}
