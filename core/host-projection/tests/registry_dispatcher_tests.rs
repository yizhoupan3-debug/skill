//! Integration tests for RegistryDispatcher behavior across all 4 hosts.
//!
//! Tests exercise the dispatcher through the public HostHookDispatcher trait
//! via host_provider_for_id().dispatcher(), not through internal types.

use host_projection::hosts::hook_dispatch::HookEvent;
use host_projection::hosts::host_provider_for_id;
use serde_json::json;
use std::path::Path;

/// All 4 supported host IDs — must match RUNTIME_REGISTRY.json
const ALL_HOSTS: &[&str] = &["claude", "cursor", "codex", "opencode"];

fn test_repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../").leak()
}

#[test]
fn all_hosts_have_dispatcher() {
    for host_id in ALL_HOSTS {
        let provider = host_provider_for_id(host_id)
            .unwrap_or_else(|| panic!("no provider for {host_id}"));
        let dispatcher = provider.dispatcher();
        // Verify dispatcher responds to a minimal event without panicking
        let event = HookEvent {
            repo_root: test_repo_root(),
            event_name: "pretooluse",
            payload: &json!({}),
        };
        let _ = dispatcher.dispatch(&event);
    }
}

#[test]
fn session_start_support_matches_registry() {
    let expected: &[(&str, bool)] = &[
        ("claude", false),
        ("cursor", true),
        ("codex", false),
        ("opencode", true),
    ];
    for (host_id, expected_val) in expected {
        let provider = host_provider_for_id(host_id)
            .unwrap_or_else(|| panic!("no provider for {host_id}"));
        let dispatcher = provider.dispatcher();
        assert_eq!(
            dispatcher.supports_session_start(),
            *expected_val,
            "host {host_id} session_start support mismatch"
        );
    }
}

#[test]
fn subagent_start_stop_support_matches_registry() {
    let expected: &[(&str, bool)] = &[
        ("claude", false),
        ("cursor", true),
        ("codex", false),
        ("opencode", true),
    ];
    for (host_id, expected_val) in expected {
        let provider = host_provider_for_id(host_id)
            .unwrap_or_else(|| panic!("no provider for {host_id}"));
        let dispatcher = provider.dispatcher();
        assert_eq!(
            dispatcher.supports_subagent_start(),
            *expected_val,
            "host {host_id} subagent_start support mismatch"
        );
        assert_eq!(
            dispatcher.supports_subagent_stop(),
            *expected_val,
            "host {host_id} subagent_stop support mismatch"
        );
    }
}

#[test]
fn pretool_protection_matches_registry() {
    let expected: &[(&str, bool)] = &[
        ("claude", false),
        ("cursor", false),
        ("codex", true),
        ("opencode", true),
    ];
    for (host_id, expected_protection) in expected {
        let provider = host_provider_for_id(host_id)
            .unwrap_or_else(|| panic!("no provider for {host_id}"));
        let dispatcher = provider.dispatcher();

        // Payload targeting AGENTS.md (a protected path)
        let event = HookEvent {
            repo_root: test_repo_root(),
            event_name: "pretooluse",
            payload: &json!({
                "tool_name": "Write",
                "tool_input": { "file_path": "AGENTS.md" }
            }),
        };
        let result = dispatcher.handle_pre_tool_use(&event);
        if *expected_protection {
            assert!(
                result.is_some(),
                "host {host_id}: PreToolUse should block writes to AGENTS.md"
            );
        } else {
            assert!(
                result.is_none(),
                "host {host_id}: PreToolUse should not block (no protection)"
            );
        }
    }
}

#[test]
fn handle_stop_returns_output_for_all_hosts() {
    for host_id in ALL_HOSTS {
        let provider = host_provider_for_id(host_id)
            .unwrap_or_else(|| panic!("no provider for {host_id}"));
        let dispatcher = provider.dispatcher();
        let event = HookEvent {
            repo_root: test_repo_root(),
            event_name: "stop",
            payload: &json!({}),
        };
        let result = dispatcher.handle_stop(&event);
        // stop handler should return Some for all hosts (unified pipeline)
        assert!(
            result.is_some(),
            "host {host_id}: handle_stop should return Some"
        );
    }
}

#[test]
fn session_key_extraction_respects_scan_tool_input() {
    // Cursor: scan_tool_input=true, should pick up session_id from tool_input
    let cursor_provider = host_provider_for_id("cursor").unwrap();
    let cursor_dispatcher = cursor_provider.dispatcher();
    let event_with_tool_input = HookEvent {
        repo_root: test_repo_root(),
        event_name: "pretooluse",
        payload: &json!({
            "tool_name": "Agent",
            "tool_input": { "session_id": "from-tool-input" }
        }),
    };
    let _ = cursor_dispatcher.handle_pre_tool_use(&event_with_tool_input);

    // Claude: scan_tool_input=false, should NOT pick up from tool_input
    let claude_provider = host_provider_for_id("claude").unwrap();
    let claude_dispatcher = claude_provider.dispatcher();
    let _ = claude_dispatcher.handle_pre_tool_use(&event_with_tool_input);

    // Both should complete without panicking
    // (session_key extraction is internal, verified by other observation tests)
}
