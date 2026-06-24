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
/// `skills/SKILL_MANIFEST.json` — cold fallback manifest.
pub const SKILL_MANIFEST_JSON: &str = "SKILL_MANIFEST.json";
/// `skills/SKILL_ROUTING_INDEX.json` — lightweight routing index.
pub const SKILL_ROUTING_INDEX_JSON: &str = "SKILL_ROUTING_INDEX.json";
/// `skills/SKILL_ROUTING_METADATA.json` — selection reasons.
pub const SKILL_ROUTING_METADATA_JSON: &str = "SKILL_ROUTING_METADATA.json";
/// `skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json` — routing explain stub.
pub const SKILL_ROUTING_RUNTIME_EXPLAIN_JSON: &str = "SKILL_ROUTING_RUNTIME_EXPLAIN.json";
/// `skills/SKILL_TIERS.json` — tier classification.
pub const SKILL_TIERS_JSON: &str = "SKILL_TIERS.json";
/// `skills/SKILL_HEALTH_MANIFEST.json` — health manifest.
pub const SKILL_HEALTH_MANIFEST_JSON: &str = "SKILL_HEALTH_MANIFEST.json";
/// `skills/SKILL_APPROVAL_POLICY.json` — approval policy.
pub const SKILL_APPROVAL_POLICY_JSON: &str = "SKILL_APPROVAL_POLICY.json";
/// `skills/SKILL_PLUGIN_CATALOG.json` — host platform support.
pub const SKILL_PLUGIN_CATALOG_JSON: &str = "SKILL_PLUGIN_CATALOG.json";
/// `skills/SKILL_ROUTING_INDEX.md` — generated routing index doc.
pub const SKILL_ROUTING_INDEX_MD: &str = "SKILL_ROUTING_INDEX.md";
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

/// `repo_root/skills/SKILL_MANIFEST.json`
pub fn manifest_json(repo_root: &Path) -> PathBuf {
    skills_root(repo_root).join(SKILL_MANIFEST_JSON)
}

/// `repo_root/skills/SKILL_ROUTING_INDEX.json`
pub fn index_json(repo_root: &Path) -> PathBuf {
    skills_root(repo_root).join(SKILL_ROUTING_INDEX_JSON)
}

/// `repo_root/skills/SKILL_ROUTING_METADATA.json`
pub fn metadata_json(repo_root: &Path) -> PathBuf {
    skills_root(repo_root).join(SKILL_ROUTING_METADATA_JSON)
}

/// `repo_root/skills/SKILL_TIERS.json`
pub fn tiers_json(repo_root: &Path) -> PathBuf {
    skills_root(repo_root).join(SKILL_TIERS_JSON)
}

/// `repo_root/skills/SKILL_HEALTH_MANIFEST.json`
pub fn health_json(repo_root: &Path) -> PathBuf {
    skills_root(repo_root).join(SKILL_HEALTH_MANIFEST_JSON)
}

/// `repo_root/skills/SKILL_APPROVAL_POLICY.json`
pub fn approval_json(repo_root: &Path) -> PathBuf {
    skills_root(repo_root).join(SKILL_APPROVAL_POLICY_JSON)
}

/// `repo_root/skills/SKILL_PLUGIN_CATALOG.json`
pub fn plugin_catalog_json(repo_root: &Path) -> PathBuf {
    skills_root(repo_root).join(SKILL_PLUGIN_CATALOG_JSON)
}

// ---------------------------------------------------------------------------
// SKILL.md paths
// ---------------------------------------------------------------------------

/// `repo_root/skills/{slug}/SKILL.md`
pub fn skill_md(repo_root: &Path, slug: &str) -> PathBuf {
    skills_root(repo_root).join(slug).join("SKILL.md")
}

/// `repo_root/skills/SKILL_LOADOUTS.json`
pub fn loadouts_json(repo_root: &Path) -> PathBuf {
    skills_root(repo_root).join(SKILL_LOADOUTS_JSON)
}

/// `repo_root/skills/SKILL_ROUTING_INDEX.md`
pub fn index_md(repo_root: &Path) -> PathBuf {
    skills_root(repo_root).join(SKILL_ROUTING_INDEX_MD)
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
