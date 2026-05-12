use super::*;
use std::io::Cursor;

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("router-rs-browser-mcp-{label}-{unique}"));
    fs::create_dir_all(&path).expect("create temp root");
    path
}

#[test]
fn browser_mcp_stdio_lists_full_tool_surface() {
    let repo_root = temp_root("list-tools");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let input = Cursor::new(
        [
            serde_json::to_string(
                &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
            )
            .unwrap(),
            serde_json::to_string(
                &json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}),
            )
            .unwrap(),
        ]
        .join("\n"),
    );
    let mut output = Vec::new();
    run_browser_mcp_stdio(input, &mut output, &mut runtime).expect("run mcp");
    let lines = String::from_utf8(output).expect("utf8");
    let payloads = lines
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("json"))
        .collect::<Vec<_>>();
    assert_eq!(payloads[0]["result"]["serverInfo"]["name"], "browser-mcp");
    let names = payloads[1]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "browser_open",
            "browser_tabs",
            "browser_close",
            "browser_get_state",
            "browser_get_elements",
            "browser_get_text",
            "browser_get_network",
            "browser_screenshot",
            "browser_click",
            "browser_fill",
            "browser_press",
            "browser_wait_for",
            "browser_save_session",
            "browser_restore_session",
            "browser_get_attached_runtime_events",
            "runtime_heartbeat",
            "session_launch",
            "session_list",
            "session_inspect",
            "session_terminate",
            "session_mark_blocked",
            "session_resume_due",
            "session_classify_block",
            "background_list",
            "background_inspect",
            "background_terminate",
            "browser_diagnostics",
            "skill_route_status",
        ]
    );
    let status_response = handle_browser_mcp_request(
            &json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "skill_route_status", "arguments": {}}}),
            &mut runtime,
        )
        .expect("status response");
    assert_eq!(status_response["result"]["isError"], false);
    assert_eq!(
        status_response["result"]["structuredContent"]["routing_tools_exposed"],
        false
    );
    fs::remove_dir_all(repo_root).expect("cleanup");
}

#[test]
fn browser_mcp_exposes_repo_skill_routing_tools_when_runtime_exists() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical repo root");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let list_response = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}}),
        &mut runtime,
    )
    .expect("list response");
    let names = list_response["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(names.contains(&"skill_route"));
    assert!(names.contains(&"skill_search"));
    assert!(names.contains(&"skill_read"));
    assert!(names.contains(&"session_list"));
    assert!(names.contains(&"background_list"));

    let route_response = handle_browser_mcp_request(
            &json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "skill_route", "arguments": {"query": "需要多 agent 执行，先判断是否应该拆 bounded subagent sidecar"}}}),
            &mut runtime,
        )
        .expect("route response");
    assert_eq!(route_response["result"]["isError"], false);
    assert_eq!(
        route_response["result"]["structuredContent"]["decision"]["selected_skill"],
        "agent-swarm-orchestration"
    );
    assert!(
        route_response["result"]["structuredContent"]["selected_skill_path"]
            .as_str()
            .unwrap()
            .ends_with("skills/agent-swarm-orchestration/SKILL.md")
    );

    let search_response = handle_browser_mcp_request(
            &json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "skill_search", "arguments": {"query": "DESIGN.md 设计规范 token", "limit": 5}}}),
            &mut runtime,
        )
        .expect("search response");
    assert_eq!(search_response["result"]["isError"], false);
    assert!(search_response["result"]["structuredContent"]["matches"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["record"]["name"] == "design-md"));

    let read_response = handle_browser_mcp_request(
            &json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "skill_read", "arguments": {"skill": "agent-swarm-orchestration"}}}),
            &mut runtime,
        )
        .expect("read response");
    assert_eq!(read_response["result"]["isError"], false);
    assert!(read_response["result"]["structuredContent"]["content"]
        .as_str()
        .unwrap()
        .contains("# agent-swarm-orchestration"));

    let session_list_response = handle_browser_mcp_request(
            &json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "session_list", "arguments": {}}}),
            &mut runtime,
        )
        .expect("session list response");
    assert_eq!(session_list_response["result"]["isError"], false);
    assert!(session_list_response["result"]["structuredContent"]["workers"].is_array());

    let background_path = repo_root
        .join("artifacts")
        .join("runtime")
        .join("background_state.json");
    let background_list_response = handle_browser_mcp_request(
            &json!({"jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {"name": "background_list", "arguments": {"statePath": background_path.to_string_lossy()}}}),
            &mut runtime,
        )
        .expect("background list response");
    assert_eq!(background_list_response["result"]["isError"], false);
    assert!(background_list_response["result"]["structuredContent"]["state"]["jobs"].is_array());

    let background_terminate_response = handle_browser_mcp_request(
            &json!({"jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": {"name": "background_terminate", "arguments": {"statePath": background_path.to_string_lossy(), "jobId": "job-1"}}}),
            &mut runtime,
        )
        .expect("background terminate response");
    assert_eq!(background_terminate_response["result"]["isError"], false);
    assert_eq!(
        background_terminate_response["result"]["structuredContent"]["job"]["status"],
        "interrupted"
    );
}

#[test]
fn browser_mcp_invalid_tool_input_is_recoverable() {
    let repo_root = temp_root("invalid-input");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let response = handle_browser_mcp_request(
            &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "browser_open", "arguments": {}}}),
            &mut runtime,
        )
        .expect("response");
    assert_eq!(response["result"]["isError"], true);
    assert_eq!(
        response["result"]["structuredContent"]["error"]["code"],
        "INVALID_INPUT"
    );
    fs::remove_dir_all(repo_root).expect("cleanup");
}

#[test]
fn browser_mcp_rust_replays_attached_runtime_events_from_resume_manifest() {
    let repo_root = temp_root("attach-replay");
    let data_root = repo_root.join("runtime-data");
    let binding_path = data_root
        .join("runtime_event_transports")
        .join("session-1__job-1.json");
    let resume_path = data_root.join("TRACE_RESUME_MANIFEST.json");
    let trace_path = data_root.join("TRACE_EVENTS.jsonl");
    fs::create_dir_all(binding_path.parent().expect("binding parent"))
        .expect("create attach fixture dir");
    fs::write(
        &binding_path,
        serde_json::to_string_pretty(&json!({
            "schema_version": "runtime-event-transport-v1",
            "stream_id": "stream::job-1",
            "session_id": "session-1",
            "job_id": "job-1",
            "binding_backend_family": "filesystem",
            "resume_mode": "after_event_id",
            "cleanup_preserves_replay": true
        }))
        .expect("serialize binding"),
    )
    .expect("write binding");
    fs::write(
        &resume_path,
        serde_json::to_string_pretty(&json!({
            "schema_version": "runtime-resume-manifest-v1",
            "session_id": "session-1",
            "job_id": "job-1",
            "event_transport_path": binding_path.display().to_string(),
            "trace_stream_path": trace_path.display().to_string(),
            "updated_at": "2026-04-23T00:00:01+00:00"
        }))
        .expect("serialize resume"),
    )
    .expect("write resume");
    fs::write(
            &trace_path,
            concat!(
                "{\"event_id\":\"evt-1\",\"kind\":\"job.started\",\"ts\":\"2026-04-23T00:00:00.000Z\"}\n",
                "{\"event_id\":\"evt-2\",\"kind\":\"job.completed\",\"ts\":\"2026-04-23T00:00:01.000Z\"}\n"
            ),
        )
        .expect("write trace");

    let mut runtime = BrowserRuntime::with_attach_config(
        repo_root.clone(),
        BrowserAttachConfig {
            runtime_attach_artifact_path: Some(resume_path.display().to_string()),
            ..BrowserAttachConfig::default()
        },
    );
    let diagnostics = runtime.diagnostics(&json!({})).expect("diagnostics");
    assert_eq!(diagnostics["attachedRuntime"]["status"], "ready");
    assert_eq!(
        diagnostics["attachedRuntime"]["inputArtifactKind"],
        Value::String("resume_manifest".to_string())
    );
    assert_eq!(diagnostics["attachedRuntime"]["eventCount"], json!(2));

    let replay = runtime
        .get_attached_runtime_events(&json!({"afterEventId": "evt-1", "limit": 5}))
        .expect("replay");
    assert_eq!(replay["events"].as_array().expect("events").len(), 1);
    assert_eq!(replay["events"][0]["event_id"], "evt-2");
    assert_eq!(
        replay["replayContext"]["resumeManifestSource"],
        Value::String("explicit_request".to_string())
    );

    fs::remove_dir_all(repo_root).expect("cleanup");
}

#[test]
fn browser_mcp_auto_discovers_newest_attach_manifest() {
    let repo_root = temp_root("attach-discovery");
    let older = repo_root
        .join("artifacts")
        .join("scratch")
        .join("older")
        .join("TRACE_RESUME_MANIFEST.json");
    let newer = repo_root
        .join("artifacts")
        .join("scratch")
        .join("newer")
        .join("TRACE_RESUME_MANIFEST.json");
    fs::create_dir_all(older.parent().expect("older parent")).expect("create older parent");
    fs::create_dir_all(newer.parent().expect("newer parent")).expect("create newer parent");
    fs::write(
        &older,
        serde_json::to_string_pretty(&json!({
            "schema_version": "runtime-resume-manifest-v1",
            "event_transport_path": "/tmp/older.json",
            "updated_at": "2026-04-23T00:00:00+00:00"
        }))
        .expect("serialize older"),
    )
    .expect("write older");
    fs::write(
        &newer,
        serde_json::to_string_pretty(&json!({
            "schema_version": "runtime-resume-manifest-v1",
            "event_transport_path": "/tmp/newer.json",
            "updated_at": "2026-04-23T00:05:00+00:00"
        }))
        .expect("serialize newer"),
    )
    .expect("write newer");

    let runtime = BrowserRuntime::new(repo_root.clone());
    assert_eq!(
        runtime.auto_discover_runtime_attach_artifact(),
        Some(newer.to_string_lossy().into_owned())
    );

    fs::remove_dir_all(repo_root).expect("cleanup");
}
