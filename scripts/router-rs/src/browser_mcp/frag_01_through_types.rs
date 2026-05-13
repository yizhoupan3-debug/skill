// MCP 常量、transport、JSON-RPC、`BrowserRuntime`/会话类型与 `struct CdpClient`（须整体移动，不得在函数中途截断）。
use crate::background_state::handle_background_state_operation;
use crate::cli::args::{TraceStreamInspectRequestPayload, TraceStreamReplayRequestPayload};
use crate::cli::runtime_ops::{
    attach_runtime_event_transport, inspect_trace_stream, replay_trace_stream,
};
use crate::framework_runtime::resolve_repo_root_arg;
use crate::route::{
    build_search_results_payload, load_records, load_records_from_manifest, route_task,
    search_skills, should_accept_manifest_fallback, should_retry_with_manifest, RouteDecision,
    SkillRecord,
};
use crate::session_supervisor::handle_session_supervisor_operation;
use chrono::{Local, SecondsFormat};
use rusqlite::Connection;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::{self, BufRead, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{connect, Message, WebSocket};

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "browser-mcp";
const SERVER_VERSION: &str = "0.3.0-rust";
/// Upper bound for a single Content-Length framed message body (aligned with hook stdin caps).
const MAX_BROWSER_MCP_CONTENT_LENGTH: usize = 4 * 1024 * 1024;
const DEFAULT_WAIT_MS: u64 = 5_000;
const DEFAULT_MAX_ELEMENTS: usize = 100;
const DEFAULT_TEXT_BUDGET: usize = 4_000;
const DEFAULT_NETWORK_LIMIT: usize = 50;
const MAX_NETWORK_EVENTS: usize = 200;
const SNAPSHOT_HISTORY_LIMIT: usize = 8;
const CDP_RECV_TIMEOUT: Duration = Duration::from_secs(6);
const RUNTIME_ATTACH_DESCRIPTOR_SCHEMA_VERSION: &str = "runtime-event-attach-descriptor-v1";
const RUNTIME_ATTACH_MODE: &str = "process_external_artifact_replay";
const RUNTIME_ATTACH_SOURCE_TRANSPORT_METHOD: &str = "describe_runtime_event_transport";
const RUNTIME_ATTACH_SOURCE_HANDOFF_METHOD: &str = "describe_runtime_event_handoff";
const RUNTIME_ATTACH_METHOD: &str = "attach_runtime_event_transport";
const RUNTIME_ATTACH_SUBSCRIBE_METHOD: &str = "subscribe_attached_runtime_events";
const RUNTIME_ATTACH_CLEANUP_METHOD: &str = "cleanup_attached_runtime_event_transport";
const RUNTIME_ATTACH_RESUME_MODE: &str = "after_event_id";
const RUNTIME_EVENT_TRANSPORT_SCHEMA_VERSION: &str = "runtime-event-transport-v1";
const RUNTIME_EVENT_HANDOFF_SCHEMA_VERSION: &str = "runtime-event-handoff-v1";
const TRACE_RESUME_MANIFEST_SCHEMA_VERSION: &str = "runtime-resume-manifest-v1";
const ROUTER_RS_TRACE_STREAM_REPLAY_SCHEMA_VERSION: &str = "router-rs-trace-stream-replay-v1";
const ROUTER_RS_TRACE_STREAM_INSPECT_SCHEMA_VERSION: &str = "router-rs-trace-stream-inspect-v1";
const ROUTER_RS_TRACE_IO_AUTHORITY: &str = "rust-runtime-trace-io";
const BACKGROUND_STATE_REQUEST_SCHEMA_VERSION: &str = "router-rs-background-state-request-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BrowserMcpTransportMode {
    ContentLength,
    NewlineDelimited,
}

pub fn run_browser_mcp_stdio_loop(
    repo_root: Option<&Path>,
    attach_config: BrowserAttachConfig,
) -> Result<(), String> {
    let repo_root = resolve_repo_root_arg(repo_root)?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut runtime = BrowserRuntime::with_attach_config(repo_root, attach_config);
    run_browser_mcp_stdio(stdin.lock(), stdout.lock(), &mut runtime)
}

pub fn resolve_browser_mcp_attach_artifact(
    repo_root: &Path,
    search_root: Option<&Path>,
) -> Option<String> {
    let roots = search_root
        .map(|root| vec![root.to_path_buf()])
        .unwrap_or_else(|| default_attach_discovery_roots(repo_root));
    select_attach_artifact_candidate(roots)
}

pub fn run_browser_mcp_stdio<R: BufRead, W: Write>(
    mut input: R,
    mut output: W,
    runtime: &mut BrowserRuntime,
) -> Result<(), String> {
    let mut transport_mode = None;
    while let Some(message) = read_browser_mcp_message(&mut input, &mut transport_mode)? {
        if let Some(response) = handle_browser_mcp_line(&message, runtime) {
            write_browser_mcp_response(
                &mut output,
                transport_mode.unwrap_or(BrowserMcpTransportMode::NewlineDelimited),
                &response,
            )?;
        }
    }
    let _ = runtime.shutdown();
    Ok(())
}

fn read_browser_mcp_message<R: BufRead>(
    input: &mut R,
    transport_mode: &mut Option<BrowserMcpTransportMode>,
) -> Result<Option<String>, String> {
    let mut first_line = String::new();
    loop {
        first_line.clear();
        let bytes = input
            .read_line(&mut first_line)
            .map_err(|err| format!("read browser MCP request failed: {err}"))?;
        if bytes == 0 {
            return Ok(None);
        }
        if !first_line.trim().is_empty() {
            break;
        }
    }

    if first_line
        .to_ascii_lowercase()
        .starts_with("content-length:")
    {
        *transport_mode = Some(BrowserMcpTransportMode::ContentLength);
        let content_length = parse_content_length_header(&first_line)?;
        if content_length > MAX_BROWSER_MCP_CONTENT_LENGTH {
            return Err(format!(
                "browser MCP Content-Length {content_length} exceeds max {MAX_BROWSER_MCP_CONTENT_LENGTH}"
            ));
        }
        loop {
            let mut header = String::new();
            let bytes = input
                .read_line(&mut header)
                .map_err(|err| format!("read browser MCP header failed: {err}"))?;
            if bytes == 0 {
                return Err("browser MCP header ended before blank line".to_string());
            }
            if header.trim().is_empty() {
                break;
            }
        }
        let mut body = vec![0_u8; content_length];
        input
            .read_exact(&mut body)
            .map_err(|err| format!("read browser MCP body failed: {err}"))?;
        return String::from_utf8(body)
            .map(Some)
            .map_err(|err| format!("decode browser MCP body failed: {err}"));
    }

    if transport_mode.is_none() {
        *transport_mode = Some(BrowserMcpTransportMode::NewlineDelimited);
    }
    Ok(Some(first_line.trim_end().to_string()))
}

fn parse_content_length_header(line: &str) -> Result<usize, String> {
    let (_, value) = line
        .split_once(':')
        .ok_or_else(|| format!("invalid browser MCP header: {line}"))?;
    value
        .trim()
        .parse::<usize>()
        .map_err(|err| format!("invalid browser MCP content length '{value}': {err}"))
}

fn write_browser_mcp_response<W: Write>(
    output: &mut W,
    transport_mode: BrowserMcpTransportMode,
    response: &Value,
) -> Result<(), String> {
    let encoded = serde_json::to_string(response)
        .map_err(|err| format!("serialize browser MCP response failed: {err}"))?;
    match transport_mode {
        BrowserMcpTransportMode::ContentLength => {
            write!(output, "Content-Length: {}\r\n\r\n{encoded}", encoded.len())
                .map_err(|err| format!("write browser MCP response failed: {err}"))?;
        }
        BrowserMcpTransportMode::NewlineDelimited => {
            writeln!(output, "{encoded}")
                .map_err(|err| format!("write browser MCP response failed: {err}"))?;
        }
    }
    output
        .flush()
        .map_err(|err| format!("flush browser MCP response failed: {err}"))?;
    Ok(())
}

fn handle_browser_mcp_line(line: &str, runtime: &mut BrowserRuntime) -> Option<Value> {
    let request = match serde_json::from_str::<Value>(line) {
        Ok(value) => value,
        Err(err) => {
            return Some(error_response(
                Value::Null,
                browser_error(
                    "INVALID_INPUT",
                    &format!("Invalid JSON input: {err}"),
                    &["send one JSON-RPC object per line"],
                    true,
                ),
            ))
        }
    };
    handle_browser_mcp_request(&request, runtime)
}

fn handle_browser_mcp_request(request: &Value, runtime: &mut BrowserRuntime) -> Option<Value> {
    let request_id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
    if method == "notifications/initialized" {
        return None;
    }
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "serverInfo": {"name": SERVER_NAME, "version": SERVER_VERSION},
            "capabilities": {"tools": {"listChanged": false}},
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tool_definitions(&runtime.repo_root)})),
        "tools/call" => handle_tools_call(&params, runtime),
        _ => Err(browser_error(
            "UNSUPPORTED_OPERATION",
            &format!("Unsupported JSON-RPC method: {method}"),
            &["call initialize", "call tools/list"],
            true,
        )),
    };
    Some(match result {
        Ok(payload) => success_response(request_id, payload),
        Err(error) => error_response(request_id, error),
    })
}

fn handle_tools_call(params: &Value, runtime: &mut BrowserRuntime) -> Result<Value, Value> {
    let tool_name = require_string(params, "name")?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let structured = match tool_name.as_str() {
        "browser_open" => runtime.open(&arguments),
        "browser_tabs" => runtime.tabs(&arguments),
        "browser_close" => runtime.close(&arguments),
        "browser_get_state" => runtime.get_state(&arguments),
        "browser_get_elements" => runtime.get_elements(&arguments),
        "browser_get_text" => runtime.get_text(&arguments),
        "browser_get_network" => runtime.get_network(&arguments),
        "browser_screenshot" => return runtime.screenshot_result(&arguments),
        "browser_click" => runtime.click(&arguments),
        "browser_fill" => runtime.fill(&arguments),
        "browser_press" => runtime.press(&arguments),
        "browser_wait_for" => runtime.wait_for(&arguments),
        "browser_save_session" => runtime.save_session(&arguments),
        "browser_restore_session" => runtime.restore_session(&arguments),
        "browser_get_attached_runtime_events" => runtime.get_attached_runtime_events(&arguments),
        "runtime_heartbeat" => runtime.runtime_heartbeat(&arguments),
        "session_launch" => runtime.session_launch(&arguments),
        "session_list" => runtime.session_list(&arguments),
        "session_inspect" => runtime.session_inspect(&arguments),
        "session_terminate" => runtime.session_terminate(&arguments),
        "session_mark_blocked" => runtime.session_mark_blocked(&arguments),
        "session_resume_due" => runtime.session_resume_due(&arguments),
        "session_classify_block" => runtime.session_classify_block(&arguments),
        "background_list" => runtime.background_list(&arguments),
        "background_inspect" => runtime.background_inspect(&arguments),
        "background_terminate" => runtime.background_terminate(&arguments),
        "skill_route" => runtime.skill_route(&arguments),
        "skill_search" => runtime.skill_search(&arguments),
        "skill_read" => runtime.skill_read(&arguments),
        "skill_route_status" => runtime.skill_route_status(),
        "browser_diagnostics" => runtime.diagnostics(&arguments),
        _ => Err(browser_error(
            "INVALID_INPUT",
            &format!("Unknown tool name: {tool_name}"),
            &["call tools/list to inspect available browser tools"],
            true,
        )),
    };
    tool_result(structured)
}

fn tool_result(structured: Result<Value, Value>) -> Result<Value, Value> {
    match structured {
        Ok(payload) => Ok(json!({
            "structuredContent": payload,
            "content": [{"type": "text", "text": serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string())}],
            "isError": false,
        })),
        Err(error) => {
            let payload = json!({"ok": false, "error": error});
            Ok(json!({
                "structuredContent": payload,
                "content": [{"type": "text", "text": serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string())}],
                "isError": true,
            }))
        }
    }
}

fn tool_definitions(repo_root: &Path) -> Vec<Value> {
    let empty_output = json!({"type": "object", "additionalProperties": true});
    let skill_tools_available = skill_runtime_available(repo_root);
    let mut tools = vec![
        tool_definition(
            "browser_open",
            "Open Browser Page",
            "Open a page in the current browser session and return the active tab.",
            json!({"type": "object", "properties": {"url": {"type": "string"}, "newTab": {"type": "boolean"}}, "required": ["url"]}),
            empty_output.clone(),
        ),
        tool_definition(
            "browser_tabs",
            "List Or Select Tabs",
            "List current tabs or switch the active tab.",
            json!({"type": "object", "properties": {"action": {"type": "string", "enum": ["list", "select"]}, "tabId": {"type": "string"}}, "required": ["action"]}),
            empty_output.clone(),
        ),
        tool_definition(
            "browser_close",
            "Close Tab Or Session",
            "Close a single tab or the entire session.",
            json!({"type": "object", "properties": {"target": {"type": "string", "enum": ["tab", "session"]}, "tabId": {"type": "string"}}, "required": ["target"]}),
            empty_output.clone(),
        ),
        tool_definition(
            "browser_get_state",
            "Get Compressed Page State",
            "Return a compressed page summary, interactive elements, and an optional diff.",
            json!({"type": "object", "properties": {"tabId": {"type": "string"}, "include": {"type": "array", "items": {"type": "string", "enum": ["summary", "interactive_elements", "diff"]}}, "sinceRevision": {"type": "integer", "minimum": 0}, "maxElements": {"type": "integer", "minimum": 1, "maximum": 100}, "textBudget": {"type": "integer", "minimum": 1, "maximum": 4000}}}),
            empty_output.clone(),
        ),
        tool_definition(
            "browser_get_elements",
            "Get Interactive Elements",
            "Return filtered interactive elements using role and text query.",
            json!({"type": "object", "properties": {"tabId": {"type": "string"}, "role": {"type": "string"}, "query": {"type": "string"}, "scopeRef": {"type": "string"}, "limit": {"type": "integer", "minimum": 1, "maximum": 100}}}),
            empty_output.clone(),
        ),
        tool_definition(
            "browser_get_text",
            "Get Visible Text",
            "Return visible text for the page or a specific element scope.",
            json!({"type": "object", "properties": {"tabId": {"type": "string"}, "scopeRef": {"type": "string"}, "maxChars": {"type": "integer", "minimum": 1, "maximum": 8000}}}),
            empty_output.clone(),
        ),
        tool_definition(
            "browser_get_network",
            "Get Recent Network Requests",
            "Return recent network requests including status, timing, and optional bodies.",
            json!({"type": "object", "properties": {"tabId": {"type": "string"}, "sinceSeconds": {"type": "integer", "minimum": 0}, "resourceTypes": {"type": "array", "items": {"type": "string"}}, "limit": {"type": "integer", "minimum": 1}}}),
            empty_output.clone(),
        ),
        tool_definition(
            "browser_screenshot",
            "Take Screenshot",
            "Take a screenshot and return it as an inline image.",
            json!({"type": "object", "properties": {"tabId": {"type": "string"}, "scopeRef": {"type": "string"}, "fullPage": {"type": "boolean"}}}),
            empty_output.clone(),
        ),
        tool_definition(
            "browser_click",
            "Click Element",
            "Click an indexed element and return an incremental page delta.",
            json!({"type": "object", "properties": {"tabId": {"type": "string"}, "ref": {"type": "string"}, "timeoutMs": {"type": "integer", "minimum": 1}}, "required": ["ref"]}),
            empty_output.clone(),
        ),
        tool_definition(
            "browser_fill",
            "Fill Element",
            "Fill an input, optionally submit, and return an incremental page delta.",
            json!({"type": "object", "properties": {"tabId": {"type": "string"}, "ref": {"type": "string"}, "value": {"type": "string"}, "submit": {"type": "boolean"}}, "required": ["ref", "value"]}),
            empty_output.clone(),
        ),
        tool_definition(
            "browser_press",
            "Press Key",
            "Press a keyboard key on the active page.",
            json!({"type": "object", "properties": {"tabId": {"type": "string"}, "key": {"type": "string"}}, "required": ["key"]}),
            empty_output.clone(),
        ),
        tool_definition(
            "browser_wait_for",
            "Wait For Condition",
            "Wait for one explicit page condition without re-reading the whole page.",
            json!({"type": "object", "properties": {"tabId": {"type": "string"}, "condition": {"type": "object", "properties": {"type": {"type": "string", "enum": ["text_appears", "text_disappears", "element_appears", "element_disappears", "url_contains", "network_idle"]}, "value": {"type": "string"}}, "required": ["type"]}, "timeoutMs": {"type": "integer", "minimum": 1}}, "required": ["condition"]}),
            empty_output.clone(),
        ),
        tool_definition(
            "browser_save_session",
            "Save Browser Session",
            "Save the current browser context storage state to disk.",
            json!({"type": "object", "properties": {"sessionPath": {"type": "string"}}}),
            empty_output.clone(),
        ),
        tool_definition(
            "browser_restore_session",
            "Restore Browser Session",
            "Restore a previously saved browser session from disk.",
            json!({"type": "object", "properties": {"sessionPath": {"type": "string"}}, "required": ["sessionPath"]}),
            empty_output.clone(),
        ),
        tool_definition(
            "browser_get_attached_runtime_events",
            "Replay Attached Runtime Events",
            "Replay runtime events through the configured Rust attach descriptor.",
            json!({"type": "object", "properties": {"afterEventId": {"type": "string"}, "limit": {"type": "integer", "minimum": 1}, "heartbeat": {"type": "boolean"}}}),
            empty_output.clone(),
        ),
        tool_definition(
            "runtime_heartbeat",
            "Heartbeat Attached Runtime",
            "Emit an idle heartbeat when no new attached runtime events are available.",
            json!({"type": "object", "properties": {"afterEventId": {"type": "string"}, "limit": {"type": "integer", "minimum": 1}}}),
            empty_output.clone(),
        ),
        tool_definition(
            "session_launch",
            "Launch Session Worker",
            "Launch one long-running worker session through the Rust session supervisor.",
            json!({"type": "object", "properties": {"statePath": {"type": "string"}, "host": {"type": "string"}, "cwd": {"type": "string"}, "prompt": {"type": "string"}, "resumeTarget": {"type": "string"}, "resumeMode": {"type": "string"}, "workerId": {"type": "string"}, "tmuxSession": {"type": "string"}, "nativeTmux": {"type": "boolean"}, "dryRun": {"type": "boolean"}}, "required": ["host", "cwd"]}),
            empty_output.clone(),
        ),
        tool_definition(
            "session_list",
            "List Session Workers",
            "List current session supervisor workers and refresh their runtime state.",
            json!({"type": "object", "properties": {"statePath": {"type": "string"}}}),
            empty_output.clone(),
        ),
        tool_definition(
            "session_inspect",
            "Inspect Session Worker",
            "Inspect one session supervisor worker by worker id.",
            json!({"type": "object", "properties": {"statePath": {"type": "string"}, "workerId": {"type": "string"}}, "required": ["workerId"]}),
            empty_output.clone(),
        ),
        tool_definition(
            "session_terminate",
            "Terminate Session Worker",
            "Terminate one session supervisor worker by worker id.",
            json!({"type": "object", "properties": {"statePath": {"type": "string"}, "workerId": {"type": "string"}, "dryRun": {"type": "boolean"}}, "required": ["workerId"]}),
            empty_output.clone(),
        ),
        tool_definition(
            "session_mark_blocked",
            "Mark Session Blocked",
            "Mark a worker blocked with evidence so resume policy can back off.",
            json!({"type": "object", "properties": {"statePath": {"type": "string"}, "workerId": {"type": "string"}, "host": {"type": "string"}, "blockedReason": {"type": "string"}, "evidenceText": {"type": "string"}, "backoffSeconds": {"type": "integer"}}, "required": ["workerId", "host", "evidenceText"]}),
            empty_output.clone(),
        ),
        tool_definition(
            "session_resume_due",
            "Resume Due Sessions",
            "Resume all due blocked workers using the supervisor resume policy.",
            json!({"type": "object", "properties": {"statePath": {"type": "string"}, "dryRun": {"type": "boolean"}}}),
            empty_output.clone(),
        ),
        tool_definition(
            "session_classify_block",
            "Classify Session Block",
            "Classify a rate-limit/block signal from host evidence text.",
            json!({"type": "object", "properties": {"statePath": {"type": "string"}, "host": {"type": "string"}, "evidenceText": {"type": "string"}}, "required": ["host", "evidenceText"]}),
            empty_output.clone(),
        ),
        tool_definition(
            "background_list",
            "List Background Jobs",
            "Return the background job snapshot from Rust durable state.",
            json!({"type": "object", "properties": {"statePath": {"type": "string"}, "backendFamily": {"type": "string"}, "sqliteDbPath": {"type": "string"}}}),
            empty_output.clone(),
        ),
        tool_definition(
            "background_inspect",
            "Inspect Background Job",
            "Return one background job by job id from Rust durable state.",
            json!({"type": "object", "properties": {"statePath": {"type": "string"}, "backendFamily": {"type": "string"}, "sqliteDbPath": {"type": "string"}, "jobId": {"type": "string"}}, "required": ["jobId"]}),
            empty_output.clone(),
        ),
        tool_definition(
            "background_terminate",
            "Terminate Background Job",
            "Mark one background job interrupted in Rust durable state.",
            json!({"type": "object", "properties": {"statePath": {"type": "string"}, "backendFamily": {"type": "string"}, "sqliteDbPath": {"type": "string"}, "jobId": {"type": "string"}, "error": {"type": "string"}}, "required": ["jobId"]}),
            empty_output.clone(),
        ),
    ];
    if skill_tools_available {
        tools.extend(skill_tool_definitions(empty_output.clone()));
    }
    tools.push(tool_definition(
        "browser_diagnostics",
        "Browser Diagnostics",
        "Return runtime health information, including whether repository skill routing tools are exposed.",
        json!({"type": "object", "properties": {}}),
        empty_output,
    ));
    if !skill_tools_available {
        tools.push(tool_definition(
            "skill_route_status",
            "Repository Skill Route Status",
            "Explain why repository skill routing tools are not exposed for this repo root.",
            json!({"type": "object", "properties": {}}),
            json!({"type": "object", "additionalProperties": true}),
        ));
    }
    tools
}

fn skill_tool_definitions(empty_output: Value) -> Vec<Value> {
    vec![
        tool_definition(
            "skill_route",
            "Route Skill Request",
            "Route a user request through this repository's skills/SKILL_ROUTING_RUNTIME.json, with SKILL_MANIFEST.json fallback, and return the selected skill plus the exact SKILL.md path to read.",
            json!({"type": "object", "properties": {"query": {"type": "string"}, "sessionId": {"type": "string"}, "allowOverlay": {"type": "boolean"}, "firstTurn": {"type": "boolean"}}, "required": ["query"]}),
            empty_output.clone(),
        ),
        tool_definition(
            "skill_search",
            "Search Repository Skills",
            "Search this repository's full skills/SKILL_MANIFEST.json catalog and return the best matching skill records.",
            json!({"type": "object", "properties": {"query": {"type": "string"}, "limit": {"type": "integer", "minimum": 1, "maximum": 50}}, "required": ["query"]}),
            empty_output.clone(),
        ),
        tool_definition(
            "skill_read",
            "Read Repository Skill",
            "Read one matched skills/<name>/SKILL.md body from this repository's canonical skills/ source.",
            json!({"type": "object", "properties": {"skill": {"type": "string"}, "maxChars": {"type": "integer", "minimum": 1, "maximum": 50000}}, "required": ["skill"]}),
            empty_output,
        ),
    ]
}

fn tool_definition(
    name: &str,
    title: &str,
    description: &str,
    input_schema: Value,
    output_schema: Value,
) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": input_schema,
        "outputSchema": output_schema,
    })
}

pub struct BrowserRuntime {
    repo_root: PathBuf,
    attach_config: BrowserAttachConfig,
    sessions: HashMap<String, SessionRecord>,
    browser_processes: HashMap<String, Child>,
    session_counter: usize,
    tab_counter: usize,
    ref_counter: usize,
    request_counter: usize,
    screenshot_counter: usize,
}

#[derive(Clone, Debug, Default)]
pub struct BrowserAttachConfig {
    runtime_attach_descriptor_path: Option<String>,
    runtime_attach_artifact_path: Option<String>,
    headless: bool,
}

impl BrowserAttachConfig {
    pub fn from_cli_and_env(
        runtime_attach_descriptor_path: Option<String>,
        runtime_attach_artifact_path: Option<String>,
        headless: Option<String>,
    ) -> Self {
        Self {
            runtime_attach_descriptor_path: runtime_attach_descriptor_path
                .or_else(|| env_non_empty("BROWSER_MCP_RUNTIME_ATTACH_DESCRIPTOR_PATH")),
            runtime_attach_artifact_path: runtime_attach_artifact_path
                .or_else(|| env_non_empty("BROWSER_MCP_RUNTIME_ATTACH_ARTIFACT_PATH")),
            headless: resolve_headless_option(headless),
        }
    }
}

#[derive(Clone, Debug)]
struct ConfiguredAttachSource {
    source: Option<&'static str>,
    path: Option<String>,
}

#[derive(Clone, Debug)]
struct LoadedRuntimeAttachDescriptor {
    descriptor: Value,
    input_artifact_kind: Option<&'static str>,
}

struct ResolvedAttachedRuntimeDescriptorContext {
    trace_stream_path: String,
    diagnostics_base: Value,
}

struct SessionRecord {
    id: String,
    created_at: String,
    viewport: ViewportSize,
    current_tab_id: Option<String>,
    tabs: HashMap<String, TabRecord>,
    _browser_pid: u32,
    user_data_dir: PathBuf,
    cdp: CdpClient,
}

struct TabRecord {
    id: String,
    target_id: String,
    session_id: String,
    url: String,
    title: String,
    page_revision: u64,
    loading_state: String,
    indexed_elements: HashMap<String, InteractiveElement>,
    fingerprint_to_ref: HashMap<String, String>,
    last_snapshot: Option<PageSnapshot>,
    snapshot_history: VecDeque<PageSnapshot>,
    network_events: Vec<NetworkEvent>,
}

#[derive(Clone, Copy, Debug)]
struct ViewportSize {
    width: u64,
    height: u64,
}

#[derive(Clone, Debug)]
struct InteractiveElement {
    ref_id: String,
    page_revision: u64,
    role: String,
    name: String,
    text: String,
    visible: bool,
    enabled: bool,
    tag: String,
    test_id: Option<String>,
    fingerprint: String,
    selector: String,
}

#[derive(Clone, Debug)]
struct ElementDescriptor {
    role: String,
    name: String,
    text: String,
    visible: bool,
    enabled: bool,
    tag: String,
    test_id: Option<String>,
    _ordinal: usize,
    selector: String,
}

#[derive(Clone, Debug)]
struct PageSnapshot {
    revision: u64,
    url: String,
    title: String,
    loading_state: String,
    summary: Value,
    interactive_elements: Vec<InteractiveElement>,
    text_content: String,
    text_lines: Vec<String>,
    _created_at: u128,
}

#[derive(Clone, Debug)]
struct NetworkEvent {
    id: String,
    method: String,
    url: String,
    status: Option<i64>,
    content_type: Option<String>,
    resource_type: String,
    timestamp: u128,
    ok: bool,
    error_text: Option<String>,
    duration_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AttachArtifactCandidate {
    path: String,
    rank: AttachArtifactCandidateRank,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct AttachArtifactCandidateRank {
    updated_at_ms: i64,
    recency_ms: i64,
    source_priority: i32,
}

struct CdpClient {
    _port: u16,
    next_id: u64,
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
}

#[cfg(test)]
mod browser_mcp_body_limit_tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn rejects_oversized_content_length() {
        let mut input = Cursor::new(format!(
            "Content-Length: {}\r\n\r\n",
            MAX_BROWSER_MCP_CONTENT_LENGTH + 1
        ));
        let mut mode = None;
        let err = read_browser_mcp_message(&mut input, &mut mode).unwrap_err();
        assert!(err.contains("exceeds max"), "{err}");
    }
}
