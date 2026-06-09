use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};

use super::*;

    static TEMP_REPO_SEQ: AtomicU64 = AtomicU64::new(0);

    fn unique_temp_repo(prefix: &str) -> PathBuf {
        let seq = TEMP_REPO_SEQ.fetch_add(1, Ordering::Relaxed);
        let mut path = std::env::temp_dir();
        path.push(format!("router-rs-mcp-common-{prefix}-{}-{seq}", std::process::id()));
        path
    }

    fn unique_test_repo(name: &str) -> PathBuf {
        let path = unique_temp_repo(&format!("mcp-{name}"));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn initialize_returns_capabilities() {
        let response = handle_initialize(Some(json!(1)));
        let result = &response["result"];
        assert_eq!(result["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(result["serverInfo"]["name"], SERVER_NAME);
        let caps = &result["capabilities"];
        assert!(caps.get("tools").is_some());
        assert!(caps.get("prompts").is_some());
        assert!(caps.get("resources").is_some());
    }

    #[test]
    fn tools_list_returns_all_expected_tools() {
        let response = handle_tools_list(Some(json!(1)));
        let tools = response["result"]["tools"].as_array().expect("tools array");
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names.len(), 9, "expected 8 tools, got: {:?}", names);
        for tool in &[
            "framework_snapshot",
            "skill_route",
            "record_evidence",
            "session_checkpoint",
            "closeout",
            "web_fetch",
            "goal_state",
            "rfv_loop",
            "web_search",
        ] {
            assert!(names.contains(tool), "missing tool: {tool}");
        }
    }

    #[test]
    fn prompts_list_returns_all_expected_prompts() {
        let response = handle_prompts_list(Some(json!(1)));
        let prompts = response["result"]["prompts"]
            .as_array()
            .expect("prompts array");
        let names: Vec<&str> = prompts
            .iter()
            .map(|p| p["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"framework_routing"));
        assert!(names.contains(&"review_gate"));
        assert!(names.contains(&"closeout_checklist"));
    }

    #[test]
    fn ping_returns_empty_result() {
        let response = handle_mcp_request(
            r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
            &unique_test_repo("ping"),
            "opencode",
        )
        .unwrap();
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 1);
        assert!(response.get("error").is_none());
    }

    #[test]
    fn unknown_method_returns_error() {
        let response = handle_mcp_request(
            r#"{"jsonrpc":"2.0","id":2,"method":"nonexistent"}"#,
            &unique_test_repo("unknown-method"),
            "opencode",
        )
        .unwrap();
        assert_eq!(response["error"]["code"], -32601);
        assert!(response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("nonexistent"));
    }

    #[test]
    fn parse_content_length_normal() {
        assert_eq!(parse_content_length("Content-Length: 42").unwrap(), 42);
    }

    #[test]
    fn parse_content_length_with_crlf() {
        assert_eq!(parse_content_length("Content-Length: 100
").unwrap(), 100);
    }

    #[test]
    fn parse_content_length_with_ows() {
        // "Content-Length :" with space before colon (RFC 7230 OWS)
        assert_eq!(parse_content_length("Content-Length : 50").unwrap(), 50);
    }

    #[test]
    fn parse_content_length_case_insensitive() {
        assert_eq!(parse_content_length("content-length: 7").unwrap(), 7);
    }

    #[test]
    fn parse_content_length_rejects_empty() {
        assert!(parse_content_length("Content-Length: ").is_err());
    }

    #[test]
    fn parse_content_length_rejects_non_numeric() {
        assert!(parse_content_length("Content-Length: abc").is_err());
    }

    #[test]
    fn parse_content_length_rejects_negative() {
        assert!(parse_content_length("Content-Length: -1").is_err());
    }

    #[test]
    fn parse_content_length_rejects_missing_header() {
        assert!(parse_content_length("X-Other: 42").is_err());
    }

    #[test]
    fn parse_content_length_large_value() {
        assert_eq!(
            parse_content_length("Content-Length: 1048576").unwrap(),
            1_048_576
        );
    }

    #[test]
    fn merged_goal_state_read_via_action() {
        let repo = unique_test_repo("goal-state-read");
        let out = tool_goal_state(
            &serde_json::json!({"action": "read"}),
            &repo,
            "goal_state",
        )
        .expect("read");
        assert!(out.contains("goal_state") || out.contains("null") || out.starts_with('{'));
        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn legacy_goal_state_read_alias_still_works() {
        let repo = unique_test_repo("goal-state-read-legacy");
        assert!(tool_goal_state(&serde_json::json!({}), &repo, "goal_state_read").is_ok());
        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn merged_rfv_loop_status_default() {
        let repo = unique_test_repo("rfv-status");
        let out = tool_rfv_loop(&serde_json::json!({}), &repo, "rfv_loop").expect("status");
        assert!(out.starts_with('{') || out.contains("null"));
        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn merged_closeout_gate_default_action() {
        let repo = unique_test_repo("closeout-gate");
        let out = tool_closeout(&serde_json::json!({}), &repo, "opencode", "closeout")
            .expect("gate");
        assert!(out.contains("closeout") || out.contains("Closeout") || out.contains("GOAL"));
        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn legacy_closeout_gate_alias_still_works() {
        let repo = unique_test_repo("closeout-gate-legacy");
        assert!(tool_closeout_gate(&serde_json::json!({}), &repo, "opencode").is_ok());
        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn resolve_active_skill_dir_from_planx_prompt() {
        let dir = router_rs::hook_common::path_guard::resolve_active_skill_dir_from_prompt(
            Path::new("/repo"),
            "please run /planx for this task",
        );
        assert_eq!(dir.as_deref(), Some("skills/planx/"));
    }

    fn framework_repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("repo root")
    }

    fn routing_fixture_repo(name: &str) -> PathBuf {
        let repo = unique_test_repo(name);
        let skill_root = framework_repo_root();
        std::fs::create_dir_all(repo.join("skills")).expect("skills dir");
        std::fs::copy(
            skill_root.join("skills/SKILL_ROUTING_RUNTIME.json"),
            repo.join("skills/SKILL_ROUTING_RUNTIME.json"),
        )
        .expect("routing runtime");
        std::fs::copy(
            skill_root.join("skills/SKILL_MANIFEST.json"),
            repo.join("skills/SKILL_MANIFEST.json"),
        )
        .expect("skill manifest");
        repo
    }

    fn setup_task_repo(name: &str, task_id: &str) -> PathBuf {
        let repo = unique_test_repo(name);
        let current = repo.join(router_rs::goal_state::ARTIFACTS_CURRENT_DIR);
        let task_dir = current.join(task_id);
        std::fs::create_dir_all(&task_dir).expect("task dir");
        std::fs::write(
            current.join("active_task.json"),
            format!(r#"{{"task_id":"{task_id}"}}"#),
        )
        .expect("active task pointer");
        std::fs::write(
            current.join("TASK_POINTERS.json"),
            format!(
                r#"{{"active_task_id":"{task_id}","focus_task_id":"{task_id}"}}"#
            ),
        )
        .expect("task pointers");
        std::fs::write(
            task_dir.join("SESSION_SUMMARY.md"),
            "# session\n\ncontinuity seed\n",
        )
        .expect("session summary");
        repo
    }

    fn mcp_tools_call(repo: &Path, host_id: &str, tool: &str, arguments: Value) -> Value {
        let request = serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "id": 42,
            "method": "tools/call",
            "params": { "name": tool, "arguments": arguments },
        }))
        .expect("serialize tools/call");
        handle_mcp_request(&request, repo, host_id).expect("mcp response")
    }

    fn mcp_tool_ok_text(response: &Value) -> String {
        assert!(
            response.get("error").is_none(),
            "unexpected jsonrpc error: {response}"
        );
        let is_error = response["result"]["isError"].as_bool().unwrap_or(false);
        assert!(!is_error, "tool error: {}", tool_text(response));
        tool_text(response).to_string()
    }

    fn tool_text(response: &Value) -> &str {
        response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or("")
    }

    #[test]
    fn tools_call_framework_snapshot_returns_envelope() {
        let repo = unique_test_repo("snapshot-envelope");
        let text = tool_framework_snapshot(&repo).expect("snapshot");
        let envelope: Value = serde_json::from_str(&text).expect("snapshot json");
        assert!(
            envelope.get("runtime_snapshot").is_some()
                || envelope.get("runtime_view").is_some()
        );
        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn tools_call_skill_route_missing_query_errors() {
        let repo = framework_repo_root();
        let response = mcp_tools_call(&repo, "opencode", "skill_route", json!({}));
        assert_eq!(response["result"]["isError"], json!(true));
        assert!(tool_text(&response).contains("Missing required argument: query"));
    }

    #[test]
    fn tools_call_skill_route_matches_swarm_orchestration() {
        let repo = routing_fixture_repo("skill-route-swarm");
        let text = tool_skill_route(
            &json!({"query": "需要多 agent 执行，先判断是否应该拆 bounded subagent sidecar"}),
            &repo,
            "opencode",
            true,
        )
        .expect("route");
        let body: Value = serde_json::from_str(&text).expect("route json");
        assert_eq!(body["routed"], json!(true));
        assert!(
            body["skill_slug"].as_str() == Some("agent-swarm-orchestration")
                || body["skill_slug"].as_str() == Some("workflow"),
            "unexpected slug: {}",
            body["skill_slug"]
        );
        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn build_evidence_entry_requires_tool_name() {
        let err = build_evidence_entry(&json!({"command": "cargo test"})).unwrap_err();
        assert!(err.to_string().contains("tool_name"));
    }

    #[test]
    fn build_evidence_entry_success_shape() {
        let entry = build_evidence_entry(&json!({
            "tool_name": "Shell",
            "command": "cargo test -p router-rs --lib",
            "exit_code": 0,
            "output": "ok",
        }))
        .expect("entry");
        assert_eq!(entry.get("tool_name").and_then(Value::as_str), Some("Shell"));
        assert_eq!(entry.get("success").and_then(Value::as_bool), Some(true));
    }

    #[test]
    fn record_evidence_appends_with_active_task() {
        let repo = setup_task_repo("record-evidence", "ev-task");
        let out = tool_record_evidence(
            &json!({
                "tool_name": "Shell",
                "command": "cargo test -q",
                "exit_code": 0,
            }),
            &repo,
        )
        .expect("record");
        assert!(out.contains("Evidence recorded"));
        let evidence_path = repo
            .join(router_rs::goal_state::ARTIFACTS_CURRENT_DIR)
            .join("ev-task")
            .join("EVIDENCE_INDEX.json");
        assert!(evidence_path.is_file(), "expected evidence index at {}", evidence_path.display());
        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn session_checkpoint_requires_summary() {
        let repo = unique_test_repo("checkpoint-no-summary");
        let err = tool_session_checkpoint(&json!({}), &repo).unwrap_err();
        assert!(err.to_string().contains("summary"));
        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn session_checkpoint_writes_summary() {
        let repo = setup_task_repo("checkpoint-write", "cp-task");
        let out = tool_session_checkpoint(
            &json!({
                "summary": "checkpoint after wave 1",
                "next_actions": ["run verify"],
                "task_id": "cp-task",
            }),
            &repo,
        )
        .expect("checkpoint");
        assert!(out.contains("Checkpoint written"));
        let summary = repo
            .join(router_rs::goal_state::ARTIFACTS_CURRENT_DIR)
            .join("cp-task")
            .join("SESSION_SUMMARY.md");
        assert!(summary.is_file());
        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn web_fetch_missing_url_errors() {
        let err = tool_web_fetch(&json!({})).unwrap_err();
        assert!(err.to_string().contains("url"));
    }

    #[test]
    fn web_fetch_rejects_loopback_url() {
        let err = tool_web_fetch(&json!({"url": "http://127.0.0.1/"})).unwrap_err();
        assert!(err.to_string().contains("web_fetch"));
    }

    #[test]
    fn web_fetch_rejects_private_literal() {
        let err = tool_web_fetch(&json!({"url": "http://10.0.0.1/secret"})).unwrap_err();
        assert!(err.to_string().contains("blocked") || err.to_string().contains("web_fetch"));
    }

    #[test]
    fn web_fetch_rejects_non_http_scheme() {
        let err = tool_web_fetch(&json!({"url": "file:///etc/passwd"})).unwrap_err();
        assert!(err.to_string().contains("http"));
    }

    #[test]
    fn goal_state_start_and_read_roundtrip() {
        let repo = setup_task_repo("goal-start", "g-task");
        tool_goal_state(
            &json!({
                "operation": "start",
                "task_id": "g-task",
                "goal": "harden mcp goal_state",
                "non_goals": ["scope creep"],
                "done_when": ["tests pass", "mcp roundtrip ok"],
                "validation_commands": ["cargo test -q"],
            }),
            &repo,
            "goal_state",
        )
        .expect("start");
        let read = tool_goal_state(&json!({"action": "read"}), &repo, "goal_state").expect("read");
        assert!(read.contains("harden mcp goal_state"));
        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn rfv_loop_mcp_start_and_status() {
        let repo = setup_task_repo("rfv-mcp", "rfv-mcp-task");
        tool_rfv_loop(
            &json!({
                "operation": "start",
                "task_id": "rfv-mcp-task",
                "goal": "rfv via mcp",
                "max_rounds": 3u64,
            }),
            &repo,
            "rfv_loop",
        )
        .expect("start");
        let status = tool_rfv_loop(&json!({}), &repo, "rfv_loop").expect("status");
        assert!(status.contains("rfv-mcp-task") || status.contains("active"));
        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn rfv_loop_mcp_append_round_rejects_invalid_verify() {
        let repo = setup_task_repo("rfv-mcp-vr", "rfv-vr");
        tool_rfv_loop(
            &json!({
                "operation": "start",
                "task_id": "rfv-vr",
                "goal": "verify enum",
                "max_rounds": 2u64,
            }),
            &repo,
            "rfv_loop",
        )
        .expect("start");
        let err = tool_rfv_loop(
            &json!({
                "operation": "append_round",
                "round": 1u64,
                "review_summary": "r1",
                "fix_summary": "f1",
                "supervisor_decision": "continue",
                "reason": "invalid verify enum probe",
                "verify_result": "kinda passed",
            }),
            &repo,
            "rfv_loop",
        )
        .expect_err("invalid verify_result");
        assert!(err.to_string().contains("verify_result must be one of"), "unexpected err: {err}");
        let _ = std::fs::remove_dir_all(repo);
    }

    /// I2: MCP `rfv_loop` path blocks PASS without window evidence by default.
    #[test]
    fn rfv_loop_mcp_blocks_pass_without_evidence_by_default() {
        let repo = setup_task_repo("rfv-mcp-i2", "rfv-i2");
        tool_rfv_loop(
            &json!({
                "operation": "start",
                "task_id": "rfv-i2",
                "goal": "mcp cross-link block",
                "max_rounds": 3u64,
            }),
            &repo,
            "rfv_loop",
        )
        .expect("start");
        let err = tool_rfv_loop(
            &json!({
                "operation": "append_round",
                "round": 1u64,
                "review_summary": "r1",
                "fix_summary": "f1",
                "supervisor_decision": "continue",
                "reason": "pass without evidence",
                "verify_result": "PASS",
            }),
            &repo,
            "rfv_loop",
        )
        .expect_err("must block PASS without evidence via MCP");
        assert!(
            err.to_string().contains("no_evidence_window"),
            "unexpected err: {err}"
        );
        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn rfv_loop_legacy_status_alias_still_works() {
        let repo = unique_test_repo("rfv-legacy-alias");
        assert!(tool_rfv_loop(&json!({}), &repo, "rfv_loop_status").is_ok());
        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn closeout_record_write_requires_task_id() {
        let repo = unique_test_repo("closeout-rw");
        let err = tool_closeout(
            &json!({"action": "record_write", "summary": "x", "verification_status": "pass"}),
            &repo,
            "opencode",
            "closeout",
        )
        .unwrap_err();
        assert!(err.to_string().contains("task_id"));
        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn tools_call_unknown_tool_errors() {
        let repo = unique_test_repo("unknown-tool");
        let response = mcp_tools_call(&repo, "opencode", "not_a_real_tool", json!({}));
        assert_eq!(response["result"]["isError"], json!(true));
        assert!(tool_text(&response).contains("Unknown tool"));
        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn resources_list_includes_active_task_uri() {
        let repo = unique_test_repo("resources-list");
        let response = handle_resources_list(Some(json!(1)), &repo);
        let uris: Vec<&str> = response["result"]["resources"]
            .as_array()
            .expect("resources")
            .iter()
            .filter_map(|r| r.get("uri").and_then(Value::as_str))
            .collect();
        assert!(uris.contains(&"framework://active_task"));
        assert!(uris.contains(&"framework://goal_state"));
        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn resources_read_goal_state_when_absent() {
        let repo = unique_test_repo("resources-read-goal");
        let response = handle_resources_read(
            Some(json!(1)),
            &json!({"params": {"uri": "framework://goal_state"}}),
            &repo,
        );
        let text = response["result"]["contents"][0]["text"]
            .as_str()
            .unwrap_or("");
        assert!(text.contains("null") || text.starts_with('{'));
        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn prompts_get_review_gate_mentions_closeout() {
        let repo = framework_repo_root();
        let response = handle_prompts_get(
            Some(json!(1)),
            &json!({"params": {"name": "review_gate"}}),
            &repo,
            "opencode",
        );
        let text = response["result"]["messages"][0]["content"]["text"]
            .as_str()
            .unwrap_or("");
        assert!(text.contains("closeout") || text.contains("Closeout"));
    }
