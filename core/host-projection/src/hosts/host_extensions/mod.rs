//! Host extension modules, organized by capability (ADR-010 §2.1).
//!
//! ## Architecture
//!
//! Instead of per-host files, code is organized by **capability**:
//!
//! | Module | Purpose | Shared? |
//! |--------|---------|---------|
//! | `config.rs` | HostHookConfig for all 4 hosts | ✅ All hosts |
//! | `dispatch.rs` | HostHookDispatcher implementations | ✅ All hosts |
//! | `pretool.rs` | PreToolUse path protection | ✅ All hosts |
//! | `codex/` | Codex-specific install + contract guard | ❌ Codex only |
//!
//! ## Registry-driven dispatch
//!
//! - `impl_host_config!` macro maps host IDs to config values
//! - `HostHookDispatcher` trait defaults handle all event types
//! - Codex overrides only `handle_pre_tool_use` for custom path protection
//! - All 4 hosts use identical event handler code paths
//!
//! ## File naming
//!
//! No file is named after a host. Files are named by what they DO.
//! The only exception is `codex/` which holds Codex-specific install logic.

pub mod config;
pub mod dispatch;
pub mod pretool;
pub mod install;
pub mod contract_guard;
pub mod schema_drift;
pub mod codex;

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
pub fn register_host_hooks() {}
