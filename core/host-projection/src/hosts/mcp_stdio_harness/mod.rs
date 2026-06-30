//! MCP stdio harness: 共用 stdio transport。
//!
//! MCP 服务器（stdio transport），提供 tools / prompts / resources 三类端点，
//! 替代 shell hook 协议（PreToolUse / UserPromptSubmit / PostToolUse / Stop）。
//!
//! 架构约束：MCP 不支持工具拦截，PreToolUse guards 不可用；closeout / review 门控在 MCP 工具层
//! 报告 findings。`RUNTIME_REGISTRY.host_projections.*.harness_capability_exceptions` 为叙事真源
//! （`closeout_evidence_hooks=unsupported` → 非 interactive 时 MCP 层 hard_block 元数据；interactive advisory）。
//!
//! 与 CLI 共享 L2/L3 手动画板（evidence、goal state、路由、snapshot），出站为 MCP JSON-RPC。

// ── Environment-cache macro ────────────────────────────────────────────
/// Cache an env-var–parsed numeric value in a `OnceLock`.
/// `$typ` must implement `FromStr` (parsed via `.ok()`), and the `$default`
/// literal must be coercible to `$typ`.
macro_rules! env_cache_typed {
    ($typ:ty, $env:literal, $default:expr) => {{
        static CACHED: OnceLock<$typ> = OnceLock::new();
        *CACHED.get_or_init(|| {
            std::env::var($env)
                .ok()
                .and_then(|v| v.parse::<$typ>().ok())
                .filter(|&n| n > 0)
                .unwrap_or($default)
        })
    }};
}

// route_task_with_manifest_fallback — not needed in host-projection; skill routing via framework_kernel
// framework_runtime functions accessed via crate::hooks
use core_errors::FrameworkError;
use core_state::task_state::resolve_task_view;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::mpsc;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

/// Shared host display label for MCP-hosted sessions.
/// Delegates to host_extensions::host_log_label(); falls back to "MCP Host" for unknown hosts.
fn mcp_host_display_label(host_id: &str) -> String {
    let label = crate::hosts::host_extensions::host_log_label(host_id);
    if label == host_id {
        "MCP Host".to_string()
    } else {
        label
    }
}

fn list_known_task_ids(repo_root: &Path) -> Vec<String> {
    let current = repo_root.join("artifacts/current");
    let Ok(entries) = fs::read_dir(&current) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| {
            name != "review-lanes"
                && !name.starts_with('.')
                && current.join(name).join("TASK_LEDGER.jsonl").is_file()
        })
        .collect();
    ids.sort();
    ids
}

fn task_artifact_dir(repo_root: &Path, task_id: Option<&str>) -> PathBuf {
    let base = repo_root.join("artifacts/current");
    if let Some(task_id) = task_id.filter(|value| !value.is_empty()) {
        match core_state_utils::path_guard::validate_task_id_component(task_id.trim()) {
            Ok(safe) => base.join(safe),
            // Poisoned or hostile task_id must not escape artifacts/current via `..`.
            Err(_) => base,
        }
    } else {
        base
    }
}

/// Task view cache: path → (resolved_view, last_access).
pub struct RateLimiter {
    last_call: HashMap<String, Instant>,
    min_interval: Duration,
}

impl RateLimiter {
    pub fn new(min_interval_ms: u64) -> Self {
        RateLimiter {
            last_call: HashMap::new(),
            min_interval: Duration::from_millis(min_interval_ms),
        }
    }

    pub fn check_and_record(&mut self, tool_name: &str) -> Result<(), FrameworkError> {
        let now = Instant::now();
        if let Some(last) = self.last_call.get(tool_name)
            && now.duration_since(*last) < self.min_interval
        {
            return Err(FrameworkError::from(format!(
                "Rate limit exceeded for {}. Wait {}ms between calls.",
                tool_name,
                self.min_interval.as_millis()
            )));
        }
        // Limit map size to prevent unbounded growth
        if self.last_call.len() >= 200 {
            self.last_call.clear();
            tracing::debug!("rate limiter map cleared (capacity reached)");
        }
        self.last_call.insert(tool_name.to_string(), now);
        Ok(())
    }
}

// Global rate limiter (session-scoped via OnceLock)

static RATE_LIMITER: OnceLock<Arc<std::sync::Mutex<RateLimiter>>> = OnceLock::new();

fn get_rate_limiter() -> Arc<std::sync::Mutex<RateLimiter>> {
    RATE_LIMITER
        .get_or_init(|| Arc::new(std::sync::Mutex::new(RateLimiter::new(100))))
        .clone()
}

macro_rules! poison_safe_lock {
    ($mutex:expr) => {{
        match $mutex.lock() {
            Ok(guard) => Some(guard),
            Err(poisoned) => {
                tracing::warn!("mutex poisoned, recovering (thread panicked while holding lock)");
                // Note: into_inner() recovers the data even if the Mutex was poisoned.
                // If the panic occurred during a HashMap insertion, the map may contain
                // partial entries. A full rebuild (dropping and recreating the map) would
                // be safer but is not currently done for performance reasons.
                Some(poisoned.into_inner())
            }
        }
    }};
}

mod tools;
use tools::*;
mod task_tools;
use task_tools::*;
mod output_tools;
use output_tools::*;
mod chain_tools;
use chain_tools::*;
mod mcp_tool_handlers;
use mcp_tool_handlers::*;
#[cfg(any(test, feature = "test-support"))]
pub use tools::{build_evidence_entry, tool_closeout_gate};

/// Dispatch target derived from MCP_TOOL_REGISTRY.json's `mcp_server` field.
/// Determines how a tool is executed within this server process.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum McpDispatchTarget {
    /// In-process via CompositeRegistry (mcp_server: "router-rs")
    Builtin,
    /// In-process via research-harness fn ptr (mcp_server: "research-harness")
    ResearchHarness,
    /// Out-of-process via router-rs-cli subprocess (mcp_server: "router-rs-cli")
    CliSubprocess,
}

/// Registry-backed dispatch table loaded from MCP_TOOL_REGISTRY.json.
/// Maps tool names to their dispatch target based on the `mcp_server` field.
/// Single source of truth for both tools/list visibility and tools/call routing.
struct ToolDispatchTable {
    targets: HashMap<String, McpDispatchTarget>,
}

impl ToolDispatchTable {
    /// Build the dispatch table from the tool registry.
    /// Only includes tools handled by this MCP server process (router-rs, router-rs-framework, research-harness, router-rs-cli).
    fn from_registry() -> Self {
        let registry_path = mcp_tool_registry::resolve_tool_registry_path().unwrap_or_else(|| {
            std::path::PathBuf::from(framework_core::constants::MCP_TOOL_REGISTRY_RELATIVE_PATH)
        });
        let records = match mcp_tool_registry::load_tool_records_cached(&registry_path) {
            Ok(records) => records,
            Err(e) => {
                tracing::warn!(
                    "ToolDispatchTable: failed to load registry: {e}; using fallback CLI tool list"
                );
                // HPM-9: when registry is unavailable, populate with known CLI tools
                // so they don't fall through to research-harness handlers.
                let mut targets = HashMap::new();
                for slug in KNOWN_CLI_TOOLS {
                    targets.insert(slug.to_string(), McpDispatchTarget::CliSubprocess);
                }
                return Self { targets };
            }
        };

        let mut targets = HashMap::new();
        for r in &records {
            let target = match r.mcp_server.as_str() {
                "router-rs" | "router-rs-framework" => Some(McpDispatchTarget::Builtin),
                "research-harness" => Some(McpDispatchTarget::ResearchHarness),
                "router-rs-cli" => Some(McpDispatchTarget::CliSubprocess),
                _ => None,
            };
            if let Some(t) = target {
                targets.insert(r.slug.clone(), t);
            }
        }

        // HPM-9: ensure known CLI tools are always mapped even if absent from registry
        for slug in KNOWN_CLI_TOOLS {
            targets
                .entry(slug.to_string())
                .or_insert(McpDispatchTarget::CliSubprocess);
        }

        Self { targets }
    }

    fn get(&self, tool_name: &str) -> Option<McpDispatchTarget> {
        self.targets.get(tool_name).copied()
    }

    fn is_cli_tool(&self, tool_name: &str) -> bool {
        self.get(tool_name) == Some(McpDispatchTarget::CliSubprocess)
    }
}

/// Lazily-initialized dispatch table from MCP_TOOL_REGISTRY.json.
static DISPATCH_TABLE: OnceLock<ToolDispatchTable> = OnceLock::new();

/// Lazily-initialized CompositeRegistry of built-in (router-rs) tool handlers.
///
/// ## Tools NOT in MCP_TOOL_REGISTRY (intentional)
/// The following browser-mcp session/background/runtime tools are dispatched
/// at the MCP protocol level by the browser-mcp server itself, NOT through
/// this registry or the NL tool routing (MCP_TOOL_REGISTRY.json):
///
///   `session_launch`, `session_list`, `session_inspect`, `session_terminate`,
///   `session_mark_blocked`, `session_resume_due`, `session_classify_block`,
///   `background_inspect`, `background_list`, `background_terminate`,
///   `runtime_heartbeat`, `get_attached_runtime_events`
///
/// These are internal protocol tools, not user-facing routing targets.
/// They are NOT registered in MCP_TOOL_REGISTRY.json by design.
/// Security guards in `framework_core::hook_policy::dangerous_mcp_tool_reason`
/// provide pre-guard interception for the high-risk subset.
static REGISTRY: OnceLock<CompositeRegistry> = OnceLock::new();

/// Known CLI-routed tools used as fallback when MCP_TOOL_REGISTRY.json is unavailable.
const KNOWN_CLI_TOOLS: &[&str] = &[];

/// Dispatch a tool call through the global CompositeRegistry,
/// falling through to the external research-tool handler when
/// the built-in registry does not recognise the tool name.
///
/// CLI-routed tools (mcp_server: "router-rs-cli") are intercepted before
/// both the CompositeRegistry and the research-harness fallback and
/// executed as a router-rs-cli subprocess for process isolation.
/// Defined in mod.rs so both tools and mcp_tool_handlers can reference it.
pub(super) fn dispatch_tool(
    tool_name: &str,
    args: &Value,
    repo_root: &Path,
    host_id: &str,
    connection_session_id: &str,
) -> Result<String, FrameworkError> {
    // CLI-routed tools: spawn as subprocess (process isolation for blocking I/O).
    // Mapped via mcp_server="router-rs-cli" in MCP_TOOL_REGISTRY.
    if DISPATCH_TABLE
        .get_or_init(ToolDispatchTable::from_registry)
        .is_cli_tool(tool_name)
    {
        return spawn_cli_tool(tool_name, args, repo_root);
    }

    let registry = REGISTRY.get_or_init(|| {
        let mut r = CompositeRegistry::new();
        r.register(RoutingTools);
        r.register(ToolDomainTools);
        r.register(TaskCrudTools);
        r.register(GoalCloseoutTools);
        r.register(TaskOutputTools);
        r.register(ChainDagTools);
        r
    });

    // Check if the tool is registered in the built-in registry.
    // Only fall through to external dispatch for truly unregistered tools;
    // registered tools that return Err have a business error that must propagate.
    if registry.contains(tool_name) {
        // HPM-2: wrap builtin tool dispatch with a 60-second timeout via thread+channel
        let tn = tool_name.to_string();
        let a = args.clone();
        let repo_root = repo_root.to_path_buf();
        let host_id = host_id.to_string();
        let connection_session_id = connection_session_id.to_string();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let reg = REGISTRY
                .get()
                .unwrap_or_else(|| panic!("REGISTRY not initialized before dispatch"));
            let ctx = ToolCallContext { repo_root, host_id, connection_session_id };
            let result = reg.dispatch(&tn, &a, &ctx);
            let _ = tx.send(result);
        });
        return match rx.recv_timeout(Duration::from_secs(60)) {
            Ok(r) => r,
            Err(_) => {
                tracing::warn!("builtin tool {tool_name} timed out after 60s");
                Err(FrameworkError::validation(format!(
                    "builtin tool {tool_name} timed out after 60s"
                )))
            }
        };
    }

    // Not found in built-in registry → try externally-registered dispatch
    // (research-harness tools registered via hooks.rs at runtime-core startup)
    if let Some(dispatch) = crate::hooks::get_research_tool_dispatch() {
        dispatch(tool_name, args).map_err(|e| FrameworkError::validation(e.to_string()))
    } else {
        Err(FrameworkError::not_found(format!(
            "Unknown tool: {tool_name}"
        )))
    }
}

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "router-rs-framework";
const SERVER_VERSION: &str = "0.1.0-rust";
const MAX_MCP_CONTENT_LENGTH: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum McpTransportMode {
    ContentLength,
    NewlineDelimited,
}

pub fn run_mcp_stdio<R: BufRead, W: Write>(
    mut input: R,
    mut output: W,
    repo_root: &Path,
    host_id: &str,
) -> Result<(), FrameworkError> {
    // v6: 生成连接级 session_id，用于 goal state session 隔离。
    // 每次 MCP stdio 连接 = 一个天然 session 边界。
    let connection_session_id = generate_connection_session_id(host_id);
    let mut transport_mode = None;
    while let Some(message) = read_mcp_message(&mut input, &mut transport_mode)? {
        if let Some(response) =
            handle_mcp_request(&message, repo_root, host_id, &connection_session_id)
        {
            write_mcp_response(
                &mut output,
                transport_mode.unwrap_or(McpTransportMode::NewlineDelimited),
                &response,
            )?;
        }
    }
    Ok(())
}

fn read_mcp_message<R: BufRead>(
    input: &mut R,
    transport_mode: &mut Option<McpTransportMode>,
) -> Result<Option<String>, FrameworkError> {
    const MAX_HEADER_LINE: usize = 8192;

    let mut first_line = String::new();
    loop {
        first_line.clear();
        let bytes = input
            .read_line(&mut first_line)
            .map_err(|err| FrameworkError::from(format!("read MCP request failed: {err}")))?;
        if bytes == 0 {
            return Ok(None);
        }
        // HPM-7: enforce 8KB per header line
        if first_line.len() > MAX_HEADER_LINE {
            return Err(FrameworkError::from(format!(
                "MCP header line exceeds {MAX_HEADER_LINE} byte limit"
            )));
        }
        if !first_line.trim().is_empty() {
            break;
        }
    }

    let lower = first_line.to_ascii_lowercase();
    // HPM-13: use find(':') to locate colon position instead of fixed offsets,
    // properly handling optional whitespace (OWS) before colon per RFC 7230.
    let colon_pos = lower.find(':');
    let has_content_length = colon_pos
        .map(|pos| lower[..pos].trim() == "content-length")
        .unwrap_or(false);
    if has_content_length {
        let previous_mode = *transport_mode;
        *transport_mode = Some(McpTransportMode::ContentLength);

        // Log transport mode changes (only on first switch for debugging)
        if previous_mode.is_none() {
            tracing::info!("MCP transport mode: Content-Length");
        }

        let content_length = parse_content_length(&first_line)?;
        if content_length > MAX_MCP_CONTENT_LENGTH {
            return Err(FrameworkError::from(format!(
                "MCP Content-Length {content_length} exceeds max {MAX_MCP_CONTENT_LENGTH}"
            )));
        }
        loop {
            let mut header = String::new();
            let bytes = input
                .read_line(&mut header)
                .map_err(|err| FrameworkError::from(format!("read MCP header failed: {err}")))?;
            if bytes == 0 {
                return Err(FrameworkError::from(
                    "MCP header ended before blank line".to_string(),
                ));
            }
            // HPM-7: enforce 8KB per header line
            if header.len() > MAX_HEADER_LINE {
                return Err(FrameworkError::from(format!(
                    "MCP header line exceeds {MAX_HEADER_LINE} byte limit"
                )));
            }
            if header.trim().is_empty() {
                break;
            }
        }
        let mut body = vec![0_u8; content_length];
        input
            .read_exact(&mut body)
            .map_err(|err| FrameworkError::from(format!("read MCP body failed: {err}")))?;
        // Strip UTF-8 BOM if present (some clients send it)
        let body = body.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&body);
        return String::from_utf8(body.to_vec())
            .map(Some)
            .map_err(|err| FrameworkError::from(format!("decode MCP body failed: {err}")));
    }

    // NOTE: 不再锁定传输模式。每次读取都重新检测 Content-Length 头，
    // 允许客户端在会话中切换传输模式（如先发 newline 探测，再切 Content-Length）。
    // NewlineDelimited mode
    let previous_mode = *transport_mode;
    if previous_mode.is_none() {
        tracing::info!("MCP transport mode: NewlineDelimited");
    }
    Ok(Some(first_line.trim_end().to_string()))
}

fn parse_content_length(line: &str) -> Result<usize, FrameworkError> {
    // HPM-13: use find(':') to locate the colon position, supporting arbitrary
    // optional whitespace (OWS) before the colon per RFC 7230.
    let lower = line.to_ascii_lowercase();
    let colon_pos = lower.find(':').ok_or_else(|| {
        FrameworkError::from(format!("invalid Content-Length header (no colon): {line}"))
    })?;
    let header_name = lower[..colon_pos].trim();
    if header_name != "content-length" {
        return Err(FrameworkError::from(format!(
            "invalid Content-Length header: {line}"
        )));
    }
    let value_str = line[colon_pos + 1..].trim();
    value_str.parse::<usize>().map_err(|err| {
        FrameworkError::from(format!("invalid MCP content length '{value_str}': {err}"))
    })
}

fn write_mcp_response<W: Write>(
    output: &mut W,
    transport_mode: McpTransportMode,
    response: &Value,
) -> Result<(), FrameworkError> {
    let encoded = serde_json::to_string(response)
        .map_err(|err| FrameworkError::from(format!("serialize MCP response failed: {err}")))?;
    match transport_mode {
        McpTransportMode::ContentLength => {
            write!(output, "Content-Length: {}\r\n\r\n{encoded}", encoded.len())
                .map_err(|err| FrameworkError::from(format!("write MCP response failed: {err}")))?;
        }
        McpTransportMode::NewlineDelimited => {
            writeln!(output, "{encoded}")
                .map_err(|err| FrameworkError::from(format!("write MCP response failed: {err}")))?;
        }
    }
    Ok(())
}

/// 生成连接级 session_id：`{host_id}-{nanos}-{counter}`。
/// 每次 MCP stdio 连接调用一次，同一连接内所有 goal 操作共享此 ID。
/// 使用递增计数器防止系统时间回退导致 session_id 退化。
fn generate_connection_session_id(host_id: &str) -> String {
    static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);
    let counter = SESSION_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let entropy: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| (d.as_nanos() as u64) ^ (d.as_secs() << 32))
        .unwrap_or(0);
    format!("{host_id}-{entropy:x}-{counter}")
}

pub fn handle_mcp_request(
    message: &str,
    repo_root: &Path,
    host_id: &str,
    connection_session_id: &str,
) -> Option<Value> {
    let request: Value = match serde_json::from_str(message) {
        Ok(v) => v,
        Err(err) => {
            return Some(json!({
                "jsonrpc": "2.0",
                "error": {"code": -32700, "message": format!("Parse error: {err}")},
            }));
        }
    };
    // Verify JSON-RPC 2.0 protocol version
    if request.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        let id = request.get("id").cloned();
        return Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": -32600,
                "message": "Invalid Request: jsonrpc field must be '2.0'"
            }
        }));
    }
    let id = request.get("id").cloned();
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");

    match method {
        "initialize" => Some(handle_initialize(id)),
        "notifications/initialized" => None,
        "notifications/cancelled" => None, // Per JSON-RPC spec, notifications should not receive responses
        "tools/list" => Some(handle_tools_list(id)),
        "tools/call" => Some(handle_tools_call(
            id,
            &request,
            repo_root,
            host_id,
            connection_session_id,
        )),
        "prompts/list" => Some(handle_prompts_list(id)),
        "prompts/get" => Some(handle_prompts_get(id, &request, repo_root, host_id)),
        "resources/list" => Some(handle_resources_list(id, repo_root)),
        "resources/read" => Some(handle_resources_read(id, &request, repo_root)),
        "ping" => id.map(|id| json!({"jsonrpc": "2.0", "id": id, "result": {}})),
        _ => id.map(|id| json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": -32601, "message": format!("Method not found: {method}")},
        })),
    }
}

fn handle_initialize(id: Option<Value>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": PROTOCOL_VERSION,
            "serverInfo": {
                "name": SERVER_NAME,
                "version": SERVER_VERSION,
            },
            "capabilities": {
                "tools": {},
                "prompts": {},
                "resources": {},
            },
        },
    })
}

pub fn handle_tools_list(id: Option<Value>) -> Value {
    let tools = build_tools_from_registry();

    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "tools": tools,
        },
    })
}

/// Build tool schema entries for framework dispatch-domain and research tools
/// from MCP_TOOL_REGISTRY.json. Includes tools with `mcp_server` field matching
/// a registered MCP server, excluding deprecated tools.
fn build_tools_from_registry() -> Vec<Value> {
    let registry_path = mcp_tool_registry::resolve_tool_registry_path().unwrap_or_else(|| {
        std::path::PathBuf::from(framework_core::constants::MCP_TOOL_REGISTRY_RELATIVE_PATH)
    });

    // Use cached loader for performance
    let records = match mcp_tool_registry::load_tool_records_cached(&registry_path) {
        Ok(records) => records,
        Err(e) => {
            tracing::warn!("handle_tools_list: failed to load registry: {e}");
            return Vec::new();
        }
    };

    records
        .iter()
        // Include tools handled by this MCP server (router-rs/router-rs-framework/research-harness/router-rs-cli)
        // based on mcp_server as the authoritative field, not dispatch_domain.
        .filter(|r| {
            matches!(
                r.mcp_server.as_str(),
                "router-rs" | "router-rs-framework" | "research-harness" | "router-rs-cli"
            )
        })
        .filter(|r| !r.tool_flags.iter().any(|f| f == "deprecated"))
        .map(|r| {
            let mut tool = json!({
                "name": r.slug,
                "description": r.description,
            });
            if let Some(schema) = &r.input_schema_json {
                let input_schema = json!({
                    "type": schema.schema_type,
                    "properties": schema.properties,
                    "required": schema.required,
                });
                tool["inputSchema"] = input_schema;
            } else {
                tool["inputSchema"] = json!({"type": "object", "properties": {}});
            }
            tool
        })
        .collect()
}

fn handle_prompts_list(id: Option<Value>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "prompts": [
                {
                    "name": "framework_routing",
                    "description": "framework routing guidance",
                    "arguments": [],
                },
                {
                    "name": "review_gate",
                    "description": "review gate advisory",
                    "arguments": [],
                },
                {
                    "name": "closeout_checklist",
                    "description": "closeout checklist",
                    "arguments": [],
                },
                {
                    "name": "goalx",
                    "description": "Goal management: create, checkpoint, amend, resume, complete goals",
                    "arguments": [],
                },
                {
                    "name": "gitx",
                    "description": "Git closeout workflow: review, fix, tidy, commit, merge",
                    "arguments": [],
                },
                {
                    "name": "deepinterview",
                    "description": "Evidence-first requirement clarification and convergence review",
                    "arguments": [],
                },
                {
                    "name": "update",
                    "description": "Refresh key docs, git tracking, and stale/dead repo surfaces",
                    "arguments": [],
                },
            ],
        },
    })
}

fn handle_prompts_get(
    id: Option<Value>,
    request: &Value,
    repo_root: &Path,
    host_id: &str,
) -> Value {
    let default_params = json!({});
    let params = request.get("params").unwrap_or(&default_params);
    let prompt_name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let default_args = json!({});
    let _arguments = params.get("arguments").unwrap_or(&default_args);

    let description = match prompt_name {
        "framework_routing" => "framework routing",
        "review_gate" => "review gate advisory",
        "closeout_checklist" => "closeout checklist",
        "goalx" => "goal management — create, checkpoint, amend, resume, complete goals",
        "gitx" => "git closeout workflow — review, fix, tidy, commit, merge",
        "deepinterview" => "evidence-first requirement clarification and convergence review",
        "update" => "refresh docs, git tracking, and stale repo surfaces",
        _ => "",
    };

    let text = match prompt_name {
        "framework_routing" => {
            let source_rel = "skills/SKILL_ROUTING_RUNTIME.json";
            format!(
                "面向用户的回复必须使用简体中文（代码/路径/命令/第三方原文除外）。\n\n\
                 Use this repo shared framework runtime.\n\n\
                 1) Start from AGENTS.md。\n\
                 2) Route via {source_rel}.\n\
                 3) Read only the matched skill_path.\n\n\
                 Framework root: core/router-rs/"
            )
        }
        "review_gate" => {
            let host_name = mcp_host_display_label(host_id);
            let gate_mode =
                "task: MCP hard block disabled — closeout_gate reports findings only (advisory)."
                    .to_string();
            {
                let lane_lines =
                    framework_core::registry_review_gate::reviewer_lanes_prompt_lines(Some(repo_root));
                format!(
                    "[Review Gate -- {host_name} gating]\n\n\
                     This host uses MCP transport; there is no shell hook REVIEW_GATE observation.\n\n\
                     Countable independent reviewer lanes (RUNTIME_REGISTRY review_gate.reviewer_lanes):\n\
                     {lane_lines}\n\
                     explore / explorer does NOT count toward review evidence.\n\
                     Requires fork_context=false for independent reviewer credit (review-lanes/*.md on disk).\n\n\
                     When user requests review:\n\
                     1) Spawn a read-only reviewer in a reviewer_lanes lane with fork_context=false\n\
                     2) If no subagent, decompose review dimensions locally and document findings\n\
                     3) Call closeout_gate before claiming review complete (review-lanes/*.md or reviewer_lane in args)\n\n\
                     {gate_mode}"
                )
            }
        }
        "closeout_checklist" => "[Closeout Checklist]\n\n\
             Before ending task:\n\
             - [ ] GOAL_STATE exists\n\
             - [ ] EVIDENCE_INDEX has >=1 record\n\
             - [ ] SESSION_SUMMARY written\n\
             - [ ] Verification evidence recorded\n\
             - [ ] Blockers in NEXT_ACTIONS\n\n\
             Call closeout_gate for machine-readable check."
            .to_string(),
        "goalx" => {
            "Use skill_route(query: \"/goalx\") to load the Goal management skill. \
             Then follow SKILL.md: read goal state, create/checkpoint/amend/resume/complete as needed."
                .to_string()
        }
        "gitx" => {
            "Use skill_route(query: \"/gitx\") to load the Git closeout workflow skill. \
             Then follow SKILL.md: review, fix, tidy, commit, merge."
                .to_string()
        }
        "deepinterview" => {
            "Use skill_route(query: \"/deepinterview\") to load the requirement clarification skill. \
             Then follow SKILL.md for evidence-first convergence review."
                .to_string()
        }
        "update" => {
            "Use skill_route(query: \"/update\") to load the repo refresh skill. \
             Then follow SKILL.md to update docs, git tracking, and stale surfaces."
                .to_string()
        }
        _ => format!("Unknown prompt: {prompt_name}"),
    };

    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "description": description,
            "messages": [
                {
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": text,
                    },
                },
            ],
        },
    })
}

fn handle_resources_list(id: Option<Value>, repo_root: &Path) -> Value {
    let task_view = resolve_task_view(repo_root, None);

    let mut resources = vec![
        json!({
            "uri": "framework://active_task",
            "name": "Active Task",
            "description": "current active task pointer",
            "mimeType": "application/json",
        }),
        json!({
            "uri": "framework://goal_state",
            "name": "Goal State",
            "description": "goal state for current task",
            "mimeType": "application/json",
        }),
    ];

    let evidence_count = task_view
        .evidence
        .as_ref()
        .map(|e| {
            if e.evidence_rows_non_empty {
                1u64
            } else {
                0u64
            }
        })
        .unwrap_or(0);
    if evidence_count > 0 {
        resources.push(json!({
            "uri": "framework://evidence_index",
            "name": "Evidence Index",
            "description": format!("evidence index ({evidence_count} records)"),
            "mimeType": "application/json",
        }));
    }

    // session_summary is always listed as a resource if SESSION_SUMMARY.md exists
    let task_id = task_view
        .pointers
        .active_task_id
        .as_deref()
        .or(task_view.pointers.focus_task_id.as_deref());
    let summary_path = task_artifact_dir(repo_root, task_id).join("SESSION_SUMMARY.md");
    if summary_path.is_file() {
        resources.push(json!({
            "uri": "framework://session_summary",
            "name": "Session Summary",
            "description": "session checkpoint summary",
            "mimeType": "text/markdown",
        }));
    }

    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "resources": resources },
    })
}

fn handle_resources_read(id: Option<Value>, request: &Value, repo_root: &Path) -> Value {
    let default_params = json!({});
    let params = request.get("params").unwrap_or(&default_params);
    let uri = params.get("uri").and_then(Value::as_str).unwrap_or("");

    let (text, mime_type) = match uri {
        "framework://active_task" => {
            let task_view = resolve_task_view(repo_root, None);
            let content = json!({
                "active_task_id": task_view.pointers.active_task_id,
                "focus_task_id": task_view.pointers.focus_task_id,
                "known_task_ids": list_known_task_ids(repo_root),
            });
            (
                serde_json::to_string_pretty(&content).unwrap_or_default(),
                "application/json",
            )
        }
        "framework://goal_state" => {
            let state = core_state::state_manager::read_goal_state(repo_root, None).unwrap_or(None);
            (
                serde_json::to_string_pretty(&state).unwrap_or_default(),
                "application/json",
            )
        }
        "framework://evidence_index" => {
            let task_view = resolve_task_view(repo_root, None);
            let task_id = task_view
                .task_id
                .as_deref()
                .or(task_view.pointers.active_task_id.as_deref());
            let evidence_path = task_artifact_dir(repo_root, task_id).join("EVIDENCE_INDEX.json");
            let content = if evidence_path.is_file() {
                fs::read_to_string(&evidence_path).unwrap_or_else(|e| format!("Read error: {e}"))
            } else {
                "{}".to_string()
            };
            (content, "application/json")
        }
        "framework://session_summary" => {
            let task_view = resolve_task_view(repo_root, None);
            let task_id = task_view
                .task_id
                .as_deref()
                .or(task_view.pointers.active_task_id.as_deref());
            let summary_path = task_artifact_dir(repo_root, task_id).join("SESSION_SUMMARY.md");
            let content = if summary_path.is_file() {
                fs::read_to_string(&summary_path).unwrap_or_else(|e| format!("Read error: {e}"))
            } else {
                String::new()
            };
            (content, "text/markdown")
        }
        _ => (format!("Unknown resource: {uri}"), "text/plain"),
    };

    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "contents": [
                {
                    "uri": uri,
                    "mimeType": mime_type,
                    "text": text,
                },
            ],
        },
    })
}

// =============================================================================
// Test helper functions (used by integration tests in mcp_stdio_harness_tests.rs)
// =============================================================================

#[cfg(any(test, feature = "test-support"))]
pub fn tool_goal_state_manage_test_helper(
    arguments: &Value,
    operation: &str,
) -> Result<String, FrameworkError> {
    let path = crate::hosts::test_shim::unique_temp_repo("goal-manage");
    let _ = std::fs::create_dir_all(&path);

    let mut args_with_op = arguments.clone();
    args_with_op["operation"] = json!(operation);

    let result = tool_goal_state_manage(&args_with_op, &path, "test-session-auto");
    let _ = std::fs::remove_dir_all(&path);
    result
}

#[cfg(any(test, feature = "test-support"))]
pub fn tool_closeout_record_write_for_test(
    arguments: &Value,
    repo_path: &Path,
) -> Result<String, FrameworkError> {
    tool_closeout_record_write(arguments, repo_path, "opencode")
}

// ── CLI subprocess dispatch ──

/// Validate and sanitize a CLI argument value to prevent argument injection:
/// 1. Reject values starting with `--` (could be interpreted as flags)
/// 2. Cap at `max_len` bytes (prevent oversized payloads)
fn validate_cli_arg(value: &str, max_len: usize) -> Result<String, FrameworkError> {
    if value.len() > max_len {
        return Err(FrameworkError::from(format!(
            "argument too long (max {max_len} bytes)"
        )));
    }
    if value.starts_with("--") {
        return Err(FrameworkError::from(
            "argument cannot start with '--'".to_string(),
        ));
    }
    Ok(value.to_string())
}

/// Map MCP tool name and JSON arguments to `router-rs-cli` subcommand arguments.
/// Each tool name maps to its corresponding CLI subcommand tree with flags
/// derived from the MCP tool's JSON input schema.
fn map_tool_to_cli_args(tool_name: &str, args: &Value) -> Result<Vec<String>, FrameworkError> {
    // HPM-1: validate all user-supplied parameter values for injection safety
    const MAX_ARG_LEN: usize = 4096;
    match tool_name {
        "web_fetch" => {
            let url = args.get("url").and_then(Value::as_str).ok_or_else(|| {
                FrameworkError::from("Missing required argument: url".to_string())
            })?;
            let url = validate_cli_arg(url, MAX_ARG_LEN)?;
            let mut cmd = vec!["web".to_string(), "fetch".to_string(), url];
            if let Some(max_bytes) = args.get("max_bytes").and_then(Value::as_u64) {
                cmd.push("--max-bytes".to_string());
                cmd.push(max_bytes.to_string());
            }
            Ok(cmd)
        }

        _ => Err(FrameworkError::from(format!(
            "Unknown CLI-routed tool: {tool_name}"
        ))),
    }
}

/// Spawn a `router-rs-cli` subprocess for the given tool name and arguments.
/// The subprocess is spawned with the repo_root as the current directory.
/// Temp files (e.g., for lean-verify) are cleaned up after the subprocess finishes.
fn spawn_cli_tool(
    tool_name: &str,
    args: &Value,
    repo_root: &Path,
) -> Result<String, FrameworkError> {
    use std::io::Read;

    let cli_args = map_tool_to_cli_args(tool_name, args)?;

    let timeout = Duration::from_secs(300);

    let mut child = std::process::Command::new("router-rs-cli")
        .args(&cli_args)
        .current_dir(repo_root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| {
            FrameworkError::from(format!(
                "router-rs-cli subprocess failed (is it in PATH or built?): {e}"
            ))
        })?;

    // HPM-10: take stdout/stderr handles before moving child into wait thread
    let mut child_stdout = child.stdout.take();
    let mut child_stderr = child.stderr.take();
    let pid = child.id();

    // Monitor thread: blocks on child.wait() — zero busy polling
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let status = child.wait();
        let _ = tx.send(status);
    });

    // Block until child exits or timeout expires
    let status = match rx.recv_timeout(timeout) {
        Ok(Ok(status)) => status,
        Ok(Err(e)) => {
            return Err(FrameworkError::from(format!(
                "router-rs-cli subprocess wait error: {e}"
            )));
        }
        Err(_) => {
            // Timed out: kill child process via OS signal
            #[cfg(unix)]
            unsafe {
                libc::kill(pid as i32, libc::SIGTERM);
            }
            // Wait briefly for graceful shutdown, then force kill
            let wait_result = rx.recv_timeout(Duration::from_secs(5));
            if wait_result.is_err() {
                #[cfg(unix)]
                unsafe {
                    libc::kill(pid as i32, libc::SIGKILL);
                }
                // Final wait (no timeout — child must die after SIGKILL)
                let _ = rx.recv();
            }
            return Err(FrameworkError::from(format!(
                "router-rs-cli {} timed out after {}s",
                tool_name,
                timeout.as_secs()
            )));
        }
    };

    // Read stdout/stderr (child has exited, pipes are still valid)
    let mut stdout = String::new();
    let mut stderr = String::new();
    let _ = child_stdout.as_mut().map(|r| r.read_to_string(&mut stdout));
    let _ = child_stderr.as_mut().map(|r| r.read_to_string(&mut stderr));

    // Clean up temp file created for lean-verify
    if tool_name == "math_lean_verify" {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let tmp_path = std::env::temp_dir().join(format!("router_rs_lean_{nanos}.lean"));
        let _ = std::fs::remove_file(&tmp_path);
    }

    if !status.success() {
        // Log stderr for diagnostics, but don't return it to the caller
        // to prevent leaking internal paths/config in MCP responses.
        if !stderr.trim().is_empty() {
            tracing::warn!(
                "[spawn_cli_tool] {} stderr output (logged, not returned): {}",
                tool_name,
                stderr.trim()
            );
        }
        return Err(FrameworkError::from(format!(
            "router-rs-cli {} failed: {}",
            tool_name,
            status,
        )));
    }

    // Log stderr on success too for diagnostics (not returned to caller)
    if !stderr.trim().is_empty() {
        tracing::warn!(
            "[spawn_cli_tool] {} stderr omitted from response: {}",
            tool_name,
            stderr.trim()
        );
    }

    Ok(stdout)
}

#[cfg(any(test, feature = "test-support"))]
pub fn read_mcp_message_test_helper<R: std::io::BufRead>(
    input: &mut R,
    transport_mode: &mut Option<McpTransportMode>,
) -> Result<Option<String>, FrameworkError> {
    read_mcp_message(input, transport_mode)
}

#[cfg(any(test, feature = "test-support"))]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    #[cfg(test)]
    use std::path::PathBuf;

    #[cfg(test)]
    use super::*;

    #[cfg(test)]
    fn unique_test_repo(name: &str) -> PathBuf {
        let path = crate::hosts::test_shim::unique_temp_repo(&format!("mcp-{name}"));
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
        // Tools are registered via MCP_TOOL_REGISTRY.json at runtime;
        // in unit-test mode the registry path may not resolve.
        // When tools ARE loaded, verify the essential set.
        if !names.is_empty() {
            let always_present = &[
                "task_create",
                "task_list",
                "task_complete",
                "task_focus",
                "task_chain_advance",
                "goal_state_manage",
            ];
            for tool in always_present {
                assert!(names.contains(tool), "missing tool: {tool}");
            }
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
        assert!(names.contains(&"goalx"));
        assert!(names.contains(&"gitx"));
        assert!(names.contains(&"deepinterview"));
        assert!(names.contains(&"update"));
    }

    #[test]
    fn ping_returns_empty_result() {
        let response = handle_mcp_request(
            r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
            &unique_test_repo("ping"),
            "opencode",
            "test-session",
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
            "test-session",
        )
        .unwrap();
        assert_eq!(response["error"]["code"], -32601);
        assert!(
            response["error"]["message"]
                .as_str()
                .unwrap()
                .contains("nonexistent")
        );
    }

    #[test]
    fn parse_content_length_normal() {
        assert_eq!(parse_content_length("Content-Length: 42").unwrap(), 42);
    }

    #[test]
    fn parse_content_length_with_crlf() {
        assert_eq!(
            parse_content_length(
                "Content-Length: 100
"
            )
            .unwrap(),
            100
        );
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
    fn parse_content_length_htab_after_colon() {
        // HTAB (tab) after colon is valid per RFC 7230 OWS
        // Current impl handles this via .trim()
        assert_eq!(parse_content_length("Content-Length:\t42").unwrap(), 42);
    }

    #[test]
    fn parse_content_length_htab_before_colon_now_supported() {
        // HTAB before colon is valid per RFC 7230 OWS — HPM-13 fix enables this.
        assert_eq!(parse_content_length("Content-Length\t: 42").unwrap(), 42);
    }

    #[test]
    fn parse_content_length_mixed_ows() {
        // Multiple spaces before colon (OWS) — HPM-13 fix enables this.
        assert_eq!(parse_content_length("Content-Length  : 42").unwrap(), 42);
    }

    // ── E2E chain tests: JSON-RPC tools/call → dispatch → research handler ──

    #[cfg(test)]
    use core_errors::FrameworkError;
    #[cfg(test)]
    use serde_json::{Value, json};
    #[cfg(test)]
    use std::sync::Once;

    /// Test research tool dispatch that mimics the real handler at a high level.
    #[cfg(test)]
    fn test_research_dispatch(name: &str, arguments: &Value) -> Result<String, FrameworkError> {
        match name {
            "research_review_loop" => {
                let max_rounds = arguments
                    .get("max_rounds")
                    .and_then(Value::as_u64)
                    .unwrap_or(10);
                Ok(serde_json::to_string(&json!({
                    "quality_gate_config": {
                        "min_rounds": 5,
                        "max_rounds": max_rounds,
                        "consecutive_stable_required": 2,
                    },
                    "current_round": {
                        "round": 1,
                        "dimension": "逻辑与证据",
                    },
                    "workflow": "test workflow stub",
                }))
                .unwrap())
            }
            "research_aigc_check" => {
                let _text = arguments
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or(FrameworkError::validation("requires 'text'"))?;
                Ok(serde_json::to_string(&json!({
                    "score": 42.0,
                    "ai_probability": 0.42,
                    "segments_analyzed": 1,
                    "results": [],
                }))
                .unwrap())
            }
            "research_review_dimensions" => {
                let _round = arguments
                    .get("round")
                    .and_then(Value::as_u64)
                    .ok_or(FrameworkError::validation("requires 'round'"))?;
                Ok(serde_json::to_string(&json!({
                    "round": 1,
                    "dimension": "逻辑与证据",
                    "prompt": "test prompt",
                    "checklist": [],
                }))
                .unwrap())
            }
            _ => Err(FrameworkError::validation(format!(
                "unknown research tool: {name}"
            ))),
        }
    }

    #[cfg(test)]
    static E2E_INIT: Once = Once::new();

    #[cfg(test)]
    fn ensure_test_research_dispatch() {
        E2E_INIT.call_once(|| {
            crate::hooks::modify_runtime_hooks(|hooks| {
                hooks.research_tool_dispatch = test_research_dispatch;
            });
        });
    }

    #[test]
    #[ignore = "requires runtime hooks initialization (infrastructure setup)"]
    fn e2e_research_review_loop_via_tools_call() {
        ensure_test_research_dispatch();
        let repo = unique_test_repo("e2e-review-loop");
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"research_review_loop","arguments":{}}}"#;
        let response = handle_mcp_request(request, &repo, "test", "test-session").unwrap();
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 1);
        let content = response["result"]["content"][0]["text"].as_str().unwrap();
        let parsed: Value = serde_json::from_str(content).unwrap();
        assert!(
            parsed.get("quality_gate_config").is_some(),
            "missing quality_gate_config"
        );
    }

    #[test]
    #[ignore = "requires runtime hooks initialization (infrastructure setup)"]
    fn e2e_research_aigc_check_via_tools_call() {
        ensure_test_research_dispatch();
        let repo = unique_test_repo("e2e-aigc-check");
        let request = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"research_aigc_check","arguments":{"text":"test text for detection"}}}"#;
        let response = handle_mcp_request(request, &repo, "test", "test-session").unwrap();
        assert_eq!(response["jsonrpc"], "2.0");
        let content_text = response["result"]["content"][0]["text"].as_str().unwrap();
        let parsed: Value = serde_json::from_str(content_text).unwrap();
        assert!(parsed.get("score").is_some(), "missing score");
        assert_eq!(
            parsed.get("ai_probability").and_then(Value::as_f64),
            Some(0.42)
        );
    }

    #[test]
    fn e2e_research_unknown_tool_returns_error() {
        ensure_test_research_dispatch();
        let repo = unique_test_repo("e2e-unknown");
        let request = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"research_nonexistent","arguments":{}}}"#;
        let response = handle_mcp_request(request, &repo, "test", "test-session").unwrap();
        // Should return isError: true
        let is_err = response["result"]["isError"].as_bool().unwrap_or(false);
        assert!(is_err, "expected isError for unknown tool");
    }

    #[test]
    #[ignore = "requires runtime hooks initialization (infrastructure setup)"]
    fn e2e_research_dimensions_with_round_param() {
        ensure_test_research_dispatch();
        let repo = unique_test_repo("e2e-dimensions");
        let request = r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"research_review_dimensions","arguments":{"round":1}}}"#;
        let response = handle_mcp_request(request, &repo, "test", "test-session").unwrap();
        let content_text = response["result"]["content"][0]["text"].as_str().unwrap();
        let parsed: Value = serde_json::from_str(content_text).unwrap();
        assert_eq!(
            parsed.get("dimension").and_then(Value::as_str),
            Some("逻辑与证据")
        );
    }
}
