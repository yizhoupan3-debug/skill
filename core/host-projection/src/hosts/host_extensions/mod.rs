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

use std::path::Path;

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

/// Check if the host supports hard gate hooks.
pub fn host_has_hard_gate(host_id: &str) -> bool {
    crate::hosts::host_provider_for_id(host_id)
        .map(|p| p.has_hard_gate_hooks())
        .unwrap_or(false)
}

/// Check if closeout evidence hooks are supported for this host.
pub fn host_closeout_evidence_supported(host_id: &str) -> bool {
    crate::hosts::host_provider_for_id(host_id)
        .map(|p| p.closeout_evidence_hooks_supported())
        .unwrap_or(false)
}

/// Get the registered hook events for a host.
pub fn host_registered_events(host_id: &str) -> &'static [&'static str] {
    crate::hosts::host_provider_for_id(host_id)
        .map(|p| p.registered_hook_events())
        .unwrap_or(&[])
}

/// Register all host-specific default hooks.
///
/// Called once during L4 bootstrap (runtime-core/lib.rs). Encapsulates host
/// extension setup that only needs L0-level access. L4→L0/L4 fn ptr
/// registrations remain in runtime-core's init sequence as prescribed by
/// ADR-010 §1.2 (registration direction L4→L0, not hardcoded in L0).
pub fn register_host_hooks() {}
