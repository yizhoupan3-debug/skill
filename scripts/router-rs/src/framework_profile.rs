use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

const REQUIRED_CORE_CAPABILITIES: [&str; 4] = ["runtime", "memory", "artifact", "orchestration"];
const COMMON_PARITY_FIELDS: [&str; 12] = [
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
const CODEX_COMMON_HOST_CAPABILITIES: [&str; 9] = [
    "artifact_contract",
    "memory_mounts",
    "mcp_servers",
    "tool_policy",
    "approval_policy",
    "loadout_policy",
    "framework_surface_policy",
    "workspace_bootstrap",
    "session_contract",
];
const CODEX_DESKTOP_HOST_CAPABILITIES: [&str; 6] = [
    "local_runtime",
    "artifact_contract",
    "memory_mounts",
    "mcp_servers",
    "automation_bridge",
    "orchestration_control",
];
const CODEX_CLI_HOST_CAPABILITIES: [&str; 8] = [
    "artifact_contract",
    "memory_mounts",
    "mcp_servers",
    "workspace_bootstrap",
    "batch_execution",
    "cron_execution",
    "ci_runner",
    "non_interactive_entrypoint",
];
const CLAUDE_CODE_HOST_CAPABILITIES: [&str; 16] = [
    "artifact_contract",
    "memory_mounts",
    "mcp_servers",
    "workspace_bootstrap",
    "batch_execution",
    "ci_runner",
    "non_interactive_entrypoint",
    "context_file",
    "settings_json",
    "settings_scope_hierarchy",
    "subagent_registry",
    "managed_policy",
    "hook_registry",
    "hook_policy",
    "hook_browser",
    "checkpoint_restore",
];
const GEMINI_CLI_HOST_CAPABILITIES: [&str; 9] = [
    "artifact_contract",
    "memory_mounts",
    "mcp_servers",
    "workspace_bootstrap",
    "batch_execution",
    "ci_runner",
    "non_interactive_entrypoint",
    "context_file",
    "settings_json",
];
const CLI_FAMILY_HOST_CAPABILITIES: [&str; 9] = [
    "artifact_contract",
    "memory_mounts",
    "mcp_servers",
    "tool_policy",
    "approval_policy",
    "loadout_policy",
    "framework_surface_policy",
    "workspace_bootstrap",
    "session_contract",
];
const HOST_SPECIFIC_METADATA_KEYS: &[&str] = &[
    "adapter_id",
    "adapter_alias_of",
    "automation_bridge_required",
    "canonical_adapter_id",
    "checkpointing_supported",
    "claude_directory_features",
    "config_root_env_var",
    "context_files",
    "controller_is_cli",
    "entrypoint_kind",
    "host_cli",
    "host_id",
    "hook_control_settings",
    "hook_definition_sources",
    "hook_environment_markers",
    "hook_event_names",
    "hook_handler_types",
    "hook_inspection_commands",
    "managed_mcp_paths",
    "managed_settings_paths",
    "mcp_config_paths",
    "plugin_hook_manifest_paths",
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
const CLI_COMMON_ADAPTER_ID: &str = "cli_common_adapter";
const CODEX_COMMON_ADAPTER_ID: &str = "codex_common_adapter";
const CODEX_CLI_ADAPTER_ID: &str = "codex_cli_adapter";
const CLAUDE_CODE_ADAPTER_ID: &str = "claude_code_adapter";
const GEMINI_CLI_ADAPTER_ID: &str = "gemini_cli_adapter";
const CODEX_DESKTOP_ADAPTER_ID: &str = "codex_desktop_adapter";
const DEFAULT_HOST_PEER_SET: &[&str] = &[
    CODEX_DESKTOP_ADAPTER_ID,
    CODEX_CLI_ADAPTER_ID,
    CLAUDE_CODE_ADAPTER_ID,
    GEMINI_CLI_ADAPTER_ID,
];
const CLI_FAMILY_TARGETS: [&str; 3] = [
    CODEX_CLI_ADAPTER_ID,
    CLAUDE_CODE_ADAPTER_ID,
    GEMINI_CLI_ADAPTER_ID,
];
const CLI_FAMILY_PARITY_ARTIFACT_ID: &str = "cli_family_parity_snapshot";
const LEGACY_CODEX_DESKTOP_ADAPTER_ID: &str = "codex_desktop_host_adapter";
const EXECUTION_CONTROLLER_CONTRACT_ARTIFACT_ID: &str = "execution_controller_contract";
const DELEGATION_CONTRACT_ARTIFACT_ID: &str = "delegation_contract";
const SUPERVISOR_STATE_CONTRACT_ARTIFACT_ID: &str = "supervisor_state_contract";
const EXECUTION_KERNEL_LIVE_FALLBACK_RETIREMENT_ARTIFACT_ID: &str =
    "execution_kernel_live_fallback_retirement_status";
const EXECUTION_KERNEL_LIVE_RESPONSE_SERIALIZATION_ARTIFACT_ID: &str =
    "execution_kernel_live_response_serialization_contract";

#[derive(Clone, Copy)]
struct AdapterDescriptor<'a> {
    adapter_id: &'a str,
    host_id: &'a str,
    transport: &'a str,
    host_capabilities: &'a [&'a str],
}

struct AdapterBuildContext<'a> {
    normalized_memory_mounts: &'a [Value],
    normalized_mcp_servers: &'a [Value],
    workspace_bootstrap: &'a Map<String, Value>,
}

struct CliFamilyAdapterInputs<'a> {
    shared_contract: &'a Map<String, Value>,
    controller_boundary: &'a Map<String, Value>,
    host_projection: &'a Map<String, Value>,
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
    pub companion_projection: CompanionProjection,
    pub cli_common_adapter: Value,
    pub codex_common_adapter: Value,
    pub codex_desktop_adapter: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compatibility_lane: Option<CompatibilityLane>,
    pub codex_cli_adapter: Value,
    pub claude_code_adapter: Value,
    pub gemini_cli_adapter: Value,
    pub cli_family_capability_discovery: Value,
    pub cli_family_parity_snapshot: Value,
    pub codex_dual_entry_parity_snapshot: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codex_desktop_alias_retirement_status: Option<Value>,
    pub execution_controller_contract: Value,
    pub delegation_contract: Value,
    pub supervisor_state_contract: Value,
    pub execution_kernel_live_fallback_retirement_status: Value,
    pub execution_kernel_live_response_serialization_contract: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompatibilityLane {
    pub codex_desktop_host_adapter: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityBundle {
    pub core: Vec<String>,
    pub optional: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompanionProjection {
    #[serde(rename = "presetRules")]
    pub preset_rules: Vec<Value>,
    #[serde(rename = "enabledSkills")]
    pub enabled_skills: Vec<Value>,
    #[serde(rename = "sessionMode")]
    pub session_mode: Value,
    #[serde(rename = "aionrsConfig")]
    pub aionrs_config: Value,
    #[serde(rename = "mcpConfig")]
    pub mcp_config: Value,
    #[serde(rename = "workspaceBootstrap")]
    pub workspace_bootstrap: Value,
    pub bridges: Value,
    #[serde(rename = "toolApprovalMapping")]
    pub tool_approval_mapping: Value,
    #[serde(rename = "fallbackSemantics")]
    pub fallback_semantics: Value,
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
    build_profile_bundle_with_legacy_alias(profile, false)
}

pub fn build_profile_bundle_with_legacy_alias(
    profile: &FrameworkProfileContract,
    include_legacy_alias_artifact: bool,
) -> Result<ProfileBundle, String> {
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
    let controller_boundary = build_cli_common_controller_boundary();
    let parity_contract = build_codex_parity_contract();
    let adapter_context = AdapterBuildContext {
        normalized_memory_mounts: &normalized_memory_mounts,
        normalized_mcp_servers: &normalized_mcp_servers,
        workspace_bootstrap: &workspace_bootstrap,
    };
    let cli_common_adapter = build_cli_common_adapter(
        profile,
        &normalized_memory_mounts,
        &normalized_mcp_servers,
        &workspace_bootstrap,
        &shared_contract,
        &controller_boundary,
        &parity_contract,
    );
    let codex_common_adapter = build_codex_common_adapter(
        profile,
        &normalized_memory_mounts,
        &normalized_mcp_servers,
        &workspace_bootstrap,
        &shared_contract,
        &controller_boundary,
        &parity_contract,
    );
    let codex_cli_adapter = build_codex_cli_adapter(
        profile,
        &normalized_memory_mounts,
        &normalized_mcp_servers,
        &workspace_bootstrap,
        &shared_contract,
        &controller_boundary,
    );
    let claude_code_adapter = build_cli_family_host_adapter(
        profile,
        AdapterDescriptor {
            adapter_id: CLAUDE_CODE_ADAPTER_ID,
            host_id: "claude-code",
            transport: "headless-exec",
            host_capabilities: &CLAUDE_CODE_HOST_CAPABILITIES,
        },
        &adapter_context,
        CliFamilyAdapterInputs {
            shared_contract: &shared_contract,
            controller_boundary: &controller_boundary,
            host_projection: &build_claude_host_projection(),
        },
    );
    let gemini_cli_adapter = build_cli_family_host_adapter(
        profile,
        AdapterDescriptor {
            adapter_id: GEMINI_CLI_ADAPTER_ID,
            host_id: "gemini-cli",
            transport: "headless-exec",
            host_capabilities: &GEMINI_CLI_HOST_CAPABILITIES,
        },
        &adapter_context,
        CliFamilyAdapterInputs {
            shared_contract: &shared_contract,
            controller_boundary: &controller_boundary,
            host_projection: &build_gemini_host_projection(),
        },
    );
    let cli_family_parity_snapshot = build_cli_family_parity_snapshot(
        &controller_boundary,
        &codex_cli_adapter,
        &claude_code_adapter,
        &gemini_cli_adapter,
    )?;
    let cli_family_capability_discovery = build_cli_family_capability_discovery(
        profile,
        &controller_boundary,
        &codex_cli_adapter,
        &claude_code_adapter,
        &gemini_cli_adapter,
    )?;
    let codex_desktop_adapter = build_codex_desktop_adapter(
        profile,
        &normalized_memory_mounts,
        &normalized_mcp_servers,
        &workspace_bootstrap,
        &shared_contract,
        &controller_boundary,
    );
    let compatibility_lane = if include_legacy_alias_artifact {
        Some(Value::Object(build_codex_desktop_host_adapter(
            &codex_desktop_adapter,
        )?))
    } else {
        None
    };
    let codex_dual_entry_parity_snapshot = build_codex_dual_entry_parity_snapshot(
        &controller_boundary,
        &codex_desktop_adapter,
        &codex_cli_adapter,
    )?;
    let codex_desktop_alias_retirement_status = if include_legacy_alias_artifact {
        Some(Value::Object(build_codex_desktop_alias_retirement_status()))
    } else {
        None
    };
    let mut control_plane_contracts = build_control_plane_contract_descriptors();
    let execution_controller_contract = control_plane_contracts
        .remove(EXECUTION_CONTROLLER_CONTRACT_ARTIFACT_ID)
        .ok_or_else(|| "missing execution controller contract descriptor".to_string())?;
    let delegation_contract = control_plane_contracts
        .remove(DELEGATION_CONTRACT_ARTIFACT_ID)
        .ok_or_else(|| "missing delegation contract descriptor".to_string())?;
    let supervisor_state_contract = control_plane_contracts
        .remove(SUPERVISOR_STATE_CONTRACT_ARTIFACT_ID)
        .ok_or_else(|| "missing supervisor state contract descriptor".to_string())?;
    let execution_kernel_live_fallback_retirement_status = control_plane_contracts
        .remove(EXECUTION_KERNEL_LIVE_FALLBACK_RETIREMENT_ARTIFACT_ID)
        .ok_or_else(|| "missing execution-kernel fallback retirement descriptor".to_string())?;
    let execution_kernel_live_response_serialization_contract = control_plane_contracts
        .remove(EXECUTION_KERNEL_LIVE_RESPONSE_SERIALIZATION_ARTIFACT_ID)
        .ok_or_else(|| {
            "missing execution-kernel live response serialization descriptor".to_string()
        })?;

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
        companion_projection: CompanionProjection {
            preset_rules: normalize_bundle_items(
                &profile.rules_bundle,
                &["rules", "items"],
                "rule",
            ),
            enabled_skills: normalize_bundle_items(
                &profile.skill_bundle,
                &["skills", "items"],
                "skill_id",
            ),
            session_mode: compile_session_mode(&profile.session_policy),
            aionrs_config: compile_aionrs_config(&profile.model_policy),
            mcp_config: value_object([("servers", Value::Array(normalized_mcp_servers))]),
            workspace_bootstrap: Value::Object(workspace_bootstrap.clone()),
            bridges: workspace_bootstrap
                .get("bridges")
                .cloned()
                .unwrap_or_else(|| Value::Object(Map::new())),
            tool_approval_mapping: compile_tool_approval_mapping(profile),
            fallback_semantics: value_object([
                ("requires_aionrs", Value::Bool(true)),
                (
                    "portable_core_preserved",
                    Value::Array(
                        REQUIRED_CORE_CAPABILITIES
                            .iter()
                            .map(|cap| Value::String((*cap).to_string()))
                            .collect(),
                    ),
                ),
                (
                    "fallback_adapter",
                    Value::String(CODEX_DESKTOP_ADAPTER_ID.to_string()),
                ),
                (
                    "legacy_fallback_aliases",
                    Value::Array(vec![Value::String(
                        LEGACY_CODEX_DESKTOP_ADAPTER_ID.to_string(),
                    )]),
                ),
            ]),
        },
        cli_common_adapter: Value::Object(cli_common_adapter),
        codex_common_adapter: Value::Object(codex_common_adapter),
        codex_desktop_adapter: Value::Object(codex_desktop_adapter),
        compatibility_lane: compatibility_lane.map(|codex_desktop_host_adapter| {
            CompatibilityLane {
                codex_desktop_host_adapter,
            }
        }),
        codex_cli_adapter: Value::Object(codex_cli_adapter),
        claude_code_adapter: Value::Object(claude_code_adapter),
        gemini_cli_adapter: Value::Object(gemini_cli_adapter),
        cli_family_capability_discovery: Value::Object(cli_family_capability_discovery),
        cli_family_parity_snapshot: Value::Object(cli_family_parity_snapshot),
        codex_dual_entry_parity_snapshot: Value::Object(codex_dual_entry_parity_snapshot),
        codex_desktop_alias_retirement_status,
        execution_controller_contract,
        delegation_contract,
        supervisor_state_contract,
        execution_kernel_live_fallback_retirement_status,
        execution_kernel_live_response_serialization_contract,
    })
}

pub fn build_codex_artifact_bundle(
    profile: &FrameworkProfileContract,
    include_legacy_alias_artifact: bool,
) -> Result<Map<String, Value>, String> {
    let bundle = build_profile_bundle_with_legacy_alias(profile, include_legacy_alias_artifact)?;
    let mut artifacts = Map::new();
    artifacts.insert("cli_common_adapter".to_string(), bundle.cli_common_adapter);
    artifacts.insert(
        "codex_common_adapter".to_string(),
        bundle.codex_common_adapter,
    );
    artifacts.insert(
        "codex_desktop_adapter".to_string(),
        bundle.codex_desktop_adapter,
    );
    artifacts.insert("codex_cli_adapter".to_string(), bundle.codex_cli_adapter);
    artifacts.insert(
        "claude_code_adapter".to_string(),
        bundle.claude_code_adapter,
    );
    artifacts.insert("gemini_cli_adapter".to_string(), bundle.gemini_cli_adapter);
    if let Some(legacy_alias) = bundle
        .compatibility_lane
        .map(|compatibility_lane| compatibility_lane.codex_desktop_host_adapter)
    {
        artifacts.insert("codex_desktop_host_adapter".to_string(), legacy_alias);
    }
    artifacts.insert(
        "cli_family_capability_discovery".to_string(),
        bundle.cli_family_capability_discovery,
    );
    artifacts.insert(
        "cli_family_parity_snapshot".to_string(),
        bundle.cli_family_parity_snapshot,
    );
    artifacts.insert(
        "codex_dual_entry_parity_snapshot".to_string(),
        bundle.codex_dual_entry_parity_snapshot,
    );
    if include_legacy_alias_artifact {
        let codex_desktop_alias_retirement_status = bundle
            .codex_desktop_alias_retirement_status
            .ok_or_else(|| {
                "missing codex desktop alias retirement status for explicit continuity lane"
                    .to_string()
            })?;
        artifacts.insert(
            "codex_desktop_alias_retirement_status".to_string(),
            codex_desktop_alias_retirement_status,
        );
    }
    artifacts.insert(
        EXECUTION_CONTROLLER_CONTRACT_ARTIFACT_ID.to_string(),
        bundle.execution_controller_contract,
    );
    artifacts.insert(
        DELEGATION_CONTRACT_ARTIFACT_ID.to_string(),
        bundle.delegation_contract,
    );
    artifacts.insert(
        SUPERVISOR_STATE_CONTRACT_ARTIFACT_ID.to_string(),
        bundle.supervisor_state_contract,
    );
    artifacts.insert(
        EXECUTION_KERNEL_LIVE_FALLBACK_RETIREMENT_ARTIFACT_ID.to_string(),
        bundle.execution_kernel_live_fallback_retirement_status,
    );
    artifacts.insert(
        EXECUTION_KERNEL_LIVE_RESPONSE_SERIALIZATION_ARTIFACT_ID.to_string(),
        bundle.execution_kernel_live_response_serialization_contract,
    );
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
    if profile.host_family == "aionrs" {
        return Err("framework core must not be pinned directly to aionrs".to_string());
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
            "framework profile metadata must stay host-neutral; move host-specific keys into adapter projections: {}",
            host_specific_metadata.join(", ")
        ));
    }
    Ok(())
}

fn normalize_bundle_items(bundle: &Value, list_keys: &[&str], fallback_field: &str) -> Vec<Value> {
    match bundle {
        Value::Object(map) => {
            for key in list_keys {
                if let Some(Value::Array(items)) = map.get(*key) {
                    return items
                        .iter()
                        .map(|item| match item {
                            Value::Object(obj) => Value::Object(obj.clone()),
                            other => value_object([(fallback_field, other.clone())]),
                        })
                        .collect();
                }
            }
            vec![Value::Object(map.clone())]
        }
        Value::Array(items) => items
            .iter()
            .map(|item| match item {
                Value::Object(obj) => Value::Object(obj.clone()),
                other => value_object([(fallback_field, other.clone())]),
            })
            .collect(),
        other => vec![value_object([("bundle_id", other.clone())])],
    }
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
                ("bridge_dir", Value::String(".aionrs/skills".to_string())),
            ])
        });
        bridges.insert("skills".to_string(), skills_bridge);
    }
    if !bridges.contains_key("memory") {
        let memory_bridge = bootstrap.get("memory_bridge").cloned().unwrap_or_else(|| {
            value_object([
                (
                    "bridge_dir",
                    Value::String(".aionrs-memory-bridge".to_string()),
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

fn compile_aionrs_config(model_policy: &Map<String, Value>) -> Value {
    let config_keys = [
        "provider",
        "model",
        "profile",
        "base_url",
        "endpoint",
        "temperature",
        "max_tokens",
        "max_output_tokens",
        "headers",
        "compat_mode",
    ];
    let mut config = Map::new();
    let mut extras = Map::new();
    for (key, value) in model_policy {
        if config_keys.contains(&key.as_str()) {
            config.insert(key.clone(), value.clone());
        } else {
            extras.insert(key.clone(), value.clone());
        }
    }

    let requested_provider = model_policy
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_lowercase();
    let builtin_provider_path = matches!(
        requested_provider.as_str(),
        "" | "anthropic" | "openai" | "aws-bedrock" | "bedrock"
    );

    let mut provider_boundary = Map::new();
    provider_boundary.insert(
        "provider_managed_by".to_string(),
        Value::String("aionrs-provider-layer".to_string()),
    );
    provider_boundary.insert(
        "supports_builtin_provider_path".to_string(),
        Value::Bool(builtin_provider_path),
    );
    provider_boundary.insert(
        "compatible_entry_required".to_string(),
        Value::Bool(!requested_provider.is_empty() && !builtin_provider_path),
    );
    provider_boundary.insert(
        "framework_core_provider_pinned".to_string(),
        Value::Bool(false),
    );
    if !extras.is_empty() {
        provider_boundary.insert("framework_model_extras".to_string(), Value::Object(extras));
    }

    value_object([
        ("config", Value::Object(config)),
        ("provider_boundary", Value::Object(provider_boundary)),
    ])
}

fn compile_tool_approval_mapping(profile: &FrameworkProfileContract) -> Value {
    value_object([
        ("tool_policy", Value::Object(profile.tool_policy.clone())),
        (
            "approval_policy",
            Value::Object(profile.approval_policy.clone()),
        ),
        (
            "loadout_policy",
            Value::Object(profile.loadout_policy.clone()),
        ),
        (
            "event_map",
            value_object([
                (
                    "request",
                    Value::String("tool.approval.request".to_string()),
                ),
                (
                    "approved",
                    Value::String("tool.approval.approved".to_string()),
                ),
                ("denied", Value::String("tool.approval.denied".to_string())),
            ]),
        ),
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

fn build_cli_common_controller_boundary() -> Map<String, Value> {
    let mut boundary = Map::new();
    boundary.insert(
        "framework_truth".to_string(),
        Value::String("framework_core".to_string()),
    );
    boundary.insert(
        "shared_adapter".to_string(),
        Value::String(CLI_COMMON_ADAPTER_ID.to_string()),
    );
    boundary.insert(
        "cli_family_entrypoints".to_string(),
        string_array(&CLI_FAMILY_TARGETS),
    );
    boundary.insert(
        "host_entrypoints".to_string(),
        string_array(DEFAULT_HOST_PEER_SET),
    );
    boundary.insert("single_source_of_truth".to_string(), Value::Bool(true));
    boundary.insert("codexcli_is_controller".to_string(), Value::Bool(false));
    boundary
}

fn build_codex_parity_contract() -> Map<String, Value> {
    let mut contract = Map::new();
    contract.insert(
        "shared_fields".to_string(),
        string_array(&COMMON_PARITY_FIELDS),
    );
    contract.insert(
        "desktop_adapter".to_string(),
        Value::String(CODEX_DESKTOP_ADAPTER_ID.to_string()),
    );
    contract.insert(
        "cli_adapters".to_string(),
        string_array(&CLI_FAMILY_TARGETS),
    );
    contract.insert(
        "cli_common_adapter".to_string(),
        Value::String(CLI_COMMON_ADAPTER_ID.to_string()),
    );
    contract.insert(
        "legacy_codex_common_adapter".to_string(),
        Value::String(CODEX_COMMON_ADAPTER_ID.to_string()),
    );
    contract
}

fn build_codex_adapter_base(
    profile: &FrameworkProfileContract,
    descriptor: AdapterDescriptor<'_>,
    context: &AdapterBuildContext<'_>,
) -> Map<String, Value> {
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
    capabilities.insert(
        "host".to_string(),
        string_array(descriptor.host_capabilities),
    );

    let mut metadata = Map::new();
    metadata.insert(
        "adapter_id".to_string(),
        Value::String(descriptor.adapter_id.to_string()),
    );
    metadata.insert(
        "host_id".to_string(),
        Value::String(descriptor.host_id.to_string()),
    );
    metadata.insert(
        "transport".to_string(),
        Value::String(descriptor.transport.to_string()),
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
        Value::Object(profile.host_capability_requirements.clone()),
    );
    payload.insert("metadata".to_string(), Value::Object(metadata));
    payload
}

fn build_cli_common_adapter(
    profile: &FrameworkProfileContract,
    normalized_memory_mounts: &[Value],
    normalized_mcp_servers: &[Value],
    workspace_bootstrap: &Map<String, Value>,
    shared_contract: &Map<String, Value>,
    controller_boundary: &Map<String, Value>,
    parity_contract: &Map<String, Value>,
) -> Map<String, Value> {
    let mut payload = build_codex_adapter_base(
        profile,
        AdapterDescriptor {
            adapter_id: CLI_COMMON_ADAPTER_ID,
            host_id: "cli-family-shared",
            transport: "host-neutral-contract",
            host_capabilities: &CLI_FAMILY_HOST_CAPABILITIES,
        },
        &AdapterBuildContext {
            normalized_memory_mounts,
            normalized_mcp_servers,
            workspace_bootstrap,
        },
    );
    payload.insert(
        "shared_contract".to_string(),
        Value::Object(shared_contract.clone()),
    );
    payload.insert(
        "bridge_contract".to_string(),
        shared_contract_workspace_bridges(shared_contract),
    );
    payload.insert(
        "controller_boundary".to_string(),
        Value::Object(controller_boundary.clone()),
    );
    payload.insert(
        "parity_contract".to_string(),
        Value::Object(parity_contract.clone()),
    );
    payload
}

fn build_runtime_surface(shared_contract: &Map<String, Value>) -> Map<String, Value> {
    let mut runtime_surface = Map::new();
    for field in COMMON_PARITY_FIELDS {
        if let Some(value) = shared_contract.get(field) {
            runtime_surface.insert(field.to_string(), value.clone());
        }
    }
    runtime_surface
}

fn shared_contract_workspace_bridges(shared_contract: &Map<String, Value>) -> Value {
    shared_contract
        .get("workspace_bootstrap")
        .and_then(Value::as_object)
        .and_then(|bootstrap| bootstrap.get("bridges"))
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()))
}

fn build_codex_common_adapter(
    profile: &FrameworkProfileContract,
    normalized_memory_mounts: &[Value],
    normalized_mcp_servers: &[Value],
    workspace_bootstrap: &Map<String, Value>,
    shared_contract: &Map<String, Value>,
    controller_boundary: &Map<String, Value>,
    parity_contract: &Map<String, Value>,
) -> Map<String, Value> {
    let mut payload = build_codex_adapter_base(
        profile,
        AdapterDescriptor {
            adapter_id: "codex_common_adapter",
            host_id: "codex-shared",
            transport: "host-neutral-contract",
            host_capabilities: &CODEX_COMMON_HOST_CAPABILITIES,
        },
        &AdapterBuildContext {
            normalized_memory_mounts,
            normalized_mcp_servers,
            workspace_bootstrap,
        },
    );
    payload.insert(
        "shared_contract".to_string(),
        Value::Object(shared_contract.clone()),
    );
    payload.insert(
        "bridge_contract".to_string(),
        shared_contract_workspace_bridges(shared_contract),
    );
    payload.insert(
        "controller_boundary".to_string(),
        Value::Object(controller_boundary.clone()),
    );
    payload.insert(
        "parity_contract".to_string(),
        Value::Object(parity_contract.clone()),
    );
    if let Some(metadata) = payload.get_mut("metadata").and_then(Value::as_object_mut) {
        metadata.insert(
            "adapter_alias_of".to_string(),
            Value::String(CLI_COMMON_ADAPTER_ID.to_string()),
        );
        metadata.insert(
            "canonical_adapter_id".to_string(),
            Value::String(CLI_COMMON_ADAPTER_ID.to_string()),
        );
    }
    if let Some(parity_contract) = payload
        .get_mut("parity_contract")
        .and_then(Value::as_object_mut)
    {
        parity_contract.insert(
            "compatibility_aliases".to_string(),
            Value::Array(vec![Value::String(CODEX_COMMON_ADAPTER_ID.to_string())]),
        );
    }
    payload
}

fn build_cli_family_host_adapter(
    profile: &FrameworkProfileContract,
    descriptor: AdapterDescriptor<'_>,
    context: &AdapterBuildContext<'_>,
    inputs: CliFamilyAdapterInputs<'_>,
) -> Map<String, Value> {
    let mut payload = build_codex_adapter_base(profile, descriptor, context);
    payload.insert(
        "common_contract".to_string(),
        Value::Object(inputs.shared_contract.clone()),
    );
    payload.insert(
        "controller_boundary".to_string(),
        Value::Object(inputs.controller_boundary.clone()),
    );
    payload.insert(
        "runtime_surface".to_string(),
        Value::Object(build_runtime_surface(inputs.shared_contract)),
    );
    payload.insert(
        "execution_surface".to_string(),
        value_object([
            ("entrypoint_kind", Value::String("headless".to_string())),
            ("non_interactive", Value::Bool(true)),
            (
                "supports_batch",
                Value::Bool(descriptor.host_capabilities.contains(&"batch_execution")),
            ),
            (
                "supports_cron",
                Value::Bool(descriptor.host_capabilities.contains(&"cron_execution")),
            ),
            (
                "supports_ci",
                Value::Bool(descriptor.host_capabilities.contains(&"ci_runner")),
            ),
            (
                "framework_truth",
                Value::String("framework_core".to_string()),
            ),
            ("controller_is_cli", Value::Bool(false)),
            (
                "shared_adapter",
                Value::String(CLI_COMMON_ADAPTER_ID.to_string()),
            ),
            ("host_cli", Value::String(descriptor.host_id.to_string())),
        ]),
    );
    payload.insert(
        "host_projection".to_string(),
        Value::Object(complete_cli_host_projection(
            descriptor.host_id,
            inputs.host_projection.clone(),
        )),
    );
    payload.insert(
        "fallback_semantics".to_string(),
        value_object([
            ("requires_aionrs", Value::Bool(false)),
            (
                "preserves_core_capabilities",
                string_array(&REQUIRED_CORE_CAPABILITIES),
            ),
            (
                "degrade_to",
                Value::String("generic_host_adapter".to_string()),
            ),
            (
                "shared_adapter",
                Value::String(CLI_COMMON_ADAPTER_ID.to_string()),
            ),
            (
                "desktop_peer",
                Value::String(CODEX_DESKTOP_ADAPTER_ID.to_string()),
            ),
            (
                "legacy_desktop_peer_aliases",
                Value::Array(vec![Value::String(
                    LEGACY_CODEX_DESKTOP_ADAPTER_ID.to_string(),
                )]),
            ),
        ]),
    );
    if let Some(fallback_semantics) = payload
        .get_mut("fallback_semantics")
        .and_then(Value::as_object_mut)
    {
        fallback_semantics.insert(
            "cli_family_peers".to_string(),
            Value::Array(
                CLI_FAMILY_TARGETS
                    .iter()
                    .copied()
                    .filter(|adapter_id| *adapter_id != descriptor.adapter_id)
                    .map(|adapter_id| Value::String(adapter_id.to_string()))
                    .collect(),
            ),
        );
    }
    payload
}

fn complete_cli_host_projection(
    host_cli: &str,
    projection: Map<String, Value>,
) -> Map<String, Value> {
    let mut completed = Map::new();
    completed.insert("host_cli".to_string(), Value::String(host_cli.to_string()));
    completed.insert("context_files".to_string(), Value::Array(vec![]));
    completed.insert("settings_paths".to_string(), Value::Array(vec![]));
    completed.insert("mcp_config_paths".to_string(), Value::Array(vec![]));
    completed.insert("config_root_env_var".to_string(), Value::Null);
    completed.insert("settings_scope_order".to_string(), Value::Array(vec![]));
    completed.insert("settings_scopes".to_string(), Value::Array(vec![]));
    completed.insert("subagent_paths".to_string(), Value::Array(vec![]));
    completed.insert(
        "claude_directory_features".to_string(),
        Value::Array(vec![]),
    );
    completed.insert("hook_event_names".to_string(), Value::Array(vec![]));
    completed.insert("hook_handler_types".to_string(), Value::Array(vec![]));
    completed.insert("hook_control_settings".to_string(), Value::Array(vec![]));
    completed.insert("hook_definition_sources".to_string(), Value::Array(vec![]));
    completed.insert("hook_inspection_commands".to_string(), Value::Array(vec![]));
    completed.insert(
        "plugin_hook_manifest_paths".to_string(),
        Value::Array(vec![]),
    );
    completed.insert("hook_environment_markers".to_string(), Value::Array(vec![]));
    completed.insert("managed_settings_paths".to_string(), Value::Array(vec![]));
    completed.insert("managed_mcp_paths".to_string(), Value::Array(vec![]));
    completed.insert("structured_output_modes".to_string(), Value::Array(vec![]));
    completed.insert("checkpointing_supported".to_string(), Value::Bool(false));
    for (key, value) in projection {
        completed.insert(key, value);
    }
    completed
}

fn build_codex_host_projection() -> Map<String, Value> {
    let mut projection = Map::new();
    projection.insert(
        "context_files".to_string(),
        Value::Array(vec![Value::String("AGENTS.md".to_string())]),
    );
    projection.insert(
        "settings_paths".to_string(),
        Value::Array(vec![
            Value::String("~/.codex/config.toml".to_string()),
            Value::String(".codex/config.toml".to_string()),
        ]),
    );
    projection.insert(
        "mcp_config_paths".to_string(),
        Value::Array(vec![Value::String(".codex/config.toml".to_string())]),
    );
    projection
}

fn build_claude_host_projection() -> Map<String, Value> {
    let mut projection = Map::new();
    projection.insert(
        "context_files".to_string(),
        Value::Array(vec![
            Value::String("CLAUDE.md".to_string()),
            Value::String("CLAUDE.local.md".to_string()),
        ]),
    );
    projection.insert(
        "settings_paths".to_string(),
        Value::Array(vec![
            Value::String("~/.claude/settings.json".to_string()),
            Value::String(".claude/settings.json".to_string()),
            Value::String(".claude/settings.local.json".to_string()),
        ]),
    );
    projection.insert(
        "mcp_config_paths".to_string(),
        Value::Array(vec![Value::String("~/.claude.json".to_string())]),
    );
    projection.insert(
        "config_root_env_var".to_string(),
        Value::String("CLAUDE_CONFIG_DIR".to_string()),
    );
    projection.insert(
        "settings_scope_order".to_string(),
        json!(["managed", "command_line", "local", "project", "user"]),
    );
    projection.insert(
        "settings_scopes".to_string(),
        json!([
            {
                "scope": "managed",
                "locations": [
                    "server-managed",
                    "managed-settings.json",
                    "managed-settings.d/*.json",
                    "managed-mcp.json"
                ],
                "shared_with_team": true
            },
            {
                "scope": "user",
                "locations": [
                    "~/.claude/settings.json",
                    "~/.claude/CLAUDE.md",
                    "~/.claude/agents/"
                ],
                "shared_with_team": false
            },
            {
                "scope": "project",
                "locations": [
                    ".claude/settings.json",
                    "CLAUDE.md",
                    ".claude/agents/"
                ],
                "shared_with_team": true
            },
            {
                "scope": "local",
                "locations": [
                    ".claude/settings.local.json",
                    "CLAUDE.local.md"
                ],
                "shared_with_team": false
            }
        ]),
    );
    projection.insert(
        "subagent_paths".to_string(),
        json!(["~/.claude/agents/", ".claude/agents/"]),
    );
    projection.insert(
        "claude_directory_features".to_string(),
        json!([
            ".claude/settings.json",
            ".claude/settings.local.json",
            ".claude/hooks/",
            ".claude/agents/",
            ".claude/commands/",
            ".claude/rules/",
            ".claude/output-styles/"
        ]),
    );
    projection.insert(
        "hook_event_names".to_string(),
        json!([
            "PreToolUse",
            "PostToolUse",
            "Notification",
            "Stop",
            "SubagentStart",
            "SubagentStop",
            "PreCompact",
            "PostCompact",
            "SessionStart",
            "SessionEnd",
            "UserPromptSubmit",
            "PostToolUseFailure",
            "StopFailure",
            "PermissionRequest",
            "PermissionDenied",
            "InstructionsLoaded",
            "ConfigChange",
            "CwdChanged",
            "FileChanged",
            "TaskCreated",
            "TaskCompleted",
            "WorktreeCreate",
            "WorktreeRemove",
            "TeammateIdle",
            "Elicitation",
            "ElicitationResult"
        ]),
    );
    projection.insert(
        "hook_handler_types".to_string(),
        json!(["command", "prompt", "agent", "http"]),
    );
    projection.insert(
        "hook_control_settings".to_string(),
        json!([
            "disableAllHooks",
            "allowManagedHooksOnly",
            "allowedHttpHookUrls",
            "httpHookAllowedEnvVars"
        ]),
    );
    projection.insert(
        "hook_definition_sources".to_string(),
        json!([
            {
                "source": "managed_settings",
                "locations": [
                    "/Library/Application Support/ClaudeCode/managed-settings.json",
                    "/etc/claude-code/managed-settings.json",
                    "C:/Program Files/ClaudeCode/managed-settings.json"
                ]
            },
            {
                "source": "user_settings",
                "locations": ["~/.claude/settings.json"]
            },
            {
                "source": "project_settings",
                "locations": [".claude/settings.json"]
            },
            {
                "source": "local_settings",
                "locations": [".claude/settings.local.json"]
            },
            {
                "source": "plugin_manifest",
                "locations": ["hooks/hooks.json"]
            },
            {
                "source": "agent_frontmatter",
                "locations": ["~/.claude/agents/*.md", ".claude/agents/*.md"]
            },
            {
                "source": "skill_frontmatter",
                "locations": [".claude/skills/*.md"]
            },
            {
                "source": "session",
                "locations": ["/hooks"]
            },
            {
                "source": "built_in",
                "locations": ["/hooks"]
            },
            {
                "source": "sdk",
                "locations": ["sdk_message_stream"]
            }
        ]),
    );
    projection.insert("hook_inspection_commands".to_string(), json!(["/hooks"]));
    projection.insert(
        "plugin_hook_manifest_paths".to_string(),
        json!(["hooks/hooks.json"]),
    );
    projection.insert(
        "hook_environment_markers".to_string(),
        json!([
            "CLAUDE_ENV_FILE",
            "CLAUDE_PROJECT_DIR",
            "CLAUDE_PLUGIN_ROOT",
            "CLAUDE_PLUGIN_DATA",
            "CLAUDE_CODE_REMOTE"
        ]),
    );
    projection.insert(
        "managed_settings_paths".to_string(),
        json!([
            "/Library/Application Support/ClaudeCode/managed-settings.json",
            "/etc/claude-code/managed-settings.json",
            "C:/Program Files/ClaudeCode/managed-settings.json"
        ]),
    );
    projection.insert(
        "managed_mcp_paths".to_string(),
        json!([
            "/Library/Application Support/ClaudeCode/managed-mcp.json",
            "/etc/claude-code/managed-mcp.json",
            "C:/Program Files/ClaudeCode/managed-mcp.json"
        ]),
    );
    projection.insert("checkpointing_supported".to_string(), Value::Bool(true));
    projection
}

fn build_gemini_host_projection() -> Map<String, Value> {
    let mut projection = Map::new();
    projection.insert(
        "context_files".to_string(),
        Value::Array(vec![Value::String("GEMINI.md".to_string())]),
    );
    projection.insert(
        "settings_paths".to_string(),
        Value::Array(vec![Value::String("~/.gemini/settings.json".to_string())]),
    );
    projection.insert(
        "mcp_config_paths".to_string(),
        Value::Array(vec![Value::String("~/.gemini/settings.json".to_string())]),
    );
    projection.insert(
        "structured_output_modes".to_string(),
        Value::Array(vec![
            Value::String("json".to_string()),
            Value::String("stream-json".to_string()),
        ]),
    );
    projection.insert("checkpointing_supported".to_string(), Value::Bool(true));
    projection
}

fn build_codex_desktop_adapter(
    profile: &FrameworkProfileContract,
    normalized_memory_mounts: &[Value],
    normalized_mcp_servers: &[Value],
    workspace_bootstrap: &Map<String, Value>,
    shared_contract: &Map<String, Value>,
    controller_boundary: &Map<String, Value>,
) -> Map<String, Value> {
    let mut payload = build_codex_adapter_base(
        profile,
        AdapterDescriptor {
            adapter_id: CODEX_DESKTOP_ADAPTER_ID,
            host_id: "codex-desktop",
            transport: "local-bridge",
            host_capabilities: &CODEX_DESKTOP_HOST_CAPABILITIES,
        },
        &AdapterBuildContext {
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
        "controller_boundary".to_string(),
        Value::Object(controller_boundary.clone()),
    );
    payload.insert(
        "runtime_surface".to_string(),
        Value::Object(build_runtime_surface(shared_contract)),
    );
    payload.insert(
        "entrypoint_contract".to_string(),
        value_object([
            ("entrypoint_kind", Value::String("interactive".to_string())),
            (
                "thread_binding",
                Value::String("desktop-thread".to_string()),
            ),
            ("automation_bridge_required", Value::Bool(true)),
            (
                "framework_truth",
                Value::String("framework_core".to_string()),
            ),
            (
                "shared_adapter",
                Value::String(CLI_COMMON_ADAPTER_ID.to_string()),
            ),
        ]),
    );
    payload.insert(
        "fallback_semantics".to_string(),
        value_object([
            ("requires_aionrs", Value::Bool(false)),
            (
                "preserves_core_capabilities",
                string_array(&REQUIRED_CORE_CAPABILITIES),
            ),
            (
                "degrade_to",
                Value::String("generic_host_adapter".to_string()),
            ),
            (
                "shared_adapter",
                Value::String(CLI_COMMON_ADAPTER_ID.to_string()),
            ),
            ("cli_peer", Value::String("codex_cli_adapter".to_string())),
        ]),
    );
    payload
}

fn build_codex_desktop_host_adapter(
    desktop_adapter: &Map<String, Value>,
) -> Result<Map<String, Value>, String> {
    let mut payload = desktop_adapter.clone();
    let metadata = payload
        .get_mut("metadata")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "codex desktop adapter missing metadata".to_string())?;
    metadata.insert(
        "adapter_id".to_string(),
        Value::String(LEGACY_CODEX_DESKTOP_ADAPTER_ID.to_string()),
    );
    metadata.insert(
        "adapter_alias_of".to_string(),
        Value::String(CODEX_DESKTOP_ADAPTER_ID.to_string()),
    );
    metadata.insert(
        "canonical_adapter_id".to_string(),
        Value::String(CODEX_DESKTOP_ADAPTER_ID.to_string()),
    );

    let entrypoint_contract = payload
        .get_mut("entrypoint_contract")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "codex desktop adapter missing entrypoint_contract".to_string())?;
    entrypoint_contract.insert(
        "canonical_adapter_id".to_string(),
        Value::String(CODEX_DESKTOP_ADAPTER_ID.to_string()),
    );
    entrypoint_contract.insert(
        "legacy_adapter_id".to_string(),
        Value::String(LEGACY_CODEX_DESKTOP_ADAPTER_ID.to_string()),
    );

    let fallback_semantics = payload
        .get_mut("fallback_semantics")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "codex desktop adapter missing fallback_semantics".to_string())?;
    fallback_semantics.insert(
        "legacy_adapter_id".to_string(),
        Value::String(LEGACY_CODEX_DESKTOP_ADAPTER_ID.to_string()),
    );
    Ok(payload)
}

fn build_codex_cli_adapter(
    profile: &FrameworkProfileContract,
    normalized_memory_mounts: &[Value],
    normalized_mcp_servers: &[Value],
    workspace_bootstrap: &Map<String, Value>,
    shared_contract: &Map<String, Value>,
    controller_boundary: &Map<String, Value>,
) -> Map<String, Value> {
    let mut payload = build_cli_family_host_adapter(
        profile,
        AdapterDescriptor {
            adapter_id: "codex_cli_adapter",
            host_id: "codex-cli",
            transport: "headless-exec",
            host_capabilities: &CODEX_CLI_HOST_CAPABILITIES,
        },
        &AdapterBuildContext {
            normalized_memory_mounts,
            normalized_mcp_servers,
            workspace_bootstrap,
        },
        CliFamilyAdapterInputs {
            shared_contract,
            controller_boundary,
            host_projection: &build_codex_host_projection(),
        },
    );
    payload.insert(
        "fallback_semantics".to_string(),
        value_object([
            ("requires_aionrs", Value::Bool(false)),
            (
                "preserves_core_capabilities",
                string_array(&REQUIRED_CORE_CAPABILITIES),
            ),
            (
                "degrade_to",
                Value::String("generic_host_adapter".to_string()),
            ),
            (
                "shared_adapter",
                Value::String(CLI_COMMON_ADAPTER_ID.to_string()),
            ),
            (
                "desktop_peer",
                Value::String(CODEX_DESKTOP_ADAPTER_ID.to_string()),
            ),
            (
                "cli_family_peers",
                Value::Array(vec![
                    Value::String(CLAUDE_CODE_ADAPTER_ID.to_string()),
                    Value::String(GEMINI_CLI_ADAPTER_ID.to_string()),
                ]),
            ),
            (
                "legacy_desktop_peer_aliases",
                Value::Array(vec![Value::String(
                    LEGACY_CODEX_DESKTOP_ADAPTER_ID.to_string(),
                )]),
            ),
        ]),
    );
    payload
}

fn build_codex_dual_entry_parity_snapshot(
    controller_boundary: &Map<String, Value>,
    desktop_adapter: &Map<String, Value>,
    cli_adapter: &Map<String, Value>,
) -> Result<Map<String, Value>, String> {
    let desktop_runtime_surface = desktop_adapter
        .get("runtime_surface")
        .and_then(Value::as_object)
        .ok_or_else(|| "codex desktop adapter missing runtime_surface".to_string())?;
    let cli_runtime_surface = cli_adapter
        .get("runtime_surface")
        .and_then(Value::as_object)
        .ok_or_else(|| "codex cli adapter missing runtime_surface".to_string())?;

    let mut parity_checks = Map::new();
    let mut all_checks_pass = true;
    for field in COMMON_PARITY_FIELDS {
        let passed = desktop_runtime_surface.get(field) == cli_runtime_surface.get(field);
        parity_checks.insert(field.to_string(), Value::Bool(passed));
        all_checks_pass &= passed;
    }

    let mut snapshot = Map::new();
    snapshot.insert(
        "framework_truth".to_string(),
        Value::String("framework_core".to_string()),
    );
    snapshot.insert(
        "shared_adapter".to_string(),
        Value::String(CLI_COMMON_ADAPTER_ID.to_string()),
    );
    snapshot.insert(
        "shared_adapter_aliases".to_string(),
        Value::Array(vec![Value::String("codex_common_adapter".to_string())]),
    );
    snapshot.insert(
        "compatibility_view_of".to_string(),
        Value::String(CLI_FAMILY_PARITY_ARTIFACT_ID.to_string()),
    );
    snapshot.insert(
        "codexcli_is_framework_controller".to_string(),
        Value::Bool(false),
    );
    snapshot.insert(
        "shared_contract_fields".to_string(),
        string_array(&COMMON_PARITY_FIELDS),
    );
    snapshot.insert("parity_checks".to_string(), Value::Object(parity_checks));
    snapshot.insert(
        "all_shared_contract_checks_pass".to_string(),
        Value::Bool(all_checks_pass),
    );
    snapshot.insert(
        "desktop".to_string(),
        value_object([
            (
                "adapter_id",
                Value::String(CODEX_DESKTOP_ADAPTER_ID.to_string()),
            ),
            (
                "entrypoint_kind",
                desktop_adapter["entrypoint_contract"]["entrypoint_kind"].clone(),
            ),
            (
                "shared_adapter",
                desktop_adapter["entrypoint_contract"]["shared_adapter"].clone(),
            ),
            (
                "legacy_aliases",
                Value::Array(vec![Value::String(
                    LEGACY_CODEX_DESKTOP_ADAPTER_ID.to_string(),
                )]),
            ),
        ]),
    );
    snapshot.insert(
        "cli".to_string(),
        value_object([
            ("adapter_id", Value::String("codex_cli_adapter".to_string())),
            (
                "entrypoint_kind",
                cli_adapter["execution_surface"]["entrypoint_kind"].clone(),
            ),
            (
                "shared_adapter",
                Value::String(CLI_COMMON_ADAPTER_ID.to_string()),
            ),
        ]),
    );
    snapshot.insert(
        "controller_boundary".to_string(),
        Value::Object(controller_boundary.clone()),
    );
    Ok(snapshot)
}

fn build_cli_family_parity_snapshot(
    controller_boundary: &Map<String, Value>,
    codex_cli_adapter: &Map<String, Value>,
    claude_code_adapter: &Map<String, Value>,
    gemini_cli_adapter: &Map<String, Value>,
) -> Result<Map<String, Value>, String> {
    let codex_runtime_surface = codex_cli_adapter
        .get("runtime_surface")
        .and_then(Value::as_object)
        .ok_or_else(|| "codex cli adapter missing runtime_surface".to_string())?;
    let claude_runtime_surface = claude_code_adapter
        .get("runtime_surface")
        .and_then(Value::as_object)
        .ok_or_else(|| "claude code adapter missing runtime_surface".to_string())?;
    let gemini_runtime_surface = gemini_cli_adapter
        .get("runtime_surface")
        .and_then(Value::as_object)
        .ok_or_else(|| "gemini cli adapter missing runtime_surface".to_string())?;

    let mut parity_checks = Map::new();
    let mut all_checks_pass = true;
    for field in COMMON_PARITY_FIELDS {
        let codex_value = codex_runtime_surface.get(field);
        let passed = codex_value == claude_runtime_surface.get(field)
            && codex_value == gemini_runtime_surface.get(field);
        parity_checks.insert(field.to_string(), Value::Bool(passed));
        all_checks_pass &= passed;
    }

    let mut snapshot = Map::new();
    snapshot.insert(
        "framework_truth".to_string(),
        Value::String("framework_core".to_string()),
    );
    snapshot.insert(
        "shared_adapter".to_string(),
        Value::String(CLI_COMMON_ADAPTER_ID.to_string()),
    );
    snapshot.insert(
        "shared_contract_fields".to_string(),
        string_array(&COMMON_PARITY_FIELDS),
    );
    snapshot.insert("parity_checks".to_string(), Value::Object(parity_checks));
    snapshot.insert(
        "all_shared_contract_checks_pass".to_string(),
        Value::Bool(all_checks_pass),
    );
    snapshot.insert(
        "cli_hosts".to_string(),
        value_object([
            (
                "codex_cli_adapter",
                build_cli_family_snapshot_entry(codex_cli_adapter)?,
            ),
            (
                "claude_code_adapter",
                build_cli_family_snapshot_entry(claude_code_adapter)?,
            ),
            (
                "gemini_cli_adapter",
                build_cli_family_snapshot_entry(gemini_cli_adapter)?,
            ),
        ]),
    );
    snapshot.insert(
        "controller_boundary".to_string(),
        Value::Object(controller_boundary.clone()),
    );
    Ok(snapshot)
}

fn build_cli_family_capability_discovery(
    profile: &FrameworkProfileContract,
    controller_boundary: &Map<String, Value>,
    codex_cli_adapter: &Map<String, Value>,
    claude_code_adapter: &Map<String, Value>,
    gemini_cli_adapter: &Map<String, Value>,
) -> Result<Map<String, Value>, String> {
    let codex_entry = build_cli_family_capability_discovery_entry(
        profile,
        AdapterDescriptor {
            adapter_id: CODEX_CLI_ADAPTER_ID,
            host_id: "codex-cli",
            transport: "headless-exec",
            host_capabilities: &CODEX_CLI_HOST_CAPABILITIES,
        },
        codex_cli_adapter,
    )?;
    let claude_entry = build_cli_family_capability_discovery_entry(
        profile,
        AdapterDescriptor {
            adapter_id: CLAUDE_CODE_ADAPTER_ID,
            host_id: "claude-code",
            transport: "headless-exec",
            host_capabilities: &CLAUDE_CODE_HOST_CAPABILITIES,
        },
        claude_code_adapter,
    )?;
    let gemini_entry = build_cli_family_capability_discovery_entry(
        profile,
        AdapterDescriptor {
            adapter_id: GEMINI_CLI_ADAPTER_ID,
            host_id: "gemini-cli",
            transport: "headless-exec",
            host_capabilities: &GEMINI_CLI_HOST_CAPABILITIES,
        },
        gemini_cli_adapter,
    )?;
    let mut payload = Map::new();
    payload.insert(
        "framework_truth".to_string(),
        Value::String("framework_core".to_string()),
    );
    payload.insert(
        "shared_adapter".to_string(),
        Value::String(CLI_COMMON_ADAPTER_ID.to_string()),
    );
    payload.insert(
        "discovery_contract".to_string(),
        Value::String("cli_family_host_capability_contract_v1".to_string()),
    );
    payload.insert(
        "required_core_capabilities".to_string(),
        string_array(&REQUIRED_CORE_CAPABILITIES),
    );
    payload.insert(
        "required_shared_contract_fields".to_string(),
        string_array(&COMMON_PARITY_FIELDS),
    );
    payload.insert(
        "cli_hosts".to_string(),
        value_object([
            (CODEX_CLI_ADAPTER_ID, Value::Object(codex_entry.clone())),
            (CLAUDE_CODE_ADAPTER_ID, Value::Object(claude_entry.clone())),
            (GEMINI_CLI_ADAPTER_ID, Value::Object(gemini_entry.clone())),
        ]),
    );
    let all_cli_hosts_compatible =
        [&codex_entry, &claude_entry, &gemini_entry]
            .iter()
            .all(|entry| {
                entry
                    .get("compatibility_passes")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            });
    payload.insert(
        "all_cli_hosts_compatible".to_string(),
        Value::Bool(all_cli_hosts_compatible),
    );
    payload.insert(
        "controller_boundary".to_string(),
        Value::Object(controller_boundary.clone()),
    );
    Ok(payload)
}

fn build_cli_family_capability_discovery_entry(
    profile: &FrameworkProfileContract,
    descriptor: AdapterDescriptor<'_>,
    adapter: &Map<String, Value>,
) -> Result<Map<String, Value>, String> {
    let execution_surface = adapter
        .get("execution_surface")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{} missing execution_surface", descriptor.adapter_id))?;
    let host_projection = adapter
        .get("host_projection")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{} missing host_projection", descriptor.adapter_id))?;
    let resolved_host_requirements =
        resolve_host_capability_requirements(profile, descriptor.host_id, descriptor.adapter_id);
    let required_host_capabilities = resolved_host_requirements
        .get("required_host_capabilities")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let available_host_capabilities: Vec<Value> = descriptor
        .host_capabilities
        .iter()
        .map(|value| Value::String((*value).to_string()))
        .collect();
    let available_set: HashSet<&str> = descriptor.host_capabilities.iter().copied().collect();
    let missing_host_capabilities: Vec<Value> = required_host_capabilities
        .iter()
        .filter_map(Value::as_str)
        .filter(|capability| !available_set.contains(*capability))
        .map(|capability| Value::String(capability.to_string()))
        .collect();

    let mut payload = Map::new();
    payload.insert(
        "adapter_id".to_string(),
        Value::String(descriptor.adapter_id.to_string()),
    );
    payload.insert(
        "host_id".to_string(),
        Value::String(descriptor.host_id.to_string()),
    );
    payload.insert(
        "transport".to_string(),
        Value::String(descriptor.transport.to_string()),
    );
    payload.insert(
        "entrypoint_kind".to_string(),
        execution_surface
            .get("entrypoint_kind")
            .cloned()
            .unwrap_or_else(|| Value::String(String::new())),
    );
    payload.insert(
        "shared_adapter".to_string(),
        execution_surface
            .get("shared_adapter")
            .cloned()
            .unwrap_or_else(|| Value::String(String::new())),
    );
    payload.insert(
        "framework_truth".to_string(),
        execution_surface
            .get("framework_truth")
            .cloned()
            .unwrap_or_else(|| Value::String(String::new())),
    );
    payload.insert(
        "works_without_aionrs".to_string(),
        Value::Bool(host_projection.get("host_cli").is_some()),
    );
    payload.insert(
        "available_host_capabilities".to_string(),
        Value::Array(available_host_capabilities),
    );
    payload.insert(
        "resolved_host_requirements".to_string(),
        Value::Object(resolved_host_requirements.clone()),
    );
    payload.insert(
        "required_host_capabilities".to_string(),
        Value::Array(required_host_capabilities),
    );
    payload.insert(
        "missing_host_capabilities".to_string(),
        Value::Array(missing_host_capabilities.clone()),
    );
    payload.insert(
        "supports_batch".to_string(),
        execution_surface
            .get("supports_batch")
            .cloned()
            .unwrap_or(Value::Bool(false)),
    );
    payload.insert(
        "supports_cron".to_string(),
        execution_surface
            .get("supports_cron")
            .cloned()
            .unwrap_or(Value::Bool(false)),
    );
    payload.insert(
        "supports_ci".to_string(),
        execution_surface
            .get("supports_ci")
            .cloned()
            .unwrap_or(Value::Bool(false)),
    );
    for field in [
        "context_files",
        "settings_paths",
        "mcp_config_paths",
        "config_root_env_var",
        "settings_scope_order",
        "subagent_paths",
        "hook_event_names",
        "hook_control_settings",
        "hook_inspection_commands",
        "hook_environment_markers",
        "checkpointing_supported",
    ] {
        payload.insert(
            field.to_string(),
            host_projection
                .get(field)
                .cloned()
                .unwrap_or_else(|| match field {
                    "config_root_env_var" => Value::Null,
                    "checkpointing_supported" => Value::Bool(false),
                    _ => Value::Array(vec![]),
                }),
        );
    }
    payload.insert(
        "compatibility_passes".to_string(),
        Value::Bool(missing_host_capabilities.is_empty()),
    );
    Ok(payload)
}

fn resolve_host_capability_requirements(
    profile: &FrameworkProfileContract,
    host_id: &str,
    adapter_id: &str,
) -> Map<String, Value> {
    let mut merged = Map::new();
    for key in ["default", host_id, adapter_id] {
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

fn build_cli_family_snapshot_entry(adapter: &Map<String, Value>) -> Result<Value, String> {
    let metadata = adapter
        .get("metadata")
        .and_then(Value::as_object)
        .ok_or_else(|| "cli adapter missing metadata".to_string())?;
    let execution_surface = adapter
        .get("execution_surface")
        .and_then(Value::as_object)
        .ok_or_else(|| "cli adapter missing execution_surface".to_string())?;
    let host_projection = adapter
        .get("host_projection")
        .and_then(Value::as_object)
        .ok_or_else(|| "cli adapter missing host_projection".to_string())?;

    Ok(value_object([
        (
            "adapter_id",
            metadata
                .get("adapter_id")
                .cloned()
                .unwrap_or_else(|| Value::String(String::new())),
        ),
        (
            "host_id",
            metadata
                .get("host_id")
                .cloned()
                .unwrap_or_else(|| Value::String(String::new())),
        ),
        (
            "entrypoint_kind",
            execution_surface
                .get("entrypoint_kind")
                .cloned()
                .unwrap_or_else(|| Value::String(String::new())),
        ),
        (
            "shared_adapter",
            execution_surface
                .get("shared_adapter")
                .cloned()
                .unwrap_or_else(|| Value::String(String::new())),
        ),
        (
            "context_files",
            host_projection
                .get("context_files")
                .cloned()
                .unwrap_or_else(|| Value::Array(vec![])),
        ),
        (
            "config_root_env_var",
            host_projection
                .get("config_root_env_var")
                .cloned()
                .unwrap_or(Value::Null),
        ),
        (
            "settings_paths",
            host_projection
                .get("settings_paths")
                .cloned()
                .unwrap_or_else(|| Value::Array(vec![])),
        ),
        (
            "mcp_config_paths",
            host_projection
                .get("mcp_config_paths")
                .cloned()
                .unwrap_or_else(|| Value::Array(vec![])),
        ),
        (
            "settings_scope_order",
            host_projection
                .get("settings_scope_order")
                .cloned()
                .unwrap_or_else(|| Value::Array(vec![])),
        ),
        (
            "subagent_paths",
            host_projection
                .get("subagent_paths")
                .cloned()
                .unwrap_or_else(|| Value::Array(vec![])),
        ),
        (
            "hook_event_names",
            host_projection
                .get("hook_event_names")
                .cloned()
                .unwrap_or_else(|| Value::Array(vec![])),
        ),
        (
            "hook_control_settings",
            host_projection
                .get("hook_control_settings")
                .cloned()
                .unwrap_or_else(|| Value::Array(vec![])),
        ),
        (
            "hook_inspection_commands",
            host_projection
                .get("hook_inspection_commands")
                .cloned()
                .unwrap_or_else(|| Value::Array(vec![])),
        ),
        (
            "plugin_hook_manifest_paths",
            host_projection
                .get("plugin_hook_manifest_paths")
                .cloned()
                .unwrap_or_else(|| Value::Array(vec![])),
        ),
        (
            "hook_environment_markers",
            host_projection
                .get("hook_environment_markers")
                .cloned()
                .unwrap_or_else(|| Value::Array(vec![])),
        ),
        (
            "checkpointing_supported",
            host_projection
                .get("checkpointing_supported")
                .cloned()
                .unwrap_or(Value::Bool(false)),
        ),
    ]))
}

fn build_codex_desktop_alias_retirement_status() -> Map<String, Value> {
    let inventory_summary = build_codex_desktop_alias_inventory_summary();
    let inventory_complete = inventory_summary
        .get("inventory_complete")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let primary_identity_risk_occurrences = inventory_summary
        .get("primary_identity_risk_occurrences")
        .and_then(Value::as_u64);
    let translation_shim_required = inventory_summary
        .get("translation_shim_required")
        .and_then(Value::as_bool);
    let runtime_primary_identity_consumers_cleared = if inventory_complete {
        Value::Bool(primary_identity_risk_occurrences == Some(0))
    } else {
        Value::Null
    };
    let mut retirement_gates = Map::new();
    retirement_gates.insert(
        "canonical_desktop_identity_locked".to_string(),
        Value::Bool(true),
    );
    retirement_gates.insert(
        "parity_snapshot_is_primary_baseline".to_string(),
        Value::Bool(true),
    );
    retirement_gates.insert(
        "compatibility_matrix_is_secondary_inventory".to_string(),
        Value::Bool(true),
    );
    retirement_gates.insert(
        "runtime_primary_identity_consumers_cleared".to_string(),
        runtime_primary_identity_consumers_cleared,
    );
    retirement_gates.insert(
        "translation_shim_required".to_string(),
        translation_shim_required
            .map(Value::Bool)
            .unwrap_or(Value::Null),
    );
    retirement_gates.insert(
        "translation_shim_ready_if_needed".to_string(),
        Value::Bool(!translation_shim_required.unwrap_or(false)),
    );

    let mut emitter_contract = Map::new();
    emitter_contract.insert(
        "python_emits_alias_artifact".to_string(),
        Value::Bool(false),
    );
    emitter_contract.insert("rust_emits_alias_artifact".to_string(), Value::Bool(false));
    emitter_contract.insert(
        "drop_requires_joint_emitter_flip".to_string(),
        Value::Bool(true),
    );
    emitter_contract.insert(
        "legacy_alias_artifact_opt_in".to_string(),
        Value::Bool(true),
    );
    emitter_contract.insert(
        "alias_may_not_gain_new_host_semantics".to_string(),
        Value::Bool(true),
    );

    let mut payload = Map::new();
    payload.insert(
        "canonical_adapter_id".to_string(),
        Value::String(CODEX_DESKTOP_ADAPTER_ID.to_string()),
    );
    payload.insert(
        "legacy_alias_id".to_string(),
        Value::String(LEGACY_CODEX_DESKTOP_ADAPTER_ID.to_string()),
    );
    payload.insert(
        "alias_lifecycle".to_string(),
        Value::String("compatibility-only".to_string()),
    );
    payload.insert(
        "alias_mode".to_string(),
        Value::String("mirror-only".to_string()),
    );
    payload.insert(
        "framework_truth".to_string(),
        Value::String("framework_core".to_string()),
    );
    payload.insert(
        "primary_regression_artifact".to_string(),
        Value::String(CLI_FAMILY_PARITY_ARTIFACT_ID.to_string()),
    );
    payload.insert(
        "codex_dual_entry_compatibility_artifact".to_string(),
        Value::String("codex_dual_entry_parity_snapshot".to_string()),
    );
    payload.insert(
        "secondary_inventory_artifact".to_string(),
        Value::String("upgrade_compatibility_matrix".to_string()),
    );
    payload.insert(
        "emitter_contract".to_string(),
        Value::Object(emitter_contract),
    );
    payload.insert(
        "retirement_gates".to_string(),
        Value::Object(retirement_gates),
    );
    payload.insert(
        "inventory_summary".to_string(),
        Value::Object(inventory_summary),
    );
    payload
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
        "host_adapters_remain_thin_projections".to_string(),
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
        Value::String("delegation".to_string()),
    );
    gate.insert("decision_before_spawn".to_string(), Value::Bool(true));
    gate.insert("spawn_is_optional".to_string(), Value::Bool(true));

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
        Value::String("delegation_contract_v1".to_string()),
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
        "delegation_state_fields".to_string(),
        json!([
            "delegation_plan_created",
            "spawn_attempted",
            "spawn_block_reason",
            "fallback_mode",
            "delegated_sidecars"
        ]),
    );
    payload.insert(
        "sidecar_contract".to_string(),
        Value::Object(sidecar_contract),
    );
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
            "running",
            "completed_unintegrated",
            "integrated",
            "failed",
            "stalled"
        ]),
    );
    schema_expectations.insert(
        "verification_fields".to_string(),
        json!(["verification_status", "last_verification_summary"]),
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
        Value::String("supervisor_state_contract_v2".to_string()),
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

fn build_execution_kernel_live_fallback_retirement_status() -> Map<String, Value> {
    let mut live_primary = Map::new();
    live_primary.insert(
        "contract_mode".to_string(),
        Value::String("rust-live-primary".to_string()),
    );
    live_primary.insert(
        "adapter_kind".to_string(),
        Value::String("router-rs".to_string()),
    );
    live_primary.insert(
        "authority".to_string(),
        Value::String("rust-execution-cli".to_string()),
    );
    live_primary.insert("family".to_string(), Value::String("rust-cli".to_string()));
    live_primary.insert("impl".to_string(), Value::String("router-rs".to_string()));

    let mut compatibility_fallback = Map::new();
    compatibility_fallback.insert("runtime_path_available".to_string(), Value::Bool(false));
    compatibility_fallback.insert(
        "retired_mode".to_string(),
        Value::String("retired".to_string()),
    );
    compatibility_fallback.insert(
        "request_behavior".to_string(),
        Value::String("surface-removed".to_string()),
    );
    compatibility_fallback.insert(
        "former_adapter_kind".to_string(),
        Value::String("python-agno".to_string()),
    );
    compatibility_fallback.insert(
        "former_authority".to_string(),
        Value::String("python-agno-kernel-adapter".to_string()),
    );
    compatibility_fallback.insert(
        "former_family".to_string(),
        Value::String("python".to_string()),
    );
    compatibility_fallback.insert("former_impl".to_string(), Value::String("agno".to_string()));
    compatibility_fallback.insert(
        "purpose_before_retirement".to_string(),
        Value::String("compatibility-only-escape-hatch".to_string()),
    );

    let mut control_surfaces = Map::new();
    control_surfaces.insert(
        "former_settings_field".to_string(),
        Value::String("rust_execute_fallback_to_python".to_string()),
    );
    control_surfaces.insert(
        "former_env_var".to_string(),
        Value::String("CODEX_AGNO_RUST_EXECUTE_FALLBACK_TO_PYTHON".to_string()),
    );
    control_surfaces.insert(
        "enabled_by_default_before_removal".to_string(),
        Value::Bool(false),
    );
    control_surfaces.insert("accepted_after_retirement".to_string(), Value::Bool(false));
    control_surfaces.insert(
        "request_behavior".to_string(),
        Value::String("surface-removed".to_string()),
    );
    control_surfaces.insert(
        "steady_state_mode".to_string(),
        Value::String("removed".to_string()),
    );
    control_surfaces.insert(
        "surface_role".to_string(),
        Value::String("removed-retired-request-surface".to_string()),
    );
    control_surfaces.insert(
        "removal_status".to_string(),
        Value::String("completed".to_string()),
    );

    let mut retirement_exit_contract = Map::new();
    retirement_exit_contract.insert(
        "surface_status".to_string(),
        Value::String("removed".to_string()),
    );
    retirement_exit_contract.insert(
        "current_decision".to_string(),
        Value::String("completed".to_string()),
    );
    retirement_exit_contract.insert(
        "removal_owner".to_string(),
        Value::String("runtime-integrator".to_string()),
    );
    retirement_exit_contract.insert("remove_when".to_string(), Value::Array(vec![]));
    let mut retirement_exit_observation_sources = Map::new();
    retirement_exit_observation_sources.insert(
        "local_runtime_health".to_string(),
        Value::Array(vec![
            Value::String("runtime_control_plane.services.execution.kernel_contract".to_string()),
            Value::String(
                "ExecutionEnvironmentService.health().kernel_live_backend_impl".to_string(),
            ),
        ]),
    );
    retirement_exit_observation_sources.insert(
        "local_contract_artifacts".to_string(),
        Value::Array(vec![
            Value::String(
                "execution_kernel_live_fallback_retirement_status.control_surfaces"
                    .to_string(),
            ),
            Value::String(
                "execution_kernel_live_fallback_retirement_status.current_contract_truth.live_fallback_request_behavior".to_string(),
            ),
        ]),
    );
    retirement_exit_observation_sources.insert(
        "external_confirmation".to_string(),
        Value::Array(vec![Value::String(
            "host or integration owner evidence that no downstream caller still probes the retired request surface".to_string(),
        )]),
    );
    retirement_exit_contract.insert(
        "observation_sources".to_string(),
        Value::Object(retirement_exit_observation_sources),
    );
    retirement_exit_contract.insert(
        "stop_rule".to_string(),
        Value::String(
            "request surface already removed from runtime settings and steady-state artifacts"
                .to_string(),
        ),
    );

    let public_runtime_contract_fields = Value::Array(vec![
        Value::String("execution_kernel".to_string()),
        Value::String("execution_kernel_authority".to_string()),
        Value::String("execution_kernel_contract_mode".to_string()),
        Value::String("execution_kernel_in_process_replacement_complete".to_string()),
        Value::String("execution_kernel_delegate".to_string()),
        Value::String("execution_kernel_delegate_authority".to_string()),
        Value::String("execution_kernel_live_primary".to_string()),
        Value::String("execution_kernel_live_primary_authority".to_string()),
    ]);
    let public_runtime_response_metadata_fields = Value::Array(vec![
        Value::String("execution_kernel_delegate_family".to_string()),
        Value::String("execution_kernel_delegate_impl".to_string()),
    ]);
    let retired_runtime_response_metadata_fields = Value::Array(vec![Value::String(
        "execution_kernel_fallback_reason".to_string(),
    )]);

    let mut current_contract_truth = Map::new();
    current_contract_truth.insert(
        "execution_kernel_contract_mode".to_string(),
        Value::String("rust-live-primary".to_string()),
    );
    current_contract_truth.insert(
        "execution_kernel_in_process_replacement_complete".to_string(),
        Value::Bool(true),
    );
    current_contract_truth.insert(
        "dry_run_delegate_kind".to_string(),
        Value::String("router-rs".to_string()),
    );
    current_contract_truth.insert(
        "dry_run_delegate_authority".to_string(),
        Value::String("rust-execution-cli".to_string()),
    );
    current_contract_truth.insert(
        "live_primary_kind".to_string(),
        Value::String("router-rs".to_string()),
    );
    current_contract_truth.insert(
        "live_primary_authority".to_string(),
        Value::String("rust-execution-cli".to_string()),
    );
    current_contract_truth.insert(
        "live_fallback_runtime_path_available".to_string(),
        Value::Bool(false),
    );
    current_contract_truth.insert(
        "live_fallback_mode".to_string(),
        Value::String("retired".to_string()),
    );
    current_contract_truth.insert(
        "live_fallback_request_behavior".to_string(),
        Value::String("surface-removed".to_string()),
    );
    current_contract_truth.insert(
        "live_fallback_request_surface".to_string(),
        Value::String("removed".to_string()),
    );
    current_contract_truth.insert(
        "live_prompt_preview_passthrough_disabled".to_string(),
        Value::Bool(true),
    );
    current_contract_truth.insert(
        "compatibility_fallback_reason_metadata_key".to_string(),
        Value::String("execution_kernel_fallback_reason".to_string()),
    );

    let mut current_response_metadata_truth = Map::new();
    current_response_metadata_truth.insert(
        "live_delegate_family".to_string(),
        Value::String("rust-cli".to_string()),
    );
    current_response_metadata_truth.insert(
        "live_delegate_impl".to_string(),
        Value::String("router-rs".to_string()),
    );
    current_response_metadata_truth.insert(
        "dry_run_delegate_family".to_string(),
        Value::String("rust-cli".to_string()),
    );
    current_response_metadata_truth.insert(
        "dry_run_delegate_impl".to_string(),
        Value::String("router-rs".to_string()),
    );
    current_response_metadata_truth.insert(
        "compatibility_fallback_reason_present_in_steady_state".to_string(),
        Value::Bool(false),
    );
    current_response_metadata_truth.insert(
        "retired_response_metadata_fields".to_string(),
        retired_runtime_response_metadata_fields.clone(),
    );

    let remaining_python_owned_surfaces = Value::Array(vec![]);

    let mut retirement_readiness = Map::new();
    retirement_readiness.insert("ready".to_string(), Value::Bool(true));
    retirement_readiness.insert("status".to_string(), Value::String("retired".to_string()));
    retirement_readiness.insert("contract_lane_complete".to_string(), Value::Bool(true));
    retirement_readiness.insert(
        "runtime_control_flow_change_required".to_string(),
        Value::Bool(false),
    );
    retirement_readiness.insert("blockers".to_string(), Value::Array(vec![]));
    retirement_readiness.insert(
        "next_safe_slice".to_string(),
        Value::String("rustification_closed".to_string()),
    );

    let mut guardrails = Map::new();
    guardrails.insert(
        "thin_projection_boundary_preserved".to_string(),
        Value::Bool(true),
    );
    guardrails.insert(
        "cli_hosts_may_not_become_framework_truth".to_string(),
        Value::Bool(true),
    );
    guardrails.insert(
        "claude_host_runtime_semantics_remain_host_owned".to_string(),
        Value::Bool(true),
    );

    let mut retirement_gates = Map::new();
    retirement_gates.insert(
        "public_runtime_contract_externalized".to_string(),
        Value::Bool(true),
    );
    retirement_gates.insert(
        "live_primary_contract_externalized".to_string(),
        Value::Bool(true),
    );
    retirement_gates.insert(
        "compatibility_fallback_contract_externalized".to_string(),
        Value::Bool(true),
    );
    retirement_gates.insert(
        "rust_only_disabled_mode_externalized".to_string(),
        Value::Bool(true),
    );
    retirement_gates.insert(
        "response_metadata_surface_externalized".to_string(),
        Value::Bool(true),
    );
    retirement_gates.insert(
        "delegate_family_impl_metadata_externalized".to_string(),
        Value::Bool(true),
    );
    retirement_gates.insert(
        "dry_run_delegate_still_python_owned".to_string(),
        Value::Bool(false),
    );
    retirement_gates.insert(
        "compatibility_fallback_runtime_path_removed".to_string(),
        Value::Bool(true),
    );
    retirement_gates.insert(
        "explicit_compatibility_requests_rejected".to_string(),
        Value::Bool(true),
    );
    retirement_gates.insert(
        "dry_run_prompt_preview_still_python_owned".to_string(),
        Value::Bool(false),
    );
    retirement_gates.insert(
        "compatibility_fallback_agent_factory_still_python_owned".to_string(),
        Value::Bool(false),
    );
    retirement_gates.insert(
        "compatibility_live_response_serialization_still_python_owned".to_string(),
        Value::Bool(false),
    );
    retirement_gates.insert(
        "compatibility_fallback_reason_metadata_still_python_owned".to_string(),
        Value::Bool(false),
    );
    retirement_gates.insert(
        "default_runtime_python_fallback_retired".to_string(),
        Value::Bool(true),
    );
    retirement_gates.insert(
        "in_process_replacement_complete".to_string(),
        Value::Bool(true),
    );

    let mut payload = Map::new();
    payload.insert(
        "framework_truth".to_string(),
        Value::String("framework_core".to_string()),
    );
    payload.insert(
        "status_contract".to_string(),
        Value::String("execution_kernel_live_fallback_retirement_status_v1".to_string()),
    );
    payload.insert(
        "affected_host_projections".to_string(),
        Value::Array(vec![
            Value::String(CODEX_DESKTOP_ADAPTER_ID.to_string()),
            Value::String(CODEX_CLI_ADAPTER_ID.to_string()),
            Value::String(CLAUDE_CODE_ADAPTER_ID.to_string()),
            Value::String(GEMINI_CLI_ADAPTER_ID.to_string()),
        ]),
    );
    payload.insert("live_primary".to_string(), Value::Object(live_primary));
    payload.insert(
        "compatibility_fallback".to_string(),
        Value::Object(compatibility_fallback),
    );
    payload.insert(
        "control_surfaces".to_string(),
        Value::Object(control_surfaces),
    );
    payload.insert(
        "retirement_exit_contract".to_string(),
        Value::Object(retirement_exit_contract),
    );
    payload.insert(
        "public_runtime_contract_fields".to_string(),
        public_runtime_contract_fields,
    );
    payload.insert(
        "public_runtime_response_metadata_fields".to_string(),
        public_runtime_response_metadata_fields,
    );
    payload.insert(
        "retired_runtime_response_metadata_fields".to_string(),
        retired_runtime_response_metadata_fields,
    );
    payload.insert(
        "current_contract_truth".to_string(),
        Value::Object(current_contract_truth),
    );
    payload.insert(
        "current_response_metadata_truth".to_string(),
        Value::Object(current_response_metadata_truth),
    );
    payload.insert(
        "remaining_python_owned_surfaces".to_string(),
        remaining_python_owned_surfaces,
    );
    payload.insert(
        "retirement_readiness".to_string(),
        Value::Object(retirement_readiness),
    );
    payload.insert("guardrails".to_string(), Value::Object(guardrails));
    payload.insert(
        "retirement_gates".to_string(),
        Value::Object(retirement_gates),
    );
    payload
}

fn build_execution_kernel_live_response_serialization_contract() -> Map<String, Value> {
    let steady_state_kernel_fields = vec![
        Value::String("execution_kernel_metadata_schema_version".to_string()),
        Value::String("execution_kernel".to_string()),
        Value::String("execution_kernel_authority".to_string()),
        Value::String("execution_kernel_contract_mode".to_string()),
        Value::String("execution_kernel_fallback_policy".to_string()),
        Value::String("execution_kernel_in_process_replacement_complete".to_string()),
        Value::String("execution_kernel_delegate".to_string()),
        Value::String("execution_kernel_delegate_authority".to_string()),
        Value::String("execution_kernel_delegate_family".to_string()),
        Value::String("execution_kernel_delegate_impl".to_string()),
        Value::String("execution_kernel_live_primary".to_string()),
        Value::String("execution_kernel_live_primary_authority".to_string()),
        Value::String("execution_kernel_live_fallback".to_string()),
        Value::String("execution_kernel_live_fallback_authority".to_string()),
        Value::String("execution_kernel_live_fallback_enabled".to_string()),
        Value::String("execution_kernel_live_fallback_mode".to_string()),
        Value::String("execution_kernel_response_shape".to_string()),
        Value::String("execution_kernel_prompt_preview_owner".to_string()),
    ];
    let mut live_primary_required_metadata_fields = steady_state_kernel_fields.clone();
    live_primary_required_metadata_fields.extend(vec![
        Value::String("run_id".to_string()),
        Value::String("status".to_string()),
        Value::String("execution_kernel_model_id_source".to_string()),
        Value::String("trace_event_count".to_string()),
        Value::String("trace_output_path".to_string()),
    ]);
    let mut dry_run_required_metadata_fields = steady_state_kernel_fields.clone();
    dry_run_required_metadata_fields.extend(vec![
        Value::String("reason".to_string()),
        Value::String("execution_kernel_contract_mode".to_string()),
        Value::String("execution_kernel_fallback_policy".to_string()),
        Value::String("trace_event_count".to_string()),
        Value::String("trace_output_path".to_string()),
    ]);
    let public_response_fields = Value::Array(vec![
        Value::String("session_id".to_string()),
        Value::String("user_id".to_string()),
        Value::String("skill".to_string()),
        Value::String("overlay".to_string()),
        Value::String("live_run".to_string()),
        Value::String("content".to_string()),
        Value::String("usage".to_string()),
        Value::String("prompt_preview".to_string()),
        Value::String("model_id".to_string()),
        Value::String("metadata".to_string()),
    ]);
    let usage_fields = Value::Array(vec![
        Value::String("input_tokens".to_string()),
        Value::String("output_tokens".to_string()),
        Value::String("total_tokens".to_string()),
        Value::String("mode".to_string()),
    ]);

    let mut usage_contract = Map::new();
    usage_contract.insert("fields".to_string(), usage_fields);
    usage_contract.insert("live_mode".to_string(), Value::String("live".to_string()));
    usage_contract.insert(
        "dry_run_mode".to_string(),
        Value::String("estimated".to_string()),
    );

    let mut runtime_response_metadata_fields = Map::new();
    runtime_response_metadata_fields.insert(
        "shared".to_string(),
        Value::Array(vec![
            Value::String("trace_event_count".to_string()),
            Value::String("trace_output_path".to_string()),
        ]),
    );
    runtime_response_metadata_fields.insert(
        "steady_state_kernel".to_string(),
        Value::Array(steady_state_kernel_fields.clone()),
    );
    runtime_response_metadata_fields.insert(
        "live_primary".to_string(),
        Value::Array(vec![
            Value::String("run_id".to_string()),
            Value::String("status".to_string()),
            Value::String("execution_mode".to_string()),
            Value::String("route_engine".to_string()),
            Value::String("diagnostic_route_mode".to_string()),
            Value::String("execution_kernel_model_id_source".to_string()),
        ]),
    );
    runtime_response_metadata_fields.insert(
        "retired_compatibility_fallback".to_string(),
        Value::Array(vec![
            Value::String("run_id".to_string()),
            Value::String("status".to_string()),
            Value::String("execution_kernel_contract_mode".to_string()),
            Value::String("execution_kernel_fallback_policy".to_string()),
            Value::String("execution_kernel_primary".to_string()),
            Value::String("execution_kernel_primary_authority".to_string()),
            Value::String("execution_kernel_fallback_reason".to_string()),
            Value::String("execution_kernel_compatibility_agent_contract".to_string()),
            Value::String("execution_kernel_compatibility_agent_kind".to_string()),
            Value::String("execution_kernel_compatibility_agent_authority".to_string()),
        ]),
    );
    runtime_response_metadata_fields.insert(
        "dry_run".to_string(),
        Value::Array(vec![
            Value::String("reason".to_string()),
            Value::String("execution_kernel_contract_mode".to_string()),
            Value::String("execution_kernel_fallback_policy".to_string()),
        ]),
    );

    let mut current_contract_truth = Map::new();
    current_contract_truth.insert(
        "public_response_model".to_string(),
        Value::String("RunTaskResponse".to_string()),
    );
    current_contract_truth.insert(
        "execution_request_schema_version".to_string(),
        Value::String("router-rs-execute-request-v1".to_string()),
    );
    current_contract_truth.insert(
        "live_primary_schema_version".to_string(),
        Value::String("router-rs-execute-response-v1".to_string()),
    );
    current_contract_truth.insert(
        "steady_state_metadata_schema_version".to_string(),
        Value::String("router-rs-execution-kernel-metadata-v1".to_string()),
    );
    current_contract_truth.insert(
        "live_primary_prompt_preview_owner".to_string(),
        Value::String("rust-execution-cli".to_string()),
    );
    current_contract_truth.insert(
        "steady_state_response_shapes".to_string(),
        json!(["live_primary", "dry_run"]),
    );
    current_contract_truth.insert(
        "retired_compatibility_fallback_prompt_preview_owner".to_string(),
        Value::String("python-agno-kernel-adapter".to_string()),
    );
    current_contract_truth.insert(
        "dry_run_prompt_preview_owner".to_string(),
        Value::String("rust-execution-cli".to_string()),
    );
    current_contract_truth.insert(
        "live_primary_model_id_source".to_string(),
        Value::String("aggregator-response.model".to_string()),
    );
    current_contract_truth.insert(
        "retired_compatibility_fallback_model_id_source".to_string(),
        Value::String("agno-run-output.model".to_string()),
    );
    current_contract_truth.insert(
        "compatibility_fallback_runtime_path".to_string(),
        Value::String("retired".to_string()),
    );
    current_contract_truth.insert(
        "compatibility_fallback_request_behavior".to_string(),
        Value::String("surface-removed".to_string()),
    );
    current_contract_truth.insert(
        "retired_compatibility_fallback_policy".to_string(),
        Value::String("infrastructure-only-explicit".to_string()),
    );
    current_contract_truth.insert(
        "retired_compatibility_agent_contract_version".to_string(),
        Value::String("execution-kernel-compatibility-agent-v1".to_string()),
    );
    current_contract_truth.insert(
        "compatibility_fallback_reason_metadata_key".to_string(),
        Value::String("execution_kernel_fallback_reason".to_string()),
    );

    let mut live_primary = Map::new();
    live_primary.insert("live_run".to_string(), Value::Bool(true));
    live_primary.insert("usage_mode".to_string(), Value::String("live".to_string()));
    live_primary.insert(
        "content_type".to_string(),
        Value::String("string".to_string()),
    );
    live_primary.insert(
        "prompt_preview_source".to_string(),
        Value::String("rust-owned-live-prompt".to_string()),
    );
    live_primary.insert("model_id_present".to_string(), Value::Bool(true));
    live_primary.insert(
        "required_metadata_fields".to_string(),
        Value::Array(live_primary_required_metadata_fields),
    );
    live_primary.insert(
        "steady_state_metadata_fields".to_string(),
        Value::Array(steady_state_kernel_fields.clone()),
    );
    live_primary.insert(
        "pass_through_metadata_fields".to_string(),
        Value::Array(vec![
            Value::String("execution_mode".to_string()),
            Value::String("route_engine".to_string()),
            Value::String("diagnostic_route_mode".to_string()),
        ]),
    );

    let mut compatibility_fallback = Map::new();
    compatibility_fallback.insert("runtime_path_available".to_string(), Value::Bool(false));
    compatibility_fallback.insert(
        "request_behavior".to_string(),
        Value::String("surface-removed".to_string()),
    );
    compatibility_fallback.insert("legacy_live_run".to_string(), Value::Bool(true));
    compatibility_fallback.insert(
        "legacy_usage_mode".to_string(),
        Value::String("live".to_string()),
    );
    compatibility_fallback.insert(
        "legacy_content_type".to_string(),
        Value::String("string".to_string()),
    );
    compatibility_fallback.insert(
        "legacy_prompt_preview_source".to_string(),
        Value::String("python-prompt-builder".to_string()),
    );
    compatibility_fallback.insert("legacy_model_id_present".to_string(), Value::Bool(true));
    compatibility_fallback.insert(
        "legacy_required_metadata_fields".to_string(),
        Value::Array(vec![
            Value::String("run_id".to_string()),
            Value::String("status".to_string()),
            Value::String("trace_event_count".to_string()),
            Value::String("trace_output_path".to_string()),
            Value::String("execution_kernel_contract_mode".to_string()),
            Value::String("execution_kernel_fallback_policy".to_string()),
            Value::String("execution_kernel_primary".to_string()),
            Value::String("execution_kernel_primary_authority".to_string()),
            Value::String("execution_kernel_fallback_reason".to_string()),
            Value::String("execution_kernel_compatibility_agent_contract".to_string()),
            Value::String("execution_kernel_compatibility_agent_kind".to_string()),
            Value::String("execution_kernel_compatibility_agent_authority".to_string()),
        ]),
    );
    compatibility_fallback.insert(
        "legacy_fallback_reason_present".to_string(),
        Value::Bool(true),
    );

    let mut dry_run = Map::new();
    dry_run.insert("live_run".to_string(), Value::Bool(false));
    dry_run.insert(
        "usage_mode".to_string(),
        Value::String("estimated".to_string()),
    );
    dry_run.insert(
        "content_type".to_string(),
        Value::String("string".to_string()),
    );
    dry_run.insert(
        "prompt_preview_source".to_string(),
        Value::String("rust-owned-dry-run-prompt".to_string()),
    );
    dry_run.insert("model_id_present".to_string(), Value::Bool(false));
    dry_run.insert(
        "required_metadata_fields".to_string(),
        Value::Array(dry_run_required_metadata_fields),
    );
    dry_run.insert(
        "steady_state_metadata_fields".to_string(),
        Value::Array(steady_state_kernel_fields),
    );
    dry_run.insert("fallback_reason_present".to_string(), Value::Bool(false));

    let mut current_response_shape_truth = Map::new();
    current_response_shape_truth.insert("live_primary".to_string(), Value::Object(live_primary));
    current_response_shape_truth.insert(
        "retired_compatibility_fallback".to_string(),
        Value::Object(compatibility_fallback),
    );
    current_response_shape_truth.insert("dry_run".to_string(), Value::Object(dry_run));

    let mut retirement_gates = Map::new();
    retirement_gates.insert(
        "response_shape_contract_externalized".to_string(),
        Value::Bool(true),
    );
    retirement_gates.insert(
        "live_primary_response_contract_externalized".to_string(),
        Value::Bool(true),
    );
    retirement_gates.insert(
        "compatibility_fallback_response_contract_externalized".to_string(),
        Value::Bool(true),
    );
    retirement_gates.insert(
        "compatibility_fallback_runtime_path_removed".to_string(),
        Value::Bool(true),
    );
    retirement_gates.insert(
        "explicit_compatibility_requests_rejected".to_string(),
        Value::Bool(true),
    );
    retirement_gates.insert(
        "compatibility_live_response_serialization_still_python_owned".to_string(),
        Value::Bool(false),
    );
    retirement_gates.insert(
        "runtime_control_flow_change_required_for_removal".to_string(),
        Value::Bool(false),
    );

    let mut guardrails = Map::new();
    guardrails.insert(
        "thin_projection_boundary_preserved".to_string(),
        Value::Bool(true),
    );
    guardrails.insert(
        "cli_hosts_may_not_become_framework_truth".to_string(),
        Value::Bool(true),
    );
    guardrails.insert(
        "claude_host_runtime_semantics_remain_host_owned".to_string(),
        Value::Bool(true),
    );

    let mut payload = Map::new();
    payload.insert(
        "framework_truth".to_string(),
        Value::String("framework_core".to_string()),
    );
    payload.insert(
        "status_contract".to_string(),
        Value::String("execution_kernel_live_response_serialization_contract_v1".to_string()),
    );
    payload.insert(
        "scope".to_string(),
        Value::String("compatibility_live_response_serialization".to_string()),
    );
    payload.insert(
        "artifact_role".to_string(),
        Value::String("shared-contract-evidence".to_string()),
    );
    payload.insert(
        "affected_host_projections".to_string(),
        Value::Array(vec![
            Value::String(CODEX_DESKTOP_ADAPTER_ID.to_string()),
            Value::String(CODEX_CLI_ADAPTER_ID.to_string()),
            Value::String(CLAUDE_CODE_ADAPTER_ID.to_string()),
            Value::String(GEMINI_CLI_ADAPTER_ID.to_string()),
        ]),
    );
    payload.insert("public_response_fields".to_string(), public_response_fields);
    payload.insert("usage_contract".to_string(), Value::Object(usage_contract));
    payload.insert(
        "runtime_response_metadata_fields".to_string(),
        Value::Object(runtime_response_metadata_fields),
    );
    payload.insert(
        "current_contract_truth".to_string(),
        Value::Object(current_contract_truth),
    );
    payload.insert(
        "current_response_shape_truth".to_string(),
        Value::Object(current_response_shape_truth),
    );
    payload.insert(
        "retirement_gates".to_string(),
        Value::Object(retirement_gates),
    );
    payload.insert("guardrails".to_string(), Value::Object(guardrails));
    payload
}

fn build_control_plane_contract_descriptors() -> Map<String, Value> {
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
    payload.insert(
        EXECUTION_KERNEL_LIVE_FALLBACK_RETIREMENT_ARTIFACT_ID.to_string(),
        Value::Object(build_execution_kernel_live_fallback_retirement_status()),
    );
    payload.insert(
        EXECUTION_KERNEL_LIVE_RESPONSE_SERIALIZATION_ARTIFACT_ID.to_string(),
        Value::Object(build_execution_kernel_live_response_serialization_contract()),
    );
    payload
}

fn build_codex_desktop_alias_inventory_summary() -> Map<String, Value> {
    let scan_root = repo_scan_root();
    let search_roots = [
        scan_root.join("codex_agno_runtime").join("src"),
        scan_root.join("scripts"),
        scan_root.join("tests"),
        scan_root.join("docs"),
        scan_root.join("aionrs_fusion_docs"),
    ];

    let mut category_counts = Map::new();
    let mut total_occurrences = 0_u64;
    let mut primary_identity_risk_occurrences = 0_u64;
    let mut compatibility_only_occurrences = 0_u64;

    for root in search_roots {
        if !root.exists() {
            continue;
        }
        for path in collect_files(&root) {
            if path.extension().and_then(|ext| ext.to_str()) == Some("pyc") {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            for line in text.lines() {
                if !line.contains(LEGACY_CODEX_DESKTOP_ADAPTER_ID) {
                    continue;
                }
                total_occurrences += 1;
                let (category, risk) = classify_alias_reference(&path);
                increment_counter(&mut category_counts, category);
                match risk {
                    "compatibility_only" => compatibility_only_occurrences += 1,
                    _ => primary_identity_risk_occurrences += 1,
                }
            }
        }
    }

    let translation_shim_required = primary_identity_risk_occurrences > 0;
    let mut summary = Map::new();
    summary.insert("inventory_complete".to_string(), Value::Bool(true));
    summary.insert(
        "legacy_alias_id".to_string(),
        Value::String(LEGACY_CODEX_DESKTOP_ADAPTER_ID.to_string()),
    );
    summary.insert(
        "total_occurrences".to_string(),
        Value::from(total_occurrences),
    );
    summary.insert(
        "category_counts".to_string(),
        Value::Object(category_counts),
    );
    summary.insert(
        "primary_identity_risk_occurrences".to_string(),
        Value::from(primary_identity_risk_occurrences),
    );
    summary.insert(
        "compatibility_only_occurrences".to_string(),
        Value::from(compatibility_only_occurrences),
    );
    summary.insert(
        "translation_shim_required".to_string(),
        Value::Bool(translation_shim_required),
    );
    summary
}

fn repo_scan_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn collect_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut entries = match fs::read_dir(root) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .collect::<Vec<_>>(),
        Err(_) => return files,
    };
    entries.sort();
    for path in entries {
        if path.is_dir() {
            let directory_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if matches!(directory_name, "target" | "__pycache__" | ".pytest_cache") {
                continue;
            }
            files.extend(collect_files(&path));
            continue;
        }
        if path.is_file() {
            files.push(path);
        }
    }
    files
}

fn classify_alias_reference(path: &Path) -> (&'static str, &'static str) {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let parts: HashSet<&str> = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect();
    if file_name == "host_adapters.py" {
        return ("compatibility_infrastructure", "compatibility_only");
    }
    if file_name == "profile_artifacts.py" {
        return ("artifact_emitter", "compatibility_only");
    }
    if file_name == "compatibility.py" {
        return ("compatibility_escape_hatch", "compatibility_only");
    }
    if file_name == "write_framework_contract_artifacts.py" {
        return ("compatibility_emitter_cli", "compatibility_only");
    }
    if file_name == "rust_router.py" {
        return ("compatibility_router_cli", "compatibility_only");
    }
    if file_name == "__init__.py" {
        return ("retired_root_export_surface", "compatibility_only");
    }
    if file_name == "framework_profile.rs" {
        return ("rust_contract_artifact_lane", "compatibility_only");
    }
    if parts.contains("tests") {
        return ("compatibility_regression_tests", "compatibility_only");
    }
    if parts.contains("docs") || parts.contains("aionrs_fusion_docs") {
        return ("compatibility_contract_docs", "compatibility_only");
    }
    ("unclassified_code", "primary_identity_risk")
}

fn increment_counter(counter: &mut Map<String, Value>, key: &str) {
    let next_value = counter.get(key).and_then(Value::as_u64).unwrap_or(0) + 1;
    counter.insert(key.to_string(), Value::from(next_value));
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
    "generic".to_string()
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
            "host_family": "generic",
            "core_capabilities": ["runtime", "memory", "artifact", "orchestration"],
            "rules_bundle": {"rules": [{"id": "outer-owned"}]},
            "skill_bundle": {"skills": ["router", "memory-bridge"]},
            "session_policy": {"mode": "bounded", "approval_mode": "manual"},
            "tool_policy": {"shell": "allow"},
            "approval_policy": {"mode": "manual"},
            "loadout_policy": {"default": "portable"},
            "framework_surface_policy": {
                "kernel": {"canonical_axes": ["routing", "memory", "continuity", "host_projection"]},
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
    fn profile_bundle_builds_companion_projection() {
        let bundle = build_profile_bundle(&sample_profile()).expect("bundle should build");
        assert_eq!(bundle.profile_id, "fusion-default");
        assert_eq!(bundle.capabilities.core.len(), 4);
        assert_eq!(bundle.companion_projection.preset_rules.len(), 1);
        assert_eq!(bundle.companion_projection.enabled_skills.len(), 2);
        assert_eq!(
            bundle.companion_projection.fallback_semantics["fallback_adapter"],
            Value::String("codex_desktop_adapter".to_string())
        );
        assert_eq!(
            bundle.cli_common_adapter["controller_boundary"]["shared_adapter"],
            Value::String("cli_common_adapter".to_string())
        );
        assert_eq!(
            bundle.codex_common_adapter["metadata"]["adapter_alias_of"],
            Value::String("cli_common_adapter".to_string())
        );
        assert_eq!(
            bundle.codex_common_adapter["parity_contract"]["cli_adapters"],
            json!([
                "codex_cli_adapter",
                "claude_code_adapter",
                "gemini_cli_adapter"
            ])
        );
        assert_eq!(
            bundle.claude_code_adapter["host_projection"]["context_files"],
            json!(["CLAUDE.md", "CLAUDE.local.md"])
        );
        assert_eq!(
            bundle.gemini_cli_adapter["host_projection"]["structured_output_modes"],
            json!(["json", "stream-json"])
        );
        assert_eq!(
            bundle.cli_common_adapter["controller_boundary"]["codexcli_is_controller"],
            Value::Bool(false)
        );
        assert_eq!(
            bundle.codex_cli_adapter["execution_surface"]["shared_adapter"],
            Value::String("cli_common_adapter".to_string())
        );
        assert_eq!(
            bundle.codex_desktop_adapter["entrypoint_contract"]["entrypoint_kind"],
            Value::String("interactive".to_string())
        );
        assert!(bundle.compatibility_lane.is_none());
        assert_eq!(
            bundle.codex_cli_adapter["execution_surface"]["entrypoint_kind"],
            Value::String("headless".to_string())
        );
        assert_eq!(
            bundle.codex_cli_adapter["execution_surface"]["controller_is_cli"],
            Value::Bool(false)
        );
        assert_eq!(
            bundle.codex_cli_adapter["common_contract"],
            bundle.codex_desktop_adapter["common_contract"]
        );
        assert_eq!(
            bundle.codex_dual_entry_parity_snapshot["desktop"]["adapter_id"],
            Value::String("codex_desktop_adapter".to_string())
        );
        assert_eq!(
            bundle.cli_family_parity_snapshot["shared_adapter"],
            Value::String("cli_common_adapter".to_string())
        );
        assert_eq!(
            bundle.cli_family_capability_discovery["discovery_contract"],
            Value::String("cli_family_host_capability_contract_v1".to_string())
        );
        assert_eq!(
            bundle.cli_family_capability_discovery["controller_boundary"]["host_entrypoints"],
            json!([
                "codex_desktop_adapter",
                "codex_cli_adapter",
                "claude_code_adapter",
                "gemini_cli_adapter"
            ])
        );
        assert_eq!(
            bundle.cli_family_capability_discovery["controller_boundary"]["cli_family_entrypoints"],
            json!([
                "codex_cli_adapter",
                "claude_code_adapter",
                "gemini_cli_adapter"
            ])
        );
        assert_eq!(
            bundle.cli_family_capability_discovery["cli_hosts"]["codex_cli_adapter"]
                ["supports_cron"],
            Value::Bool(true)
        );
        assert_eq!(
            bundle.cli_family_capability_discovery["cli_hosts"]["claude_code_adapter"]["transport"],
            Value::String("headless-exec".to_string())
        );
        assert_eq!(
            bundle.cli_family_parity_snapshot["cli_hosts"]["gemini_cli_adapter"]["context_files"],
            json!(["GEMINI.md"])
        );
        assert_eq!(
            bundle.codex_dual_entry_parity_snapshot["shared_adapter"],
            Value::String("cli_common_adapter".to_string())
        );
        assert_eq!(
            bundle.codex_dual_entry_parity_snapshot["all_shared_contract_checks_pass"],
            Value::Bool(true)
        );
        assert_eq!(
            bundle.codex_dual_entry_parity_snapshot["parity_checks"]["artifact_contract"],
            Value::Bool(true)
        );
        assert!(bundle.codex_desktop_alias_retirement_status.is_none());
        assert_eq!(
            bundle.execution_controller_contract["status_contract"],
            Value::String("execution_controller_contract_v1".to_string())
        );
        assert_eq!(
            bundle.execution_controller_contract["controller"]["primary_owner"],
            Value::String("execution-controller-coding".to_string())
        );
        assert_eq!(
            bundle.delegation_contract["gate"]["gate_skill"],
            Value::String("subagent-delegation".to_string())
        );
        assert_eq!(
            bundle.supervisor_state_contract["state_artifact_path"],
            Value::String(".supervisor_state.json".to_string())
        );
        assert_eq!(
            bundle.supervisor_state_contract["schema_expectations"]["delegation_fields"],
            json!([
                "delegation_plan_created",
                "spawn_attempted",
                "spawn_block_reason",
                "fallback_mode",
                "delegated_sidecars"
            ])
        );
        assert_eq!(
            bundle.execution_kernel_live_fallback_retirement_status["status_contract"],
            Value::String("execution_kernel_live_fallback_retirement_status_v1".to_string())
        );
        assert_eq!(
            bundle.execution_kernel_live_fallback_retirement_status["retirement_readiness"]
                ["ready"],
            Value::Bool(true)
        );
        assert_eq!(
            bundle.execution_kernel_live_fallback_retirement_status["retirement_gates"]
                ["dry_run_delegate_still_python_owned"],
            Value::Bool(false)
        );
        assert_eq!(
            bundle.execution_kernel_live_fallback_retirement_status
                ["remaining_python_owned_surfaces"],
            json!([])
        );
        assert_eq!(
            bundle.execution_kernel_live_fallback_retirement_status
                ["current_response_metadata_truth"]
                ["compatibility_fallback_reason_present_in_steady_state"],
            Value::Bool(false)
        );
        assert_eq!(
            bundle.execution_kernel_live_fallback_retirement_status
                ["retired_runtime_response_metadata_fields"],
            json!(["execution_kernel_fallback_reason"])
        );
        assert_eq!(
            bundle.execution_kernel_live_response_serialization_contract["status_contract"],
            Value::String("execution_kernel_live_response_serialization_contract_v1".to_string())
        );
        assert_eq!(
            bundle.execution_kernel_live_response_serialization_contract
                ["runtime_response_metadata_fields"]["shared"],
            json!(["trace_event_count", "trace_output_path"])
        );
        assert_eq!(
            bundle.execution_kernel_live_response_serialization_contract
                ["current_response_shape_truth"]["retired_compatibility_fallback"]
                ["runtime_path_available"],
            Value::Bool(false)
        );
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
            bundle.cli_common_adapter["shared_contract"]["workspace_bootstrap"],
            expected_bootstrap
        );
        assert_eq!(
            bundle.cli_common_adapter["bridge_contract"],
            expected_bootstrap["bridges"]
        );
        assert_eq!(
            bundle.codex_desktop_adapter["common_contract"]["workspace_bootstrap"],
            expected_bootstrap
        );
        assert_eq!(
            bundle.codex_cli_adapter["common_contract"]["workspace_bootstrap"],
            expected_bootstrap
        );
        assert_eq!(
            bundle.codex_cli_adapter["runtime_surface"]["workspace_bootstrap"],
            expected_bootstrap
        );
    }

    #[test]
    fn profile_bundle_quarantines_legacy_alias_in_compatibility_lane() {
        let bundle = build_profile_bundle_with_legacy_alias(&sample_profile(), true)
            .expect("bundle should build");
        let serialized = serde_json::to_value(&bundle).expect("bundle should serialize");
        let compatibility_lane = bundle
            .compatibility_lane
            .as_ref()
            .expect("compatibility lane should be present when opt-in is enabled");

        assert_eq!(
            compatibility_lane.codex_desktop_host_adapter["metadata"]["adapter_alias_of"],
            Value::String("codex_desktop_adapter".to_string())
        );
        assert!(serialized.get("codex_desktop_host_adapter").is_none());
        assert_eq!(
            serialized["compatibility_lane"]["codex_desktop_host_adapter"]["metadata"]
                ["canonical_adapter_id"],
            Value::String("codex_desktop_adapter".to_string())
        );
    }

    #[test]
    fn codex_dual_entry_snapshot_preserves_framework_core_truth() {
        let bundle = build_profile_bundle(&sample_profile()).expect("bundle should build");
        assert_eq!(
            bundle.codex_common_adapter["shared_contract"]["artifact_contract"],
            json!({"layout": "stable-v1"})
        );
        assert_eq!(
            bundle.cli_common_adapter["shared_contract"]["framework_surface_policy"]
                ["default_surface"]["default_loadouts"],
            json!(["default_surface_loadout"])
        );
        assert_eq!(
            bundle.codex_dual_entry_parity_snapshot["controller_boundary"]
                ["single_source_of_truth"],
            Value::Bool(true)
        );
        assert_eq!(
            bundle.codex_dual_entry_parity_snapshot["desktop"]["shared_adapter"],
            Value::String("cli_common_adapter".to_string())
        );
        assert_eq!(
            bundle.codex_dual_entry_parity_snapshot["cli"]["shared_adapter"],
            Value::String("cli_common_adapter".to_string())
        );
        assert_eq!(
            bundle.codex_dual_entry_parity_snapshot["codexcli_is_framework_controller"],
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
        assert_eq!(
            descriptors[EXECUTION_KERNEL_LIVE_FALLBACK_RETIREMENT_ARTIFACT_ID],
            Value::Object(build_execution_kernel_live_fallback_retirement_status())
        );
        assert_eq!(
            descriptors[EXECUTION_KERNEL_LIVE_RESPONSE_SERIALIZATION_ARTIFACT_ID],
            Value::Object(build_execution_kernel_live_response_serialization_contract())
        );
    }

    #[test]
    fn codex_artifact_bundle_exposes_first_class_outputs() {
        let artifacts =
            build_codex_artifact_bundle(&sample_profile(), false).expect("artifacts should build");
        assert_eq!(artifacts.len(), 14);
        assert_eq!(
            artifacts["cli_common_adapter"]["controller_boundary"]["shared_adapter"],
            Value::String("cli_common_adapter".to_string())
        );
        assert_eq!(
            artifacts["codex_common_adapter"]["metadata"]["adapter_alias_of"],
            Value::String("cli_common_adapter".to_string())
        );
        assert_eq!(
            artifacts["claude_code_adapter"]["host_projection"]["context_files"],
            json!(["CLAUDE.md", "CLAUDE.local.md"])
        );
        assert_eq!(
            artifacts["gemini_cli_adapter"]["host_projection"]["structured_output_modes"],
            json!(["json", "stream-json"])
        );
        assert_eq!(
            artifacts["cli_family_capability_discovery"]["all_cli_hosts_compatible"],
            Value::Bool(true)
        );
        assert_eq!(
            artifacts["cli_family_capability_discovery"]["controller_boundary"]["host_entrypoints"],
            json!([
                "codex_desktop_adapter",
                "codex_cli_adapter",
                "claude_code_adapter",
                "gemini_cli_adapter"
            ])
        );
        assert_eq!(
            artifacts["cli_family_capability_discovery"]["cli_hosts"]["codex_cli_adapter"]
                ["supports_cron"],
            Value::Bool(true)
        );
        assert_eq!(
            artifacts["cli_family_capability_discovery"]["cli_hosts"]["claude_code_adapter"]
                ["transport"],
            Value::String("headless-exec".to_string())
        );
        assert_eq!(
            artifacts["codex_common_adapter"]["controller_boundary"]["framework_truth"],
            Value::String("framework_core".to_string())
        );
        assert_eq!(
            artifacts["codex_desktop_adapter"]["entrypoint_contract"]["entrypoint_kind"],
            Value::String("interactive".to_string())
        );
        assert!(!artifacts.contains_key("codex_desktop_host_adapter"));
        assert_eq!(
            artifacts["codex_cli_adapter"]["execution_surface"]["controller_is_cli"],
            Value::Bool(false)
        );
        assert_eq!(
            artifacts["cli_family_parity_snapshot"]["all_shared_contract_checks_pass"],
            Value::Bool(true)
        );
        assert_eq!(
            artifacts["codex_dual_entry_parity_snapshot"]["all_shared_contract_checks_pass"],
            Value::Bool(true)
        );
        assert_eq!(
            artifacts["execution_controller_contract"]["controller"]["primary_owner"],
            Value::String("execution-controller-coding".to_string())
        );
        assert_eq!(
            artifacts["delegation_contract"]["gate"]["gate_skill"],
            Value::String("subagent-delegation".to_string())
        );
        assert_eq!(
            artifacts["supervisor_state_contract"]["state_artifact_path"],
            Value::String(".supervisor_state.json".to_string())
        );
        assert_eq!(
            artifacts["supervisor_state_contract"]["schema_expectations"]["verification_fields"],
            json!(["verification_status", "last_verification_summary"])
        );
        assert_eq!(
            artifacts["execution_kernel_live_fallback_retirement_status"]["live_primary"]
                ["contract_mode"],
            Value::String("rust-live-primary".to_string())
        );
        assert_eq!(
            artifacts["execution_kernel_live_fallback_retirement_status"]["retirement_readiness"]
                ["runtime_control_flow_change_required"],
            Value::Bool(false)
        );
        assert_eq!(
            artifacts["execution_kernel_live_fallback_retirement_status"]["current_contract_truth"]
                ["dry_run_delegate_kind"],
            Value::String("router-rs".to_string())
        );
        assert_eq!(
            artifacts["execution_kernel_live_fallback_retirement_status"]["current_contract_truth"]
                ["live_prompt_preview_passthrough_disabled"],
            Value::Bool(true)
        );
        assert_eq!(
            artifacts["execution_kernel_live_fallback_retirement_status"]
                ["current_response_metadata_truth"]["live_delegate_family"],
            Value::String("rust-cli".to_string())
        );
        assert_eq!(
            artifacts["execution_kernel_live_fallback_retirement_status"]
                ["current_response_metadata_truth"]
                ["compatibility_fallback_reason_present_in_steady_state"],
            Value::Bool(false)
        );
        assert_eq!(
            artifacts["execution_kernel_live_response_serialization_contract"]["status_contract"],
            Value::String("execution_kernel_live_response_serialization_contract_v1".to_string())
        );
        assert_eq!(
            artifacts["execution_kernel_live_response_serialization_contract"]
                ["runtime_response_metadata_fields"]["live_primary"],
            json!([
                "run_id",
                "status",
                "execution_mode",
                "route_engine",
                "diagnostic_route_mode",
                "execution_kernel_model_id_source"
            ])
        );
        assert_eq!(
            artifacts["execution_kernel_live_response_serialization_contract"]
                ["current_response_shape_truth"]["dry_run"]["model_id_present"],
            Value::Bool(false)
        );
    }

    #[test]
    fn codex_artifact_bundle_can_opt_in_continuity_alias_artifact() {
        let artifacts =
            build_codex_artifact_bundle(&sample_profile(), true).expect("artifacts should build");
        assert!(artifacts.contains_key("codex_desktop_alias_retirement_status"));
        assert_eq!(
            artifacts["cli_common_adapter"]["controller_boundary"]["cli_family_entrypoints"],
            json!([
                "codex_cli_adapter",
                "claude_code_adapter",
                "gemini_cli_adapter"
            ])
        );
        assert_eq!(
            artifacts["codex_common_adapter"]["controller_boundary"]["framework_truth"],
            Value::String("framework_core".to_string())
        );
        assert_eq!(
            artifacts["codex_desktop_adapter"]["entrypoint_contract"]["entrypoint_kind"],
            Value::String("interactive".to_string())
        );
        assert_eq!(
            artifacts["codex_cli_adapter"]["execution_surface"]["entrypoint_kind"],
            Value::String("headless".to_string())
        );
        assert_eq!(
            artifacts["codex_desktop_alias_retirement_status"]["canonical_adapter_id"],
            Value::String("codex_desktop_adapter".to_string())
        );
        assert_eq!(
            artifacts["codex_desktop_alias_retirement_status"]["primary_regression_artifact"],
            Value::String("cli_family_parity_snapshot".to_string())
        );
        assert_eq!(
            artifacts["claude_code_adapter"]["host_projection"]["settings_paths"],
            json!([
                "~/.claude/settings.json",
                ".claude/settings.json",
                ".claude/settings.local.json"
            ])
        );
        assert_eq!(
            artifacts["cli_family_capability_discovery"]["all_cli_hosts_compatible"],
            Value::Bool(true)
        );
        assert_eq!(
            artifacts["execution_kernel_live_fallback_retirement_status"]["compatibility_fallback"]
                ["retired_mode"],
            Value::String("retired".to_string())
        );
        assert_eq!(
            artifacts["execution_kernel_live_fallback_retirement_status"]
                ["public_runtime_contract_fields"],
            json!([
                "execution_kernel",
                "execution_kernel_authority",
                "execution_kernel_contract_mode",
                "execution_kernel_in_process_replacement_complete",
                "execution_kernel_delegate",
                "execution_kernel_delegate_authority",
                "execution_kernel_live_primary",
                "execution_kernel_live_primary_authority"
            ])
        );
        assert_eq!(
            artifacts["execution_kernel_live_fallback_retirement_status"]
                ["public_runtime_response_metadata_fields"],
            json!([
                "execution_kernel_delegate_family",
                "execution_kernel_delegate_impl"
            ])
        );
        assert_eq!(
            artifacts["execution_kernel_live_fallback_retirement_status"]
                ["retired_runtime_response_metadata_fields"],
            json!(["execution_kernel_fallback_reason"])
        );
        assert_eq!(
            artifacts["execution_kernel_live_fallback_retirement_status"]["retirement_gates"]
                ["compatibility_fallback_reason_metadata_still_python_owned"],
            Value::Bool(false)
        );
        assert_eq!(
            artifacts["execution_kernel_live_fallback_retirement_status"]["retirement_gates"]
                ["response_metadata_surface_externalized"],
            Value::Bool(true)
        );
        assert_eq!(
            artifacts["execution_kernel_live_response_serialization_contract"]["retirement_gates"]
                ["compatibility_live_response_serialization_still_python_owned"],
            Value::Bool(false)
        );
        assert_eq!(
            artifacts["execution_kernel_live_response_serialization_contract"]
                ["current_contract_truth"]["live_primary_model_id_source"],
            Value::String("aggregator-response.model".to_string())
        );
        assert_eq!(
            artifacts["codex_desktop_host_adapter"]["metadata"]["adapter_alias_of"],
            Value::String("codex_desktop_adapter".to_string())
        );
    }

    #[test]
    fn validation_rejects_aionrs_pinned_host_family() {
        let mut profile = sample_profile();
        profile.host_family = "aionrs".to_string();
        let error = build_profile_bundle(&profile).expect_err("should reject pinned host family");
        assert!(error.contains("must not be pinned directly to aionrs"));
    }

    #[test]
    fn validation_rejects_host_specific_metadata_in_framework_truth() {
        let mut profile = sample_profile();
        profile
            .metadata
            .insert("hook_event_names".to_string(), json!(["PreToolUse"]));
        let error = build_profile_bundle(&profile)
            .expect_err("should reject host-specific metadata in framework truth");
        assert!(error.contains("host-neutral"));
        assert!(error.contains("hook_event_names"));
    }
}
