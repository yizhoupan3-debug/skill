#![allow(clippy::unwrap_used, clippy::expect_used)]
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
            "browser_diagnostics",
        ]
    );
    fs::remove_dir_all(repo_root).expect("cleanup");
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

#[test]
fn truncate_text_short_string_unchanged() {
    assert_eq!(truncate_text("hello", 10), "hello");
}

#[test]
fn truncate_text_exact_boundary_unchanged() {
    assert_eq!(truncate_text("abcde", 5), "abcde");
}

#[test]
fn truncate_text_long_string_appends_ellipsis() {
    assert_eq!(truncate_text("abcdef", 4), "abc...");
}

#[test]
fn truncate_text_empty_string() {
    assert_eq!(truncate_text("", 100), "");
}

#[test]
fn truncate_text_zero_max_returns_ellipsis() {
    // max_chars=0 -> take(0-1) underflows but saturating_sub handles it
    assert_eq!(truncate_text("hello", 0), "...");
}

#[test]
fn truncate_text_max_chars_one_returns_ellipsis() {
    // max_chars=1 -> keep 0 chars + "..." = "..."
    assert_eq!(truncate_text("hello", 1), "...");
}

// ───────────────────────────────────────────────────────────────────
// 纯函数测试：to_text_lines
// ───────────────────────────────────────────────────────────────────

#[test]
fn to_text_lines_deduplicates_and_trims() {
    let lines = to_text_lines("  hello  \n  world  \n  hello  \n\n");
    assert_eq!(lines, vec!["hello", "world"]);
}

#[test]
fn to_text_lines_empty_input() {
    let lines = to_text_lines("");
    assert!(lines.is_empty());
}

#[test]
fn to_text_lines_limits_to_50() {
    let input = (0..100)
        .map(|i| format!("line{i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let lines = to_text_lines(&input);
    assert_eq!(lines.len(), 50);
}

#[test]
fn to_text_lines_truncates_long_lines() {
    let long_line = "x".repeat(300);
    let lines = to_text_lines(&long_line);
    assert_eq!(lines.len(), 1);
    // truncate_text takes max_chars-1 chars then appends "..." = 239 + 3 = 242
    assert!(lines[0].len() <= 242, "got len={}", lines[0].len());
    assert!(lines[0].ends_with("..."));
}

// ───────────────────────────────────────────────────────────────────
// 纯函数测试：cdp_key_name
// ───────────────────────────────────────────────────────────────────

#[test]
fn cdp_key_name_return_maps_to_enter() {
    assert_eq!(cdp_key_name("Return"), "Enter");
}

#[test]
fn cdp_key_name_passthrough() {
    assert_eq!(cdp_key_name("Tab"), "Tab");
    assert_eq!(cdp_key_name("Escape"), "Escape");
}

// ───────────────────────────────────────────────────────────────────
// 纯函数测试：json_string_literal
// ───────────────────────────────────────────────────────────────────

#[test]
fn json_string_literal_simple() {
    assert_eq!(json_string_literal("hello"), "\"hello\"");
}

#[test]
fn json_string_literal_with_special_chars() {
    assert_eq!(json_string_literal("a\"b\nc"), "\"a\\\"b\\nc\"");
}

// ───────────────────────────────────────────────────────────────────
// 纯函数测试：decode_base64
// ───────────────────────────────────────────────────────────────────

#[test]
fn decode_base64_standard() {
    // "Hello" = "SGVsbG8="
    assert_eq!(decode_base64("SGVsbG8=").unwrap(), b"Hello");
}

#[test]
fn decode_base64_empty() {
    assert_eq!(decode_base64("").unwrap(), b"");
}

#[test]
fn decode_base64_with_padding() {
    // "a" = "YQ=="
    assert_eq!(decode_base64("YQ==").unwrap(), b"a");
}

#[test]
fn decode_base64_with_whitespace() {
    assert_eq!(decode_base64("SGVs\nbG8=").unwrap(), b"Hello");
}

#[test]
fn decode_base64_invalid_byte() {
    assert!(decode_base64("SGVs!bG8=").is_err());
}

#[test]
fn decode_base64_whitespace_only() {
    assert_eq!(decode_base64("   \n  \t  ").unwrap(), b"");
}

// ───────────────────────────────────────────────────────────────────
// 纯函数测试：value_string
// ───────────────────────────────────────────────────────────────────

#[test]
fn value_string_none() {
    assert_eq!(value_string(None), "");
}

#[test]
fn value_string_null() {
    assert_eq!(value_string(Some(&Value::Null)), "");
}

#[test]
fn value_string_string() {
    assert_eq!(value_string(Some(&json!("hello"))), "hello");
}

#[test]
fn value_string_number() {
    assert_eq!(value_string(Some(&json!(42))), "42");
}

#[test]
fn value_string_bool() {
    assert_eq!(value_string(Some(&json!(true))), "true");
}

#[test]
fn value_string_array_joined() {
    assert_eq!(value_string(Some(&json!(["a", "b", "c"]))), "a b c");
}

// ───────────────────────────────────────────────────────────────────
// 纯函数测试：json_type_name
// ───────────────────────────────────────────────────────────────────

#[test]
fn json_type_name_all_variants() {
    assert_eq!(json_type_name(&Value::Null), "NoneType");
    assert_eq!(json_type_name(&json!(true)), "bool");
    assert_eq!(json_type_name(&json!(42)), "int");
    assert_eq!(json_type_name(&json!("hi")), "str");
    assert_eq!(json_type_name(&json!([])), "list");
    assert_eq!(json_type_name(&json!({})), "dict");
}

// ───────────────────────────────────────────────────────────────────
// 纯函数测试：parse_content_length_header
// ───────────────────────────────────────────────────────────────────

#[test]
fn parse_content_length_header_valid() {
    assert_eq!(
        parse_content_length_header("Content-Length: 42").unwrap(),
        42
    );
}

#[test]
fn parse_content_length_header_with_whitespace() {
    assert_eq!(
        parse_content_length_header("Content-Length:   100  ").unwrap(),
        100
    );
}

#[test]
fn parse_content_length_header_invalid_format() {
    assert!(parse_content_length_header("NoColon").is_err());
}

#[test]
fn parse_content_length_header_not_a_number() {
    assert!(parse_content_length_header("Content-Length: abc").is_err());
}

// ───────────────────────────────────────────────────────────────────
// 纯函数测试：create_fingerprint
// ───────────────────────────────────────────────────────────────────

fn make_descriptor(role: &str, name: &str, tag: &str, test_id: Option<&str>) -> ElementDescriptor {
    ElementDescriptor {
        role: role.to_string(),
        name: name.to_string(),
        text: String::new(),
        visible: true,
        enabled: true,
        tag: tag.to_string(),
        test_id: test_id.map(str::to_string),
        selector: String::new(),
    }
}

#[test]
fn fingerprint_uses_test_id_when_present() {
    let d = make_descriptor("button", "Submit", "button", Some("submit-btn"));
    let mut counts = HashMap::new();
    assert_eq!(create_fingerprint(&d, &mut counts), "tid::submit-btn");
}

#[test]
fn fingerprint_uses_role_name_tag_base() {
    let d = make_descriptor("button", "OK", "button", None);
    let mut counts = HashMap::new();
    assert_eq!(create_fingerprint(&d, &mut counts), "button::OK::button");
}

#[test]
fn fingerprint_deduplicates_with_counter() {
    let d1 = make_descriptor("button", "OK", "button", None);
    let d2 = make_descriptor("button", "OK", "button", None);
    let mut counts = HashMap::new();
    assert_eq!(create_fingerprint(&d1, &mut counts), "button::OK::button");
    assert_eq!(create_fingerprint(&d2, &mut counts), "button::OK::button#2");
}

// ───────────────────────────────────────────────────────────────────
// 纯函数测试：has_meaningful_change
// ───────────────────────────────────────────────────────────────────

fn make_snapshot(url: &str, title: &str, text: &str) -> PageSnapshot {
    PageSnapshot {
        revision: 1,
        url: url.to_string(),
        title: title.to_string(),
        loading_state: "idle".to_string(),
        summary: json!({}),
        interactive_elements: vec![],
        text_content: text.to_string(),
        text_lines: vec![text.to_string()],
    }
}

#[test]
fn meaningful_change_same_snapshot_is_false() {
    let s = make_snapshot("http://a", "T", "body");
    assert!(!has_meaningful_change(&s, &s.clone()));
}

#[test]
fn meaningful_change_different_url_is_true() {
    let s1 = make_snapshot("http://a", "T", "body");
    let s2 = make_snapshot("http://b", "T", "body");
    assert!(has_meaningful_change(&s1, &s2));
}

#[test]
fn meaningful_change_different_title_is_true() {
    let s1 = make_snapshot("http://a", "T1", "body");
    let s2 = make_snapshot("http://a", "T2", "body");
    assert!(has_meaningful_change(&s1, &s2));
}

#[test]
fn meaningful_change_different_text_is_true() {
    let s1 = make_snapshot("http://a", "T", "text1");
    let s2 = make_snapshot("http://a", "T", "text2");
    assert!(has_meaningful_change(&s1, &s2));
}

// ───────────────────────────────────────────────────────────────────
// 纯函数测试：compute_delta
// ───────────────────────────────────────────────────────────────────

#[test]
fn compute_delta_same_snapshots_have_no_changes() {
    let s = make_snapshot("http://a", "Title", "hello");
    let delta = compute_delta(&s, &s.clone());
    assert_eq!(delta["urlChanged"], json!(false));
    assert_eq!(delta["titleChanged"], json!(false));
    assert_eq!(delta["newElements"], json!([]));
    assert_eq!(delta["removedRefs"], json!([]));
}

#[test]
fn compute_delta_detects_url_change() {
    let s1 = make_snapshot("http://a", "Title", "text");
    let s2 = make_snapshot("http://b", "Title", "text");
    let delta = compute_delta(&s1, &s2);
    assert_eq!(delta["urlChanged"], json!(true));
}

#[test]
fn compute_delta_alerts_on_error_text() {
    let s1 = make_snapshot("http://a", "T", "ok");
    let mut s2 = make_snapshot("http://a", "T", "something error occurred");
    s2.text_lines = vec!["something error occurred".to_string()];
    let delta = compute_delta(&s1, &s2);
    let alerts = delta["alerts"].as_array().unwrap();
    assert!(!alerts.is_empty());
}

// ───────────────────────────────────────────────────────────────────
// 纯函数测试：descriptor_leaf / descriptor_string / descriptor_bool
// ───────────────────────────────────────────────────────────────────

#[test]
fn descriptor_leaf_nested() {
    let v = json!({"a": {"b": {"c": 42}}});
    assert_eq!(descriptor_leaf(&v, &["a", "b", "c"]), Some(&json!(42)));
    assert_eq!(descriptor_leaf(&v, &["a", "x"]), None);
}

#[test]
fn descriptor_string_extract() {
    let v = json!({"schema_version": "v1"});
    assert_eq!(
        descriptor_string(&v, &["schema_version"]),
        Some("v1".to_string())
    );
    assert_eq!(descriptor_string(&v, &["missing"]), None);
}

#[test]
fn descriptor_bool_extract() {
    let v = json!({"replay": true});
    assert_eq!(descriptor_bool(&v, &["replay"]), Some(true));
    assert_eq!(descriptor_bool(&v, &["missing"]), None);
}

#[test]
fn descriptor_resolved_artifact_fallback() {
    let v = json!({"resolved_artifacts": {"trace_stream_path": "/tmp/trace.jsonl"}});
    assert_eq!(
        descriptor_resolved_artifact(&v, "trace_stream_path"),
        Some("/tmp/trace.jsonl".to_string())
    );
}

#[test]
fn descriptor_resolved_artifact_top_level_fallback() {
    let v = json!({"trace_stream_path": "/tmp/trace.jsonl"});
    assert_eq!(
        descriptor_resolved_artifact(&v, "trace_stream_path"),
        Some("/tmp/trace.jsonl".to_string())
    );
}

// ───────────────────────────────────────────────────────────────────
// 纯函数测试：browser_error / runtime_error
// ───────────────────────────────────────────────────────────────────

#[test]
fn browser_error_structure() {
    let err = browser_error("TEST_CODE", "test message", &["action1"], true);
    assert_eq!(err["code"], "TEST_CODE");
    assert_eq!(err["message"], "test message");
    assert_eq!(err["recoverable"], true);
    let actions = err["suggested_next_actions"].as_array().unwrap();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0], "action1");
}

#[test]
fn runtime_error_uses_runtime_code() {
    let err = runtime_error("SESSION_FAILED", "timeout");
    assert_eq!(err["code"], "SESSION_FAILED");
}

// ───────────────────────────────────────────────────────────────────
// 纯函数测试：success_response / error_response
// ───────────────────────────────────────────────────────────────────

#[test]
fn success_response_wraps_result() {
    let resp = success_response(json!(1), json!({"ok": true}));
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    assert_eq!(resp["result"]["ok"], true);
}

#[test]
fn error_response_wraps_error() {
    let resp = error_response(json!(5), browser_error("CODE", "msg", &[], false));
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 5);
    assert_eq!(resp["error"]["code"], -32000);
}

// ───────────────────────────────────────────────────────────────────
// 参数解析测试：require_string / optional_string / optional_bool / optional_u64
// ───────────────────────────────────────────────────────────────────

#[test]
fn require_string_present() {
    let v = json!({"key": "value"});
    assert_eq!(require_string(&v, "key").unwrap(), "value");
}

#[test]
fn require_string_missing() {
    let v = json!({});
    assert!(require_string(&v, "key").is_err());
}

#[test]
fn require_string_empty() {
    let v = json!({"key": ""});
    assert!(require_string(&v, "key").is_err());
}

#[test]
fn require_string_whitespace_only() {
    let v = json!({"key": "   "});
    assert!(require_string(&v, "key").is_err());
}

#[test]
fn optional_string_present() {
    let v = json!({"key": "value"});
    assert_eq!(optional_string(&v, "key"), Some("value".to_string()));
}

#[test]
fn optional_string_missing() {
    let v = json!({});
    assert_eq!(optional_string(&v, "key"), None);
}

#[test]
fn optional_bool_present() {
    let v = json!({"flag": true});
    assert_eq!(optional_bool(&v, "flag"), Some(true));
}

#[test]
fn optional_bool_missing() {
    let v = json!({});
    assert_eq!(optional_bool(&v, "flag"), None);
}

#[test]
fn optional_u64_present() {
    let v = json!({"n": 42});
    assert_eq!(optional_u64(&v, "n").unwrap(), Some(42));
}

#[test]
fn optional_u64_missing() {
    let v = json!({});
    assert_eq!(optional_u64(&v, "n").unwrap(), None);
}

#[test]
fn optional_u64_invalid_type() {
    let v = json!({"n": "bad"});
    assert!(optional_u64(&v, "n").is_err());
}

#[test]
fn optional_usize_with_default() {
    let v = json!({});
    assert_eq!(optional_usize(&v, "n", 100).unwrap(), 100);
}

#[test]
fn optional_string_array_present() {
    let v = json!({"arr": ["a", "b"]});
    assert_eq!(
        optional_string_array(&v, "arr"),
        Some(vec!["a".to_string(), "b".to_string()])
    );
}

#[test]
fn optional_string_array_missing() {
    let v = json!({});
    assert_eq!(optional_string_array(&v, "arr"), None);
}

// ───────────────────────────────────────────────────────────────────
// JSON-RPC 路由测试
// ───────────────────────────────────────────────────────────────────

#[test]
fn handle_browser_mcp_request_initialize() {
    let repo_root = temp_root("init");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let resp = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        &mut runtime,
    )
    .expect("response");
    assert_eq!(resp["result"]["serverInfo"]["name"], "browser-mcp");
    assert_eq!(resp["result"]["protocolVersion"], PROTOCOL_VERSION);
    fs::remove_dir_all(repo_root).unwrap();
}

#[test]
fn handle_browser_mcp_request_ping() {
    let repo_root = temp_root("ping");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let resp = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "ping", "params": {}}),
        &mut runtime,
    )
    .expect("response");
    assert_eq!(resp["result"], json!({}));
    fs::remove_dir_all(repo_root).unwrap();
}

#[test]
fn handle_browser_mcp_request_unsupported_method() {
    let repo_root = temp_root("unsupported");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let resp = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "bogus/method", "params": {}}),
        &mut runtime,
    )
    .expect("response");
    assert_eq!(resp["error"]["code"], -32000);
    fs::remove_dir_all(repo_root).unwrap();
}

#[test]
fn handle_browser_mcp_request_notifications_initialized_returns_none() {
    let repo_root = temp_root("notif");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let result = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        &mut runtime,
    );
    assert!(result.is_none());
    fs::remove_dir_all(repo_root).unwrap();
}

#[test]
fn handle_browser_mcp_line_invalid_json() {
    let repo_root = temp_root("bad-json");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let result = handle_browser_mcp_line("not json {{", &mut runtime);
    let resp = result.expect("should return error response");
    assert!(resp.get("error").is_some());
    fs::remove_dir_all(repo_root).unwrap();
}

// ───────────────────────────────────────────────────────────────────
// BrowserRuntime 状态管理测试（不需要真实浏览器）
// ───────────────────────────────────────────────────────────────────

#[test]
fn runtime_required_session_id_empty_is_error() {
    let repo_root = temp_root("no-session");
    let runtime = BrowserRuntime::new(repo_root.clone());
    assert!(runtime.required_session_id().is_err());
    fs::remove_dir_all(repo_root).unwrap();
}

#[test]
fn runtime_session_not_found_error_structure() {
    let err = session_not_found_error();
    assert_eq!(err["code"], "SESSION_NOT_FOUND");
    assert_eq!(err["recoverable"], true);
}

#[test]
fn runtime_diagnostics_zero_sessions() {
    let repo_root = temp_root("diag");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let diag = runtime.diagnostics(&json!({})).unwrap();
    assert_eq!(diag["sessions"], 0);
    assert_eq!(diag["tabs"], 0);
    assert_eq!(diag["runtimeVersion"], SERVER_VERSION);
    assert_eq!(diag["attachedRuntime"]["status"], "not_configured");
    fs::remove_dir_all(repo_root).unwrap();
}

// ───────────────────────────────────────────────────────────────────
// 工具定义完整性测试
// ───────────────────────────────────────────────────────────────────

#[test]
fn tool_definitions_contain_required_browser_tools() {
    let repo_root = temp_root("tool-defs");
    let tools = tool_definitions(&repo_root);
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    for required in &[
        "browser_open",
        "browser_click",
        "browser_fill",
        "browser_press",
        "browser_tabs",
        "browser_close",
        "browser_get_state",
        "browser_screenshot",
        "browser_diagnostics",
    ] {
        assert!(names.contains(required), "missing tool: {required}");
    }
    fs::remove_dir_all(repo_root).unwrap();
}

#[test]
fn tool_definitions_have_input_schema() {
    let repo_root = temp_root("schema-check");
    let tools = tool_definitions(&repo_root);
    for tool in &tools {
        assert!(
            tool.get("inputSchema").is_some(),
            "tool {} missing inputSchema",
            tool["name"]
        );
        assert!(
            tool.get("outputSchema").is_some(),
            "tool {} missing outputSchema",
            tool["name"]
        );
    }
    fs::remove_dir_all(repo_root).unwrap();
}

#[test]
fn tool_definitions_required_fields_present() {
    let repo_root = temp_root("required-fields");
    let tools = tool_definitions(&repo_root);
    let browser_open = tools.iter().find(|t| t["name"] == "browser_open").unwrap();
    let required = browser_open["inputSchema"]["required"].as_array().unwrap();
    assert!(required.contains(&json!("url")));
    fs::remove_dir_all(repo_root).unwrap();
}

// ───────────────────────────────────────────────────────────────────
// tool_result 包装测试
// ───────────────────────────────────────────────────────────────────

#[test]
fn tool_result_ok_wraps_structured_content() {
    let result = tool_result(Ok(json!({"data": 42})));
    let payload = result.unwrap();
    assert_eq!(payload["isError"], false);
    assert_eq!(payload["structuredContent"]["data"], 42);
}

#[test]
fn tool_result_err_wraps_error_as_is_error_true() {
    let err = browser_error("ERR", "msg", &[], true);
    let result = tool_result(Err(err));
    let payload = result.unwrap();
    assert_eq!(payload["isError"], true);
    assert_eq!(payload["structuredContent"]["ok"], false);
    assert_eq!(payload["structuredContent"]["error"]["code"], "ERR");
}

// ───────────────────────────────────────────────────────────────────
// normalize_runtime_locator_for_existing_file
// ───────────────────────────────────────────────────────────────────

#[test]
fn normalize_runtime_locator_existing_absolute_path() {
    let dir = temp_root("normalize");
    let file = dir.join("test.json");
    fs::write(&file, "{}").unwrap();
    let result = normalize_runtime_locator_for_existing_file(&file.to_string_lossy());
    assert_eq!(result, file.to_string_lossy());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn normalize_runtime_locator_nonexistent_returns_original() {
    let result = normalize_runtime_locator_for_existing_file("/nonexistent/path/file.json");
    assert_eq!(result, "/nonexistent/path/file.json");
}

// ───────────────────────────────────────────────────────────────────
// BrowserAttachConfig::from_cli_and_env 测试
// ───────────────────────────────────────────────────────────────────

#[test]
fn attach_config_default_values() {
    let config = BrowserAttachConfig::default();
    assert!(config.runtime_attach_descriptor_path.is_none());
    assert!(config.runtime_attach_artifact_path.is_none());
    assert!(!config.headless);
}

#[test]
fn attach_config_from_cli_explicit_values() {
    let config = BrowserAttachConfig::from_cli_and_env(
        Some("/path/to/descriptor".to_string()),
        Some("/path/to/artifact".to_string()),
        Some("true".to_string()),
    );
    assert_eq!(
        config.runtime_attach_descriptor_path,
        Some("/path/to/descriptor".to_string())
    );
    assert_eq!(
        config.runtime_attach_artifact_path,
        Some("/path/to/artifact".to_string())
    );
    assert!(config.headless);
}

#[test]
fn resolve_headless_option_default_is_true() {
    assert!(resolve_headless_option(None));
}

#[test]
fn resolve_headless_option_false_string() {
    assert!(!resolve_headless_option(Some("false".to_string())));
}

#[test]
fn resolve_headless_option_true_string() {
    assert!(resolve_headless_option(Some("true".to_string())));
}

// ───────────────────────────────────────────────────────────────────
// opt_string_value 测试
// ───────────────────────────────────────────────────────────────────

#[test]
fn opt_string_value_some() {
    assert_eq!(opt_string_value(Some("hello".to_string())), json!("hello"));
}

#[test]
fn opt_string_value_none() {
    assert_eq!(opt_string_value(None), Value::Null);
}

// ───────────────────────────────────────────────────────────────────
// Content-Length 传输模式测试
// ───────────────────────────────────────────────────────────────────

#[test]
fn content_length_transport_roundtrip() {
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": "ping"}).to_string();
    let framed = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
    let mut input = Cursor::new(framed);
    let mut mode = None;
    let message = read_browser_mcp_message(&mut input, &mut mode)
        .unwrap()
        .unwrap();
    assert_eq!(mode, Some(BrowserMcpTransportMode::ContentLength));
    let parsed: Value = serde_json::from_str(&message).unwrap();
    assert_eq!(parsed["method"], "ping");
}

#[test]
fn newline_delimited_transport_roundtrip() {
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": "ping"});
    let mut input = Cursor::new(body.to_string());
    let mut mode = None;
    let message = read_browser_mcp_message(&mut input, &mut mode)
        .unwrap()
        .unwrap();
    assert_eq!(mode, Some(BrowserMcpTransportMode::NewlineDelimited));
    let parsed: Value = serde_json::from_str(&message).unwrap();
    assert_eq!(parsed["method"], "ping");
}

#[test]
fn write_browser_mcp_response_newline_delimited() {
    let mut output = Vec::new();
    write_browser_mcp_response(
        &mut output,
        BrowserMcpTransportMode::NewlineDelimited,
        &json!({"ok": true}),
    )
    .unwrap();
    let line = String::from_utf8(output).unwrap();
    let parsed: Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(parsed["ok"], true);
}

#[test]
fn write_browser_mcp_response_content_length() {
    let mut output = Vec::new();
    write_browser_mcp_response(
        &mut output,
        BrowserMcpTransportMode::ContentLength,
        &json!({"ok": true}),
    )
    .unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.starts_with("Content-Length:"));
    assert!(text.contains("\r\n\r\n"));
}

#[test]
fn read_browser_mcp_message_eof_returns_none() {
    let mut input = Cursor::new(Vec::<u8>::new());
    let mut mode = None;
    let result = read_browser_mcp_message(&mut input, &mut mode).unwrap();
    assert!(result.is_none());
}

// ───────────────────────────────────────────────────────────────────
// MCP 工具调用测试（通过 JSON-RPC 入口）
// ───────────────────────────────────────────────────────────────────

#[test]
fn tools_call_unknown_tool_returns_error() {
    let repo_root = temp_root("unknown-tool");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let resp = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "nonexistent_tool", "arguments": {}}}),
        &mut runtime,
    ).unwrap();
    assert_eq!(resp["result"]["isError"], true);
    assert_eq!(
        resp["result"]["structuredContent"]["error"]["code"],
        "INVALID_INPUT"
    );
    fs::remove_dir_all(repo_root).unwrap();
}

#[test]
fn browser_click_without_session_returns_session_not_found() {
    let repo_root = temp_root("click-no-sess");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let resp = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "browser_click", "arguments": {"ref": "el_1"}}}),
        &mut runtime,
    ).unwrap();
    assert_eq!(resp["result"]["isError"], true);
    assert_eq!(
        resp["result"]["structuredContent"]["error"]["code"],
        "SESSION_NOT_FOUND"
    );
    fs::remove_dir_all(repo_root).unwrap();
}

#[test]
fn browser_fill_without_session_returns_session_not_found() {
    let repo_root = temp_root("fill-no-sess");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let resp = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "browser_fill", "arguments": {"ref": "el_1", "value": "test"}}}),
        &mut runtime,
    ).unwrap();
    assert_eq!(resp["result"]["isError"], true);
    assert_eq!(
        resp["result"]["structuredContent"]["error"]["code"],
        "SESSION_NOT_FOUND"
    );
    fs::remove_dir_all(repo_root).unwrap();
}

#[test]
fn browser_press_without_session_returns_session_not_found() {
    let repo_root = temp_root("press-no-sess");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let resp = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "browser_press", "arguments": {"key": "Enter"}}}),
        &mut runtime,
    ).unwrap();
    assert_eq!(resp["result"]["isError"], true);
    assert_eq!(
        resp["result"]["structuredContent"]["error"]["code"],
        "SESSION_NOT_FOUND"
    );
    fs::remove_dir_all(repo_root).unwrap();
}

#[test]
fn browser_tabs_without_session_returns_session_not_found() {
    let repo_root = temp_root("tabs-no-sess");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let resp = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "browser_tabs", "arguments": {"action": "list"}}}),
        &mut runtime,
    ).unwrap();
    assert_eq!(resp["result"]["isError"], true);
    assert_eq!(
        resp["result"]["structuredContent"]["error"]["code"],
        "SESSION_NOT_FOUND"
    );
    fs::remove_dir_all(repo_root).unwrap();
}

#[test]
fn browser_close_without_session_returns_session_not_found() {
    let repo_root = temp_root("close-no-sess");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let resp = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "browser_close", "arguments": {"target": "session"}}}),
        &mut runtime,
    ).unwrap();
    assert_eq!(resp["result"]["isError"], true);
    assert_eq!(
        resp["result"]["structuredContent"]["error"]["code"],
        "SESSION_NOT_FOUND"
    );
    fs::remove_dir_all(repo_root).unwrap();
}

#[test]
fn browser_get_state_without_session_returns_session_not_found() {
    let repo_root = temp_root("state-no-sess");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let resp = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "browser_get_state", "arguments": {}}}),
        &mut runtime,
    ).unwrap();
    assert_eq!(resp["result"]["isError"], true);
    assert_eq!(
        resp["result"]["structuredContent"]["error"]["code"],
        "SESSION_NOT_FOUND"
    );
    fs::remove_dir_all(repo_root).unwrap();
}

#[test]
fn browser_get_text_without_session_returns_session_not_found() {
    let repo_root = temp_root("text-no-sess");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let resp = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "browser_get_text", "arguments": {}}}),
        &mut runtime,
    ).unwrap();
    assert_eq!(resp["result"]["isError"], true);
    assert_eq!(
        resp["result"]["structuredContent"]["error"]["code"],
        "SESSION_NOT_FOUND"
    );
    fs::remove_dir_all(repo_root).unwrap();
}

#[test]
fn browser_get_elements_without_session_returns_session_not_found() {
    let repo_root = temp_root("elements-no-sess");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let resp = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "browser_get_elements", "arguments": {}}}),
        &mut runtime,
    ).unwrap();
    assert_eq!(resp["result"]["isError"], true);
    assert_eq!(
        resp["result"]["structuredContent"]["error"]["code"],
        "SESSION_NOT_FOUND"
    );
    fs::remove_dir_all(repo_root).unwrap();
}

#[test]
fn browser_get_network_without_session_returns_session_not_found() {
    let repo_root = temp_root("network-no-sess");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let resp = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "browser_get_network", "arguments": {}}}),
        &mut runtime,
    ).unwrap();
    assert_eq!(resp["result"]["isError"], true);
    assert_eq!(
        resp["result"]["structuredContent"]["error"]["code"],
        "SESSION_NOT_FOUND"
    );
    fs::remove_dir_all(repo_root).unwrap();
}

// ───────────────────────────────────────────────────────────────────
// AttachArtifactCandidateRank 排序测试
// ───────────────────────────────────────────────────────────────────

#[test]
fn attach_candidate_rank_higher_updated_at_wins() {
    let a = AttachArtifactCandidate {
        path: "a".to_string(),
        rank: AttachArtifactCandidateRank {
            updated_at_ms: 100,
            recency_ms: 0,
            source_priority: 0,
        },
    };
    let b = AttachArtifactCandidate {
        path: "b".to_string(),
        rank: AttachArtifactCandidateRank {
            updated_at_ms: 200,
            recency_ms: 0,
            source_priority: 0,
        },
    };
    assert!(b.rank > a.rank);
}

#[test]
fn attach_candidate_rank_same_updated_at_uses_recency() {
    let a = AttachArtifactCandidate {
        path: "a".to_string(),
        rank: AttachArtifactCandidateRank {
            updated_at_ms: 100,
            recency_ms: 50,
            source_priority: 0,
        },
    };
    let b = AttachArtifactCandidate {
        path: "b".to_string(),
        rank: AttachArtifactCandidateRank {
            updated_at_ms: 100,
            recency_ms: 200,
            source_priority: 0,
        },
    };
    assert!(b.rank > a.rank);
}

#[test]
fn attach_candidate_rank_same_recency_uses_source_priority() {
    let a = AttachArtifactCandidate {
        path: "a".to_string(),
        rank: AttachArtifactCandidateRank {
            updated_at_ms: 100,
            recency_ms: 50,
            source_priority: 0,
        },
    };
    let b = AttachArtifactCandidate {
        path: "b".to_string(),
        rank: AttachArtifactCandidateRank {
            updated_at_ms: 100,
            recency_ms: 50,
            source_priority: 1,
        },
    };
    assert!(b.rank > a.rank);
}

// ───────────────────────────────────────────────────────────────────
// interactive_element_value 测试
// ───────────────────────────────────────────────────────────────────

#[test]
fn interactive_element_value_contains_expected_fields() {
    let element = InteractiveElement {
        ref_id: "el_1".to_string(),
        page_revision: 5,
        role: "button".to_string(),
        name: "Submit".to_string(),
        text: "Submit Form".to_string(),
        visible: true,
        enabled: true,
        tag: "button".to_string(),
        test_id: Some("submit-btn".to_string()),
        fingerprint: "button::Submit::button".to_string(),
        selector: "button#submit".to_string(),
    };
    let v = interactive_element_value(&element);
    assert_eq!(v["ref"], "el_1");
    assert_eq!(v["pageRevision"], 5);
    assert_eq!(v["role"], "button");
    assert_eq!(v["name"], "Submit");
    assert_eq!(v["visible"], true);
    assert_eq!(v["locatorHint"]["tag"], "button");
    assert_eq!(v["locatorHint"]["testId"], "submit-btn");
}

// ───────────────────────────────────────────────────────────────────
// network_event_value 测试
// ───────────────────────────────────────────────────────────────────

#[test]
fn network_event_value_contains_expected_fields() {
    let event = NetworkEvent {
        id: "req_1".to_string(),
        method: "GET".to_string(),
        url: "https://example.com".to_string(),
        status: Some(200),
        content_type: Some("text/html".to_string()),
        resource_type: "Document".to_string(),
        timestamp: 1000,
        ok: true,
        error_text: None,
        duration_ms: Some(50),
    };
    let v = network_event_value(&event);
    assert_eq!(v["id"], "req_1");
    assert_eq!(v["method"], "GET");
    assert_eq!(v["status"], 200);
    assert_eq!(v["ok"], true);
    assert_eq!(v["durationMs"], 50);
    assert!(v["errorText"].is_null());
}

#[test]
fn network_event_value_error_state() {
    let event = NetworkEvent {
        id: "req_err".to_string(),
        method: String::new(),
        url: String::new(),
        status: None,
        content_type: None,
        resource_type: "XHR".to_string(),
        timestamp: 2000,
        ok: false,
        error_text: Some("net::ERR_CONNECTION_RESET".to_string()),
        duration_ms: None,
    };
    let v = network_event_value(&event);
    assert_eq!(v["ok"], false);
    assert_eq!(v["errorText"], "net::ERR_CONNECTION_RESET");
    assert!(v["durationMs"].is_null());
    assert!(v["status"].is_null());
}

// ───────────────────────────────────────────────────────────────────
// should_skip_attach_discovery_dir 测试
// ───────────────────────────────────────────────────────────────────

#[test]
fn should_skip_common_dirs() {
    assert!(should_skip_attach_discovery_dir(Path::new("/tmp/.git")));
    assert!(should_skip_attach_discovery_dir(Path::new(
        "/tmp/node_modules"
    )));
    assert!(should_skip_attach_discovery_dir(Path::new("/tmp/target")));
    assert!(!should_skip_attach_discovery_dir(Path::new(
        "/tmp/artifacts"
    )));
    assert!(!should_skip_attach_discovery_dir(Path::new(
        "/tmp/my-project"
    )));
}

// ───────────────────────────────────────────────────────────────────
// BrowserMcpTransportMode 测试
// ───────────────────────────────────────────────────────────────────

#[test]
fn transport_mode_equality() {
    assert_eq!(
        BrowserMcpTransportMode::ContentLength,
        BrowserMcpTransportMode::ContentLength
    );
    assert_eq!(
        BrowserMcpTransportMode::NewlineDelimited,
        BrowserMcpTransportMode::NewlineDelimited
    );
    assert_ne!(
        BrowserMcpTransportMode::ContentLength,
        BrowserMcpTransportMode::NewlineDelimited
    );
}

// ───────────────────────────────────────────────────────────────────
// attach_descriptor_needs_rust_hydration 测试
// ───────────────────────────────────────────────────────────────────

#[test]
fn attach_descriptor_needs_hydration_with_artifact_paths() {
    let desc = json!({
        "requested_artifacts": {
            "binding_artifact_path": "/tmp/binding.json"
        }
    });
    assert!(attach_descriptor_needs_rust_hydration(&desc));
}

#[test]
fn attach_descriptor_no_hydration_without_artifact_paths() {
    let desc = json!({
        "schema_version": "runtime-event-attach-descriptor-v1",
        "attach_mode": "process_external_artifact_replay"
    });
    assert!(!attach_descriptor_needs_rust_hydration(&desc));
}

// ───────────────────────────────────────────────────────────────────
// parse_rfc3339_millis 测试
// ───────────────────────────────────────────────────────────────────

#[test]
fn parse_rfc3339_valid() {
    let ms = parse_rfc3339_millis("2026-04-23T00:00:00+00:00").unwrap();
    assert!(ms > 0);
}

#[test]
fn parse_rfc3339_invalid() {
    assert!(parse_rfc3339_millis("not-a-date").is_none());
}

// --- Additional integration tests ---

#[test]
fn browser_mcp_initialize_returns_server_info() {
    let repo_root = temp_root("init");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let response = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        &mut runtime,
    )
    .expect("init response");
    assert_eq!(response["result"]["serverInfo"]["name"], "browser-mcp");
    assert_eq!(response["result"]["serverInfo"]["version"], "0.3.0-rust");
    assert_eq!(response["result"]["protocolVersion"], "2024-11-05");
    fs::remove_dir_all(repo_root).expect("cleanup");
}

#[test]
fn browser_mcp_ping_returns_empty_result() {
    let repo_root = temp_root("ping");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let response = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "ping", "params": {}}),
        &mut runtime,
    )
    .expect("ping response");
    assert_eq!(response["result"], json!({}));
    fs::remove_dir_all(repo_root).expect("cleanup");
}

#[test]
fn browser_mcp_notifications_initialized_returns_none() {
    let repo_root = temp_root("notif");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let response = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}}),
        &mut runtime,
    );
    assert!(response.is_none());
    fs::remove_dir_all(repo_root).expect("cleanup");
}

#[test]
fn browser_mcp_unsupported_method_returns_error() {
    let repo_root = temp_root("unsupported");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let response = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "nonexistent/method", "params": {}}),
        &mut runtime,
    )
    .expect("response");
    assert!(response["error"].is_object());
    assert_eq!(response["error"]["code"], -32000);
    fs::remove_dir_all(repo_root).expect("cleanup");
}

#[test]
fn browser_mcp_unknown_tool_returns_error() {
    let repo_root = temp_root("unknown-tool");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let response = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "nonexistent_tool", "arguments": {}}}),
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
fn browser_mcp_browser_diagnostics_returns_health() {
    let repo_root = temp_root("diagnostics");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let response = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "browser_diagnostics", "arguments": {}}}),
        &mut runtime,
    )
    .expect("response");
    assert_eq!(response["result"]["isError"], false);
    assert!(response["result"]["structuredContent"].is_object());
    fs::remove_dir_all(repo_root).expect("cleanup");
}

#[test]
fn browser_mcp_browser_open_requires_url() {
    let repo_root = temp_root("open-no-url");
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
fn browser_mcp_browser_click_requires_ref() {
    let repo_root = temp_root("click-no-ref");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let response = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "browser_click", "arguments": {}}}),
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
fn browser_mcp_browser_fill_requires_ref_and_value() {
    let repo_root = temp_root("fill-missing");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let response = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "browser_fill", "arguments": {"ref": "ref_1"}}}),
        &mut runtime,
    )
    .expect("response");
    assert_eq!(response["result"]["isError"], true);
    fs::remove_dir_all(repo_root).expect("cleanup");
}

#[test]
fn browser_mcp_browser_press_requires_key() {
    let repo_root = temp_root("press-no-key");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let response = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "browser_press", "arguments": {}}}),
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
fn browser_mcp_tool_definitions_include_output_schema() {
    let repo_root = temp_root("output-schema");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let response = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}}),
        &mut runtime,
    )
    .expect("response");
    let tools = response["result"]["tools"].as_array().unwrap();
    for tool in tools {
        assert!(tool.get("name").is_some(), "tool missing name");
        assert!(tool.get("title").is_some(), "tool missing title");
        assert!(
            tool.get("description").is_some(),
            "tool missing description"
        );
        assert!(
            tool.get("inputSchema").is_some(),
            "tool missing inputSchema"
        );
        assert!(
            tool.get("outputSchema").is_some(),
            "tool missing outputSchema"
        );
    }
    fs::remove_dir_all(repo_root).expect("cleanup");
}

#[test]
fn browser_mcp_session_inspect_requires_worker_id() {
    let repo_root = temp_root("inspect-no-id");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let response = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "session_inspect", "arguments": {}}}),
        &mut runtime,
    )
    .expect("response");
    assert_eq!(response["result"]["isError"], true);
    fs::remove_dir_all(repo_root).expect("cleanup");
}

#[test]
fn browser_mcp_session_terminate_requires_worker_id() {
    let repo_root = temp_root("terminate-no-id");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let response = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "session_terminate", "arguments": {}}}),
        &mut runtime,
    )
    .expect("response");
    assert_eq!(response["result"]["isError"], true);
    fs::remove_dir_all(repo_root).expect("cleanup");
}

#[test]
fn browser_mcp_session_mark_blocked_requires_fields() {
    let repo_root = temp_root("mark-blocked");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let response = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "session_mark_blocked", "arguments": {}}}),
        &mut runtime,
    )
    .expect("response");
    assert_eq!(response["result"]["isError"], true);
    fs::remove_dir_all(repo_root).expect("cleanup");
}

#[test]
fn browser_mcp_background_inspect_requires_job_id() {
    let repo_root = temp_root("bg-inspect-no-id");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let response = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "background_inspect", "arguments": {}}}),
        &mut runtime,
    )
    .expect("response");
    assert_eq!(response["result"]["isError"], true);
    fs::remove_dir_all(repo_root).expect("cleanup");
}

#[test]
fn browser_mcp_background_terminate_requires_job_id() {
    let repo_root = temp_root("bg-terminate-no-id");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let response = handle_browser_mcp_request(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "background_terminate", "arguments": {}}}),
        &mut runtime,
    )
    .expect("response");
    assert_eq!(response["result"]["isError"], true);
    fs::remove_dir_all(repo_root).expect("cleanup");
}

#[test]
fn browser_mcp_invalid_json_returns_error() {
    let repo_root = temp_root("invalid-json");
    let mut runtime = BrowserRuntime::new(repo_root.clone());
    let input = Cursor::new("not valid json\n");
    let mut output = Vec::new();
    run_browser_mcp_stdio(input, &mut output, &mut runtime).expect("run");
    let lines = String::from_utf8(output).expect("utf8");
    let response: Value = serde_json::from_str(lines.trim()).expect("json");
    assert_eq!(response["error"]["code"], -32000);
    fs::remove_dir_all(repo_root).expect("cleanup");
}
