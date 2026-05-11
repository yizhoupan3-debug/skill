//! Host projection identifiers and `--to` tool names aligned with
//! `configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported`.

use serde_json::{json, Map, Value};
use std::path::Path;

const RUNTIME_REGISTRY_SCHEMA_VERSION: &str = "framework-runtime-registry-v1";
const RUNTIME_REGISTRY_PATH: &str = "configs/framework/RUNTIME_REGISTRY.json";
const HOST_ADAPTER_CONTRACT_PATH: &str = "docs/host_adapter_contract.md";

pub(crate) fn load_runtime_registry_json(framework_root: &Path) -> Result<Value, String> {
    let path = framework_root.join("configs/framework/RUNTIME_REGISTRY.json");
    if !path.is_file() {
        return Err(format!(
            "runtime registry not found under framework root {} (expected {})",
            framework_root.display(),
            path.display()
        ));
    }
    let payload = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let parsed: Value = serde_json::from_str(&payload).map_err(|e| {
        format!(
            "invalid JSON in {}: {e}; see {HOST_ADAPTER_CONTRACT_PATH}",
            path.display()
        )
    })?;
    let sv = parsed
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!(
                "RUNTIME_REGISTRY.json missing schema_version at {}",
                path.display()
            )
        })?;
    if sv != RUNTIME_REGISTRY_SCHEMA_VERSION {
        return Err(format!(
            "unsupported RUNTIME_REGISTRY schema_version {:?} at {}",
            sv,
            path.display()
        ));
    }
    Ok(parsed)
}

pub(crate) fn host_targets_supported_host_ids(registry: &Value) -> Result<Vec<String>, String> {
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
pub(crate) fn skills_install_tool_for_host_id(
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

pub(crate) fn projection_status_for_host_id(
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

pub(crate) fn host_is_installable(registry: &Value, host_id: &str) -> Result<bool, String> {
    let id = host_id.trim();
    host_target_metadata(registry, id)?
        .get("installable")
        .and_then(Value::as_bool)
        .ok_or_else(|| host_metadata_error(id, "installable"))
}

pub(crate) fn skills_install_tools_ordered(framework_root: &Path) -> Result<Vec<String>, String> {
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

pub(crate) fn installable_host_id_and_skills_install_tool_pairs(
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

pub(crate) fn host_id_and_skills_install_tool_pairs(
    framework_root: &Path,
) -> Result<Vec<(String, String)>, String> {
    let reg = load_runtime_registry_json(framework_root)?;
    host_id_and_skills_install_tool_pairs_from_registry(&reg)
}

pub(crate) fn host_id_and_skills_install_tool_pairs_from_registry(
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

pub(crate) fn sync_manifest_shared_system_block(repo_root: &Path) -> Result<Value, String> {
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
        "agent_policy_entrypoint": "AGENTS.md",
        "supported_hosts": supported_hosts,
        "host_entrypoints": Value::Object(host_entrypoints),
    }))
}

pub(crate) fn host_entrypoints_value_for_id(
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
        assert!(pairs.iter().any(|(host_id, _)| host_id == "codex-app"));
        let reg = load_runtime_registry_json(&root).expect("registry");
        for (host_id, tool) in pairs {
            assert!(
                matches!(tool.as_str(), "codex" | "cursor" | "claude"),
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
    fn installable_pairs_exclude_runtime_only_codex_app() {
        let root = repo_root();
        let pairs = installable_host_id_and_skills_install_tool_pairs(&root).expect("pairs");
        assert!(pairs.iter().any(|(host_id, _)| host_id == "codex-cli"));
        assert!(pairs.iter().any(|(host_id, _)| host_id == "cursor"));
        assert!(pairs.iter().any(|(host_id, _)| host_id == "claude-code"));
        assert!(!pairs.iter().any(|(host_id, _)| host_id == "codex-app"));
    }

    #[test]
    fn supported_host_without_metadata_fails_with_maintenance_hint() {
        let reg = json!({
            "schema_version": RUNTIME_REGISTRY_SCHEMA_VERSION,
            "host_targets": {
                "supported": ["codex-cli", "new-host"],
                "metadata": {
                    "codex-cli": {
                        "install_tool": "codex",
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
