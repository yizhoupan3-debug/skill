//! E7 step 2: PostToolUse handler — touch-state persistence, reviewer evidence, context nudges.

use router_rs::framework_error::FrameworkError;
use router_rs::hook_common::{normalize_subagent_type, normalize_tool_name};
use router_rs::review_gate_engine::{claude_independent_reviewer_evidence, fork_context_from_values};
use serde_json::{json, Value};
use std::path::Path;

use super::{
    active_stdio_agent_hook_host, add_context, is_framework_source_path, is_settings_path,
    payload_relative_paths, SETTINGS_CHANGED_CONTEXT,
};
use super::session::{
    load_review_gate_disk, persist_touch_state, review_state_path,
    with_claude_review_state_lock, write_review_state_unlocked, AgentDiskState, ReviewGateState,
};

const FRAMEWORK_CHANGED_CONTEXT: &str =
    "Framework routing/runtime files changed; run the targeted Rust contract tests before finishing.";

fn agent_tool_input(payload: &Value) -> Value {
    payload
        .as_object()
        .and_then(router_rs::hook_common::tool_input_value_from_map)
        .unwrap_or_else(|| json!({}))
}

fn reviewer_lane(tool_input: &Value, payload: &Value) -> bool {
    let subagent_type = normalize_subagent_type(
        tool_input
            .get("subagent_type")
            .or_else(|| tool_input.get("agent_type"))
            .or_else(|| tool_input.get("type"))
            .or_else(|| payload.get("subagent_type"))
            .or_else(|| payload.get("agent_type"))
            .and_then(Value::as_str),
    );
    !subagent_type.is_empty()
        && router_rs::runtime_registry::is_claude_reviewer_lane_from_registry(&subagent_type, None)
}

pub fn subagent_tool(payload: &Value) -> bool {
    let name = normalize_tool_name(
        payload
            .get("tool_name")
            .or_else(|| payload.get("tool"))
            .or_else(|| payload.get("name"))
            .and_then(Value::as_str),
    );
    tool_name_implies_subagent(&name)
}

pub fn tool_name_implies_subagent(normalized: &str) -> bool {
    if matches!(
        normalized,
        "task"
            | "functions.task"
            | "functions.subagent"
            | "functions.spawn_agent"
            | "subagent"
            | "spawn_agent"
    ) {
        return true;
    }
    if normalized.ends_with("_subagent")
        || normalized.ends_with("_spawn_agent")
        || normalized.ends_with(".subagent")
        || normalized.ends_with(".spawn_agent")
    {
        return true;
    }
    normalized
        .split('.')
        .any(|seg| seg == "subagent" || seg == "spawn_agent")
}

fn record_reviewer_evidence(repo_root: &Path, payload: &Value) {
    let path = review_state_path(repo_root, payload);
    let tool_input = agent_tool_input(payload);
    let fork = fork_context_from_values(&tool_input, Some(payload));
    if let Err(err) = with_claude_review_state_lock(&path, || {
        let mut state = match load_review_gate_disk(repo_root, payload) {
            AgentDiskState::Unreadable => {
                eprintln!(
                    "[router-rs] {} review_gate state unreadable on PostToolUse: {}",
                    active_stdio_agent_hook_host().log_label(),
                    path.display()
                );
                return Err(FrameworkError::other("review_gate_unreadable"));
            }
            AgentDiskState::Absent => ReviewGateState::default(),
            AgentDiskState::Ok(s) => s,
        };
        if !state.review_required || state.review_override {
            return Ok(());
        }
        if !payload_is_successful_tool(payload) {
            return Ok(());
        }
        if subagent_tool(payload)
            && claude_independent_reviewer_evidence(reviewer_lane(&tool_input, payload), fork)
        {
            state.independent_reviewer_seen = true;
            write_review_state_unlocked(&path, &state)?;
        }
        Ok(())
    }) {
        if err.to_hook_exit() != "review_gate_unreadable" {
            eprintln!(
                "[router-rs] {} review_gate evidence record failed: {err}",
                active_stdio_agent_hook_host().log_label()
            );
        }
    }
}

fn bash_command(payload: &Value) -> Option<&str> {
    payload
        .get("tool_input")
        .and_then(Value::as_object)
        .and_then(|tool_input| tool_input.get("command"))
        .or_else(|| payload.get("command"))
        .and_then(Value::as_str)
}

pub fn payload_is_successful_bash(payload: &Value) -> bool {
    if payload.get("tool_name").and_then(Value::as_str) != Some("Bash") {
        return false;
    }
    payload_is_successful_tool(payload)
}

pub fn payload_is_successful_tool(payload: &Value) -> bool {
    if payload
        .get("is_error")
        .and_then(Value::as_bool)
        .is_some_and(|v| v)
    {
        return false;
    }
    if payload.get("error").is_some_and(|v| !v.is_null()) {
        return false;
    }
    match payload_exit_code(payload) {
        Some(0) => true,
        Some(_) => false,
        None => true,
    }
}

fn payload_exit_code(payload: &Value) -> Option<i64> {
    find_numeric_key(payload, &["exit_code", "exitCode", "status"])
}

fn find_numeric_key(value: &Value, keys: &[&str]) -> Option<i64> {
    match value {
        Value::Object(map) => {
            for key in keys {
                if let Some(number) = map.get(*key).and_then(Value::as_i64) {
                    return Some(number);
                }
            }
            map.values().find_map(|child| find_numeric_key(child, keys))
        }
        Value::Array(items) => items.iter().find_map(|child| find_numeric_key(child, keys)),
        _ => None,
    }
}

fn payload_runs_settings_validation(payload: &Value) -> bool {
    let Some(command) = bash_command(payload) else {
        return false;
    };
    let lowered = command.to_ascii_lowercase();
    (lowered.contains("jq") || lowered.contains("python") || lowered.contains("node"))
        && active_stdio_agent_hook_host()
            .settings_guarded_paths()
            .iter()
            .any(|p| lowered.contains(&p.to_ascii_lowercase()))
}

fn payload_runs_framework_tests(payload: &Value) -> bool {
    let Some(command) = bash_command(payload) else {
        return false;
    };
    let lowered = command.to_ascii_lowercase();
    if !lowered.contains("cargo test") {
        return false;
    }
    [
        "--manifest-path core/router-rs/cargo.toml",
        "core/router-rs/cargo.toml",
        "router-rs",
        "--test policy_contracts",
        "--test documentation_contracts",
        "--test host_integration",
    ]
    .iter()
    .any(|hint| lowered.contains(hint))
}

/// Claude PostToolUse: `None` → silent; `Some` → additionalContext nudge.
pub fn evaluate_claude_post_tool_use(repo_root: &Path, payload: &Value) -> Option<Value> {
    record_reviewer_evidence(repo_root, payload);
    let paths = payload_relative_paths(repo_root, payload);
    let touched_settings = paths.iter().any(|path| is_settings_path(path));
    let touched_framework = paths.iter().any(|path| is_framework_source_path(path));
    let settings_validated =
        payload_is_successful_bash(payload) && payload_runs_settings_validation(payload);
    let framework_tested =
        payload_is_successful_bash(payload) && payload_runs_framework_tests(payload);
    if touched_settings || touched_framework || settings_validated || framework_tested {
        persist_touch_state(
            repo_root,
            payload,
            touched_settings,
            touched_framework,
            settings_validated,
            framework_tested,
        );
    }
    match (touched_settings, touched_framework) {
        (true, true) => add_context(
            "PostToolUse",
            &format!("{SETTINGS_CHANGED_CONTEXT}\n{FRAMEWORK_CHANGED_CONTEXT}"),
        ),
        (true, false) => add_context("PostToolUse", SETTINGS_CHANGED_CONTEXT),
        (false, true) => add_context("PostToolUse", FRAMEWORK_CHANGED_CONTEXT),
        (false, false) => None,
    }
}
