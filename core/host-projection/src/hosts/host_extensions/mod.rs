//! Host extension implementations, organized by functionality.
//!
//! Each host has its own module holding the `HostHookDispatcher` implementation.
//! Shared logic lives in the parent module (`hosts/hook_dispatch.rs`).

// ── Per-host implementations (private) ──
mod claude;
mod codex;
mod cursor;
mod opencode;

// ── Public re-exports ──

pub mod claude_impl {
    pub use super::claude::*;
}

/// Codex implementations.
pub mod codex_impl {
    pub use super::codex::*;
}

/// Cursor implementations.
pub mod cursor_impl {
    pub use super::cursor::*;
}

/// OpenCode implementations.
pub mod opencode_impl {
    pub use super::opencode::*;
}

// ── Shared host extension utilities ──

use serde_json::Value;
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

// ── Common imports available to all per-host implementations ──
// Per-host files should import directly from core_state::utils::json_io

/// Register all host-specific default hooks into the L0 function pointer registry.
///
/// Called from L4 runtime-core bootstrap so that L4 code never references
/// per-host extension functions by name (ADR-010 §4 host isolation).
pub fn register_host_hooks() {
    // Review gate handler — cursor-specific implementation lives in L0.
    crate::hooks::register_review_gate_handler(cursor_impl::run_cursor_review_gate);
}
