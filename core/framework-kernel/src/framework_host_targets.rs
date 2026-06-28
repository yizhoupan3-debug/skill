//! Host projection identifiers and `--to` tool names aligned with
//! `configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported`.

use crate::runtime_registry::{
    HOST_ADAPTER_CONTRACT_PATH, RUNTIME_REGISTRY_PATH, load_runtime_registry_json,
};
use core_errors::FrameworkError;
use serde_json::{Map, Value, json};
use std::path::Path;

/// Canonical agent policy path used across all hosts.
const AGENT_POLICY_PATH: &str = "AGENTS.md";

pub fn host_targets_supported_host_ids(registry: &Value) -> Result<Vec<String>, FrameworkError> {
    let out = registry
        .get("host_targets")
        .and_then(|o| o.get("supported"))
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    v.as_str().map(|s| s.trim()).filter(|s| !s.is_empty())
                })
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            FrameworkError::validation("RUNTIME_REGISTRY: host_targets.supported missing or empty (see configs/framework/RUNTIME_REGISTRY.json)")
        })?;
    Ok(out)
}

fn host_metadata_error(host_id: &str, field: &str) -> FrameworkError {
    FrameworkError::validation(format!(
        "RUNTIME_REGISTRY host_targets.supported lists {host_id:?} but host_targets.metadata.{host_id}.{field} is missing or invalid; update {RUNTIME_REGISTRY_PATH} and {HOST_ADAPTER_CONTRACT_PATH}"
    ))
}

fn host_target_metadata<'a>(
    registry: &'a Value,
    host_id: &str,
) -> Result<&'a serde_json::Map<String, Value>, FrameworkError> {
    registry
        .get("host_targets")
        .and_then(|o| o.get("metadata"))
        .and_then(|o| o.get(host_id))
        .and_then(Value::as_object)
        .ok_or_else(|| host_metadata_error(host_id, "<host>"))
}

/// Logical id in `host_targets.supported` → `framework host-integration --to …` spelling.
pub fn skills_install_tool_for_host_id(
    registry: &Value,
    host_id: &str,
) -> Result<String, FrameworkError> {
    let id = host_id.trim();
    let tool = host_target_metadata(registry, id)?
        .get("install_tool")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| host_metadata_error(id, "install_tool"))?;
    Ok(tool.to_string())
}

pub(crate) fn projection_status_for_host_id(
    registry: &Value,
    host_id: &str,
) -> Result<String, FrameworkError> {
    let id = host_id.trim();
    let status = host_target_metadata(registry, id)?
        .get("projection_status")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| host_metadata_error(id, "projection_status"))?;
    Ok(status.to_string())
}

pub fn host_is_installable(registry: &Value, host_id: &str) -> Result<bool, FrameworkError> {
    let id = host_id.trim();
    host_target_metadata(registry, id)?
        .get("installable")
        .and_then(Value::as_bool)
        .ok_or_else(|| host_metadata_error(id, "installable"))
}

pub fn skills_install_tools_ordered(framework_root: &Path) -> Result<Vec<String>, FrameworkError> {
    let reg = load_runtime_registry_json(framework_root)?;
    installable_skills_tools_from_registry(&reg)
}

fn installable_skills_tools_from_registry(reg: &Value) -> Result<Vec<String>, FrameworkError> {
    let ids = host_targets_supported_host_ids(reg)?;
    let mut tools = Vec::with_capacity(ids.len());
    for id in &ids {
        if !host_is_installable(reg, id)?
            || projection_status_for_host_id(reg, id)? != "implemented"
        {
            continue;
        }
        let t = skills_install_tool_for_host_id(reg, id)?;
        if !tools.contains(&t) {
            tools.push(t);
        }
    }
    Ok(tools)
}

pub fn installable_host_id_and_skills_install_tool_pairs(
    framework_root: &Path,
) -> Result<Vec<(String, String)>, FrameworkError> {
    let reg = load_runtime_registry_json(framework_root)?;
    let pairs = host_id_and_skills_install_tool_pairs_from_registry(&reg)?;
    pairs
        .into_iter()
        .filter_map(|(host_id, tool)| {
            let installable = match host_is_installable(&reg, &host_id) {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let status = match projection_status_for_host_id(&reg, &host_id) {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            if installable && status == "implemented" {
                Some(Ok((host_id, tool)))
            } else {
                None
            }
        })
        .collect()
}

pub fn host_id_and_skills_install_tool_pairs(
    framework_root: &Path,
) -> Result<Vec<(String, String)>, FrameworkError> {
    let reg = load_runtime_registry_json(framework_root)?;
    host_id_and_skills_install_tool_pairs_from_registry(&reg)
}

pub(crate) fn host_id_and_skills_install_tool_pairs_from_registry(
    registry: &Value,
) -> Result<Vec<(String, String)>, FrameworkError> {
    let ids = host_targets_supported_host_ids(registry)?;
    let mut pairs = Vec::with_capacity(ids.len());
    for id in ids {
        let tool = skills_install_tool_for_host_id(registry, &id)?;
        pairs.push((id, tool));
    }
    Ok(pairs)
}

pub fn sync_manifest_shared_system_block(repo_root: &Path) -> Result<Value, FrameworkError> {
    let reg = load_runtime_registry_json(repo_root)?;
    let pairs = host_id_and_skills_install_tool_pairs_from_registry(&reg)?;
    let supported_hosts: Vec<Value> = pairs.iter().map(|(id, _)| json!(id)).collect();
    let mut host_entrypoints = Map::new();
    for (id, _) in &pairs {
        host_entrypoints.insert(id.clone(), host_entrypoints_value_for_id(&reg, id)?);
    }
    Ok(json!({
        "policy": "host-specific-agent-policy-v1",
        "routing_source_of_truth": "skills/",
        "agent_policy_entrypoint": AGENT_POLICY_PATH,
        "supported_hosts": supported_hosts,
        "host_entrypoints": Value::Object(host_entrypoints),
    }))
}

pub fn host_provider_manifest_host_ids(registry: &Value) -> Result<Vec<String>, FrameworkError> {
    let supported = host_targets_supported_host_ids(registry)?;
    let manifest = registry
        .get("host_targets")
        .and_then(|o| o.get("host_providers"))
        .and_then(Value::as_object)
        .ok_or_else(|| {
            FrameworkError::validation(format!(
                "RUNTIME_REGISTRY missing host_targets.host_providers; see {HOST_ADAPTER_CONTRACT_PATH}"
            ))
        })?;
    for host_id in &supported {
        if !manifest.contains_key(host_id) {
            return Err(FrameworkError::validation(format!(
                "RUNTIME_REGISTRY host_targets.host_providers missing supported host `{host_id}`"
            )));
        }
    }
    for host_id in manifest.keys() {
        if !supported.iter().any(|id| id == host_id) {
            return Err(FrameworkError::validation(format!(
                "RUNTIME_REGISTRY host_targets.host_providers has `{host_id}` not in host_targets.supported"
            )));
        }
    }
    Ok(supported)
}

pub fn validate_host_providers_against_registry(registry: &Value) -> Result<(), FrameworkError> {
    // Note: host_provider validation requires HostProvider trait (in host-projection).
    // Full validation is performed by runtime-core's build.rs and host_integration.
    let _supported = host_provider_manifest_host_ids(registry)?;
    Ok(())
}

/// Ensure each `host_providers` row matches `hosts/mod.rs` `#[cfg(feature)]` provider mods and optional hooks mods.
pub(crate) fn validate_host_provider_mod_declarations(
    registry: &Value,
    _hosts_mod_rs: &str,
    cargo_toml: &str,
) -> Result<(), FrameworkError> {
    let manifest = registry
        .get("host_targets")
        .and_then(|o| o.get("host_providers"))
        .and_then(Value::as_object)
        .ok_or_else(|| {
            FrameworkError::validation(format!(
                "RUNTIME_REGISTRY missing host_targets.host_providers; see {HOST_ADAPTER_CONTRACT_PATH}"
            ))
        })?;

    for (host_id, entry) in manifest {
        let entry = entry.as_object().ok_or_else(|| {
            FrameworkError::validation(format!("host_providers.{host_id} must be an object"))
        })?;
        let cargo_feature = entry
            .get("cargo_feature")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                FrameworkError::validation(format!(
                    "host_providers.{host_id}.cargo_feature required"
                ))
            })?;

        if !cargo_toml_declares_feature(cargo_toml, cargo_feature) {
            return Err(FrameworkError::validation(format!(
                "host_providers.{host_id}: Cargo.toml [features] missing `{cargo_feature}`; \
                 add it manually (features are not generated from RUNTIME_REGISTRY.json)"
            )));
        }

        // hosts/mod.rs is now a re-export shim from host-projection;
        // skip mod-declaration validation (host-projection owns the modules).
    }
    Ok(())
}

fn cargo_toml_declares_feature(cargo_toml: &str, feature: &str) -> bool {
    let needle = format!("{feature} =");
    cargo_toml
        .lines()
        .any(|line| line.trim().starts_with(&needle))
}

pub fn host_entrypoints_value_for_id(
    registry: &Value,
    host_id: &str,
) -> Result<Value, FrameworkError> {
    let id = host_id.trim();
    let value = host_target_metadata(registry, id)?
        .get("host_entrypoints")
        .cloned()
        .ok_or_else(|| host_metadata_error(id, "host_entrypoints"))?;
    match &value {
        Value::String(text) if !text.trim().is_empty() => Ok(value),
        Value::Array(items)
            if !items.is_empty()
                && items
                    .iter()
                    .all(|item| item.as_str().is_some_and(|text| !text.trim().is_empty())) =>
        {
            Ok(value)
        }
        _ => Err(host_metadata_error(id, "host_entrypoints")),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::runtime_registry::{ALL_HOST_IDS, RUNTIME_REGISTRY_SCHEMA_VERSION};
    use std::path::PathBuf;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }

    #[test]
    fn registry_hosts_map_to_install_tools_and_manifest_entrypoints() {
        let root = repo_root();
        let pairs = host_id_and_skills_install_tool_pairs(&root).expect("pairs");
        assert!(pairs.iter().any(|(host_id, _)| host_id == "opencode"));
        let reg = load_runtime_registry_json(&root).expect("registry");
        for (host_id, tool) in pairs {
            assert!(
                ALL_HOST_IDS.contains(&tool.as_str()),
                "unexpected mapping {host_id} -> {tool}"
            );
            assert_eq!(
                projection_status_for_host_id(&reg, &host_id).unwrap(),
                "implemented"
            );
            host_entrypoints_value_for_id(&reg, &host_id).unwrap();
        }
    }

    #[test]
    fn supported_hosts_match_agents_closed_set() {
        let root = repo_root();
        let reg = load_runtime_registry_json(&root).expect("registry");
        let supported = host_targets_supported_host_ids(&reg).expect("supported");
        let mut sorted = supported.clone();
        sorted.sort();
        let mut expected: Vec<&str> = crate::runtime_registry::ALL_HOST_IDS.to_vec();
        expected.sort();
        assert_eq!(
            sorted, expected,
            "supported host set must match build-time ALL_HOST_IDS"
        );
    }

    #[test]
    fn installable_pairs_exclude_retired_host_ids() {
        let root = repo_root();
        let pairs = installable_host_id_and_skills_install_tool_pairs(&root).expect("pairs");
        for host_id in crate::runtime_registry::ALL_HOST_IDS {
            assert!(
                pairs.iter().any(|(hid, _)| hid == *host_id),
                "installable_pairs must include {host_id}"
            );
        }
        assert!(!pairs.iter().any(|(host_id, _)| host_id == "codex-app"));
        assert!(!pairs.iter().any(|(host_id, _)| host_id == "codex-cli"));
        assert!(!pairs.iter().any(|(host_id, _)| host_id == "claude-desktop"));
    }

    #[test]
    fn host_provider_manifest_aligns_with_supported_hosts() {
        let root = repo_root();
        let reg = load_runtime_registry_json(&root).expect("registry");
        let manifest_ids = host_provider_manifest_host_ids(&reg).expect("host_providers manifest");
        let supported = host_targets_supported_host_ids(&reg).expect("supported");
        assert_eq!(manifest_ids, supported);
    }

    #[test]
    fn host_provider_mod_declarations_align_with_registry() {
        let root = repo_root();
        let reg = load_runtime_registry_json(&root).expect("registry");
        let hosts_mod = std::fs::read_to_string(root.join("core/host-projection/src/hosts/mod.rs"))
            .expect("hosts/mod.rs");
        let cargo_toml =
            std::fs::read_to_string(root.join("core/runtime-core/Cargo.toml")).expect("Cargo.toml");
        validate_host_provider_mod_declarations(&reg, &hosts_mod, &cargo_toml)
            .expect("host_providers vs hosts/mod.rs + Cargo.toml features");
    }

    #[test]
    fn supported_host_without_metadata_fails_with_maintenance_hint() {
        let reg = json!({
            "schema_version": RUNTIME_REGISTRY_SCHEMA_VERSION,
            "host_targets": {
                "supported": ["cursor", "new-host"],
                "metadata": {
                    "cursor": {
                        "install_tool": "cursor",
                        "projection_status": "implemented",
                        "installable": true,
                        "host_entrypoints": "AGENTS.md"
                    }
                }
            }
        });
        let err = host_id_and_skills_install_tool_pairs_from_registry(&reg)
            .expect_err("new host without metadata must fail closed");
        assert!(err.to_string().contains("new-host"), "{err}");
        assert!(err.to_string().contains(RUNTIME_REGISTRY_PATH), "{err}");
        assert!(
            err.to_string().contains(HOST_ADAPTER_CONTRACT_PATH),
            "{err}"
        );
    }
}
