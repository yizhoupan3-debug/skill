//! Host-specific extension points (ADR §2.1).
//!
//! Single module containing all host-specific behavior.
//! Core event pipeline lives in unified handlers (stop_dispatch, event_handlers);
//! host differences (state management, tool mappings, signal extraction) live here
//! as private implementation modules. The public API provides unified dispatch.

// ── Per-host implementations (private) ──
mod claude_impl;
mod codex_impl;
mod cursor_impl;
mod opencode_impl;

// ── Public re-exports for external consumers ──
// These are the public APIs that other modules need access to.
// The goal is to eventually move all this logic into unified handlers.

/// Claude-specific implementations.
pub mod claude {
    pub use super::claude_impl::*;
}

/// Codex-specific implementations.
pub mod codex {
    pub use super::codex_impl::*;
}

/// Cursor-specific implementations.
pub mod cursor {
    pub use super::cursor_impl::*;
}

/// OpenCode-specific implementations.
pub mod opencode {
    pub use super::opencode_impl::*;
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
