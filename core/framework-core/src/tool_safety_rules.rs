//! Tool safety rules: path protection constants.
//! Extracted from claude_hooks.rs (Phase 4 S1).
//! Shared across 4 hosts for PreToolUse path guarding.
//!
//! Lifecycle classification (2026-06-23 audit):
//!   - write-only:  SKILL_PLUGIN_CATALOG.json
//!     Written by `router-rs framework skills refresh`, never read as data by the runtime.
//!   - document-only: RUNTIME_PROVIDER_REGISTRY.json — declared in hook_policy, does not
//!     drive routing or hook execution. See hook_policy.rs provider_registry_policy.

/// Auxiliary JSON files that are write-only (generated, never read by the runtime).
pub const WRITE_ONLY_AUXILIARY_FILES: &[&str] = &[
    "skills/SKILL_PLUGIN_CATALOG.json",
];

/// Auxiliary JSON files that are document-only (policy declaration, no runtime effect).
pub const DOCUMENT_ONLY_AUXILIARY_FILES: &[&str] =
    &["configs/framework/RUNTIME_PROVIDER_REGISTRY.json"];
