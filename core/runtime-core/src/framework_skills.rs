//! `router-rs framework skills validate|refresh` — replaces retired `skill-compiler-rs`.

use serde_json::{json, Value};
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SkillsCommand {
    pub repo_root: PathBuf,
    pub write: bool,
    /// When true with `write`, also emit minimal companion stubs (not policy-complete).
    pub write_companions: bool,
}

pub fn validate_skills(repo_root: &Path) -> Result<(), String> {
    let skills_root = repo_root.join("skills");
    let runtime_path = skills_root.join("SKILL_ROUTING_RUNTIME.json");
    let manifest_path = skills_root.join("SKILL_MANIFEST.json");
    for path in [&runtime_path, &manifest_path] {
        if !path.is_file() {
            return Err(format!(
                "framework skills validate: missing {}",
                path.display()
            ));
        }
    }
    let runtime: Value =
        serde_json::from_str(&fs::read_to_string(&runtime_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;

    let mut errors = Vec::new();
    for (label, doc) in [("runtime", &runtime), ("manifest", &manifest)] {
        collect_missing_skill_paths(repo_root, doc, label, &mut errors);
    }

    let disk_slugs = discover_skill_md_slugs(&skills_root)?;
    let runtime_slugs = skill_slugs_from_index(&runtime)?;
    for slug in [
        "discussx",
        "planx",
        "implementx",
        "verifyx",
    ] {
        if !runtime_slugs.contains(slug) {
            errors.push(format!(
                "runtime missing expected my-lifecycle framework_command slug: {slug}"
            ));
        }
    }

    let manifest_slugs = skill_slugs_from_index(&manifest)?;
    for slug in ["discussx", "planx", "implementx", "verifyx"] {
        if !manifest_slugs.contains(slug) {
            errors.push(format!(
                "manifest missing My lifecycle slug {slug}; edit skills/SKILL_MANIFEST.json then framework skills refresh --write --write-companions"
            ));
        }
    }

    if errors.is_empty() {
        eprintln!(
            "framework skills validate: ok ({} on-disk SKILL.md, {} runtime rows)",
            disk_slugs.len(),
            runtime_slugs.len()
        );
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

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
        eprintln!("framework skills refresh: wrote skills/SKILL_TIERS.json only (companions unchanged; pass --write-companions to regenerate stubs)");
    }
    validate_skills(&cmd.repo_root)
}

fn write_routing_companion_stubs(repo_root: &Path) -> Result<(), String> {
    let manifest_path = repo_root.join("skills/SKILL_MANIFEST.json");
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let keys: Vec<&str> = manifest["keys"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    let path_idx = keys.iter().position(|k| *k == "skill_path").unwrap_or(7);
    let host_idx = keys.iter().position(|k| *k == "host_platforms");

    // Load all registered hosts from RUNTIME_REGISTRY so [supported] expands correctly.
    let mut all_hosts: Vec<Value> = {
        let reg_path = repo_root.join("configs/framework/RUNTIME_REGISTRY.json");
        serde_json::from_str::<Value>(&fs::read_to_string(&reg_path).unwrap_or_default())
            .ok()
            .and_then(|r| r.get("host_targets")?.get("supported")?.as_array().cloned())
            .unwrap_or_else(|| json!(["claude-code", "codex", "cursor", "opencode"]).as_array().cloned().unwrap())
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
            // Expand [supported] wildcard to all registered hosts.
            let platforms = if raw_platforms.as_array().is_some_and(|a| {
                a.len() == 1 && a[0].as_str() == Some("supported")
            }) {
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
    let policy_path = repo_root.join("configs/framework/FRAMEWORK_SURFACE_POLICY.json");
    let policy: Value =
        serde_json::from_str(&fs::read_to_string(&policy_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if let Some(hot) = policy["default_surface"]["hot_first_turn_owners"].as_array() {
        for slug in hot.iter().filter_map(|v| v.as_str()) {
            if let Some(entry) = metadata_skills.get_mut(slug) {
                if let Some(obj) = entry.as_object_mut() {
                    obj.insert(
                        "selection_reason".to_string(),
                        Value::String("allowlisted first-turn owner".to_string()),
                    );
                }
            }
        }
    }
    let stubs: [(&str, Value); 6] = [
        (
            "skills/SKILL_PLUGIN_CATALOG.json",
            json!({
                "schema_version": "skill-plugin-catalog-v1",
                "source_of_truth": false,
                "derived_from": "skills/SKILL_MANIFEST.json",
                "skills": plugin_skills
            }),
        ),
        (
            "skills/SKILL_ROUTING_METADATA.json",
            json!({
                "schema_version": "skill-routing-metadata-v1",
                "source_of_truth": false,
                "skills": metadata_skills
            }),
        ),
        (
            "skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json",
            json!({
                "schema_version": "skill-routing-runtime-explain-v1",
                "source_of_truth": false,
                "note": "stub; hot routing truth is skills/SKILL_ROUTING_RUNTIME.json"
            }),
        ),
        (
            "skills/SKILL_HEALTH_MANIFEST.json",
            json!({
                "schema_version": "skill-health-manifest-v1",
                "source_of_truth": false,
                "skills": {}
            }),
        ),
        (
            "skills/SKILL_APPROVAL_POLICY.json",
            json!({
                "schema_version": "skill-approval-policy-v1",
                "source_of_truth": false,
                "derived_from": "skills/SKILL_MANIFEST.json",
                "policies": {}
            }),
        ),
        ("skills/SKILL_ROUTING_INDEX.md", json!(null)),
    ];
    for (rel, value) in stubs {
        let path = repo_root.join(rel);
        if rel.ends_with(".md") {
            let body = "# Generated routing index (stub)\n\nSee `skills/SKILL_ROUTING_RUNTIME.json` and `skills/SKILL_MANIFEST.json`. Maintained by `router-rs framework skills refresh --write`.\n";
            crate::atomic_write::write_atomic_text(&path, body).map_err(|e| e.to_string())?;
        } else {
            crate::atomic_write::write_atomic_json(&path, &value).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn write_skill_tiers_from_surface_policy(repo_root: &Path) -> Result<(), String> {
    let policy_path = repo_root.join("configs/framework/FRAMEWORK_SURFACE_POLICY.json");
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
        "schema_version": "skill-tiers-v1",
        "source_of_truth": false,
        "derived_from": "configs/framework/FRAMEWORK_SURFACE_POLICY.json",
        "report_status": "generated_debug_report",
        "summary": {
            "activation_counts": activation_counts,
            "tier_counts": tier_counts,
        }
    });
    let dest = repo_root.join("skills/SKILL_TIERS.json");
    crate::atomic_write::write_atomic_json(&dest, &out).map_err(|e| e.to_string())
}

fn collect_missing_skill_paths(
    repo_root: &Path,
    doc: &Value,
    label: &str,
    errors: &mut Vec<String>,
) {
    let Some(rows) = doc.get("skills").and_then(Value::as_array) else {
        errors.push(format!("{label}: missing skills array"));
        return;
    };
    let keys = doc
        .get("keys")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    let path_idx = keys.iter().position(|k| *k == "skill_path");
    for row in rows {
        let path = if let Some(idx) = path_idx {
            row.get(idx).and_then(Value::as_str)
        } else {
            row.get(7).and_then(Value::as_str)
        };
        if let Some(rel) = path {
            let full = repo_root.join(rel);
            if !full.is_file() {
                errors.push(format!("{label}: missing skill_path file {rel}"));
            }
        }
    }
}

fn skill_slugs_from_index(doc: &Value) -> Result<HashSet<String>, String> {
    let rows = doc
        .get("skills")
        .and_then(Value::as_array)
        .ok_or_else(|| "skills index missing skills array".to_string())?;
    Ok(rows
        .iter()
        .filter_map(|row| row.get(0).and_then(Value::as_str).map(str::to_string))
        .collect())
}

fn discover_skill_md_slugs(skills_root: &Path) -> Result<BTreeSet<String>, String> {
    let mut slugs = BTreeSet::new();
    walk_skill_md(skills_root, skills_root, &mut slugs)?;
    Ok(slugs)
}

fn walk_skill_md(base: &Path, dir: &Path, slugs: &mut BTreeSet<String>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                if name.starts_with('.') {
                    continue;
                }
            }
            walk_skill_md(base, &path, slugs)?;
        } else if path.file_name().and_then(|s| s.to_str()) == Some("SKILL.md") {
            if let Some(name) = parse_skill_name(&path)? {
                slugs.insert(name);
            }
        }
    }
    Ok(())
}

fn parse_skill_name(path: &Path) -> Result<Option<String>, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    if !text.starts_with("---") {
        return Ok(None);
    }
    let end = text[3..].find("\n---").map(|i| i + 3);
    let Some(end) = end else {
        return Ok(None);
    };
    let front = &text[3..end];
    for line in front.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("name:") {
            return Ok(Some(rest.trim().to_string()));
        }
    }
    Ok(None)
}
