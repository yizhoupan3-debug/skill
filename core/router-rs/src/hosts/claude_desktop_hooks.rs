//! Claude Desktop MCP agent: `router-rs claude-desktop agent --repo-root …`。
//!
//! MCP 服务器（stdio transport），提供 tools / prompts / resources 三类端点，
//! 替代 Claude Code CLI 的 shell hook 协议（PreToolUse / UserPromptSubmit / PostToolUse / Stop）。
//!
//! 架构约束：MCP 不支持工具拦截，因此 PreToolUse guards（framework/settings path guard、
//! dangerous bash guard）在 Desktop 上不可用，依赖 CLAUDE.md 指令自律。
//! Stop / UserPromptSubmit 无 shell 硬拦；非 my-light 时 `closeout_gate` / `goal_state_manage complete` 在 MCP 工具层硬拦。
//!
//! 与 CLI 共享 L2/L3 手动画板（evidence、goal state、路由、snapshot），出站为 MCP JSON-RPC。

use crate::cli::route_task_with_manifest_fallback;
use crate::framework_runtime::{
    build_automatic_continuity_checkpoint_payload_with_task_id,
    build_framework_runtime_snapshot_envelope,
    resolve_repo_root_arg,
};
use crate::route::{filter_records_for_host, load_records_cached_for_stdio};
use crate::skill_repo::skill_routing_runtime_json;
use crate::session_call_tracker::{
    check_anomalies, init_tracker, read_tracker_state, record_tool_call,
};
use crate::hook_common::is_review_prompt;
use crate::review_gate_engine::{claude_independent_reviewer_evidence, fork_context_from_values};
use crate::task_state::resolve_task_view;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

const WEB_FETCH_MAX_BYTES_DEFAULT: usize = 50_000;
const WEB_FETCH_TIMEOUT_SECS: u64 = 30;

fn mcp_host_supports_hard_closeout(host_id: &str) -> bool {
    matches!(
        host_id,
        "antigravity-app" | "antigravity" | "claude-desktop"
    )
}

/// Shared host display label for MCP-hosted sessions.
/// Used by hard-block, closeout gate, and review gate prompts.
fn mcp_host_display_label(host_id: &str) -> &'static str {
    match host_id {
        "antigravity-app" | "antigravity" => "Antigravity App",
        "claude-desktop" => "Claude Desktop",
        "opencode" => "Opencode",
        _ => "MCP Host",
    }
}

fn mcp_host_hard_block_label(host_id: &str) -> &'static str {
    mcp_host_display_label(host_id)
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
        .filter(|name| name != "review-lanes" && !name.starts_with('.'))
        .collect();
    ids.sort();
    ids
}

fn task_artifact_dir(repo_root: &Path, task_id: Option<&str>) -> PathBuf {
    let base = repo_root.join("artifacts/current");
    if let Some(task_id) = task_id.filter(|value| !value.is_empty()) {
        match crate::path_guard::validate_task_id_component(task_id.trim()) {
            Ok(safe) => base.join(safe),
            // Poisoned or hostile task_id must not escape artifacts/current via `..`.
            Err(_) => base,
        }
    } else {
        base
    }
}

/// Cache entry for framework_snapshot responses (30 second TTL).
struct SnapshotCache {
    content: String,
    expires_at: Instant,
}

impl SnapshotCache {
    fn is_valid(&self) -> bool {
        Instant::now() < self.expires_at
    }
}

/// Rate limiter state for tool call frequency control.
pub(crate) struct RateLimiter {
    last_call: HashMap<String, Instant>,
    min_interval: Duration,
}

impl RateLimiter {
    pub(crate) fn new(min_interval_ms: u64) -> Self {
        RateLimiter {
            last_call: HashMap::new(),
            min_interval: Duration::from_millis(min_interval_ms),
        }
    }

    pub(crate) fn check_and_record(&mut self, tool_name: &str) -> Result<(), String> {
        let now = Instant::now();
        if let Some(last) = self.last_call.get(tool_name) {
            if now.duration_since(*last) < self.min_interval {
                return Err(format!(
                    "Rate limit exceeded for {}. Wait {}ms between calls.",
                    tool_name,
                    self.min_interval.as_millis()
                ));
            }
        }
        self.last_call.insert(tool_name.to_string(), now);
        Ok(())
    }
}

// Global caches and rate limiter (session-scoped via OnceLock)
static SNAPSHOT_CACHE: OnceLock<Arc<std::sync::Mutex<Option<SnapshotCache>>>> = OnceLock::new();
static TASK_VIEW_CACHE: OnceLock<
    Arc<std::sync::Mutex<Option<(crate::task_state::ResolvedTaskView, Instant)>>>,
> = OnceLock::new();
static RATE_LIMITER: OnceLock<Arc<std::sync::Mutex<RateLimiter>>> = OnceLock::new();

/// Poison-safe lock helper that recovers from mutex poisoning.
/// Returns the guard, or None if lock acquisition failed.
macro_rules! poison_safe_lock {
    ($mutex:expr) => {{
        match $mutex.lock() {
            Ok(guard) => Some(guard),
            Err(poisoned) => {
                eprintln!(
                    "[router-rs warning] mutex poisoned, recovering (thread panicked while holding lock)"
                );
                Some(poisoned.into_inner())
            }
        }
    }};
}

fn get_snapshot_cache() -> &'static Arc<std::sync::Mutex<Option<SnapshotCache>>> {
    SNAPSHOT_CACHE.get_or_init(|| Arc::new(std::sync::Mutex::new(None)))
}

fn get_task_view_cache(
) -> &'static Arc<std::sync::Mutex<Option<(crate::task_state::ResolvedTaskView, Instant)>>> {
    TASK_VIEW_CACHE.get_or_init(|| Arc::new(std::sync::Mutex::new(None)))
}

fn get_rate_limiter() -> &'static Arc<std::sync::Mutex<RateLimiter>> {
    RATE_LIMITER.get_or_init(|| {
        let interval = if cfg!(test) { 0 } else { 100 };
        Arc::new(std::sync::Mutex::new(RateLimiter::new(interval)))
    })
}

/// Get snapshot cache TTL from environment variable.
/// Default: 30 seconds. Env: ROUTER_RS_DESKTOP_SNAPSHOT_CACHE_TTL_SECS
fn snapshot_cache_ttl_secs() -> u64 {
    static CACHED: OnceLock<u64> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("ROUTER_RS_DESKTOP_SNAPSHOT_CACHE_TTL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(30)
    })
}

/// Get task view cache TTL from environment variable.
/// Default: 5 seconds. Env: ROUTER_RS_DESKTOP_TASK_VIEW_CACHE_TTL_SECS
fn task_view_cache_ttl_secs() -> u64 {
    static CACHED: OnceLock<u64> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("ROUTER_RS_DESKTOP_TASK_VIEW_CACHE_TTL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(5)
    })
}

/// Get cached task view with configurable TTL (default 5 seconds).
fn get_cached_task_view(repo_root: &Path) -> crate::task_state::ResolvedTaskView {
    let ttl_secs = task_view_cache_ttl_secs();
    {
        let cache = get_task_view_cache();
        if let Some(guard) = poison_safe_lock!(cache) {
            if let Some((ref view, ref expires_at)) = *guard {
                if Instant::now() < *expires_at {
                    return view.clone();
                }
            }
        }
    }

    // Cache miss: recompute
    let view = resolve_task_view(repo_root, None);

    // Update cache with configurable TTL
    {
        let cache = get_task_view_cache();
        if let Some(mut guard) = poison_safe_lock!(cache) {
            *guard = Some((view.clone(), Instant::now() + Duration::from_secs(ttl_secs)));
        }
    }

    view
}

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "router-rs-framework";
const SERVER_VERSION: &str = "0.1.0-rust";
const MAX_MCP_CONTENT_LENGTH: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum McpTransportMode {
    ContentLength,
    NewlineDelimited,
}

pub fn run_claude_desktop_mcp_loop(repo_root_arg: Option<&Path>) -> Result<(), String> {
    let repo_root = resolve_repo_root_arg(repo_root_arg)?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_mcp_stdio(stdin.lock(), stdout.lock(), &repo_root, "claude-desktop")
}

pub fn run_antigravity_mcp_loop(repo_root_arg: Option<&Path>) -> Result<(), String> {
    let repo_root = resolve_repo_root_arg(repo_root_arg)?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_mcp_stdio(stdin.lock(), stdout.lock(), &repo_root, "antigravity-app")
}

pub(crate) fn run_mcp_stdio<R: BufRead, W: Write>(
    mut input: R,
    mut output: W,
    repo_root: &Path,
    host_id: &str,
) -> Result<(), String> {
    // 初始化 session tracker（session 级别，只执行一次）
    // 注意：init_tracker 失败不会阻塞 MCP 服务，因为某些环境可能不支持 tracker 文件
    if let Err(e) = init_tracker(repo_root) {
        eprintln!(
            "[router-rs warning] init_tracker failed: session call tracking may not work. \
             Error: {}. This is non-fatal for MCP operation.",
            e
        );
    }
    let mut transport_mode = None;
    while let Some(message) = read_mcp_message(&mut input, &mut transport_mode)? {
        if let Some(response) = handle_mcp_request(&message, repo_root, host_id) {
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
) -> Result<Option<String>, String> {
    let mut first_line = String::new();
    loop {
        first_line.clear();
        let bytes = input
            .read_line(&mut first_line)
            .map_err(|err| format!("read MCP request failed: {err}"))?;
        if bytes == 0 {
            return Ok(None);
        }
        if !first_line.trim().is_empty() {
            break;
        }
    }

    let lower = first_line.to_ascii_lowercase();
    // HTTP headers may have optional whitespace (OWS) before the colon per RFC 7230
    let has_content_length =
        lower.starts_with("content-length:") || lower.starts_with("content-length :");
    if has_content_length {
        let previous_mode = *transport_mode;
        *transport_mode = Some(McpTransportMode::ContentLength);

        // Log transport mode changes (only on first switch for debugging)
        if previous_mode.is_none() {
            eprintln!("[router-rs info] MCP transport mode: Content-Length");
        }

        let content_length = parse_content_length(&first_line)?;
        if content_length > MAX_MCP_CONTENT_LENGTH {
            return Err(format!(
                "MCP Content-Length {content_length} exceeds max {MAX_MCP_CONTENT_LENGTH}"
            ));
        }
        loop {
            let mut header = String::new();
            let bytes = input
                .read_line(&mut header)
                .map_err(|err| format!("read MCP header failed: {err}"))?;
            if bytes == 0 {
                return Err("MCP header ended before blank line".to_string());
            }
            if header.trim().is_empty() {
                break;
            }
        }
        let mut body = vec![0_u8; content_length];
        input
            .read_exact(&mut body)
            .map_err(|err| format!("read MCP body failed: {err}"))?;
        // Strip UTF-8 BOM if present (some clients send it)
        let body = body.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&body);
        return String::from_utf8(body.to_vec())
            .map(Some)
            .map_err(|err| format!("decode MCP body failed: {err}"));
    }

    // NOTE: 不再锁定传输模式。每次读取都重新检测 Content-Length 头，
    // 允许客户端在会话中切换传输模式（如先发 newline 探测，再切 Content-Length）。
    // NewlineDelimited mode
    let previous_mode = *transport_mode;
    if previous_mode.is_none() {
        eprintln!("[router-rs info] MCP transport mode: NewlineDelimited");
    }
    Ok(Some(first_line.trim_end().to_string()))
}

fn parse_content_length(line: &str) -> Result<usize, String> {
    // Handle both "Content-Length:" and "Content-Length :" (OWS)
    // Note: line may contain trailing \r\n from read_line
    let lower = line.to_ascii_lowercase();
    let value_str = if lower.starts_with("content-length :") {
        // Skip "content-length :" (15 chars)
        line[15..].trim()
    } else if lower.starts_with("content-length:") {
        // Skip "content-length:" (14 chars)
        line[14..].trim()
    } else {
        return Err(format!("invalid Content-Length header: {}", line));
    };
    value_str
        .parse::<usize>()
        .map_err(|err| format!("invalid MCP content length '{value_str}': {err}"))
}

fn write_mcp_response<W: Write>(
    output: &mut W,
    transport_mode: McpTransportMode,
    response: &Value,
) -> Result<(), String> {
    let encoded = serde_json::to_string(response)
        .map_err(|err| format!("serialize MCP response failed: {err}"))?;
    match transport_mode {
        McpTransportMode::ContentLength => {
            write!(output, "Content-Length: {}\r\n\r\n{encoded}", encoded.len())
                .map_err(|err| format!("write MCP response failed: {err}"))?;
        }
        McpTransportMode::NewlineDelimited => {
            writeln!(output, "{encoded}")
                .map_err(|err| format!("write MCP response failed: {err}"))?;
        }
    }
    Ok(())
}

pub(crate) fn handle_mcp_request(message: &str, repo_root: &Path, host_id: &str) -> Option<Value> {
    let request: Value = match serde_json::from_str(message) {
        Ok(v) => v,
        Err(err) => {
            return Some(json!({
                "jsonrpc": "2.0",
                "error": {"code": -32700, "message": format!("Parse error: {err}")},
            }));
        }
    };
    let id = request.get("id").cloned();
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");

    match method {
        "initialize" => Some(handle_initialize(id)),
        "notifications/initialized" => None,
        "notifications/cancelled" => None, // Per JSON-RPC spec, notifications should not receive responses
        "tools/list" => Some(handle_tools_list(id)),
        "tools/call" => Some(handle_tools_call(id, &request, repo_root, host_id)),
        "prompts/list" => Some(handle_prompts_list(id)),
        "prompts/get" => Some(handle_prompts_get(id, &request, repo_root, host_id)),
        "resources/list" => Some(handle_resources_list(id, repo_root)),
        "resources/read" => Some(handle_resources_read(id, &request, repo_root)),
        "ping" => id.map(|id| json!({"jsonrpc": "2.0", "id": id, "result": {}})),
        _ => Some(json!({
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

pub(crate) fn handle_tools_list(id: Option<Value>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "tools": [
                {
                    "name": "framework_snapshot",
                    "description": "返回当前仓库的框架运行时快照（与 `router-rs framework snapshot` 同源），含完整连续性视图。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                    },
                },
                {
                    "name": "skill_route",
                    "description": "传入自然语言查询，返回匹配的 skill 路由结果（与热路由 `SKILL_ROUTING_RUNTIME.json` 同源）。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": "自然语言查询",
                            },
                        },
                        "required": ["query"],
                    },
                },
                {
                    "name": "record_evidence",
                    "description": "追加一条 evidence 记录到当前 task 的 EVIDENCE_INDEX（与 CLI PostToolUse 自动追加同形）。agent 应在执行验证类命令后主动调用。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "tool_name": {
                                "type": "string",
                                "description": "工具名（如 Bash、Read、Write）",
                            },
                            "command": {
                                "type": "string",
                                "description": "执行的命令或操作描述",
                            },
                            "exit_code": {
                                "type": "integer",
                                "description": "exit code，0 表示成功",
                            },
                            "output": {
                                "type": "string",
                                "description": "命令输出摘要（可选，最多 2000 字符）",
                            },
                        },
                        "required": ["tool_name", "command"],
                    },
                },
                {
                    "name": "session_checkpoint",
                    "description": "写入 SESSION_SUMMARY 和 NEXT_ACTIONS checkpoint（与 CLI Stop 自动写入同形）。agent 应在完成阶段性工作时主动调用。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "summary": {
                                "type": "string",
                                "description": "当前会话进展摘要",
                            },
                            "next_actions": {
                                "type": "array",
                                "items": {"type": "string"},
                                "description": "下一步行动列表",
                            },
                            "task_id": {
                                "type": "string",
                                "description": "task id，默认当前 active task",
                            },
                        },
                        "required": ["summary"],
                    },
                },
                {
                    "name": "closeout_gate",
                    "description": "返回 closeout 状态与缺失项清单（与 CLI Stop 同源）。非 my-light 时未满足则后续 complete 硬拦；my-light 为 advisory 自检。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "task_id": {
                                "type": "string",
                                "description": "task id，默认当前 active task",
                            },
                        },
                    },
                },
                {
                    "name": "goal_state_read",
                    "description": "读取当前 task 的 GOAL_STATE.json 内容。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "task_id": {
                                "type": "string",
                                "description": "task id，默认当前 active task",
                            },
                        },
                    },
                },
                {
                    "name": "rfv_loop_status",
                    "description": "查看 RFV 循环状态。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "task_id": {
                                "type": "string",
                                "description": "task id，默认当前 active task",
                            },
                        },
                    },
                },
                {
                    "name": "rfv_loop_manage",
                    "description": "管理 RFV 循环：start / append_round。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "operation": {
                                "type": "string",
                                "enum": ["start", "append_round"],
                                "description": "操作类型",
                            },
                            "task_id": {
                                "type": "string",
                                "description": "task id，默认当前 active task",
                            },
                            "round": {
                                "type": "integer",
                                "description": "RFV round number (append_round 时需要)",
                            },
                            "goal": {
                                "type": "string",
                                "description": "RFV goal (start 时需要)",
                            },
                            "review_summary": {
                                "type": "string",
                                "description": "append_round 时需要",
                            },
                            "fix_summary": {
                                "type": "string",
                                "description": "append_round 时需要",
                            },
                        },
                        "required": ["operation"],
                    },
                },
                {
                    "name": "closeout_record_write",
                    "description": "写入并验证 closeout record。写入 artifacts/closeout/<task_id>.json 并返回验证结果。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "task_id": {
                                "type": "string",
                                "description": "task id",
                            },
                            "summary": {
                                "type": "string",
                                "description": "任务摘要",
                            },
                            "verification_status": {
                                "type": "string",
                                "enum": ["passed", "failed", "partial", "not_run"],
                                "description": "验证状态",
                            },
                            "changed_files": {
                                "type": "array",
                                "items": {"type": "string"},
                                "description": "变更的文件列表",
                            },
                            "commands_run": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "command": {"type": "string"},
                                        "exit_code": {"type": "integer"},
                                        "duration_ms": {"type": "integer"},
                                    },
                                },
                                "description": "执行的命令列表",
                            },
                            "blockers": {
                                "type": "array",
                                "items": {"type": "string"},
                            },
                            "risks": {
                                "type": "array",
                                "items": {"type": "string"},
                            },
                            "notes": {
                                "type": "string",
                            },
                        },
                        "required": ["task_id", "summary", "verification_status"],
                    },
                },
                {
                    "name": "web_fetch",
                    "description": "只读 HTTP GET 抓取外部 URL（绕过 Bash 沙箱；Desktop MCP 进程内执行）。返回 status、body 摘要。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "url": {
                                "type": "string",
                                "description": "http(s) URL",
                            },
                            "max_bytes": {
                                "type": "integer",
                                "description": "响应体最大字节数（默认 50000）",
                            },
                        },
                        "required": ["url"],
                    },
                },
                {
                    "name": "goal_state_manage",
                    "description": "管理 Goal 状态：start / checkpoint / pause / resume / complete / clear / block。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "operation": {
                                "type": "string",
                                "enum": ["start", "checkpoint", "pause", "resume", "complete", "clear", "block"],
                                "description": "操作类型",
                            },
                            "task_id": {
                                "type": "string",
                                "description": "task id，默认当前 active task",
                            },
                            "goal": {
                                "type": "string",
                                "description": "goal 内容（start 时需要）",
                            },
                            "note": {
                                "type": "string",
                                "description": "备注信息",
                            "blocker": {
                                "type": "string",
                                "description": "blocker 描述（block 时需要）",
                            },
                            },
                        },
                        "required": ["operation"],
                    },
                },
            ],
        },
    })
}

fn handle_tools_call(id: Option<Value>, request: &Value, repo_root: &Path, host_id: &str) -> Value {
    let default_params = json!({});
    let params = request.get("params").unwrap_or(&default_params);
    let tool_name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let default_args = json!({});
    let arguments = params.get("arguments").unwrap_or(&default_args);

    // Check rate limit before processing
    {
        let limiter = get_rate_limiter();
        if let Some(mut guard) = poison_safe_lock!(limiter) {
            if let Err(e) = guard.check_and_record(tool_name) {
                return json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": format!("Rate limit: {}. Consider batching operations.", e) }],
                        "isError": true,
                    },
                });
            }
        }
    }

    // Track every tool call for anomaly detection.
    if let Err(e) = record_tool_call(repo_root, tool_name) {
        eprintln!("[router-rs warning] record_tool_call failed: {e}");
    }

    // MCP hard closeout (Antigravity App + Claude Desktop; my-light skips)
    if mcp_host_supports_hard_closeout(host_id) {
        let task_id_override = arguments
            .get("task_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let task_view = resolve_task_view(repo_root, task_id_override);
        let lifecycle_profile = task_view
            .goal_state
            .as_ref()
            .and_then(|g| g.get("lifecycle_profile").and_then(Value::as_str))
            .unwrap_or("");
        let hard_block_disabled = mcp_closeout_hard_block_disabled(repo_root, lifecycle_profile);
        let host_name = mcp_host_hard_block_label(host_id);

        if !hard_block_disabled && tool_name == "goal_state_manage" {
            if let Some("complete") = arguments.get("operation").and_then(Value::as_str) {
                match evaluate_mcp_closeout_gate(arguments, repo_root, host_id) {
                    Ok(verdict) if verdict.hard_block => {
                        return json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "content": [{
                                    "type": "text",
                                    "text": format!(
                                        "Error: [{host_name} Hard Block] Cannot mark goal as complete because closeout gates are not satisfied. Detail:\n{}",
                                        verdict.formatted,
                                    )
                                }],
                                "isError": true,
                            },
                        });
                    }
                    Err(e) => {
                        return json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "content": [{ "type": "text", "text": format!("Error during pre-closeout check: {e}") }],
                                "isError": true,
                            },
                        });
                    }
                    _ => {}
                }
            }
        }
    }

    let mut result = match tool_name {
        "framework_snapshot" => tool_framework_snapshot(repo_root),
        "skill_route" => tool_skill_route(arguments, repo_root, host_id),
        "record_evidence" => tool_record_evidence(arguments, repo_root),
        "session_checkpoint" => tool_session_checkpoint(arguments, repo_root),
        "closeout_gate" => tool_closeout_gate(arguments, repo_root, host_id),
        "rfv_loop_status" => tool_rfv_loop_status(arguments, repo_root),
        "rfv_loop_manage" => tool_rfv_loop_manage(arguments, repo_root),
        "goal_state_manage" => tool_goal_state_manage(arguments, repo_root),
        "goal_state_read" => tool_goal_state_read(arguments, repo_root),
        "closeout_record_write" => tool_closeout_record_write(arguments, repo_root, host_id),
        "web_fetch" => tool_web_fetch(arguments),
        _ => Err(format!("Unknown tool: {tool_name}")),
    };

    if mcp_host_supports_hard_closeout(host_id) && tool_name == "closeout_gate" {
        if let Ok(ref content) = result {
            let task_id_override = arguments
                .get("task_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let task_view = resolve_task_view(repo_root, task_id_override);
            let lifecycle_profile = task_view
                .goal_state
                .as_ref()
                .and_then(|g| g.get("lifecycle_profile").and_then(Value::as_str))
                .unwrap_or("");
            if !mcp_closeout_hard_block_disabled(repo_root, lifecycle_profile) {
                if let Ok(verdict) = evaluate_mcp_closeout_gate(arguments, repo_root, host_id) {
                    if verdict.hard_block {
                        let host_name = mcp_host_hard_block_label(host_id);
                        result = Err(format!(
                            "[{host_name} Hard Block] Closeout Gate not satisfied. Details:\n{content}",
                        ));
                    }
                }
            }
        }
    }

    match result {
        Ok(content) => {
            // Check for anomalies and append warnings if detected
            let warnings = check_anomalies(repo_root).unwrap_or_default();

            let final_content = if warnings.is_empty() {
                content
            } else {
                let warning_text = warnings.join("; ");
                format!("{}\n\n[Session Warning] {}", content, warning_text)
            };

            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{ "type": "text", "text": final_content }],
                },
            })
        }
        Err(err) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{ "type": "text", "text": format!("Error: {err}") }],
                "isError": true,
            },
        }),
    }
}

fn tool_framework_snapshot(repo_root: &Path) -> Result<String, String> {
    let ttl_secs = snapshot_cache_ttl_secs();
    // Try to read from cache (configurable TTL, default 30 seconds)
    {
        let cache = get_snapshot_cache();
        if let Some(guard) = poison_safe_lock!(cache) {
            if let Some(ref cached) = *guard {
                if cached.is_valid() {
                    return Ok(cached.content.clone());
                }
            }
        }
    }

    // Cache miss: recompute
    let envelope = build_framework_runtime_snapshot_envelope(repo_root, None, None)?;
    let content = serde_json::to_string_pretty(&envelope).map_err(|e| e.to_string())?;

    // Update cache with configurable TTL
    {
        let cache = get_snapshot_cache();
        if let Some(mut guard) = poison_safe_lock!(cache) {
            *guard = Some(SnapshotCache {
                content: content.clone(),
                expires_at: Instant::now() + Duration::from_secs(ttl_secs),
            });
        }
    }

    Ok(content)
}

/// Invalidate evidence-dependent caches (snapshot / task view).
fn invalidate_evidence_caches() {
    // Clear snapshot cache
    if let Some(mut guard) = poison_safe_lock!(get_snapshot_cache()) {
        *guard = None;
    }
    // Clear task view cache
    if let Some(mut guard) = poison_safe_lock!(get_task_view_cache()) {
        *guard = None;
    }
}

fn tool_skill_route(arguments: &Value, repo_root: &Path, host_id: &str) -> Result<String, String> {
    let query = arguments
        .get("query")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: query")?;

    // Dynamically determine first_turn: true only if no routing tools have been called yet.
    // This prevents stale routing behavior on subsequent calls within the same session.
    let first_turn = read_tracker_state(repo_root)
        .map(|state| {
            let per_tool = state.get("per_tool").and_then(|v| v.as_object());
            let has_routing = per_tool
                .map(|m| m.contains_key("skill_route"))
                .unwrap_or(false);
            !has_routing
        })
        .unwrap_or(true); // Default to first_turn=true on error

    let runtime_path = skill_routing_runtime_json(repo_root);
    let records = load_records_cached_for_stdio(Some(&runtime_path), None)?;
    let records = filter_records_for_host(records.as_ref(), Some(host_id))?;
    let decision = route_task_with_manifest_fallback(
        &records,
        Some(repo_root),
        None,
        Some(host_id),
        query,
        "session",
        true, // allow_overlay: true
        first_turn,
    )?;
    if decision.selected_skill == "none" || decision.selected_skill.is_empty() {
        return Ok(json!({
            "routed": false,
            "skill_slug": null,
            "skill_path": null,
            "match_reason": "no match",
        })
        .to_string());
    }
    Ok(json!({
        "routed": true,
        "skill_slug": decision.selected_skill,
        "skill_path": decision.selected_skill_path,
        "match_reason": decision.reasons.get(0).cloned().unwrap_or_default(),
    })
    .to_string())
}

pub(crate) fn build_evidence_entry(arguments: &Value) -> Result<Map<String, Value>, String> {
    let tool_name = arguments
        .get("tool_name")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: tool_name")?;
    let command = arguments
        .get("command")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: command")?;
    let exit_code = arguments.get("exit_code").and_then(Value::as_i64);
    let output = arguments.get("output").and_then(Value::as_str);

    let mut entry = Map::new();
    entry.insert("kind".to_string(), json!("mcp_record_evidence"));
    entry.insert("source".to_string(), json!("mcp_record_evidence"));
    entry.insert("tool_name".to_string(), json!(tool_name));
    entry.insert("command_preview".to_string(), json!(command));
    entry.insert(
        "recorded_at".to_string(),
        json!(crate::framework_runtime::current_local_timestamp()),
    );
    if let Some(ec) = exit_code {
        entry.insert("exit_code".to_string(), json!(ec));
        entry.insert("success".to_string(), json!(ec == 0));
    }
    if let Some(text) = output {
        let max_chars = evidence_output_max_chars();
        let trimmed: String = text.chars().take(max_chars).collect();
        entry.insert("output".to_string(), json!(trimmed));
    }
    Ok(entry)
}

fn tool_record_evidence(arguments: &Value, repo_root: &Path) -> Result<String, String> {
    let entry = build_evidence_entry(arguments)?;
    let tool_name = entry
        .get("tool_name")
        .and_then(Value::as_str)
        .map(str::to_string);
    let tool_name_display = tool_name.as_deref().unwrap_or("");
    let command = entry
        .get("command_preview")
        .and_then(Value::as_str)
        .map(str::to_string);
    let command_display = command.as_deref().unwrap_or("");
    let exit_code = arguments.get("exit_code").and_then(Value::as_i64);
    let output = arguments.get("output").and_then(Value::as_str);

    crate::framework_runtime::append_evidence_index_merged_row(repo_root, None, entry)?;

    // H2 FIX: Invalidate caches after evidence is written to ensure fresh data on next read
    invalidate_evidence_caches();

    let exit_display = exit_code
        .map(|ec| ec.to_string())
        .unwrap_or_else(|| "null".to_string());
    let honor_note = " (honor-system: not bound to host tool execution — verify independently)";
    if let Some(text) = output {
        let max_chars = evidence_output_max_chars();
        let trimmed = text.chars().take(max_chars).collect::<String>();
        Ok(format!(
            "Evidence recorded{honor_note}: {tool_name_display} '{command_display}' -> exit={exit_display}\n{trimmed}"
        ))
    } else {
        Ok(format!(
            "Evidence recorded{honor_note}: {tool_name_display} '{command_display}' -> exit={exit_display}"
        ))
    }
}

/// 获取 evidence output 的最大字符数配置。
/// 默认 2000 字符，可通过 `ROUTER_RS_EVIDENCE_OUTPUT_MAX_CHARS` 环境变量覆盖。
fn evidence_output_max_chars() -> usize {
    static CACHED: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("ROUTER_RS_EVIDENCE_OUTPUT_MAX_CHARS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(2000)
    })
}

fn tool_session_checkpoint(arguments: &Value, repo_root: &Path) -> Result<String, String> {
    let summary = arguments
        .get("summary")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: summary")?;
    let next_actions: Vec<String> = arguments
        .get("next_actions")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let task_id = arguments.get("task_id").and_then(Value::as_str);

    let payload = build_automatic_continuity_checkpoint_payload_with_task_id(
        repo_root,
        summary,
        &next_actions.join(", "),
        task_id,
        true,
        false,
    );
    crate::framework_runtime::write_framework_session_artifacts(payload)
        .map_err(|e| format!("Checkpoint write failed: {e}"))?;

    // H2 FIX: Invalidate caches after checkpoint is written to ensure fresh data on next read
    invalidate_evidence_caches();

    Ok(format!(
        "Checkpoint written: summary={}, next_actions_count={}",
        summary.chars().count(),
        next_actions.len()
    ))
}

fn goal_suggests_review_work(goal_state: &Value) -> bool {
    if goal_state
        .get("goal")
        .and_then(Value::as_str)
        .is_some_and(is_review_prompt)
    {
        return true;
    }
    goal_state
        .get("done_when")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().filter_map(Value::as_str).any(is_review_prompt)
        })
}

fn desktop_review_evidence_attested(arguments: &Value, repo_root: &Path, task_id: &str) -> bool {
    // 自动扫描 artifacts/current/<task_id>/review-lanes 目录下的 Markdown 证据工件
    let review_lanes_dir = task_artifact_dir(
        repo_root,
        if task_id.is_empty() {
            None
        } else {
            Some(task_id)
        },
    )
    .join("review-lanes");

    if review_lanes_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&review_lanes_dir) {
            let mut valid_findings_found = false;
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if !content.trim().is_empty() {
                            valid_findings_found = true;
                            break;
                        }
                    }
                }
            }
            if valid_findings_found {
                return true;
            }
        }
    }

    let lane = arguments
        .get("reviewer_lane")
        .or_else(|| arguments.get("subagent_type"))
        .or_else(|| arguments.get("agent_type"))
        .and_then(Value::as_str);
    let Some(lane) = lane else {
        return false;
    };
    let review_lane =
        crate::runtime_registry::is_claude_reviewer_lane_from_registry(lane, Some(repo_root));
    let fork = fork_context_from_values(arguments, None);
    claude_independent_reviewer_evidence(review_lane, fork)
}

#[derive(Debug, Clone)]
pub(crate) struct McpCloseoutGateVerdict {
    pub all_clear: bool,
    pub checkpoint_only: bool,
    pub hard_block: bool,
    pub formatted: String,
}

fn mcp_closeout_host_label(host_id: &str) -> &'static str {
    mcp_host_display_label(host_id)
}

fn mcp_closeout_hard_block_disabled(repo_root: &Path, lifecycle_profile: &str) -> bool {
    crate::runtime_registry::lifecycle_profile_disables_review_gate_hard_block(
        Some(repo_root),
        lifecycle_profile,
    )
    .unwrap_or(false)
}

pub(crate) fn evaluate_mcp_closeout_gate(
    arguments: &Value,
    repo_root: &Path,
    host_id: &str,
) -> Result<McpCloseoutGateVerdict, String> {
    let task_id_override = arguments
        .get("task_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let task_view = resolve_task_view(repo_root, task_id_override);
    let mut findings: Vec<String> = Vec::new();
    let host_name = mcp_closeout_host_label(host_id);

    findings.push(format!(
        "review_gate: {host_name} has no hook REVIEW_GATE — reviewer evidence is honor-system / self-attested (prompts/review_gate)"
    ));

    let goal_present = task_view.goal_state.is_some();
    if !goal_present {
        findings.push("goal_state: no GOAL_STATE.json".to_string());
    } else {
        findings.push("goal_state: present".to_string());
    }

    let evidence_success = task_view
        .evidence
        .as_ref()
        .map(|e| e.has_successful_verification)
        .unwrap_or(false);
    let task_id = task_view.task_id.as_deref().unwrap_or("");

    if !evidence_success {
        findings.push("evidence: no successful EVIDENCE_INDEX records".to_string());
    } else {
        findings.push("evidence: successful records present".to_string());
        if !task_id.is_empty()
            && crate::autopilot_goal::task_evidence_success_only_self_attested(repo_root, task_id)
        {
            findings.push(
                "WARN: evidence: only self-attested MCP record_evidence rows — verify independently"
                    .to_string(),
            );
        }
    }
    let summary_path = task_artifact_dir(repo_root, if task_id.is_empty() { None } else { Some(task_id) })
        .join("SESSION_SUMMARY.md");
    let has_summary = summary_path.is_file();
    if !has_summary {
        findings.push(format!(
            "checkpoint: missing SESSION_SUMMARY at {}",
            summary_path.display()
        ));
    } else {
        findings.push("checkpoint: SESSION_SUMMARY.md on disk".to_string());
    }

    let lifecycle_profile = task_view
        .goal_state
        .as_ref()
        .and_then(|g| g.get("lifecycle_profile").and_then(Value::as_str))
        .unwrap_or("");
    let hard_block_disabled = mcp_closeout_hard_block_disabled(repo_root, lifecycle_profile);

    let review_goal = task_view
        .goal_state
        .as_ref()
        .is_some_and(goal_suggests_review_work);
    let has_review_evidence = desktop_review_evidence_attested(arguments, repo_root, task_id);

    if review_goal && !has_review_evidence {
        findings.push(format!(
            "WARN: review_gate: GOAL suggests review work but no hook-level reviewer evidence on {host_name} — spawn claude_reviewer_lanes with fork_context=false and write review-lanes/*.md, or pass reviewer_lane + fork_context=false in closeout_gate args"
        ));
    } else if review_goal {
        findings.push(
            "review_gate: GOAL suggests review; reviewer evidence attested in closeout_gate args or review-lanes"
                .to_string(),
        );
    }

    let mut all_clear = goal_present && evidence_success && has_summary;
    if review_goal && !has_review_evidence {
        all_clear = false;
    }

    let checkpoint_only =
        !all_clear && goal_present && evidence_success && (!review_goal || has_review_evidence);

    let verdict_label = if all_clear {
        "PASS: all closeout gates satisfied"
    } else if hard_block_disabled {
        if checkpoint_only {
            "ADVISORY: checkpoint missing — call session_checkpoint before complete (my-light: MCP hard block disabled)"
        } else {
            "ADVISORY: closeout gates not satisfied (my-light: MCP hard block disabled)"
        }
    } else if checkpoint_only {
        "BLOCKED: checkpoint missing — call session_checkpoint before complete (MCP hard block when not my-light)"
    } else {
        "BLOCKED: closeout gates not satisfied (MCP hard block when not my-light)"
    };

    let formatted = format!(
        "[Closeout Gate] {verdict_label}\n\n{}",
        findings.join("\n")
    );

    Ok(McpCloseoutGateVerdict {
        all_clear,
        checkpoint_only,
        hard_block: !all_clear && !hard_block_disabled,
        formatted,
    })
}

pub(crate) fn tool_closeout_gate(arguments: &Value, repo_root: &Path, host_id: &str) -> Result<String, String> {
    Ok(evaluate_mcp_closeout_gate(arguments, repo_root, host_id)?.formatted)
}

fn tool_closeout_record_write(arguments: &Value, repo_root: &Path, host_id: &str) -> Result<String, String> {
    let task_id = arguments
        .get("task_id")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: task_id")?;
    let summary = arguments
        .get("summary")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: summary")?;
    let verification_status = arguments
        .get("verification_status")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: verification_status")?;

    let mut record = Map::new();
    record.insert(
        "schema_version".to_string(),
        json!(crate::closeout_enforcement::CLOSEOUT_RECORD_SCHEMA_VERSION),
    );
    record.insert("task_id".to_string(), json!(task_id));
    record.insert(
        "ended_at".to_string(),
        json!(crate::framework_runtime::current_local_timestamp()),
    );
    record.insert("summary".to_string(), json!(summary));
    record.insert(
        "verification_status".to_string(),
        json!(verification_status),
    );

    if let Some(files) = arguments.get("changed_files").and_then(Value::as_array) {
        record.insert("changed_files".to_string(), json!(files));
    }
    if let Some(cmds) = arguments.get("commands_run").and_then(Value::as_array) {
        record.insert("commands_run".to_string(), json!(cmds));
    }
    if let Some(blockers) = arguments.get("blockers").and_then(Value::as_array) {
        if !blockers.is_empty() {
            record.insert("blockers".to_string(), json!(blockers));
        }
    }
    if let Some(risks) = arguments.get("risks").and_then(Value::as_array) {
        if !risks.is_empty() {
            record.insert("risks".to_string(), json!(risks));
        }
    }
    if let Some(notes) = arguments.get("notes").and_then(Value::as_str) {
        if !notes.is_empty() {
            record.insert("notes".to_string(), json!(notes));
        }
    }

    // Ensure parent directory exists
    let record_path = crate::framework_runtime::closeout_record_path_for_task(repo_root, task_id)
        .map_err(|e| format!("invalid task_id: {e}"))?;
    if let Some(parent) = record_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create closeout directory failed: {e}"))?;
    }

    // Write the record
    let content = serde_json::to_string_pretty(&record).map_err(|e| format!("serialize closeout record failed: {e}"))?;
    fs::write(&record_path, &content).map_err(|e| format!("write closeout record failed: {e}"))?;

    // Evaluate the record
    let eval_result = crate::framework_runtime::evaluate_closeout_record_file_for_task(
        repo_root,
        task_id,
        &record_path,
    );
    let eval = match eval_result {
        Ok(v) => v,
        Err(e) => json!({"error": e}),
    };

    let closeout_allowed = eval
        .get("closeout_allowed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let violations: Vec<String> = eval
        .get("violations")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|v| {
                    let rule = v.get("rule").and_then(Value::as_str).unwrap_or("unknown");
                    let detail = v
                        .get("detail")
                        .and_then(Value::as_str)
                        .unwrap_or("no detail");
                    format!("[{rule}] {detail}")
                })
                .collect()
        })
        .unwrap_or_default();

    let mut result = json!({
        "closeout_allowed": closeout_allowed,
        "record_path": record_path.to_string_lossy().to_string(),
        "violations": violations,
    });

    if let Ok(mcp_verdict) = evaluate_mcp_closeout_gate(
        &json!({ "task_id": task_id }),
        repo_root,
        host_id,
    ) {
        let task_view = crate::task_state::resolve_task_view(repo_root, Some(task_id));
        let lifecycle_profile = task_view
            .goal_state
            .as_ref()
            .and_then(|g| g.get("lifecycle_profile").and_then(Value::as_str))
            .unwrap_or("");
        let hard_block_disabled = mcp_closeout_hard_block_disabled(repo_root, lifecycle_profile);
        if let Some(obj) = result.as_object_mut() {
            obj.insert(
                "mcp_closeout_gate".to_string(),
                json!({
                    "all_clear": mcp_verdict.all_clear,
                    "checkpoint_only": mcp_verdict.checkpoint_only,
                    "hard_block": mcp_verdict.hard_block && !hard_block_disabled,
                }),
            );
        }
    }

    Ok(serde_json::to_string_pretty(&result).map_err(|e| format!("serialize closeout result failed: {e}"))?)
}

const WEB_FETCH_MAX_REDIRECTS: usize = 5;

fn tool_web_fetch(arguments: &Value) -> Result<String, String> {
    let url = arguments
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or("Missing required argument: url")?;
    // Validate + resolve DNS in one pass to pin results before building client.
    let (parsed_url, initial_addrs) = crate::web_fetch_guard::validate_and_resolve_web_fetch_url(url)?;
    let max_bytes = arguments
        .get("max_bytes")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(WEB_FETCH_MAX_BYTES_DEFAULT)
        .clamp(1, WEB_FETCH_MAX_BYTES_DEFAULT);
    let mut client_builder = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(WEB_FETCH_TIMEOUT_SECS))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("router-rs-framework-mcp/0.1");
    for key in ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy", "ALL_PROXY"] {
        if let Ok(proxy_url) = std::env::var(key) {
            let trimmed = proxy_url.trim();
            if !trimmed.is_empty() {
                if let Ok(proxy) = reqwest::Proxy::all(trimmed) {
                    client_builder = client_builder.proxy(proxy);
                    break;
                }
            }
        }
    }
    // Pin DNS results from validate_and_resolve to prevent rebinding TOCTOU.
    let pin_host = parsed_url.host_str()
        .ok_or_else(|| format!("web_fetch URL missing host: {url}"))?;
    for addr in &initial_addrs {
        client_builder = client_builder.resolve(pin_host, *addr);
    }
    let mut client = client_builder
        .build()
        .map_err(|err| format!("web_fetch client build failed: {err}"))?;
    let mut current_url = url.to_string();
    let mut response = None;
    for hop in 0..=WEB_FETCH_MAX_REDIRECTS {
        let resp = client
            .get(&current_url)
            .send()
            .map_err(|err| format!("web_fetch request failed: {err}"))?;
        if resp.status().is_redirection() {
            if hop >= WEB_FETCH_MAX_REDIRECTS {
                return Err(format!(
                    "web_fetch exceeded {WEB_FETCH_MAX_REDIRECTS} redirects"
                ));
            }
            let location = resp
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| {
                    format!(
                        "web_fetch redirect missing Location header (status {})",
                        resp.status()
                    )
                })?;
            let base = reqwest::Url::parse(&current_url)
                .map_err(|err| format!("web_fetch redirect base URL invalid: {err}"))?;
            current_url = crate::web_fetch_guard::resolve_web_fetch_redirect(&base, location)?;
            // Pin DNS for redirect target to prevent DNS rebinding TOCTOU.
            let redirect_parsed = reqwest::Url::parse(&current_url)
                .map_err(|err| format!("web_fetch redirect URL parse failed: {err}"))?;
            let rp_host = redirect_parsed.host_str()
                .ok_or_else(|| format!("web_fetch redirect URL missing host: {current_url}"))?;
            let rp_port = redirect_parsed.port()
                .unwrap_or(if redirect_parsed.scheme() == "https" { 443 } else { 80 });
            let rp_addrs = crate::web_fetch_guard::resolve_web_fetch_addresses(rp_host, rp_port)?;
            // Rebuild client with pinned DNS for redirect target.
            let mut rb = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(WEB_FETCH_TIMEOUT_SECS))
                .redirect(reqwest::redirect::Policy::none())
                .user_agent("router-rs-framework-mcp/0.1");
            for addr in &rp_addrs {
                rb = rb.resolve(rp_host, *addr);
            }
            client = rb.build()
                .map_err(|err| format!("web_fetch client rebuild failed: {err}"))?;
            continue;
        }
        response = Some(resp);
        break;
    }
    let response = response.ok_or_else(|| "web_fetch: no response".to_string())?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = response
        .bytes()
        .map_err(|err| format!("web_fetch read body failed: {err}"))?;
    let truncated = body.len() > max_bytes;
    let slice = &body[..body.len().min(max_bytes)];
    let body_text = String::from_utf8_lossy(slice).into_owned();
    let payload = json!({
        "url": current_url,
        "status": status,
        "content_type": content_type,
        "content_length": body.len(),
        "truncated": truncated,
        "body": body_text,
    });
    serde_json::to_string_pretty(&payload).map_err(|err| format!("web_fetch serialize failed: {err}"))
}

fn tool_goal_state_read(arguments: &Value, repo_root: &Path) -> Result<String, String> {
    let task_id = arguments.get("task_id").and_then(Value::as_str);
    let state = crate::autopilot_goal::read_goal_state(repo_root, task_id);
    serde_json::to_string_pretty(&state).map_err(|e| e.to_string())
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
            ],
        },
    })
}

fn handle_prompts_get(id: Option<Value>, request: &Value, repo_root: &Path, host_id: &str) -> Value {
    let default_params = json!({});
    let params = request.get("params").unwrap_or(&default_params);
    let prompt_name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let default_args = json!({});
    let _arguments = params.get("arguments").unwrap_or(&default_args);

    let description = match prompt_name {
        "framework_routing" => "framework routing",
        "review_gate" => "review gate advisory",
        "closeout_checklist" => "closeout checklist",
        _ => "",
    };

    let text = match prompt_name {
        "framework_routing" => {
            let source_rel = "skills/SKILL_ROUTING_RUNTIME.json";
            format!(
                "面向用户的回复必须使用简体中文（代码/路径/命令/第三方原文除外）。\n\n\
                 Use this repo shared framework runtime.\n\n\
                 1) Start from AGENTS.md.\n\
                 2) Route via {source_rel}.\n\
                 3) Read only the matched skill_path.\n\n\
                 Framework root: core/router-rs/"
            )
        }
        "review_gate" => {
            let host_name = mcp_host_display_label(host_id);
            let gate_mode = if mcp_host_supports_hard_closeout(host_id) {
                format!(
                    "{host_name} closeout is hard-blocked at MCP tool level for non-my-light profiles — unsatisfied closeout blocks goal_state_manage complete."
                )
            } else {
                "Desktop review gate is advisory only — MCP cannot hard-block Stop.".to_string()
            };
            {
                let lanes = crate::runtime_registry::claude_reviewer_lanes_sorted(Some(repo_root));
                let lane_lines = if lanes.is_empty() {
                    "- (registry claude_reviewer_lanes unavailable)\n".to_string()
                } else {
                    lanes
                        .iter()
                        .map(|lane| format!("- {lane}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                format!(
                    "[Review Gate -- {host_name} gating]\n\n\
                     This host uses MCP transport; there is no shell hook REVIEW_GATE observation.\n\n\
                     Countable independent reviewer lanes (RUNTIME_REGISTRY review_gate.claude_reviewer_lanes):\n\
                     {lane_lines}\n\
                     explore / explorer does NOT count toward review evidence.\n\
                     Requires fork_context=false for independent reviewer credit (review-lanes/*.md on disk).\n\n\
                     When user requests review:\n\
                     1) Spawn a read-only reviewer in a claude_reviewer_lanes lane with fork_context=false\n\
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
    let task_view = get_cached_task_view(repo_root);

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
            let task_view = get_cached_task_view(repo_root);
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
            let state = crate::autopilot_goal::read_goal_state(repo_root, None);
            (
                serde_json::to_string_pretty(&state).unwrap_or_default(),
                "application/json",
            )
        }
        "framework://evidence_index" => {
            let task_view = get_cached_task_view(repo_root);
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
            let task_view = get_cached_task_view(repo_root);
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

fn tool_rfv_loop_status(arguments: &Value, repo_root: &Path) -> Result<String, String> {
    let task_id = arguments.get("task_id").and_then(Value::as_str);
    let state = crate::rfv_loop::read_rfv_loop_state(repo_root, task_id)?;
    serde_json::to_string_pretty(&state).map_err(|e| e.to_string())
}

fn parse_rfv_round_argument(value: Option<&Value>) -> Result<u64, String> {
    let Some(v) = value else {
        return Err("append_round requires 'round' argument (integer)".to_string());
    };
    if let Some(n) = v.as_u64() {
        return Ok(n);
    }
    if let Some(n) = v.as_i64() {
        if n >= 0 {
            return Ok(n as u64);
        }
    }
    Err("append_round requires 'round' argument (integer)".to_string())
}

fn tool_rfv_loop_manage(arguments: &Value, repo_root: &Path) -> Result<String, String> {
    let operation = arguments
        .get("operation")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: operation (string)")?;
    let task_id = arguments.get("task_id").and_then(Value::as_str);

    // repo_root is a &Path, convert to string for the payload
    let repo_root_str = repo_root.to_string_lossy().to_string();

    let mut payload = json!({
        "repo_root": repo_root_str,
        "operation": operation,
    });
    if let Some(tid) = task_id {
        payload["task_id"] = json!(tid);
    }

    // Per-operation required fields
    match operation {
        "start" => {
            let goal = arguments
                .get("goal")
                .and_then(Value::as_str)
                .ok_or("start requires 'goal' argument (string)")?;
            payload["goal"] = json!(goal);
            if let Some(mr) = arguments.get("max_rounds").and_then(Value::as_u64) {
                payload["max_rounds"] = json!(mr);
            }
            if let Some(er) = arguments
                .get("allow_external_research")
                .and_then(Value::as_bool)
            {
                payload["allow_external_research"] = json!(er);
            }
        }
        "append_round" => {
            let round = parse_rfv_round_argument(arguments.get("round"))?;
            payload["round"] = json!(round);

            // Validate required string arguments with specific error messages
            let review_summary = arguments
                .get("review_summary")
                .and_then(Value::as_str)
                .ok_or("append_round requires 'review_summary' argument (string)")?;
            payload["review_summary"] = json!(review_summary);

            let fix_summary = arguments
                .get("fix_summary")
                .and_then(Value::as_str)
                .ok_or("append_round requires 'fix_summary' argument (string)")?;
            payload["fix_summary"] = json!(fix_summary);

            let verify_result = arguments
                .get("verify_result")
                .and_then(Value::as_str)
                .ok_or("append_round requires 'verify_result' argument (string)")?;
            payload["verify_result"] = json!(verify_result);

            let supervisor_decision = arguments
                .get("supervisor_decision")
                .and_then(Value::as_str)
                .ok_or("append_round requires 'supervisor_decision' argument (string)")?;
            payload["supervisor_decision"] = json!(supervisor_decision);

            let reason = arguments
                .get("reason")
                .and_then(Value::as_str)
                .ok_or("append_round requires 'reason' argument (string)")?;
            payload["reason"] = json!(reason);
        }
        _ => {
            return Err(format!(
                "Unknown RFV loop operation: {operation}. Valid operations: start, append_round"
            ))
        }
    }

    let result = crate::rfv_loop::framework_rfv_loop(payload)?;
    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
}

fn tool_goal_state_manage(arguments: &Value, repo_root: &Path) -> Result<String, String> {
    let operation = arguments
        .get("operation")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: operation")?;
    let task_id = arguments.get("task_id").and_then(Value::as_str);

    let repo_root_str = repo_root.to_string_lossy().to_string();

    let mut payload = json!({
        "repo_root": repo_root_str,
        "operation": operation,
    });
    if let Some(tid) = task_id {
        payload["task_id"] = json!(tid);
    }

    match operation {
        "start" => {
            let goal = arguments
                .get("goal")
                .and_then(Value::as_str)
                .ok_or("start requires 'goal' argument (string)")?;
            payload["goal"] = json!(goal);
            if let Some(ng) = arguments.get("non_goals").and_then(Value::as_array) {
                payload["non_goals"] = json!(ng);
            }
            if let Some(dw) = arguments.get("done_when").and_then(Value::as_array) {
                payload["done_when"] = json!(dw);
            }
            if let Some(vc) = arguments
                .get("validation_commands")
                .and_then(Value::as_array)
            {
                payload["validation_commands"] = json!(vc);
            }
            if let Some(dud) = arguments.get("drive_until_done").and_then(Value::as_bool) {
                payload["drive_until_done"] = json!(dud);
            }
        }
        "checkpoint" => {
            let note = arguments
                .get("note")
                .and_then(Value::as_str)
                .ok_or("checkpoint requires 'note' argument (string)")?;
            payload["note"] = json!(note);
        }
        "block" => {
            let blocker = arguments
                .get("blocker")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .ok_or("block requires 'blocker' argument (string)")?;
            payload["blocker"] = json!(blocker);
        }
        "append_round" => {
            // append_round is handled in rfv_loop, not here
            return Err(
                "append_round is not a valid goal_state_manage operation. \
                 Use rfv_loop_manage with operation=append_round instead.".to_string(),
            );
        }
        "pause" | "resume" | "complete" | "clear" => {
            // No additional required args
        }
        _ => return Err(format!("Unknown goal operation: {operation}. Valid operations: start, checkpoint, pause, resume, complete, clear, block")),
    }

    let result = crate::autopilot_goal::framework_goal_drive(payload)?;

    // Invalidate snapshot/task_view caches after goal state write (H3 FIX)
    let op = arguments.get("operation").and_then(|v| v.as_str()).unwrap_or("");
    if op != "status" {
        invalidate_evidence_caches();
    }
    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
}

// =============================================================================
// Test helper functions (used by integration tests in claude_desktop_hooks_tests.rs)
// =============================================================================

#[cfg(test)]
pub(crate) fn tool_goal_state_manage_test_helper(
    arguments: &Value,
    operation: &str,
) -> Result<String, String> {
    let path = crate::claude_desktop_test_support::unique_temp_repo("goal-manage");
    let _ = std::fs::create_dir_all(&path);

    let mut args_with_op = arguments.clone();
    args_with_op["operation"] = json!(operation);

    let result = tool_goal_state_manage(&args_with_op, &path);
    let _ = std::fs::remove_dir_all(&path);
    result
}

#[cfg(test)]
pub(crate) fn tool_closeout_record_write_for_test(
    arguments: &Value,
    repo_path: &Path,
) -> Result<String, String> {
    tool_closeout_record_write(arguments, repo_path, "claude-desktop")
}

#[cfg(test)]
pub(crate) fn tool_rfv_loop_manage_test_helper(
    arguments: &Value,
    operation: &str,
) -> Result<String, String> {
    let path = crate::claude_desktop_test_support::unique_temp_repo("rfv-manage");
    let _ = std::fs::create_dir_all(&path);

    let mut args_with_op = arguments.clone();
    args_with_op["operation"] = json!(operation);

    let result = tool_rfv_loop_manage(&args_with_op, &path);
    let _ = std::fs::remove_dir_all(&path);
    result
}

#[cfg(test)]
pub(crate) fn get_snapshot_ttl_for_test() -> u64 {
    snapshot_cache_ttl_secs()
}

#[cfg(test)]
pub(crate) fn get_task_view_ttl_for_test() -> u64 {
    task_view_cache_ttl_secs()
}

#[cfg(test)]
pub(crate) fn read_mcp_message_test_helper<R: std::io::BufRead>(
    input: &mut R,
    transport_mode: &mut Option<McpTransportMode>,
) -> Result<Option<String>, String> {
    read_mcp_message(input, transport_mode)
}

#[cfg(test)]
pub(crate) fn init_tracker_for_test(path: &std::path::Path) -> Result<(), String> {
    crate::session_call_tracker::init_tracker(path)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn unique_test_repo(name: &str) -> PathBuf {
        let path = crate::claude_desktop_test_support::unique_temp_repo(&format!("mcp-{name}"));
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
        assert_eq!(names.len(), 11, "expected 11 tools, got: {:?}", names);
        for tool in &[
            "framework_snapshot",
            "skill_route",
            "record_evidence",
            "session_checkpoint",
            "closeout_gate",
            "closeout_record_write",
            "web_fetch",
            "goal_state_read",
            "rfv_loop_status",
            "rfv_loop_manage",
            "goal_state_manage",
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
            "claude-desktop",
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
            "claude-desktop",
        )
        .unwrap();
        assert_eq!(response["error"]["code"], -32601);
        assert!(response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("nonexistent"));
    }
}
