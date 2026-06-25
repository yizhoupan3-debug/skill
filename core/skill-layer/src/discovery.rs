//! Skill discovery: find SKILL.md files, resolve slugs, safe path resolution.

use crate::paths;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum DiscoveryError {
    Io(std::io::Error),
    InvalidSlug(String),
}

impl std::fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::InvalidSlug(s) => write!(f, "invalid skill slug: `{s}`"),
        }
    }
}

impl std::error::Error for DiscoveryError {}

impl From<std::io::Error> for DiscoveryError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

// ---------------------------------------------------------------------------
// Slug discovery (disk scan)
// ---------------------------------------------------------------------------

/// Scan `skills_root` recursively and return all slugs that have a SKILL.md.
///
/// Extracted from `runtime-infra::framework_skills::discover_skill_md_slugs`.
pub fn discover_skill_md_slugs(skills_root: &Path) -> Result<BTreeSet<String>, DiscoveryError> {
    let mut slugs = BTreeSet::new();
    walk_skill_md(skills_root, &mut slugs)?;
    Ok(slugs)
}

fn walk_skill_md(dir: &Path, slugs: &mut BTreeSet<String>) -> Result<(), DiscoveryError> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            // Skip hidden directories
            if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && name.starts_with('.') {
                    continue;
                }
            walk_skill_md(&path, slugs)?;
        } else if path.file_name().and_then(|s| s.to_str()) == Some("SKILL.md")
            && let Some(name) = parse_skill_name_from_path(&path)? {
                slugs.insert(name);
            }
    }
    Ok(())
}

/// Read a SKILL.md file and extract the `name` field from frontmatter.
///
/// Uses a lightweight extraction (only looks for `name:` key) rather than
/// the full `parse_frontmatter` parser, since discovery only needs the slug.
pub fn parse_skill_name_from_path(path: &Path) -> Result<Option<String>, DiscoveryError> {
    let text = fs::read_to_string(path)?;
    // Lightweight: just extract frontmatter block and find name: line
    let Some(block) = crate::frontmatter_parser::extract_frontmatter_block(&text) else {
        return Ok(None);
    };
    for line in block.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("name:") {
            return Ok(Some(rest.trim().to_string()));
        }
    }
    Ok(None)
}

// ---------------------------------------------------------------------------
// Safe path resolution (slug → SKILL.md path)
// ---------------------------------------------------------------------------

/// Resolve a skill slug to its SKILL.md path with safety checks.
///
/// Extracted from `host-projection::tools::skill_body_path`.
///
/// # Safety
/// - Slug must not contain `..` or `/`
/// - Path must resolve to a file under `skills_root`
pub fn safe_skill_md_path(
    skills_root: &Path,
    slug: &str,
) -> Result<PathBuf, DiscoveryError> {
    // Validate slug characters
    if slug.contains("..") || slug.contains('/') || slug.contains('\\') {
        return Err(DiscoveryError::InvalidSlug(slug.to_string()));
    }

    let path = skills_root.join(slug).join("SKILL.md");

    // Verify the resolved path is still under skills_root
    let canonical_skills = fs::canonicalize(skills_root)
        .unwrap_or_else(|_| skills_root.to_path_buf());
    if let Ok(canonical_path) = fs::canonicalize(&path)
        && !canonical_path.starts_with(&canonical_skills) {
            return Err(DiscoveryError::InvalidSlug(format!(
                "path traversal detected: {slug}"
            )));
        }

    Ok(path)
}

/// Resolve a skill slug from a manifest's `skill_path` field.
///
/// Extracted from `host-projection::tools::skill_body_path_from_manifest`.
pub fn skill_md_from_manifest(
    repo_root: &Path,
    manifest: &serde_json::Value,
    slug: &str,
) -> Option<PathBuf> {
    let keys: Vec<String> = manifest["keys"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let slug_idx = keys.iter().position(|k| k == "slug");
    let path_idx = keys.iter().position(|k| k == "skill_path");

    if let (Some(slug_idx), Some(path_idx)) = (slug_idx, path_idx)
        && let Some(rows) = manifest["skills"].as_array() {
            for row in rows {
                if row.get(slug_idx).and_then(|v| v.as_str()) == Some(slug)
                    && let Some(rel) = row.get(path_idx).and_then(|v| v.as_str()) {
                        let full = repo_root.join(rel);
                        if full.exists() {
                            return Some(full);
                        }
                    }
            }
        }

    // Fallback to standard path
    let fallback = paths::skill_md(repo_root, slug);
    if fallback.exists() {
        Some(fallback)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Repo root discovery
// ---------------------------------------------------------------------------

/// Check if a path looks like a skill policy repo root.
///
/// Criteria: contains both `skills/SKILL_ROUTING_RUNTIME.json` and `AGENTS.md`.
pub fn is_skill_repo_root(path: &Path) -> bool {
    paths::runtime_json(path).exists() && path.join("AGENTS.md").exists()
}

/// Discover the skill repo root by walking up from `start` or using the
/// `SKILL_POLICY_REPO_ROOT` environment variable.
pub fn find_skill_repo_root(start: &Path) -> Option<PathBuf> {
    // 1. Check env var
    if let Ok(env_root) = std::env::var("SKILL_POLICY_REPO_ROOT") {
        let p = PathBuf::from(env_root);
        if is_skill_repo_root(&p) {
            return Some(p);
        }
    }

    // 2. Walk up from start
    let mut current = start.to_path_buf();
    loop {
        if is_skill_repo_root(&current) {
            return Some(current);
        }
        if !current.pop() {
            break;
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_skills_root(tmp: &Path) {
        let skills = tmp.join("skills");
        fs::create_dir_all(skills.join("alpha")).unwrap();
        fs::write(
            skills.join("alpha/SKILL.md"),
            "---\nname: alpha\ndescription: A\nrouting_layer: L2\nrouting_owner: owner\nrouting_gate: none\nrouting_priority: P2\nsession_start: n/a\ntrigger_hints: [a]\n---\n# alpha\n",
        )
        .unwrap();
        fs::create_dir_all(skills.join(".hidden")).unwrap();
        fs::write(skills.join(".hidden/SKILL.md"), "---\nname: hidden\n---\n").unwrap();
    }

    #[test]
    fn discover_finds_visible_skills() {
        let tmp = tempfile::tempdir().unwrap();
        setup_skills_root(tmp.path());
        let slugs = discover_skill_md_slugs(&tmp.path().join("skills")).unwrap();
        assert!(slugs.contains("alpha"));
        assert!(!slugs.contains("hidden")); // .hidden dir skipped
    }

    #[test]
    fn safe_path_rejects_traversal() {
        let tmp = tempfile::tempdir().unwrap();
        let err = safe_skill_md_path(tmp.path(), "../etc/passwd").unwrap_err();
        assert!(matches!(err, DiscoveryError::InvalidSlug(_)));
    }

    #[test]
    fn find_repo_root_walks_up() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        // Create required files under skills/ subdirectory
        let skills_dir = repo.join("skills");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(skills_dir.join("SKILL_ROUTING_RUNTIME.json"), "{}").unwrap();
        fs::write(repo.join("AGENTS.md"), "").unwrap();
        let deep = repo.join("a/b/c");
        fs::create_dir_all(&deep).unwrap();
        assert_eq!(find_skill_repo_root(&deep), Some(repo.to_path_buf()));
    }

    #[test]
    fn parse_skill_name_extracts_name() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("SKILL.md");
        fs::write(&path, "---\nname: test\n---\n").unwrap();
        let name = parse_skill_name_from_path(&path)
            .expect("parse_skill_name_from_path should not error");
        assert_eq!(name.as_deref(), Some("test"));
    }
}
