//! Host extension modules, organized by capability (ADR-010 §2.1).
//!
//! ## Architecture
//!
//! Code is organized by **capability**, not by host:
//!
//! | Module | Purpose |
//! |--------|---------|
//! | `dispatch.rs` | HostHookDispatcher implementations |
//! | `pretool.rs` | PreToolUse path protection |
//! | `schema_drift.rs` | Hook schema drift detection |
//!
//! All host lifecycle data (profile_id, driver_binary, registered_hook_events, etc.)
//! is now generated from `RUNTIME_REGISTRY.json` via `host_targets.metadata.*`
//! and `host-projection/build.rs`. CLI hook dispatch is registry-driven via
//! `host_provider_registry().dispatcher()`. No per-host source files remain.

pub mod dispatch;
pub mod pretool;
pub mod schema_drift;

// ── HostHookConfig constants (merged from config.rs) ──

/// Get the registered hook events for a host from the HostProvider registry.
/// Used by schema drift validation (migrated to runtime-core, Wave 4a-ii).
pub fn host_registered_hook_events(host_id: &str) -> &'static [&'static str] {
    crate::hosts::host_provider_for_id(host_id)
        .map(|p| p.registered_hook_events())
        .unwrap_or(&[])
}

// Backward-compatible re-exports
pub use dispatch::*;

/// Get the active host's log label for error messages.
pub fn host_log_label(host_id: &str) -> String {
    crate::hosts::host_provider_for_id(host_id)
        .map(|p| {
            let id = p.host_id();
            let mut chars = id.chars();
            match chars.next() {
                Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
                None => host_id.to_string(),
            }
        })
        .unwrap_or_else(|| host_id.to_string())
}

// host_has_hard_gate, host_closeout_evidence_supported, host_registered_events,
// and register_host_hooks were removed in Round8 — callers now use
// host_provider_for_id() trait methods directly.
