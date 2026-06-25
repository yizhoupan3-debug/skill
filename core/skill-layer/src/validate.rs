//! Skill validation: frontmatter schema, registry consistency, path integrity.
//!
//! This is the skill layer's own validation infrastructure. All skill-specific
//! logic lives here; runtime-infra delegates via thin wrappers.

use crate::columnar;
use crate::discovery;
use crate::frontmatter::{RecordKind, RoutingGate, RoutingOwner};
use crate::frontmatter_parser;
use crate::generate::FRONTMATTER_KEYS;
use crate::paths;
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
pub fn validate_skill_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("skill name must not be empty".into());
    }
    if name.chars().any(|c| c.is_control()) {
        return Err(format!("skill name `{name}` must not contain control characters"));
    }
    if name.contains("..") {
        return Err(format!("skill name `{name}` must not contain '..' (path traversal)"));
    }
    if name.contains('/') || name.contains('\\') {
        return Err(format!("skill name `{name}` must not contain path separators"));
    }
    if name.starts_with('~') || name.starts_with('/') {
        return Err(format!("skill name `{name}` must not start with absolute path prefix"));
    }
    Ok(())
}

/// Run all validations against a repo root.
///
/// Checks:
/// 1. Runtime JSON file exists
/// 2. Every skill_path in runtime points to an existing file
/// 3. Every on-disk SKILL.md passes frontmatter schema validation
/// 4. Frontmatter name matches slug
/// 5. Optional generated files exist (as warnings)
/// 6. Disk vs runtime slug cross-reference
pub fn validate_all(repo_root: &Path) -> Result<ValidationReport, String> {
    let skills_root = paths::skills_root(repo_root);
    let runtime_path = paths::runtime_json(repo_root);

    // 1. Check required files exist
    if !runtime_path.is_file() {
        return Err(format!(
            "framework skills validate: missing {}",
            runtime_path.display()
        ));
    }

    let runtime: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&runtime_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;

    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // 2. Check skill_path references exist
    collect_missing_skill_paths(repo_root, &runtime, "runtime", &mut errors);

    // 4. Disk slug discovery + cross-reference
    let disk_slugs = discovery::discover_skill_md_slugs(&skills_root)
        .map_err(|e| e.to_string())?;
    let disk_set: HashSet<&String> = disk_slugs.iter().collect();
    let runtime_slugs: HashSet<String> = columnar::extract_slugs(&runtime).into_iter().collect();

    // 5. Frontmatter schema validation
    for slug in &disk_slugs {
        let path = paths::skill_md(repo_root, slug);
        match fs::read_to_string(&path) {
            Ok(text) => match frontmatter_parser::parse_and_validate(&text) {
                Ok((fm, fm_warnings)) => {
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
                }
                Err(e) => {
                    errors.push(format!("frontmatter: {slug}: {e}"));
                }
            },
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
        ("SKILL_TIERS.json", paths::tiers_json(repo_root)),
        ("SKILL_HEALTH_MANIFEST.json", paths::health_json(repo_root)),
        ("SKILL_LOADOUTS.json", paths::loadouts_json(repo_root)),
    ];
    for (name, path) in optional_files {
        if !path.exists() {
            warnings.push(format!("optional file missing: {name}"));
        }
    }

    // 8. Frontmatter ←→ registry consistency check
    if let Err(e) = check_frontmatter_vs_registry(repo_root, &mut errors, &mut warnings) {
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
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let path_idx = crate::columnar::key_index(&keys, "skill_path");

    for row in rows {
        let rel = path_idx
            .and_then(|i| row.get(i))
            .and_then(|v| v.as_str());
        if let Some(rel) = rel {
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
/// After Phase 3 (generate), the registry is the source of truth for all
/// routing metadata.  This function detects:
///
/// 1. **Value mismatch** — a field exists in both but differs → error.
/// 2. **Missing field** — registry has a value but frontmatter omits it → error.
/// 3. **Orphan field** — a frontmatter key with no registry column → warning.
/// 4. **Null registry** — frontmatter has a value but registry is null → warning.
fn check_frontmatter_vs_registry(
    repo_root: &Path,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let runtime_path = paths::runtime_json(repo_root);
    let doc: Value =
        serde_json::from_str(&fs::read_to_string(&runtime_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;

    let keys: Vec<String> = doc["keys"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let col_idx: HashMap<&str, usize> = keys
        .iter()
        .enumerate()
        .map(|(i, k)| (k.as_str(), i))
        .collect();

    let slug_idx = match col_idx.get("slug") {
        Some(&i) => i,
        None => return Err("runtime JSON missing slug column".to_string()),
    };

    let known_yaml_keys: HashSet<&str> =
        FRONTMATTER_KEYS.iter().map(|(_, y)| *y).collect();

    let Some(rows) = doc["skills"].as_array() else {
        return Ok(());
    };

    for row in rows {
        let Some(row_arr) = row.as_array() else { continue; };
        if slug_idx >= row_arr.len() { continue; }
        let Some(slug) = row_arr[slug_idx].as_str() else { continue; };

        let skill_md_path = paths::skill_md(repo_root, slug);
        let text = match fs::read_to_string(&skill_md_path) {
            Ok(t) => t,
            Err(_) => continue,
        };

        let fm_block = match frontmatter_parser::extract_frontmatter_block(&text) {
            Some(b) => b,
            None => continue,
        };

        let fm_value: Value = match serde_yml::from_str(fm_block) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let Some(fm_map) = fm_value.as_object() else { continue; };

        // 1. Orphan detection — frontmatter field has no registry column
        for yaml_key in fm_map.keys() {
            if !known_yaml_keys.contains(yaml_key.as_str()) {
                warnings.push(format!(
                    "{slug}: orphan frontmatter field `{yaml_key}` not in registry schema"
                ));
            }
        }

        // 2. Field-by-field comparison
        for &(registry_col, yaml_key) in FRONTMATTER_KEYS {
            if registry_col == "slug" {
                continue; // slug→name mapping is validated by frontmatter schema
            }

            let reg_val = col_idx.get(registry_col).and_then(|&idx| row_arr.get(idx));
            let fm_val = fm_map.get(yaml_key);

            match (reg_val, fm_val) {
                // Both non-null and different → value mismatch
                (Some(reg), Some(fm)) if !reg.is_null() && reg != fm => {
                    errors.push(format!(
                        "{slug}: registry `{registry_col}` ≠ frontmatter `{yaml_key}` — registry={reg}, frontmatter={fm}",
                    ));
                }
                // Registry has non-null, non-empty value, frontmatter missing
                (Some(reg), None)
                    if !reg.is_null() && !is_empty_value(reg) =>
                {
                    errors.push(format!(
                        "{slug}: frontmatter missing `{yaml_key}` (registry has `{registry_col}`={reg})",
                    ));
                }
                // Registry is null/None, frontmatter has value → run backfill
                (Some(reg), Some(fm)) if reg.is_null() && !is_empty_value(fm) => {
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

/// Return true if a JSON value is null/empty/zero-length.
fn is_empty_value(v: &Value) -> bool {
    v.is_null()
        || (v.is_string() && v.as_str().unwrap_or("").is_empty())
        || (v.is_array() && v.as_array().is_none_or(|a| a.is_empty()))
        || (v.is_object() && v.as_object().is_none_or(|o| o.is_empty()))
}
