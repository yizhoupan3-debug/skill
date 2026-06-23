//! Host extension modules, organized by capability (ADR-010 §2.1).
//!
//! ## Architecture
//!
//! Code is organized by **capability**, not by host:
//!
//! | Module | Purpose |
//! |--------|---------|
//! | `config.rs` | HostHookConfig for all 4 hosts |
//! | `dispatch.rs` | HostHookDispatcher implementations |
//! | `pretool.rs` | PreToolUse path protection |
//! | `contract_guard.rs` | Contract guard event handling |
//! | `schema_drift.rs` | Hook schema drift detection |
//!
//! All host lifecycle data (profile_id, driver_binary, registered_hook_events, etc.)
//! is now generated from `RUNTIME_REGISTRY.json` via `host_targets.metadata.*`
//! and `host-projection/build.rs`. CLI hook dispatch is registry-driven via
//! `host_provider_registry().dispatcher()`. No per-host source files remain.

pub mod config;
pub mod dispatch;
pub mod pretool;
pub mod contract_guard;
pub mod schema_drift;

// Backward-compatible re-exports
pub use config::*;
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
