//! Skill refresh and validate — self-contained skill-layer CLI entry points.
//!
//! This module owns the complete skill lifecycle: validate, refresh,
//! generate health/approval, write tiers and companion stubs.
//! router-rs calls these functions directly; no runtime-infra middleman.

use crate::constants;
use crate::paths;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

// ---------------------------------------------------------------------------
// Public CLI types
// ---------------------------------------------------------------------------

/// CLI command parameters for `framework skills validate|refresh`.
#[derive(Debug, Clone)]
pub struct SkillsCommand {
    pub repo_root: std::path::PathBuf,
    pub write: bool,
    pub write_companions: bool,
}

/// Validation report from `validate_skills`.
pub struct ValidationReport {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub disk_count: usize,
    pub runtime_count: usize,
}

// ---------------------------------------------------------------------------
// validate
// ---------------------------------------------------------------------------

/// Run full skill validation: frontmatter schema, registry consistency,
/// path integrity, and print results.
pub fn validate_skills(repo_root: &Path) -> Result<(), String> {
    let report = crate::validate::validate_all(repo_root).map_err(|e| e.to_string())?;
    if report.errors.is_empty() {
        eprintln!(
            "framework skills validate: ok ({} on-disk SKILL.md, {} runtime rows, {} warnings)",
            report.disk_count,
            report.runtime_count,
            report.warnings.len()
        );
        for w in &report.warnings {
            eprintln!("  warning: {w}");
        }
        Ok(())
    } else {
        Err(report.errors.join("\n"))
    }
}

// ---------------------------------------------------------------------------
// refresh
// ---------------------------------------------------------------------------

/// Full refresh: write tiers, companion stubs, health manifest, approval policy.
pub fn refresh_skills(cmd: &SkillsCommand) -> Result<(), String> {
    if !cmd.write {
        return validate_skills(&cmd.repo_root);
    }
    write_skill_tiers_from_surface_policy(&cmd.repo_root)?;
    if cmd.write_companions {
        write_routing_companion_stubs(&cmd.repo_root)?;
        eprintln!(
            "framework skills refresh: wrote skills/SKILL_TIERS.json and routing companion stubs"
        );
    } else {
        eprintln!(
            "framework skills refresh: wrote skills/SKILL_TIERS.json only (companions unchanged; pass --write-companions to regenerate stubs)"
        );
    }
    generate_health_and_approval(&cmd.repo_root)?;
    validate_skills(&cmd.repo_root)
}

/// Generate health manifest and approval policy.
pub fn generate_health_and_approval(repo_root: &Path) -> Result<(), String> {
    crate::health::generate_health_manifest(repo_root).map_err(|e| e.to_string())?;
    crate::approval::generate_approval_policy(repo_root).map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// write_skill_tiers
// ---------------------------------------------------------------------------

fn write_skill_tiers_from_surface_policy(repo_root: &Path) -> Result<(), String> {
    let policy_path = paths::surface_policy_json(repo_root);
    let policy: Value =
        serde_json::from_str(&fs::read_to_string(&policy_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let activation_counts = policy
        .pointer("/skill_system/activation_counts")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let tier_counts = policy
        .pointer("/skill_system/tier_counts")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let out = json!({
        "schema_version": constants::SCHEMA_TIERS,
        "source_of_truth": false,
        "derived_from": "configs/framework/FRAMEWORK_SURFACE_POLICY.json",
        "report_status": "generated_debug_report",
        "summary": {
            "activation_counts": activation_counts,
            "tier_counts": tier_counts,
        }
    });
    let dest = paths::tiers_json(repo_root);
    core_state::utils::atomic_write::write_atomic_json(&dest, &out).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// write_routing_companion_stubs
// ---------------------------------------------------------------------------

fn write_routing_companion_stubs(repo_root: &Path) -> Result<(), String> {
    let manifest_path = paths::manifest_json(repo_root);
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let keys: Vec<String> = manifest["keys"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let path_idx = crate::columnar::key_index(&keys, "skill_path")
        .ok_or("manifest missing skill_path column")?;
    let host_idx = crate::columnar::key_index(&keys, "host_platforms");

    // Load all registered hosts from RUNTIME_REGISTRY
    let mut all_hosts: Vec<Value> = {
        let reg_path = paths::runtime_registry_json(repo_root);
        let reg_content = fs::read_to_string(&reg_path).unwrap_or_default();
        serde_json::from_str::<Value>(&reg_content)
            .ok()
            .and_then(|r| r.get("host_targets")?.get("supported")?.as_array().cloned())
            .unwrap_or_default()
    };
    all_hosts.sort_by(|a, b| a.as_str().unwrap_or("").cmp(b.as_str().unwrap_or("")));

    let mut plugin_skills = serde_json::Map::new();
    let mut metadata_skills = serde_json::Map::new();
    if let Some(rows) = manifest["skills"].as_array() {
        for row in rows {
            let slug = row.get(0).and_then(Value::as_str).unwrap_or("");
            let skill_path = row.get(path_idx).and_then(Value::as_str).unwrap_or("");
            let raw_platforms = host_idx
                .and_then(|i| row.get(i))
                .cloned()
                .unwrap_or_else(|| json!(["supported"]));
            let platforms = if raw_platforms
                .as_array()
                .is_some_and(|a| a.len() == 1 && a[0].as_str() == Some("supported"))
            {
                json!(all_hosts)
            } else {
                raw_platforms
            };
            plugin_skills.insert(
                slug.to_string(),
                json!({
                    "kind": "skill",
                    "skill_path": skill_path,
                    "host_support": { "platforms": platforms }
                }),
            );
            metadata_skills.insert(
                slug.to_string(),
                json!({
                    "selection_reason": "manifest row (router-rs framework skills refresh stub)"
                }),
            );
        }
    }
    let policy_path = paths::surface_policy_json(repo_root);
    let policy: Value =
        serde_json::from_str(&fs::read_to_string(&policy_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if let Some(hot) = policy["default_surface"]["hot_first_turn_owners"].as_array() {
        for slug in hot.iter().filter_map(|v| v.as_str()) {
            if let Some(entry) = metadata_skills.get_mut(slug)
                && let Some(obj) = entry.as_object_mut()
            {
                obj.insert(
                    "selection_reason".to_string(),
                    Value::String("allowlisted first-turn owner".to_string()),
                );
            }
        }
    }
    let stubs: [(&str, Value); 6] = [
        (
            paths::SKILL_PLUGIN_CATALOG_JSON,
            json!({
                "schema_version": constants::SCHEMA_PLUGIN_CATALOG,
                "source_of_truth": false,
                "derived_from": "skills/SKILL_MANIFEST.json",
                "skills": plugin_skills
            }),
        ),
        (
            paths::SKILL_ROUTING_METADATA_JSON,
            json!({
                "schema_version": constants::SCHEMA_METADATA,
                "source_of_truth": false,
                "skills": metadata_skills
            }),
        ),
        (
            paths::SKILL_ROUTING_RUNTIME_EXPLAIN_JSON,
            json!({
                "schema_version": constants::SCHEMA_EXPLAIN,
                "source_of_truth": false,
                "note": "stub; hot routing truth is skills/SKILL_ROUTING_RUNTIME.json"
            }),
        ),
        (
            paths::SKILL_HEALTH_MANIFEST_JSON,
            json!({
                "schema_version": constants::SCHEMA_HEALTH,
                "source_of_truth": false,
                "skills": {}
            }),
        ),
        (
            paths::SKILL_APPROVAL_POLICY_JSON,
            json!({
                "schema_version": constants::SCHEMA_APPROVAL,
                "source_of_truth": false,
                "derived_from": "skills/SKILL_MANIFEST.json",
                "policies": {}
            }),
        ),
        (paths::SKILL_ROUTING_INDEX_MD, json!(null)),
    ];
    for (rel, value) in stubs {
        let path = repo_root.join(rel);
        if rel.ends_with(".md") {
            let body = "# Generated routing index (stub)\n\nSee `skills/SKILL_ROUTING_RUNTIME.json` and `skills/SKILL_MANIFEST.json`. Maintained by `router-rs framework skills refresh --write`.\n";
            core_state::utils::atomic_write::write_atomic_text(&path, body)
                .map_err(|e| e.to_string())?;
        } else {
            core_state::utils::atomic_write::write_atomic_json(&path, &value)
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
