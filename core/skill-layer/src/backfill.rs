//! Registry backfill: populate null registry columns from SKILL.md frontmatter.
//!
//! This is the first stage of making the registry the single source of truth.
//! It reads each SKILL.md, extracts the frontmatter, and writes non-null values
//! into any null cells in the corresponding registry row. Existing values are
//! preserved (backfill is null-only).
//!
//! Columns that can be backfilled from SKILL.md frontmatter:
//!   short_description, metadata, risk, allowed_tools,
//!   runtime_requirements, network_access, approval_required_tools
//!
//! Columns NOT backfilled (no frontmatter source): tags, when_to_use, do_not_use

use crate::frontmatter_parser;
use crate::paths;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

/// Result of a backfill operation.
pub struct BackfillReport {
    pub total_skills: usize,
    pub skills_with_frontmatter: usize,
    pub cells_filled: usize,
    pub columns: HashMap<String, usize>,
    pub errors: Vec<String>,
    pub dry_run: bool,
}

// ---------------------------------------------------------------------------
// Mapping: frontmatter field → registry column name
// ---------------------------------------------------------------------------

/// Pairs of (frontmatter field name in SkillFrontmatter, registry column name).
/// Only columns that can be backfilled from SKILL.md frontmatter are listed.
const BACKFILLABLE_FIELDS: &[(&str, &str)] = &[
    ("short_description", "short_description"),
    ("metadata", "metadata"),
    ("risk", "risk"),
    ("allowed_tools", "allowed_tools"),
    ("runtime_requirements", "runtime_requirements"),
    ("network_access", "network_access"),
    ("approval_required_tools", "approval_required_tools"),
];

/// Returns true if a Value should be treated as non-null for backfill purposes.
fn is_non_null(v: &Value) -> bool {
    !v.is_null()
        && !(v.is_string() && v.as_str().unwrap_or("").trim().is_empty())
        && !(v.is_array() && v.as_array().unwrap_or(&vec![]).is_empty())
}

// ---------------------------------------------------------------------------
// Frontmatter → Value serialization
// ---------------------------------------------------------------------------

/// Convert a parsed frontmatter field to its registry JSON value.
fn frontmatter_field_to_value(
    fm: &crate::frontmatter::SkillFrontmatter,
    field: &str,
) -> Option<Value> {
    match field {
        "short_description" => fm.short_description.as_ref().map(|v| Value::String(v.clone())),
        "metadata" => fm.metadata.clone(),
        "risk" => fm.risk.as_ref().map(|v| Value::String(v.clone())),
        "allowed_tools" => fm
            .allowed_tools
            .as_ref()
            .map(|v| Value::Array(v.iter().map(|s| Value::String(s.clone())).collect())),
        "runtime_requirements" => fm.runtime_requirements.clone(),
        "network_access" => fm.network_access.as_ref().map(|v| Value::String(v.clone())),
        "approval_required_tools" => fm
            .approval_required_tools
            .as_ref()
            .map(|v| Value::Array(v.iter().map(|s| Value::String(s.clone())).collect())),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Core backfill logic
// ---------------------------------------------------------------------------

/// Run a null-only backfill from SKILL.md frontmatter into the registry JSON.
///
/// `dry_run=true` only scans and reports — no files are modified.
pub fn backfill_registry(repo_root: &Path, dry_run: bool) -> Result<BackfillReport, String> {
    let runtime_path = paths::runtime_json(repo_root);
    let mut doc: Value =
        serde_json::from_str(&fs::read_to_string(&runtime_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;

    let keys: Vec<String> = doc["keys"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .ok_or_else(|| "runtime JSON missing keys array".to_string())?;

    // Build column index lookup
    let col_idx: HashMap<&str, usize> = keys
        .iter()
        .enumerate()
        .map(|(i, k)| (k.as_str(), i))
        .collect();

    // Verify required columns exist
    let slug_idx = *col_idx
        .get("slug")
        .ok_or_else(|| "runtime JSON missing slug column".to_string())?;

    let mut report = BackfillReport {
        total_skills: 0,
        skills_with_frontmatter: 0,
        cells_filled: 0,
        columns: HashMap::new(),
        errors: Vec::new(),
        dry_run,
    };

    // Single pass: iterate rows, read frontmatter, fill null cells.
    if let Some(rows) = doc["skills"].as_array_mut() {
        report.total_skills = rows.len();

        for row in rows.iter_mut() {
            let Some(row_arr) = row.as_array_mut() else {
                report.errors.push("non-array row".to_string());
                continue;
            };
            if slug_idx >= row_arr.len() {
                continue;
            }
            let slug = match row_arr[slug_idx].as_str() {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => continue,
            };

            // Read and parse SKILL.md frontmatter
            let skill_md_path = paths::skill_md(repo_root, &slug);
            let fm_text = match fs::read_to_string(&skill_md_path) {
                Ok(t) => t,
                Err(e) => {
                    report.errors.push(format!("{slug}: cannot read SKILL.md: {e}"));
                    continue;
                }
            };
            let Ok((fm, _warnings)) = frontmatter_parser::parse_and_validate(&fm_text) else {
                report.errors.push(format!("{slug}: frontmatter parse failed"));
                continue;
            };
            report.skills_with_frontmatter += 1;

            // Backfill null cells from frontmatter
            for &(fm_field, registry_col) in BACKFILLABLE_FIELDS {
                let Some(&ci) = col_idx.get(registry_col) else {
                    continue;
                };
                if ci >= row_arr.len() {
                    continue;
                }
                if is_non_null(&row_arr[ci]) {
                    continue;
                }
                if let Some(val) = frontmatter_field_to_value(&fm, fm_field) {
                    row_arr[ci] = val;
                    report.cells_filled += 1;
                    *report.columns.entry(registry_col.to_string()).or_insert(0) += 1;
                }
            }
        }
    }

    // Write the updated JSON (if not dry-run and any changes were made)
    if !dry_run && report.cells_filled > 0 {
        core_state::utils::atomic_write::write_atomic_json(&runtime_path, &doc)
            .map_err(|e| format!("failed to write backfilled registry: {e}"))?;
    }

    Ok(report)
}
