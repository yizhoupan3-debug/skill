use core_errors::FrameworkError;
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

mod contracts;
mod profile_validate;
mod types;

pub use contracts::build_control_plane_contract_descriptors;
pub(crate) use contracts::{
    build_capability_surface, build_delegation_contract, build_execution_controller_contract,
    build_execution_protocol_contract, build_host_alias_entrypoints, build_runtime_surface,
    build_supervisor_state_contract,
};
pub(crate) use profile_validate::validate_framework_profile;
pub use types::{
    CapabilityBundle, DELEGATION_CONTRACT_ARTIFACT_ID, EXECUTION_CONTROLLER_CONTRACT_ARTIFACT_ID,
    EXECUTION_PROTOCOL_CONTRACT_ARTIFACT_ID, FrameworkProfileContract, HostProfileBuildContext,
    ProfileBundle, REQUIRED_CORE_CAPABILITIES, SUPERVISOR_STATE_CONTRACT_ARTIFACT_ID,
};

// ── repo root (build-time only) ──

fn repo_scan_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

// ── load / validate / bundle ──

pub fn load_framework_profile(path: &Path) -> Result<FrameworkProfileContract, FrameworkError> {
    let text = fs::read_to_string(path).map_err(|err| {
        FrameworkError::validation(format!("failed reading {}: {err}", path.display()))
    })?;
    let profile: FrameworkProfileContract = serde_json::from_str(&text).map_err(|err| {
        FrameworkError::validation(format!("failed parsing {}: {err}", path.display()))
    })?;
    validate_framework_profile(&profile)?;
    Ok(profile)
}

pub fn build_profile_bundle(
    profile: FrameworkProfileContract,
) -> Result<ProfileBundle, FrameworkError> {
    validate_framework_profile(&profile)?;

    let normalized_mcp_servers = normalize_mcp_servers(&profile.mcp_servers);
    let workspace_bootstrap = compile_workspace_bootstrap(&profile);
    let shared_contract =
        build_shared_contract(&profile, &normalized_mcp_servers, &workspace_bootstrap);
    let host_specs = load_host_profile_specs()?;

    let mut host_profiles = Map::new();
    let mut full_host_profiles = Map::new();
    let mut host_payloads = Map::new();
    for spec in &host_specs {
        let payload = spec.build_payload();
        host_payloads.insert(spec.host_key.clone(), Value::Object(payload.clone()));

        let compact = build_host_profile(
            &profile,
            &normalized_mcp_servers,
            &workspace_bootstrap,
            &shared_contract,
            &payload,
            spec,
            false,
        );
        let full = build_host_profile(
            &profile,
            &normalized_mcp_servers,
            &workspace_bootstrap,
            &shared_contract,
            &payload,
            spec,
            true,
        );
        host_profiles.insert(spec.host_key.clone(), Value::Object(compact));
        full_host_profiles.insert(spec.host_key.clone(), Value::Object(full));
    }

    let FrameworkProfileContract {
        profile_id,
        display_name,
        framework_profile_version,
        runtime_family,
        host_family,
        core_capabilities,
        optional_capabilities,
        rules_bundle,
        skill_bundle,
        session_policy,
        tool_policy,
        approval_policy,
        loadout_policy,
        framework_surface_policy,
        artifact_contract,
        model_policy,
        mcp_servers: _,
        workspace_bootstrap: _,
        host_capability_requirements,
        metadata,
    } = profile;

    Ok(ProfileBundle {
        profile_id,
        display_name,
        framework_profile_version,
        runtime_family,
        host_family,
        capabilities: CapabilityBundle {
            core: core_capabilities,
            optional: optional_capabilities,
        },
        rules_bundle,
        skill_bundle,
        session_policy,
        tool_policy,
        approval_policy,
        loadout_policy,
        framework_surface_policy,
        artifact_contract,
        model_policy,
        mcp_servers: normalized_mcp_servers,
        workspace_bootstrap,
        host_capability_requirements,
        metadata,
        host_profiles,
        full_host_profiles,
        host_payloads,
    })
}

pub fn build_profile_artifact_bundle(
    profile: FrameworkProfileContract,
    full: bool,
) -> Result<Map<String, Value>, FrameworkError> {
    let bundle = build_profile_bundle(profile)?;
    if full {
        Ok(bundle.full_host_profiles)
    } else {
        Ok(bundle.host_profiles)
    }
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

    let default_skills = || {
        value_object([
            ("project_dir", Value::String("skills".to_string())),
            (
                "user_dir",
                Value::String("${CODEX_HOME}/skills".to_string()),
            ),
            ("source_dir", Value::String("skills".to_string())),
        ])
    };

    // Check upfront (immutable borrow) to decide if skills need patching;
    // this avoids a simultaneous mutable borrow on `bootstrap` later.
    let needs_skills = match bootstrap.get("resources").and_then(Value::as_object) {
        Some(res) => !res.contains_key("skills"),
        None => true,
    };

    if needs_skills {
        let skills = match bootstrap.remove("skills") {
            Some(skills_val) => skills_val,
            None => default_skills(),
        };
        match bootstrap
            .get_mut("resources")
            .and_then(Value::as_object_mut)
        {
            Some(res) => {
                res.insert("skills".to_string(), skills);
            }
            None => {
                let mut resources = Map::new();
                resources.insert("skills".to_string(), skills);
                bootstrap.insert("resources".to_string(), Value::Object(resources));
            }
        }
    }

    bootstrap.remove("bridges");
    bootstrap.remove("skill_bridge");
    bootstrap
}

fn compile_session_mode(session_policy: &Map<String, Value>) -> Value {
    let mut extras = Map::new();
    for (key, value) in session_policy {
        if !matches!(
            key.as_str(),
            "mode" | "approval_mode" | "history_policy" | "takeover"
        ) {
            extras.insert(key.clone(), value.clone());
        }
    }

    let Value::Object(mut result) = serde_json::json!({
        "mode": session_policy.get("mode"),
        "approval_mode": session_policy.get("approval_mode"),
        "history_policy": session_policy.get("history_policy"),
        "takeover": session_policy.get("takeover"),
        "extras": extras,
    }) else {
        unreachable!()
    };
    // json! serializes `None` (from .get() for a missing key) as `Value::Null`.
    // Replace Null entries with the defaults that the old code applied.
    if matches!(result.get("mode"), Some(Value::Null)) {
        result.insert("mode".to_string(), Value::String("default".to_string()));
    }
    if matches!(result.get("approval_mode"), Some(Value::Null)) {
        result.insert(
            "approval_mode".to_string(),
            Value::String("inherit".to_string()),
        );
    }
    if matches!(result.get("history_policy"), Some(Value::Null)) {
        result.insert(
            "history_policy".to_string(),
            Value::String("host-managed".to_string()),
        );
    }
    if matches!(result.get("takeover"), Some(Value::Null)) {
        result.insert("takeover".to_string(), Value::Bool(false));
    }
    Value::Object(result)
}

// ── shared contract ──

fn build_shared_contract(
    profile: &FrameworkProfileContract,
    normalized_mcp_servers: &[Value],
    workspace_bootstrap: &Map<String, Value>,
) -> Map<String, Value> {
    let Value::Object(contract) = serde_json::json!({
        "routing": {
            "mode": profile
                .session_policy
                .get("mode")
                .and_then(Value::as_str)
                .unwrap_or("default"),
            "session_mode": compile_session_mode(&profile.session_policy),
        },
        "framework_surface_policy": profile.framework_surface_policy,
        "continuity_contract": {
            "checkpointing_supported": false,
            "max_continuation_depth": 5,
            "continuation_links": [],
        },
        "artifact_contract": profile.artifact_contract,
        "tool_policy": profile.tool_policy,
        "approval_policy": profile.approval_policy,
        "loadout_policy": profile.loadout_policy,
        "workspace_bootstrap": workspace_bootstrap,
        "session_contract": {
            "session_policy": profile.session_policy,
            "model_policy": profile.model_policy,
        },
        "execution_protocol_contract": build_execution_protocol_contract(),
        "execution_controller_contract": build_execution_controller_contract(),
        "delegation_contract": build_delegation_contract(),
        "supervisor_state_contract": build_supervisor_state_contract(),
        "mcp_servers": normalized_mcp_servers,
        "host_projection": {
            "mode": "shared-rust-core",
            "metadata": profile.metadata,
        },
    }) else {
        unreachable!()
    };
    contract
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
        // Vec<String> serialization to JSON never fails; safe unwrap.
        #[allow(clippy::unwrap_used)]
        let caps = serde_json::to_value(&self.capabilities).unwrap();
        payload.insert("capabilities".to_string(), caps);
        payload.insert("host_id".to_string(), Value::String(self.host_cli.clone()));
        payload.extend(self.projection.clone());
        payload
    }
}

fn load_host_profile_specs() -> Result<Vec<HostProfileSpec>, FrameworkError> {
    let registry_path = repo_scan_root()
        .join("configs")
        .join("framework")
        .join("RUNTIME_REGISTRY.json");
    let raw = fs::read_to_string(&registry_path).map_err(|err| {
        FrameworkError::validation(format!("failed reading {}: {err}", registry_path.display()))
    })?;
    let registry: Value = serde_json::from_str(&raw).map_err(|err| {
        FrameworkError::validation(format!("failed parsing {}: {err}", registry_path.display()))
    })?;
    let projections = registry
        .get("host_projections")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            FrameworkError::validation(
                "RUNTIME_REGISTRY.json missing host_projections for profile bundle",
            )
        })?;
    let mut keys = projections.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    let mut specs = Vec::with_capacity(keys.len());
    for host_key in keys {
        let projection = projections
            .get(&host_key)
            .and_then(Value::as_object)
            .ok_or_else(|| {
                FrameworkError::validation(format!("host_projections.{host_key} must be an object"))
            })?;
        let host_cli = projection
            .get("host_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                FrameworkError::validation(format!(
                    "host_projections.{host_key}.host_id is required"
                ))
            })?
            .to_string();
        let transport = projection
            .get("transport")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                FrameworkError::validation(format!(
                    "host_projections.{host_key}.transport is required"
                ))
            })?
            .to_string();
        let capabilities = projection
            .get("capabilities")
            .and_then(Value::as_array)
            .ok_or_else(|| FrameworkError::validation(format!("host_projections.{host_key}.capabilities is required")))?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::trim)
                    .filter(|text| !text.is_empty())
                    .map(str::to_string)
                    .ok_or_else(|| {
                        FrameworkError::validation(format!(
                            "host_projections.{host_key}.capabilities must contain non-empty strings"
                        ))
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
    let mut profile_map = {
        let Value::Object(map) = serde_json::json!({
            "profile_id": profile.profile_id,
            "display_name": profile.display_name,
            "framework_profile_version": profile.framework_profile_version,
            "runtime_family": profile.runtime_family,
            "host_family": profile.host_family,
        }) else {
            unreachable!()
        };
        map
    };
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
            Some("mcp_server") => ctx.normalized_mcp_servers.iter().any(|server| {
                server.get("server_id").and_then(Value::as_str) == Some(key.as_str())
            }),
            Some("workspace_bootstrap") => ctx.workspace_bootstrap.contains_key(key),
            _ => false,
        };
        profile_map.insert(format!("capability_{key}"), Value::Bool(satisfied));
    }
}

// ── merge helpers ──

fn merge_json_maps(target: &mut Map<String, Value>, override_map: &Map<String, Value>) {
    target.extend(override_map.clone());
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
    #![allow(clippy::unwrap_used, clippy::expect_used)]
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
        let result = build_profile_bundle(profile);
        assert!(
            result.is_ok(),
            "build_profile_bundle failed: {:?}",
            result.err()
        );
    }

    #[test]
    fn profile_bundle_serialization_matches_snapshot() {
        let profile = sample_profile();
        let bundle = build_profile_bundle(profile).expect("build_profile_bundle");
        let json = serde_json::to_value(&bundle).expect("serialize");
        insta::assert_json_snapshot!("profile_bundle_contract", json);
    }

    #[test]
    fn shared_contract_serialization_matches_snapshot() {
        let profile = sample_profile();
        let mcp = normalize_mcp_servers(&[]);
        let wb = compile_workspace_bootstrap(&profile);
        let shared = build_shared_contract(&profile, &mcp, &wb);
        insta::assert_json_snapshot!("shared_contract", shared);
    }

    #[test]
    fn validation_rejects_host_specific_metadata_in_framework_truth() {
        let mut profile = sample_profile();
        profile
            .metadata
            .insert("settings_paths".to_string(), json!([".codex/config.toml"]));
        let error = build_profile_bundle(profile)
            .expect_err("should reject host-specific metadata in framework truth");
        assert!(error.to_string().contains("shared-core-only"));
    }

    #[tokio::test]
    async fn build_profile_bundle_is_send_safe() {
        let profile = sample_profile();
        let result = tokio::task::spawn_blocking(move || build_profile_bundle(profile))
            .await
            .expect("spawn_blocking");
        assert!(result.is_ok(), "build_profile_bundle in tokio context");
    }

    #[tokio::test]
    async fn load_framework_profile_from_path_is_send_safe() {
        let tmp = std::env::temp_dir();
        let path = tmp.join("test-framework-profile.json");
        let profile = sample_profile();
        let json = serde_json::to_string_pretty(&profile).expect("serialize");
        std::fs::write(&path, &json).expect("write");
        let path_clone = path.clone();
        let result = tokio::task::spawn_blocking(move || load_framework_profile(&path_clone))
            .await
            .expect("spawn_blocking");
        assert!(result.is_ok(), "load_framework_profile in tokio context");
        let _ = std::fs::remove_file(&path);
    }
}
