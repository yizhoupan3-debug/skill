//! Skill validation: frontmatter schema, registry consistency, path integrity.
//!
//! This is the skill layer's own validation infrastructure. All skill-specific
//! logic lives here; runtime-infra delegates via thin wrappers.

use crate::discovery;
use crate::frontmatter::{RecordKind, RoutingGate, RoutingOwner};
use crate::frontmatter_parser;
use crate::generate::FRONTMATTER_KEYS;
use crate::paths;
use core_errors::FrameworkError;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Aggregated validation report.
pub struct ValidationReport {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub disk_count: usize,
    pub runtime_count: usize,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Validate a skill name/slug: must be non-empty, no path traversal, no path separators, no control characters.
pub fn validate_skill_name(name: &str) -> Result<(), FrameworkError> {
    if name.is_empty() {
        return Err(FrameworkError::Validation {
            message: "skill name must not be empty".into(),
        });
    }
    if name.chars().any(|c| c.is_control()) {
        return Err(FrameworkError::Validation {
            message: format!("skill name `{name}` must not contain control characters"),
        });
    }
    if name.contains("..") {
        return Err(FrameworkError::Validation {
            message: format!("skill name `{name}` must not contain '..' (path traversal)"),
        });
    }
    if name.contains('/') || name.contains('\\') {
        return Err(FrameworkError::Validation {
            message: format!("skill name `{name}` must not contain path separators"),
        });
    }
    if name.starts_with('~') || name.starts_with('/') {
        return Err(FrameworkError::Validation {
            message: format!("skill name `{name}` must not start with absolute path prefix"),
        });
    }
    Ok(())
}

/// Run all validations against a repo root.
///
/// Checks:
/// 1. Runtime JSON file exists
/// 2. Every skill_path in runtime points to an existing file
/// 3. Disk slug discovery
/// 4. Frontmatter schema validation (includes name-vs-slug consistency)
/// 5. Disk vs runtime slug cross-reference
/// 6. Optional generated files exist (as warnings)
/// 7. Frontmatter <--> registry consistency check
pub fn validate_all(repo_root: &Path) -> Result<ValidationReport, FrameworkError> {
    let skills_root = paths::skills_root(repo_root);
    let runtime_path = paths::runtime_json(repo_root);

    // 1. Check required files exist
    if !runtime_path.is_file() {
        return Err(FrameworkError::Validation {
            message: format!(
                "framework skills validate: missing {}",
                runtime_path.display()
            ),
        });
    }

    let runtime: serde_json::Value = serde_json::from_str(&fs::read_to_string(&runtime_path)?)?;

    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // 2. Check skill_path references exist
    collect_missing_skill_paths(repo_root, &runtime, "runtime", &mut errors);

    // 4. Disk slug discovery + cross-reference
    let disk_slugs = discovery::discover_skill_md_slugs(&skills_root)?;
    let disk_set: HashSet<&String> = disk_slugs.iter().collect();
    let runtime_slugs: HashSet<String> = runtime["skills"]
        .as_array()
        .map(|rows| {
            let key_names: Vec<String> = runtime["keys"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let slug_idx = key_names.iter().position(|k| k == "slug");
            rows.iter()
                .filter_map(|row| {
                    slug_idx
                        .and_then(|i| row.get(i))
                        .and_then(|v| v.as_str())
                        .map(String::from)
                })
                .collect()
        })
        .unwrap_or_default();

    // 4. Frontmatter schema validation
    const MAX_SKILL_MD_SIZE: u64 = 10 * 1024 * 1024; // 10 MiB
    // Build frontmatter map to avoid double I/O (used by check_frontmatter_vs_registry)
    let mut frontmatter_map: HashMap<String, crate::frontmatter::SkillFrontmatter> = HashMap::new();
    let mut raw_skill_md_text: HashMap<String, String> = HashMap::new();
    for slug in &disk_slugs {
        let path = paths::skill_md(repo_root, slug);
        // SL-9: pre-check file size to prevent OOM
        match fs::metadata(&path) {
            Ok(meta) if meta.len() > MAX_SKILL_MD_SIZE => {
                errors.push(format!(
                    "{slug}: SKILL.md exceeds size limit ({} bytes, max {MAX_SKILL_MD_SIZE})",
                    meta.len()
                ));
                continue;
            }
            _ => {}
        }
        match fs::read_to_string(&path) {
            Ok(text) => {
                raw_skill_md_text.insert(slug.clone(), text.clone());
                match frontmatter_parser::parse_and_validate(&text) {
                    Ok((fm, fm_warnings)) => {
                        frontmatter_map.insert(slug.clone(), fm.clone());
                        for w in fm_warnings {
                            warnings.push(format!("{slug}: {w}"));
                        }
                        // Framework_command conventions
                        if fm.kind == Some(RecordKind::FrameworkCommand) {
                            if fm.routing_gate != RoutingGate::None {
                                errors.push(format!(
                                    "{slug}: framework_command must have `routing_gate: none`, got `{:?}`",
                                    fm.routing_gate
                                ));
                            }
                            if fm.routing_owner != RoutingOwner::Owner {
                                errors.push(format!(
                                    "{slug}: framework_command must have `routing_owner: owner`, got `{:?}`",
                                    fm.routing_owner
                                ));
                            }
                            if !fm.trigger_hints.iter().any(|h| h.starts_with('/')) {
                                warnings.push(format!(
                                    "{slug}: framework_command should have at least one `/`-prefixed trigger_hint"
                                ));
                            }
                        }
                        // SL-4: name-vs-slug consistency (check 5)
                        if fm.name != slug.as_str() {
                            errors.push(format!(
                                "{slug}: frontmatter name `{}` does not match directory slug",
                                fm.name
                            ));
                        }
                    }
                    Err(e) => {
                        errors.push(format!("frontmatter: {slug}: {e}"));
                    }
                }
            }
            Err(e) => {
                errors.push(format!("frontmatter: {slug}: cannot read SKILL.md: {e}"));
            }
        }
    }

    // 6. Disk vs runtime cross-reference
    for slug in &runtime_slugs {
        if !disk_set.contains(slug) {
            warnings.push(format!("slug `{slug}` in RUNTIME but no SKILL.md on disk"));
        }
    }
    for slug in &disk_slugs {
        if !runtime_slugs.contains(slug) {
            warnings.push(format!("on-disk SKILL.md `{slug}` not in RUNTIME registry"));
        }
    }

    // 7. Check optional generated files exist (as warnings)
    let optional_files = [
        ("SKILL_LOADOUTS.json", paths::loadouts_json(repo_root)),
    ];
    for (name, path) in optional_files {
        if !path.exists() {
            warnings.push(format!("optional file missing: {name}"));
        }
    }

    // 8. Frontmatter ←→ registry consistency check (using pre-parsed frontmatter)
    if let Err(e) = check_frontmatter_vs_registry(
        repo_root,
        &frontmatter_map,
        &raw_skill_md_text,
        &mut errors,
        &mut warnings,
    ) {
        warnings.push(format!("registry consistency check: {e}"));
    }

    Ok(ValidationReport {
        errors,
        warnings,
        disk_count: disk_slugs.len(),
        runtime_count: runtime_slugs.len(),
    })
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Check that every skill_path in a registry document points to an existing file.
fn collect_missing_skill_paths(
    repo_root: &Path,
    doc: &serde_json::Value,
    label: &str,
    errors: &mut Vec<String>,
) {
    let Some(rows) = doc.get("skills").and_then(serde_json::Value::as_array) else {
        errors.push(format!("{label}: missing skills array"));
        return;
    };
    let keys: Vec<String> = doc["keys"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let path_idx = keys.iter().position(|k| k == "skill_path");

    for row in rows {
        let rel = path_idx.and_then(|i| row.get(i)).and_then(|v| v.as_str());
        if let Some(rel) = rel {
            // SL-1: reject absolute paths (Path::join replaces base for absolute paths)
            if std::path::Path::new(rel).is_absolute() {
                errors.push(format!("{label}: absolute skill_path not allowed: {rel}"));
                continue;
            }
            // Also reject ParentDir components (path traversal)
            if std::path::Path::new(rel)
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                errors.push(format!(
                    "{label}: skill_path '{rel}' contains '..' traversal"
                ));
                continue;
            }
            let full = repo_root.join(rel);
            if !full.is_file() {
                errors.push(format!("{label}: missing skill_path file {rel}"));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Registry consistency check (Phase 4)
// ---------------------------------------------------------------------------

/// Check that every SKILL.md frontmatter field matches the registry.
///
/// Uses pre-parsed frontmatter maps from validate_all to avoid double I/O.
///
/// After Phase 3 (generate), the registry is the source of truth for all
/// routing metadata.  This function detects:
///
/// 1. **Value mismatch** — a field exists in both but differs → error.
/// 2. **Missing field** — registry has a value but frontmatter omits it → error.
/// 3. **Orphan field** — a frontmatter key with no registry column → warning.
/// 4. **Null registry** — frontmatter has a value but registry is null → warning.
fn check_frontmatter_vs_registry(
    repo_root: &Path,
    frontmatter_map: &HashMap<String, crate::frontmatter::SkillFrontmatter>,
    raw_text_map: &HashMap<String, String>,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> Result<(), FrameworkError> {
    let runtime_path = paths::runtime_json(repo_root);
    let doc: Value = serde_json::from_str(&fs::read_to_string(&runtime_path)?)?;

    let keys: Vec<String> = doc["keys"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let col_idx: HashMap<&str, usize> = keys
        .iter()
        .enumerate()
        .map(|(i, k)| (k.as_str(), i))
        .collect();

    let slug_idx = match col_idx.get("slug") {
        Some(&i) => i,
        None => {
            return Err(FrameworkError::Validation {
                message: "runtime JSON missing slug column".into(),
            });
        }
    };

    let known_yaml_keys: HashSet<&str> = FRONTMATTER_KEYS.iter().map(|(_, y)| *y).collect();

    let Some(rows) = doc["skills"].as_array() else {
        return Ok(());
    };

    for row in rows {
        let Some(row_arr) = row.as_array() else {
            continue;
        };
        if slug_idx >= row_arr.len() {
            continue;
        }
        let Some(slug) = row_arr[slug_idx].as_str() else {
            continue;
        };

        // Use pre-parsed frontmatter (already validated by validate_all)
        // to avoid re-reading and re-parsing SKILL.md.
        let Some(fm) = frontmatter_map.get(slug) else {
            continue; // SKILL.md had parse errors — already reported
        };

        // Orphan detection: extract raw YAML keys from cached text
        if let Some(raw_text) = raw_text_map.get(slug) {
            if let Some(fm_block) = frontmatter_parser::extract_frontmatter_block(raw_text) {
                if let Ok(fm_value) = serde_yml::from_str::<Value>(fm_block) {
                    if let Some(fm_map) = fm_value.as_object() {
                        for yaml_key in fm_map.keys() {
                            if !known_yaml_keys.contains(yaml_key.as_str()) {
                                warnings.push(format!(
                                    "{slug}: orphan frontmatter field `{yaml_key}` not in registry schema"
                                ));
                            }
                        }
                    }
                }
            }
        }

        // 2. Field-by-field comparison using strong-typed frontmatter
        for &(registry_col, yaml_key) in FRONTMATTER_KEYS {
            if registry_col == "slug" {
                continue; // slug→name mapping is validated by frontmatter schema
            }

            let reg_val = col_idx.get(registry_col).and_then(|&idx| row_arr.get(idx));

            // Convert strong-typed frontmatter field to JSON Value for comparison
            let fm_val = frontmatter_field_to_value(fm, yaml_key);

            // Skip default scene comparison: both frontmatter default ("general")
            // and registry null are equivalent.
            if yaml_key == "scene" {
                if fm_val.is_none() || fm_val == Some(Value::String("general".into())) {
                    continue;
                }
            }

            match (reg_val, fm_val) {
                // Both non-null and different → value mismatch (trim whitespace for string fields)
                (Some(reg), Some(fm_v)) if !reg.is_null() && !values_equal(reg, &fm_v) => {
                    errors.push(format!(
                        "{slug}: registry `{registry_col}` ≠ frontmatter `{yaml_key}` — registry={reg}, frontmatter={fm_v}",
                    ));
                }
                // Registry has non-null, non-empty value, frontmatter missing
                (Some(reg), None) if !reg.is_null() && !is_empty_value(reg) => {
                    errors.push(format!(
                        "{slug}: frontmatter missing `{yaml_key}` (registry has `{registry_col}`={reg})",
                    ));
                }
                // Registry is null/None, frontmatter has value → run backfill
                (Some(reg), Some(fm_v)) if reg.is_null() && !is_empty_value(&fm_v) => {
                    warnings.push(format!(
                        "{slug}: frontmatter `{yaml_key}` populated but registry `{registry_col}` is null — run `backfill`",
                    ));
                }
                _ => {} // both absent, or null↔null ↔ fine
            }
        }
    }

    Ok(())
}

/// Convert a SkillFrontmatter field to its registry JSON Value representation.
fn frontmatter_field_to_value(
    fm: &crate::frontmatter::SkillFrontmatter,
    yaml_key: &str,
) -> Option<Value> {
    match yaml_key {
        "name" => Some(Value::String(fm.name.clone())),
        "description" => Some(Value::String(fm.description.clone())),
        "routing_layer" => Some(Value::String(fm.layer_str().to_string())),
        "routing_owner" => Some(Value::String(
            format!("{:?}", fm.routing_owner).to_lowercase(),
        )),
        "routing_gate" => Some(Value::String(
            format!("{:?}", fm.routing_gate).to_lowercase(),
        )),
        "routing_priority" => Some(Value::String(format!("{:?}", fm.routing_priority))),
        "session_start" => match fm.session_start {
            crate::frontmatter::SessionStart::NA => Some(Value::String("n/a".into())),
            _ => Some(Value::String(
                format!("{:?}", fm.session_start).to_lowercase(),
            )),
        },
        "trigger_hints" => Some(Value::Array(
            fm.trigger_hints
                .iter()
                .map(|s| Value::String(s.clone()))
                .collect(),
        )),
        "short_description" => fm
            .short_description
            .as_ref()
            .map(|s| Value::String(s.clone())),
        "metadata" => fm.metadata.clone(),
        "kind" => fm.kind.map(|k| Value::String(k.as_str().to_string())),
        "scene" => fm.scene.as_ref().map(|s| Value::String(s.clone())),
        "sub_scene" => fm.sub_scene.as_ref().map(|s| Value::String(s.clone())),
        "when_to_use" => fm.when_to_use.as_ref().map(|s| Value::String(s.clone())),
        "do_not_use" => fm.do_not_use.as_ref().map(|s| Value::String(s.clone())),
        _ => None,
    }
}

/// Return true if a JSON value is null/empty/zero-length.
fn is_empty_value(v: &Value) -> bool {
    matches!(v, Value::Null)
        || v.as_str().map(|s| s.trim().is_empty()).unwrap_or(false)
        || v.as_object().map(|o| o.is_empty()).unwrap_or(false)
        || v.as_array().map(|a| a.is_empty()).unwrap_or(false)
}

/// Compare two JSON values, treating string values as equal when whitespace-normalized.
/// Handles YAML block scalar (`|`) internal newlines vs registry single-line strings.
fn values_equal(a: &Value, b: &Value) -> bool {
    if a == b {
        return true;
    }
    match (a.as_str(), b.as_str()) {
        (Some(sa), Some(sb)) => normalize_ws(sa) == normalize_ws(sb),
        _ => false,
    }
}

/// Collapse all whitespace sequences to single space, trim, for string comparison.
fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
