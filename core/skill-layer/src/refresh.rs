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
    pub backfill: bool,
    pub dry_run: bool,
    pub generate: Option<String>,
}

// ---------------------------------------------------------------------------
// validate
// ---------------------------------------------------------------------------

/// Run full skill validation: frontmatter schema, registry consistency,
/// path integrity, and print results.
pub fn validate_skills(repo_root: &Path) -> Result<(), String> {
    let report = crate::validate::validate_all(repo_root).map_err(|e| e.to_string())?;
    if report.errors.is_empty() {
        tracing::info!(
            "framework skills validate: ok ({} on-disk SKILL.md, {} runtime rows, {} warnings)",
            report.disk_count,
            report.runtime_count,
            report.warnings.len()
        );
        for w in &report.warnings {
            tracing::warn!("  warning: {w}");
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
    if !cmd.write && !cmd.backfill && cmd.generate.is_none() {
        return validate_skills(&cmd.repo_root);
    }
    if cmd.backfill {
        let report = crate::backfill::backfill_registry(&cmd.repo_root, cmd.dry_run)?;
        if cmd.dry_run {
            tracing::info!(
                "framework skills backfill --dry-run: {}/{} skills w/ frontmatter, {} cells would be filled across {} columns",
                report.skills_with_frontmatter,
                report.total_skills,
                report.cells_filled,
                report.columns.len(),
            );
            for (col, count) in &report.columns {
                tracing::info!("  {col}: {count}");
            }
        } else {
            tracing::info!(
                "framework skills backfill: {}/{} skills w/ frontmatter, {} cells filled across {} columns",
                report.skills_with_frontmatter,
                report.total_skills,
                report.cells_filled,
                report.columns.len(),
            );
            for (col, count) in &report.columns {
                tracing::info!("  {col}: {count}");
            }
            if !report.errors.is_empty() {
                tracing::warn!("  {} errors (SKILL.md read/parse)", report.errors.len());
            }
        }
    }
    if let Some(ref generate_target) = cmd.generate {
        let slug = if generate_target == "all" { None } else { Some(generate_target.as_str()) };
        let report = crate::generate::generate_frontmatter(&cmd.repo_root, slug, cmd.dry_run)?;
        if cmd.dry_run {
            tracing::info!(
                "framework skills generate{slug_msg}: {}/{} generated, {}/{} skipped (--dry-run)",
                report.skills_generated,
                report.total_skills,
                report.skills_skipped,
                report.total_skills,
                slug_msg = slug.map(|s| format!(" --slug {s}")).unwrap_or_default(),
            );
        } else {
            tracing::info!(
                "framework skills generate{slug_msg}: {}/{} generated, {}/{} skipped",
                report.skills_generated,
                report.total_skills,
                report.skills_skipped,
                report.total_skills,
                slug_msg = slug.map(|s| format!(" --slug {s}")).unwrap_or_default(),
            );
            for err in &report.errors {
                tracing::warn!("  error: {err}");
            }
        }
    }
    if !cmd.write {
        return Ok(());
    }
    write_skill_tiers_from_surface_policy(&cmd.repo_root)?;
    generate_health_manifest(&cmd.repo_root)?;
    validate_skills(&cmd.repo_root)
}

/// Generate health manifest and approval policy.
pub fn generate_health_manifest(repo_root: &Path) -> Result<(), String> {
    crate::health::generate_health_manifest(repo_root).map_err(|e| e.to_string())
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
    core_state_utils::atomic_write::write_atomic_json(&dest, &out).map_err(|e| e.to_string())
}



// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
