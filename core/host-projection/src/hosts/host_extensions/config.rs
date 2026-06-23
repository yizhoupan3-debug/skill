//! Host extension configuration — event constants and hook state lookup.
//!
//! All host lifecycle data is generated from RUNTIME_REGISTRY.json by
//! `host-projection/build.rs`. This module only retains constants that
//! cannot be generated (subtracted-events list) and the generated hook
//! events accessor for runtime-exit-gate schema drift validation.

use crate::hosts::hook_dispatch::HostHookConfig;

// ── Registered hook events ──

/// Get the registered hook events for a host from the HostProvider registry.
/// Used by runtime-exit-gate schema drift validation.
pub fn host_registered_hook_events(host_id: &str) -> &'static [&'static str] {
    crate::hosts::host_provider_for_id(host_id)
        .map(|p| p.registered_hook_events())
        .unwrap_or(&[])
}

/// Events removed from default .cursor/hooks.json (dispatch defaults to no-op).
/// Handler implementations remain in L0 for recovery paths.
pub const CURSOR_HOOKS_SUBTRACTED_EVENTS: &[&str] = &[
    "afterAgentResponse", "beforeShellExecution", "afterShellExecution",
    "afterFileEdit", "preCompact",
];
