use serde_json::{Map, Value};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

mod contracts;
mod types;

pub use contracts::{
    build_capability_surface, build_control_plane_contract_descriptors, build_delegation_contract,
    build_execution_controller_contract, build_execution_protocol_contract,
    build_host_alias_entrypoints, build_runtime_surface, build_supervisor_state_contract,
    complete_host_payload,
};
pub use types::{
    CapabilityBundle, DELEGATION_CONTRACT_ARTIFACT_ID, EXECUTION_CONTROLLER_CONTRACT_ARTIFACT_ID,
    EXECUTION_PROTOCOL_CONTRACT_ARTIFACT_ID, FrameworkProfileContract, HostProfileBuildContext,
    ProfileBundle, REQUIRED_CORE_CAPABILITIES, SUPERVISOR_STATE_CONTRACT_ARTIFACT_ID,
};

use types::HOST_SPECIFIC_METADATA_KEYS;

// ── repo root (build-time only) ──

fn repo_scan_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

// ── load / validate / bundle ──

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

    let normalized_mcp_servers = normalize_mcp_servers(&profile.mcp_servers);
    let workspace_bootstrap = compile_workspace_bootstrap(profile);
    let shared_contract =
        build_shared_contract(profile, &normalized_mcp_servers, &workspace_bootstrap);
    let host_specs = load_host_profile_specs()?;
    let codex_spec = host_specs
        .iter()
        .find(|spec| spec.host_key == "codex")
        .ok_or_else(|| {
            "RUNTIME_REGISTRY host_projections must include codex for legacy codex_profile"
                .to_string()
        })?;
    let codex_host_payload = codex_spec.build_payload();
    let mut host_payloads = Map::new();
    for spec in &host_specs {
        host_payloads.insert(spec.host_key.clone(), Value::Object(spec.build_payload()));
    }
    let codex_profile = build_host_profile(
        profile,
        &normalized_mcp_servers,
        &workspace_bootstrap,
        &shared_contract,
        &codex_host_payload,
        codex_spec,
        false,
    );
    let full_codex_profile = build_host_profile(
        profile,
        &normalized_mcp_servers,
        &workspace_bootstrap,
        &shared_contract,
        &codex_host_payload,
        codex_spec,
        true,
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
        mcp_servers: normalized_mcp_servers.clone(),
        workspace_bootstrap: workspace_bootstrap.clone(),
        host_capability_requirements: profile.host_capability_requirements.clone(),
        metadata: profile.metadata.clone(),
        codex_profile: Value::Object(codex_profile),
        full_codex_profile: Value::Object(full_codex_profile),
        host_payloads,
    })
}

pub fn build_codex_artifact_bundle(
    profile: &FrameworkProfileContract,
    full: bool,
) -> Result<Map<String, Value>, String> {
    let bundle = build_profile_bundle(profile)?;
    let mut artifacts = Map::new();
    artifacts.insert(
        "codex_profile".to_string(),
        if full {
            bundle.full_codex_profile
        } else {
            bundle.codex_profile
        },
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
    if profile.host_family.trim() != "shared-rust-core" {
        return Err("framework core must be pinned to shared-rust-core".to_string());
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
            "framework profile metadata must stay shared-core-only; move host-private keys into explicit host payloads: {}",
            host_specific_metadata.join(", ")
        ));
    }
    Ok(())
}

// ── normalization ──

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

fn compile_workspace_bootstrap(profile: &FrameworkProfileContract) -> Map<String, Value> {
    let mut bootstrap = profile.workspace_bootstrap.clone();
    let mut resources = bootstrap
        .get("resources")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    if !resources.contains_key("skills") {
        let skills = bootstrap.get("skills").cloned().unwrap_or_else(|| {
            value_object([
                ("project_dir", Value::String("skills".to_string())),
                (
                    "user_dir",
                    Value::String("${CODEX_HOME}/skills".to_string()),
                ),
                ("source_dir", Value::String("skills".to_string())),
            ])
        });
        resources.insert("skills".to_string(), skills);
    }
    bootstrap.insert("resources".to_string(), Value::Object(resources));
    bootstrap.remove("bridges");
    bootstrap.remove("skill_bridge");
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

// ── shared contract ──

fn build_shared_contract(
    profile: &FrameworkProfileContract,
    normalized_mcp_servers: &[Value],
    workspace_bootstrap: &Map<String, Value>,
) -> Map<String, Value> {
    let mut shared_contract = Map::new();
    shared_contract.insert(
        "routing".to_string(),
        serde_json::json!({
            "mode": profile
                .session_policy
                .get("mode")
                .and_then(Value::as_str)
                .unwrap_or("default"),
            "session_mode": compile_session_mode(&profile.session_policy),
        }),
    );
    shared_contract.insert(
        "framework_surface_policy".to_string(),
        Value::Object(profile.framework_surface_policy.clone()),
    );
    shared_contract.insert(
        "continuity_contract".to_string(),
        serde_json::json!({
            "checkpointing_supported": false,
            "max_continuation_depth": 5,
            "continuation_links": [],
        }),
    );
    shared_contract.insert(
        "artifact_contract".to_string(),
        Value::Object(profile.artifact_contract.clone()),
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
        "workspace_bootstrap".to_string(),
        Value::Object(workspace_bootstrap.clone()),
    );
    shared_contract.insert(
        "session_contract".to_string(),
        serde_json::json!({
            "session_policy": profile.session_policy,
            "model_policy": profile.model_policy,
        }),
    );
    shared_contract.insert(
        "execution_protocol_contract".to_string(),
        Value::Object(build_execution_protocol_contract()),
    );
    shared_contract.insert(
        "execution_controller_contract".to_string(),
        Value::Object(build_execution_controller_contract()),
    );
    shared_contract.insert(
        "delegation_contract".to_string(),
        Value::Object(build_delegation_contract()),
    );
    shared_contract.insert(
        "supervisor_state_contract".to_string(),
        Value::Object(build_supervisor_state_contract()),
    );
    shared_contract.insert(
        "mcp_servers".to_string(),
        Value::Array(normalized_mcp_servers.to_vec()),
    );
    shared_contract.insert(
        "host_projection".to_string(),
        serde_json::json!({
            "mode": "shared-rust-core",
            "metadata": profile.metadata,
        }),
    );
    shared_contract
}

// ── HostProfileSpec ──

#[derive(Clone, Debug)]
struct HostProfileSpec {
    host_key: String,
    host_cli: String,
    transport: String,
    capabilities: Vec<String>,
    projection: Map<String, Value>,
}

impl HostProfileSpec {
    fn build_payload(&self) -> Map<String, Value> {
        use contracts::complete_host_payload;
        let mut payload = complete_host_payload(&self.host_cli, Map::new());
        payload.insert(
            "transport".to_string(),
            Value::String(self.transport.clone()),
        );
        payload.insert(
            "capabilities".to_string(),
            Value::Array(
                self.capabilities
                    .iter()
                    .map(|c| Value::String(c.clone()))
                    .collect(),
            ),
        );
        payload.insert("host_id".to_string(), Value::String(self.host_cli.clone()));
        for (key, value) in &self.projection {
            payload.insert(key.clone(), value.clone());
        }
        payload
    }
}

fn load_host_profile_specs() -> Result<Vec<HostProfileSpec>, String> {
    let registry_path = repo_scan_root()
        .join("configs")
        .join("framework")
        .join("RUNTIME_REGISTRY.json");
    let raw = fs::read_to_string(&registry_path)
        .map_err(|err| format!("failed reading {}: {err}", registry_path.display()))?;
    let registry: Value = serde_json::from_str(&raw)
        .map_err(|err| format!("failed parsing {}: {err}", registry_path.display()))?;
    let projections = registry
        .get("host_projections")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "RUNTIME_REGISTRY.json missing host_projections for profile bundle".to_string()
        })?;
    let mut keys = projections.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    let mut specs = Vec::with_capacity(keys.len());
    for host_key in keys {
        let projection = projections
            .get(&host_key)
            .and_then(Value::as_object)
            .ok_or_else(|| format!("host_projections.{host_key} must be an object"))?;
        let host_cli = projection
            .get("host_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("host_projections.{host_key}.host_id is required"))?
            .to_string();
        let transport = projection
            .get("transport")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("host_projections.{host_key}.transport is required"))?
            .to_string();
        let capabilities = projection
            .get("capabilities")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("host_projections.{host_key}.capabilities is required"))?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::trim)
                    .filter(|text| !text.is_empty())
                    .map(str::to_string)
                    .ok_or_else(|| {
                        format!(
                            "host_projections.{host_key}.capabilities must contain non-empty strings"
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        specs.push(HostProfileSpec {
            host_key,
            host_cli,
            transport,
            capabilities,
            projection: projection.clone(),
        });
    }
    Ok(specs)
}

#[allow(clippy::too_many_arguments)]
fn build_host_profile(
    profile: &FrameworkProfileContract,
    normalized_mcp_servers: &[Value],
    workspace_bootstrap: &Map<String, Value>,
    shared_contract: &Map<String, Value>,
    host_payload: &Map<String, Value>,
    host_spec: &HostProfileSpec,
    include_full_contract: bool,
) -> Map<String, Value> {
    let ctx = HostProfileBuildContext {
        normalized_mcp_servers,
        workspace_bootstrap,
    };
    let mut profile_map = Map::new();
    profile_map.insert(
        "profile_id".to_string(),
        Value::String(profile.profile_id.clone()),
    );
    profile_map.insert(
        "display_name".to_string(),
        Value::String(profile.display_name.clone()),
    );
    profile_map.insert(
        "framework_profile_version".to_string(),
        Value::String(profile.framework_profile_version.clone()),
    );
    profile_map.insert(
        "runtime_family".to_string(),
        Value::String(profile.runtime_family.clone()),
    );
    profile_map.insert(
        "host_family".to_string(),
        Value::String(profile.host_family.clone()),
    );
    profile_map.insert(
        "capabilities".to_string(),
        serde_json::json!({
            "core": profile.core_capabilities,
            "optional": profile.optional_capabilities,
            "host": host_spec.capabilities,
        }),
    );

    let runtime_surface = build_runtime_surface(shared_contract);
    let capability_surface = build_capability_surface(shared_contract);

    if include_full_contract {
        let mut full_shared = shared_contract.clone();
        merge_json_maps(&mut full_shared, &runtime_surface);
        merge_json_maps(&mut full_shared, &capability_surface);
        profile_map.insert("profile".to_string(), Value::Object(full_shared));
    } else {
        profile_map.insert("runtime".to_string(), Value::Object(runtime_surface));
        profile_map.insert(
            "capability_surface".to_string(),
            Value::Object(capability_surface),
        );
        profile_map.insert(
            "host_payload".to_string(),
            Value::Object(host_payload.clone()),
        );
        profile_map.insert(
            "host_alias_entrypoints".to_string(),
            build_host_alias_entrypoints(&host_spec.host_key),
        );
    }

    resolve_host_capability_requirements(profile, &ctx, &mut profile_map);

    profile_map
}

fn resolve_host_capability_requirements(
    profile: &FrameworkProfileContract,
    ctx: &HostProfileBuildContext,
    profile_map: &mut Map<String, Value>,
) {
    for (key, requirement) in &profile.host_capability_requirements {
        let satisfied = match requirement.as_str() {
            Some("required") => true,
            Some("optional") => false,
            Some("mcp_server") => ctx
                .normalized_mcp_servers
                .iter()
                .any(|server| server.get("server_id") == Some(&Value::String(key.clone()))),
            Some("workspace_bootstrap") => ctx.workspace_bootstrap.contains_key(key),
            _ => false,
        };
        profile_map.insert(format!("capability_{key}"), Value::Bool(satisfied));
    }
}

// ── merge helpers ──

fn merge_json_maps(target: &mut Map<String, Value>, override_map: &Map<String, Value>) {
    for (key, value) in override_map {
        target.insert(key.clone(), value.clone());
    }
}

// ── tiny helpers ──

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

// ── tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_profile() -> FrameworkProfileContract {
        serde_json::from_value(json!({
            "profile_id": "test-profile",
            "display_name": "Test Profile",
            "framework_profile_version": "0.1.0",
            "runtime_family": "portable",
            "host_family": "shared-rust-core",
            "core_capabilities": ["runtime", "artifact", "orchestration"],
        }))
        .expect("sample profile")
    }

    #[test]
    fn load_and_roundtrip() {
        let profile = sample_profile();
        assert_eq!(profile.profile_id, "test-profile");
        assert_eq!(profile.framework_profile_version, "0.1.0");
        assert_eq!(profile.core_capabilities.len(), 3);
    }

    #[test]
    fn validate_passes_for_valid_profile() {
        let profile = sample_profile();
        assert!(validate_framework_profile(&profile).is_ok());
    }

    #[test]
    fn validate_rejects_missing_profile_id() {
        let mut profile = sample_profile();
        profile.profile_id = "  ".to_string();
        assert!(validate_framework_profile(&profile).is_err());
    }

    #[test]
    fn validate_rejects_wrong_host_family() {
        let mut profile = sample_profile();
        profile.host_family = "some-other".to_string();
        assert!(validate_framework_profile(&profile).is_err());
    }

    #[test]
    fn validate_rejects_missing_core_capabilities() {
        let mut profile = sample_profile();
        profile.core_capabilities = vec!["runtime".to_string()];
        assert!(validate_framework_profile(&profile).is_err());
    }

    #[test]
    fn build_shared_contract_creates_all_keys() {
        let profile = sample_profile();
        let mcp = normalize_mcp_servers(&[]);
        let wb = compile_workspace_bootstrap(&profile);
        let shared = build_shared_contract(&profile, &mcp, &wb);
        for key in &[
            "routing",
            "framework_surface_policy",
            "continuity_contract",
            "artifact_contract",
            "tool_policy",
            "approval_policy",
            "loadout_policy",
            "workspace_bootstrap",
            "session_contract",
            "execution_protocol_contract",
            "execution_controller_contract",
            "delegation_contract",
            "supervisor_state_contract",
            "mcp_servers",
            "host_projection",
        ] {
            assert!(shared.contains_key(*key), "shared_contract missing {key}");
        }
    }

    #[test]
    fn build_profile_bundle_succeeds_for_valid_profile() {
        let profile = sample_profile();
        let result = build_profile_bundle(&profile);
        assert!(
            result.is_ok(),
            "build_profile_bundle failed: {:?}",
            result.err()
        );
    }

    #[test]
    fn validation_rejects_host_specific_metadata_in_framework_truth() {
        let mut profile = sample_profile();
        profile
            .metadata
            .insert("settings_paths".to_string(), json!([".codex/config.toml"]));
        let error = build_profile_bundle(&profile)
            .expect_err("should reject host-specific metadata in framework truth");
        assert!(error.contains("shared-core-only"));
    }
}
