//! MCP stdio harness: Opencode 共用 stdio transport。
//!
//! MCP 服务器（stdio transport），提供 tools / prompts / resources 三类端点，
//! 替代 shell hook 协议（PreToolUse / UserPromptSubmit / PostToolUse / Stop）。
//!
//! 架构约束：MCP 不支持工具拦截，PreToolUse guards 不可用；closeout / review 门控在 MCP 工具层
//! 报告 findings。`RUNTIME_REGISTRY.host_projections.*.harness_capability_exceptions` 为叙事真源
//! （`closeout_evidence_hooks=unsupported` → 非 my-light 时 MCP 层 hard_block 元数据；my-light advisory）。
//!
//! 与 CLI 共享 L2/L3 手动画板（evidence、goal state、路由、snapshot），出站为 MCP JSON-RPC。

// route_task_with_manifest_fallback — not needed in host-projection; skill routing via framework_kernel
// framework_runtime functions accessed via crate::hooks
use crate::hooks::{check_anomalies, init_tracker, read_tracker_state, record_tool_call};
use core_policy::hook_common::is_review_prompt;
use core_policy::review_gate_engine::{
    fork_context_from_values, review_independent_reviewer_evidence,
};
use core_state::task_state::resolve_task_view;
use routing_engine::route::filter_records_for_host;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

const WEB_FETCH_MAX_BYTES_DEFAULT: usize = 50_000;
const WEB_FETCH_TIMEOUT_SECS: u64 = 30;

/// Shared host display label for MCP-hosted sessions.
/// Used by closeout hard-block and advisory review gate prompts (review never hard-blocks Stop).
/// Resolved from HostProvider registry; falls back to "MCP Host" for unknown hosts.
fn mcp_host_display_label(host_id: &str) -> String {
    crate::hosts::host_provider_for_id(host_id)
        .map(|p| {
            // Capitalize first letter of host_id as display name
            let id = p.host_id();
            let mut chars = id.chars();
            match chars.next() {
                Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
                None => "MCP Host".to_string(),
            }
        })
        .unwrap_or_else(|| "MCP Host".to_string())
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
        match core_state::utils::path_guard::validate_task_id_component(task_id.trim()) {
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

    pub fn check_and_record(&mut self, tool_name: &str) -> Result<(), String> {
        let now = Instant::now();
        if let Some(last) = self.last_call.get(tool_name)
            && now.duration_since(*last) < self.min_interval {
                return Err(format!(
                    "Rate limit exceeded for {}. Wait {}ms between calls.",
                    tool_name,
                    self.min_interval.as_millis()
                ));
            }
        self.last_call.insert(tool_name.to_string(), now);
        Ok(())
    }
}

// Global caches and rate limiter (session-scoped via OnceLock)
// Caches use RwLock for concurrent reads; rate limiter uses Mutex (read+write in one call).

/// Task view cache entry: (resolved_view, last_access_instant).
type TaskViewCacheEntry = (core_state::task_state::ResolvedTaskView, Instant);

/// Task view cache: path → (resolved_view, last_access).
type TaskViewCache = HashMap<PathBuf, TaskViewCacheEntry>;

static SNAPSHOT_CACHE: OnceLock<Arc<std::sync::RwLock<Option<SnapshotCache>>>> = OnceLock::new();
static TASK_VIEW_CACHE: OnceLock<Arc<std::sync::RwLock<TaskViewCache>>> = OnceLock::new();
static RATE_LIMITER: OnceLock<Arc<std::sync::Mutex<RateLimiter>>> = OnceLock::new();

/// Poison-safe lock helpers that recover from lock poisoning.
macro_rules! poison_safe_read_lock {
    ($lock:expr) => {{
        match $lock.read() {
            Ok(guard) => Some(guard),
            Err(poisoned) => {
                eprintln!("[router-rs warning] rwlock poisoned (read), recovering");
                Some(poisoned.into_inner())
            }
        }
    }};
}

macro_rules! poison_safe_write_lock {
    ($lock:expr) => {{
        match $lock.write() {
            Ok(guard) => Some(guard),
            Err(poisoned) => {
                eprintln!("[router-rs warning] rwlock poisoned (write), recovering");
                Some(poisoned.into_inner())
            }
        }
    }};
}

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

mod tools;
use tools::*;
#[cfg(any(test, feature = "test-support"))]
pub use tools::{build_evidence_entry, tool_closeout_gate};
fn get_snapshot_cache() -> &'static Arc<std::sync::RwLock<Option<SnapshotCache>>> {
    SNAPSHOT_CACHE.get_or_init(|| Arc::new(std::sync::RwLock::new(None)))
}

fn get_task_view_cache() -> &'static Arc<
    std::sync::RwLock<HashMap<PathBuf, (core_state::task_state::ResolvedTaskView, Instant)>>,
> {
    TASK_VIEW_CACHE.get_or_init(|| Arc::new(std::sync::RwLock::new(HashMap::new())))
}

fn get_rate_limiter() -> &'static Arc<std::sync::Mutex<RateLimiter>> {
    RATE_LIMITER.get_or_init(|| {
        // In test mode or when test-support feature is enabled, disable rate limiting
        let interval = if cfg!(test) || cfg!(feature = "test-support") {
            0
        } else {
            100
        };
        Arc::new(std::sync::Mutex::new(RateLimiter::new(interval)))
    })
}

/// Reset rate limiter state for integration tests (clears all recorded timestamps).
pub fn reset_rate_limiter_for_test() {
    if let Some(limiter) = RATE_LIMITER.get()
        && let Ok(mut guard) = limiter.lock() {
            guard.last_call.clear();
        }
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

/// Maximum number of entries in the TASK_VIEW_CACHE to prevent unbounded memory growth.
const TASK_VIEW_CACHE_MAX_ENTRIES: usize = 128;

/// Evict expired entries, then oldest entries if still over capacity.
fn evict_task_view_cache_if_needed(
    cache: &mut HashMap<PathBuf, (core_state::task_state::ResolvedTaskView, Instant)>,
) {
    let now = Instant::now();
    // Phase 1: remove expired entries
    cache.retain(|_, (_, expires_at)| now < *expires_at);
    // Phase 2: if still over capacity, remove oldest entries by expiration time
    if cache.len() > TASK_VIEW_CACHE_MAX_ENTRIES {
        let mut entries: Vec<_> = cache.iter().map(|(k, v)| (k.clone(), v.1)).collect();
        entries.sort_by_key(|(_, exp)| *exp);
        let to_remove = cache.len() - TASK_VIEW_CACHE_MAX_ENTRIES;
        for (key, _) in entries.iter().take(to_remove) {
            cache.remove(key);
        }
    }
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
fn get_cached_task_view(repo_root: &Path) -> core_state::task_state::ResolvedTaskView {
    let ttl_secs = task_view_cache_ttl_secs();
    let cache_key = repo_root.to_path_buf();
    {
        let cache = get_task_view_cache();
        if let Some(guard) = poison_safe_read_lock!(cache)
            && let Some((view, expires_at)) = guard.get(&cache_key)
                && Instant::now() < *expires_at {
                    return view.clone();
                }
    }

    // Cache miss: recompute
    let view = resolve_task_view(repo_root, None);

    // Update cache with configurable TTL, evicting stale/overflow entries
    {
        let cache = get_task_view_cache();
        if let Some(mut guard) = poison_safe_write_lock!(cache) {
            guard.insert(
                cache_key,
                (view.clone(), Instant::now() + Duration::from_secs(ttl_secs)),
            );
            evict_task_view_cache_if_needed(&mut guard);
        }
    }

    view
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
        // Skip "content-length :" (16 chars)
        line[16..].trim()
    } else if lower.starts_with("content-length:") {
        // Skip "content-length:" (15 chars)
        line[15..].trim()
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

/// 生成连接级 session_id：`{host_id}-{nanos}`。
/// 每次 MCP stdio 连接调用一次，同一连接内所有 goal 操作共享此 ID。
fn generate_connection_session_id(host_id: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{host_id}-{nanos}")
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

pub fn handle_tools_list(id: Option<Value>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "tools": [
                {
                    "name": "framework_snapshot",
                    "description": "框架运行时快照（summary/full）。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "detail_level": {"type": "string", "enum": ["summary", "full"]},
                        },
                    },
                },
                {
                    "name": "skill_route",
                    "description": "自然语言查询匹配 skill 路由结果。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": {"type": "string"},
                        },
                        "required": ["query"],
                    },
                },
                {
                    "name": "skill_search",
                    "description": "搜索 skill 目录，返回最佳匹配。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": {"type": "string"},
                            "hostId": {"type": "string"},
                            "limit": {"type": "integer", "minimum": 1, "maximum": 50},
                        },
                        "required": ["query"],
                    },
                },
                {
                    "name": "skill_read",
                    "description": "读取 skill 的 SKILL.md 内容。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "skill": {"type": "string"},
                            "maxChars": {"type": "integer", "minimum": 1, "maximum": 50000},
                        },
                        "required": ["skill"],
                    },
                },
                {
                    "name": "skill_route_status",
                    "description": "Check routing artifacts exist.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                    },
                },
                {
                    "name": "record_evidence",
                    "description": "追加 evidence 到 EVIDENCE_INDEX。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "tool_name": {"type": "string"},
                            "command": {"type": "string"},
                            "exit_code": {"type": "integer"},
                            "output": {"type": "string"},
                        },
                        "required": ["tool_name", "command"],
                    },
                },
                {
                    "name": "session_checkpoint",
                    "description": "写入 SESSION_SUMMARY + NEXT_ACTIONS checkpoint。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "summary": {"type": "string"},
                            "next_actions": {"type": "array", "items": {"type": "string"}},
                            "task_id": {"type": "string"},
                        },
                        "required": ["summary"],
                    },
                },
                {
                    "name": "closeout_gate",
                    "description": "closeout 就绪状态与缺失项清单（advisory）。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "task_id": {"type": "string"},
                            "reviewer_lane": {"type": "string"},
                            "subagent_type": {"type": "string"},
                            "agent_type": {"type": "string"},
                        },
                    },
                },
                {
                    "name": "goal_state_read",
                    "description": "读取当前 task 的 GOAL_STATE.json 内容。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "task_id": {"type": "string"},
                        },
                    },
                },
                {
                    "name": "rfv_loop_status",
                    "description": "查看 RFV 循环状态。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "task_id": {"type": "string"},
                        },
                    },
                },
                {
                    "name": "rfv_loop_manage",
                    "description": "管理 RFV 循环 (start|append_round)。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "operation": {"type": "string", "enum": ["start", "append_round"]},
                            "task_id": {"type": "string"},
                            "session_id": {"type": "string"},
                            "round": {"type": "integer"},
                            "goal": {"type": "string"},
                            "review_summary": {"type": "string"},
                            "fix_summary": {"type": "string"},
                            "verify_result": {"type": "string", "enum": ["PASS", "FAIL", "SKIPPED", "UNKNOWN"]},
                            "supervisor_decision": {"type": "string"},
                            "reason": {"type": "string"},
                            "max_rounds": {"type": "integer"},
                            "allow_external_research": {"type": "boolean"},
                        },
                        "required": ["operation"],
                    },
                },
                {
                    "name": "closeout_record_write",
                    "description": "写入并验证 closeout record。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "task_id": {"type": "string"},
                            "summary": {"type": "string"},
                            "verification_status": {"type": "string", "enum": ["passed", "failed", "partial", "not_run"]},
                            "changed_files": {"type": "array", "items": {"type": "string"}},
                            "commands_run": {"type": "array", "items": {"type": "object", "properties": {"command": {"type": "string"}, "exit_code": {"type": "integer"}, "duration_ms": {"type": "integer"}}}},
                            "blockers": {"type": "array", "items": {"type": "string"}},
                            "risks": {"type": "array", "items": {"type": "string"}},
                            "notes": {"type": "string"},
                        },
                        "required": ["task_id", "summary", "verification_status"],
                    },
                },
                {
                    "name": "web_fetch",
                    "description": "只读 HTTP GET 抓取外部 URL。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "url": {"type": "string"},
                            "max_bytes": {"type": "integer"},
                        },
                        "required": ["url"],
                    },
                },
                {
                    "name": "routing_evolution",
                    "description": "路由日志分析 (stats|analyze|extract|calibrate)。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "operation": {"type": "string", "enum": ["stats", "analyze", "extract", "calibrate"]},
                            "skill": {"type": "string"},
                            "days": {"type": "integer"},
                        },
                        "required": ["operation"],
                    },
                },
                {
                    "name": "goal_state_manage",
                    "description": "管理 Goal 状态 (start|checkpoint|pause|resume|complete|clear|block)。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "operation": {"type": "string", "enum": ["start", "checkpoint", "pause", "resume", "complete", "clear", "block"]},
                            "task_id": {"type": "string"},
                            "session_id": {"type": "string"},
                            "goal": {"type": "string"},
                            "note": {"type": "string"},
                            "blocker": {"type": "string"},
                            "non_goals": {"type": "array", "items": {"type": "string"}},
                            "done_when": {"type": "array", "items": {"type": "string"}},
                            "validation_commands": {"type": "array", "items": {"type": "string"}},
                            "drive_until_done": {"type": "boolean"},
                            "lifecycle_profile": {"type": "string", "enum": ["my", "my-light", "interactive", "loop-auto"]},
                            "current_horizon": {"type": "string"},
                            "completion_gates": {"type": "object"},
                            "metadata": {"type": "object"},
                            "set_focus": {"type": "boolean"},
                        },
                        "required": ["operation"],
                    },
                },
                {
                    "name": "research_aigc_check",
                    "description": "AIGC 检测：返回 AI 概率评分(0-100)和逐段分析。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "text": {"type": "string"},
                            "language": {"type": "string", "enum": ["en", "zh"]},
                        },
                        "required": ["text"],
                    },
                },
                {
                    "name": "research_aigc_humanize",
                    "description": "AIGC 降重：句法改写/词汇替换/句式多样化。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "text": {"type": "string"},
                            "language": {"type": "string", "enum": ["en", "zh"]},
                            "preserve_academic_tone": {"type": "boolean"},
                        },
                        "required": ["text"],
                    },
                },
                {
                    "name": "research_review_dimensions",
                    "description": "获取审稿维度 prompt (round 1-7+)。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "round": {"type": "integer"},
                            "manuscript_summary": {"type": "string"},
                        },
                        "required": ["round"],
                    },
                },
                {
                    "name": "research_claim_drift",
                    "description": "检测 claim 漂移：原始 vs 当前声明的相似度和证据变化。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "original_claims": {
                                "type": "array",
                                "items": {"type": "object", "properties": {"id": {"type": "string"}, "text": {"type": "string"}}, "required": ["id", "text"]},
                            },
                            "current_claims": {
                                "type": "array",
                                "items": {"type": "object", "properties": {"id": {"type": "string"}, "text": {"type": "string"}}, "required": ["id", "text"]},
                            },
                        },
                        "required": ["original_claims", "current_claims"],
                    },
                },
                {
                    "name": "research_review_loop",
                    "description": "启动多轮对抗审稿循环。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "max_rounds": {"type": "integer"},
                            "min_rounds": {"type": "integer"},
                            "consecutive_stable_required": {"type": "integer"},
                        },
                        "required": [],
                    },
                },
            ],
        },
    })
}

fn tool_goal_state_read(arguments: &Value, repo_root: &Path) -> Result<String, String> {
    let task_id = arguments.get("task_id").and_then(Value::as_str);
    let state = core_state::state_manager::read_goal_state(repo_root, task_id);
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
            let task_view = get_cached_task_view(repo_root);
            let lifecycle_profile = task_lifecycle_profile(&task_view);
            let gate_mode =
                mcp_closeout_gate_mode_narrative(repo_root, host_id, &host_name, lifecycle_profile);
            {
                let lane_lines =
                    core_policy::registry_review_gate::reviewer_lanes_prompt_lines(Some(repo_root));
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
            let state = core_state::state_manager::read_goal_state(repo_root, None);
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
    let state = core_state::state_manager::read_rfv_loop_state(repo_root, task_id)?;
    serde_json::to_string_pretty(&state).map_err(|e| e.to_string())
}

fn parse_rfv_round_argument(value: Option<&Value>) -> Result<u64, String> {
    let Some(v) = value else {
        return Err("append_round requires 'round' argument (integer)".to_string());
    };
    if let Some(n) = v.as_u64() {
        return Ok(n);
    }
    if let Some(n) = v.as_i64()
        && n >= 0 {
            return Ok(n as u64);
        }
    Err("append_round requires 'round' argument (integer)".to_string())
}

fn tool_rfv_loop_manage(
    arguments: &Value,
    repo_root: &Path,
    connection_session_id: &str,
) -> Result<String, String> {
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
            // inject connection_session_id if not explicit
            let session_id = arguments
                .get("session_id")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(connection_session_id);
            payload["session_id"] = json!(session_id);
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
            if !matches!(verify_result, "PASS" | "FAIL" | "SKIPPED" | "UNKNOWN") {
                return Err(format!(
                    "verify_result must be one of PASS/FAIL/SKIPPED/UNKNOWN, got: {verify_result}"
                ));
            }
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
            ));
        }
    }

    // Prefer runtime-core's full implementation (has append_round support).
    // Fall back to core-state's lightweight version if runtime-core hook not registered.
    let result = match crate::hooks::rfv_loop_drive_registered() {
        Some(f) => f(payload)?,
        None => core_state::rfv_loop::framework_rfv_loop(payload)?,
    };
    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
}

fn tool_goal_state_manage(
    arguments: &Value,
    repo_root: &Path,
    connection_session_id: &str,
) -> Result<String, String> {
    let operation = arguments
        .get("operation")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: operation")?;

    // Auto-resolve task_id from TASK_POINTERS.json (shared with all other tools)
    let task_id = match arguments.get("task_id").and_then(Value::as_str).filter(|s| !s.trim().is_empty()) {
        Some(tid) => tid.to_string(),
        None => core_state::state_manager::read_primary_task_id(repo_root)
            .ok_or("No active task_id in TASK_POINTERS.json (start a task first or provide task_id explicitly)")?,
    };

    let repo_root_str = repo_root.to_string_lossy().to_string();

    let mut payload = json!({
        "repo_root": repo_root_str,
        "operation": operation,
    });
    payload["task_id"] = json!(task_id);

    match operation {
        "start" => {
            let goal = arguments
                .get("goal")
                .and_then(Value::as_str)
                .ok_or("start requires 'goal' argument (string)")?;
            payload["goal"] = json!(goal);

            // drive_until_done defaults to true (matches core-state behavior)
            let drive_until_done = arguments
                .get("drive_until_done")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            payload["drive_until_done"] = json!(drive_until_done);

            // Auto-fill contract fields when drive_until_done=true and not explicitly provided
            if drive_until_done {
                if arguments.get("non_goals").is_none() {
                    payload["non_goals"] = json!(["不处理此 goal 范围外的功能"]);
                }
                if arguments.get("done_when").is_none() {
                    payload["done_when"] = json!([
                        format!("goal 已完成: {goal}"),
                        "cargo check / test 通过".to_string(),
                    ]);
                }
                if arguments.get("validation_commands").is_none() {
                    payload["validation_commands"] =
                        json!(["cargo check --workspace", "cargo test --workspace"]);
                }
            }

            // Pass through explicitly provided contract fields (override defaults)
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

            // pass through optional session_id, or inject connection-level
            let session_id = arguments
                .get("session_id")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(connection_session_id);
            payload["session_id"] = json!(session_id);
            // pass-through: downstream state_manager consumes these for start
            if let Some(lp) = arguments.get("lifecycle_profile").and_then(Value::as_str) {
                match lp {
                    "my" | "my-light" | "interactive" | "loop-auto" => {
                        payload["lifecycle_profile"] = json!(lp);
                    },
                    _ => return Err(format!(
                        "Invalid lifecycle_profile: {lp}. Must be one of: my, my-light, interactive, loop-auto"
                    )),
                }
            }
            if let Some(ch) = arguments.get("current_horizon").and_then(Value::as_str) {
                payload["current_horizon"] = json!(ch);
            }
            if let Some(cg) = arguments.get("completion_gates") {
                payload["completion_gates"] = cg.clone();
            }
            if let Some(md) = arguments.get("metadata") {
                payload["metadata"] = md.clone();
            }
            if let Some(sf) = arguments.get("set_focus").and_then(Value::as_bool) {
                payload["set_focus"] = json!(sf);
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
            // Defensive: not in goal_state_manage schema enum, but prevents confusion
            // if a caller sends it here instead of rfv_loop_manage.
            return Err("append_round is not a valid goal_state_manage operation. \
                 Use rfv_loop_manage with operation=append_round instead."
                .to_string());
        }
        "pause" | "resume" | "complete" | "clear" => {
            // No additional required args
        }
        _ => {
            return Err(format!(
                "Unknown goal operation: {operation}. Valid operations: start, checkpoint, pause, resume, complete, clear, block"
            ));
        }
    }

    let result = core_state::state_manager::framework_goal_drive(payload)?;

    // Invalidate snapshot/task_view caches after goal state write (H3 FIX)
    invalidate_evidence_caches();
    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
}

// =============================================================================
// Test helper functions (used by integration tests in mcp_stdio_harness_tests.rs)
// =============================================================================

#[cfg(any(test, feature = "test-support"))]
pub fn tool_goal_state_manage_test_helper(
    arguments: &Value,
    operation: &str,
) -> Result<String, String> {
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
) -> Result<String, String> {
    tool_closeout_record_write(arguments, repo_path, "opencode")
}

#[cfg(any(test, feature = "test-support"))]
pub fn tool_rfv_loop_manage_test_helper(
    arguments: &Value,
    operation: &str,
) -> Result<String, String> {
    let path = crate::hosts::test_shim::unique_temp_repo("rfv-manage");
    let _ = std::fs::create_dir_all(&path);

    let mut args_with_op = arguments.clone();
    args_with_op["operation"] = json!(operation);

    let result = tool_rfv_loop_manage(&args_with_op, &path, "test-session-auto");
    let _ = std::fs::remove_dir_all(&path);
    result
}

#[cfg(any(test, feature = "test-support"))]
pub fn get_snapshot_ttl_for_test() -> u64 {
    snapshot_cache_ttl_secs()
}

#[cfg(any(test, feature = "test-support"))]
pub fn get_task_view_ttl_for_test() -> u64 {
    task_view_cache_ttl_secs()
}

#[cfg(any(test, feature = "test-support"))]
pub fn read_mcp_message_test_helper<R: std::io::BufRead>(
    input: &mut R,
    transport_mode: &mut Option<McpTransportMode>,
) -> Result<Option<String>, String> {
    read_mcp_message(input, transport_mode)
}

#[cfg(any(test, feature = "test-support"))]
pub fn init_tracker_for_test(path: &std::path::Path) -> Result<(), String> {
    crate::hooks::init_tracker(path)
}

#[cfg(any(test, feature = "test-support"))]
mod tests {
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
        assert_eq!(names.len(), 20, "expected 20 tools, got: {:?}", names);
        for tool in &[
            "framework_snapshot",
            "skill_route",
            "skill_search",
            "skill_read",
            "skill_route_status",
            "record_evidence",
            "session_checkpoint",
            "closeout_gate",
            "closeout_record_write",
            "routing_evolution",
            "web_fetch",
            "goal_state_read",
            "rfv_loop_status",
            "rfv_loop_manage",
            "goal_state_manage",
            "research_aigc_check",
            "research_aigc_humanize",
            "research_review_dimensions",
            "research_claim_drift",
            "research_review_loop",
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
}
