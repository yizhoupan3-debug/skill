#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use serde_json::json;
use std::path::{Path, PathBuf};

#[test]
fn test_silent_success_returns_empty_json() {
    assert_eq!(silent_success(), json!({}));
}

#[test]
fn test_add_context_formats_correctly() {
    let result = add_context("TestEvent", "hello world");
    assert!(result.is_some());
    let val = result.unwrap();
    assert_eq!(val["context_append"], "[TestEvent] hello world");
}

#[test]
fn test_normalize_path_lexical_removes_dot_segments() {
    let path = Path::new("a/./b/../c");
    let result = normalize_path_lexical(path);
    assert_eq!(result, Path::new("a/c"));
}

#[test]
fn test_compact_repo_relative_segments_removes_extra_parent() {
    let result = compact_repo_relative_segments("a/../../b");
    assert_eq!(result, Some(PathBuf::from("b")));
}

#[test]
fn test_is_host_private_path_matches_dot_claude() {
    assert!(is_host_private_path("repo/.claude/hooks.json"));
    assert!(!is_host_private_path("repo/src/main.rs"));
}

#[test]
fn test_is_framework_source_path() {
    assert!(is_framework_source_path("src/main.rs"));
    assert!(!is_framework_source_path("README.md"));
}

#[test]
fn test_bash_command_extracts_from_payload() {
    let payload = json!({"command": "cargo build"});
    assert_eq!(bash_command(&payload), Some("cargo build"));
}

#[test]
fn test_payload_looks_like_foreign_hook_stdin() {
    let cursor_payload = json!({
        "cursor_version": "1.0",
        "workspace_roots": ["/path"],
        "hook_event_name": "stop"
    });
    assert!(payload_looks_like_foreign_hook_stdin(&cursor_payload));
    let claude_payload = json!({"event": "stop"});
    assert!(!payload_looks_like_foreign_hook_stdin(&claude_payload));
}

