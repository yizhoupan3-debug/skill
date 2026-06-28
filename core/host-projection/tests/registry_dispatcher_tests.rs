//! Integration tests for RegistryDispatcher behavior across all supported hosts.
//!
//! Tests exercise the dispatcher through the public HostHookDispatcher trait
//! via host_provider_for_id().dispatcher(), not through internal types.
//!
//! All expected values are read from RUNTIME_REGISTRY.json at test time —
//! the registry is the single source of truth for host metadata.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use host_projection::hosts::hook_dispatch::HookEvent;
use host_projection::hosts::host_provider_for_id;
use serde_json::{Value, json};
use std::path::Path;

/// Load RUNTIME_REGISTRY.json and return per-host metadata.
fn host_metadata() -> serde_json::Map<String, Value> {
    let framework_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../");
    let registry =
        framework_kernel::runtime_registry::load_runtime_registry_payload(&framework_root)
            .expect("load RUNTIME_REGISTRY.json");
    registry
        .get("host_targets")
        .and_then(|ht| ht.get("metadata"))
        .and_then(|m| m.as_object())
        .expect("host_targets.metadata in RUNTIME_REGISTRY.json")
        .clone()
}

fn test_repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../").leak()
}

#[test]
fn all_hosts_have_dispatcher() {
    for host_id in framework_kernel::runtime_registry::ALL_HOST_IDS {
        let provider =
            host_provider_for_id(host_id).unwrap_or_else(|| panic!("no provider for {host_id}"));
        let _ = provider.dispatcher();
    }
}

#[test]
fn session_start_support_matches_registry() {
    let metadata = host_metadata();
    for host_id in framework_kernel::runtime_registry::ALL_HOST_IDS {
        let expected = metadata
            .get(*host_id)
            .and_then(|m| m.get("session_start"))
            .and_then(Value::as_bool)
            .unwrap_or_else(|| panic!("{host_id}: no session_start in registry metadata"));
        let provider = host_provider_for_id(host_id).unwrap();
        assert_eq!(
            provider.dispatcher().supports_session_start(),
            expected,
            "host {host_id}: session_start mismatch"
        );
    }
}

#[test]
fn subagent_start_stop_support_matches_registry() {
    let metadata = host_metadata();
    for host_id in framework_kernel::runtime_registry::ALL_HOST_IDS {
        let m = metadata
            .get(*host_id)
            .unwrap_or_else(|| panic!("{host_id}: missing metadata"));
        let expected_start = m
            .get("subagent_start")
            .and_then(Value::as_bool)
            .unwrap_or_else(|| panic!("{host_id}: no subagent_start"));
        let expected_stop = m
            .get("subagent_stop")
            .and_then(Value::as_bool)
            .unwrap_or_else(|| panic!("{host_id}: no subagent_stop"));
        let provider = host_provider_for_id(host_id).unwrap();
        let dispatcher = provider.dispatcher();
        assert_eq!(
            dispatcher.supports_subagent_start(),
            expected_start,
            "host {host_id}: subagent_start mismatch"
        );
        assert_eq!(
            dispatcher.supports_subagent_stop(),
            expected_stop,
            "host {host_id}: subagent_stop mismatch"
        );
    }
}

#[test]
fn pretool_protection_matches_registry() {
    let metadata = host_metadata();
    for host_id in framework_kernel::runtime_registry::ALL_HOST_IDS {
        let expected_protection = metadata
            .get(*host_id)
            .and_then(|m| m.get("pretool_path_protection"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let provider = host_provider_for_id(host_id).unwrap();
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
        if expected_protection {
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
fn handle_stop_does_not_panic() {
    for host_id in framework_kernel::runtime_registry::ALL_HOST_IDS {
        let provider =
            host_provider_for_id(host_id).unwrap_or_else(|| panic!("no provider for {host_id}"));
        let dispatcher = provider.dispatcher();
        let event = HookEvent {
            repo_root: test_repo_root(),
            event_name: "stop",
            payload: &json!({}),
        };
        let _ = dispatcher.handle_stop(&event);
        // handle_stop returns None when the pipeline has nothing to report
        // (empty payload, no state files on disk). Only panics are errors here.
    }
}

#[test]
fn session_key_extraction_respects_scan_tool_input() {
    for host_id in framework_kernel::runtime_registry::ALL_HOST_IDS {
        let provider = host_provider_for_id(host_id).unwrap();
        let dispatcher = provider.dispatcher();
        let event_with_tool_input = HookEvent {
            repo_root: test_repo_root(),
            event_name: "pretooluse",
            payload: &json!({
                "tool_name": "Agent",
                "tool_input": { "session_id": "from-tool-input" }
            }),
        };
        let _ = dispatcher.handle_pre_tool_use(&event_with_tool_input);
        // Just verify no panic (session_key extraction is internal)
    }
}
