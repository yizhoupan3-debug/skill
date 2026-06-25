use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const REQUIRED_CORE_CAPABILITIES: [&str; 3] = ["runtime", "artifact", "orchestration"];
pub const RUNTIME_SURFACE_FIELDS: [&str; 3] = ["routing", "continuity_contract", "host_projection"];
pub const CAPABILITY_SURFACE_FIELDS: [&str; 12] = [
    "artifact_contract",
    "mcp_servers",
    "tool_policy",
    "approval_policy",
    "loadout_policy",
    "framework_surface_policy",
    "workspace_bootstrap",
    "session_contract",
    "execution_protocol_contract",
    "execution_controller_contract",
    "delegation_contract",
    "supervisor_state_contract",
];
pub const HOST_SPECIFIC_METADATA_KEYS: &[&str] = &[
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
    "structured_output_modes",
    "subagent_paths",
    "supports_batch",
    "supports_ci",
    "supports_cron",
    "thread_binding",
    "transport",
];
pub const EXECUTION_CONTROLLER_CONTRACT_ARTIFACT_ID: &str = "execution_controller_contract";
pub const EXECUTION_PROTOCOL_CONTRACT_ARTIFACT_ID: &str = "execution_protocol_contract";
pub const DELEGATION_CONTRACT_ARTIFACT_ID: &str = "delegation_contract";
pub const SUPERVISOR_STATE_CONTRACT_ARTIFACT_ID: &str = "supervisor_state_contract";

pub struct HostProfileBuildContext<'a> {
    pub normalized_mcp_servers: &'a [Value],
    pub workspace_bootstrap: &'a Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub mcp_servers: Vec<Value>,
    pub workspace_bootstrap: Map<String, Value>,
    pub host_capability_requirements: Map<String, Value>,
    pub metadata: Map<String, Value>,
    pub host_profiles: Map<String, Value>,
    pub full_host_profiles: Map<String, Value>,
    pub host_payloads: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityBundle {
    pub core: Vec<String>,
    pub optional: Vec<String>,
}

fn default_framework_profile_version() -> String {
    "0.1.0".to_string()
}

fn default_runtime_family() -> String {
    "portable".to_string()
}

fn default_host_family() -> String {
    "shared-rust-core".to_string()
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
