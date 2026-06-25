//! Registry → SKILL.md frontmatter generation.
//!
//! Reads the registry (SKILL_ROUTING_RUNTIME.json) and regenerates the YAML
//! frontmatter block for each (or a specific) SKILL.md. The body of the file
//! (everything after the closing `---`) is preserved unchanged.
//!
//! This is the reverse of the backfill operation: backfill reads SKILL.md →
//! registry, generate reads registry → SKILL.md.

use crate::frontmatter_parser;
use crate::paths;
use core_state_utils::atomic_write::write_atomic_text;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

/// Result of a generation operation.
pub struct GenerateReport {
    pub total_skills: usize,
    pub skills_generated: usize,
    pub skills_skipped: usize,
    pub errors: Vec<String>,
    pub dry_run: bool,
}

// ---------------------------------------------------------------------------
// Registry → frontmatter key mapping
// ---------------------------------------------------------------------------

/// (registry_column, frontmatter_yaml_key)
/// Only columns that should appear in SKILL.md frontmatter are listed.
pub const FRONTMATTER_KEYS: &[(&str, &str)] = &[
    ("slug", "name"),
    ("description", "description"),
    ("layer", "routing_layer"),
    ("owner", "routing_owner"),
    ("gate", "routing_gate"),
    ("priority", "routing_priority"),
    ("session_start", "session_start"),
    ("trigger_hints", "trigger_hints"),
    ("short_description", "short_description"),
    ("source", "source"),
    ("risk", "risk"),
    ("metadata", "metadata"),
    ("allowed_tools", "allowed_tools"),
    ("runtime_requirements", "runtime_requirements"),
    ("network_access", "network_access"),
    ("approval_required_tools", "approval_required_tools"),
];

/// Columns whose value should be omitted from frontmatter when null or empty.
fn should_omit(v: &Value) -> bool {
    v.is_null()
        || (v.is_string() && v.as_str().unwrap_or("").trim().is_empty())
        || (v.is_array() && v.as_array().unwrap_or(&vec![]).is_empty())
        || (v.is_object() && v.as_object().is_some_and(|o| o.is_empty()))
}

// ---------------------------------------------------------------------------
// Frontmatter generation
// ---------------------------------------------------------------------------

/// Build the YAML frontmatter string for a registry row.
fn build_frontmatter_yaml(row: &[Value], col_idx: &HashMap<&str, usize>) -> Result<String, String> {
    let mut map = serde_json::Map::new();

    for &(registry_col, yaml_key) in FRONTMATTER_KEYS {
        let Some(&idx) = col_idx.get(registry_col) else { continue };
        if idx >= row.len() {
            continue;
        }
        let val = &row[idx];
        if should_omit(val) {
            continue;
        }
        map.insert(yaml_key.to_string(), val.clone());
    }

    // Special handling: always include trigger_hints (even if empty)
    if let Some(&idx) = col_idx.get("trigger_hints")
        && idx < row.len() {
            let hints = &row[idx];
            if should_omit(hints) {
                map.insert("trigger_hints".to_string(), Value::Array(vec![]));
            }
        }

    let yaml_text = serde_yml::to_string(&Value::Object(map))
        .map_err(|e| format!("YAML serialization failed: {e}"))?;

    Ok(yaml_text)
}

// ---------------------------------------------------------------------------
// Core generation logic
// ---------------------------------------------------------------------------

/// Generate frontmatter for all (or a specific) skills from the registry.
///
/// - `slug`: if set, only this skill is regenerated.
/// - `dry_run`: if true, only report what would change.
pub fn generate_frontmatter(
    repo_root: &Path,
    slug: Option<&str>,
    dry_run: bool,
) -> Result<GenerateReport, String> {
    let runtime_path = paths::runtime_json(repo_root);
    let doc: Value =
        serde_json::from_str(&fs::read_to_string(&runtime_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;

    let keys: Vec<String> = doc["keys"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .ok_or_else(|| "runtime JSON missing keys array".to_string())?;

    let col_idx: HashMap<&str, usize> = keys
        .iter()
        .enumerate()
        .map(|(i, k)| (k.as_str(), i))
        .collect();
    let slug_idx = *col_idx
        .get("slug")
        .ok_or_else(|| "runtime JSON missing slug column".to_string())?;

    let total_skills = doc["skills"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);

    let mut report = GenerateReport {
        total_skills,
        skills_generated: 0,
        skills_skipped: 0,
        errors: Vec::new(),
        dry_run,
    };

    let Some(rows) = doc["skills"].as_array() else {
        return Err("runtime JSON missing skills array".to_string());
    };

    for row in rows {
        let Some(row_arr) = row.as_array() else {
            continue;
        };
        if slug_idx >= row_arr.len() {
            continue;
        }
        let current_slug = match row_arr[slug_idx].as_str() {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => continue,
        };

        // Filter by slug if specified
        if let Some(target) = slug
            && current_slug != target {
                continue;
            }

        let skill_md_path = paths::skill_md(repo_root, &current_slug);

        // Read existing SKILL.md
        let existing = match fs::read_to_string(&skill_md_path) {
            Ok(t) => t,
            Err(e) => {
                report.errors.push(format!("{current_slug}: cannot read SKILL.md: {e}"));
                continue;
            }
        };

        // Extract the body (everything after the closing ---)
        let body = frontmatter_parser::extract_body(&existing).unwrap_or(&existing);

        // Generate new frontmatter YAML
        let yaml = match build_frontmatter_yaml(row_arr, &col_idx) {
            Ok(y) => y,
            Err(e) => {
                report.errors.push(format!("{current_slug}: {e}"));
                continue;
            }
        };

        let new_content = format!("---\n{}---\n{}", yaml, body);

        // Check if anything changed
        if new_content == existing {
            report.skills_skipped += 1;
            continue;
        }

        // Write (if not dry-run)
        if !dry_run {
            write_atomic_text(&skill_md_path, &new_content)
                .map_err(|e| format!("{current_slug}: write failed: {e}"))?;
        }

        report.skills_generated += 1;
    }

    Ok(report)
}
