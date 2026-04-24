use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

const REQUIRED_CORE_CAPABILITIES: [&str; 4] = ["runtime", "memory", "artifact", "orchestration"];
const RUNTIME_SURFACE_FIELDS: [&str; 12] = [
    "artifact_contract",
    "memory_mounts",
    "mcp_servers",
    "tool_policy",
    "approval_policy",
    "loadout_policy",
    "framework_surface_policy",
    "workspace_bootstrap",
    "session_contract",
    "execution_controller_contract",
    "delegation_contract",
    "supervisor_state_contract",
];
const CODEX_HOST_CAPABILITIES: [&str; 13] = [
    "artifact_contract",
    "memory_mounts",
    "mcp_servers",
    "workspace_bootstrap",
    "batch_execution",
    "cron_execution",
    "ci_runner",
    "non_interactive_entrypoint",
    "external_session_supervisor",
    "rate_limit_auto_resume",
    "host_resume_entrypoint",
    "host_tmux_worker_management",
    "framework_alias_entrypoints",
];
const CODEX_HOST_PAYLOAD_KEY: &str = "host_adapter_payload";
const HOST_SPECIFIC_METADATA_KEYS: &[&str] = &[
    "adapter_id",
    "adapter_alias_of",
    "automation_bridge_required",
    "canonical_adapter_id",
    "checkpointing_supported",
    "config_root_env_var",
    "context_files",
    "controller_is_cli",
    "entrypoint_kind",
    "host_cli",
    "host_id",
    "managed_mcp_paths",
    "managed_settings_paths",
    "mcp_config_paths",
    "settings_paths",
    "settings_scope_order",
    "settings_scopes",
    "shared_adapter",
    "structured_output_modes",
    "subagent_paths",
    "supports_batch",
    "supports_ci",
    "supports_cron",
    "thread_binding",
    "transport",
];
const CODEX_ADAPTER_ID: &str = "codex_adapter";
const EXECUTION_CONTROLLER_CONTRACT_ARTIFACT_ID: &str = "execution_controller_contract";
const DELEGATION_CONTRACT_ARTIFACT_ID: &str = "delegation_contract";
const SUPERVISOR_STATE_CONTRACT_ARTIFACT_ID: &str = "supervisor_state_contract";

struct CodexProfileBuildContext<'a> {
    normalized_memory_mounts: &'a [Value],
    normalized_mcp_servers: &'a [Value],
    workspace_bootstrap: &'a Map<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FrameworkProfileContract {
    pub profile_id: String,
    pub display_name: String,
    #[serde(default = "default_framework_profile_version")]
    pub framework_profile_version: String,
    #[serde(default = "default_runtime_family")]
    pub runtime_family: String,
    #[serde(default = "default_host_family")]
    pub host_family: String,
    #[serde(default = "default_core_capabilities")]
    pub core_capabilities: Vec<String>,
    #[serde(default)]
    pub optional_capabilities: Vec<String>,
    #[serde(default = "default_rules_bundle")]
    pub rules_bundle: Value,
    #[serde(default = "default_skill_bundle")]
    pub skill_bundle: Value,
    #[serde(default)]
    pub session_policy: Map<String, Value>,
    #[serde(default)]
    pub tool_policy: Map<String, Value>,
    #[serde(default)]
    pub approval_policy: Map<String, Value>,
    #[serde(default)]
    pub loadout_policy: Map<String, Value>,
    #[serde(default)]
    pub framework_surface_policy: Map<String, Value>,
    #[serde(default)]
    pub artifact_contract: Map<String, Value>,
    #[serde(default)]
    pub model_policy: Map<String, Value>,
    #[serde(default)]
    pub memory_mounts: Vec<Value>,
    #[serde(default)]
    pub mcp_servers: Vec<Value>,
    #[serde(default)]
    pub workspace_bootstrap: Map<String, Value>,
    #[serde(default)]
    pub host_capability_requirements: Map<String, Value>,
    #[serde(default)]
    pub metadata: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileBundle {
    pub profile_id: String,
    pub display_name: String,
    pub framework_profile_version: String,
    pub runtime_family: String,
    pub host_family: String,
    pub capabilities: CapabilityBundle,
    pub rules_bundle: Value,
    pub skill_bundle: Value,
    pub session_policy: Map<String, Value>,
    pub tool_policy: Map<String, Value>,
    pub approval_policy: Map<String, Value>,
    pub loadout_policy: Map<String, Value>,
    pub framework_surface_policy: Map<String, Value>,
    pub artifact_contract: Map<String, Value>,
    pub model_policy: Map<String, Value>,
    pub memory_mounts: Vec<Value>,
    pub mcp_servers: Vec<Value>,
    pub workspace_bootstrap: Map<String, Value>,
    pub host_capability_requirements: Map<String, Value>,
    pub metadata: Map<String, Value>,
    pub codex_adapter: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityBundle {
    pub core: Vec<String>,
    pub optional: Vec<String>,
}

pub fn load_framework_profile(path: &Path) -> Result<FrameworkProfileContract, String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("failed reading {}: {err}", path.display()))?;
    let profile: FrameworkProfileContract = serde_json::from_str(&text)
        .map_err(|err| format!("failed parsing {}: {err}", path.display()))?;
    validate_framework_profile(&profile)?;
    Ok(profile)
}

pub fn build_profile_bundle(profile: &FrameworkProfileContract) -> Result<ProfileBundle, String> {
    validate_framework_profile(profile)?;

    let normalized_memory_mounts = normalize_mounts(&profile.memory_mounts);
    let normalized_mcp_servers = normalize_mcp_servers(&profile.mcp_servers);
    let workspace_bootstrap = compile_workspace_bootstrap(profile, &normalized_memory_mounts);
    let shared_contract = build_codex_shared_contract(
        profile,
        &normalized_memory_mounts,
        &normalized_mcp_servers,
        &workspace_bootstrap,
    );
    let codex_adapter = build_codex_adapter(
        profile,
        &normalized_memory_mounts,
        &normalized_mcp_servers,
        &workspace_bootstrap,
        &shared_contract,
    );
    Ok(ProfileBundle {
        profile_id: profile.profile_id.clone(),
        display_name: profile.display_name.clone(),
        framework_profile_version: profile.framework_profile_version.clone(),
        runtime_family: profile.runtime_family.clone(),
        host_family: profile.host_family.clone(),
        capabilities: CapabilityBundle {
            core: profile.core_capabilities.clone(),
            optional: profile.optional_capabilities.clone(),
        },
        rules_bundle: profile.rules_bundle.clone(),
        skill_bundle: profile.skill_bundle.clone(),
        session_policy: profile.session_policy.clone(),
        tool_policy: profile.tool_policy.clone(),
        approval_policy: profile.approval_policy.clone(),
        loadout_policy: profile.loadout_policy.clone(),
        framework_surface_policy: profile.framework_surface_policy.clone(),
        artifact_contract: profile.artifact_contract.clone(),
        model_policy: profile.model_policy.clone(),
        memory_mounts: normalized_memory_mounts.clone(),
        mcp_servers: normalized_mcp_servers.clone(),
        workspace_bootstrap: workspace_bootstrap.clone(),
        host_capability_requirements: profile.host_capability_requirements.clone(),
        metadata: profile.metadata.clone(),
        codex_adapter: Value::Object(codex_adapter),
    })
}

pub fn build_codex_artifact_bundle(
    profile: &FrameworkProfileContract,
) -> Result<Map<String, Value>, String> {
    let bundle = build_profile_bundle(profile)?;
    let mut artifacts = Map::new();
    artifacts.insert("codex_adapter".to_string(), bundle.codex_adapter);
    Ok(artifacts)
}

fn validate_framework_profile(profile: &FrameworkProfileContract) -> Result<(), String> {
    if profile.profile_id.trim().is_empty() {
        return Err("framework profile missing profile_id".to_string());
    }
    if profile.display_name.trim().is_empty() {
        return Err("framework profile missing display_name".to_string());
    }
    if profile.framework_profile_version.trim().is_empty() {
        return Err("framework profile missing framework_profile_version".to_string());
    }
    if profile.host_family.trim() != "codex" {
        return Err("framework core must be pinned to Codex".to_string());
    }

    let capability_set = profile
        .core_capabilities
        .iter()
        .map(|value| value.as_str())
        .collect::<HashSet<_>>();
    let missing = REQUIRED_CORE_CAPABILITIES
        .iter()
        .filter(|cap| !capability_set.contains(**cap))
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!(
            "framework profile missing core capabilities: {}",
            missing.join(", ")
        ));
    }
    let host_specific_metadata = profile
        .metadata
        .keys()
        .filter(|key| HOST_SPECIFIC_METADATA_KEYS.contains(&key.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !host_specific_metadata.is_empty() {
        return Err(format!(
            "framework profile metadata must stay Codex-core-only; move Codex host-private keys into codex_adapter.host_adapter_payload: {}",
            host_specific_metadata.join(", ")
        ));
    }
    Ok(())
}

fn normalize_mounts(memory_mounts: &[Value]) -> Vec<Value> {
    memory_mounts
        .iter()
        .map(|mount| match mount {
            Value::Object(obj) => {
                let mut payload = obj.clone();
                if !payload.contains_key("mount_id") {
                    let mount_id = payload
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("unnamed-memory-mount")
                        .to_string();
                    payload.insert("mount_id".to_string(), Value::String(mount_id));
                }
                Value::Object(payload)
            }
            other => value_object([
                ("mount_id", Value::String(value_to_string(other))),
                ("source", Value::String(value_to_string(other))),
                (
                    "bridge_kind",
                    Value::String("framework-memory-mount".to_string()),
                ),
            ]),
        })
        .collect()
}

fn normalize_mcp_servers(mcp_servers: &[Value]) -> Vec<Value> {
    mcp_servers
        .iter()
        .map(|server| match server {
            Value::Object(obj) => {
                let mut payload = obj.clone();
                if !payload.contains_key("server_id") {
                    let server_id = payload
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("unnamed-mcp-server")
                        .to_string();
                    payload.insert("server_id".to_string(), Value::String(server_id));
                }
                Value::Object(payload)
            }
            other => value_object([("server_id", Value::String(value_to_string(other)))]),
        })
        .collect()
}

fn compile_workspace_bootstrap(
    profile: &FrameworkProfileContract,
    normalized_memory_mounts: &[Value],
) -> Map<String, Value> {
    let mut bootstrap = profile.workspace_bootstrap.clone();
    let mut bridges = bootstrap
        .get("bridges")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    if !bridges.contains_key("skills") {
        let skills_bridge = bootstrap.get("skill_bridge").cloned().unwrap_or_else(|| {
            value_object([
                ("project_dir", Value::String(".codex/skills".to_string())),
                ("user_dir", Value::String("~/.codex/skills".to_string())),
                ("bridge_dir", Value::String(".codex/skills".to_string())),
            ])
        });
        bridges.insert("skills".to_string(), skills_bridge);
    }
    if !bridges.contains_key("memory") {
        let memory_bridge = bootstrap.get("memory_bridge").cloned().unwrap_or_else(|| {
            value_object([
                (
                    "bridge_dir",
                    Value::String(".codex/memory-bridge".to_string()),
                ),
                ("mounts", Value::Array(normalized_memory_mounts.to_vec())),
            ])
        });
        bridges.insert("memory".to_string(), memory_bridge);
    }
    bootstrap.insert("bridges".to_string(), Value::Object(bridges));
    bootstrap
}

fn compile_session_mode(session_policy: &Map<String, Value>) -> Value {
    let mut extras = Map::new();
    for (key, value) in session_policy {
        if matches!(
            key.as_str(),
            "mode" | "approval_mode" | "history_policy" | "takeover"
        ) {
            continue;
        }
        extras.insert(key.clone(), value.clone());
    }

    value_object([
        (
            "mode",
            session_policy
                .get("mode")
                .cloned()
                .unwrap_or_else(|| Value::String("default".to_string())),
        ),
        (
            "approval_mode",
            session_policy
                .get("approval_mode")
                .cloned()
                .unwrap_or_else(|| Value::String("inherit".to_string())),
        ),
        (
            "history_policy",
            session_policy
                .get("history_policy")
                .cloned()
                .unwrap_or_else(|| Value::String("host-managed".to_string())),
        ),
        (
            "takeover",
            session_policy
                .get("takeover")
                .cloned()
                .unwrap_or(Value::Bool(false)),
        ),
        ("extras", Value::Object(extras)),
    ])
}

fn build_codex_shared_contract(
    profile: &FrameworkProfileContract,
    normalized_memory_mounts: &[Value],
    normalized_mcp_servers: &[Value],
    workspace_bootstrap: &Map<String, Value>,
) -> Map<String, Value> {
    let mut shared_contract = Map::new();
    shared_contract.insert(
        "artifact_contract".to_string(),
        Value::Object(profile.artifact_contract.clone()),
    );
    shared_contract.insert(
        "memory_mounts".to_string(),
        Value::Array(normalized_memory_mounts.to_vec()),
    );
    shared_contract.insert(
        "mcp_servers".to_string(),
        Value::Array(normalized_mcp_servers.to_vec()),
    );
    shared_contract.insert(
        "tool_policy".to_string(),
        Value::Object(profile.tool_policy.clone()),
    );
    shared_contract.insert(
        "approval_policy".to_string(),
        Value::Object(profile.approval_policy.clone()),
    );
    shared_contract.insert(
        "loadout_policy".to_string(),
        Value::Object(profile.loadout_policy.clone()),
    );
    shared_contract.insert(
        "framework_surface_policy".to_string(),
        Value::Object(profile.framework_surface_policy.clone()),
    );
    shared_contract.insert(
        "workspace_bootstrap".to_string(),
        Value::Object(workspace_bootstrap.clone()),
    );
    shared_contract.insert(
        "session_contract".to_string(),
        compile_session_mode(&profile.session_policy),
    );
    let control_plane_contracts = build_control_plane_contract_descriptors();
    shared_contract.insert(
        EXECUTION_CONTROLLER_CONTRACT_ARTIFACT_ID.to_string(),
        control_plane_contracts
            .get(EXECUTION_CONTROLLER_CONTRACT_ARTIFACT_ID)
            .cloned()
            .expect("execution controller contract descriptor should exist"),
    );
    shared_contract.insert(
        DELEGATION_CONTRACT_ARTIFACT_ID.to_string(),
        control_plane_contracts
            .get(DELEGATION_CONTRACT_ARTIFACT_ID)
            .cloned()
            .expect("delegation contract descriptor should exist"),
    );
    shared_contract.insert(
        SUPERVISOR_STATE_CONTRACT_ARTIFACT_ID.to_string(),
        control_plane_contracts
            .get(SUPERVISOR_STATE_CONTRACT_ARTIFACT_ID)
            .cloned()
            .expect("supervisor state contract descriptor should exist"),
    );
    shared_contract
}

fn build_codex_profile_output_base(
    profile: &FrameworkProfileContract,
    context: &CodexProfileBuildContext<'_>,
) -> Map<String, Value> {
    let resolved_host_capability_requirements = resolve_codex_capability_requirements(profile);
    let mut capabilities = Map::new();
    capabilities.insert(
        "core".to_string(),
        Value::Array(
            profile
                .core_capabilities
                .iter()
                .cloned()
                .map(Value::String)
                .collect(),
        ),
    );
    capabilities.insert(
        "optional".to_string(),
        Value::Array(
            profile
                .optional_capabilities
                .iter()
                .cloned()
                .map(Value::String)
                .collect(),
        ),
    );
    capabilities.insert("host".to_string(), string_array(&CODEX_HOST_CAPABILITIES));

    let mut metadata = Map::new();
    metadata.insert(
        "adapter_id".to_string(),
        Value::String(CODEX_ADAPTER_ID.to_string()),
    );
    metadata.insert("host_id".to_string(), Value::String("codex".to_string()));
    metadata.insert(
        "transport".to_string(),
        Value::String("native-codex".to_string()),
    );
    metadata.insert("deep_adaptation_not_fork".to_string(), Value::Bool(false));
    metadata.insert(
        "upgrade_zone".to_string(),
        Value::String("upstream-safe-zone".to_string()),
    );

    let mut payload = Map::new();
    payload.insert(
        "profile_id".to_string(),
        Value::String(profile.profile_id.clone()),
    );
    payload.insert(
        "display_name".to_string(),
        Value::String(profile.display_name.clone()),
    );
    payload.insert(
        "framework_profile_version".to_string(),
        Value::String(profile.framework_profile_version.clone()),
    );
    payload.insert(
        "host_family".to_string(),
        Value::String(profile.host_family.clone()),
    );
    payload.insert(
        "runtime_family".to_string(),
        Value::String(profile.runtime_family.clone()),
    );
    payload.insert("capabilities".to_string(), Value::Object(capabilities));
    payload.insert("rules_bundle".to_string(), profile.rules_bundle.clone());
    payload.insert("skill_bundle".to_string(), profile.skill_bundle.clone());
    payload.insert(
        "session_policy".to_string(),
        Value::Object(profile.session_policy.clone()),
    );
    payload.insert(
        "tool_policy".to_string(),
        Value::Object(profile.tool_policy.clone()),
    );
    payload.insert(
        "approval_policy".to_string(),
        Value::Object(profile.approval_policy.clone()),
    );
    payload.insert(
        "loadout_policy".to_string(),
        Value::Object(profile.loadout_policy.clone()),
    );
    payload.insert(
        "framework_surface_policy".to_string(),
        Value::Object(profile.framework_surface_policy.clone()),
    );
    payload.insert(
        "artifact_contract".to_string(),
        Value::Object(profile.artifact_contract.clone()),
    );
    payload.insert(
        "model_policy".to_string(),
        Value::Object(profile.model_policy.clone()),
    );
    payload.insert(
        "memory_mounts".to_string(),
        Value::Array(context.normalized_memory_mounts.to_vec()),
    );
    payload.insert(
        "mcp_servers".to_string(),
        Value::Array(context.normalized_mcp_servers.to_vec()),
    );
    payload.insert(
        "workspace_bootstrap".to_string(),
        Value::Object(context.workspace_bootstrap.clone()),
    );
    payload.insert(
        "host_capability_requirements".to_string(),
        Value::Object(resolved_host_capability_requirements),
    );
    payload.insert("metadata".to_string(), Value::Object(metadata));
    payload
}

fn build_runtime_surface(shared_contract: &Map<String, Value>) -> Map<String, Value> {
    let mut runtime_surface = Map::new();
    for field in RUNTIME_SURFACE_FIELDS {
        if let Some(value) = shared_contract.get(field) {
            runtime_surface.insert(field.to_string(), value.clone());
        }
    }
    runtime_surface
}

fn complete_codex_host_payload(codex_host_fields: Map<String, Value>) -> Map<String, Value> {
    let mut completed = Map::new();
    completed.insert("host_cli".to_string(), Value::String("codex".to_string()));
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
    for (key, value) in codex_host_fields {
        completed.insert(key, value);
    }
    completed
}

fn build_host_alias_entrypoints(host_key: &str) -> Value {
    let registry_path = repo_scan_root()
        .join("configs")
        .join("framework")
        .join("RUNTIME_REGISTRY.json");
    let aliases = fs::read_to_string(&registry_path)
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .and_then(|payload| payload.get("framework_native_aliases").cloned())
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

fn build_codex_host_payload() -> Map<String, Value> {
    let mut payload = Map::new();
    payload.insert(
        "context_files".to_string(),
        Value::Array(vec![Value::String("AGENTS.md".to_string())]),
    );
    payload.insert(
        "settings_paths".to_string(),
        Value::Array(vec![
            Value::String("~/.codex/config.toml".to_string()),
            Value::String(".codex/config.toml".to_string()),
        ]),
    );
    payload.insert(
        "mcp_config_paths".to_string(),
        Value::Array(vec![Value::String(".codex/config.toml".to_string())]),
    );
    payload.insert(
        "session_supervisor_driver".to_string(),
        Value::String("codex_driver".to_string()),
    );
    payload.insert(
        "resume_command_examples".to_string(),
        json!(["codex resume --last", "codex resume <session_id>"]),
    );
    payload.insert(
        "framework_alias_entrypoints".to_string(),
        build_host_alias_entrypoints("codex-cli"),
    );
    payload.insert(
        "gpt_model_path_contract".to_string(),
        json!({
            "path_kind": "native-openai-compatible",
            "preferred_for_gpt_family": true,
            "adapter_loss_profile": "minimal",
            "avoidable_loss_sources": [],
            "reduce_loss_by": [
                "use /v1 OpenAI-compatible endpoint directly",
                "avoid Anthropic-compatible request/response translation for GPT-default work"
            ]
        }),
    );
    payload
}

fn build_codex_adapter(
    profile: &FrameworkProfileContract,
    normalized_memory_mounts: &[Value],
    normalized_mcp_servers: &[Value],
    workspace_bootstrap: &Map<String, Value>,
    shared_contract: &Map<String, Value>,
) -> Map<String, Value> {
    let mut payload = build_codex_profile_output_base(
        profile,
        &CodexProfileBuildContext {
            normalized_memory_mounts,
            normalized_mcp_servers,
            workspace_bootstrap,
        },
    );
    payload.insert(
        "common_contract".to_string(),
        Value::Object(shared_contract.clone()),
    );
    payload.insert(
        "runtime_surface".to_string(),
        Value::Object(build_runtime_surface(shared_contract)),
    );
    payload.insert(
        "execution_surface".to_string(),
        value_object([
            ("entrypoint_kind", Value::String("codex".to_string())),
            ("non_interactive", Value::Bool(true)),
            ("supports_batch", Value::Bool(true)),
            ("supports_cron", Value::Bool(true)),
            ("supports_ci", Value::Bool(true)),
            (
                "framework_truth",
                Value::String("framework_core".to_string()),
            ),
            ("controller_is_cli", Value::Bool(false)),
            ("host_cli", Value::String("codex".to_string())),
        ]),
    );
    payload.insert(
        CODEX_HOST_PAYLOAD_KEY.to_string(),
        Value::Object(complete_codex_host_payload(build_codex_host_payload())),
    );
    payload
}

fn resolve_codex_capability_requirements(profile: &FrameworkProfileContract) -> Map<String, Value> {
    let mut merged = Map::new();
    for key in ["default", "codex", CODEX_ADAPTER_ID] {
        if let Some(Value::Object(requirements)) = profile.host_capability_requirements.get(key) {
            merge_json_maps(&mut merged, requirements);
        }
    }
    merged
}

fn merge_json_maps(target: &mut Map<String, Value>, override_map: &Map<String, Value>) {
    for (key, value) in override_map {
        if let Some(existing) = target.get_mut(key) {
            match (existing, value) {
                (Value::Object(existing_object), Value::Object(override_value)) => {
                    merge_json_maps(existing_object, override_value);
                }
                (Value::Array(existing_array), Value::Array(override_value)) => {
                    for item in override_value {
                        if !existing_array
                            .iter()
                            .any(|existing_item| existing_item == item)
                        {
                            existing_array.push(item.clone());
                        }
                    }
                }
                (existing_value, override_value) => {
                    *existing_value = override_value.clone();
                }
            }
        } else {
            target.insert(key.clone(), value.clone());
        }
    }
}

fn build_execution_controller_contract() -> Map<String, Value> {
    let mut controller = Map::new();
    controller.insert(
        "primary_owner".to_string(),
        Value::String("execution-controller-coding".to_string()),
    );
    controller.insert(
        "role".to_string(),
        Value::String("kernel-level-execution-controller".to_string()),
    );
    controller.insert(
        "framework_phase".to_string(),
        Value::String("runtime-orchestration".to_string()),
    );
    controller.insert(
        "state_artifact".to_string(),
        Value::String(".supervisor_state.json".to_string()),
    );
    controller.insert(
        "user_facing_aliases".to_string(),
        json!(["gsd", "get shit done"]),
    );

    let mut gsd_execution_posture = Map::new();
    gsd_execution_posture.insert(
        "label".to_string(),
        Value::String("get-shit-done".to_string()),
    );
    gsd_execution_posture.insert(
        "auto_continue_safe_local_work".to_string(),
        Value::Bool(true),
    );
    gsd_execution_posture.insert(
        "main_thread_stays_decision_heavy".to_string(),
        Value::Bool(true),
    );
    gsd_execution_posture.insert("verify_before_done".to_string(), Value::Bool(true));
    gsd_execution_posture.insert(
        "runtime_dependency".to_string(),
        Value::String("none".to_string()),
    );

    let mut boundaries = Map::new();
    boundaries.insert(
        "codex_adapter_remains_compatibility_key".to_string(),
        Value::Bool(true),
    );
    boundaries.insert(
        "runtime_branching_changes_required".to_string(),
        Value::Bool(false),
    );
    boundaries.insert(
        "business_code_mutation_required".to_string(),
        Value::Bool(false),
    );
    boundaries.insert("single_framework_truth".to_string(), Value::Bool(true));

    let mut phase_model = Map::new();
    phase_model.insert(
        "state_owner".to_string(),
        Value::String(SUPERVISOR_STATE_CONTRACT_ARTIFACT_ID.to_string()),
    );
    phase_model.insert(
        "phase_field".to_string(),
        Value::String("active_phase".to_string()),
    );
    phase_model.insert(
        "verification_field".to_string(),
        Value::String("verification.verification_status".to_string()),
    );
    phase_model.insert("resumable".to_string(), Value::Bool(true));

    let mut retained_local_authority = Map::new();
    retained_local_authority.insert("orchestration_decisions".to_string(), Value::Bool(true));
    retained_local_authority.insert("final_integration_judgment".to_string(), Value::Bool(true));
    retained_local_authority.insert("rollback_decision".to_string(), Value::Bool(true));

    let mut payload = Map::new();
    payload.insert(
        "framework_truth".to_string(),
        Value::String("framework_core".to_string()),
    );
    payload.insert(
        "contract_artifact".to_string(),
        Value::String(EXECUTION_CONTROLLER_CONTRACT_ARTIFACT_ID.to_string()),
    );
    payload.insert(
        "status_contract".to_string(),
        Value::String("execution_controller_contract_v1".to_string()),
    );
    payload.insert(
        "artifact_role".to_string(),
        Value::String("shared-contract-evidence".to_string()),
    );
    payload.insert("controller".to_string(), Value::Object(controller));
    payload.insert(
        "gsd_execution_posture".to_string(),
        Value::Object(gsd_execution_posture),
    );
    payload.insert("boundaries".to_string(), Value::Object(boundaries));
    payload.insert(
        "continuity_artifacts".to_string(),
        json!([
            "SESSION_SUMMARY.md",
            "NEXT_ACTIONS.json",
            "EVIDENCE_INDEX.json",
            "TRACE_METADATA.json",
            ".supervisor_state.json"
        ]),
    );
    payload.insert(
        "required_execution_contract_fields".to_string(),
        json!([
            "goal",
            "scope",
            "forbidden_scope",
            "acceptance_criteria",
            "evidence_required"
        ]),
    );
    payload.insert("phase_model".to_string(), Value::Object(phase_model));
    payload.insert(
        "retained_local_authority".to_string(),
        Value::Object(retained_local_authority),
    );
    payload
}

fn build_delegation_contract() -> Map<String, Value> {
    let mut gate = Map::new();
    gate.insert(
        "gate_skill".to_string(),
        Value::String("subagent-delegation".to_string()),
    );
    gate.insert(
        "gate_type".to_string(),
        Value::String("multi_agent_routing".to_string()),
    );
    gate.insert("decision_before_spawn".to_string(), Value::Bool(true));
    gate.insert("spawn_is_optional".to_string(), Value::Bool(true));
    gate.insert(
        "route_outcomes".to_string(),
        json!(["local", "subagent", "team"]),
    );
    gate.insert(
        "team_route_skill".to_string(),
        Value::String("team".to_string()),
    );

    let mut local_supervisor_mode = Map::new();
    local_supervisor_mode.insert(
        "preserves_sidecar_boundaries".to_string(),
        Value::Bool(true),
    );
    local_supervisor_mode.insert("preserves_output_contracts".to_string(), Value::Bool(true));
    local_supervisor_mode.insert(
        "allowed_when_runtime_blocks_spawning".to_string(),
        Value::Bool(true),
    );

    let mut sidecar_contract = Map::new();
    sidecar_contract.insert("bounded_parallelism_only".to_string(), Value::Bool(true));
    sidecar_contract.insert(
        "main_thread_stays_decision_heavy".to_string(),
        Value::Bool(true),
    );
    sidecar_contract.insert("integration_remains_local".to_string(), Value::Bool(true));
    sidecar_contract.insert(
        "worker_traces_sink_to_artifacts".to_string(),
        Value::Bool(true),
    );

    let mut team_contract = Map::new();
    team_contract.insert("supervisor_owned_continuity".to_string(), Value::Bool(true));
    team_contract.insert(
        "integration_and_qa_stay_supervisor_led".to_string(),
        Value::Bool(true),
    );
    team_contract.insert(
        "resume_and_recovery_are_first_class".to_string(),
        Value::Bool(true),
    );

    let mut payload = Map::new();
    payload.insert(
        "framework_truth".to_string(),
        Value::String("framework_core".to_string()),
    );
    payload.insert(
        "contract_artifact".to_string(),
        Value::String(DELEGATION_CONTRACT_ARTIFACT_ID.to_string()),
    );
    payload.insert(
        "status_contract".to_string(),
        Value::String("delegation_contract_v4".to_string()),
    );
    payload.insert(
        "artifact_role".to_string(),
        Value::String("shared-contract-evidence".to_string()),
    );
    payload.insert("gate".to_string(), Value::Object(gate));
    payload.insert(
        "local_supervisor_mode".to_string(),
        Value::Object(local_supervisor_mode),
    );
    payload.insert(
        "selection_matrix".to_string(),
        json!({
            "local_when": [
                "immediate blocker is faster to solve on the main thread",
                "task is tightly coupled and sidecar boundaries are weak",
                "delegation overhead would exceed throughput gains"
            ],
            "subagent_when": [
                "bounded sidecars exist with non-overlapping write scopes",
                "search, audit, implementation, or verification can run as lane-local outputs",
                "integration and final judgment should still stay local"
            ],
            "team_when": [
                "supervisor-led worker lifecycle management is part of the task",
                "integration, qa, cleanup, or resume/recovery are first-class workflow phases",
                "shared continuity must remain supervisor-owned while multiple lanes stay active"
            ]
        }),
    );
    payload.insert(
        "delegation_state_fields".to_string(),
        json!([
            "routing_decision",
            "orchestration_mode",
            "delegation_plan_created",
            "spawn_attempted",
            "spawn_block_reason",
            "fallback_mode",
            "delegated_sidecars",
            "delegated_lanes"
        ]),
    );
    payload.insert(
        "lane_contract_fields".to_string(),
        json!([
            "lane_id",
            "lane_owner",
            "bounded_write_scope",
            "expected_output",
            "integration_status",
            "verification_status",
            "recovery_anchor"
        ]),
    );
    payload.insert(
        "retry_resume_fields".to_string(),
        json!([
            "retry_policy",
            "resume_policy",
            "escalation_path",
            "integration_preconditions"
        ]),
    );
    payload.insert(
        "sidecar_contract".to_string(),
        Value::Object(sidecar_contract),
    );
    payload.insert("team_contract".to_string(), Value::Object(team_contract));
    payload.insert(
        "non_goals".to_string(),
        json!([
            "runtime_spawn_policy_rewrite",
            "host-specific delegation_branching",
            "overlapping_write_scopes_between_workers"
        ]),
    );
    payload
}

fn build_supervisor_state_contract() -> Map<String, Value> {
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
    schema_expectations.insert(
        "verification_fields".to_string(),
        json!(["verification_status", "last_verification_summary"]),
    );
    schema_expectations.insert(
        "team_state_fields".to_string(),
        json!([
            "delegation_planned",
            "spawn_pending",
            "spawn_blocked",
            "integration_pending",
            "resume_required",
            "cleanup_pending"
        ]),
    );
    schema_expectations.insert(
        "lane_fields".to_string(),
        json!([
            "lane_id",
            "lane_owner",
            "goal",
            "bounded_scope",
            "forbidden_scope",
            "expected_output",
            "integration_status",
            "verification_status",
            "recovery_anchor"
        ]),
    );

    let mut cross_artifact_alignment = Map::new();
    cross_artifact_alignment.insert(
        "continuity_artifacts_must_share_task_story".to_string(),
        Value::Bool(true),
    );
    cross_artifact_alignment.insert("phase_must_be_resumable".to_string(), Value::Bool(true));
    cross_artifact_alignment.insert(
        "delegation_structure_must_be_explicit".to_string(),
        Value::Bool(true),
    );
    cross_artifact_alignment.insert(
        "lane_outputs_must_remain_lane_local_until_integrated".to_string(),
        Value::Bool(true),
    );

    let mut compatibility_rules = Map::new();
    compatibility_rules.insert("rust_may_validate_or_emit".to_string(), Value::Bool(true));
    compatibility_rules.insert(
        "python_may_continue_to_author".to_string(),
        Value::Bool(true),
    );
    compatibility_rules.insert(
        "no_shadow_replacement_artifact".to_string(),
        Value::Bool(true),
    );

    let mut payload = Map::new();
    payload.insert(
        "framework_truth".to_string(),
        Value::String("framework_core".to_string()),
    );
    payload.insert(
        "contract_artifact".to_string(),
        Value::String(SUPERVISOR_STATE_CONTRACT_ARTIFACT_ID.to_string()),
    );
    payload.insert(
        "status_contract".to_string(),
        Value::String("supervisor_state_contract_v3".to_string()),
    );
    payload.insert(
        "artifact_role".to_string(),
        Value::String("shared-contract-evidence".to_string()),
    );
    payload.insert(
        "state_artifact_path".to_string(),
        Value::String(".supervisor_state.json".to_string()),
    );
    payload.insert(
        "schema_expectations".to_string(),
        Value::Object(schema_expectations),
    );
    payload.insert(
        "cross_artifact_alignment".to_string(),
        Value::Object(cross_artifact_alignment),
    );
    payload.insert(
        "compatibility_rules".to_string(),
        Value::Object(compatibility_rules),
    );
    payload
}

pub fn build_control_plane_contract_descriptors() -> Map<String, Value> {
    let mut payload = Map::new();
    payload.insert(
        EXECUTION_CONTROLLER_CONTRACT_ARTIFACT_ID.to_string(),
        Value::Object(build_execution_controller_contract()),
    );
    payload.insert(
        DELEGATION_CONTRACT_ARTIFACT_ID.to_string(),
        Value::Object(build_delegation_contract()),
    );
    payload.insert(
        SUPERVISOR_STATE_CONTRACT_ARTIFACT_ID.to_string(),
        Value::Object(build_supervisor_state_contract()),
    );
    payload
}

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

fn string_array(values: &[&str]) -> Value {
    Value::Array(
        values
            .iter()
            .map(|value| Value::String((*value).to_string()))
            .collect(),
    )
}

fn value_object<const N: usize>(pairs: [(&str, Value); N]) -> Value {
    let mut object = Map::new();
    for (key, value) in pairs {
        object.insert(key.to_string(), value);
    }
    Value::Object(object)
}

fn default_framework_profile_version() -> String {
    "0.1.0".to_string()
}

fn default_runtime_family() -> String {
    "portable".to_string()
}

fn default_host_family() -> String {
    "codex".to_string()
}

fn default_core_capabilities() -> Vec<String> {
    REQUIRED_CORE_CAPABILITIES
        .iter()
        .map(|value| (*value).to_string())
        .collect()
}

fn default_rules_bundle() -> Value {
    Value::String("default".to_string())
}

fn default_skill_bundle() -> Value {
    Value::String("default".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_profile() -> FrameworkProfileContract {
        serde_json::from_value(json!({
            "profile_id": "fusion-default",
            "display_name": "Fusion Default",
            "framework_profile_version": "0.1.0",
            "runtime_family": "portable",
            "host_family": "codex",
            "core_capabilities": ["runtime", "memory", "artifact", "orchestration"],
            "rules_bundle": {"rules": [{"id": "outer-owned"}]},
            "skill_bundle": {"skills": ["router", "memory-bridge"]},
            "session_policy": {"mode": "bounded", "approval_mode": "manual"},
            "tool_policy": {"shell": "allow"},
            "approval_policy": {"mode": "manual"},
            "loadout_policy": {"default": "portable"},
            "framework_surface_policy": {
                "kernel": {"canonical_axes": ["routing", "memory", "continuity", "codex_host_payload"]},
                "default_surface": {"default_loadouts": ["default_surface_loadout"]}
            },
            "artifact_contract": {"layout": "stable-v1"},
            "model_policy": {"provider": "openai", "model": "gpt-5"},
            "memory_mounts": ["project"],
            "mcp_servers": ["local-memory"]
        }))
        .expect("sample profile should deserialize")
    }

    #[test]
    fn profile_bundle_builds_first_class_rust_profile() {
        let bundle = build_profile_bundle(&sample_profile()).expect("bundle should build");
        assert_eq!(bundle.profile_id, "fusion-default");
        assert_eq!(bundle.capabilities.core.len(), 4);
        assert_eq!(bundle.host_family, "codex");
        assert_eq!(
            bundle.codex_adapter["metadata"]["adapter_id"],
            Value::String("codex_adapter".to_string())
        );
        assert_eq!(
            bundle.codex_adapter["execution_surface"]["entrypoint_kind"],
            Value::String("codex".to_string())
        );
        assert_eq!(
            bundle.codex_adapter["execution_surface"]["controller_is_cli"],
            Value::Bool(false)
        );
        assert_eq!(
            bundle.codex_adapter["host_adapter_payload"]["host_cli"],
            Value::String("codex".to_string())
        );
        let serialized = serde_json::to_value(&bundle).expect("bundle should serialize");
        assert!(serialized.get("cli_common_adapter").is_none());
        assert!(serialized.get("codex_cli_adapter").is_none());
        assert!(serialized.get("codex_desktop_adapter").is_none());
        assert!(serialized.get("codex_common_adapter").is_none());
        assert!(serialized.get("cli_family_capability_discovery").is_none());
        assert!(serialized.get("cli_family_parity_snapshot").is_none());
        assert!(serialized.get("codex_dual_entry_parity_snapshot").is_none());
        assert!(serialized.get("execution_controller_contract").is_none());
        assert!(serialized.get("delegation_contract").is_none());
        assert!(serialized.get("supervisor_state_contract").is_none());
        assert!(serialized
            .get("execution_kernel_live_fallback_retirement_status")
            .is_none());
        assert!(serialized
            .get("execution_kernel_live_response_serialization_contract")
            .is_none());
    }

    #[test]
    fn profile_bundle_keeps_workspace_bootstrap_single_sourced() {
        let mut profile = sample_profile();
        profile.memory_mounts = vec![json!({
            "mount_id": "project",
            "source": ".codex/memory"
        })];
        profile.workspace_bootstrap = serde_json::from_value(json!({
            "skill_bridge": {
                "project_dir": ".codex/skills"
            },
            "bridges": {
                "memory": {
                    "bridge_dir": ".memory-shadow",
                    "mounts": []
                }
            }
        }))
        .expect("workspace bootstrap should deserialize");

        let bundle = build_profile_bundle(&profile).expect("bundle should build");
        let expected_bootstrap = json!({
            "skill_bridge": {
                "project_dir": ".codex/skills"
            },
            "bridges": {
                "memory": {
                    "bridge_dir": ".memory-shadow",
                    "mounts": []
                },
                "skills": {
                    "project_dir": ".codex/skills"
                }
            }
        });

        assert_eq!(
            Value::Object(bundle.workspace_bootstrap.clone()),
            expected_bootstrap
        );
        assert_eq!(
            bundle.codex_adapter["common_contract"]["workspace_bootstrap"],
            expected_bootstrap
        );
        assert_eq!(
            bundle.codex_adapter["runtime_surface"]["workspace_bootstrap"],
            expected_bootstrap
        );
        assert!(bundle.codex_adapter.get("bridge_contract").is_none());
        assert!(bundle.codex_adapter.get("source_contract").is_none());
    }

    #[test]
    fn profile_bundle_resolves_codex_capability_requirements() {
        let mut profile = sample_profile();
        profile.host_capability_requirements = serde_json::from_value(json!({
            "default": {
                "required_host_capabilities": ["artifact_contract"]
            },
            "codex": {
                "required_host_capabilities": ["batch_execution"]
            }
        }))
        .expect("host capability requirements should deserialize");

        let bundle = build_profile_bundle(&profile).expect("bundle should build");

        assert_eq!(
            Value::Object(bundle.host_capability_requirements.clone()),
            json!({
                "default": {
                    "required_host_capabilities": ["artifact_contract"]
                },
                "codex": {
                    "required_host_capabilities": ["batch_execution"]
                }
            })
        );
        assert_eq!(
            bundle.codex_adapter["host_capability_requirements"],
            json!({
                "required_host_capabilities": [
                    "artifact_contract",
                    "batch_execution"
                ]
            })
        );
    }

    #[test]
    fn profile_bundle_legacy_alias_opt_in_stays_out_of_runtime_bundle() {
        let bundle = build_profile_bundle(&sample_profile()).expect("bundle should build");
        let serialized = serde_json::to_value(&bundle).expect("bundle should serialize");
        assert!(serialized.get("compatibility_lane").is_none());
        assert!(serialized.get("codex_desktop_host_adapter").is_none());
        assert!(serialized
            .get("codex_desktop_alias_retirement_status")
            .is_none());
    }

    #[test]
    fn codex_adapter_preserves_framework_core_truth() {
        let bundle = build_profile_bundle(&sample_profile()).expect("bundle should build");
        assert_eq!(
            bundle.codex_adapter["common_contract"]["framework_surface_policy"]["default_surface"]
                ["default_loadouts"],
            json!(["default_surface_loadout"])
        );
        assert_eq!(
            bundle.codex_adapter["execution_surface"]["controller_is_cli"],
            Value::Bool(false)
        );
    }

    #[test]
    fn control_plane_contract_descriptors_share_one_rust_source() {
        let descriptors = build_control_plane_contract_descriptors();

        assert_eq!(
            descriptors[EXECUTION_CONTROLLER_CONTRACT_ARTIFACT_ID],
            Value::Object(build_execution_controller_contract())
        );
        assert_eq!(
            descriptors[DELEGATION_CONTRACT_ARTIFACT_ID],
            Value::Object(build_delegation_contract())
        );
        assert_eq!(
            descriptors[SUPERVISOR_STATE_CONTRACT_ARTIFACT_ID],
            Value::Object(build_supervisor_state_contract())
        );
        assert_eq!(descriptors.len(), 3);
    }

    #[test]
    fn codex_artifact_bundle_exposes_first_class_outputs() {
        let artifacts =
            build_codex_artifact_bundle(&sample_profile()).expect("artifacts should build");
        assert_eq!(artifacts.len(), 1);
        assert_eq!(
            artifacts["codex_adapter"]["host_adapter_payload"]["gpt_model_path_contract"]
                ["preferred_for_gpt_family"],
            Value::Bool(true)
        );
        assert_eq!(
            artifacts["codex_adapter"]["execution_surface"]["controller_is_cli"],
            Value::Bool(false)
        );
        assert!(!artifacts.contains_key("codex_desktop_host_adapter"));
    }

    #[test]
    fn codex_artifact_bundle_ignores_removed_legacy_alias_opt_in() {
        let artifacts =
            build_codex_artifact_bundle(&sample_profile()).expect("artifacts should build");
        assert_eq!(artifacts.len(), 1);
        assert!(artifacts.contains_key("codex_adapter"));
        assert!(!artifacts.contains_key("cli_common_adapter"));
        assert!(!artifacts.contains_key("codex_cli_adapter"));
        assert!(!artifacts.contains_key("codex_desktop_adapter"));
        assert!(!artifacts.contains_key("codex_common_adapter"));
        assert!(!artifacts.contains_key("cli_family_capability_discovery"));
        assert!(!artifacts.contains_key("cli_family_parity_snapshot"));
        assert!(!artifacts.contains_key("codex_dual_entry_parity_snapshot"));
        assert!(!artifacts.contains_key("execution_kernel_live_fallback_retirement_status"));
        assert!(!artifacts.contains_key("execution_kernel_live_response_serialization_contract"));
        assert!(!artifacts.contains_key("codex_desktop_host_adapter"));
        assert!(!artifacts.contains_key("codex_desktop_alias_retirement_status"));
    }

    #[test]
    fn validation_rejects_non_codex_host_family() {
        let mut profile = sample_profile();
        profile.host_family = "legacy-host".to_string();
        let error = build_profile_bundle(&profile).expect_err("should reject pinned host family");
        assert!(error.contains("must be pinned to Codex"));
    }

    #[test]
    fn validation_rejects_host_specific_metadata_in_framework_truth() {
        let mut profile = sample_profile();
        profile
            .metadata
            .insert("settings_paths".to_string(), json!([".codex/config.toml"]));
        let error = build_profile_bundle(&profile)
            .expect_err("should reject host-specific metadata in framework truth");
        assert!(error.contains("Codex-core-only"));
        assert!(error.contains("settings_paths"));
    }
}
