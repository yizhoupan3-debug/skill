//! Structured observation payloads for hook outbound JSON (`router_rs_observation`).

use serde_json::{json, Map, Value};

pub const ROUTER_RS_HOOK_OBSERVATION_SCHEMA_VERSION: &str = "router-rs-hook-observation-v1";

#[derive(Debug, Clone, Copy)]
pub enum HookObservationHost {
    Cursor,
    Codex,
}

#[derive(Debug, Clone)]
struct GateClassified {
    code: &'static str,
    human_prefix: String,
}

fn shorten_line(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.len() <= max {
        return t.to_string();
    }
    let mut cut = max.min(t.len());
    while cut > 0 && !t.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}...", &t[..cut])
}

fn classify_router_rs_after_leader(line: &str) -> Option<GateClassified> {
    let rest = line.strip_prefix("router-rs ").unwrap_or("").trim();
    let token = rest.split_whitespace().next().unwrap_or("");
    let code = match token {
        "REVIEW_GATE" => "review_gate",
        "CODEX_REVIEW_GATE" => "codex_review_gate",
        "AG_FOLLOWUP" => "ag_followup",
        _ if !token.is_empty() => "unknown_router_rs",
        _ => return None,
    };
    Some(GateClassified {
        code,
        human_prefix: if token.is_empty() {
            "router-rs".to_string()
        } else {
            format!("router-rs {token}")
        },
    })
}

fn classify_line_followup(line: &str) -> Option<GateClassified> {
    let t = line.trim();
    if t.starts_with("CLOSEOUT_FOLLOWUP") {
        return Some(GateClassified {
            code: "closeout_followup",
            human_prefix: shorten_line(t, 120),
        });
    }
    if t.starts_with("router-rs ") {
        return classify_router_rs_after_leader(t);
    }
    if t.starts_with("router-rs：") || t.starts_with("router-rs:") {
        return Some(GateClassified {
            code: "hook_state_degraded",
            human_prefix: shorten_line(t, 120),
        });
    }
    None
}

fn classify_additional(text: &str) -> Option<GateClassified> {
    let mut picked: Option<(u8, GateClassified)> = None;
    for line in text.lines() {
        let t = line.trim();
        let cand = if t.starts_with("CLOSEOUT_FOLLOWUP") {
            Some((
                0_u8,
                GateClassified {
                    code: "closeout_followup",
                    human_prefix: shorten_line(t, 120),
                },
            ))
        } else if t.starts_with("router-rs ") {
            classify_router_rs_after_leader(t).map(|g| (1_u8, g))
        } else if t.starts_with("router-rs：") || t.starts_with("router-rs:") {
            Some((
                1_u8,
                GateClassified {
                    code: "hook_state_degraded",
                    human_prefix: shorten_line(t, 120),
                },
            ))
        } else if t.starts_with("AUTOPILOT_DRIVE") || t.contains("AUTOPILOT_DRIVE:") {
            Some((
                2_u8,
                GateClassified {
                    code: "autopilot_drive",
                    human_prefix: "AUTOPILOT_DRIVE".to_string(),
                },
            ))
        } else if t.contains("RFV_LOOP_CONTINUE") {
            Some((
                3_u8,
                GateClassified {
                    code: "rfv_loop_continue",
                    human_prefix: "RFV_LOOP_CONTINUE".to_string(),
                },
            ))
        } else if t.starts_with("SESSION_CLOSE_STYLE") || t.contains("SESSION_CLOSE_STYLE:") {
            Some((
                5_u8,
                GateClassified {
                    code: "session_close_style",
                    human_prefix: "SESSION_CLOSE_STYLE".to_string(),
                },
            ))
        } else {
            None
        };
        if let Some((pri, g)) = cand {
            let replace = picked
                .as_ref()
                .map(|(existing_pri, _)| pri < *existing_pri)
                .unwrap_or(true);
            if replace {
                picked = Some((pri, g));
            }
            if pri == 0 {
                break;
            }
        }
    }
    picked.map(|(_, g)| g)
}

fn classify_gate(followup: Option<&str>, additional: Option<&str>) -> Option<GateClassified> {
    if let Some(f) = followup {
        let line = f.lines().next().unwrap_or("").trim();
        if let Some(g) = classify_line_followup(line) {
            return Some(g);
        }
    }
    additional.and_then(classify_additional)
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

    match gate.map(|g| g.code) {
        Some(
            "review_gate" | "codex_review_gate" | "ag_followup" | "closeout_followup"
            | "subagent_limit",
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
            code: "subagent_limit",
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
        let v = json!({
            "continue": false,
            "followup_message": "router-rs REVIEW_GATE incomplete phase=0 need=subagent",
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
    fn codex_lifecycle_context_input_error_shape() {
        let msg = "Codex lifecycle hook input schema invalid: missing hook_event_name/event.";
        let v = json!({
            "decision": "block",
            "message": msg,
            "reason": msg,
            "hookSpecificOutput": {
                "hookEventName": "CodexLifecycleContext",
                "permissionDecision": "deny",
                "permissionDecisionReason": msg,
            },
        });
        let o = build_router_rs_observation_value(&v, HookObservationHost::Codex);
        assert_eq!(o["gate"]["code"], "lifecycle_context_input_error");
        assert_eq!(o["gate"]["blocking"], true);
    }
}
