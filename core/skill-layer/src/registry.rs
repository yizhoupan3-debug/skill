//! Unified skill registry: cross-source query and consistency validation.
//!
//! Loads data from SKILL_ROUTING_RUNTIME.json, SKILL_MANIFEST.json,
//! SKILL_ROUTING_INDEX.json, SKILL_TIERS.json, SKILL_HEALTH_MANIFEST.json,
//! and on-disk SKILL.md files, then provides a single query API.

use crate::columnar;
#[cfg(test)]
use crate::constants;
use crate::frontmatter::SkillFrontmatter;
use crate::frontmatter_parser;
use crate::paths;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Unified view of a skill across all registry sources.
#[derive(Debug, Clone)]
pub struct SkillRegistryEntry {
    pub slug: String,
    pub skill_path: Option<String>,
    pub layer: String,
    pub owner: String,
    pub gate: String,
    pub priority: String,
    pub description: String,
    pub session_start: String,
    pub trigger_hints: Vec<String>,
    pub kind: String,
    pub skill_flags: Vec<String>,
    pub frontmatter: Option<SkillFrontmatter>,
}

/// Consistency issue found during cross-source validation.
#[derive(Debug, Clone)]
pub struct ConsistencyIssue {
    pub severity: IssueSeverity,
    pub slug: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueSeverity {
    Error,
    Warning,
}

/// Registry-level errors.
#[derive(Debug)]
pub enum RegistryError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Json(e) => write!(f, "JSON error: {e}"),
        }
    }
}

impl std::error::Error for RegistryError {}

impl From<std::io::Error> for RegistryError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for RegistryError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Load columnar rows from a file, returning slug→row mapping.
fn load_rows(path: &Path) -> Result<HashMap<String, Vec<serde_json::Value>>, RegistryError> {
    let doc: serde_json::Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    Ok(columnar::load_columnar_rows(&doc))
}

// ---------------------------------------------------------------------------
// SkillRegistry
// ---------------------------------------------------------------------------

/// Unified skill registry.
pub struct SkillRegistry {
    pub repo_root: PathBuf,
}

impl SkillRegistry {
    pub fn new(repo_root: PathBuf) -> Self {
        Self { repo_root }
    }

    /// List all known skill slugs from the runtime registry.
    pub fn list_slugs(&self) -> Result<Vec<String>, RegistryError> {
        let runtime_path = paths::runtime_json(&self.repo_root);
        let rows = load_rows(&runtime_path)?;
        Ok(rows.into_keys().collect())
    }

    /// Get unified entry for a single skill by slug.
    pub fn get(&self, slug: &str) -> Result<Option<SkillRegistryEntry>, RegistryError> {
        let all = self.all()?;
        Ok(all.into_iter().find(|e| e.slug == slug))
    }

    /// Get all entries from the runtime registry.
    pub fn all(&self) -> Result<Vec<SkillRegistryEntry>, RegistryError> {
        let runtime_path = paths::runtime_json(&self.repo_root);
        let doc: serde_json::Value = serde_json::from_str(&fs::read_to_string(&runtime_path)?)?;
        let keys = columnar::parse_columnar_keys(&doc);

        let mut entries = Vec::new();
        if let Some(skills_arr) = doc["skills"].as_array() {
            for row in skills_arr {
                let row_vals: Vec<serde_json::Value> = row
                    .as_array()
                    .map(|a| a.to_vec())
                    .unwrap_or_default();
                let slug = columnar::col_string(&row_vals, &keys, "slug").unwrap_or_default();

                // Try to load frontmatter
                let fm_path = paths::skill_md(&self.repo_root, &slug);
                let frontmatter = if fm_path.exists() {
                    fs::read_to_string(&fm_path)
                        .ok()
                        .and_then(|text| frontmatter_parser::parse_frontmatter(&text).ok())
                } else {
                    None
                };

                entries.push(SkillRegistryEntry {
                    slug,
                    skill_path: columnar::col_string(&row_vals, &keys, "skill_path"),
                    layer: columnar::col_string(&row_vals, &keys, "layer").unwrap_or_default(),
                    owner: columnar::col_string(&row_vals, &keys, "owner").unwrap_or_default(),
                    gate: columnar::col_string(&row_vals, &keys, "gate").unwrap_or_default(),
                    priority: columnar::col_string(&row_vals, &keys, "priority").unwrap_or_default(),
                    description: columnar::col_string(&row_vals, &keys, "description").unwrap_or_default(),
                    session_start: columnar::col_string(&row_vals, &keys, "session_start").unwrap_or_default(),
                    trigger_hints: columnar::col_str_vec(&row_vals, &keys, "trigger_hints"),
                    kind: columnar::col_string(&row_vals, &keys, "kind").unwrap_or_else(|| "skill".into()),
                    skill_flags: columnar::col_str_vec(&row_vals, &keys, "skill_flags"),
                    frontmatter,
                });
            }
        }
        Ok(entries)
    }

    /// Cross-source consistency validation.
    pub fn validate_consistency(&self) -> Result<Vec<ConsistencyIssue>, RegistryError> {
        let mut issues = Vec::new();

        // 1. Load all sources
        let runtime_path = paths::runtime_json(&self.repo_root);
        let manifest_path = paths::manifest_json(&self.repo_root);
        let index_path = paths::index_json(&self.repo_root);

        if !runtime_path.exists() {
            issues.push(ConsistencyIssue {
                severity: IssueSeverity::Error,
                slug: None,
                message: "SKILL_ROUTING_RUNTIME.json not found".into(),
            });
            return Ok(issues);
        }

        let runtime_rows = load_rows(&runtime_path)?;
        let manifest_rows = if manifest_path.exists() {
            load_rows(&manifest_path)?
        } else {
            HashMap::new()
        };
        let index_rows = if index_path.exists() {
            load_rows(&index_path)?
        } else {
            HashMap::new()
        };

        let runtime_slugs: HashSet<&String> = runtime_rows.keys().collect();
        let manifest_slugs: HashSet<&String> = manifest_rows.keys().collect();
        let index_slugs: HashSet<&String> = index_rows.keys().collect();

        // 2. RUNTIME ⊆ MANIFEST
        for slug in &runtime_slugs {
            if !manifest_slugs.contains(*slug) {
                issues.push(ConsistencyIssue {
                    severity: IssueSeverity::Error,
                    slug: Some((**slug).clone()),
                    message: "in RUNTIME but not in MANIFEST".into(),
                });
            }
        }

        // 3. INDEX ⊆ RUNTIME
        for slug in &index_slugs {
            if !runtime_slugs.contains(*slug) {
                issues.push(ConsistencyIssue {
                    severity: IssueSeverity::Warning,
                    slug: Some((**slug).clone()),
                    message: "in INDEX but not in RUNTIME".into(),
                });
            }
        }

        // 4. Each slug has on-disk SKILL.md
        for slug in &runtime_slugs {
            let path = paths::skill_md(&self.repo_root, slug);
            if !path.exists() {
                issues.push(ConsistencyIssue {
                    severity: IssueSeverity::Error,
                    slug: Some((**slug).clone()),
                    message: "no SKILL.md on disk".into(),
                });
            }
        }

        // 5. Frontmatter name matches slug
        for slug in &runtime_slugs {
            let path = paths::skill_md(&self.repo_root, slug);
            if path.exists() {
                if let Ok(text) = fs::read_to_string(&path) {
                    if let Ok(fm) = frontmatter_parser::parse_frontmatter(&text) {
                        if fm.name != **slug {
                            issues.push(ConsistencyIssue {
                                severity: IssueSeverity::Warning,
                                slug: Some((**slug).clone()),
                                message: format!(
                                    "frontmatter name `{}` doesn't match slug `{}`",
                                    fm.name, slug
                                ),
                            });
                        }
                    }
                }
            }
        }

        Ok(issues)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_minimal_runtime(skills_root: &Path, slugs: &[&str]) {
        let keys = serde_json::json!(["slug", "layer", "owner", "gate"]);
        let skills: Vec<serde_json::Value> = slugs
            .iter()
            .map(|s| serde_json::json!([s, "L2", "owner", "none"]))
            .collect();
        let doc = serde_json::json!({
            "schema_version": constants::SCHEMA_RUNTIME,
            "keys": keys,
            "skills": skills
        });
        fs::write(
            skills_root.join("SKILL_ROUTING_RUNTIME.json"),
            serde_json::to_string_pretty(&doc).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn list_slugs_reads_runtime() {
        let tmp = tempfile::tempdir().unwrap();
        let skills_root = tmp.path().join("skills");
        fs::create_dir_all(&skills_root).unwrap();
        create_minimal_runtime(&skills_root, &["alpha", "beta"]);

        let registry = SkillRegistry::new(tmp.path().into());
        let slugs = registry.list_slugs().unwrap();
        assert!(slugs.contains(&"alpha".to_string()));
        assert!(slugs.contains(&"beta".to_string()));
    }

    #[test]
    fn validate_consistency_catches_missing_skill_md() {
        let tmp = tempfile::tempdir().unwrap();
        let skills_root = tmp.path().join("skills");
        fs::create_dir_all(&skills_root).unwrap();
        create_minimal_runtime(&skills_root, &["orphan"]);

        let manifest = serde_json::json!({
            "schema_version": constants::SCHEMA_MANIFEST,
            "keys": ["slug"],
            "skills": [["orphan"]]
        });
        fs::write(
            skills_root.join("SKILL_MANIFEST.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();

        let registry = SkillRegistry::new(tmp.path().into());
        let issues = registry.validate_consistency().unwrap();
        assert!(issues.iter().any(|i| i.message.contains("no SKILL.md")));
    }
}
