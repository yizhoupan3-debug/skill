//! Structured observation payloads for hook outbound JSON (`router_rs_observation`).

use crate::hook_observation_rules::{
    classify_additional_context, classify_followup_first_line, shorten_line, GateClassified,
};
use serde_json::{json, Map, Value};

pub const ROUTER_RS_HOOK_OBSERVATION_SCHEMA_VERSION: &str = "router-rs-hook-observation-v1";

#[derive(Debug, Clone, Copy)]
pub enum HookObservationHost {
    Cursor,
    Codex,
    /// Claude Code hook JSON (`router-rs claude hook`).
    ClaudeCode,
    /// Qoder IDE Agent hook JSON (`router-rs qoder hook`).
    Qoder,
}

fn classify_gate(followup: Option<&str>, additional: Option<&str>) -> Option<GateClassified> {
    if let Some(f) = followup {
        let line = f.lines().next().unwrap_or("").trim();
        if let Some(g) = classify_followup_first_line(line) {
            return Some(g);
        }
    }
    additional.and_then(classify_additional_context)
}

fn extract_surfaces(output: &Value, host: HookObservationHost) -> (Option<String>, Option<String>) {
    match host {
        HookObservationHost::Cursor => (
            output
                .get("followup_message")
                .and_then(Value::as_str)
                .map(|s| s.to_string()),
            output
                .get("additional_context")
                .and_then(Value::as_str)
                .map(|s| s.to_string()),
        ),
        HookObservationHost::Codex => {
            let followup = output
                .get("followup_message")
                .and_then(Value::as_str)
                .map(|s| s.to_string());
            let additional = output
                .pointer("/hookSpecificOutput/additionalContext")
                .and_then(Value::as_str)
                .map(|s| s.to_string());
            (followup, additional)
        }
        HookObservationHost::ClaudeCode | HookObservationHost::Qoder => {
            let followup = output
                .get("stopReason")
                .or_else(|| output.get("systemMessage"))
                .or_else(|| output.get("followup_message"))
                .and_then(Value::as_str)
                .map(|s| s.to_string())
                .or_else(|| {
                    output
                        .get("message")
                        .or_else(|| output.get("reason"))
                        .and_then(Value::as_str)
                        .map(|s| s.to_string())
                });
            let additional = output
                .pointer("/hookSpecificOutput/additionalContext")
                .and_then(Value::as_str)
                .map(|s| s.to_string());
            (followup, additional)
        }
    }
}

fn nonempty_trimmed_str(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

/// Session/task pointers for downstream automation; key omitted when empty (see harness docs).
fn extract_observation_correlation(output: &Value) -> Option<Value> {
    let mut m = Map::new();
    if let Some(s) = nonempty_trimmed_str(output.get("session_id")) {
        m.insert("session_id".to_string(), Value::String(s));
    }
    if let Some(s) = nonempty_trimmed_str(output.get("task_id")) {
        m.insert("task_id".to_string(), Value::String(s));
    }
    if let Some(ti) = output.get("tool_input").and_then(Value::as_object) {
        if !m.contains_key("session_id") {
            if let Some(s) = nonempty_trimmed_str(ti.get("session_id")) {
                m.insert("session_id".to_string(), Value::String(s));
            }
        }
        if !m.contains_key("task_id") {
            if let Some(s) = nonempty_trimmed_str(ti.get("task_id")) {
                m.insert("task_id".to_string(), Value::String(s));
            }
        }
    }
    if m.is_empty() {
        None
    } else {
        Some(Value::Object(m))
    }
}

fn observation_payload(
    host_str: &str,
    gate: Value,
    output: &Value,
    projection_notes: Value,
) -> Value {
    let mut obj = Map::new();
    obj.insert(
        "schema_version".into(),
        Value::String(ROUTER_RS_HOOK_OBSERVATION_SCHEMA_VERSION.to_string()),
    );
    obj.insert("host".into(), Value::String(host_str.to_string()));
    obj.insert("gate".into(), gate);
    if let Some(c) = extract_observation_correlation(output) {
        obj.insert("correlation".into(), c);
    }
    obj.insert("projection_notes".into(), projection_notes);
    Value::Object(obj)
}

fn compute_blocking(output: &Value, gate: Option<&GateClassified>) -> bool {
    let permission_deny = output.get("permission").and_then(Value::as_str) == Some("deny");
    let decision_block = output.get("decision").and_then(Value::as_str) == Some("block");
    let continue_false = output.get("continue").and_then(Value::as_bool) == Some(false);
    let continue_true = output.get("continue").and_then(Value::as_bool) == Some(true);
    let base = permission_deny || decision_block || continue_false;

    match gate.map(|g| g.code.as_str()) {
        Some(
            "review_gate" | "codex_review_gate" | "claude_review_gate" | "qoder_review_gate"
            | "ag_followup" | "closeout_followup" | "subagent_limit",
        ) => {
            if continue_true {
                base
            } else {
                true
            }
        }
        _ => base,
    }
}

fn maybe_subagent_limit_gate(output: &Value) -> Option<GateClassified> {
    if output.get("permission").and_then(Value::as_str) != Some("deny") {
        return None;
    }
    let msg = output
        .get("user_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    if msg.contains("subagent") && (msg.contains("router-rs") || msg.contains("上限")) {
        return Some(GateClassified {
            code: "subagent_limit".to_string(),
            human_prefix: shorten_line(msg, 120),
        });
    }
    None
}

/// Attach `router_rs_observation` to the top-level hook JSON (Cursor or Codex lifecycle output).
pub fn attach_router_rs_observation(output: &mut Value, host: HookObservationHost) {
    let obs = build_router_rs_observation_value(output, host);
    if let Some(obj) = output.as_object_mut() {
        obj.insert("router_rs_observation".to_string(), obs);
    }
}

pub fn strip_router_rs_observation(output: &mut Value) {
    if let Some(obj) = output.as_object_mut() {
        obj.remove("router_rs_observation");
    }
}

pub fn build_router_rs_observation_value(output: &Value, host: HookObservationHost) -> Value {
    let host_str = match host {
        HookObservationHost::Cursor => "cursor",
        HookObservationHost::Codex => "codex",
        HookObservationHost::ClaudeCode => "claude-code",
        HookObservationHost::Qoder => "qoder",
    };

    if output.get("contract_guard").is_some()
        && output.get("decision").and_then(Value::as_str) == Some("block")
    {
        return observation_payload(
            host_str,
            json!({
                "code": "contract_guard_block",
                "blocking": true,
                "human_prefix": "contract_guard",
            }),
            output,
            Value::Null,
        );
    }

    if matches!(host, HookObservationHost::Codex)
        && output
            .pointer("/hookSpecificOutput/hookEventName")
            .and_then(Value::as_str)
            == Some("CodexLifecycleContext")
        && output.get("decision").and_then(Value::as_str) == Some("block")
    {
        let msg = output
            .get("message")
            .or_else(|| output.get("reason"))
            .and_then(Value::as_str)
            .unwrap_or("lifecycle_context_input_error");
        return observation_payload(
            host_str,
            json!({
                "code": "lifecycle_context_input_error",
                "blocking": true,
                "human_prefix": shorten_line(msg, 120),
            }),
            output,
            Value::Null,
        );
    }

    if let Some(g) = maybe_subagent_limit_gate(output) {
        let blocking = compute_blocking(output, Some(&g));
        return observation_payload(
            host_str,
            json!({
                "code": g.code,
                "blocking": blocking,
                "human_prefix": g.human_prefix,
            }),
            output,
            Value::Null,
        );
    }

    let (followup, additional) = extract_surfaces(output, host);
    let gate = classify_gate(followup.as_deref(), additional.as_deref());
    let blocking = compute_blocking(output, gate.as_ref());

    let gate_json = if let Some(ref g) = gate {
        json!({
            "code": g.code,
            "blocking": blocking,
            "human_prefix": g.human_prefix,
        })
    } else {
        Value::Null
    };

    observation_payload(host_str, gate_json, output, Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn classifies_review_gate_followup() {
        let followup = format!(
            "router-rs REVIEW_GATE incomplete phase=0 {} {}",
            crate::cursor_hooks::REVIEW_GATE_FOLLOWUP_NEED_SEGMENT,
            crate::cursor_hooks::REVIEW_GATE_FOLLOWUP_HINT_SEGMENT
        );
        let v = json!({
            "continue": false,
            "followup_message": followup,
        });
        let o = build_router_rs_observation_value(&v, HookObservationHost::Cursor);
        assert_eq!(o["gate"]["code"], "review_gate");
        assert_eq!(o["gate"]["blocking"], true);
    }

    #[test]
    fn classifies_autopilot_from_additional_only() {
        let v = json!({
            "continue": true,
            "additional_context": "AUTOPILOT_DRIVE: stale\nGoal: x",
        });
        let o = build_router_rs_observation_value(&v, HookObservationHost::Cursor);
        assert_eq!(o["gate"]["code"], "autopilot_drive");
        assert_eq!(o["gate"]["blocking"], false);
    }

    #[test]
    fn correlation_from_session_and_task_ids() {
        let v = json!({
            "continue": true,
            "session_id": "sess-1",
            "task_id": "task-9",
            "additional_context": "AUTOPILOT_DRIVE: x",
        });
        let o = build_router_rs_observation_value(&v, HookObservationHost::Cursor);
        assert_eq!(o["correlation"]["session_id"], "sess-1");
        assert_eq!(o["correlation"]["task_id"], "task-9");
    }

    #[test]
    fn correlation_prefers_top_level_over_tool_input() {
        let v = json!({
            "continue": true,
            "session_id": "top",
            "tool_input": {"session_id": "nested"},
            "followup_message": "router-rs REVIEW_GATE x",
        });
        let o = build_router_rs_observation_value(&v, HookObservationHost::Cursor);
        assert_eq!(o["correlation"]["session_id"], "top");
    }

    #[test]
    fn claude_stop_classifies_claude_review_gate() {
        let v = json!({
            "continue": false,
            "decision": "block",
            "stopReason": "router-rs CLAUDE_REVIEW_GATE incomplete: run subagent",
        });
        let o = build_router_rs_observation_value(&v, HookObservationHost::ClaudeCode);
        assert_eq!(o["host"], "claude-code");
        assert_eq!(o["gate"]["code"], "claude_review_gate");
        assert_eq!(o["gate"]["blocking"], true);
    }

    #[test]
    fn qoder_stop_classifies_qoder_review_gate() {
        let v = json!({
            "continue": false,
            "decision": "block",
            "stopReason": "router-rs QODER_REVIEW_GATE incomplete: run subagent",
        });
        let o = build_router_rs_observation_value(&v, HookObservationHost::Qoder);
        assert_eq!(o["host"], "qoder");
        assert_eq!(o["gate"]["code"], "qoder_review_gate");
        assert_eq!(o["gate"]["blocking"], true);
    }

    #[test]
    fn golden_unknown_router_rs_token_in_followup() {
        let v = json!({
            "continue": true,
            "followup_message": "router-rs WEIRD_TOKEN hello",
        });
        let o = build_router_rs_observation_value(&v, HookObservationHost::Cursor);
        assert_eq!(o["gate"]["code"], "unknown_router_rs");
    }

    #[test]
    fn golden_hook_state_degraded_fullwidth_colon() {
        let v = json!({
            "continue": true,
            "followup_message": "router-rs：disk full",
        });
        let o = build_router_rs_observation_value(&v, HookObservationHost::Cursor);
        assert_eq!(o["gate"]["code"], "hook_state_degraded");
    }

    #[test]
    fn golden_closeout_followup_wins_in_additional() {
        let v = json!({
            "continue": true,
            "additional_context": "CLOSEOUT_FOLLOWUP please\nAUTOPILOT_DRIVE: x",
        });
        let o = build_router_rs_observation_value(&v, HookObservationHost::Cursor);
        assert_eq!(o["gate"]["code"], "closeout_followup");
    }
}
