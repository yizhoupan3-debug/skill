//! Centralized schema version constants for all skill registry artifacts.
//!
//! Every `schema_version` string in the codebase MUST reference a constant here.
//! This prevents version drift (e.g. stub v1 vs generator v2).

// ---------------------------------------------------------------------------
// Skill registry schemas (reading side)
// ---------------------------------------------------------------------------

/// `skills/SKILL_ROUTING_RUNTIME.json` — hot routing surface.
pub const SCHEMA_RUNTIME: &str = "skill-routing-runtime-v3";

// ---------------------------------------------------------------------------
// Skill registry schemas (generated / write side)
// ---------------------------------------------------------------------------

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
