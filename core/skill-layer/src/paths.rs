//! Centralized skill file path constants and constructors.
//!
//! Eliminates hardcoded `"skills/SKILL_ROUTING_RUNTIME.json"` strings
//! scattered across 15+ locations in the codebase.

use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Directory
// ---------------------------------------------------------------------------

/// The `skills/` subdirectory name.
pub const SKILLS_DIR: &str = "skills";

/// Return `repo_root/skills/`.
pub fn skills_root(repo_root: &Path) -> PathBuf {
    repo_root.join(SKILLS_DIR)
}

// ---------------------------------------------------------------------------
// Registry files (in `skills/`)
// ---------------------------------------------------------------------------

/// `skills/SKILL_ROUTING_RUNTIME.json` — hot routing surface.
pub const SKILL_ROUTING_RUNTIME_JSON: &str = "SKILL_ROUTING_RUNTIME.json";
/// `skills/SKILL_LOADOUTS.json` — skill loadout profiles.
pub const SKILL_LOADOUTS_JSON: &str = "SKILL_LOADOUTS.json";

// ---------------------------------------------------------------------------
// Configs (framework-level paths)
// ---------------------------------------------------------------------------

/// `configs/framework/RUNTIME_REGISTRY.json` — framework runtime registry.
pub const RUNTIME_REGISTRY_JSON: &str = "configs/framework/RUNTIME_REGISTRY.json";
/// `configs/framework/FRAMEWORK_SURFACE_POLICY.json` — surface policy.
pub const FRAMEWORK_SURFACE_POLICY_JSON: &str = "configs/framework/FRAMEWORK_SURFACE_POLICY.json";

// ---------------------------------------------------------------------------
// Path constructors
// ---------------------------------------------------------------------------

/// `repo_root/skills/SKILL_ROUTING_RUNTIME.json`
pub fn runtime_json(repo_root: &Path) -> PathBuf {
    skills_root(repo_root).join(SKILL_ROUTING_RUNTIME_JSON)
}

/// `repo_root/skills/SKILL_LOADOUTS.json`
pub fn loadouts_json(repo_root: &Path) -> PathBuf {
    skills_root(repo_root).join(SKILL_LOADOUTS_JSON)
}

// ---------------------------------------------------------------------------
// SKILL.md paths
// ---------------------------------------------------------------------------

/// `repo_root/skills/{slug}/SKILL.md`
pub fn skill_md(repo_root: &Path, slug: &str) -> PathBuf {
    // Validate slug to prevent path traversal
    if slug.is_empty() || slug.starts_with('/') || slug.contains("..") {
        tracing::warn!("invalid slug '{slug}' passed to skill_md");
    }
    skills_root(repo_root).join(slug).join("SKILL.md")
}

// ---------------------------------------------------------------------------
// Configs constructors
// ---------------------------------------------------------------------------

/// `repo_root/configs/framework/RUNTIME_REGISTRY.json`
pub fn runtime_registry_json(repo_root: &Path) -> PathBuf {
    repo_root.join(RUNTIME_REGISTRY_JSON)
}

/// `repo_root/configs/framework/FRAMEWORK_SURFACE_POLICY.json`
pub fn surface_policy_json(repo_root: &Path) -> PathBuf {
    repo_root.join(FRAMEWORK_SURFACE_POLICY_JSON)
}
