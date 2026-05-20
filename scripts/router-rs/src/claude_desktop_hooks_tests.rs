//! Tests for Claude Desktop MCP integration.
//!
//! These tests verify the fixes applied in the Claude Desktop full-chain review.

#[cfg(test)]
mod desktop_mcp_tests {
    use crate::claude_desktop_test_support;
    use serde_json::json;
    use std::path::{Path, PathBuf};

    fn test_repo_dir() -> PathBuf {
        let path = claude_desktop_test_support::unique_temp_repo("mcp");
        claude_desktop_test_support::seed_minimal_current_task_layout(&path);
        path
    }

    fn evidence_content(repo_path: &Path) -> serde_json::Value {
        crate::claude_desktop_hooks::read_evidence_index(repo_path).unwrap_or(json!({}))
    }

    #[test]
    fn tools_list_matches_handler() {
        let response = crate::claude_desktop_hooks::handle_tools_list(Some(json!(1)));
        let tools = response["result"]["tools"].as_array().expect("tools array");
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();

        let handler_arms = [
            "framework_digest",
            "framework_snapshot",
            "skill_route",
            "record_evidence",
            "session_checkpoint",
            "closeout_gate",
            "goal_state_read",
            "rfv_loop_status",
            "rfv_loop_manage",
            "closeout_record_write",
            "goal_state_manage",
        ];
        for name in &handler_arms {
            assert!(names.contains(name), "tool {name} missing from tools/list");
        }
        assert_eq!(
            names.len(),
            handler_arms.len(),
            "expected {} tools, got {}: {:?}",
            handler_arms.len(),
            names.len(),
            names
        );
    }

    #[test]
    fn evidence_records_exit_code_correctly() {
        let repo = test_repo_dir();

        let args_with = json!({
            "tool_name": "Bash",
            "command": "ls",
            "exit_code": 0,
            "output": "file1.txt",
        });
        let entry_result = crate::claude_desktop_hooks::build_evidence_entry(&args_with);
        assert!(
            entry_result.is_ok(),
            "build_evidence_entry failed: {:?}",
            entry_result
        );
        let entry = entry_result.unwrap();
        assert_eq!(entry.get("exit_code").and_then(|v| v.as_i64()), Some(0));
        assert_eq!(entry.get("success").and_then(|v| v.as_bool()), Some(true));

        let args_without = json!({
            "tool_name": "Read",
            "command": "cat foo.txt",
            "output": "contents",
        });
        let entry_without =
            crate::claude_desktop_hooks::build_evidence_entry(&args_without).unwrap();
        assert!(entry_without.get("exit_code").is_none());
        assert!(entry_without.get("success").is_none());

        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn closeout_record_write_writes_file_and_validates() {
        let repo = test_repo_dir();
        let args = json!({
            "task_id": "test-closeout",
            "summary": "Test completed successfully",
            "verification_status": "passed",
            "changed_files": ["src/main.rs"],
            "commands_run": [
                {"command": "cargo test", "exit_code": 0}
            ],
        });
        let result = crate::claude_desktop_hooks::tool_closeout_record_write_for_test(&args, &repo);
        assert!(result.is_ok(), "closeout_record_write failed: {:?}", result);
        let output = result.unwrap();
        // Should contain closeout_allowed field
        assert!(output.contains("closeout_allowed") || output.contains("closeout"));
        // File should exist at the expected path
        let record_path = repo
            .join("artifacts")
            .join("closeout")
            .join("test-closeout.json");
        assert!(
            record_path.is_file(),
            "record file should exist at {:?}",
            record_path
        );
        // Verify content
        let content = std::fs::read_to_string(&record_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["task_id"], "test-closeout");
        assert_eq!(parsed["verification_status"], "passed");
        assert!(parsed["schema_version"]
            .as_str()
            .unwrap()
            .contains("closeout-record"));

        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn closeout_record_write_missing_required_field_returns_error() {
        let repo = test_repo_dir();
        // Missing task_id
        let args = json!({
            "summary": "test",
            "verification_status": "passed",
        });
        let result = crate::claude_desktop_hooks::tool_closeout_record_write_for_test(&args, &repo);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("task_id"),
            "should mention missing task_id, got: {err}"
        );

        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn json_semantic_comparison() {
        let json_a = r#"{"a":1,"b":2}"#;
        let json_b = r#"{
            "a": 1,
            "b": 2
        }"#;
        let val_a: serde_json::Value = serde_json::from_str(json_a).unwrap();
        let val_b: serde_json::Value = serde_json::from_str(json_b).unwrap();
        assert_eq!(val_a, val_b);
    }
}

#[cfg(test)]
mod transport_mode_tests {
    #[test]
    fn transport_mode_content_length_requires_blank_line_separator() {
        // M7 FIX: Replace false assertion with actual test for Content-Length transport mode
        use std::io::{BufReader, Cursor};

        // Body: {"jsonrpc":"2.0","id":1,"method":"ping"} = 35 bytes
        let body = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}";
        let input = format!("Content-Length: {}\r\nX-Custom: value\r\n\r\n", body.len());
        let mut full_input = input.into_bytes();
        full_input.extend_from_slice(body);
        let cursor = Cursor::new(full_input);
        let mut reader = BufReader::new(cursor);
        let mut transport_mode = None;

        let result = crate::claude_desktop_hooks::read_mcp_message_test_helper(
            &mut reader,
            &mut transport_mode,
        );
        assert!(result.is_ok());
        let msg = result.unwrap().unwrap();
        assert!(msg.contains("jsonrpc"));
        assert!(transport_mode.is_some());
    }
}

#[cfg(test)]
mod session_tracker_tests {
    #[test]
    fn atomic_write_pattern() {
        use crate::session_call_tracker::test_lock_roundtrip;
        assert!(test_lock_roundtrip());
    }
}

#[cfg(test)]
mod routing_tests {
    use std::collections::HashSet;

    #[test]
    fn filter_warning_logged_when_all_filtered() {
        use crate::route::filter_records_for_host;
        use crate::route::SkillRecord;

        let records = vec![SkillRecord {
            slug: "test-skill".to_string(),
            slug_lower: "test-skill".to_string(),
            owner: "owner".to_string(),
            owner_lower: "owner".to_string(),
            layer: "L0".to_string(),
            gate: "source".to_string(),
            gate_lower: "source".to_string(),
            session_start: "required".to_string(),
            session_start_lower: "required".to_string(),
            priority: "P2".to_string(),
            summary: "test".to_string(),
            skill_path: None,
            gate_phrases: vec![],
            trigger_hints: vec![],
            name_tokens: HashSet::new(),
            keyword_tokens: HashSet::new(),
            alias_tokens: HashSet::new(),
            do_not_use_tokens: HashSet::new(),
            framework_alias_entrypoints: vec![],
            metadata_positive_triggers: vec![],
            host_platforms: vec!["cursor".to_string()],
            record_kind: "skill".to_string(),
            primary_allowed: true,
            fallback_policy_mode: "default".to_string(),
        }];
        let result = filter_records_for_host(records, Some("claude-desktop"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("host_id"));
        assert!(err.contains("claude-desktop"));
    }
}

#[cfg(test)]
mod parameter_validation_tests {
    use serde_json::json;

    #[test]
    fn goal_start_requires_goal_argument() {
        let args = json!({});
        let result =
            crate::claude_desktop_hooks::tool_goal_state_manage_test_helper(&args, "start");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("goal"));
    }

    #[test]
    fn goal_checkpoint_requires_note_argument() {
        let args = json!({});
        let result =
            crate::claude_desktop_hooks::tool_goal_state_manage_test_helper(&args, "checkpoint");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("note"));
    }

    #[test]
    fn goal_append_round_not_valid_operation() {
        let args = json!({
            "round": 1,
            "review_summary": "test",
            "fix_summary": "test",
            "verify_result": "test",
            "supervisor_decision": "test",
            "reason": "test"
        });
        let result =
            crate::claude_desktop_hooks::tool_goal_state_manage_test_helper(&args, "append_round");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("rfv_loop_manage"));
    }

    #[test]
    fn unknown_goal_operation_returns_error() {
        let args = json!({});
        let result = crate::claude_desktop_hooks::tool_goal_state_manage_test_helper(
            &args,
            "invalid_operation",
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Unknown goal operation"));
        assert!(err.contains("start") && err.contains("checkpoint"));
    }

    #[test]
    fn rfv_start_requires_goal_argument() {
        let args = json!({});
        let result = crate::claude_desktop_hooks::tool_rfv_loop_manage_test_helper(&args, "start");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("goal"));
    }

    #[test]
    fn rfv_append_round_requires_all_arguments() {
        let args = json!({"round": 1});
        let result =
            crate::claude_desktop_hooks::tool_rfv_loop_manage_test_helper(&args, "append_round");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("review_summary"));
    }

    #[test]
    fn unknown_rfv_operation_returns_error() {
        let args = json!({});
        let result = crate::claude_desktop_hooks::tool_rfv_loop_manage_test_helper(
            &args,
            "invalid_operation",
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Unknown RFV loop operation"));
        assert!(err.contains("start") && err.contains("append_round"));
    }
}

#[cfg(test)]
mod cache_ttl_tests {
    #[test]
    fn digest_cache_ttl_defaults_to_5_seconds() {
        let ttl = crate::claude_desktop_hooks::get_digest_ttl_for_test();
        assert_eq!(ttl, 5);
    }

    #[test]
    fn snapshot_cache_ttl_defaults_to_30_seconds() {
        let ttl = crate::claude_desktop_hooks::get_snapshot_ttl_for_test();
        assert_eq!(ttl, 30);
    }

    #[test]
    fn task_view_cache_ttl_defaults_to_5_seconds() {
        let ttl = crate::claude_desktop_hooks::get_task_view_ttl_for_test();
        assert_eq!(ttl, 5);
    }
}

#[cfg(test)]
mod transport_mode_read_tests {
    use std::io::{BufReader, Cursor};

    #[test]
    fn content_length_mode_detected() {
        // Body: {"jsonrpc":"2.0","id":1,"method":"ping"} = 35 bytes
        let body = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}";
        let input = format!("Content-Length: {}\r\n\r\n", body.len());
        let mut full_input = input.into_bytes();
        full_input.extend_from_slice(body);
        let cursor = Cursor::new(full_input);
        let mut reader = BufReader::new(cursor);
        let mut transport_mode = None;

        let result = crate::claude_desktop_hooks::read_mcp_message_test_helper(
            &mut reader,
            &mut transport_mode,
        );
        assert!(result.is_ok());
        assert!(transport_mode.is_some());
    }

    #[test]
    fn newline_delimited_mode_detected() {
        let input = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n";
        let cursor = Cursor::new(input.to_vec());
        let mut reader = BufReader::new(cursor);
        let mut transport_mode = None;

        let result = crate::claude_desktop_hooks::read_mcp_message_test_helper(
            &mut reader,
            &mut transport_mode,
        );
        assert!(result.is_ok());
        assert!(transport_mode.is_none());
    }

    #[test]
    fn empty_input_returns_none() {
        let input = b"";
        let cursor = Cursor::new(input.to_vec());
        let mut reader = BufReader::new(cursor);
        let mut transport_mode = None;

        let result = crate::claude_desktop_hooks::read_mcp_message_test_helper(
            &mut reader,
            &mut transport_mode,
        );
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn leading_whitespace_is_skipped() {
        let input = b"   \n   \n{\"jsonrpc\":\"2.0\"}\n";
        let cursor = Cursor::new(input.to_vec());
        let mut reader = BufReader::new(cursor);
        let mut transport_mode = None;

        let result = crate::claude_desktop_hooks::read_mcp_message_test_helper(
            &mut reader,
            &mut transport_mode,
        );
        assert!(result.is_ok());
        let msg = result.unwrap().unwrap();
        assert!(msg.contains("jsonrpc"));
    }
}

#[cfg(test)]
mod init_tracker_error_handling_tests {
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn init_tracker_failure_is_non_fatal() {
        // M6 FIX: Replace tautological assertion with actual behavior verification
        let temp_dir = std::env::temp_dir();
        let test_path = temp_dir.join("router-rs-test-non-writable").join("nested");
        let _ = std::fs::create_dir_all(&test_path);
        let _ = std::fs::set_permissions(&test_path, std::fs::Permissions::from_mode(0o444));

        // init_tracker should return an error (not panic) for non-writable directories
        let result = crate::claude_desktop_hooks::init_tracker_for_test(&test_path);

        let _ = std::fs::set_permissions(&test_path, std::fs::Permissions::from_mode(0o755));
        let _ = std::fs::remove_dir_all(&test_path.parent().unwrap());

        // The key assertion: function returned (didn't panic), result should be Err
        assert!(
            result.is_err(),
            "init_tracker should return error for non-writable path, not panic"
        );
    }
}

#[cfg(test)]
mod rate_limiter_tests {
    // M4 FIX: Add RateLimiter tests (previously 0% coverage)

    #[test]
    fn rate_limiter_allows_first_call() {
        // RateLimiter should allow the first call for any tool
        let mut limiter = crate::claude_desktop_hooks::RateLimiter::new(1000);
        let result = limiter.check_and_record("test_tool");
        assert!(result.is_ok(), "first call should be allowed");
    }

    #[test]
    fn rate_limiter_blocks_rapid_repeated_calls() {
        // RateLimiter should block calls within the minimum interval
        let mut limiter = crate::claude_desktop_hooks::RateLimiter::new(1000); // 1 second interval
        let _ = limiter.check_and_record("test_tool");
        let result = limiter.check_and_record("test_tool");
        assert!(result.is_err(), "rapid repeated calls should be blocked");
        let err = result.unwrap_err();
        assert!(err.contains("Rate limit exceeded"));
    }

    #[test]
    fn rate_limiter_allows_different_tools() {
        // Different tools should not affect each other's rate limits
        let mut limiter = crate::claude_desktop_hooks::RateLimiter::new(10000); // 10 second interval
        let _ = limiter.check_and_record("tool_a");
        let result = limiter.check_and_record("tool_b");
        assert!(
            result.is_ok(),
            "different tools should have independent rate limits"
        );
    }
}

#[cfg(test)]
mod json_parse_error_tests {
    use crate::claude_desktop_test_support;

    fn unique_test_repo_dir() -> std::path::PathBuf {
        claude_desktop_test_support::unique_temp_repo("json-parse")
    }

    // M5 FIX: Add JSON parse error path tests

    #[test]
    fn malformed_json_returns_parse_error() {
        // Test that malformed JSON returns -32700 error code
        let path = unique_test_repo_dir();
        claude_desktop_test_support::seed_minimal_current_task_layout(&path);

        let response = crate::claude_desktop_hooks::handle_mcp_request("not valid json {", &path);

        // Should return an error response
        assert!(
            response.is_some(),
            "should return a response for parse error"
        );
        let resp = response.unwrap();
        assert_eq!(resp["jsonrpc"], "2.0");
        assert!(resp.get("error").is_some(), "should have error field");
        assert_eq!(resp["error"]["code"], -32700, "should be parse error code");

        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn valid_json_missing_method_returns_error() {
        // Test that valid JSON but missing method field returns appropriate error
        let path = unique_test_repo_dir();
        claude_desktop_test_support::seed_minimal_current_task_layout(&path);

        // Missing method field
        let response =
            crate::claude_desktop_hooks::handle_mcp_request(r#"{"jsonrpc":"2.0","id":1}"#, &path);

        assert!(response.is_some());
        let resp = response.unwrap();
        // Unknown method error
        assert!(resp.get("error").is_some());
        assert_eq!(resp["error"]["code"], -32601);

        let _ = std::fs::remove_dir_all(&path);
    }
}
