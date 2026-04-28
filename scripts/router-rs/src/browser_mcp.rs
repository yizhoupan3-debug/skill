use crate::framework_runtime::resolve_repo_root_arg;
use crate::route::{
    build_search_results_payload, load_records, load_records_from_manifest, route_task,
    search_skills, RouteDecision, SkillRecord,
};
use crate::{
    attach_runtime_event_transport, inspect_trace_stream, replay_trace_stream,
    TraceStreamInspectRequestPayload, TraceStreamReplayRequestPayload,
};
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

impl BrowserRuntime {
    #[cfg(test)]
    fn new(repo_root: PathBuf) -> Self {
        Self::with_attach_config(repo_root, BrowserAttachConfig::default())
    }

    fn with_attach_config(repo_root: PathBuf, attach_config: BrowserAttachConfig) -> Self {
        Self {
            repo_root,
            attach_config,
            sessions: HashMap::new(),
            browser_processes: HashMap::new(),
            session_counter: 0,
            tab_counter: 0,
            ref_counter: 0,
            request_counter: 0,
            screenshot_counter: 0,
        }
    }

    fn skill_route(&self, input: &Value) -> Result<Value, Value> {
        let query = required_string_arg(input, "query")?;
        let session_id =
            optional_string(input, "sessionId").unwrap_or_else(|| "cowork-mcp".to_string());
        let allow_overlay = optional_bool(input, "allowOverlay").unwrap_or(true);
        let first_turn = optional_bool(input, "firstTurn").unwrap_or(true);
        let runtime_path = skill_runtime_path(&self.repo_root);
        let manifest_path = skill_manifest_path(&self.repo_root);
        if !runtime_path.is_file() {
            return Err(skill_error(
                "SKILL_RUNTIME_MISSING",
                &format!(
                    "Missing repository skill runtime: {}",
                    runtime_path.display()
                ),
            ));
        }
        let records = load_records(Some(&runtime_path), Some(&manifest_path))
            .map_err(|err| skill_error("SKILL_ROUTE_FAILED", &err))?;
        let decision = route_with_full_manifest_fallback(
            &records,
            &manifest_path,
            &query,
            &session_id,
            allow_overlay,
            first_turn,
        )
        .map_err(|err| skill_error("SKILL_ROUTE_FAILED", &err))?;
        let selected_path = skill_body_path(&self.repo_root, &decision.selected_skill)
            .map_err(|err| skill_error("SKILL_READ_BLOCKED", &err))?;
        let overlay_path = decision
            .overlay_skill
            .as_ref()
            .map(|slug| skill_body_path(&self.repo_root, slug))
            .transpose()
            .map_err(|err| skill_error("SKILL_READ_BLOCKED", &err))?;
        Ok(json!({
            "schema_version": "cowork-skill-route-v1",
            "authority": "router-rs-browser-mcp",
            "repo_root": self.repo_root.to_string_lossy(),
            "runtime_path": runtime_path.to_string_lossy(),
            "manifest_path": manifest_path.to_string_lossy(),
            "decision": decision,
            "selected_skill_path": selected_path.to_string_lossy(),
            "overlay_skill_path": overlay_path.map(|path| path.to_string_lossy().to_string()),
            "next_step": "Read selected_skill_path from the canonical skills/ source before doing task work.",
        }))
    }

    fn skill_search(&self, input: &Value) -> Result<Value, Value> {
        let query = required_string_arg(input, "query")?;
        let limit = optional_u64(input, "limit")?.unwrap_or(10).clamp(1, 50) as usize;
        let manifest_path = skill_manifest_path(&self.repo_root);
        if !manifest_path.is_file() {
            return Err(skill_error(
                "SKILL_MANIFEST_MISSING",
                &format!(
                    "Missing repository skill manifest: {}",
                    manifest_path.display()
                ),
            ));
        }
        let records = load_records_from_manifest(&manifest_path)
            .map_err(|err| skill_error("SKILL_SEARCH_FAILED", &err))?;
        let rows = search_skills(&records, &query, limit);
        let results = build_search_results_payload(&query, rows);
        serde_json::to_value(results)
            .map_err(|err| skill_error("SKILL_SEARCH_FAILED", &err.to_string()))
    }

    fn skill_read(&self, input: &Value) -> Result<Value, Value> {
        let slug = required_string_arg(input, "skill")?;
        let max_chars = optional_u64(input, "maxChars")?
            .unwrap_or(20_000)
            .clamp(1, 50_000) as usize;
        let path = skill_body_path(&self.repo_root, &slug)
            .map_err(|err| skill_error("SKILL_READ_BLOCKED", &err))?;
        let content = fs::read_to_string(&path).map_err(|err| {
            skill_error("SKILL_READ_FAILED", &format!("{}: {err}", path.display()))
        })?;
        let truncated = content.chars().count() > max_chars;
        Ok(json!({
            "schema_version": "cowork-skill-read-v1",
            "authority": "router-rs-browser-mcp",
            "skill": slug,
            "path": path.to_string_lossy(),
            "content": truncate_text(&content, max_chars),
            "truncated": truncated,
        }))
    }

    fn skill_route_status(&self) -> Result<Value, Value> {
        let runtime_path = skill_runtime_path(&self.repo_root);
        let manifest_path = skill_manifest_path(&self.repo_root);
        Ok(json!({
            "schema_version": "cowork-skill-route-status-v1",
            "authority": "router-rs-browser-mcp",
            "repo_root": self.repo_root.to_string_lossy(),
            "skills_dir_exists": self.repo_root.join("skills").is_dir(),
            "runtime_path": runtime_path.to_string_lossy(),
            "runtime_exists": runtime_path.is_file(),
            "manifest_path": manifest_path.to_string_lossy(),
            "manifest_exists": manifest_path.is_file(),
            "routing_tools_exposed": skill_runtime_available(&self.repo_root),
        }))
    }

    fn open(&mut self, input: &Value) -> Result<Value, Value> {
        let url = required_string_arg(input, "url")?;
        let new_tab = optional_bool(input, "newTab").unwrap_or(false);
        let session_id = self.get_or_create_session()?;
        let tab_id = {
            let current_tab_id = self
                .sessions
                .get(&session_id)
                .and_then(|session| session.current_tab_id.clone());
            if new_tab || current_tab_id.is_none() {
                self.create_tab(&session_id)?
            } else {
                current_tab_id.unwrap_or_default()
            }
        };

        let session_cdp_id = self.tab_session_id(&session_id, &tab_id)?;
        let cdp = self.cdp_mut(&session_id)?;
        cdp.call(Some(&session_cdp_id), "Page.navigate", json!({"url": url}))?;
        self.wait_for_page_ready(&session_id, &tab_id, DEFAULT_WAIT_MS)?;
        self.refresh_snapshot(&session_id, &tab_id)?;
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.current_tab_id = Some(tab_id.clone());
        }

        Ok(json!({
            "session": self.session_view(&session_id)?,
            "tab": self.tab_view(&session_id, &tab_id)?,
        }))
    }

    fn tabs(&mut self, input: &Value) -> Result<Value, Value> {
        let action = required_string_arg(input, "action")?;
        let session_id = self.required_session_id()?;
        if action == "select" {
            let tab_id = required_string_arg(input, "tabId")?;
            if !self
                .sessions
                .get(&session_id)
                .is_some_and(|session| session.tabs.contains_key(&tab_id))
            {
                return Err(browser_error(
                    "TAB_NOT_FOUND",
                    &format!("Tab {tab_id} was not found."),
                    &["call browser_tabs with action=list"],
                    true,
                ));
            }
            if let Some(session) = self.sessions.get_mut(&session_id) {
                session.current_tab_id = Some(tab_id);
            }
        } else if action != "list" {
            return Err(browser_error(
                "INVALID_INPUT",
                "action must be list or select.",
                &["pass action=list or action=select"],
                true,
            ));
        }

        let session = self
            .sessions
            .get(&session_id)
            .ok_or_else(session_not_found_error)?;
        let tabs = session
            .tabs
            .keys()
            .map(|tab_id| self.tab_view(&session_id, tab_id))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(json!({"currentTabId": session.current_tab_id, "tabs": tabs}))
    }

    fn close(&mut self, input: &Value) -> Result<Value, Value> {
        let target = required_string_arg(input, "target")?;
        let session_id = self.required_session_id()?;
        if target == "session" {
            let remaining_tabs = self
                .sessions
                .get(&session_id)
                .map(|session| session.tabs.len())
                .unwrap_or_default();
            self.dispose_session(&session_id)?;
            return Ok(json!({"ok": true, "closed": "session", "remainingTabs": remaining_tabs}));
        }
        if target != "tab" {
            return Err(browser_error(
                "INVALID_INPUT",
                "target must be tab or session.",
                &["pass target=tab or target=session"],
                true,
            ));
        }
        let tab_id = optional_string(input, "tabId")
            .or_else(|| {
                self.sessions
                    .get(&session_id)
                    .and_then(|session| session.current_tab_id.clone())
            })
            .ok_or_else(|| {
                browser_error(
                    "TAB_NOT_FOUND",
                    "No active tab is available.",
                    &["call browser_open"],
                    true,
                )
            })?;
        let target_id = self
            .sessions
            .get(&session_id)
            .and_then(|session| session.tabs.get(&tab_id))
            .map(|tab| tab.target_id.clone())
            .ok_or_else(|| {
                browser_error(
                    "TAB_NOT_FOUND",
                    &format!("Tab {tab_id} was not found."),
                    &["call browser_tabs with action=list"],
                    true,
                )
            })?;
        let cdp = self.cdp_mut(&session_id)?;
        let _ = cdp.call(None, "Target.closeTarget", json!({"targetId": target_id}));
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.tabs.remove(&tab_id);
            session.current_tab_id = session.tabs.keys().next().cloned();
            let remaining = session.tabs.len();
            if remaining == 0 {
                let _ = self.dispose_session(&session_id);
            }
            return Ok(json!({"ok": true, "closed": "tab", "remainingTabs": remaining}));
        }
        Err(session_not_found_error())
    }

    fn get_state(&mut self, input: &Value) -> Result<Value, Value> {
        let (session_id, tab_id) = self.resolve_tab_ids(input)?;
        let previous = self
            .sessions
            .get(&session_id)
            .and_then(|session| session.tabs.get(&tab_id))
            .and_then(|tab| tab.last_snapshot.clone());
        let snapshot = self.refresh_snapshot(&session_id, &tab_id)?;
        let include = optional_string_array(input, "include").unwrap_or_else(|| {
            vec![
                "summary".to_string(),
                "interactive_elements".to_string(),
                "diff".to_string(),
            ]
        });
        let max_elements = optional_usize(input, "maxElements", DEFAULT_MAX_ELEMENTS)?;
        let text_budget = optional_usize(input, "textBudget", DEFAULT_TEXT_BUDGET)?;
        let since_revision = optional_u64(input, "sinceRevision")?;
        let base_snapshot = if let Some(revision) = since_revision {
            self.sessions
                .get(&session_id)
                .and_then(|session| session.tabs.get(&tab_id))
                .and_then(|tab| {
                    tab.snapshot_history
                        .iter()
                        .find(|snapshot| snapshot.revision == revision)
                        .cloned()
                })
        } else {
            previous
        };
        if since_revision.is_some() && base_snapshot.is_none() {
            return Err(browser_error(
                "STALE_STATE_REVISION",
                "Requested sinceRevision is no longer retained.",
                &["call browser_get_state without sinceRevision"],
                true,
            ));
        }

        let mut state = Map::new();
        state.insert("tab".to_string(), self.tab_view(&session_id, &tab_id)?);
        if include.iter().any(|item| item == "summary") {
            state.insert(
                "summary".to_string(),
                compact_summary(&snapshot.summary, text_budget),
            );
        }
        if include.iter().any(|item| item == "interactive_elements") {
            state.insert(
                "interactiveElements".to_string(),
                Value::Array(
                    snapshot
                        .interactive_elements
                        .iter()
                        .take(max_elements)
                        .map(interactive_element_value)
                        .collect(),
                ),
            );
        }
        if include.iter().any(|item| item == "diff") {
            let delta = base_snapshot
                .as_ref()
                .map(|base| compute_delta(base, &snapshot))
                .unwrap_or_else(|| {
                    json!({
                        "fromRevision": snapshot.revision,
                        "toRevision": snapshot.revision,
                        "urlChanged": false,
                        "titleChanged": false,
                        "newElements": [],
                        "removedRefs": [],
                        "newText": [],
                        "alerts": [],
                    })
                });
            state.insert("diff".to_string(), delta);
        }
        Ok(Value::Object(state))
    }

    fn get_elements(&mut self, input: &Value) -> Result<Value, Value> {
        let (session_id, tab_id) = self.resolve_tab_ids(input)?;
        let snapshot = self.refresh_snapshot(&session_id, &tab_id)?;
        let role = optional_string(input, "role").map(|value| value.to_lowercase());
        let query = optional_string(input, "query").map(|value| value.to_lowercase());
        let limit = optional_usize(input, "limit", DEFAULT_MAX_ELEMENTS)?;
        let matches = snapshot
            .interactive_elements
            .into_iter()
            .filter(|element| {
                role.as_ref()
                    .map(|role| element.role.to_lowercase() == *role)
                    .unwrap_or(true)
            })
            .filter(|element| {
                query
                    .as_ref()
                    .map(|query| {
                        format!("{} {}", element.name, element.text)
                            .to_lowercase()
                            .contains(query)
                    })
                    .unwrap_or(true)
            })
            .take(limit)
            .map(|element| interactive_element_value(&element))
            .collect::<Vec<_>>();
        Ok(json!({"matches": matches}))
    }

    fn get_text(&mut self, input: &Value) -> Result<Value, Value> {
        let (session_id, tab_id) = self.resolve_tab_ids(input)?;
        let max_chars = optional_usize(input, "maxChars", DEFAULT_TEXT_BUDGET)?;
        let text = if let Some(scope_ref) = optional_string(input, "scopeRef") {
            let selector = self.selector_for_ref(&session_id, &tab_id, &scope_ref)?;
            self.evaluate_string(
                &session_id,
                &tab_id,
                &format!(
                    "(function(){{const el=document.querySelector({}); return el ? (el.innerText || el.textContent || '') : '';}})()",
                    json_string_literal(&selector)
                ),
            )?
        } else {
            self.evaluate_string(
                &session_id,
                &tab_id,
                "document.body ? (document.body.innerText || '').replace(/\\s+$/g, '').trim() : ''",
            )?
        };
        Ok(
            json!({"text": truncate_text(&text, max_chars), "tab": self.tab_view(&session_id, &tab_id)?}),
        )
    }

    fn get_network(&mut self, input: &Value) -> Result<Value, Value> {
        let (session_id, tab_id) = self.resolve_tab_ids(input)?;
        self.drain_cdp_events(&session_id, DEFAULT_WAIT_MS / 5)?;
        let since_seconds = optional_u64(input, "sinceSeconds")?.unwrap_or(20);
        let limit = optional_usize(input, "limit", DEFAULT_NETWORK_LIMIT)?;
        let resource_types = optional_string_array(input, "resourceTypes")
            .unwrap_or_default()
            .into_iter()
            .map(|item| item.to_lowercase())
            .collect::<Vec<_>>();
        let cutoff = now_millis().saturating_sub((since_seconds as u128) * 1000);
        let requests = self
            .sessions
            .get(&session_id)
            .and_then(|session| session.tabs.get(&tab_id))
            .map(|tab| {
                tab.network_events
                    .iter()
                    .filter(|event| event.timestamp >= cutoff)
                    .filter(|event| {
                        resource_types.is_empty()
                            || resource_types.contains(&event.resource_type.to_lowercase())
                    })
                    .rev()
                    .take(limit)
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
            .into_iter()
            .rev()
            .map(network_event_value)
            .collect::<Vec<_>>();
        Ok(json!({"requests": requests}))
    }

    fn screenshot_result(&mut self, input: &Value) -> Result<Value, Value> {
        let (session_id, tab_id) = self.resolve_tab_ids(input)?;
        let full_page = optional_bool(input, "fullPage").unwrap_or(false);
        let clip = if let Some(scope_ref) = optional_string(input, "scopeRef") {
            Some(self.element_clip(&session_id, &tab_id, &scope_ref)?)
        } else {
            None
        };
        let image_id = format!("img_{}_{}", now_millis(), self.screenshot_counter + 1);
        self.screenshot_counter += 1;
        let screenshot_dir = self
            .repo_root
            .join("output")
            .join("browser-mcp-screenshots");
        fs::create_dir_all(&screenshot_dir).map_err(|err| {
            browser_error(
                "SCREENSHOT_FAILED",
                &format!("create screenshot directory failed: {err}"),
                &["verify output directory permissions"],
                true,
            )
        })?;
        let path = screenshot_dir.join(format!("{image_id}.png"));
        let mut params = Map::new();
        params.insert("format".to_string(), Value::String("png".to_string()));
        params.insert("fromSurface".to_string(), Value::Bool(true));
        if full_page {
            params.insert("captureBeyondViewport".to_string(), Value::Bool(true));
        }
        if let Some(clip) = clip {
            params.insert("clip".to_string(), clip);
        }
        let response = {
            let session_cdp_id = self.tab_session_id(&session_id, &tab_id)?;
            let cdp = self.cdp_mut(&session_id)?;
            cdp.call(
                Some(&session_cdp_id),
                "Page.captureScreenshot",
                Value::Object(params),
            )?
        };
        let data = response
            .get("data")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                browser_error(
                    "SCREENSHOT_FAILED",
                    "Chrome did not return screenshot data.",
                    &["try browser_screenshot again"],
                    true,
                )
            })?;
        let bytes = decode_base64(data).map_err(|err| {
            browser_error(
                "SCREENSHOT_FAILED",
                &format!("decode screenshot failed: {err}"),
                &["try browser_screenshot again"],
                true,
            )
        })?;
        fs::write(&path, bytes).map_err(|err| {
            browser_error(
                "SCREENSHOT_FAILED",
                &format!("write screenshot failed: {err}"),
                &["verify output directory permissions"],
                true,
            )
        })?;
        let meta = json!({"imageId": image_id, "path": path.to_string_lossy()});
        Ok(json!({
            "structuredContent": meta,
            "content": [
                {"type": "image", "data": data, "mimeType": "image/png"},
                {"type": "text", "text": serde_json::to_string_pretty(&meta).unwrap_or_else(|_| "{}".to_string())}
            ],
            "isError": false,
        }))
    }

    fn click(&mut self, input: &Value) -> Result<Value, Value> {
        let ref_id = required_string_arg(input, "ref")?;
        let timeout_ms = optional_u64(input, "timeoutMs")?.unwrap_or(DEFAULT_WAIT_MS);
        let (session_id, tab_id) = self.resolve_tab_ids(input)?;
        let before = self.refresh_snapshot(&session_id, &tab_id)?;
        let selector = self.selector_for_ref(&session_id, &tab_id, &ref_id)?;
        self.runtime_call(
            &session_id,
            &tab_id,
            &format!(
                "async function(){{const el=document.querySelector({}); if(!el) throw new Error('element not found'); el.scrollIntoView({{block:'center',inline:'center'}}); el.click(); return true;}}",
                json_string_literal(&selector)
            ),
            timeout_ms,
        )?;
        self.wait_for_page_ready(&session_id, &tab_id, timeout_ms)?;
        let after = self.refresh_snapshot(&session_id, &tab_id)?;
        Ok(json!({
            "ok": true,
            "action": "click",
            "ref": ref_id,
            "tab": self.tab_view(&session_id, &tab_id)?,
            "delta": compute_delta(&before, &after),
        }))
    }

    fn fill(&mut self, input: &Value) -> Result<Value, Value> {
        let ref_id = required_string_arg(input, "ref")?;
        let value = required_string_arg(input, "value")?;
        let submit = optional_bool(input, "submit").unwrap_or(false);
        let (session_id, tab_id) = self.resolve_tab_ids(input)?;
        let before = self.refresh_snapshot(&session_id, &tab_id)?;
        let selector = self.selector_for_ref(&session_id, &tab_id, &ref_id)?;
        self.runtime_call(
            &session_id,
            &tab_id,
            &format!(
                "async function(){{const el=document.querySelector({}); if(!el) throw new Error('element not found'); el.scrollIntoView({{block:'center',inline:'center'}}); el.focus(); el.value={}; el.dispatchEvent(new Event('input',{{bubbles:true}})); el.dispatchEvent(new Event('change',{{bubbles:true}})); if({}){{const event=new KeyboardEvent('keydown',{{key:'Enter',bubbles:true}}); el.dispatchEvent(event); if(el.form) el.form.requestSubmit ? el.form.requestSubmit() : el.form.submit();}} return true;}}",
                json_string_literal(&selector),
                json_string_literal(&value),
                if submit { "true" } else { "false" }
            ),
            DEFAULT_WAIT_MS,
        )?;
        self.wait_for_page_ready(&session_id, &tab_id, DEFAULT_WAIT_MS)?;
        let after = self.refresh_snapshot(&session_id, &tab_id)?;
        Ok(json!({
            "ok": true,
            "action": "fill",
            "ref": ref_id,
            "tab": self.tab_view(&session_id, &tab_id)?,
            "delta": compute_delta(&before, &after),
        }))
    }

    fn press(&mut self, input: &Value) -> Result<Value, Value> {
        let key = required_string_arg(input, "key")?;
        let (session_id, tab_id) = self.resolve_tab_ids(input)?;
        let before = self.refresh_snapshot(&session_id, &tab_id)?;
        let cdp_key = cdp_key_name(&key);
        let session_cdp_id = self.tab_session_id(&session_id, &tab_id)?;
        let cdp = self.cdp_mut(&session_id)?;
        cdp.call(
            Some(&session_cdp_id),
            "Input.dispatchKeyEvent",
            json!({"type": "keyDown", "key": cdp_key}),
        )?;
        cdp.call(
            Some(&session_cdp_id),
            "Input.dispatchKeyEvent",
            json!({"type": "keyUp", "key": cdp_key}),
        )?;
        self.wait_for_page_ready(&session_id, &tab_id, DEFAULT_WAIT_MS)?;
        let after = self.refresh_snapshot(&session_id, &tab_id)?;
        Ok(json!({
            "ok": true,
            "action": "press",
            "tab": self.tab_view(&session_id, &tab_id)?,
            "delta": compute_delta(&before, &after),
        }))
    }

    fn wait_for(&mut self, input: &Value) -> Result<Value, Value> {
        let condition = input.get("condition").ok_or_else(|| {
            browser_error(
                "INVALID_INPUT",
                "condition is required.",
                &["provide condition.type"],
                true,
            )
        })?;
        let condition_type = required_string_arg(condition, "type")?;
        let condition_value = optional_string(condition, "value");
        let timeout_ms = optional_u64(input, "timeoutMs")?.unwrap_or(DEFAULT_WAIT_MS);
        let (session_id, tab_id) = self.resolve_tab_ids(input)?;
        match condition_type.as_str() {
            "text_appears" => {
                let value = condition_value.ok_or_else(|| {
                    browser_error(
                        "INVALID_INPUT",
                        "value is required for text_appears.",
                        &["provide condition.value"],
                        true,
                    )
                })?;
                self.wait_for_js_condition(
                    &session_id,
                    &tab_id,
                    &format!(
                        "document.body && document.body.innerText.includes({})",
                        json_string_literal(&value)
                    ),
                    timeout_ms,
                )?;
            }
            "text_disappears" => {
                let value = condition_value.ok_or_else(|| {
                    browser_error(
                        "INVALID_INPUT",
                        "value is required for text_disappears.",
                        &["provide condition.value"],
                        true,
                    )
                })?;
                self.wait_for_js_condition(
                    &session_id,
                    &tab_id,
                    &format!(
                        "!(document.body && document.body.innerText.includes({}))",
                        json_string_literal(&value)
                    ),
                    timeout_ms,
                )?;
            }
            "element_appears" => {
                let ref_id = condition_value.ok_or_else(|| {
                    browser_error(
                        "INVALID_INPUT",
                        "value is required for element_appears.",
                        &["provide element ref"],
                        true,
                    )
                })?;
                let selector = self.selector_for_ref(&session_id, &tab_id, &ref_id)?;
                self.wait_for_js_condition(
                    &session_id,
                    &tab_id,
                    &format!(
                        "!!document.querySelector({})",
                        json_string_literal(&selector)
                    ),
                    timeout_ms,
                )?;
            }
            "element_disappears" => {
                let ref_id = condition_value.ok_or_else(|| {
                    browser_error(
                        "INVALID_INPUT",
                        "value is required for element_disappears.",
                        &["provide element ref"],
                        true,
                    )
                })?;
                let selector = self.selector_for_ref(&session_id, &tab_id, &ref_id)?;
                self.wait_for_js_condition(
                    &session_id,
                    &tab_id,
                    &format!(
                        "!document.querySelector({})",
                        json_string_literal(&selector)
                    ),
                    timeout_ms,
                )?;
            }
            "url_contains" => {
                let value = condition_value.ok_or_else(|| {
                    browser_error(
                        "INVALID_INPUT",
                        "value is required for url_contains.",
                        &["provide condition.value"],
                        true,
                    )
                })?;
                self.wait_for_js_condition(
                    &session_id,
                    &tab_id,
                    &format!("location.href.includes({})", json_string_literal(&value)),
                    timeout_ms,
                )?;
            }
            "network_idle" => self.drain_cdp_events(&session_id, timeout_ms)?,
            _ => {
                return Err(browser_error(
                    "UNSUPPORTED_OPERATION",
                    &format!("Unsupported wait condition: {condition_type}."),
                    &["use a supported condition type"],
                    true,
                ))
            }
        }
        self.refresh_snapshot(&session_id, &tab_id)?;
        Ok(json!({"ok": true, "tab": self.tab_view(&session_id, &tab_id)?, "condition": condition}))
    }

    fn save_session(&mut self, input: &Value) -> Result<Value, Value> {
        let session_id = self.required_session_id()?;
        let default_path = self
            .repo_root
            .join("output")
            .join("browser-mcp-sessions")
            .join(format!("{session_id}.json"));
        let session_path = optional_string(input, "sessionPath")
            .map(PathBuf::from)
            .unwrap_or(default_path);
        if let Some(parent) = session_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                browser_error(
                    "SESSION_SAVE_FAILED",
                    &format!("create session directory failed: {err}"),
                    &["verify output directory permissions"],
                    true,
                )
            })?;
        }
        let cdp = self.cdp_mut(&session_id)?;
        let cookies = cdp.call(None, "Storage.getCookies", json!({}))?;
        fs::write(
            &session_path,
            serde_json::to_string_pretty(&json!({
                "schemaVersion": "browser-mcp-rust-session-v1",
                "savedAt": current_local_timestamp(),
                "cookies": cookies.get("cookies").cloned().unwrap_or_else(|| json!([])),
            }))
            .unwrap_or_else(|_| "{}".to_string()),
        )
        .map_err(|err| {
            browser_error(
                "SESSION_SAVE_FAILED",
                &format!("write session failed: {err}"),
                &["verify output directory permissions"],
                true,
            )
        })?;
        Ok(
            json!({"ok": true, "path": session_path.to_string_lossy(), "savedAt": current_local_timestamp()}),
        )
    }

    fn restore_session(&mut self, input: &Value) -> Result<Value, Value> {
        let session_path = PathBuf::from(required_string_arg(input, "sessionPath")?);
        let raw = fs::read_to_string(&session_path).map_err(|err| {
            browser_error(
                "INVALID_INPUT",
                &format!(
                    "Session snapshot not found: {} ({err})",
                    session_path.display()
                ),
                &["call browser_save_session first", "verify the path"],
                true,
            )
        })?;
        let payload: Value = serde_json::from_str(&raw).map_err(|err| {
            browser_error(
                "INVALID_INPUT",
                &format!("Session snapshot is invalid JSON: {err}"),
                &["call browser_save_session again"],
                true,
            )
        })?;
        let session_ids = self.sessions.keys().cloned().collect::<Vec<_>>();
        for session_id in session_ids {
            let _ = self.dispose_session(&session_id);
        }
        let session_id = self.get_or_create_session()?;
        if let Some(cookies) = payload.get("cookies").and_then(Value::as_array) {
            let cdp = self.cdp_mut(&session_id)?;
            cdp.call(None, "Storage.setCookies", json!({"cookies": cookies}))?;
        }
        Ok(
            json!({"ok": true, "restoredFrom": session_path.to_string_lossy(), "sessionId": session_id}),
        )
    }

    fn get_attached_runtime_events(&mut self, input: &Value) -> Result<Value, Value> {
        let limit = optional_usize(input, "limit", 100)?;
        if limit == 0 {
            return Err(browser_error(
                "INVALID_INPUT",
                "limit must be a positive integer.",
                &["provide a positive integer limit"],
                true,
            ));
        }
        let resolved = self.resolve_attached_runtime_descriptor_context()?;
        let after_event_id = optional_string(input, "afterEventId");
        let replay = replay_trace_stream(TraceStreamReplayRequestPayload {
            path: Some(resolved.trace_stream_path.clone()),
            event_stream_text: None,
            compaction_manifest_path: None,
            compaction_manifest_text: None,
            compaction_state_text: None,
            compaction_artifact_index_text: None,
            compaction_delta_text: None,
            session_id: None,
            job_id: None,
            stream_scope_fields: None,
            after_event_id: after_event_id.clone(),
            limit: Some(limit),
        })
        .map_err(|err| {
            if err.contains("Unknown event id for stream resume") {
                browser_error(
                    "ATTACHED_RUNTIME_CURSOR_NOT_FOUND",
                    &format!(
                        "No attached runtime event was found for afterEventId={}.",
                        after_event_id.clone().unwrap_or_default()
                    ),
                    &[
                        "call browser_get_attached_runtime_events without afterEventId",
                        "inspect browser_diagnostics",
                    ],
                    true,
                )
            } else {
                browser_error(
                    "ATTACHED_RUNTIME_TRACE_UNAVAILABLE",
                    &err,
                    &[
                        "inspect browser_diagnostics",
                        "refresh the attach descriptor or trace artifacts",
                    ],
                    true,
                )
            }
        })?;
        if replay.schema_version != ROUTER_RS_TRACE_STREAM_REPLAY_SCHEMA_VERSION
            || replay.authority != ROUTER_RS_TRACE_IO_AUTHORITY
        {
            return Err(browser_error(
                "ATTACHED_RUNTIME_TRACE_UNAVAILABLE",
                "router-rs trace replay returned an unexpected schema.",
                &[
                    "inspect browser_diagnostics",
                    "refresh the attach descriptor or trace artifacts",
                ],
                true,
            ));
        }
        let last_event = replay.events.last();
        let next_cursor = last_event.map(|event| {
            json!({
                "eventId": event.get("event_id").and_then(Value::as_str),
                "eventIndex": replay.next_cursor.as_ref().map(|cursor| cursor.event_index).unwrap_or_else(|| replay.window_start_index + replay.events.len().saturating_sub(1)),
            })
        });
        let mut attached_runtime = resolved.diagnostics_base.clone();
        attached_runtime["eventCount"] = json!(replay.event_count);
        attached_runtime["latestEventId"] = opt_string_value(replay.latest_event_id);
        attached_runtime["latestEventKind"] = opt_string_value(replay.latest_event_kind);
        attached_runtime["latestEventTimestamp"] = opt_string_value(replay.latest_event_timestamp);
        Ok(json!({
            "ok": true,
            "attachedRuntime": attached_runtime,
            "replayContext": attached_runtime_replay_context(&resolved.diagnostics_base),
            "events": replay.events,
            "afterEventId": after_event_id,
            "hasMore": replay.has_more,
            "nextCursor": next_cursor,
            "heartbeat": if optional_bool(input, "heartbeat").unwrap_or(false) && replay.events.is_empty() { json!({"status": "idle"}) } else { Value::Null },
        }))
    }

    fn diagnostics(&mut self, _input: &Value) -> Result<Value, Value> {
        let mut tabs = 0usize;
        let mut network_events = 0usize;
        for session in self.sessions.values() {
            tabs += session.tabs.len();
            for tab in session.tabs.values() {
                network_events += tab.network_events.len();
            }
        }
        let screenshot_count = fs::read_dir(
            self.repo_root
                .join("output")
                .join("browser-mcp-screenshots"),
        )
        .ok()
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| {
                    entry.path().extension().and_then(|value| value.to_str()) == Some("png")
                })
                .count()
        })
        .unwrap_or(0);
        Ok(json!({
            "sessions": self.sessions.len(),
            "tabs": tabs,
            "networkEventBufferSize": network_events,
            "screenshotCount": screenshot_count,
            "runtimeVersion": SERVER_VERSION,
            "attachedRuntime": self.attached_runtime_diagnostics(),
        }))
    }

    fn attached_runtime_diagnostics(&self) -> Value {
        let configured_source = self.configured_runtime_attach_source();
        let base = base_attached_runtime_diagnostics(&configured_source);
        if configured_source.source.is_none() {
            return base;
        }
        match self.resolve_attached_runtime_descriptor_context() {
            Ok(resolved) => match inspect_trace_stream(TraceStreamInspectRequestPayload {
                path: Some(resolved.trace_stream_path),
                event_stream_text: None,
                compaction_manifest_path: None,
                compaction_manifest_text: None,
                compaction_state_text: None,
                compaction_artifact_index_text: None,
                compaction_delta_text: None,
                session_id: None,
                job_id: None,
                stream_scope_fields: None,
            }) {
                Ok(summary) => {
                    if summary.schema_version != ROUTER_RS_TRACE_STREAM_INSPECT_SCHEMA_VERSION
                        || summary.authority != ROUTER_RS_TRACE_IO_AUTHORITY
                    {
                        let mut diagnostics = resolved.diagnostics_base;
                        diagnostics["status"] = Value::String("trace_unavailable".to_string());
                        diagnostics["warning"] = Value::String(
                            "router-rs trace inspect returned an unexpected schema.".to_string(),
                        );
                        return diagnostics;
                    }
                    let mut diagnostics = resolved.diagnostics_base;
                    diagnostics["eventCount"] = json!(summary.event_count);
                    diagnostics["latestEventId"] = opt_string_value(summary.latest_event_id);
                    diagnostics["latestEventKind"] = opt_string_value(summary.latest_event_kind);
                    diagnostics["latestEventTimestamp"] =
                        opt_string_value(summary.latest_event_timestamp);
                    diagnostics
                }
                Err(err) => {
                    let mut diagnostics = resolved.diagnostics_base;
                    diagnostics["status"] = Value::String("trace_unavailable".to_string());
                    diagnostics["warning"] = Value::String(err);
                    diagnostics
                }
            },
            Err(error) => self.attached_runtime_error_diagnostics(&configured_source, base, error),
        }
    }

    fn attached_runtime_error_diagnostics(
        &self,
        configured_source: &ConfiguredAttachSource,
        base: Value,
        error: Value,
    ) -> Value {
        let code = error.get("code").and_then(Value::as_str).unwrap_or("");
        let mut diagnostics = self
            .load_runtime_attach_descriptor()
            .ok()
            .map(|loaded| {
                self.project_attached_runtime_diagnostics(
                    configured_source,
                    &loaded.descriptor,
                    loaded.input_artifact_kind,
                    descriptor_resolved_artifact(&loaded.descriptor, "trace_stream_path"),
                )
            })
            .unwrap_or(base);
        diagnostics["status"] = Value::String(
            match code {
                "ATTACHED_RUNTIME_UNSUPPORTED_BACKEND" => "unsupported_backend",
                "ATTACHED_RUNTIME_TRACE_UNAVAILABLE" => "trace_unavailable",
                _ => "invalid_descriptor",
            }
            .to_string(),
        );
        diagnostics["warning"] = error.get("message").cloned().unwrap_or_else(|| {
            Value::String("failed to load runtime attach descriptor".to_string())
        });
        diagnostics
    }

    fn configured_runtime_attach_source(&self) -> ConfiguredAttachSource {
        if let Some(path) = self
            .attach_config
            .runtime_attach_descriptor_path
            .as_ref()
            .filter(|path| !path.trim().is_empty())
        {
            return ConfiguredAttachSource {
                source: Some("descriptor_path"),
                path: Some(path.clone()),
            };
        }
        if let Some(path) = self
            .attach_config
            .runtime_attach_artifact_path
            .as_ref()
            .filter(|path| !path.trim().is_empty())
        {
            return ConfiguredAttachSource {
                source: Some("attach_artifact_path"),
                path: Some(path.clone()),
            };
        }
        if let Some(path) = self.auto_discover_runtime_attach_artifact() {
            return ConfiguredAttachSource {
                source: Some("attach_artifact_path"),
                path: Some(path),
            };
        }
        ConfiguredAttachSource {
            source: None,
            path: None,
        }
    }

    fn resolve_attached_runtime_descriptor_context(
        &self,
    ) -> Result<ResolvedAttachedRuntimeDescriptorContext, Value> {
        let configured_source = self.configured_runtime_attach_source();
        if configured_source.source.is_none() {
            return Err(browser_error(
                "ATTACHED_RUNTIME_NOT_CONFIGURED",
                "No runtime attach descriptor is configured for browser-mcp.",
                &[
                    "start browser-mcp with --runtime-attach-descriptor-path",
                    "or --runtime-attach-artifact-path",
                    "or set BROWSER_MCP_RUNTIME_ATTACH_DESCRIPTOR_PATH",
                ],
                true,
            ));
        }

        let loaded = self.load_runtime_attach_descriptor().map_err(|err| {
            browser_error(
                "ATTACHED_RUNTIME_INVALID_DESCRIPTOR",
                &err,
                &[
                    "refresh the descriptor from describe_runtime_event_handoff",
                    "inspect browser_diagnostics",
                ],
                true,
            )
        })?;
        let descriptor = loaded.descriptor;
        let replay_supported =
            descriptor_bool(&descriptor, &["attach_capabilities", "artifact_replay"]) == Some(true);
        let trace_stream_path = descriptor_resolved_artifact(&descriptor, "trace_stream_path");
        let diagnostics_base = self.project_attached_runtime_diagnostics(
            &configured_source,
            &descriptor,
            loaded.input_artifact_kind,
            trace_stream_path.clone(),
        );

        if descriptor_string(&descriptor, &["schema_version"]).as_deref()
            != Some(RUNTIME_ATTACH_DESCRIPTOR_SCHEMA_VERSION)
            || descriptor_string(&descriptor, &["attach_mode"]).as_deref()
                != Some(RUNTIME_ATTACH_MODE)
            || !replay_supported
        {
            return Err(browser_error(
                "ATTACHED_RUNTIME_INVALID_DESCRIPTOR",
                "runtime attach descriptor must be artifact-replay capable and match the Rust-first schema.",
                &[
                    "refresh the descriptor from describe_runtime_event_handoff",
                    "inspect browser_diagnostics",
                ],
                true,
            ));
        }

        let backend_family = descriptor_string(&descriptor, &["artifact_backend_family"])
            .unwrap_or_else(|| "filesystem".to_string());
        if backend_family != "filesystem" && backend_family != "sqlite" {
            return Err(browser_error(
                "ATTACHED_RUNTIME_UNSUPPORTED_BACKEND",
                &format!(
                    "browser-mcp attach consumer currently supports filesystem/sqlite replay only (got {backend_family})"
                ),
                &[
                    "use a filesystem- or sqlite-backed attach descriptor for browser-mcp replay",
                    "inspect browser_diagnostics",
                ],
                true,
            ));
        }

        let Some(trace_stream_path) = trace_stream_path else {
            return Err(browser_error(
                "ATTACHED_RUNTIME_TRACE_UNAVAILABLE",
                "runtime attach descriptor must carry a canonical resolved_artifacts.trace_stream_path.",
                &["refresh the descriptor from describe_runtime_event_handoff"],
                true,
            ));
        };

        Ok(ResolvedAttachedRuntimeDescriptorContext {
            trace_stream_path,
            diagnostics_base,
        })
    }

    fn load_runtime_attach_descriptor(&self) -> Result<LoadedRuntimeAttachDescriptor, String> {
        let configured_source = self.configured_runtime_attach_source();
        match configured_source.source {
            Some("descriptor_path") => {
                self.read_runtime_attach_descriptor_file(configured_source.path.as_deref())
            }
            Some("attach_artifact_path") => self
                .build_runtime_attach_descriptor_from_artifact_path(
                    configured_source.path.as_deref(),
                ),
            _ => Err("runtime attach descriptor is not configured".to_string()),
        }
    }

    fn read_runtime_attach_descriptor_file(
        &self,
        descriptor_path: Option<&str>,
    ) -> Result<LoadedRuntimeAttachDescriptor, String> {
        let descriptor_path = descriptor_path
            .ok_or_else(|| "runtime attach descriptor path is missing".to_string())?;
        let raw = fs::read_to_string(descriptor_path)
            .map_err(|err| format!("read runtime attach descriptor failed: {err}"))?;
        let parsed = serde_json::from_str::<Value>(&raw)
            .map_err(|err| format!("parse runtime attach descriptor failed: {err}"))?;
        if !parsed.is_object() {
            return Err("runtime attach descriptor must decode to a JSON object".to_string());
        }
        self.canonicalize_attach_descriptor_if_possible(parsed)
    }

    fn build_runtime_attach_descriptor_from_artifact_path(
        &self,
        artifact_path: Option<&str>,
    ) -> Result<LoadedRuntimeAttachDescriptor, String> {
        let artifact_path =
            artifact_path.ok_or_else(|| "runtime attach artifact path is missing".to_string())?;
        let resolved_path = normalize_runtime_locator_for_existing_file(artifact_path);
        if let Ok(raw) = fs::read_to_string(&resolved_path) {
            let parsed = serde_json::from_str::<Value>(&raw)
                .map_err(|err| format!("parse runtime attach artifact failed: {err}"))?;
            if !parsed.is_object() {
                return Err("runtime attach artifact returned an unknown schema".to_string());
            }
            let schema = descriptor_string(&parsed, &["schema_version"]);
            if matches!(
                schema.as_deref(),
                Some(RUNTIME_EVENT_TRANSPORT_SCHEMA_VERSION)
                    | Some(RUNTIME_EVENT_HANDOFF_SCHEMA_VERSION)
                    | Some(TRACE_RESUME_MANIFEST_SCHEMA_VERSION)
            ) {
                if let Ok(loaded) =
                    self.try_hydrate_runtime_attach_descriptor_from_artifact_path(&resolved_path)
                {
                    return Ok(loaded);
                }
            }
            if schema.as_deref() == Some(RUNTIME_ATTACH_DESCRIPTOR_SCHEMA_VERSION) {
                return self.canonicalize_attach_descriptor_if_possible(parsed);
            }
            if let Ok(loaded) =
                self.try_hydrate_runtime_attach_descriptor_from_artifact_path(&resolved_path)
            {
                return Ok(loaded);
            }
            return Err("runtime attach artifact returned an unknown schema".to_string());
        }
        self.try_hydrate_runtime_attach_descriptor_from_artifact_path(artifact_path)
    }

    fn try_hydrate_runtime_attach_descriptor_from_artifact_path(
        &self,
        artifact_path: &str,
    ) -> Result<LoadedRuntimeAttachDescriptor, String> {
        self.hydrate_runtime_attach_descriptor_via_rust(None, Some(artifact_path), None, None)
            .or_else(|_| {
                self.hydrate_runtime_attach_descriptor_via_rust(
                    None,
                    None,
                    Some(artifact_path),
                    None,
                )
            })
            .or_else(|_| {
                self.hydrate_runtime_attach_descriptor_via_rust(
                    None,
                    None,
                    None,
                    Some(artifact_path),
                )
            })
    }

    fn canonicalize_attach_descriptor_if_possible(
        &self,
        descriptor: Value,
    ) -> Result<LoadedRuntimeAttachDescriptor, String> {
        match self.hydrate_runtime_attach_descriptor_via_rust(
            Some(descriptor.clone()),
            None,
            None,
            None,
        ) {
            Ok(hydrated) => {
                assert_attach_descriptor_matches_canonical(&descriptor, &hydrated.descriptor)?;
                assert_attach_descriptor_contract(&hydrated.descriptor)?;
                Ok(hydrated)
            }
            Err(err) => {
                if attach_descriptor_needs_rust_hydration(&descriptor) {
                    return Err(err);
                }
                assert_attach_descriptor_contract(&descriptor)?;
                Ok(LoadedRuntimeAttachDescriptor {
                    descriptor,
                    input_artifact_kind: Some("attach_descriptor"),
                })
            }
        }
    }

    fn hydrate_runtime_attach_descriptor_via_rust(
        &self,
        attach_descriptor: Option<Value>,
        binding_artifact_path: Option<&str>,
        handoff_path: Option<&str>,
        resume_manifest_path: Option<&str>,
    ) -> Result<LoadedRuntimeAttachDescriptor, String> {
        let attached = attach_runtime_event_transport(json!({
            "attach_descriptor": attach_descriptor,
            "binding_artifact_path": binding_artifact_path,
            "handoff_path": handoff_path,
            "resume_manifest_path": resume_manifest_path,
        }))?;
        let descriptor = attached
            .get("attach_descriptor")
            .cloned()
            .filter(Value::is_object)
            .ok_or_else(|| {
                "runtime attach transport payload is missing attach_descriptor".to_string()
            })?;
        let input_artifact_kind = if attach_descriptor.is_some() {
            Some("attach_descriptor")
        } else if binding_artifact_path.is_some() {
            Some("binding_artifact")
        } else if handoff_path.is_some() {
            Some("handoff")
        } else if resume_manifest_path.is_some() {
            Some("resume_manifest")
        } else {
            None
        };
        Ok(LoadedRuntimeAttachDescriptor {
            descriptor,
            input_artifact_kind,
        })
    }

    fn project_attached_runtime_diagnostics(
        &self,
        configured_source: &ConfiguredAttachSource,
        descriptor: &Value,
        input_artifact_kind: Option<&str>,
        trace_stream_path: Option<String>,
    ) -> Value {
        json!({
            "status": "ready",
            "descriptorSource": configured_source.source,
            "descriptorPath": configured_source.path,
            "inputArtifactKind": input_artifact_kind,
            "schemaVersion": descriptor_string(descriptor, &["schema_version"]),
            "attachMode": descriptor_string(descriptor, &["attach_mode"]),
            "artifactBackendFamily": descriptor_string(descriptor, &["artifact_backend_family"]),
            "recommendedEntrypoint": descriptor_string(descriptor, &["recommended_entrypoint"]),
            "sourceTransportMethod": descriptor_string(descriptor, &["source_transport_method"]),
            "sourceHandoffMethod": descriptor_string(descriptor, &["source_handoff_method"]),
            "traceStreamPath": trace_stream_path,
            "bindingArtifactSource": descriptor_string(descriptor, &["resolution", "binding_artifact_path"]),
            "handoffSource": descriptor_string(descriptor, &["resolution", "handoff_path"]),
            "resumeManifestSource": descriptor_string(descriptor, &["resolution", "resume_manifest_path"]),
            "traceStreamSource": descriptor_string(descriptor, &["resolution", "trace_stream_path"]),
            "replaySupported": descriptor_bool(descriptor, &["attach_capabilities", "artifact_replay"]).unwrap_or(false),
            "eventCount": 0,
            "latestEventId": null,
            "latestEventKind": null,
            "latestEventTimestamp": null,
            "warning": null,
        })
    }

    fn auto_discover_runtime_attach_artifact(&self) -> Option<String> {
        resolve_browser_mcp_attach_artifact(&self.repo_root, None)
    }

    fn get_or_create_session(&mut self) -> Result<String, Value> {
        if let Some(session_id) = self.sessions.keys().next().cloned() {
            return Ok(session_id);
        }
        let chrome_path = find_chrome_binary()?;
        let port = allocate_debug_port();
        let session_id = format!("sess_{:03}", self.session_counter + 1);
        self.session_counter += 1;
        let user_data_dir = std::env::temp_dir().join(format!(
            "browser-mcp-rust-{}-{}",
            std::process::id(),
            now_millis()
        ));
        fs::create_dir_all(&user_data_dir).map_err(|err| {
            browser_error(
                "BROWSER_LAUNCH_FAILED",
                &format!("create user data dir failed: {err}"),
                &["verify temp directory permissions"],
                false,
            )
        })?;
        let mut command = Command::new(&chrome_path);
        command
            .arg(format!("--remote-debugging-port={port}"))
            .arg(format!("--user-data-dir={}", user_data_dir.display()));
        if self.attach_config.headless {
            command.arg("--headless=new");
        }
        let child = command
            .arg("--disable-gpu")
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("about:blank")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| {
                browser_error(
                    "BROWSER_LAUNCH_FAILED",
                    &format!("launch Chrome failed: {err}"),
                    &["install Google Chrome or set BROWSER_MCP_CHROME_PATH"],
                    false,
                )
            })?;
        let browser_pid = child.id();
        wait_for_cdp(port)?;
        self.browser_processes.insert(session_id.clone(), child);
        self.sessions.insert(
            session_id.clone(),
            SessionRecord {
                id: session_id.clone(),
                created_at: current_local_timestamp(),
                viewport: ViewportSize {
                    width: 1440,
                    height: 900,
                },
                current_tab_id: None,
                tabs: HashMap::new(),
                _browser_pid: browser_pid,
                user_data_dir,
                cdp: CdpClient::connect(port)?,
            },
        );
        Ok(session_id)
    }

    fn create_tab(&mut self, session_id: &str) -> Result<String, Value> {
        let target = self.cdp_mut(session_id)?.call(
            None,
            "Target.createTarget",
            json!({"url": "about:blank"}),
        )?;
        let target_id = target
            .get("targetId")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                browser_error(
                    "BROWSER_TARGET_FAILED",
                    "Chrome did not return a targetId.",
                    &["try browser_open again"],
                    true,
                )
            })?
            .to_string();
        let attached = self.cdp_mut(session_id)?.call(
            None,
            "Target.attachToTarget",
            json!({"targetId": target_id, "flatten": true}),
        )?;
        let session_cdp_id = attached
            .get("sessionId")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                browser_error(
                    "BROWSER_TARGET_FAILED",
                    "Chrome did not return a CDP sessionId.",
                    &["try browser_open again"],
                    true,
                )
            })?
            .to_string();
        let tab_id = format!("tab_{:02}", self.tab_counter + 1);
        self.tab_counter += 1;
        {
            let cdp = self.cdp_mut(session_id)?;
            cdp.call(Some(&session_cdp_id), "Page.enable", json!({}))?;
            cdp.call(Some(&session_cdp_id), "Runtime.enable", json!({}))?;
            cdp.call(Some(&session_cdp_id), "Network.enable", json!({}))?;
            cdp.call(
                Some(&session_cdp_id),
                "Emulation.setDeviceMetricsOverride",
                json!({"width": 1440, "height": 900, "deviceScaleFactor": 1, "mobile": false}),
            )?;
        }
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.tabs.insert(
                tab_id.clone(),
                TabRecord {
                    id: tab_id.clone(),
                    target_id,
                    session_id: session_cdp_id,
                    url: "about:blank".to_string(),
                    title: "Untitled".to_string(),
                    page_revision: 0,
                    loading_state: "loading".to_string(),
                    indexed_elements: HashMap::new(),
                    fingerprint_to_ref: HashMap::new(),
                    last_snapshot: None,
                    snapshot_history: VecDeque::new(),
                    network_events: Vec::new(),
                },
            );
            session.current_tab_id = Some(tab_id.clone());
        }
        Ok(tab_id)
    }

    fn dispose_session(&mut self, session_id: &str) -> Result<(), Value> {
        if let Some(mut child) = self.browser_processes.remove(session_id) {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(session) = self.sessions.remove(session_id) {
            let _ = fs::remove_dir_all(session.user_data_dir);
        }
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), Value> {
        let ids = self.sessions.keys().cloned().collect::<Vec<_>>();
        for session_id in ids {
            self.dispose_session(&session_id)?;
        }
        Ok(())
    }

    fn cdp_mut(&mut self, session_id: &str) -> Result<&mut CdpClient, Value> {
        self.sessions
            .get_mut(session_id)
            .map(|session| &mut session.cdp)
            .ok_or_else(session_not_found_error)
    }

    fn required_session_id(&self) -> Result<String, Value> {
        self.sessions
            .keys()
            .next()
            .cloned()
            .ok_or_else(session_not_found_error)
    }

    fn resolve_tab_ids(&self, input: &Value) -> Result<(String, String), Value> {
        let session_id = self.required_session_id()?;
        let tab_id = optional_string(input, "tabId")
            .or_else(|| {
                self.sessions
                    .get(&session_id)
                    .and_then(|session| session.current_tab_id.clone())
            })
            .ok_or_else(|| {
                browser_error(
                    "TAB_NOT_FOUND",
                    "No active tab exists.",
                    &["call browser_open"],
                    true,
                )
            })?;
        if !self
            .sessions
            .get(&session_id)
            .is_some_and(|session| session.tabs.contains_key(&tab_id))
        {
            return Err(browser_error(
                "TAB_NOT_FOUND",
                &format!("Tab {tab_id} was not found."),
                &["call browser_tabs with action=list"],
                true,
            ));
        }
        Ok((session_id, tab_id))
    }

    fn tab_session_id(&self, session_id: &str, tab_id: &str) -> Result<String, Value> {
        self.sessions
            .get(session_id)
            .and_then(|session| session.tabs.get(tab_id))
            .map(|tab| tab.session_id.clone())
            .ok_or_else(|| {
                browser_error(
                    "TAB_NOT_FOUND",
                    &format!("Tab {tab_id} was not found."),
                    &["call browser_tabs with action=list"],
                    true,
                )
            })
    }

    fn session_view(&self, session_id: &str) -> Result<Value, Value> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or_else(session_not_found_error)?;
        Ok(json!({
            "sessionId": session.id,
            "createdAt": session.created_at,
            "viewport": {"width": session.viewport.width, "height": session.viewport.height},
            "currentTabId": session.current_tab_id,
        }))
    }

    fn tab_view(&self, session_id: &str, tab_id: &str) -> Result<Value, Value> {
        let tab = self
            .sessions
            .get(session_id)
            .and_then(|session| session.tabs.get(tab_id))
            .ok_or_else(|| {
                browser_error(
                    "TAB_NOT_FOUND",
                    &format!("Tab {tab_id} was not found."),
                    &["call browser_tabs with action=list"],
                    true,
                )
            })?;
        Ok(json!({
            "tabId": tab.id,
            "url": tab.url,
            "title": tab.title,
            "pageRevision": tab.page_revision,
            "loadingState": tab.loading_state,
        }))
    }

    fn wait_for_page_ready(
        &mut self,
        session_id: &str,
        tab_id: &str,
        timeout_ms: u64,
    ) -> Result<(), Value> {
        let deadline = SystemTime::now() + Duration::from_millis(timeout_ms);
        while SystemTime::now() < deadline {
            self.drain_cdp_events(session_id, 100)?;
            let state = self
                .evaluate_string(session_id, tab_id, "document.readyState")
                .unwrap_or_else(|_| "complete".to_string());
            if state == "complete" || state == "interactive" {
                self.drain_cdp_events(session_id, 250)?;
                return Ok(());
            }
        }
        Ok(())
    }

    fn refresh_snapshot(&mut self, session_id: &str, tab_id: &str) -> Result<PageSnapshot, Value> {
        self.drain_cdp_events(session_id, 250)?;
        let previous_ref_map = self
            .sessions
            .get(session_id)
            .and_then(|session| session.tabs.get(tab_id))
            .map(|tab| tab.fingerprint_to_ref.clone())
            .unwrap_or_default();
        let snapshot = self.capture_snapshot(session_id, tab_id, &previous_ref_map)?;
        let mut effective = snapshot.clone();
        if let Some(session) = self.sessions.get_mut(session_id) {
            let tab = session.tabs.get_mut(tab_id).ok_or_else(|| {
                browser_error(
                    "TAB_NOT_FOUND",
                    &format!("Tab {tab_id} was not found."),
                    &["call browser_tabs with action=list"],
                    true,
                )
            })?;
            let changed = tab
                .last_snapshot
                .as_ref()
                .map(|previous| has_meaningful_change(previous, &snapshot))
                .unwrap_or(true);
            if changed {
                tab.page_revision += 1;
                effective.revision = tab.page_revision;
                for element in &mut effective.interactive_elements {
                    element.page_revision = tab.page_revision;
                }
                tab.last_snapshot = Some(effective.clone());
                tab.snapshot_history.push_back(effective.clone());
                while tab.snapshot_history.len() > SNAPSHOT_HISTORY_LIMIT {
                    tab.snapshot_history.pop_front();
                }
            } else if let Some(last) = tab.last_snapshot.clone() {
                effective = last;
            }
            tab.url = effective.url.clone();
            tab.title = effective.title.clone();
            tab.loading_state = effective.loading_state.clone();
            tab.indexed_elements = effective
                .interactive_elements
                .iter()
                .map(|element| (element.ref_id.clone(), element.clone()))
                .collect();
            tab.fingerprint_to_ref = effective
                .interactive_elements
                .iter()
                .map(|element| (element.fingerprint.clone(), element.ref_id.clone()))
                .collect();
        }
        Ok(effective)
    }

    fn capture_snapshot(
        &mut self,
        session_id: &str,
        tab_id: &str,
        previous_ref_map: &HashMap<String, String>,
    ) -> Result<PageSnapshot, Value> {
        let loading_state = self.detect_loading_state(session_id, tab_id)?;
        let title = self.evaluate_string(session_id, tab_id, "document.title")?;
        let url = self.evaluate_string(session_id, tab_id, "location.href")?;
        let summary = self.evaluate_json(session_id, tab_id, summary_expression())?;
        let text_content = truncate_text(
            &self.evaluate_string(
                session_id,
                tab_id,
                "document.body ? (document.body.innerText || '').replace(/\\s+$/g, '').trim() : ''",
            )?,
            DEFAULT_TEXT_BUDGET,
        );
        let descriptors = self.collect_element_descriptors(session_id, tab_id)?;
        let interactive_elements = self.build_interactive_elements(descriptors, previous_ref_map);
        Ok(PageSnapshot {
            revision: 0,
            url,
            title,
            loading_state,
            summary,
            interactive_elements,
            text_lines: to_text_lines(&text_content),
            text_content,
            _created_at: now_millis(),
        })
    }

    fn detect_loading_state(&mut self, session_id: &str, tab_id: &str) -> Result<String, Value> {
        match self
            .evaluate_string(session_id, tab_id, "document.readyState")?
            .as_str()
        {
            "loading" => Ok("loading".to_string()),
            "interactive" => Ok("domcontentloaded".to_string()),
            _ => Ok("idle".to_string()),
        }
    }

    fn collect_element_descriptors(
        &mut self,
        session_id: &str,
        tab_id: &str,
    ) -> Result<Vec<ElementDescriptor>, Value> {
        let payload = self.evaluate_json(session_id, tab_id, element_collection_expression())?;
        let items = payload.as_array().cloned().unwrap_or_default();
        let mut descriptors = Vec::new();
        for item in items {
            descriptors.push(ElementDescriptor {
                role: value_str(item.get("role")).to_string(),
                name: value_str(item.get("name")).to_string(),
                text: value_str(item.get("text")).to_string(),
                visible: item
                    .get("visible")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                enabled: item.get("enabled").and_then(Value::as_bool).unwrap_or(true),
                tag: value_str(item.get("tag")).to_string(),
                test_id: item
                    .get("testId")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                _ordinal: item.get("ordinal").and_then(Value::as_u64).unwrap_or(0) as usize,
                selector: value_str(item.get("selector")).to_string(),
            });
        }
        Ok(descriptors)
    }

    fn build_interactive_elements(
        &mut self,
        descriptors: Vec<ElementDescriptor>,
        previous_ref_map: &HashMap<String, String>,
    ) -> Vec<InteractiveElement> {
        let mut fingerprint_counts: HashMap<String, usize> = HashMap::new();
        descriptors
            .into_iter()
            .take(DEFAULT_MAX_ELEMENTS * 3)
            .map(|descriptor| {
                let fingerprint = create_fingerprint(&descriptor, &mut fingerprint_counts);
                let ref_id = previous_ref_map
                    .get(&fingerprint)
                    .cloned()
                    .unwrap_or_else(|| {
                        self.ref_counter += 1;
                        format!("el_{}", self.ref_counter)
                    });
                InteractiveElement {
                    ref_id,
                    page_revision: 0,
                    role: descriptor.role,
                    name: descriptor.name,
                    text: descriptor.text,
                    visible: descriptor.visible,
                    enabled: descriptor.enabled,
                    tag: descriptor.tag,
                    test_id: descriptor.test_id,
                    fingerprint,
                    selector: descriptor.selector,
                }
            })
            .collect()
    }

    fn selector_for_ref(
        &self,
        session_id: &str,
        tab_id: &str,
        ref_id: &str,
    ) -> Result<String, Value> {
        let tab = self
            .sessions
            .get(session_id)
            .and_then(|session| session.tabs.get(tab_id))
            .ok_or_else(|| {
                browser_error(
                    "TAB_NOT_FOUND",
                    &format!("Tab {tab_id} was not found."),
                    &["call browser_tabs with action=list"],
                    true,
                )
            })?;
        let element = tab.indexed_elements.get(ref_id).ok_or_else(|| {
            browser_error(
                "STALE_ELEMENT_REF",
                &format!("Element ref {ref_id} is stale or unknown."),
                &["call browser_get_state", "call browser_get_elements"],
                true,
            )
        })?;
        if element.page_revision != tab.page_revision {
            return Err(browser_error(
                "STALE_ELEMENT_REF",
                &format!(
                    "Ref {ref_id} belongs to revision {}; current is {}.",
                    element.page_revision, tab.page_revision
                ),
                &["call browser_get_state", "call browser_get_elements"],
                true,
            ));
        }
        Ok(element.selector.clone())
    }

    fn element_clip(
        &mut self,
        session_id: &str,
        tab_id: &str,
        ref_id: &str,
    ) -> Result<Value, Value> {
        let selector = self.selector_for_ref(session_id, tab_id, ref_id)?;
        let payload = self.evaluate_json(
            session_id,
            tab_id,
            &format!(
                "(function(){{const el=document.querySelector({}); if(!el) return null; const r=el.getBoundingClientRect(); return {{x:Math.max(0,r.x), y:Math.max(0,r.y), width:Math.max(1,r.width), height:Math.max(1,r.height), scale:1}};}})()",
                json_string_literal(&selector)
            ),
        )?;
        if payload.is_null() {
            return Err(browser_error(
                "ELEMENT_NOT_VISIBLE",
                &format!("Unable to resolve locator for {ref_id}."),
                &["call browser_get_state", "use a fresher ref"],
                true,
            ));
        }
        Ok(payload)
    }

    fn evaluate_string(
        &mut self,
        session_id: &str,
        tab_id: &str,
        expression: &str,
    ) -> Result<String, Value> {
        let value = self.evaluate_json(session_id, tab_id, expression)?;
        Ok(value_string(Some(&value)))
    }

    fn evaluate_json(
        &mut self,
        session_id: &str,
        tab_id: &str,
        expression: &str,
    ) -> Result<Value, Value> {
        let session_cdp_id = self.tab_session_id(session_id, tab_id)?;
        let cdp = self.cdp_mut(session_id)?;
        let response = cdp.call(
            Some(&session_cdp_id),
            "Runtime.evaluate",
            json!({"expression": expression, "returnByValue": true, "awaitPromise": true}),
        )?;
        if let Some(details) = response.get("exceptionDetails") {
            return Err(browser_error(
                "EVALUATION_FAILED",
                &format!("page evaluation failed: {details}"),
                &["retry after the page settles"],
                true,
            ));
        }
        Ok(response
            .get("result")
            .and_then(|result| result.get("value"))
            .cloned()
            .unwrap_or(Value::Null))
    }

    fn runtime_call(
        &mut self,
        session_id: &str,
        tab_id: &str,
        declaration: &str,
        _timeout_ms: u64,
    ) -> Result<Value, Value> {
        let session_cdp_id = self.tab_session_id(session_id, tab_id)?;
        let cdp = self.cdp_mut(session_id)?;
        let response = cdp.call(
            Some(&session_cdp_id),
            "Runtime.evaluate",
            json!({"expression": format!("({declaration})()"), "awaitPromise": true, "returnByValue": true}),
        )?;
        if response.get("exceptionDetails").is_some() {
            return Err(browser_error(
                "ACTION_FAILED",
                "browser action failed in page context.",
                &["call browser_get_state", "use a fresher ref"],
                true,
            ));
        }
        Ok(response
            .get("result")
            .and_then(|result| result.get("value"))
            .cloned()
            .unwrap_or(Value::Null))
    }

    fn wait_for_js_condition(
        &mut self,
        session_id: &str,
        tab_id: &str,
        expression: &str,
        timeout_ms: u64,
    ) -> Result<(), Value> {
        let deadline = SystemTime::now() + Duration::from_millis(timeout_ms);
        while SystemTime::now() < deadline {
            if self
                .evaluate_json(session_id, tab_id, expression)
                .ok()
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
            {
                return Ok(());
            }
            self.drain_cdp_events(session_id, 100)?;
        }
        Err(browser_error(
            "WAIT_TIMEOUT",
            "Timed out waiting for browser condition.",
            &["inspect browser_get_state", "increase timeoutMs"],
            true,
        ))
    }

    fn drain_cdp_events(&mut self, session_id: &str, timeout_ms: u64) -> Result<(), Value> {
        let events = {
            let cdp = self.cdp_mut(session_id)?;
            cdp.drain_events(Duration::from_millis(timeout_ms))?
        };
        for event in events {
            self.handle_cdp_event(session_id, event);
        }
        Ok(())
    }

    fn handle_cdp_event(&mut self, session_id: &str, event: Value) {
        let method = event.get("method").and_then(Value::as_str).unwrap_or("");
        let cdp_session_id = event.get("sessionId").and_then(Value::as_str).unwrap_or("");
        let params = event.get("params").cloned().unwrap_or_else(|| json!({}));
        let Some(tab_id) = self.tab_id_by_cdp_session(session_id, cdp_session_id) else {
            return;
        };
        if method == "Network.responseReceived" {
            let response = params.get("response").cloned().unwrap_or_else(|| json!({}));
            let request = params.get("request").cloned().unwrap_or_else(|| json!({}));
            let event = NetworkEvent {
                id: format!("req_{}", self.request_counter + 1),
                method: value_str(request.get("method")).to_string(),
                url: value_str(response.get("url")).to_string(),
                status: response.get("status").and_then(Value::as_i64),
                content_type: response
                    .get("mimeType")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                resource_type: value_str(params.get("type")).to_string(),
                timestamp: now_millis(),
                ok: response
                    .get("status")
                    .and_then(Value::as_i64)
                    .map(|status| (200..400).contains(&status))
                    .unwrap_or(false),
                error_text: None,
                duration_ms: None,
            };
            self.request_counter += 1;
            self.push_network_event(session_id, &tab_id, event);
        } else if method == "Network.loadingFailed" {
            let event = NetworkEvent {
                id: format!("req_{}", self.request_counter + 1),
                method: String::new(),
                url: String::new(),
                status: None,
                content_type: None,
                resource_type: value_str(params.get("type")).to_string(),
                timestamp: now_millis(),
                ok: false,
                error_text: params
                    .get("errorText")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                duration_ms: None,
            };
            self.request_counter += 1;
            self.push_network_event(session_id, &tab_id, event);
        }
    }

    fn tab_id_by_cdp_session(&self, session_id: &str, cdp_session_id: &str) -> Option<String> {
        self.sessions.get(session_id).and_then(|session| {
            session
                .tabs
                .iter()
                .find(|(_, tab)| tab.session_id == cdp_session_id)
                .map(|(tab_id, _)| tab_id.clone())
        })
    }

    fn push_network_event(&mut self, session_id: &str, tab_id: &str, event: NetworkEvent) {
        if let Some(tab) = self
            .sessions
            .get_mut(session_id)
            .and_then(|session| session.tabs.get_mut(tab_id))
        {
            tab.network_events.push(event);
            if tab.network_events.len() > MAX_NETWORK_EVENTS {
                let remove = tab.network_events.len() - MAX_NETWORK_EVENTS;
                tab.network_events.drain(0..remove);
            }
        }
    }
}

impl CdpClient {
    fn connect(port: u16) -> Result<Self, Value> {
        let websocket_url = cdp_version_json(port)?
            .get("webSocketDebuggerUrl")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| {
                browser_error(
                    "CDP_CONNECT_FAILED",
                    "Chrome did not expose a browser websocket URL.",
                    &["retry browser_open"],
                    true,
                )
            })?;
        let (socket, _) = connect(websocket_url.as_str()).map_err(|err| {
            browser_error(
                "CDP_CONNECT_FAILED",
                &format!("connect Chrome CDP websocket failed: {err}"),
                &["retry browser_open"],
                true,
            )
        })?;
        Ok(Self {
            _port: port,
            next_id: 0,
            socket,
        })
    }

    fn call(
        &mut self,
        session_id: Option<&str>,
        method: &str,
        params: Value,
    ) -> Result<Value, Value> {
        self.next_id += 1;
        let id = self.next_id;
        let mut message = Map::new();
        message.insert("id".to_string(), json!(id));
        message.insert("method".to_string(), Value::String(method.to_string()));
        message.insert("params".to_string(), params);
        if let Some(session_id) = session_id {
            message.insert(
                "sessionId".to_string(),
                Value::String(session_id.to_string()),
            );
        }
        self.socket
            .send(Message::Text(Value::Object(message).to_string()))
            .map_err(|err| {
                browser_error(
                    "CDP_CALL_FAILED",
                    &format!("{method} send failed: {err}"),
                    &["retry after refreshing browser state"],
                    true,
                )
            })?;
        self.set_read_timeout(CDP_RECV_TIMEOUT)?;
        loop {
            let event = self.read_message()?;
            if event.get("id").and_then(Value::as_u64) == Some(id) {
                if let Some(error) = event.get("error") {
                    return Err(browser_error(
                        "CDP_CALL_FAILED",
                        &format!("{method} failed: {error}"),
                        &["retry after refreshing browser state"],
                        true,
                    ));
                }
                return Ok(event.get("result").cloned().unwrap_or_else(|| json!({})));
            }
        }
    }

    fn drain_events(&mut self, timeout: Duration) -> Result<Vec<Value>, Value> {
        self.set_read_timeout(timeout)?;
        let mut events = Vec::new();
        loop {
            match self.socket.read() {
                Ok(Message::Text(text)) => {
                    if let Ok(value) = serde_json::from_str::<Value>(&text) {
                        events.push(value);
                    }
                }
                Ok(Message::Binary(_)) | Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {}
                Ok(Message::Close(_)) => break,
                Ok(Message::Frame(_)) => {}
                Err(tungstenite::Error::Io(err))
                    if err.kind() == io::ErrorKind::WouldBlock
                        || err.kind() == io::ErrorKind::TimedOut =>
                {
                    break;
                }
                Err(err) => {
                    return Err(browser_error(
                        "CDP_CALL_FAILED",
                        &format!("read CDP event failed: {err}"),
                        &["retry after refreshing browser state"],
                        true,
                    ))
                }
            }
        }
        Ok(events)
    }

    fn read_message(&mut self) -> Result<Value, Value> {
        loop {
            match self.socket.read() {
                Ok(Message::Text(text)) => {
                    return serde_json::from_str::<Value>(&text).map_err(|err| {
                        browser_error(
                            "CDP_CALL_FAILED",
                            &format!("parse CDP message failed: {err}"),
                            &["retry after refreshing browser state"],
                            true,
                        )
                    });
                }
                Ok(Message::Binary(_)) | Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {}
                Ok(Message::Close(_)) => {
                    return Err(browser_error(
                        "CDP_CALL_FAILED",
                        "Chrome CDP websocket closed.",
                        &["retry browser_open"],
                        true,
                    ))
                }
                Ok(Message::Frame(_)) => {}
                Err(err) => {
                    return Err(browser_error(
                        "CDP_CALL_FAILED",
                        &format!("read CDP response failed: {err}"),
                        &["retry after refreshing browser state"],
                        true,
                    ))
                }
            }
        }
    }

    fn set_read_timeout(&mut self, timeout: Duration) -> Result<(), Value> {
        match self.socket.get_mut() {
            MaybeTlsStream::Plain(stream) => {
                stream.set_read_timeout(Some(timeout)).map_err(|err| {
                    browser_error(
                        "CDP_CALL_FAILED",
                        &format!("set CDP timeout failed: {err}"),
                        &["retry browser_open"],
                        true,
                    )
                })
            }
            _ => Ok(()),
        }
    }
}

fn wait_for_cdp(port: u16) -> Result<(), Value> {
    let deadline = SystemTime::now() + Duration::from_secs(8);
    while SystemTime::now() < deadline {
        if cdp_version_json(port).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(browser_error(
        "BROWSER_LAUNCH_FAILED",
        "Chrome remote debugging endpoint did not become ready.",
        &["retry browser_open"],
        false,
    ))
}

fn cdp_version_json(port: u16) -> Result<Value, Value> {
    cdp_http_json(port, "/json/version")
}

fn cdp_http_json(port: u16, path: &str) -> Result<Value, Value> {
    reqwest::blocking::get(format!("http://127.0.0.1:{port}{path}"))
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.json::<Value>())
        .map_err(|err| {
            browser_error(
                "CDP_HTTP_FAILED",
                &format!("Chrome CDP HTTP request failed: {err}"),
                &["verify Chrome remote debugging is reachable"],
                true,
            )
        })
}

fn find_chrome_binary() -> Result<PathBuf, Value> {
    if let Ok(path) = std::env::var("BROWSER_MCP_CHROME_PATH") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
    }
    let candidates = [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
    ];
    candidates
        .iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
        .ok_or_else(|| {
            browser_error(
                "BROWSER_LAUNCH_FAILED",
                "No Chrome/Chromium binary was found.",
                &["install Google Chrome", "set BROWSER_MCP_CHROME_PATH"],
                false,
            )
        })
}

fn allocate_debug_port() -> u16 {
    49_000 + ((now_millis() % 10_000) as u16)
}

fn summary_expression() -> &'static str {
    r#"(function(){
const main = document.querySelector('main') || document.body;
const mainText = ((main && main.textContent) || '').replace(/\s+/g, ' ').trim();
const visibleText = ((document.body && document.body.innerText) || '').trim();
const seen = new Set();
const messages = [];
for (const raw of visibleText.split('\n')) {
  const line = raw.trim();
  if (line && !seen.has(line)) {
    seen.add(line);
    messages.push(line);
    if (messages.length >= 8) break;
  }
}
return {mainGoalArea: mainText.slice(0, 240), visibleMessages: messages.map(line => line.slice(0,160)), forms: document.querySelectorAll('form').length, dialogs: document.querySelectorAll('dialog,[role="dialog"],[aria-modal="true"]').length};
})()"#
}

fn element_collection_expression() -> &'static str {
    r#"(function(){
const selector = 'a,button,input,textarea,select,[role="button"],[role="link"],[contenteditable="true"],summary';
function roleFor(el){
  const role = el.getAttribute('role');
  if (role) return role;
  const tag = el.tagName.toLowerCase();
  if (tag === 'a') return 'link';
  if (tag === 'button' || el.type === 'button' || el.type === 'submit') return 'button';
  if (tag === 'input' || tag === 'textarea' || el.isContentEditable) return 'textbox';
  if (tag === 'select') return 'combobox';
  return tag;
}
function cssPath(el){
  if (el.dataset && el.dataset.testid) return `[data-testid="${CSS.escape(el.dataset.testid)}"]`;
  const parts = [];
  let node = el;
  while (node && node.nodeType === 1 && node !== document.body) {
    let part = node.tagName.toLowerCase();
    if (node.id) {
      part += `#${CSS.escape(node.id)}`;
      parts.unshift(part);
      break;
    }
    const parent = node.parentElement;
    if (!parent) break;
    const siblings = Array.from(parent.children).filter(child => child.tagName === node.tagName);
    if (siblings.length > 1) part += `:nth-of-type(${siblings.indexOf(node) + 1})`;
    parts.unshift(part);
    node = parent;
  }
  return parts.join(' > ');
}
return Array.from(document.querySelectorAll(selector)).map((el, index) => {
  const rect = el.getBoundingClientRect();
  const visible = !!(rect.width && rect.height) && getComputedStyle(el).visibility !== 'hidden' && getComputedStyle(el).display !== 'none';
  const label = el.getAttribute('aria-label') || el.getAttribute('placeholder') || el.innerText || el.value || el.textContent || '';
  return {role: roleFor(el), name: String(label).replace(/\s+/g,' ').trim().slice(0,120), text: String(el.innerText || el.textContent || '').replace(/\s+/g,' ').trim().slice(0,160), visible, enabled: !el.disabled, tag: el.tagName.toLowerCase(), testId: el.dataset ? el.dataset.testid || null : null, ordinal: index, selector: cssPath(el)};
}).filter(item => item.visible);
})()"#
}

fn create_fingerprint(
    descriptor: &ElementDescriptor,
    counts: &mut HashMap<String, usize>,
) -> String {
    if let Some(test_id) = descriptor.test_id.as_ref() {
        return format!("tid::{test_id}");
    }
    let base = format!(
        "{}::{}::{}",
        descriptor.role, descriptor.name, descriptor.tag
    );
    let count = counts.entry(base.clone()).or_insert(0);
    *count += 1;
    if *count == 1 {
        base
    } else {
        format!("{base}#{}", *count)
    }
}

fn has_meaningful_change(previous: &PageSnapshot, next: &PageSnapshot) -> bool {
    if previous.url != next.url || previous.title != next.title {
        return true;
    }
    if previous.text_content != next.text_content {
        return true;
    }
    let previous_fingerprints = previous
        .interactive_elements
        .iter()
        .map(|element| element.fingerprint.as_str())
        .collect::<std::collections::HashSet<_>>();
    let next_fingerprints = next
        .interactive_elements
        .iter()
        .map(|element| element.fingerprint.as_str())
        .collect::<std::collections::HashSet<_>>();
    previous_fingerprints != next_fingerprints
}

fn compute_delta(previous: &PageSnapshot, next: &PageSnapshot) -> Value {
    let previous_refs = previous
        .interactive_elements
        .iter()
        .map(|element| element.ref_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let next_refs = next
        .interactive_elements
        .iter()
        .map(|element| element.ref_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let previous_text = previous
        .text_lines
        .iter()
        .map(String::as_str)
        .collect::<std::collections::HashSet<_>>();
    json!({
        "fromRevision": previous.revision,
        "toRevision": next.revision,
        "urlChanged": previous.url != next.url,
        "titleChanged": previous.title != next.title,
        "newElements": next.interactive_elements.iter().filter(|element| !previous_refs.contains(element.ref_id.as_str())).take(10).map(|element| json!({"ref": element.ref_id, "role": element.role, "name": element.name})).collect::<Vec<_>>(),
        "removedRefs": previous.interactive_elements.iter().filter(|element| !next_refs.contains(element.ref_id.as_str())).take(10).map(|element| Value::String(element.ref_id.clone())).collect::<Vec<_>>(),
        "newText": next.text_lines.iter().filter(|line| !previous_text.contains(line.as_str())).take(10).cloned().collect::<Vec<_>>(),
        "alerts": next.text_lines.iter().filter(|line| line.to_ascii_lowercase().contains("error") || line.to_ascii_lowercase().contains("failed") || line.to_ascii_lowercase().contains("invalid") || line.to_ascii_lowercase().contains("warning")).take(5).cloned().collect::<Vec<_>>(),
    })
}

fn interactive_element_value(element: &InteractiveElement) -> Value {
    json!({
        "ref": element.ref_id,
        "pageRevision": element.page_revision,
        "role": element.role,
        "name": element.name,
        "text": element.text,
        "visible": element.visible,
        "enabled": element.enabled,
        "locatorHint": {"tag": element.tag, "testId": element.test_id},
        "fingerprint": element.fingerprint,
    })
}

fn network_event_value(event: NetworkEvent) -> Value {
    json!({
        "id": event.id,
        "method": event.method,
        "url": event.url,
        "status": event.status,
        "contentType": event.content_type,
        "resourceType": event.resource_type,
        "timestamp": event.timestamp,
        "ok": event.ok,
        "errorText": event.error_text,
        "durationMs": event.duration_ms,
    })
}

fn env_non_empty(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn resolve_headless_option(cli_value: Option<String>) -> bool {
    cli_value
        .or_else(|| env_non_empty("BROWSER_MCP_HEADLESS"))
        .map(|value| value != "false")
        .unwrap_or(true)
}

fn opt_string_value(value: Option<String>) -> Value {
    value.map(Value::String).unwrap_or(Value::Null)
}

fn base_attached_runtime_diagnostics(configured_source: &ConfiguredAttachSource) -> Value {
    json!({
        "status": "not_configured",
        "descriptorSource": configured_source.source,
        "descriptorPath": configured_source.path,
        "inputArtifactKind": null,
        "schemaVersion": null,
        "attachMode": null,
        "artifactBackendFamily": null,
        "recommendedEntrypoint": null,
        "sourceTransportMethod": null,
        "sourceHandoffMethod": null,
        "traceStreamPath": null,
        "bindingArtifactSource": null,
        "handoffSource": null,
        "resumeManifestSource": null,
        "traceStreamSource": null,
        "replaySupported": false,
        "eventCount": 0,
        "latestEventId": null,
        "latestEventKind": null,
        "latestEventTimestamp": null,
        "warning": null,
    })
}

fn attached_runtime_replay_context(diagnostics: &Value) -> Value {
    json!({
        "descriptorSource": diagnostics.get("descriptorSource").cloned().unwrap_or(Value::Null),
        "descriptorPath": diagnostics.get("descriptorPath").cloned().unwrap_or(Value::Null),
        "inputArtifactKind": diagnostics.get("inputArtifactKind").cloned().unwrap_or(Value::Null),
        "attachMode": diagnostics.get("attachMode").cloned().unwrap_or(Value::Null),
        "artifactBackendFamily": diagnostics.get("artifactBackendFamily").cloned().unwrap_or(Value::Null),
        "recommendedEntrypoint": diagnostics.get("recommendedEntrypoint").cloned().unwrap_or(Value::Null),
        "sourceTransportMethod": diagnostics.get("sourceTransportMethod").cloned().unwrap_or(Value::Null),
        "sourceHandoffMethod": diagnostics.get("sourceHandoffMethod").cloned().unwrap_or(Value::Null),
        "traceStreamPath": diagnostics.get("traceStreamPath").cloned().unwrap_or(Value::Null),
        "bindingArtifactSource": diagnostics.get("bindingArtifactSource").cloned().unwrap_or(Value::Null),
        "handoffSource": diagnostics.get("handoffSource").cloned().unwrap_or(Value::Null),
        "resumeManifestSource": diagnostics.get("resumeManifestSource").cloned().unwrap_or(Value::Null),
        "traceStreamSource": diagnostics.get("traceStreamSource").cloned().unwrap_or(Value::Null),
    })
}

fn descriptor_leaf<'a>(descriptor: &'a Value, path_parts: &[&str]) -> Option<&'a Value> {
    let mut current = descriptor;
    for part in path_parts {
        current = current.get(*part)?;
    }
    Some(current)
}

fn descriptor_string(descriptor: &Value, path_parts: &[&str]) -> Option<String> {
    descriptor_leaf(descriptor, path_parts)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn descriptor_bool(descriptor: &Value, path_parts: &[&str]) -> Option<bool> {
    descriptor_leaf(descriptor, path_parts).and_then(Value::as_bool)
}

fn descriptor_resolved_artifact(descriptor: &Value, field: &str) -> Option<String> {
    descriptor_string(descriptor, &["resolved_artifacts", field])
        .or_else(|| descriptor_string(descriptor, &[field]))
}

fn normalize_runtime_locator_for_existing_file(locator: &str) -> String {
    let path = PathBuf::from(locator);
    if path.exists() {
        return path.to_string_lossy().into_owned();
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(&path))
        .ok()
        .filter(|candidate| candidate.exists())
        .map(|candidate| candidate.to_string_lossy().into_owned())
        .unwrap_or_else(|| locator.to_string())
}

fn normalized_descriptor_value(value: Option<&Value>, path_like: bool) -> Option<String> {
    let value = value?;
    if path_like {
        return value.as_str().filter(|item| !item.is_empty()).map(|item| {
            let path = PathBuf::from(item);
            if path.is_absolute() {
                path
            } else {
                std::env::current_dir()
                    .map(|cwd| cwd.join(path))
                    .unwrap_or_else(|_| PathBuf::from(item))
            }
            .to_string_lossy()
            .into_owned()
        });
    }
    Some(match value {
        Value::String(item) => item.clone(),
        Value::Bool(item) => item.to_string(),
        Value::Number(item) => item.to_string(),
        Value::Null => "null".to_string(),
        other => other.to_string(),
    })
}

fn assert_attach_descriptor_leaf_matches_canonical(
    original: &Value,
    canonical: &Value,
    path_parts: &[&str],
    path_like: bool,
) -> Result<(), String> {
    let Some(requested) = descriptor_leaf(original, path_parts) else {
        return Ok(());
    };
    if requested.is_null() {
        return Ok(());
    }
    let resolved = descriptor_leaf(canonical, path_parts).ok_or_else(|| {
        format!(
            "runtime attach descriptor must already carry canonical {}",
            path_parts.join(".")
        )
    })?;
    if normalized_descriptor_value(Some(requested), path_like)
        != normalized_descriptor_value(Some(resolved), path_like)
    {
        return Err(format!(
            "runtime attach descriptor must already match canonical {}",
            path_parts.join(".")
        ));
    }
    Ok(())
}

fn assert_attach_descriptor_matches_canonical(
    original: &Value,
    canonical: &Value,
) -> Result<(), String> {
    for field in [
        ["requested_artifacts", "binding_artifact_path"],
        ["requested_artifacts", "handoff_path"],
        ["requested_artifacts", "resume_manifest_path"],
        ["resolved_artifacts", "binding_artifact_path"],
        ["resolved_artifacts", "handoff_path"],
        ["resolved_artifacts", "resume_manifest_path"],
        ["resolved_artifacts", "trace_stream_path"],
    ] {
        assert_attach_descriptor_leaf_matches_canonical(original, canonical, &field, true)?;
    }
    for field in [
        &["attach_mode"][..],
        &["artifact_backend_family"][..],
        &["source_transport_method"][..],
        &["source_handoff_method"][..],
        &["attach_method"][..],
        &["subscribe_method"][..],
        &["cleanup_method"][..],
        &["resume_mode"][..],
        &["cleanup_semantics"][..],
        &["recommended_entrypoint"][..],
        &["attach_capabilities", "artifact_replay"][..],
        &["attach_capabilities", "live_remote_stream"][..],
        &["attach_capabilities", "cleanup_preserves_replay"][..],
        &["resolution", "binding_artifact_path"][..],
        &["resolution", "handoff_path"][..],
        &["resolution", "resume_manifest_path"][..],
        &["resolution", "trace_stream_path"][..],
    ] {
        assert_attach_descriptor_leaf_matches_canonical(original, canonical, field, false)?;
    }
    Ok(())
}

fn assert_attach_descriptor_contract(descriptor: &Value) -> Result<(), String> {
    for (field, expected) in [
        ("attach_mode", RUNTIME_ATTACH_MODE),
        (
            "source_transport_method",
            RUNTIME_ATTACH_SOURCE_TRANSPORT_METHOD,
        ),
        (
            "source_handoff_method",
            RUNTIME_ATTACH_SOURCE_HANDOFF_METHOD,
        ),
        ("attach_method", RUNTIME_ATTACH_METHOD),
        ("subscribe_method", RUNTIME_ATTACH_SUBSCRIBE_METHOD),
        ("cleanup_method", RUNTIME_ATTACH_CLEANUP_METHOD),
        ("resume_mode", RUNTIME_ATTACH_RESUME_MODE),
    ] {
        if let Some(value) = descriptor_string(descriptor, &[field]) {
            if value != expected {
                return Err(format!(
                    "runtime attach descriptor must use {field}={expected}"
                ));
            }
        }
    }
    if let Some(value) = descriptor_bool(descriptor, &["attach_capabilities", "artifact_replay"]) {
        if !value {
            return Err(
                "runtime attach descriptor must advertise attach_capabilities.artifact_replay=true"
                    .to_string(),
            );
        }
    }
    if let Some(value) = descriptor_bool(
        descriptor,
        &["attach_capabilities", "cleanup_preserves_replay"],
    ) {
        if !value {
            return Err(
                "runtime attach descriptor must advertise attach_capabilities.cleanup_preserves_replay=true"
                    .to_string(),
            );
        }
    }
    if let Some(value) = descriptor_bool(descriptor, &["attach_capabilities", "live_remote_stream"])
    {
        if value {
            return Err(
                "runtime attach descriptor must advertise attach_capabilities.live_remote_stream=false"
                    .to_string(),
            );
        }
    }
    Ok(())
}

fn attach_descriptor_needs_rust_hydration(descriptor: &Value) -> bool {
    [
        ["requested_artifacts", "binding_artifact_path"],
        ["requested_artifacts", "handoff_path"],
        ["requested_artifacts", "resume_manifest_path"],
        ["resolved_artifacts", "binding_artifact_path"],
        ["resolved_artifacts", "handoff_path"],
        ["resolved_artifacts", "resume_manifest_path"],
    ]
    .iter()
    .any(|path_parts| {
        descriptor_string(descriptor, path_parts)
            .map(|value| !value.is_empty())
            .unwrap_or(false)
    })
}

fn collect_attach_artifact_candidates(root: &Path, candidates: &mut Vec<AttachArtifactCandidate>) {
    if !root.exists() {
        return;
    }
    collect_filesystem_attach_candidates(root, candidates);
    collect_sqlite_attach_candidates(root, candidates);
}

fn default_attach_discovery_roots(repo_root: &Path) -> Vec<PathBuf> {
    vec![
        repo_root
            .join("framework_runtime")
            .join("artifacts")
            .join("scratch"),
        repo_root.join("artifacts").join("scratch"),
        repo_root.join("artifacts").join("current"),
        repo_root.to_path_buf(),
    ]
}

fn select_attach_artifact_candidate(roots: Vec<PathBuf>) -> Option<String> {
    let mut candidates = Vec::new();
    for root in roots {
        collect_attach_artifact_candidates(&root, &mut candidates);
    }
    candidates.sort_by(|left, right| {
        right
            .rank
            .cmp(&left.rank)
            .then_with(|| right.path.cmp(&left.path))
    });
    candidates
        .into_iter()
        .next()
        .map(|candidate| candidate.path)
}

fn collect_filesystem_attach_candidates(
    root: &Path,
    candidates: &mut Vec<AttachArtifactCandidate>,
) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_filesystem_attach_candidates(&path, candidates);
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        let in_transport_dir = path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
            == Some("runtime_event_transports");
        if file_name != "TRACE_RESUME_MANIFEST.json" && !in_transport_dir {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(payload) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let recency_ms = path
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or(0);
        if file_name == "TRACE_RESUME_MANIFEST.json" {
            if let Some(candidate) =
                manifest_attach_candidate(&payload, path.to_string_lossy().into_owned(), recency_ms)
            {
                candidates.push(candidate);
            }
        } else if let Some(candidate) =
            binding_attach_candidate(&payload, path.to_string_lossy().into_owned(), recency_ms)
        {
            candidates.push(candidate);
        }
    }
}

fn collect_sqlite_attach_candidates(root: &Path, candidates: &mut Vec<AttachArtifactCandidate>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_sqlite_attach_candidates(&path, candidates);
            continue;
        }
        if !file_type.is_file()
            || path.file_name().and_then(|name| name.to_str())
                != Some("runtime_checkpoint_store.sqlite3")
        {
            continue;
        }
        let recency_ms = path
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or(0);
        append_sqlite_attach_candidates(&path, recency_ms, candidates);
    }
}

fn append_sqlite_attach_candidates(
    db_path: &Path,
    recency_ms: i64,
    candidates: &mut Vec<AttachArtifactCandidate>,
) {
    let Ok(conn) = Connection::open(db_path) else {
        return;
    };
    let Ok(mut stmt) = conn.prepare(
        "SELECT rowid, payload_key, payload_text FROM runtime_storage_payloads \
         WHERE payload_key LIKE '%TRACE_RESUME_MANIFEST.json' \
            OR payload_key LIKE '%runtime_event_transports/%.json'",
    ) else {
        return;
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    }) else {
        return;
    };
    for row in rows.filter_map(Result::ok) {
        let (row_id, payload_key, payload_text) = row;
        let Ok(payload) = serde_json::from_str::<Value>(&payload_text) else {
            continue;
        };
        let row_recency = recency_ms.saturating_add(row_id);
        let attach_path = sqlite_payload_locator(db_path, &payload_key);
        if payload_key.ends_with("TRACE_RESUME_MANIFEST.json") {
            if let Some(candidate) = manifest_attach_candidate(&payload, attach_path, row_recency) {
                candidates.push(candidate);
            }
        } else if let Some(candidate) = binding_attach_candidate(
            &sqlite_rooted_binding_payload(db_path, payload),
            attach_path,
            row_recency,
        ) {
            candidates.push(candidate);
        }
    }
}

fn sqlite_payload_locator(db_path: &Path, payload_key: &str) -> String {
    let path = PathBuf::from(payload_key);
    if path.is_absolute() {
        return path.to_string_lossy().into_owned();
    }
    db_path
        .parent()
        .map(|parent| parent.join(&path))
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn sqlite_rooted_binding_payload(db_path: &Path, mut payload: Value) -> Value {
    let Some(binding_path) = descriptor_string(&payload, &["binding_artifact_path"]) else {
        return payload;
    };
    if PathBuf::from(&binding_path).is_absolute() {
        return payload;
    }
    if let Some(map) = payload.as_object_mut() {
        map.insert(
            "binding_artifact_path".to_string(),
            Value::String(sqlite_payload_locator(db_path, &binding_path)),
        );
    }
    payload
}

fn manifest_attach_candidate(
    payload: &Value,
    attach_path: String,
    recency_ms: i64,
) -> Option<AttachArtifactCandidate> {
    if descriptor_string(payload, &["schema_version"]).as_deref()
        != Some(TRACE_RESUME_MANIFEST_SCHEMA_VERSION)
    {
        return None;
    }
    descriptor_string(payload, &["event_transport_path"])?;
    Some(AttachArtifactCandidate {
        path: attach_path,
        rank: AttachArtifactCandidateRank {
            updated_at_ms: descriptor_string(payload, &["updated_at"])
                .as_deref()
                .and_then(parse_rfc3339_millis)
                .unwrap_or(0),
            recency_ms,
            source_priority: 1,
        },
    })
}

fn binding_attach_candidate(
    payload: &Value,
    fallback_attach_path: String,
    recency_ms: i64,
) -> Option<AttachArtifactCandidate> {
    if descriptor_string(payload, &["schema_version"]).as_deref()
        != Some(RUNTIME_EVENT_TRANSPORT_SCHEMA_VERSION)
    {
        return None;
    }
    if descriptor_string(payload, &["binding_backend_family"]).as_deref() == Some("filesystem") {
        return None;
    }
    let path = descriptor_string(payload, &["binding_artifact_path"])
        .filter(|path| !path.is_empty())
        .unwrap_or(fallback_attach_path);
    Some(AttachArtifactCandidate {
        path,
        rank: AttachArtifactCandidateRank {
            updated_at_ms: 0,
            recency_ms,
            source_priority: 0,
        },
    })
}

fn parse_rfc3339_millis(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|datetime| datetime.timestamp_millis())
}

fn compact_summary(summary: &Value, text_budget: usize) -> Value {
    json!({
        "mainGoalArea": truncate_text(value_str(summary.get("mainGoalArea")), text_budget),
        "visibleMessages": summary.get("visibleMessages").and_then(Value::as_array).cloned().unwrap_or_default().into_iter().map(|value| Value::String(truncate_text(&value_string(Some(&value)), text_budget.min(200)))).collect::<Vec<_>>(),
        "forms": summary.get("forms").and_then(Value::as_u64).unwrap_or(0),
        "dialogs": summary.get("dialogs").and_then(Value::as_u64).unwrap_or(0),
    })
}

fn browser_error(
    code: &str,
    message: &str,
    suggested_next_actions: &[&str],
    recoverable: bool,
) -> Value {
    json!({
        "code": code,
        "message": message,
        "recoverable": recoverable,
        "suggested_next_actions": suggested_next_actions,
    })
}

fn skill_error(code: &str, message: &str) -> Value {
    browser_error(
        code,
        message,
        &[
            "ensure the MCP server was started with --repo-root pointing at the repository root",
            "ensure skills/SKILL_ROUTING_RUNTIME.json and skills/SKILL_MANIFEST.json are generated",
        ],
        true,
    )
}

fn skill_runtime_path(repo_root: &Path) -> PathBuf {
    repo_root.join("skills/SKILL_ROUTING_RUNTIME.json")
}

fn skill_manifest_path(repo_root: &Path) -> PathBuf {
    repo_root.join("skills/SKILL_MANIFEST.json")
}

fn skill_runtime_available(repo_root: &Path) -> bool {
    skill_runtime_path(repo_root).is_file() && repo_root.join("skills").is_dir()
}

fn skill_body_path(repo_root: &Path, slug: &str) -> Result<PathBuf, String> {
    let clean = slug.trim();
    if clean.is_empty()
        || clean.contains('/')
        || clean.contains('\\')
        || clean.contains("..")
        || clean.starts_with('.')
    {
        return Err(format!("invalid skill slug: {slug}"));
    }

    let manifest_path = skill_manifest_path(repo_root);
    if manifest_path.is_file() {
        if let Some(path) = skill_body_path_from_manifest(repo_root, &manifest_path, clean)? {
            return Ok(path);
        }
    }

    let path = repo_root.join("skills").join(clean).join("SKILL.md");
    if !path.is_file() {
        return Err(format!("skill body not found: {}", path.display()));
    }
    Ok(path)
}

fn skill_body_path_from_manifest(
    repo_root: &Path,
    manifest_path: &Path,
    slug: &str,
) -> Result<Option<PathBuf>, String> {
    let payload = crate::route::read_json(manifest_path)?;
    let keys = payload
        .get("keys")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("manifest missing keys: {}", manifest_path.display()))?;
    let key_index = keys
        .iter()
        .enumerate()
        .filter_map(|(idx, key)| key.as_str().map(|raw| (raw.to_string(), idx)))
        .collect::<std::collections::HashMap<_, _>>();
    let idx_slug = *key_index
        .get("slug")
        .ok_or_else(|| format!("manifest missing slug key: {}", manifest_path.display()))?;
    let Some(idx_skill_path) = key_index.get("skill_path").copied() else {
        return Ok(None);
    };
    let rows = payload
        .get("skills")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("manifest missing skills rows: {}", manifest_path.display()))?;
    for row in rows.iter().filter_map(Value::as_array) {
        if row.get(idx_slug).and_then(Value::as_str) != Some(slug) {
            continue;
        }
        let Some(skill_path) = row.get(idx_skill_path).and_then(Value::as_str) else {
            continue;
        };
        if skill_path.starts_with('/')
            || skill_path.contains("..")
            || !skill_path.ends_with("SKILL.md")
        {
            return Err(format!("invalid skill_path for {slug}: {skill_path}"));
        }
        let path = repo_root.join(skill_path);
        if !path.is_file() {
            return Err(format!("skill body not found: {}", path.display()));
        }
        return Ok(Some(path));
    }
    Ok(None)
}

fn route_with_full_manifest_fallback(
    runtime_records: &[SkillRecord],
    manifest_path: &Path,
    query: &str,
    session_id: &str,
    allow_overlay: bool,
    first_turn: bool,
) -> Result<RouteDecision, String> {
    let hot_decision = route_task(
        runtime_records,
        query,
        session_id,
        allow_overlay,
        first_turn,
    )?;
    if !manifest_path.is_file() {
        return Ok(hot_decision);
    }
    let full_records = load_records_from_manifest(manifest_path)?;
    if full_records.len() <= runtime_records.len() {
        return Ok(hot_decision);
    }
    let full_decision = route_task(&full_records, query, session_id, allow_overlay, first_turn)?;
    if full_decision.score > hot_decision.score
        || (full_decision.score == hot_decision.score
            && full_decision.selected_skill != hot_decision.selected_skill)
        || (full_decision.selected_skill == hot_decision.selected_skill
            && full_decision.overlay_skill.is_some()
            && hot_decision.overlay_skill.is_none())
    {
        Ok(full_decision)
    } else {
        Ok(hot_decision)
    }
}

fn session_not_found_error() -> Value {
    browser_error(
        "SESSION_NOT_FOUND",
        "No active browser session exists.",
        &["call browser_open"],
        true,
    )
}

fn success_response(request_id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": request_id, "result": result})
}

fn error_response(request_id: Value, error: Value) -> Value {
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("Browser MCP server error");
    json!({"jsonrpc": "2.0", "id": request_id, "error": {"code": -32000, "message": message, "data": error}})
}

fn require_string(payload: &Value, key: &str) -> Result<String, Value> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            browser_error(
                "INVALID_INPUT",
                &format!("Missing required string field '{key}'"),
                &[&format!("provide a non-empty string for '{key}'")],
                true,
            )
        })
}

fn required_string_arg(payload: &Value, key: &str) -> Result<String, Value> {
    require_string(payload, key)
}

fn optional_string(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn optional_bool(payload: &Value, key: &str) -> Option<bool> {
    payload.get(key).and_then(Value::as_bool)
}

fn optional_u64(payload: &Value, key: &str) -> Result<Option<u64>, Value> {
    match payload.get(key) {
        None => Ok(None),
        Some(Value::Number(number)) => number.as_u64().map(Some).ok_or_else(|| {
            browser_error(
                "INVALID_INPUT",
                &format!("Expected unsigned integer for '{key}'"),
                &[&format!("pass '{key}' as an unsigned integer")],
                true,
            )
        }),
        Some(other) => Err(browser_error(
            "INVALID_INPUT",
            &format!(
                "Expected integer for '{key}', got {}",
                json_type_name(other)
            ),
            &[&format!("pass '{key}' as an integer")],
            true,
        )),
    }
}

fn optional_usize(payload: &Value, key: &str, default: usize) -> Result<usize, Value> {
    optional_u64(payload, key).map(|value| value.unwrap_or(default as u64) as usize)
}

fn optional_string_array(payload: &Value, key: &str) -> Option<Vec<String>> {
    payload.get(key).and_then(Value::as_array).map(|items| {
        items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect::<Vec<_>>()
    })
}

fn value_str(value: Option<&Value>) -> &str {
    value.and_then(Value::as_str).unwrap_or("")
}

fn value_string(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(flag)) => flag.to_string(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| value_string(Some(item)))
            .collect::<Vec<_>>()
            .join(" "),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(_) => "int",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut output = text
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    output.push_str("...");
    output
}

fn to_text_lines(text: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| seen.insert((*line).to_string()))
        .take(50)
        .map(|line| truncate_text(line, 240))
        .collect()
}

fn current_local_timestamp() -> String {
    Local::now()
        .to_rfc3339_opts(SecondsFormat::Secs, false)
        .to_string()
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn cdp_key_name(key: &str) -> String {
    match key {
        "Return" => "Enter".to_string(),
        other => other.to_string(),
    }
}

fn json_string_literal(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn decode_base64(input: &str) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0u8;
    for ch in input.bytes() {
        let value = match ch {
            b'A'..=b'Z' => ch - b'A',
            b'a'..=b'z' => ch - b'a' + 26,
            b'0'..=b'9' => ch - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b'\r' | b'\n' | b'\t' | b' ' => continue,
            other => return Err(format!("invalid base64 byte {other}")),
        } as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push(((buffer >> bits) & 0xff) as u8);
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
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

        let route_response = handle_browser_mcp_request(
            &json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "skill_route", "arguments": {"query": "路由系统触发稳定吗"}}}),
            &mut runtime,
        )
        .expect("route response");
        assert_eq!(route_response["result"]["isError"], false);
        assert_eq!(
            route_response["result"]["structuredContent"]["decision"]["selected_skill"],
            "skill-framework-developer"
        );
        assert!(
            route_response["result"]["structuredContent"]["selected_skill_path"]
                .as_str()
                .unwrap()
                .ends_with("skills/skill-framework-developer/SKILL.md")
        );

        let search_response = handle_browser_mcp_request(
            &json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "skill_search", "arguments": {"query": "路由系统", "limit": 5}}}),
            &mut runtime,
        )
        .expect("search response");
        assert_eq!(search_response["result"]["isError"], false);
        assert!(search_response["result"]["structuredContent"]["matches"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["record"]["name"] == "skill-framework-developer"));

        let read_response = handle_browser_mcp_request(
            &json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "skill_read", "arguments": {"skill": "skill-framework-developer"}}}),
            &mut runtime,
        )
        .expect("read response");
        assert_eq!(read_response["result"]["isError"], false);
        assert!(read_response["result"]["structuredContent"]["content"]
            .as_str()
            .unwrap()
            .contains("# skill-framework-developer"));
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
            .join("framework_runtime")
            .join("artifacts")
            .join("scratch")
            .join("older")
            .join("TRACE_RESUME_MANIFEST.json");
        let newer = repo_root
            .join("framework_runtime")
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
}
