//! Host projection identifiers and `--to` tool names aligned with
//! `configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported`.

use crate::hosts::codex_hooks::CODEX_AGENT_POLICY_PATH;
use crate::runtime_registry::{
    load_runtime_registry_json, HOST_ADAPTER_CONTRACT_PATH, RUNTIME_REGISTRY_PATH,
};
use serde_json::{json, Map, Value};
use std::path::Path;

pub fn host_targets_supported_host_ids(registry: &Value) -> Result<Vec<String>, String> {
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
            "RUNTIME_REGISTRY: host_targets.supported missing or empty (see configs/framework/RUNTIME_REGISTRY.json)"
                .to_string()
        })?;
    Ok(out)
}

fn host_metadata_error(host_id: &str, field: &str) -> String {
    format!(
        "RUNTIME_REGISTRY host_targets.supported lists {host_id:?} but host_targets.metadata.{host_id}.{field} is missing or invalid; update {RUNTIME_REGISTRY_PATH} and {HOST_ADAPTER_CONTRACT_PATH}"
    )
}

fn host_target_metadata<'a>(
    registry: &'a Value,
    host_id: &str,
) -> Result<&'a serde_json::Map<String, Value>, String> {
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
) -> Result<String, String> {
    let id = host_id.trim();
    let tool = host_target_metadata(registry, id)?
        .get("install_tool")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| host_metadata_error(id, "install_tool"))?;
    Ok(tool.to_string())
}

pub fn projection_status_for_host_id(
    registry: &Value,
    host_id: &str,
) -> Result<String, String> {
    let id = host_id.trim();
    let status = host_target_metadata(registry, id)?
        .get("projection_status")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| host_metadata_error(id, "projection_status"))?;
    Ok(status.to_string())
}

pub fn host_is_installable(registry: &Value, host_id: &str) -> Result<bool, String> {
    let id = host_id.trim();
    host_target_metadata(registry, id)?
        .get("installable")
        .and_then(Value::as_bool)
        .ok_or_else(|| host_metadata_error(id, "installable"))
}

pub fn skills_install_tools_ordered(framework_root: &Path) -> Result<Vec<String>, String> {
    let reg = load_runtime_registry_json(framework_root)?;
    installable_skills_tools_from_registry(&reg)
}

fn installable_skills_tools_from_registry(reg: &Value) -> Result<Vec<String>, String> {
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
) -> Result<Vec<(String, String)>, String> {
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
) -> Result<Vec<(String, String)>, String> {
    let reg = load_runtime_registry_json(framework_root)?;
    host_id_and_skills_install_tool_pairs_from_registry(&reg)
}

pub fn host_id_and_skills_install_tool_pairs_from_registry(
    registry: &Value,
) -> Result<Vec<(String, String)>, String> {
    let ids = host_targets_supported_host_ids(registry)?;
    let mut pairs = Vec::with_capacity(ids.len());
    for id in ids {
        let tool = skills_install_tool_for_host_id(registry, &id)?;
        pairs.push((id, tool));
    }
    Ok(pairs)
}

pub fn sync_manifest_shared_system_block(repo_root: &Path) -> Result<Value, String> {
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
        "agent_policy_entrypoint": CODEX_AGENT_POLICY_PATH,
        "supported_hosts": supported_hosts,
        "host_entrypoints": Value::Object(host_entrypoints),
    }))
}

pub fn host_provider_manifest_host_ids(registry: &Value) -> Result<Vec<String>, String> {
    let supported = host_targets_supported_host_ids(registry)?;
    let manifest = registry
        .get("host_targets")
        .and_then(|o| o.get("host_providers"))
        .and_then(Value::as_object)
        .ok_or_else(|| {
            format!(
                "RUNTIME_REGISTRY missing host_targets.host_providers; see {HOST_ADAPTER_CONTRACT_PATH}"
            )
        })?;
    for host_id in &supported {
        if !manifest.contains_key(host_id) {
            return Err(format!(
                "RUNTIME_REGISTRY host_targets.host_providers missing supported host `{host_id}`"
            ));
        }
    }
    for host_id in manifest.keys() {
        if !supported.iter().any(|id| id == host_id) {
            return Err(format!(
                "RUNTIME_REGISTRY host_targets.host_providers has `{host_id}` not in host_targets.supported"
            ));
        }
    }
    Ok(supported)
}

pub fn validate_host_providers_against_registry(registry: &Value) -> Result<(), String> {
    let supported = host_provider_manifest_host_ids(registry)?;
    crate::hosts::host_provider::validate_host_providers_against_registry(&supported)
}

/// Ensure each `host_providers` row matches `hosts/mod.rs` `#[cfg(feature)]` provider mods and optional hooks mods.
pub fn validate_host_provider_mod_declarations(
    registry: &Value,
    hosts_mod_rs: &str,
    cargo_toml: &str,
) -> Result<(), String> {
    let manifest = registry
        .get("host_targets")
        .and_then(|o| o.get("host_providers"))
        .and_then(Value::as_object)
        .ok_or_else(|| {
            format!(
                "RUNTIME_REGISTRY missing host_targets.host_providers; see {HOST_ADAPTER_CONTRACT_PATH}"
            )
        })?;

    for (host_id, entry) in manifest {
        let entry = entry.as_object().ok_or_else(|| {
            format!("host_providers.{host_id} must be an object")
        })?;
        let cargo_feature = entry
            .get("cargo_feature")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("host_providers.{host_id}.cargo_feature required"))?;
        let provider_module = entry
            .get("provider_module")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("host_providers.{host_id}.provider_module required"))?;

        if !cargo_toml_declares_feature(cargo_toml, cargo_feature) {
            return Err(format!(
                "host_providers.{host_id}: Cargo.toml [features] missing `{cargo_feature}`; \
                 add it manually (features are not generated from RUNTIME_REGISTRY.json)"
            ));
        }

        if !hosts_mod_has_cfg_provider(hosts_mod_rs, cargo_feature, provider_module) {
            return Err(format!(
                "host_providers.{host_id}: expected `#[cfg(feature = \"{cargo_feature}\")] mod {provider_module};` \
                 in core/runtime-core/src/hosts/mod.rs (registry provider_module / cargo_feature mismatch)"
            ));
        }

        if let Some(hooks_module) = entry.get("hooks_module").and_then(Value::as_str) {
            if !hosts_mod_declares_module(hosts_mod_rs, hooks_module) {
                return Err(format!(
                    "host_providers.{host_id}: expected `mod {hooks_module}` or `pub mod {hooks_module}` \
                     in core/runtime-core/src/hosts/mod.rs (registry hooks_module mismatch)"
                ));
            }
        }
    }
    Ok(())
}

fn cargo_toml_declares_feature(cargo_toml: &str, feature: &str) -> bool {
    let needle = format!("{feature} =");
    cargo_toml
        .lines()
        .any(|line| line.trim().starts_with(&needle))
}

fn hosts_mod_has_cfg_provider(hosts_mod_rs: &str, cargo_feature: &str, provider_module: &str) -> bool {
    let cfg_needle = format!(r#"cfg(feature = "{cargo_feature}")"#);
    let mod_needle = format!("mod {provider_module}");
    let lines: Vec<&str> = hosts_mod_rs.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        if !line.contains(&cfg_needle) {
            continue;
        }
        let window_end = (index + 3).min(lines.len());
        let window = lines[index..window_end].join("\n");
        if window.contains(&mod_needle) {
            return true;
        }
    }
    false
}

fn hosts_mod_declares_module(hosts_mod_rs: &str, module: &str) -> bool {
    let mod_needle = format!("mod {module}");
    hosts_mod_rs
        .lines()
        .any(|line| line.contains(&mod_needle))
}

pub fn host_entrypoints_value_for_id(
    registry: &Value,
    host_id: &str,
) -> Result<Value, String> {
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
    use super::*;
    use crate::runtime_registry::RUNTIME_REGISTRY_SCHEMA_VERSION;
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
                matches!(
                    tool.as_str(),
                    "codex" | "cursor" | "claude" | "opencode"
                ),
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
        assert_eq!(
            sorted,
            vec![
                "claude-code".to_string(),
                "codex".to_string(),
                "cursor".to_string(),
                "opencode".to_string(),
            ],
            "AGENTS.md / MIGRATION.md closed-set must match RUNTIME_REGISTRY.host_targets.supported"
        );
    }

    #[test]
    fn installable_pairs_exclude_retired_host_ids() {
        let root = repo_root();
        let pairs = installable_host_id_and_skills_install_tool_pairs(&root).expect("pairs");
        assert!(pairs.iter().any(|(host_id, _)| host_id == "cursor"));
        assert!(pairs.iter().any(|(host_id, _)| host_id == "claude-code"));
        assert!(pairs.iter().any(|(host_id, _)| host_id == "opencode"));
        assert!(pairs.iter().any(|(host_id, _)| host_id == "codex"));
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
    fn supported_hosts_each_have_provider() {
        let root = repo_root();
        let reg = load_runtime_registry_json(&root).expect("registry");
        let supported = host_targets_supported_host_ids(&reg).expect("supported");
        for host_id in supported {
            assert!(
                crate::hosts::host_provider_for_id(&host_id).is_some(),
                "RUNTIME_REGISTRY supported host `{host_id}` has no HostProvider registration"
            );
        }
    }

    #[test]
    fn host_provider_skeletons_align_with_supported_hosts() {
        let root = repo_root();
        let reg = load_runtime_registry_json(&root).expect("registry");
        validate_host_providers_against_registry(&reg).expect("HostProvider registry vs supported");
    }

    #[test]
    fn host_provider_mod_declarations_align_with_registry() {
        let root = repo_root();
        let reg = load_runtime_registry_json(&root).expect("registry");
        let hosts_mod = std::fs::read_to_string(root.join("core/runtime-core/src/hosts/mod.rs"))
            .expect("hosts/mod.rs");
        let cargo_toml = std::fs::read_to_string(root.join("core/runtime-core/Cargo.toml"))
            .expect("Cargo.toml");
        validate_host_provider_mod_declarations(&reg, &hosts_mod, &cargo_toml)
            .expect("host_providers vs hosts/mod.rs + Cargo.toml features");
    }

    #[test]
    fn host_provider_p4_metadata_aligns_with_host_projections() {
        let root = repo_root();
        let reg = load_runtime_registry_json(&root).expect("registry");
        let projections = reg
            .get("host_projections")
            .and_then(Value::as_object)
            .expect("host_projections");
        for host_id in ["cursor", "claude-code", "opencode", "codex"] {
            let projection = projections
                .get(host_id)
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("missing projection for {host_id}"));
            let provider = crate::hosts::host_provider_for_id(host_id)
                .unwrap_or_else(|| panic!("missing HostProvider for {host_id}"));
            assert_eq!(
                provider.profile_id(),
                projection
                    .get("profile_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
            );
            assert_eq!(
                provider.session_supervisor_driver(),
                projection
                    .get("session_supervisor_driver")
                    .and_then(Value::as_str)
                    .unwrap_or("unsupported")
            );
            assert_eq!(
                provider.capabilities().transport_type,
                projection
                    .get("transport")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
            );
            assert_eq!(
                provider.context_file(),
                projection
                    .get("context_files")
                    .and_then(Value::as_array)
                    .and_then(|arr| arr.first())
                    .and_then(Value::as_str)
                    .unwrap_or_default()
            );
            let registry_harness = projection
                .get("harness_capabilities")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let harness_exceptions: Vec<&str> = projection
                .get("harness_capability_exceptions")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter(|row| {
                            row.get("status").and_then(Value::as_str) == Some("unsupported")
                        })
                        .filter_map(|row| row.get("cap").and_then(Value::as_str))
                        .collect()
                })
                .unwrap_or_default();
            let effective_harness: Vec<&str> = registry_harness
                .iter()
                .copied()
                .filter(|cap| !harness_exceptions.contains(cap))
                .collect();
            assert_eq!(provider.harness_capabilities(), effective_harness.as_slice());
            let has_hard_gate = projection
                .get("capabilities")
                .and_then(Value::as_array)
                .map(|arr| arr.iter().any(|v| v.as_str() == Some("hard_gate_hooks")))
                .unwrap_or(false);
            assert_eq!(provider.has_hard_gate_hooks(), has_hard_gate);
            let closeout_supported = projection
                .get("harness_capabilities")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .any(|v| v.as_str() == Some("closeout_evidence_hooks"))
                })
                .unwrap_or(false)
                && !projection
                    .get("harness_capability_exceptions")
                    .and_then(Value::as_array)
                    .map(|arr| {
                        arr.iter().any(|row| {
                            row.get("cap").and_then(Value::as_str)
                                == Some("closeout_evidence_hooks")
                                && row.get("status").and_then(Value::as_str) == Some("unsupported")
                        })
                    })
                    .unwrap_or(false);
            assert_eq!(
                provider.closeout_evidence_hooks_supported(),
                closeout_supported
            );
            let review_observable = projection
                .get("harness_capabilities")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .any(|v| v.as_str() == Some("review_gate_router_observation"))
                })
                .unwrap_or(false)
                && !projection
                    .get("harness_capability_exceptions")
                    .and_then(Value::as_array)
                    .map(|arr| {
                        arr.iter().any(|row| {
                            row.get("cap").and_then(Value::as_str)
                                == Some("review_gate_router_observation")
                                && row.get("status").and_then(Value::as_str) == Some("unsupported")
                        })
                    })
                    .unwrap_or(false);
            assert_eq!(
                provider.review_gate_router_observable(),
                review_observable
            );
            assert_eq!(
                provider.requires_strict_pre_tool_fallback_default(),
                crate::framework_runtime::host_requires_strict_pre_tool_fallback(
                    host_id, &root, None
                )
                .unwrap_or_else(|e| panic!("{host_id}: {e}"))
            );
        }
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
        assert!(err.contains("new-host"), "{err}");
        assert!(err.contains(RUNTIME_REGISTRY_PATH), "{err}");
        assert!(err.contains(HOST_ADAPTER_CONTRACT_PATH), "{err}");
    }
}
