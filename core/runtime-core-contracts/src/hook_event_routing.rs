//! Cross-host hook lifecycle event routing contract.

use serde_json::{Value, json};

pub const HOOK_EVENT_ROUTING_SCHEMA_VERSION: &str = "router-rs-hook-event-routing-v1";
pub const HOOK_EVENT_ROUTING_AUTHORITY: &str = "rust-hook-event-routing";

/// Canonical lifecycle surface shared across native hook hosts (Codex spelling).
pub const CANONICAL_LIFECYCLE_EVENTS: &[&str] = &[
    "SessionStart",
    "PreToolUse",
    "UserPromptSubmit",
    "PostToolUse",
    "Stop",
    "SubagentStart",
    "SubagentStop",
];

/// Host-specific extensions that do not break cross-host routing semantics.
pub const EXTENDED_LIFECYCLE_EVENTS: &[&str] = &["SessionEnd"];

pub fn hook_event_routing_contract() -> Value {
    json!({
        "schema_version": HOOK_EVENT_ROUTING_SCHEMA_VERSION,
        "authority": HOOK_EVENT_ROUTING_AUTHORITY,
        "canonical_lifecycle_events": CANONICAL_LIFECYCLE_EVENTS,
        "extended_lifecycle_events": EXTENDED_LIFECYCLE_EVENTS,
        "routing_rules": [
            "native_hook_hosts_map_registered_events_to_canonical_or_extended",
            "anemic_hosts_have_empty_registered_events_and_stdio_fallback",
            "hook_telemetry_surface_matches_registry_transport",
            "observation_host_id_matches_host_id_for_native_hooks"
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
}
