//! Centralized schema version constants for all skill registry artifacts.
//!
//! Every `schema_version` string in the codebase MUST reference a constant here.
//! This prevents version drift (e.g. stub v1 vs generator v2).

// ---------------------------------------------------------------------------
// Skill registry schemas (reading side)
// ---------------------------------------------------------------------------

/// `skills/SKILL_ROUTING_RUNTIME.json` — hot routing surface.
pub const SCHEMA_RUNTIME: &str = "skill-routing-runtime-v3";
/// `skills/SKILL_MANIFEST.json` — cold fallback manifest.
pub const SCHEMA_MANIFEST: &str = "skill-manifest-v2";
/// `skills/SKILL_ROUTING_INDEX.json` — lightweight routing index.
pub const SCHEMA_INDEX: &str = "skill-routing-index-v2";
/// `skills/SKILL_ROUTING_METADATA.json` — selection reasons.
pub const SCHEMA_METADATA: &str = "skill-routing-metadata-v1";
/// `skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json` — routing explain stub.
pub const SCHEMA_EXPLAIN: &str = "skill-routing-runtime-explain-v1";

// ---------------------------------------------------------------------------
// Skill registry schemas (generated / write side)
// ---------------------------------------------------------------------------

/// `skills/SKILL_TIERS.json` — tier classification (generated).
pub const SCHEMA_TIERS: &str = "skill-tiers-v1";
/// `skills/SKILL_HEALTH_MANIFEST.json` — health manifest (generated).
pub const SCHEMA_HEALTH: &str = "skill-health-manifest-v1";
/// `skills/SKILL_APPROVAL_POLICY.json` — approval policy (generated).
pub const SCHEMA_APPROVAL: &str = "skill-approval-policy-v1";
/// `skills/SKILL_PLUGIN_CATALOG.json` — host platform support (generated).
pub const SCHEMA_PLUGIN_CATALOG: &str = "skill-plugin-catalog-v1";

// ---------------------------------------------------------------------------
// CLI / harness schemas
// ---------------------------------------------------------------------------

/// Frontmatter contract lint schema.
pub const SCHEMA_SKILL_CONTRACT_LINT: &str = "router-rs-harness-skill-contract-lint-v1";

// ---------------------------------------------------------------------------
// Runtime registry schema (configs/framework/)
// ---------------------------------------------------------------------------

/// `configs/framework/RUNTIME_REGISTRY.json` — framework runtime registry.
pub const SCHEMA_RUNTIME_REGISTRY: &str = "framework-runtime-registry-v2";
