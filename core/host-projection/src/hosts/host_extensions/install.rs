//! Shared hook installation framework for all 4 hosts.
//!
//! Each host provides its own hook event registrations and command templates.
//! The actual file writing is handled by `host_entrypoint_sync.rs`.
//! Codex has the most elaborate install; Cursor and OpenCode use thinner wrappers.

use serde_json::{Value, json};
use crate::host_entrypoint_sync::HostEntrypointPayloadProvider;

// ── Install mode & merge stat (shared types) ──

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallMode { Apply, Check }

#[derive(Debug, Clone)]
pub struct HooksMergeStat {
    pub status: &'static str,
    pub preserved_existing_entries: usize,
    pub added_entries: usize,
    pub removed_legacy_entries: usize,
}

/// Build the hook JSON manifest for a given host.
/// `events` — list of hook event names (e.g. `["PostToolUse", "Stop"]`).
/// `host_id` — used to build the router-rs command template.
pub fn build_host_hook_manifest(events: &[&str], host_id: &str) -> Value {
    let commands: Vec<Value> = events.iter().map(|event| {
        json!({
            "event": event,
            "command": format!("router-rs {host_id} hook --event={event} --repo-root \"$PWD\""),
        })
    }).collect();
    json!({ "hooks": commands })
}

/// Build a host entrypoint payload provider for hook installation.
/// `host_id` — used to build the router-rs command template.
pub fn build_host_entrypoint_provider(host_id: &str, events: &[&str]) -> HostEntrypointPayloadProvider {
    HostEntrypointPayloadProvider {
        authority: "router-rs",
        host_id: host_id.to_string(),
        hook_projection: build_host_hook_manifest(events, host_id),
        readme_md: String::new(),
    }
}

/// Build codex hook command (used by codex/install.rs).
pub fn build_codex_hook_command(event_name: &str) -> String {
    format!(
        "cargo run --manifest-path core/router-rs/Cargo.toml -- codex hook --event={event_name} --repo-root \"$PWD\""
    )
}
