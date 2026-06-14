use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use super::types::{
    CAPABILITY_SURFACE_FIELDS, EXECUTION_CONTROLLER_CONTRACT_ARTIFACT_ID,
    EXECUTION_PROTOCOL_CONTRACT_ARTIFACT_ID, DELEGATION_CONTRACT_ARTIFACT_ID,
    RUNTIME_SURFACE_FIELDS, SUPERVISOR_STATE_CONTRACT_ARTIFACT_ID, FrameworkProfileContract,
};

fn repo_scan_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(raw) => raw.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn value_object<const N: usize>(pairs: [(&str, Value); N]) -> Value {
    let mut object = Map::new();
    for (key, value) in pairs {
        object.insert(key.to_string(), value);
    }
    Value::Object(object)
}

fn value_array(items: Vec<Value>) -> Value {
    Value::Array(items)
}

pub fn build_runtime_surface(shared_contract: &Map<String, Value>) -> Map<String, Value> {
    let mut runtime_surface = Map::new();
    for field in RUNTIME_SURFACE_FIELDS {
        if let Some(value) = shared_contract.get(field) {
            runtime_surface.insert(field.to_string(), value.clone());
        }
    }
    runtime_surface
}

pub fn build_capability_surface(shared_contract: &Map<String, Value>) -> Map<String, Value> {
    let mut capability_surface = Map::new();
    for field in CAPABILITY_SURFACE_FIELDS {
        if let Some(value) = shared_contract.get(field) {
            capability_surface.insert(field.to_string(), value.clone());
        }
    }
    capability_surface
}

pub fn complete_host_payload(host_cli: &str, host_fields: Map<String, Value>) -> Map<String, Value> {
    let mut completed = Map::new();
    completed.insert("host_cli".to_string(), Value::String(host_cli.to_string()));
    completed.insert("context_files".to_string(), Value::Array(vec![]));
    completed.insert("settings_paths".to_string(), Value::Array(vec![]));
    completed.insert("mcp_config_paths".to_string(), Value::Array(vec![]));
    completed.insert("config_root_env_var".to_string(), Value::Null);
    completed.insert("settings_scope_order".to_string(), Value::Array(vec![]));
    completed.insert("settings_scopes".to_string(), Value::Array(vec![]));
    completed.insert("subagent_paths".to_string(), Value::Array(vec![]));
    completed.insert("managed_settings_paths".to_string(), Value::Array(vec![]));
    completed.insert("managed_mcp_paths".to_string(), Value::Array(vec![]));
    completed.insert("structured_output_modes".to_string(), Value::Array(vec![]));
    completed.insert("checkpointing_supported".to_string(), Value::Bool(false));
    completed.insert("session_supervisor_driver".to_string(), Value::Null);
    completed.insert("resume_command_examples".to_string(), Value::Array(vec![]));
    completed.insert(
        "framework_alias_entrypoints".to_string(),
        Value::Object(Map::new()),
    );
    for (key, value) in host_fields {
        completed.insert(key, value);
    }
    completed
}

pub fn build_host_alias_entrypoints(host_key: &str) -> Value {
    let registry_path = repo_scan_root()
        .join("configs")
        .join("framework")
        .join("RUNTIME_REGISTRY.json");
    let aliases = fs::read_to_string(&registry_path)
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .and_then(|payload| payload.get("framework_commands").cloned())
        .and_then(|aliases| aliases.as_object().cloned());
    let mut entrypoints = Map::new();
    if let Some(aliases) = aliases {
        let mut alias_names = aliases.keys().cloned().collect::<Vec<_>>();
        alias_names.sort();
        for alias_name in alias_names {
            let Some(entrypoint) = aliases
                .get(&alias_name)
                .and_then(|record| record.get("host_entrypoints"))
                .and_then(|host_entrypoints| host_entrypoints.get(host_key))
                .and_then(Value::as_str)
            else {
                continue;
            };
            entrypoints.insert(alias_name, Value::String(entrypoint.to_string()));
        }
    }
    Value::Object(entrypoints)
}

pub fn build_execution_protocol_contract() -> Map<String, Value> {
    let mut contract = Map::new();
    contract.insert(
        "schema_version".to_string(),
        Value::String("execution-protocol-v1".to_string()),
    );
    contract.insert(
        "artifact_id".to_string(),
        Value::String(EXECUTION_PROTOCOL_CONTRACT_ARTIFACT_ID.to_string()),
    );
    contract.insert(
        "genesis".to_string(),
        value_object([
            ("engine", Value::String("skill-framework".to_string())),
            ("framework_profile_version", Value::String("0.1.0".to_string())),
            ("seed_tool", Value::String("resolve".to_string())),
            ("seed_agent", Value::String("init".to_string())),
            ("initial_state", Value::String("goal_acquisition".to_string())),
        ]),
    );
    contract.insert(
        "phases".to_string(),
        value_array(vec![
            value_object([
                ("phase_id", Value::String("resolve".to_string())),
                ("entry_message", Value::String("planning_contract".to_string())),
            ]),
            value_object([
                ("phase_id", Value::String("execute".to_string())),
                ("entry_message", Value::String("execution_contract".to_string())),
            ]),
            value_object([
                ("phase_id", Value::String("verify".to_string())),
                ("entry_message", Value::String("verification_report".to_string())),
            ]),
            value_object([
                ("phase_id", Value::String("closeout".to_string())),
                ("entry_message", Value::String("closeout_record".to_string())),
            ]),
        ]),
    );
    contract.insert(
        "termination".to_string(),
        value_object([
            ("normal", Value::String("phase_complete".to_string())),
            (
                "abnormal",
                Value::String("error_or_interrupt".to_string()),
            ),
        ]),
    );
    contract
}

pub fn build_execution_controller_contract() -> Map<String, Value> {
    let mut contract = Map::new();
    contract.insert(
        "schema_version".to_string(),
        Value::String("execution-controller-v1".to_string()),
    );
    contract.insert(
        "artifact_id".to_string(),
        Value::String(EXECUTION_CONTROLLER_CONTRACT_ARTIFACT_ID.to_string()),
    );
    contract.insert(
        "controller_kind".to_string(),
        Value::String("event-driven-delegation".to_string()),
    );
    contract.insert(
        "event_driven".to_string(),
        value_object([
            ("agent_idle_timeout_secs", json!(600)),
            (
                "dispatchers",
                value_array(vec![
                    value_object([
                        ("event", Value::String("task_idle".to_string())),
                        (
                            "handler",
                            Value::String("task_timeout_handler".to_string()),
                        ),
                    ]),
                    value_object([
                        ("event", Value::String("task_done".to_string())),
                        (
                            "handler",
                            Value::String("task_completion_handler".to_string()),
                        ),
                    ]),
                    value_object([
                        (
                            "event",
                            Value::String("sub_task_submitted".to_string()),
                        ),
                        (
                            "handler",
                            Value::String("sub_task_handler".to_string()),
                        ),
                    ]),
                    value_object([
                        (
                            "event",
                            Value::String("supervisor_tick".to_string()),
                        ),
                        (
                            "handler",
                            Value::String("supervisor_tick_handler".to_string()),
                        ),
                    ]),
                ]),
            ),
        ]),
    );
    contract
}

pub fn build_delegation_contract() -> Map<String, Value> {
    let mut contract = Map::new();
    contract.insert(
        "schema_version".to_string(),
        Value::String("delegation-v1".to_string()),
    );
    contract.insert(
        "artifact_id".to_string(),
        Value::String(DELEGATION_CONTRACT_ARTIFACT_ID.to_string()),
    );
    contract.insert(
        "delegation_kind".to_string(),
        Value::String("hierarchical".to_string()),
    );
    contract.insert(
        "spawning_policy".to_string(),
        value_object([
            ("max_sub_agents", json!(5)),
            (
                "rate_limit",
                value_object([
                    ("window_secs", json!(60)),
                    ("max_spawns", json!(10)),
                ]),
            ),
            ("default_timeout_secs", json!(300)),
        ]),
    );
    contract.insert(
        "fallback".to_string(),
        value_object([
            ("local_fallback", Value::Bool(true)),
            (
                "fallback_instructions",
                Value::String("complete sub-goal without spawning".to_string()),
            ),
        ]),
    );

    let tool = value_object([
        ("tool_name", Value::String("task".to_string())),
        (
            "tool_description",
            Value::String("Navigate to a specific file/line and run a task".to_string()),
        ),
    ]);

    let mut delegations = Map::new();
    delegations.insert("console".to_string(), tool);
    delegations.insert(
        "subagent_config".to_string(),
        value_object([
            ("tool_name", Value::String("subagent".to_string())),
            (
                "tool_description",
                Value::String("Spawn a sub-agent for a focused task".to_string()),
            ),
        ]),
    );
    contract.insert("delegations".to_string(), Value::Object(delegations));
    contract
}

pub fn build_supervisor_state_contract() -> Map<String, Value> {
    let mut schema_expectations = Map::new();
    schema_expectations.insert(
        "top_level_fields".to_string(),
        json!([
            "schema_version",
            "task_id",
            "task_summary",
            "controller",
            "primary_owner",
            "active_phase",
            "execution_contract",
            "delegation",
            "workers",
            "progress",
            "verification",
            "open_blockers",
            "next_actions"
        ]),
    );
    schema_expectations.insert(
        "execution_contract_fields".to_string(),
        json!([
            "goal",
            "scope",
            "forbidden_scope",
            "acceptance_criteria",
            "evidence_required"
        ]),
    );
    schema_expectations.insert(
        "delegation_fields".to_string(),
        json!([
            "delegation_plan_created",
            "spawn_attempted",
            "spawn_block_reason",
            "fallback_mode",
            "delegated_sidecars"
        ]),
    );
    schema_expectations.insert(
        "workers_fields".to_string(),
        json!([
            "planned",
            "running",
            "completed_unintegrated",
            "integrated",
            "failed_recoverable",
            "failed_terminal",
            "stalled"
        ]),
    );

    let mut contract = Map::new();
    contract.insert(
        "schema_version".to_string(),
        Value::String("supervisor-state-v1".to_string()),
    );
    contract.insert(
        "artifact_id".to_string(),
        Value::String(SUPERVISOR_STATE_CONTRACT_ARTIFACT_ID.to_string()),
    );
    contract.insert("schema_expectations".to_string(), Value::Object(schema_expectations));
    contract.insert(
        "supervisor_kind".to_string(),
        Value::String("phase-driven".to_string()),
    );
    contract
}

pub fn build_control_plane_contract_descriptors() -> Map<String, Value> {
    let mut descriptors = Map::new();
    descriptors.insert(
        EXECUTION_PROTOCOL_CONTRACT_ARTIFACT_ID.to_string(),
        json!({ "contract_type": "protocol", "schema_version": "execution-protocol-v1" }),
    );
    descriptors.insert(
        EXECUTION_CONTROLLER_CONTRACT_ARTIFACT_ID.to_string(),
        json!({ "contract_type": "controller", "schema_version": "execution-controller-v1" }),
    );
    descriptors.insert(
        DELEGATION_CONTRACT_ARTIFACT_ID.to_string(),
        json!({ "contract_type": "delegation", "schema_version": "delegation-v1" }),
    );
    descriptors.insert(
        SUPERVISOR_STATE_CONTRACT_ARTIFACT_ID.to_string(),
        json!({ "contract_type": "supervisor_state", "schema_version": "supervisor-state-v1" }),
    );
    descriptors
}
