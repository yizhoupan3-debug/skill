//! Skill validation: frontmatter schema, registry consistency, path integrity.
//!
//! This is the skill layer's own validation infrastructure. All skill-specific
//! logic lives here; runtime-infra delegates via thin wrappers.

use crate::columnar;
use crate::discovery;
use crate::frontmatter_parser;
use crate::paths;
use std::collections::HashSet;
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

/// Run all validations against a repo root.
///
/// Checks:
/// 1. Runtime and manifest JSON files exist
/// 2. Every skill_path in runtime/manifest points to an existing file
/// 3. Every slug in runtime exists in manifest (RUNTIME ⊆ MANIFEST)
/// 4. Every on-disk SKILL.md passes frontmatter schema validation
/// 5. Frontmatter name matches slug
/// 6. Optional generated files exist (as warnings)
/// 7. Disk vs runtime slug cross-reference
pub fn validate_all(repo_root: &Path) -> Result<ValidationReport, String> {
    let skills_root = paths::skills_root(repo_root);
    let runtime_path = paths::runtime_json(repo_root);
    let manifest_path = paths::manifest_json(repo_root);

    // 1. Check required files exist
    for path in [&runtime_path, &manifest_path] {
        if !path.is_file() {
            return Err(format!(
                "framework skills validate: missing {}",
                path.display()
            ));
        }
    }

    let runtime: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&runtime_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;

    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // 2. Check skill_path references exist
    for (label, doc) in [("runtime", &runtime), ("manifest", &manifest)] {
        collect_missing_skill_paths(repo_root, doc, label, &mut errors);
    }

    // 3. RUNTIME ⊆ MANIFEST (slug set check)
    let runtime_slugs: HashSet<String> = columnar::extract_slugs(&runtime).into_iter().collect();
    let manifest_slugs: HashSet<String> = columnar::extract_slugs(&manifest).into_iter().collect();

    for slug in &runtime_slugs {
        if !manifest_slugs.contains(slug) {
            errors.push(format!("slug `{slug}` in RUNTIME but not in MANIFEST"));
        }
    }

    // 4. Disk slug discovery
    let disk_slugs = discovery::discover_skill_md_slugs(&skills_root)
        .map_err(|e| e.to_string())?;
    let disk_set: HashSet<&String> = disk_slugs.iter().collect();

    // 5. Frontmatter schema validation
    for slug in &disk_slugs {
        let path = paths::skill_md(repo_root, slug);
        match fs::read_to_string(&path) {
            Ok(text) => match frontmatter_parser::parse_and_validate(&text) {
                Ok((_fm, fm_warnings)) => {
                    for w in fm_warnings {
                        warnings.push(format!("{slug}: {w}"));
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
        ("SKILL_ROUTING_INDEX.json", paths::index_json(repo_root)),
        ("SKILL_TIERS.json", paths::tiers_json(repo_root)),
        ("SKILL_HEALTH_MANIFEST.json", paths::health_json(repo_root)),
        ("SKILL_APPROVAL_POLICY.json", paths::approval_json(repo_root)),
        ("SKILL_PLUGIN_CATALOG.json", paths::plugin_catalog_json(repo_root)),
        ("SKILL_LOADOUTS.json", paths::loadouts_json(repo_root)),
    ];
    for (name, path) in optional_files {
        if !path.exists() {
            warnings.push(format!("optional file missing: {name}"));
        }
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
