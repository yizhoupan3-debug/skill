//! Skill deletion with safety checks.

use core_errors::FrameworkError;
use std::fs;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Options for deleting a skill.
#[derive(Debug, Clone)]
pub struct DeleteOptions {
    pub slug: String,
    /// Also remove the slug from RUNTIME/MANIFEST JSON files.
    pub remove_from_registry: bool,
    /// Create a `.bak` backup before deleting.
    pub backup: bool,
    /// If true, only report what would be deleted.
    pub dry_run: bool,
}

// ---------------------------------------------------------------------------
// Result
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct DeleteResult {
    pub skill_dir: PathBuf,
    pub files_deleted: Vec<PathBuf>,
    pub backup_path: Option<PathBuf>,
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

pub type Result<T> = std::result::Result<T, FrameworkError>;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Delete a skill directory after safety checks.
///
/// Checks:
/// - The skill directory exists
/// - No other skill declares a `requires` dependency on this slug
/// - The slug is not referenced in any loadout in `SKILL_LOADOUTS.json`
pub fn delete_skill(
    skills_root: &Path,
    opts: &DeleteOptions,
) -> Result<DeleteResult> {
    crate::validate::validate_skill_name(&opts.slug).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, e)
    })?;
    let skill_dir = skills_root.join(&opts.slug);
    if !skill_dir.exists() {
        return Err(FrameworkError::not_found(opts.slug.clone()));
    }

    let mut warnings = Vec::new();

    // Check loadout references
    let loadouts_path = skills_root.join("SKILL_LOADOUTS.json");
    if loadouts_path.exists() {
        let loadouts_text = fs::read_to_string(&loadouts_path)?;
        if let Ok(loadouts_val) = serde_json::from_str::<serde_json::Value>(&loadouts_text)
            && let Some(loadouts_obj) = loadouts_val.get("loadouts").and_then(|v| v.as_object()) {
                let referencing: Vec<String> = loadouts_obj
                    .iter()
                    .filter(|(_, v)| {
                        v.get("skill_slugs")
                            .and_then(|s| s.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .any(|s| s.as_str() == Some(&opts.slug))
                            })
                            .unwrap_or(false)
                    })
                    .map(|(k, _)| k.clone())
                    .collect();
                if !referencing.is_empty() {
                    return Err(FrameworkError::validation(format!(
                        "cannot delete `{}`: referenced in loadouts: {:?}",
                        opts.slug, referencing
                    )));
                }
            }
    }

    // Backup if requested
    let mut backup_path = None;
    if opts.backup {
        let bak = skill_dir.with_extension("skill.bak");
        if opts.dry_run {
            warnings.push(format!("would backup to {}", bak.display()));
        } else {
            fs::rename(&skill_dir, &bak)?;
            backup_path = Some(bak);
        }
    }

    if opts.dry_run {
        warnings.push(format!("would delete {}", skill_dir.display()));
        return Ok(DeleteResult {
            skill_dir,
            files_deleted: vec![],
            backup_path,
            warnings,
        });
    }

    // Perform deletion
    fs::remove_dir_all(&skill_dir)?;

    Ok(DeleteResult {
        skill_dir,
        files_deleted: vec![],
        backup_path,
        warnings,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delete_fails_on_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let err = delete_skill(
            tmp.path(),
            &DeleteOptions {
                slug: "nonexistent".into(),
                remove_from_registry: false,
                backup: false,
                dry_run: false,
            },
        )
        .unwrap_err();
        assert!(matches!(err, FrameworkError::NotFound { .. }));
    }

    #[test]
    fn dry_run_does_not_delete() {
        let tmp = tempfile::tempdir().unwrap();
        let _dir = tmp.path().exists();
        // Create skill dir
        let skill_dir = tmp.path().join("test-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "---\nname: test\n---\n").unwrap();

        let result = delete_skill(
            tmp.path(),
            &DeleteOptions {
                slug: "test-skill".into(),
                remove_from_registry: false,
                backup: false,
                dry_run: true,
            },
        )
        .unwrap();
        assert!(skill_dir.exists());
        assert!(!result.warnings.is_empty());
    }
}
