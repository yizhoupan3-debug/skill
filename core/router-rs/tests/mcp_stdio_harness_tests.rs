//! Tests for Claude Desktop MCP integration.
//!
//! These tests verify the fixes applied in the Claude Desktop full-chain review.

#[cfg(test)]
mod desktop_mcp_tests {
    use crate::mcp_stdio_test_support;
    use serde_json::{Value, json};
    use std::path::PathBuf;

    fn test_repo_dir() -> PathBuf {
        let path = mcp_stdio_test_support::unique_temp_repo("mcp");
        mcp_stdio_test_support::seed_minimal_current_task_layout(&path);
        path
    }

    #[test]
    fn tools_list_matches_handler() {
        let response = crate::mcp_stdio_harness::handle_tools_list(Some(json!(1)));
        let tools = response["result"]["tools"].as_array().expect("tools array");
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();

        let handler_arms = [
            "framework_snapshot",
            "skill_route",
            "skill_search",
            "skill_read",
            "skill_route_status",
            "record_evidence",
            "session_checkpoint",
            "closeout_gate",
            "goal_state_read",
            "closeout_record_write",
            "web_fetch",
            "goal_state_manage",
            "research_aigc_check",
            "research_review_dimensions",
            "research_claim_drift",
            "research_review_loop",
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

    fn response_text(response: &Value) -> String {
        response["result"]["content"][0]["text"]
            .as_str()
            .or_else(|| response["error"]["message"].as_str())
            .unwrap_or("")
            .to_string()
    }

    #[test]
    fn web_fetch_rejects_non_http_scheme() {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "web_fetch",
                "arguments": { "url": "file:///etc/passwd" }
            }
        });
        let response = crate::mcp_stdio_harness::handle_mcp_request(
            &req.to_string(),
            &test_repo_dir(),
            "opencode",
            "test-session",
        )
        .expect("web_fetch response");
        let text = response_text(&response);
        assert!(
            text.contains("web_fetch only supports http(s)"),
            "unexpected response: {text}"
        );
    }

    #[test]
    fn mcp_stdio_harness_pre_guard_blocks_session_resume_due() {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "session_resume_due",
                "arguments": { "workerId": "w1" }
            }
        });
        let response = crate::mcp_stdio_harness::handle_mcp_request(
            &req.to_string(),
            &test_repo_dir(),
            "opencode",
            "test-session",
        )
        .expect("pre-guard response");
        let text = response_text(&response);
        assert!(
            text.contains("session_resume_due") || text.contains("stale"),
            "expected pre-guard block, got: {text}"
        );
        assert!(
            !text.contains("Unknown tool"),
            "pre-guard must block before unknown-tool handler: {text}"
        );
    }

    #[test]
    fn web_fetch_blocks_loopback() {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "web_fetch",
                "arguments": { "url": "http://127.0.0.1/" }
            }
        });
        let response = crate::mcp_stdio_harness::handle_mcp_request(
            &req.to_string(),
            &test_repo_dir(),
            "opencode",
            "test-session",
        )
        .expect("web_fetch response");
        assert_eq!(response["result"]["isError"], true);
        let text = response_text(&response);
        // Assert stable invariants: isError flag + web_fetch context.
        // Avoids fragility on exact error wording (e.g. "SSRF" vs "blocked IP" vs "private IP range").
        assert!(
            text.contains("web_fetch"),
            "expected web_fetch error context, got: {text}"
        );
    }

    #[test]
    #[ignore = "requires network; run with ROUTER_RS_WEB_FETCH_NETWORK=1"]
    fn web_fetch_fetches_https_example() {
        if std::env::var_os("ROUTER_RS_WEB_FETCH_NETWORK").is_none() {
            eprintln!("skip: set ROUTER_RS_WEB_FETCH_NETWORK=1 to run network web_fetch test");
            return;
        }
        let req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "web_fetch",
                "arguments": { "url": "https://example.com" }
            }
        });
        let response = crate::mcp_stdio_harness::handle_mcp_request(
            &req.to_string(),
            &test_repo_dir(),
            "opencode",
            "test-session",
        )
        .expect("web_fetch response");
        let text = response_text(&response);
        assert!(text.contains("\"status\": 200"), "expected 200: {text}");
        assert!(text.contains("Example Domain"), "expected body: {text}");
    }

    #[test]
    fn closeout_gate_requires_session_summary_file() {
        let repo = test_repo_dir();
        // Pointer 机制已移除，需显式传 task_id
        let out = crate::mcp_stdio_harness::tool_closeout_gate(
            &json!({"task_id": "no-summary-task"}),
            &repo,
            "opencode",
        )
        .expect("closeout_gate");
        assert!(
            out.contains("BLOCKED") || out.contains("checkpoint: missing"),
            "without SESSION_SUMMARY must not PASS: {out}"
        );
        assert!(!out.starts_with("[Closeout Gate] PASS"));
    }

    #[test]
    fn session_summary_resource_rejects_path_traversal_task_id() {
        let repo = test_repo_dir();
        std::fs::write(repo.join("ESCAPED.md"), "escaped-content").expect("marker");
        std::fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"../../ESCAPED"}"#,
        )
        .expect("active_task");
        let response = crate::mcp_stdio_harness::handle_mcp_request(
            r#"{"jsonrpc":"2.0","id":1,"method":"resources/read","params":{"uri":"framework://session_summary"}}"#,
            &repo,
            "opencode",
            "test-session",
        )
        .expect("resources/read");
        let text = response["result"]["contents"][0]["text"]
            .as_str()
            .expect("summary text");
        assert!(
            text.contains("Test Session"),
            "must read artifacts/current/SESSION_SUMMARY.md, not repo escape: {text}"
        );
        assert!(
            !text.contains("escaped-content"),
            "path traversal via task_id: {text}"
        );
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn review_gate_prompt_mentions_reviewer_lanes_and_explore() {
        let repo = test_repo_dir();
        let response = crate::mcp_stdio_harness::handle_mcp_request(
            r#"{"jsonrpc":"2.0","id":1,"method":"prompts/get","params":{"name":"review_gate"}}"#,
            &repo,
            "opencode",
            "test-session",
        )
        .expect("prompts/get response");
        let text = response["result"]["messages"][0]["content"]["text"]
            .as_str()
            .expect("prompt text");
        assert!(
            text.contains("reviewer_lanes"),
            "expected reviewer_lanes in review_gate prompt: {text}"
        );
        assert!(
            text.contains("explore"),
            "expected explore exclusion in review_gate prompt: {text}"
        );
        assert!(
            text.contains("fork_context=false"),
            "expected fork_context=false in review_gate prompt: {text}"
        );
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn framework_routing_prompt_inlines_simplified_chinese_language_policy() {
        let repo = test_repo_dir();
        let response = crate::mcp_stdio_harness::handle_mcp_request(
            r#"{"jsonrpc":"2.0","id":1,"method":"prompts/get","params":{"name":"framework_routing"}}"#,
            &repo,
            "opencode",
            "test-session",
        )
        .expect("prompts/get response");
        let text = response["result"]["messages"][0]["content"]["text"]
            .as_str()
            .expect("prompt text");
        assert!(
            text.contains("简体中文"),
            "framework_routing prompt must inline language policy: {text}"
        );
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn closeout_gate_warns_when_goal_suggests_review_without_evidence() {
        let repo = test_repo_dir();
        let task_id = "test-task";
        let task_dir = repo.join("artifacts/current").join(task_id);
        std::fs::create_dir_all(&task_dir).unwrap();
        std::fs::write(
            task_dir.join("GOAL_STATE.json"),
            r#"{"schema_version":"router-rs-goal-v1","status":"running","goal":"深度 review 这个 PR"}"#,
        )
        .unwrap();

        let out = crate::mcp_stdio_harness::tool_closeout_gate(
            &json!({"task_id": task_id}),
            &repo,
            "opencode",
        )
        .expect("closeout_gate");
        assert!(
            out.contains("no hook REVIEW_GATE"),
            "expected static Desktop advisory: {out}"
        );
        assert!(
            out.contains("WARN: review_gate: GOAL suggests review work"),
            "expected review WARN when goal arms review without attestation: {out}"
        );

        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn closeout_gate_accepts_review_lanes_markdown_on_disk() {
        let repo = test_repo_dir();
        let task_id = "test-task";
        let task_dir = repo.join("artifacts/current").join(task_id);
        let review_lanes = task_dir.join("review-lanes");
        std::fs::create_dir_all(&review_lanes).unwrap();
        std::fs::write(
            task_dir.join("GOAL_STATE.json"),
            r#"{"schema_version":"router-rs-goal-v1","status":"running","goal":"深度 review 这个 PR"}"#,
        )
        .unwrap();
        std::fs::write(review_lanes.join("lane-a.md"), "[P2] example — ok").unwrap();

        let out = crate::mcp_stdio_harness::tool_closeout_gate(
            &json!({"task_id": task_id}),
            &repo,
            "opencode",
        )
        .expect("closeout_gate");
        assert!(
            !out.contains("WARN: review_gate: GOAL suggests review work"),
            "review-lanes evidence should clear review WARN: {out}"
        );
        assert!(
            out.contains("reviewer evidence attested"),
            "expected review-lanes acknowledgement: {out}"
        );

        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn closeout_gate_warns_when_evidence_is_only_self_attested() {
        let repo = test_repo_dir();
        let task_id = "test-task";
        let task_dir = repo.join("artifacts/current").join(task_id);
        std::fs::create_dir_all(&task_dir).unwrap();
        std::fs::write(
            task_dir.join("EVIDENCE_INDEX.json"),
            r#"{"artifacts":[{"kind":"mcp_record_evidence","source":"mcp_record_evidence","success":true,"command_preview":"cargo test"}]}"#,
        )
        .unwrap();
        std::fs::write(
            task_dir.join("GOAL_STATE.json"),
            r#"{"schema_version":"router-rs-goal-v1","status":"running","goal":"test"}"#,
        )
        .unwrap();

        let out = crate::mcp_stdio_harness::tool_closeout_gate(
            &json!({"task_id": task_id}),
            &repo,
            "opencode",
        )
        .expect("closeout_gate");
        assert!(
            out.contains("WARN: evidence: only self-attested"),
            "expected self-attest warning in: {out}"
        );

        let _ = std::fs::remove_dir_all(&repo);
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
        let entry_result = crate::mcp_stdio_harness::build_evidence_entry(&args_with);
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
        let entry_without = crate::mcp_stdio_harness::build_evidence_entry(&args_without).unwrap();
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
        let result = crate::mcp_stdio_harness::tool_closeout_record_write_for_test(&args, &repo);
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
        assert!(
            parsed["schema_version"]
                .as_str()
                .unwrap()
                .contains("closeout-record")
        );

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
        let result = crate::mcp_stdio_harness::tool_closeout_record_write_for_test(&args, &repo);
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
        // M7: Content-Length transport mode test
        use std::io::{BufReader, Cursor};

        // Body: {"jsonrpc":"2.0","id":1,"method":"ping"} = 35 bytes
        let body = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}";
        let input = format!("Content-Length: {}\r\nX-Custom: value\r\n\r\n", body.len());
        let mut full_input = input.into_bytes();
        full_input.extend_from_slice(body);
        let cursor = Cursor::new(full_input);
        let mut reader = BufReader::new(cursor);
        let mut transport_mode = None;

        let result = crate::mcp_stdio_harness::read_mcp_message_test_helper(
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
mod routing_tests {
    use std::collections::HashSet;

    #[test]
    fn filter_warning_logged_when_all_filtered() {
        use crate::route::SkillRecord;
        use crate::route::filter_records_for_host;

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
            skill_flags: vec![],
        }];
        let result = filter_records_for_host(records, Some("retired-claude-desktop"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("host_id"));
        assert!(err.contains("retired-claude-desktop"));
    }
}

#[cfg(test)]
mod parameter_validation_tests {
    use serde_json::json;

    #[test]
    fn goal_start_requires_goal_argument() {
        let args = json!({"task_id": "t1"});
        let result = crate::mcp_stdio_harness::tool_goal_state_manage_test_helper(&args, "start");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("goal"),
            "expected error about 'goal', got: {err}"
        );
    }

    #[test]
    fn goal_checkpoint_requires_note_argument() {
        let args = json!({"task_id": "t1"});
        let result =
            crate::mcp_stdio_harness::tool_goal_state_manage_test_helper(&args, "checkpoint");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("note"),
            "expected error about 'note', got: {err}"
        );
    }

    #[test]
    fn goal_append_round_not_valid_operation() {
        let args = json!({
            "task_id": "t1",
            "round": 1,
            "review_summary": "test",
            "fix_summary": "test",
            "verify_result": "test",
            "supervisor_decision": "test",
            "reason": "test"
        });
        let result =
            crate::mcp_stdio_harness::tool_goal_state_manage_test_helper(&args, "append_round");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("Unknown goal operation"),
            "expected 'Unknown goal operation', got: {err}"
        );
        assert!(err.contains("start") && err.contains("checkpoint"));
    }

    #[test]
    fn goal_start_requires_task_id_explicitly() {
        let args = json!({"goal": "test goal"});
        let result = crate::mcp_stdio_harness::tool_goal_state_manage_test_helper(&args, "start");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("task_id"),
            "Expected task_id error, got: {err}"
        );
    }

    #[test]
    fn goal_checkpoint_requires_task_id_explicitly() {
        let args = json!({"note": "checkpoint note"});
        let result =
            crate::mcp_stdio_harness::tool_goal_state_manage_test_helper(&args, "checkpoint");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("task_id"),
            "Expected task_id error, got: {err}"
        );
    }

    #[test]
    fn goal_block_requires_blocker() {
        let args = json!({"task_id": "test-task"});
        let result = crate::mcp_stdio_harness::tool_goal_state_manage_test_helper(&args, "block");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("blocker"),
            "Expected blocker error, got: {err}"
        );
    }

    #[test]
    fn goal_block_operation_accepted_with_blocker() {
        let args = json!({"task_id": "test-task", "blocker": "dependency conflict"});
        let result = crate::mcp_stdio_harness::tool_goal_state_manage_test_helper(&args, "block");
        if let Err(ref err) = result {
            assert!(
                !err.contains("Missing required argument: blocker"),
                "blocker validation broken: {err}"
            );
        }
    }

    #[test]
    fn goal_start_with_explicit_task_id_succeeds() {
        let args = json!({
            "task_id": "cache-test-task",
            "goal": "test goal for cache invalidation",
            "drive_until_done": false
        });
        let result = crate::mcp_stdio_harness::tool_goal_state_manage_test_helper(&args, "start");
        assert!(
            result.is_ok(),
            "start with explicit task_id failed: {:?}",
            result.err()
        );
    }
}

#[cfg(test)]
mod cache_ttl_tests {
    #[test]
    fn snapshot_cache_ttl_defaults_to_30_seconds() {
        let ttl = crate::mcp_stdio_harness::get_snapshot_ttl_for_test();
        assert_eq!(ttl, 30);
    }

    #[test]
    fn task_view_cache_ttl_defaults_to_5_seconds() {
        let ttl = crate::mcp_stdio_harness::get_task_view_ttl_for_test();
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

        let result = crate::mcp_stdio_harness::read_mcp_message_test_helper(
            &mut reader,
            &mut transport_mode,
        );
        if let Err(ref e) = result {
            eprintln!("ERROR: {}", e);
        }
        assert!(result.is_ok());
        assert!(transport_mode.is_some());
    }

    #[test]
    fn newline_delimited_mode_detected() {
        let input = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n";
        let cursor = Cursor::new(input.to_vec());
        let mut reader = BufReader::new(cursor);
        let mut transport_mode = None;

        let result = crate::mcp_stdio_harness::read_mcp_message_test_helper(
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

        let result = crate::mcp_stdio_harness::read_mcp_message_test_helper(
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

        let result = crate::mcp_stdio_harness::read_mcp_message_test_helper(
            &mut reader,
            &mut transport_mode,
        );
        assert!(result.is_ok());
        let msg = result.unwrap().unwrap();
        assert!(msg.contains("jsonrpc"));
    }
}

#[cfg(test)]
mod rate_limiter_tests {
    // M4: RateLimiter tests

    #[test]
    fn rate_limiter_allows_first_call() {
        // RateLimiter should allow the first call for any tool
        let mut limiter = crate::mcp_stdio_harness::RateLimiter::new(1000);
        let result = limiter.check_and_record("test_tool");
        assert!(result.is_ok(), "first call should be allowed");
    }

    #[test]
    fn rate_limiter_blocks_rapid_repeated_calls() {
        // RateLimiter should block calls within the minimum interval
        let mut limiter = crate::mcp_stdio_harness::RateLimiter::new(1000); // 1 second interval
        let _ = limiter.check_and_record("test_tool");
        let result = limiter.check_and_record("test_tool");
        assert!(result.is_err(), "rapid repeated calls should be blocked");
        let err = result.unwrap_err();
        assert!(err.contains("Rate limit exceeded"));
    }

    #[test]
    fn rate_limiter_allows_different_tools() {
        // Different tools should not affect each other's rate limits
        let mut limiter = crate::mcp_stdio_harness::RateLimiter::new(10000); // 10 second interval
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
    use crate::mcp_stdio_test_support;

    fn unique_test_repo_dir() -> std::path::PathBuf {
        mcp_stdio_test_support::unique_temp_repo("json-parse")
    }

    // M5: JSON parse error path tests

    #[test]
    fn malformed_json_returns_parse_error() {
        // Test that malformed JSON returns -32700 error code
        let path = unique_test_repo_dir();
        mcp_stdio_test_support::seed_minimal_current_task_layout(&path);

        let response = crate::mcp_stdio_harness::handle_mcp_request(
            "not valid json {",
            &path,
            "opencode",
            "test-session",
        );

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
        mcp_stdio_test_support::seed_minimal_current_task_layout(&path);

        // Missing method field
        let response = crate::mcp_stdio_harness::handle_mcp_request(
            r#"{"jsonrpc":"2.0","id":1}"#,
            &path,
            "opencode",
            "test-session",
        );

        assert!(response.is_some());
        let resp = response.unwrap();
        // Unknown method error
        assert!(resp.get("error").is_some());
        assert_eq!(resp["error"]["code"], -32601);

        let _ = std::fs::remove_dir_all(&path);
    }
}

#[cfg(test)]
mod claude_desktop_hard_blocking_tests {
    use crate::mcp_stdio_test_support;
    use serde_json::json;
    use std::path::PathBuf;

    fn test_repo_dir() -> PathBuf {
        let path = mcp_stdio_test_support::unique_temp_repo("desktop-mcp-hard");
        mcp_stdio_test_support::seed_minimal_current_task_layout(&path);
        path
    }

    #[test]
    fn desktop_closeout_gate_advisory_unsatisfied_by_default() {
        let repo = test_repo_dir();
        // Advisory-only mode: closeout_gate reports findings but does not hard-block.
        let req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "closeout_gate",
                "arguments": {}
            }
        });
        let response = crate::mcp_stdio_harness::handle_mcp_request(
            &req.to_string(),
            &repo,
            "opencode",
            "test-session",
        )
        .expect("response");
        assert!(!response["result"]["isError"].as_bool().unwrap_or(false));
        let text = response["result"]["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains("ADVISORY"),
            "expected ADVISORY verdict; got {text}"
        );
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn desktop_allows_unsatisfied_under_task_profile() {
        let repo = test_repo_dir();
        let task_id = "desktop-task";
        std::fs::write(
            repo.join("artifacts/current/active_task.json"),
            format!(r#"{{"task_id":"{task_id}"}}"#),
        )
        .unwrap();
        let task_dir = repo.join("artifacts/current").join(task_id);
        std::fs::create_dir_all(&task_dir).unwrap();
        std::fs::write(
            task_dir.join("GOAL_STATE.json"),
            r#"{"schema_version":"router-rs-goal-v1","status":"running","lifecycle_profile":"task","goal":"x"}"#,
        )
        .unwrap();
        let req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "closeout_gate",
                "arguments": { "task_id": task_id }
            }
        });
        let response = crate::mcp_stdio_harness::handle_mcp_request(
            &req.to_string(),
            &repo,
            "opencode",
            "test-session",
        )
        .expect("response");
        assert!(!response["result"]["isError"].as_bool().unwrap_or(false));
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn desktop_goal_state_manage_complete_advisory_unsatisfied() {
        let repo = test_repo_dir();
        // Advisory mode: no hard block, but task_id validation still applies
        let req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "goal_state_manage",
                "arguments": { "operation": "complete" }
            }
        });
        let response = crate::mcp_stdio_harness::handle_mcp_request(
            &req.to_string(),
            &repo,
            "opencode",
            "test-session",
        )
        .expect("response");
        let error_msg = response["result"]["content"][0]["text"].as_str().unwrap();
        assert!(
            !error_msg.contains("[Claude Desktop Hard Block]"),
            "should NOT contain hard block message in advisory mode; got {error_msg}"
        );
        assert!(
            error_msg.contains("task_id")
                || error_msg.contains("GOAL_STATE")
                || !response["result"]["isError"].as_bool().unwrap_or(false),
            "expected task_id/GOAL_STATE validation or success (not hard block); got {error_msg}"
        );
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn desktop_review_goal_without_evidence_advisory() {
        let repo = test_repo_dir();
        let task_id = "desktop-strict-review";
        std::fs::write(
            repo.join("artifacts/current/active_task.json"),
            format!(r#"{{"task_id":"{task_id}"}}"#),
        )
        .unwrap();
        let task_dir = repo.join("artifacts/current").join(task_id);
        std::fs::create_dir_all(&task_dir).unwrap();
        std::fs::write(
            task_dir.join("GOAL_STATE.json"),
            r#"{
                "schema_version": "router-rs-goal-v1",
                "status": "running",
                "lifecycle_profile": "strict",
                "goal": "深度 review 这个 PR",
                "non_goals": [],
                "done_when": [],
                "validation_commands": [],
                "checkpoints": []
            }"#,
        )
        .unwrap();
        std::fs::write(
            task_dir.join("EVIDENCE_INDEX.json"),
            r#"{"artifacts":[{"kind":"mcp_record_evidence","source":"mcp_record_evidence","success":true,"command_preview":"cargo test"}]}"#,
        )
        .unwrap();
        std::fs::write(task_dir.join("SESSION_SUMMARY.md"), "summary").unwrap();
        let req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "goal_state_manage",
                "arguments": { "task_id": task_id, "operation": "complete" }
            }
        });
        let response = crate::mcp_stdio_harness::handle_mcp_request(
            &req.to_string(),
            &repo,
            "opencode",
            "test-session",
        )
        .expect("response");
        // Advisory mode: NOT hard-blocked even with review goal and no reviewer evidence
        assert!(
            !response["result"]["isError"].as_bool().unwrap_or(false),
            "advisory mode should not block; got {:?}",
            response["result"]["content"][0]["text"]
        );
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn desktop_closeout_record_write_includes_mcp_closeout_gate_field() {
        let repo = test_repo_dir();
        let task_id = "test-task";
        let req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "closeout_record_write",
                "arguments": {
                    "task_id": task_id,
                    "summary": "done",
                    "verification_status": "passed"
                }
            }
        });
        let response = crate::mcp_stdio_harness::handle_mcp_request(
            &req.to_string(),
            &repo,
            "opencode",
            "test-session",
        )
        .expect("response");
        assert!(!response["result"]["isError"].as_bool().unwrap_or(false));
        let text = response["result"]["content"][0]["text"].as_str().unwrap();
        let payload: serde_json::Value = serde_json::from_str(text).expect("json");
        assert!(
            payload.get("closeout_allowed").is_some(),
            "closeout_record_write should return closeout_allowed field; got: {payload}"
        );
        let _ = std::fs::remove_dir_all(&repo);
    }
}

#[cfg(test)]
mod claude_desktop_stdio_e2e_tests {
    use std::io::Write;
    use std::process::{Command, Stdio};

    fn router_rs_bin() -> std::path::PathBuf {
        crate::router_self::resolve_router_rs_test_bin()
    }

    #[test]
    fn stdio_initialize_tools_list_and_skill_route() {
        let bin = router_rs_bin();
        let repo = std::env::current_dir()
            .unwrap()
            .ancestors()
            .find(|p| {
                p.join("configs/framework/RUNTIME_REGISTRY.json").is_file()
                    && p.join("core/router-rs/Cargo.toml").is_file()
            })
            .expect("framework repo root")
            .to_path_buf();

        let mut child = Command::new(bin)
            .args([
                "host",
                "agent",
                "opencode",
                "--repo-root",
                repo.to_str().unwrap(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn MCP agent");

        let stdin = child.stdin.as_mut().expect("stdin");
        let requests = [
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"skill_route","arguments":{"query":"gitx"}}}"#,
        ];
        for line in requests {
            writeln!(stdin, "{line}").expect("write stdin");
        }
        drop(child.stdin.take());

        let output = child.wait_with_output().expect("wait");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stdout.contains("framework_snapshot"),
            "tools/list missing framework_snapshot; stdout={stdout} stderr={stderr}"
        );
        assert!(
            stdout.contains("gitx") && stdout.contains("routed"),
            "skill_route missing gitx route; stdout={stdout} stderr={stderr}"
        );
    }
}
