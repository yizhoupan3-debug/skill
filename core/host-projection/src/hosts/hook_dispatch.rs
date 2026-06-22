//! Re-export all shared hook types / traits / utilities from hook-layer.
//!
//! Host-specific dispatch behaviour (kernel bootstrap, closeout checks, audit
//! injection) is wired via `HostHookConfig` method overrides in each host's
//! `impl` block — see `claude_hooks.rs`, `cursor_hooks/dispatcher.rs`,
//! `codex_hooks/dispatcher.rs`, and `opencode_hooks.rs`.
//!
//! Public API path: `host_projection::hosts::hook_dispatch::*` (unchanged).

pub use hook_layer::hook_dispatch::*;
