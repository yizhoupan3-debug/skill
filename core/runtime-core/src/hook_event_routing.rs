//! Cross-host hook lifecycle event routing contract (Roadmap v5 §6.4 cat.3).

use serde_json::{json, Value};

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

fn normalized_event_key(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Map a host-native hook event name to the shared canonical (or extended) lifecycle id.
pub fn canonical_hook_event(raw: &str) -> Option<&'static str> {
    match normalized_event_key(raw).as_str() {
        "sessionstart" => Some("SessionStart"),
        "pretooluse" | "toolexecutebefore" => Some("PreToolUse"),
        "toolexecuteafter" | "posttooluse" => Some("PostToolUse"),
        "userpromptsubmit" | "beforesubmitprompt" => Some("UserPromptSubmit"),
        "sessionend" | "sessionidle" => Some("SessionEnd"),
        "stop" => Some("Stop"),
        "subagentstart" => Some("SubagentStart"),
        "subagentstop" => Some("SubagentStop"),
        "sessioncreated" | "sessiondeleted" => Some("SessionEnd"),
        "permissionasked" | "permissionreplied" => Some("PreToolUse"),
        "fileedited" | "shellenv" => Some("PostToolUse"),
        _ => None,
    }
}

pub fn routable_lifecycle_events() -> impl Iterator<Item = &'static str> {
    CANONICAL_LIFECYCLE_EVENTS
        .iter()
        .copied()
        .chain(EXTENDED_LIFECYCLE_EVENTS.iter().copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_hook_event_maps_cursor_and_codex_spellings() {
        assert_eq!(canonical_hook_event("beforeSubmitPrompt"), Some("UserPromptSubmit"));
        assert_eq!(canonical_hook_event("UserPromptSubmit"), Some("UserPromptSubmit"));
        assert_eq!(canonical_hook_event("post-tool-use"), Some("PostToolUse"));
        assert_eq!(canonical_hook_event("sessionEnd"), Some("SessionEnd"));
        assert!(canonical_hook_event("unknown-event").is_none());
    }
}
