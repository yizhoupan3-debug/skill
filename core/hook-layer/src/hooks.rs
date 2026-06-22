//! hook-layer: 函数指针注册表、hook 分发、路由分发。
//!
//! runtime-core 依赖的 function-pointer slots 注册表。
//! 宿主应用在启动时通过 `register_*` 函数注入真实实现。

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

// ── Function pointer type aliases (reduce type_complexity warnings) ──

/// Route task with manifest fallback: (records, runtime_path, manifest_path, host_id, query, session_id, allow_overlay, first_turn) -> Result<RouteDecision, String>
type RouteTaskFn = fn(
    &[routing_engine::route::SkillRecord],
    Option<&Path>,
    Option<&Path>,
    Option<&str>,
    &str,
    &str,
    bool,
    bool,
) -> Result<RouteDecision, String>;

/// Build framework runtime snapshot envelope: (repo_root, runtime_path, host_id) -> Result<Value, String>
type BuildSnapshotFn = fn(&Path, Option<&Path>, Option<&str>) -> Result<Value, String>;

/// Build snapshot with level: (repo_root, runtime_path, host_id, level) -> Result<Value, String>
type BuildSnapshotWithLevelFn = fn(&Path, Option<&Path>, Option<&str>, &str) -> Result<Value, String>;

/// Build automatic continuity checkpoint payload: (repo_root, task_id, session_id, current_query, allow_overlay, first_turn) -> Value
type BuildCheckpointFn = fn(&Path, &str, &str, Option<&str>, bool, bool) -> Value;

/// Append evidence index row: (repo_root, task_id, metadata) -> Result<(), String>
type AppendEvidenceFn = fn(&Path, Option<&str>, serde_json::Map<String, Value>) -> Result<(), String>;

/// Register a `OnceLock` cell with consistent diagnostics on double-registration.
/// In `#[cfg(test)]` mode, includes a backtrace to help debug conflicting registrations.
pub fn once_lock_set<T>(lock: &OnceLock<T>, value: T, name: &str) {
    lock.set(value).unwrap_or_else(|_| {
        #[cfg(test)]
        {
            let bt = std::backtrace::Backtrace::force_capture();
            tracing::warn!(
                "{name} already registered — second call ignored\nbacktrace:\n{bt:?}"
            );
        }
        #[cfg(not(test))]
        tracing::warn!("{name} already registered — second call ignored");
    });
}

// ────────────────────────────────────────────────────────────────
// Mirror types (avoid dependency on runtime-core definitions)
// ────────────────────────────────────────────────────────────────

/// Mirror of `runtime_core_contracts::router_rs_observation::HookObservationHost`.
///
/// SYNC REQUIREMENT: When adding a new host, update BOTH:
/// 1. This enum (add variant + `from_host_id` + `as_str` arms)
/// 2. `PaperProseHookHost` enum below (same file)
///
/// The runtime-core-contracts newtype version resolves via the host provider registry
/// (`host_telemetry_for_id()`) and does NOT need an enum change — only this mirror does.
/// Long-term: replace this enum with `&'static str` to eliminate the dual-source risk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookObservationHost {
    Cursor,
    Codex,
    Claude,
    OpenCode,
}
pub type HookObservationHostType = HookObservationHost;

impl HookObservationHost {
    pub fn from_host_id(host_id: &str) -> Option<Self> {
        match host_id {
            "cursor" => Some(Self::Cursor),
            "codex" => Some(Self::Codex),
            "claude" => Some(Self::Claude),
            "opencode" => Some(Self::OpenCode),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cursor => "cursor",
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::OpenCode => "opencode",
        }
    }
}

/// Mirror of `runtime_core::paper_prose_hook::PaperProseHookHost`.
///
/// SYNC REQUIREMENT: must have the same variants as `HookObservationHost` above.
/// See `HookObservationHost` doc comment for the full sync protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaperProseHookHost {
    Cursor,
    Codex,
    Claude,
    OpenCode,
}
pub type PaperProseHookHostType = PaperProseHookHost;

impl PaperProseHookHost {
    /// Per-host env var controlling prose hook injection.
    pub fn env_var(self) -> &'static str {
        match self {
            Self::Cursor => "ROUTER_RS_CURSOR_PAPER_PROSE_HOOK",
            Self::Codex => "ROUTER_RS_CODEX_PAPER_PROSE_HOOK",
            Self::Claude => "ROUTER_RS_CLAUDE_PAPER_PROSE_HOOK",
            Self::OpenCode => "ROUTER_RS_OPENCODE_PAPER_PROSE_HOOK",
        }
    }

    /// Per-host env var controlling adversarial review hook injection.
    pub fn adversarial_env_var(self) -> &'static str {
        match self {
            Self::Cursor => "ROUTER_RS_CURSOR_PAPER_ADVERSARIAL_HOOK",
            Self::Codex => "ROUTER_RS_CODEX_PAPER_ADVERSARIAL_HOOK",
            Self::Claude => "ROUTER_RS_CLAUDE_PAPER_ADVERSARIAL_HOOK",
            Self::OpenCode => "ROUTER_RS_OPENCODE_PAPER_ADVERSARIAL_HOOK",
        }
    }

    pub fn from_host_lifecycle_state_dir(_state_dir_leaf: &str) -> Self {
        Self::Codex
    }

    pub fn from_host_id(host_id: &str) -> Option<Self> {
        match host_id {
            "cursor" => Some(Self::Cursor),
            "codex" => Some(Self::Codex),
            "claude" => Some(Self::Claude),
            "opencode" => Some(Self::OpenCode),
            _ => None,
        }
    }
}

/// Goal readiness flags used by review gate handlers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GoalReadiness {
    pub contract: bool,
    pub progress: bool,
    pub verification: bool,
}

// ────────────────────────────────────────────────────────────────
// Constants
// ────────────────────────────────────────────────────────────────

/// Mirror of `runtime_core::mcp_pre_guard::McpPreGuardVerdict`.
#[derive(Debug, Clone, Default)]
pub struct McpPreGuardVerdict {
    pub blocked: bool,
    pub reason: Option<String>,
}

/// Mirror of `routing_engine::route::RouteDecision`.
#[derive(Debug, Clone, Default)]
pub struct RouteDecision {
    pub selected_skill: String,
    pub selected_skill_path: Option<String>,
    pub reasons: Vec<String>,
    pub score: f64,
}

/// Mirror of `runtime_core::runtime_envelope_ids::MAX_CONCURRENT_SUBAGENTS_LIMIT`.
pub const MAX_CONCURRENT_SUBAGENTS_LIMIT: usize = 24;

/// Mirror of `runtime_core::rfv_loop::RFV_EXTERNAL_RESEARCH_SCHEMA_REL_PATH`.
pub const RFV_EXTERNAL_RESEARCH_SCHEMA_REL_PATH: &str =
    "configs/framework/RFV_EXTERNAL_RESEARCH.schema.json";

// ────────────────────────────────────────────────────────────────
// router_env_flags: thin wrappers over core_policy::env_flags
// ────────────────────────────────────────────────────────────────

pub fn router_rs_env_enabled_default_true(var_name: &str) -> bool {
    core_policy::env_flags::env_enabled_default_true(var_name)
}

pub fn router_rs_env_enabled_default_false(var_name: &str) -> bool {
    core_policy::env_flags::env_enabled_default_false(var_name)
}

pub fn router_rs_operator_inject_globally_enabled() -> bool {
    router_rs_env_enabled_default_true("ROUTER_RS_OPERATOR_INJECT")
}

pub fn router_rs_hook_legacy_subtracted_events_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS")
}

pub fn router_rs_hook_silent_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_HOOK_SILENT")
        || router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_SILENT")
}

pub fn router_rs_hook_outbound_context_max_bytes() -> usize {
    let key_canonical = "ROUTER_RS_HOOK_OUTBOUND_CONTEXT_MAX_CHARS";
    let key_legacy = "ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS";
    parse_env_usize(key_canonical)
        .or_else(|| parse_env_usize(key_legacy))
        .unwrap_or(8192)
}

pub fn router_rs_review_fork_context_missing_infer_false_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE")
}

pub fn router_rs_pre_goal_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_PRE_GOAL_ENABLED")
}

pub fn router_rs_hook_state_lock_retries() -> u32 {
    parse_env_u32("ROUTER_RS_CURSOR_HOOK_STATE_LOCK_RETRIES").unwrap_or(8)
}

pub fn router_rs_hook_state_file_sync_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_STATE_FILE_SYNC")
}

pub fn router_rs_hook_state_dir_sync_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_STATE_DIR_SYNC")
}

pub fn router_rs_review_pending_cycle_max() -> usize {
    parse_env_usize("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX").unwrap_or(3)
}

pub fn router_rs_review_gate_stop_max_nudges_cap() -> Option<u32> {
    #[cfg(test)]
    {
        let raw = std::env::var("ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES")
            .ok()
            .or_else(|| std::env::var("ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES").ok());
        raw.as_ref()?;
    }
    parse_env_u32("ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES")
        .or_else(|| parse_env_u32("ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES"))
}

pub fn router_rs_pre_goal_strict_disk_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK")
}

pub fn router_rs_hook_state_fail_open_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_STATE_FAIL_OPEN")
}

pub fn router_rs_cargo_check_sync_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_CARGO_CHECK_SYNC")
}

pub fn router_rs_hook_state_legacy_full_sweep_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP")
}

pub fn router_rs_hook_state_stale_sweep_days() -> u64 {
    parse_env_u64("ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS").unwrap_or(7)
}

/// §1.3: 自动清理 hook-state 目录中超过 [stale_sweep_days] 天的旧文件。
/// 在 hook-state 写入时调用，概率性触发（1/10）以避免每次写入都扫描目录。
/// 返回清理的文件数。
pub fn sweep_stale_hook_state_files(hook_state_dir: &Path) -> usize {
    // 概率性触发：1/10 的写入会触发清理
    {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let mut hasher = DefaultHasher::new();
        nanos.hash(&mut hasher);
        if !hasher.finish().is_multiple_of(10) {
            return 0;
        }
    }

    let days = router_rs_hook_state_stale_sweep_days();
    let cutoff = std::time::Duration::from_secs(days * 86400);
    let now = std::time::SystemTime::now();
    let mut cleaned = 0;

    let entries = match std::fs::read_dir(hook_state_dir) {
        Ok(e) => e,
        Err(_) => return 0,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let modified = match entry.metadata().and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(_) => continue,
        };
        if now.duration_since(modified).unwrap_or_default() > cutoff
            && std::fs::remove_file(&path).is_ok() {
                cleaned += 1;
            }
    }

    if cleaned > 0 {
        eprintln!(
            "[router-rs] hook-state sweep: removed {cleaned} stale file(s) from {}",
            hook_state_dir.display()
        );
    }
    cleaned
}

pub fn router_rs_sessionstart_context_max_bytes() -> usize {
    parse_env_usize("ROUTER_RS_CURSOR_SESSIONSTART_CONTEXT_MAX_BYTES").unwrap_or(64 * 1024)
}

fn parse_env_usize(var: &str) -> Option<usize> {
    std::env::var(var).ok().and_then(|v| v.trim().parse().ok())
}

fn parse_env_u32(var: &str) -> Option<u32> {
    std::env::var(var).ok().and_then(|v| v.trim().parse().ok())
}

fn parse_env_u64(var: &str) -> Option<u64> {
    std::env::var(var).ok().and_then(|v| v.trim().parse().ok())
}

// ────────────────────────────────────────────────────────────────
// Cross-host stdin reader (shared by claude, codex, opencode)
// ────────────────────────────────────────────────────────────────

/// Read stdin with 4 MiB limit and UTF-8 error normalization.
/// Shared across all hook-based hosts (claude, codex, opencode).
pub fn read_stdin_limited<R: std::io::Read>(reader: &mut R) -> Result<String, String> {
    use std::io::Read as _;
    const LIMIT: u64 = 4 * 1024 * 1024;
    let mut input = String::new();
    let mut limited = reader.take(LIMIT);
    limited.read_to_string(&mut input).map_err(|err| {
        let msg = err.to_string();
        let lower = msg.to_ascii_lowercase();
        if matches!(err.kind(), std::io::ErrorKind::InvalidData)
            || lower.contains("utf-8")
            || lower.contains("utf8")
            || lower.contains("utf")
        {
            return "stdin_invalid_utf8".to_string();
        }
        msg
    })?;
    if limited.limit() == 0 {
        let inner = limited.into_inner();
        let mut probe = [0u8; 1];
        if inner.read(&mut probe).map_err(|err| err.to_string())? > 0 {
            return Err("stdin payload exceeds 4 MiB limit".to_string());
        }
    }
    Ok(input)
}

/// Read stdin as JSON object with 4 MiB limit. Returns empty object if stdin is empty.
/// Rejects non-object JSON (arrays, strings, numbers, etc.) with an error.
pub fn read_stdin_json_limited() -> Result<Value, String> {
    let mut stdin = std::io::stdin();
    let input = read_stdin_limited(&mut stdin)?;
    if input.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }
    let val: Value = serde_json::from_str(&input).map_err(|_| "stdin_json_invalid".to_string())?;
    if !val.is_object() {
        return Err("stdin_json_not_object: expected JSON object".to_string());
    }
    Ok(val)
}

// ────────────────────────────────────────────────────────────────
// hook_timing: function-pointer proxies (OnceLock)
// ────────────────────────────────────────────────────────────────

static MARK_HOOK_START: OnceLock<fn()> = OnceLock::new();
static ADD_LOCK_WAIT_MS: OnceLock<fn(u64)> = OnceLock::new();
static ADD_CARGO_CHECK_MS: OnceLock<fn(u64)> = OnceLock::new();
static EMIT_HOOK_TIMING_LINE: OnceLock<fn(&str)> = OnceLock::new();

pub fn register_hook_timing(
    mark_start: fn(),
    add_lock_wait: fn(u64),
    add_cargo: fn(u64),
    emit_line: fn(&str),
) {
    once_lock_set(&MARK_HOOK_START, mark_start, "MARK_HOOK_START");
    once_lock_set(&ADD_LOCK_WAIT_MS, add_lock_wait, "ADD_LOCK_WAIT_MS");
    once_lock_set(&ADD_CARGO_CHECK_MS, add_cargo, "ADD_CARGO_CHECK_MS");
    once_lock_set(&EMIT_HOOK_TIMING_LINE, emit_line, "EMIT_HOOK_TIMING_LINE");
}

pub fn mark_hook_start() {
    if let Some(f) = MARK_HOOK_START.get() { f() }
}

pub fn add_lock_wait_ms(ms: u64) {
    if let Some(f) = ADD_LOCK_WAIT_MS.get() { f(ms) }
}

pub fn add_cargo_check_ms(ms: u64) {
    if let Some(f) = ADD_CARGO_CHECK_MS.get() { f(ms) }
}

pub fn emit_hook_timing_line(event: &str) {
    if let Some(f) = EMIT_HOOK_TIMING_LINE.get() { f(event) }
}

// ────────────────────────────────────────────────────────────────
// telemetry_emit: function-pointer proxies (OnceLock)
// ────────────────────────────────────────────────────────────────

static EMIT_HOOK_FIRED: OnceLock<fn(&str, &str)> = OnceLock::new();
static EMIT_TOOL_CALL: OnceLock<fn(&str, u64, bool)> = OnceLock::new();
static HOOK_ACTION_FROM_OPTIONAL_OUTPUT: OnceLock<fn(Option<&Value>) -> &'static str> =
    OnceLock::new();

pub fn register_telemetry(
    emit_hook_fired: fn(&str, &str),
    emit_tool_call: fn(&str, u64, bool),
    hook_action: fn(Option<&Value>) -> &'static str,
) {
    once_lock_set(&EMIT_HOOK_FIRED, emit_hook_fired, "EMIT_HOOK_FIRED");
    once_lock_set(&EMIT_TOOL_CALL, emit_tool_call, "EMIT_TOOL_CALL");
    once_lock_set(&HOOK_ACTION_FROM_OPTIONAL_OUTPUT, hook_action, "HOOK_ACTION_FROM_OPTIONAL_OUTPUT");
}

pub fn emit_hook_fired(hook_name: &str, action: &str) {
    if let Some(f) = EMIT_HOOK_FIRED.get() { f(hook_name, action) }
}

pub fn emit_tool_call(tool: &str, duration_ms: u64, success: bool) {
    if let Some(f) = EMIT_TOOL_CALL.get() { f(tool, duration_ms, success) }
}

pub fn hook_action_from_optional_output(output: Option<&Value>) -> &'static str {
    HOOK_ACTION_FROM_OPTIONAL_OUTPUT
        .get()
        .map(|f| f(output))
        .unwrap_or("unknown")
}

// ────────────────────────────────────────────────────────────────
// session_call_tracker: function-pointer proxies (OnceLock)
// ────────────────────────────────────────────────────────────────

#[allow(clippy::type_complexity)]
static INIT_TRACKER: OnceLock<fn(&Path) -> Result<(), String>> = OnceLock::new();
#[allow(clippy::type_complexity)]
static RECORD_TOOL_CALL: OnceLock<fn(&Path, &str, Option<&Value>) -> Result<(), String>> =
    OnceLock::new();
#[allow(clippy::type_complexity)]
static READ_TRACKER_STATE: OnceLock<fn(&Path) -> Result<Value, String>> = OnceLock::new();

pub fn register_session_call_tracker(
    init: fn(&Path) -> Result<(), String>,
    record: fn(&Path, &str, Option<&Value>) -> Result<(), String>,
    read_state: fn(&Path) -> Result<Value, String>,
) {
    once_lock_set(&INIT_TRACKER, init, "INIT_TRACKER");
    once_lock_set(&RECORD_TOOL_CALL, record, "RECORD_TOOL_CALL");
    once_lock_set(&READ_TRACKER_STATE, read_state, "READ_TRACKER_STATE");
}

pub fn init_tracker(repo_root: &Path) -> Result<(), String> {
    INIT_TRACKER.get().map(|f| f(repo_root)).unwrap_or(Ok(()))
}

pub fn record_tool_call(
    repo_root: &Path,
    tool_name: &str,
    cache_stats: Option<&Value>,
) -> Result<(), String> {
    RECORD_TOOL_CALL
        .get()
        .map(|f| f(repo_root, tool_name, cache_stats))
        .unwrap_or(Ok(()))
}

pub fn read_tracker_state(repo_root: &Path) -> Result<Value, String> {
    READ_TRACKER_STATE
        .get()
        .map(|f| f(repo_root))
        .unwrap_or(Ok(serde_json::json!({})))
}

// ────────────────────────────────────────────────────────────────
// framework_runtime: function-pointer proxies (OnceLock)
// ────────────────────────────────────────────────────────────────

#[allow(clippy::type_complexity)]
static BUILD_FRAMEWORK_CONTRACT: OnceLock<fn(&Path) -> Result<Value, String>> = OnceLock::new();
#[allow(clippy::type_complexity)]
static TRY_APPEND_POST_TOOL_SHELL: OnceLock<fn(&Path, &Value, &str) -> Result<(), String>> =
    OnceLock::new();
static CLOSEOUT_ENFORCEMENT: OnceLock<fn() -> bool> = OnceLock::new();
#[allow(clippy::type_complexity)]
static CLOSEOUT_RECORD_PATH: OnceLock<fn(&Path, &str) -> Result<PathBuf, String>> = OnceLock::new();
#[allow(clippy::type_complexity)]
static EVALUATE_CLOSEOUT: OnceLock<fn(&Path, &str, &Path) -> Result<Value, String>> =
    OnceLock::new();
static FIRST_TASK_ID: OnceLock<fn(&Path) -> Option<String>> = OnceLock::new();
static EVIDENCE_APPEND: OnceLock<fn(Value) -> Result<Value, String>> = OnceLock::new();
static EXTRACT_DURATION: OnceLock<fn(&Value) -> Option<u64>> = OnceLock::new();
static POST_TOOL_SUCCEEDED: OnceLock<fn(&Value) -> bool> = OnceLock::new();
static CLOSEOUT_STOP_FOLLOWUP: OnceLock<fn(&Path, &str) -> Option<String>> = OnceLock::new();

// 10 fn pointer params — above threshold=8, OK to keep.
// Each argument is a distinct registration slot stored in a OnceLock static.
// Extracting a struct would add ceremony to callers without reducing surface.
#[allow(clippy::too_many_arguments)]
pub fn register_framework_runtime(
    build_contract: fn(&Path) -> Result<Value, String>,
    append_shell: fn(&Path, &Value, &str) -> Result<(), String>,
    enforcement: fn() -> bool,
    record_path: fn(&Path, &str) -> Result<PathBuf, String>,
    eval_closeout: fn(&Path, &str, &Path) -> Result<Value, String>,
    first_task: fn(&Path) -> Option<String>,
    evidence_append: fn(Value) -> Result<Value, String>,
    extract_duration: fn(&Value) -> Option<u64>,
    post_tool_ok: fn(&Value) -> bool,
    closeout_followup: fn(&Path, &str) -> Option<String>,
) {
    once_lock_set(&BUILD_FRAMEWORK_CONTRACT, build_contract, "BUILD_FRAMEWORK_CONTRACT");
    once_lock_set(&TRY_APPEND_POST_TOOL_SHELL, append_shell, "TRY_APPEND_POST_TOOL_SHELL");
    once_lock_set(&CLOSEOUT_ENFORCEMENT, enforcement, "CLOSEOUT_ENFORCEMENT");
    once_lock_set(&CLOSEOUT_RECORD_PATH, record_path, "CLOSEOUT_RECORD_PATH");
    once_lock_set(&EVALUATE_CLOSEOUT, eval_closeout, "EVALUATE_CLOSEOUT");
    once_lock_set(&FIRST_TASK_ID, first_task, "FIRST_TASK_ID");
    once_lock_set(&EVIDENCE_APPEND, evidence_append, "EVIDENCE_APPEND");
    once_lock_set(&EXTRACT_DURATION, extract_duration, "EXTRACT_DURATION");
    once_lock_set(&POST_TOOL_SUCCEEDED, post_tool_ok, "POST_TOOL_SUCCEEDED");
    once_lock_set(&CLOSEOUT_STOP_FOLLOWUP, closeout_followup, "CLOSEOUT_STOP_FOLLOWUP");
}

pub fn build_framework_contract_summary_envelope(repo_root: &Path) -> Result<Value, String> {
    BUILD_FRAMEWORK_CONTRACT
        .get()
        .map(|f| f(repo_root))
        .unwrap_or_else(|| Err("framework_runtime not registered".into()))
}

pub fn try_append_post_tool_shell_evidence(
    repo_root: &Path,
    event: &Value,
    kind: &str,
) -> Result<(), String> {
    TRY_APPEND_POST_TOOL_SHELL
        .get()
        .map(|f| f(repo_root, event, kind))
        .unwrap_or(Ok(()))
}

pub fn closeout_programmatic_enforcement_enabled() -> bool {
    CLOSEOUT_ENFORCEMENT.get().map(|f| f()).unwrap_or(false)
}

pub fn closeout_record_path_for_task(repo_root: &Path, task_id: &str) -> Result<PathBuf, String> {
    CLOSEOUT_RECORD_PATH
        .get()
        .map(|f| f(repo_root, task_id))
        .unwrap_or_else(|| Err("framework_runtime not registered".into()))
}

pub fn evaluate_closeout_record_file_for_task(
    repo_root: &Path,
    task_id: &str,
    record_path: &Path,
) -> Result<Value, String> {
    EVALUATE_CLOSEOUT
        .get()
        .map(|f| f(repo_root, task_id, record_path))
        .unwrap_or_else(|| Err("framework_runtime not registered".into()))
}

pub fn first_task_id_from_registry(repo_root: &Path) -> Option<String> {
    FIRST_TASK_ID.get().map(|f| f(repo_root)).unwrap_or(None)
}

pub fn framework_hook_evidence_append(payload: Value) -> Result<Value, String> {
    EVIDENCE_APPEND
        .get()
        .map(|f| f(payload))
        .unwrap_or_else(|| Err("framework_runtime not registered".into()))
}

pub fn extract_post_tool_duration_ms(event: &Value) -> Option<u64> {
    EXTRACT_DURATION.get().map(|f| f(event)).unwrap_or(None)
}

pub fn post_tool_call_succeeded(event: &Value) -> bool {
    POST_TOOL_SUCCEEDED.get().map(|f| f(event)).unwrap_or(true)
}

pub fn closeout_stop_followup_for_completion_text(repo_root: &Path, text: &str) -> Option<String> {
    CLOSEOUT_STOP_FOLLOWUP
        .get()
        .map(|f| f(repo_root, text))
        .unwrap_or(None)
}

// ────────────────────────────────────────────────────────────────
// router_rs_observation: function-pointer proxies (OnceLock)
// ────────────────────────────────────────────────────────────────

static ATTACH_OBSERVATION: OnceLock<fn(&mut Value, HookObservationHost)> = OnceLock::new();
static STRIP_OBSERVATION: OnceLock<fn(&mut Value)> = OnceLock::new();

pub fn register_router_rs_observation(
    attach: fn(&mut Value, HookObservationHost),
    strip: fn(&mut Value),
) {
    once_lock_set(&ATTACH_OBSERVATION, attach, "ATTACH_OBSERVATION");
    once_lock_set(&STRIP_OBSERVATION, strip, "STRIP_OBSERVATION");
}

pub fn attach_router_rs_observation(output: &mut Value, host: HookObservationHost) {
    if let Some(f) = ATTACH_OBSERVATION.get() { f(output, host) }
}

pub fn strip_router_rs_observation(output: &mut Value) {
    if let Some(f) = STRIP_OBSERVATION.get() { f(output) }
}

// ────────────────────────────────────────────────────────────────
// hook_outbound_protect: default policy
//
// DESIGN INTENT: host-projection's outbound protection is intentionally a no-op in production.
//
// The authoritative implementation lives in runtime-core-contracts (hook_outbound_protect.rs),
// which runtime-core re-exports and registers via register_hook_outbound_protect().
// ────────────────────────────────────────────────────────────────
// hook_outbound_protect: function-pointer proxies (OnceLock)
// ────────────────────────────────────────────────────────────────

static OUTBOUND_PROTECTED: OnceLock<fn(&str) -> bool> = OnceLock::new();
static TRUNCATE_OUTBOUND: OnceLock<fn(&str, usize, &str) -> String> = OnceLock::new();

pub fn register_hook_outbound_protect(
    is_protected: fn(&str) -> bool,
    truncate: fn(&str, usize, &str) -> String,
) {
    once_lock_set(&OUTBOUND_PROTECTED, is_protected, "OUTBOUND_PROTECTED");
    once_lock_set(&TRUNCATE_OUTBOUND, truncate, "TRUNCATE_OUTBOUND");
}

pub fn hook_outbound_line_is_framework_protected(line: &str) -> bool {
    OUTBOUND_PROTECTED.get().map(|f| f(line)).unwrap_or(false)
}

pub fn truncate_hook_outbound_lines_preserving(
    combined: &str,
    max_bytes: usize,
    suffix: &str,
) -> String {
    TRUNCATE_OUTBOUND
        .get()
        .map(|f| f(combined, max_bytes, suffix))
        .unwrap_or_else(|| combined.to_string())
}

// hook_posttool_normalize: default policy (register removed — was never called in production)
// ────────────────────────────────────────────────────────────────

#[cfg(not(test))]
pub fn synthetic_post_tool_evidence_shape(_event: &Value) -> Value {
    serde_json::json!({})
}

#[cfg(test)]
pub use crate::test_helpers::{
    register_hook_posttool_normalize,
    synthetic_post_tool_evidence_shape,
};

// ────────────────────────────────────────────────────────────────
// ship_readiness: function-pointer proxies (OnceLock)
// ────────────────────────────────────────────────────────────────

static EVAL_GOAL_READINESS: OnceLock<fn(&Path, &Value, &str) -> GoalReadiness> = OnceLock::new();
static GOAL_STOP_FOLLOWUP: OnceLock<fn(bool, bool, bool, u32) -> String> = OnceLock::new();

pub fn register_ship_readiness(
    evaluate: fn(&Path, &Value, &str) -> GoalReadiness,
    followup: fn(bool, bool, bool, u32) -> String,
) {
    once_lock_set(&EVAL_GOAL_READINESS, evaluate, "EVAL_GOAL_READINESS");
    once_lock_set(&GOAL_STOP_FOLLOWUP, followup, "GOAL_STOP_FOLLOWUP");
}

pub fn evaluate_goal_readiness_from_disk(
    repo_root: &Path,
    goal: &Value,
    task_id: &str,
) -> GoalReadiness {
    EVAL_GOAL_READINESS
        .get()
        .map(|f| f(repo_root, goal, task_id))
        .unwrap_or_default()
}

pub fn goal_stop_followup_line(
    contract: bool,
    progress: bool,
    verification: bool,
    goal_followup_count: u32,
) -> String {
    GOAL_STOP_FOLLOWUP
        .get()
        .map(|f| f(contract, progress, verification, goal_followup_count))
        .unwrap_or_default()
}

// ────────────────────────────────────────────────────────────────
// paper hooks: function-pointer proxies (OnceLock)
// ────────────────────────────────────────────────────────────────

/// Append paper prose/adversarial context: (repo_root, prompt_text, contexts, host)
type AppendPaperContextFn = fn(&Path, &str, &mut Vec<String>, PaperProseHookHost);

/// Merge paper prose/adversarial before submit: (repo_root, output, prompt_text, use_followup_message)
type MergePaperContextFn = fn(&Path, &mut Value, &str, bool);

static APPEND_PROSE: OnceLock<AppendPaperContextFn> = OnceLock::new();
static MERGE_PROSE: OnceLock<MergePaperContextFn> = OnceLock::new();
static APPEND_ADVERSARIAL: OnceLock<AppendPaperContextFn> = OnceLock::new();
static MERGE_ADVERSARIAL: OnceLock<MergePaperContextFn> = OnceLock::new();

pub fn register_paper_hooks(
    append_prose: AppendPaperContextFn,
    merge_prose: MergePaperContextFn,
    append_adversarial: AppendPaperContextFn,
    merge_adversarial: MergePaperContextFn,
) {
    once_lock_set(&APPEND_PROSE, append_prose, "APPEND_PROSE");
    once_lock_set(&MERGE_PROSE, merge_prose, "MERGE_PROSE");
    once_lock_set(&APPEND_ADVERSARIAL, append_adversarial, "APPEND_ADVERSARIAL");
    once_lock_set(&MERGE_ADVERSARIAL, merge_adversarial, "MERGE_ADVERSARIAL");
}

pub fn maybe_append_paper_prose_context(
    repo_root: &Path,
    prompt_text: &str,
    contexts: &mut Vec<String>,
    host: PaperProseHookHost,
) {
    if let Some(f) = APPEND_PROSE
        .get() { f(repo_root, prompt_text, contexts, host) }
}

pub fn maybe_merge_paper_prose_before_submit(
    repo_root: &Path,
    output: &mut Value,
    prompt_text: &str,
    use_followup_message: bool,
) {
    if let Some(f) = MERGE_PROSE
        .get() { f(repo_root, output, prompt_text, use_followup_message) }
}

pub fn maybe_append_paper_adversarial_context(
    repo_root: &Path,
    prompt_text: &str,
    contexts: &mut Vec<String>,
    host: PaperProseHookHost,
) {
    if let Some(f) = APPEND_ADVERSARIAL
        .get() { f(repo_root, prompt_text, contexts, host) }
}

pub fn maybe_merge_paper_adversarial_before_submit(
    repo_root: &Path,
    output: &mut Value,
    prompt_text: &str,
    use_followup_message: bool,
) {
    if let Some(f) = MERGE_ADVERSARIAL
        .get() { f(repo_root, output, prompt_text, use_followup_message) }
}

// ────────────────────────────────────────────────────────────────
// research activity log: function-pointer proxy (OnceLock)
// ────────────────────────────────────────────────────────────────

static RESEARCH_ACTIVITY: OnceLock<fn(&Path, &str, &str)> = OnceLock::new();

pub fn register_research_activity_hook(f: fn(&Path, &str, &str)) {
    once_lock_set(&RESEARCH_ACTIVITY, f, "RESEARCH_ACTIVITY");
}

pub fn maybe_record_research_activity(repo_root: &Path, tool_name: &str, summary: &str) {
    if let Some(f) = RESEARCH_ACTIVITY.get() {
        f(repo_root, tool_name, summary)
    }
}

// ────────────────────────────────────────────────────────────────
// kernel_bootstrap: function-pointer proxy (OnceLock)
// ────────────────────────────────────────────────────────────────

static ENSURE_KERNEL: OnceLock<fn()> = OnceLock::new();

pub fn register_kernel_bootstrap(f: fn()) {
    once_lock_set(&ENSURE_KERNEL, f, "ENSURE_KERNEL");
}

pub(crate) fn ensure_kernel_bootstrap() {
    if let Some(f) = ENSURE_KERNEL.get() { f() }
    #[cfg(test)]
    crate::test_helpers::install_test_deps();
}

/// Public entry point that only calls the registered kernel bootstrap function (if any).
/// Host-projection wraps this with its own `#[cfg(test)] install_test_deps()`.
pub fn ensure_kernel_bootstrap_registered() {
    if let Some(f) = ENSURE_KERNEL.get() { f() }
}

// ────────────────────────────────────────────────────────────────
// Additional hooks needed by claude_hooks / mcp_stdio_harness
// (appended during host-projection hooks consolidation)
// ────────────────────────────────────────────────────────────────

// ── framework_runtime_extra ──

/// Resolve repo root argument: (repo_root) -> Result<PathBuf, String>
type ResolveRepoRootFn = fn(Option<&Path>) -> Result<PathBuf, String>;

/// Check anomalies: (repo_root) -> Result<anomaly_list, String>
type CheckAnomaliesFn = fn(&Path) -> Result<Vec<String>, String>;

static RESOLVE_REPO_ROOT_ARG: OnceLock<ResolveRepoRootFn> = OnceLock::new();
static CURRENT_LOCAL_TIMESTAMP: OnceLock<fn() -> String> = OnceLock::new();
static WRITE_FRAMEWORK_SESSION_ARTIFACTS: OnceLock<fn(Value) -> Result<Value, String>> =
    OnceLock::new();
static ROUTE_TASK_WITH_MANIFEST_FALLBACK: OnceLock<RouteTaskFn> = OnceLock::new();
static BUILD_FRAMEWORK_RUNTIME_SNAPSHOT_ENVELOPE: OnceLock<BuildSnapshotFn> = OnceLock::new();
static BUILD_FRAMEWORK_RUNTIME_SNAPSHOT_ENVELOPE_WITH_LEVEL: OnceLock<BuildSnapshotWithLevelFn> = OnceLock::new();
static BUILD_AUTOMATIC_CONTINUITY_CHECKPOINT_PAYLOAD: OnceLock<BuildCheckpointFn> = OnceLock::new();
static APPEND_EVIDENCE_INDEX: OnceLock<AppendEvidenceFn> = OnceLock::new();
static HOOK_ACTION_FROM_OUTPUT: OnceLock<fn(&Value) -> &'static str> = OnceLock::new();
static CLOSEOUT_RECORD_SCHEMA_VERSION_FN: OnceLock<fn() -> &'static str> = OnceLock::new();
static CHECK_ANOMALIES: OnceLock<CheckAnomaliesFn> = OnceLock::new();

// ── web_fetch_guard ──

/// Validate and resolve web fetch URL: (url) -> Result<(resolved_url, addresses), String>
type ValidateWebFetchUrlFn = fn(&str) -> Result<(String, Vec<String>), String>;

/// Resolve web fetch redirect: (base_url, location) -> Result<resolved_url, String>
type ResolveWebFetchRedirectFn = fn(&str, &str) -> Result<String, String>;

/// Resolve web fetch addresses: (host, port) -> Result<addresses, String>
type ResolveWebFetchAddressesFn = fn(&str, u16) -> Result<Vec<String>, String>;

static VALIDATE_AND_RESOLVE_WEB_FETCH_URL: OnceLock<ValidateWebFetchUrlFn> = OnceLock::new();
static RESOLVE_WEB_FETCH_REDIRECT: OnceLock<ResolveWebFetchRedirectFn> = OnceLock::new();
static RESOLVE_WEB_FETCH_ADDRESSES: OnceLock<ResolveWebFetchAddressesFn> = OnceLock::new();

// ── mcp_pre_guard ──

static EVALUATE_MCP_PRE_GUARD_SAFE: OnceLock<fn(&str, &Value, &Path) -> McpPreGuardVerdict> =
    OnceLock::new();

// 10+ fn pointer params in a registration pattern — above threshold=8, OK to keep.
// Each is a distinct OnceLock slot; struct would not reduce surface.
#[allow(clippy::too_many_arguments)]
pub fn register_framework_runtime_extra(
    resolve_repo_root_arg: ResolveRepoRootFn,
    current_local_timestamp: fn() -> String,
    write_framework_session_artifacts: fn(Value) -> Result<Value, String>,
    route_task_with_manifest_fallback: RouteTaskFn,
    build_framework_runtime_snapshot_envelope: BuildSnapshotFn,
    build_automatic_continuity_checkpoint_payload: BuildCheckpointFn,
    append_evidence_index: AppendEvidenceFn,
    hook_action_from_output: fn(&Value) -> &'static str,
    closeout_record_schema_version: fn() -> &'static str,
    check_anomalies: CheckAnomaliesFn,
) {
    once_lock_set(&RESOLVE_REPO_ROOT_ARG, resolve_repo_root_arg, "RESOLVE_REPO_ROOT_ARG");
    once_lock_set(&CURRENT_LOCAL_TIMESTAMP, current_local_timestamp, "CURRENT_LOCAL_TIMESTAMP");
    once_lock_set(&WRITE_FRAMEWORK_SESSION_ARTIFACTS, write_framework_session_artifacts, "WRITE_FRAMEWORK_SESSION_ARTIFACTS");
    once_lock_set(&ROUTE_TASK_WITH_MANIFEST_FALLBACK, route_task_with_manifest_fallback, "ROUTE_TASK_WITH_MANIFEST_FALLBACK");
    once_lock_set(&BUILD_FRAMEWORK_RUNTIME_SNAPSHOT_ENVELOPE, build_framework_runtime_snapshot_envelope, "BUILD_FRAMEWORK_RUNTIME_SNAPSHOT_ENVELOPE");
    once_lock_set(&BUILD_AUTOMATIC_CONTINUITY_CHECKPOINT_PAYLOAD, build_automatic_continuity_checkpoint_payload, "BUILD_AUTOMATIC_CONTINUITY_CHECKPOINT_PAYLOAD");
    once_lock_set(&APPEND_EVIDENCE_INDEX, append_evidence_index, "APPEND_EVIDENCE_INDEX");
    once_lock_set(&HOOK_ACTION_FROM_OUTPUT, hook_action_from_output, "HOOK_ACTION_FROM_OUTPUT");
    once_lock_set(&CLOSEOUT_RECORD_SCHEMA_VERSION_FN, closeout_record_schema_version, "CLOSEOUT_RECORD_SCHEMA_VERSION_FN");
    once_lock_set(&CHECK_ANOMALIES, check_anomalies, "CHECK_ANOMALIES");
}

pub fn register_web_fetch_guard_extra(
    validate_url: ValidateWebFetchUrlFn,
    resolve_redirect: ResolveWebFetchRedirectFn,
    resolve_addresses: ResolveWebFetchAddressesFn,
) {
    once_lock_set(&VALIDATE_AND_RESOLVE_WEB_FETCH_URL, validate_url, "VALIDATE_AND_RESOLVE_WEB_FETCH_URL");
    once_lock_set(&RESOLVE_WEB_FETCH_REDIRECT, resolve_redirect, "RESOLVE_WEB_FETCH_REDIRECT");
    once_lock_set(&RESOLVE_WEB_FETCH_ADDRESSES, resolve_addresses, "RESOLVE_WEB_FETCH_ADDRESSES");
}

pub fn register_mcp_pre_guard_extra(evaluate: fn(&str, &Value, &Path) -> McpPreGuardVerdict) {
    once_lock_set(&EVALUATE_MCP_PRE_GUARD_SAFE, evaluate, "EVALUATE_MCP_PRE_GUARD_SAFE");
}

pub fn resolve_repo_root_arg(repo_root: Option<&Path>) -> Result<PathBuf, String> {
    RESOLVE_REPO_ROOT_ARG
        .get()
        .map(|f| f(repo_root))
        .unwrap_or_else(|| {
            // Default: use CARGO_MANIFEST_DIR or current dir
            std::env::current_dir().map_err(|e| e.to_string())
        })
}

pub fn current_local_timestamp() -> String {
    CURRENT_LOCAL_TIMESTAMP
        .get()
        .map(|f| f())
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".into())
}

pub fn write_framework_session_artifacts(payload: Value) -> Result<Value, String> {
    WRITE_FRAMEWORK_SESSION_ARTIFACTS
        .get()
        .map(|f| f(payload))
        .unwrap_or_else(|| Err("hooks not registered".into()))
}

pub fn route_task_with_manifest_fallback(
    runtime_records: &[routing_engine::route::SkillRecord],
    runtime_path: Option<&Path>,
    manifest_path: Option<&Path>,
    host_id: Option<&str>,
    query: &str,
    session_id: &str,
    allow_overlay: bool,
    first_turn: bool,
) -> Result<RouteDecision, String> {
    ROUTE_TASK_WITH_MANIFEST_FALLBACK
        .get()
        .map(|f| {
            f(
                runtime_records,
                runtime_path,
                manifest_path,
                host_id,
                query,
                session_id,
                allow_overlay,
                first_turn,
            )
        })
        .unwrap_or_else(|| Err("hooks not registered".into()))
}

pub fn build_framework_runtime_snapshot_envelope(
    repo_root: &Path,
    artifact_root_override: Option<&Path>,
    task_id_override: Option<&str>,
) -> Result<Value, String> {
    BUILD_FRAMEWORK_RUNTIME_SNAPSHOT_ENVELOPE
        .get()
        .map(|f| f(repo_root, artifact_root_override, task_id_override))
        .unwrap_or_else(|| Err("hooks not registered".into()))
}

pub fn build_framework_runtime_snapshot_envelope_with_level(
    repo_root: &Path,
    artifact_root_override: Option<&Path>,
    task_id_override: Option<&str>,
    detail_level: &str,
) -> Result<Value, String> {
    if let Some(f) = BUILD_FRAMEWORK_RUNTIME_SNAPSHOT_ENVELOPE_WITH_LEVEL.get() {
        f(
            repo_root,
            artifact_root_override,
            task_id_override,
            detail_level,
        )
    } else {
        // Fallback: use old function pointer (ignores detail_level, returns old format)
        build_framework_runtime_snapshot_envelope(
            repo_root,
            artifact_root_override,
            task_id_override,
        )
    }
}

pub fn register_build_framework_runtime_snapshot_envelope_with_level(
    func: BuildSnapshotWithLevelFn,
) {
    once_lock_set(&BUILD_FRAMEWORK_RUNTIME_SNAPSHOT_ENVELOPE_WITH_LEVEL, func, "BUILD_FRAMEWORK_RUNTIME_SNAPSHOT_ENVELOPE_WITH_LEVEL");
}

pub fn build_automatic_continuity_checkpoint_payload(
    repo_root: &Path,
    task_line: &str,
    summary_text: &str,
    task_id: Option<&str>,
    repointer_focus: bool,
    update_registry_only_if_known: bool,
) -> Value {
    BUILD_AUTOMATIC_CONTINUITY_CHECKPOINT_PAYLOAD
        .get()
        .map(|f| {
            f(
                repo_root,
                task_line,
                summary_text,
                task_id,
                repointer_focus,
                update_registry_only_if_known,
            )
        })
        .unwrap_or(Value::Null)
}

pub fn check_anomalies(repo_root: &Path) -> Result<Vec<String>, String> {
    CHECK_ANOMALIES
        .get()
        .map(|f| f(repo_root))
        .unwrap_or_else(|| Ok(vec![]))
}

pub fn append_evidence_index(
    repo_root: &Path,
    task_id: Option<&str>,
    entry: serde_json::Map<String, Value>,
) -> Result<(), String> {
    APPEND_EVIDENCE_INDEX
        .get()
        .map(|f| f(repo_root, task_id, entry))
        .unwrap_or_else(|| Err("hooks not registered".into()))
}

pub fn hook_action_from_output(output: &Value) -> &'static str {
    HOOK_ACTION_FROM_OUTPUT
        .get()
        .map(|f| f(output))
        .unwrap_or("unknown")
}

pub fn closeout_record_schema_version() -> &'static str {
    CLOSEOUT_RECORD_SCHEMA_VERSION_FN
        .get()
        .map(|f| f())
        .unwrap_or("closeout-record-v1")
}

pub fn validate_and_resolve_web_fetch_url(url: &str) -> Result<(String, Vec<String>), String> {
    VALIDATE_AND_RESOLVE_WEB_FETCH_URL
        .get()
        .map(|f| f(url))
        .unwrap_or_else(|| Err("hooks not registered".into()))
}

pub fn resolve_web_fetch_redirect(base: &str, location: &str) -> Result<String, String> {
    RESOLVE_WEB_FETCH_REDIRECT
        .get()
        .map(|f| f(base, location))
        .unwrap_or_else(|| Err("hooks not registered".into()))
}

pub fn resolve_web_fetch_addresses(host: &str, port: u16) -> Result<Vec<String>, String> {
    RESOLVE_WEB_FETCH_ADDRESSES
        .get()
        .map(|f| f(host, port))
        .unwrap_or_else(|| Err("hooks not registered".into()))
}

pub fn evaluate_mcp_pre_guard_safe(
    tool_name: &str,
    arguments: &Value,
    repo_root: &Path,
) -> McpPreGuardVerdict {
    EVALUATE_MCP_PRE_GUARD_SAFE
        .get()
        .map(|f| f(tool_name, arguments, repo_root))
        .unwrap_or(McpPreGuardVerdict {
            blocked: false,
            reason: None,
        })
}

// ── Quality Gate full implementation hook (registered by runtime-core) ──
static QUALITY_GATE_DRIVE: OnceLock<fn(Value) -> Result<Value, String>> = OnceLock::new();

pub fn register_quality_gate_drive(func: fn(Value) -> Result<Value, String>) {
    once_lock_set(&QUALITY_GATE_DRIVE, func, "QUALITY_GATE_DRIVE");
}

/// Call the registered quality_gate implementation (runtime-core has append_round support).
/// Returns None if not registered (caller should fall back to core-state).
pub fn quality_gate_drive_registered() -> Option<fn(Value) -> Result<Value, String>> {
    QUALITY_GATE_DRIVE.get().copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn env_enabled_default_true_returns_true_when_unset() {
        unsafe { std::env::remove_var("ROUTER_RS_OPERATOR_INJECT") };
        assert!(router_rs_operator_inject_globally_enabled());
    }

    #[test]
    fn env_enabled_default_false_returns_false_when_unset() {
        unsafe { std::env::remove_var("ROUTER_RS_HOOK_SILENT") };
        assert!(!router_rs_hook_silent_enabled());
    }

    #[test]
    fn outbound_context_max_bytes_default() {
        unsafe { std::env::remove_var("ROUTER_RS_HOOK_OUTBOUND_CONTEXT_MAX_CHARS") };
        unsafe { std::env::remove_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS") };
        assert_eq!(router_rs_hook_outbound_context_max_bytes(), 8192);
    }

    #[test]
    fn hook_state_lock_retries_default() {
        unsafe { std::env::remove_var("ROUTER_RS_CURSOR_HOOK_STATE_LOCK_RETRIES") };
        assert_eq!(router_rs_hook_state_lock_retries(), 8);
    }

    #[test]
    fn review_pending_cycle_max_default() {
        unsafe { std::env::remove_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX") };
        assert_eq!(router_rs_review_pending_cycle_max(), 3);
    }

    #[test]
    fn stale_sweep_days_default() {
        unsafe { std::env::remove_var("ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS") };
        assert_eq!(router_rs_hook_state_stale_sweep_days(), 7);
    }

    #[test]
    fn goal_readiness_default_all_false() {
        let r = GoalReadiness::default();
        assert!(!r.contract);
        assert!(!r.progress);
        assert!(!r.verification);
    }

    #[test]
    fn hook_outbound_default_not_protected() {
        assert!(!hook_outbound_line_is_framework_protected("any line"));
    }

    #[test]
    fn truncate_outbound_default_passthrough() {
        let input = "hello world";
        assert_eq!(
            truncate_hook_outbound_lines_preserving(input, 5, "..."),
            input
        );
    }

    #[test]
    fn synthetic_post_tool_default_empty() {
        let result = synthetic_post_tool_evidence_shape(&json!({"tool": "test"}));
        assert_eq!(result, json!({}));
    }

    #[test]
    fn evaluate_goal_readiness_default_all_false() {
        let r =
            evaluate_goal_readiness_from_disk(Path::new("/tmp"), &json!({"goal": "test"}), "t1");
        assert!(!r.contract);
        assert!(!r.progress);
        assert!(!r.verification);
    }

    #[test]
    fn goal_stop_followup_default_empty() {
        assert!(goal_stop_followup_line(false, false, false, 0).is_empty());
    }

    #[test]
    fn hook_observation_host_roundtrip() {
        for host in [
            HookObservationHost::Cursor,
            HookObservationHost::Codex,
            HookObservationHost::Claude,
            HookObservationHost::OpenCode,
        ] {
            assert_eq!(HookObservationHost::from_host_id(host.as_str()), Some(host));
        }
        assert_eq!(HookObservationHost::from_host_id("unknown"), None);
    }

    #[test]
    fn paper_prose_hook_host_env_var() {
        assert_eq!(
            PaperProseHookHost::Cursor.env_var(),
            "ROUTER_RS_CURSOR_PAPER_PROSE_HOOK"
        );
        assert_eq!(
            PaperProseHookHost::Codex.env_var(),
            "ROUTER_RS_CODEX_PAPER_PROSE_HOOK"
        );
        assert_eq!(
            PaperProseHookHost::Claude.env_var(),
            "ROUTER_RS_CLAUDE_PAPER_PROSE_HOOK"
        );
        assert_eq!(
            PaperProseHookHost::OpenCode.env_var(),
            "ROUTER_RS_OPENCODE_PAPER_PROSE_HOOK"
        );
    }

    #[test]
    fn hook_observation_host_json_snapshot() {
        // Snapshot all HookObservationHost enum variant serializations.
        let variants: Vec<_> = ["cursor", "codex", "claude", "opencode"]
            .iter()
            .filter_map(|h| HookObservationHost::from_host_id(h))
            .collect();
        insta::assert_debug_snapshot!(variants);
    }

    #[test]
    fn route_decision_default() {
        let d = RouteDecision::default();
        assert!(d.selected_skill.is_empty());
        assert!(d.reasons.is_empty());
        assert_eq!(d.score, 0.0);
    }

    #[test]
    fn route_decision_snapshot() {
        // Snapshot a populated RouteDecision — covers all fields including
        // selected_skill_path (Some/None) and non-empty reasons.
        let d = RouteDecision {
            selected_skill: "code-review".to_string(),
            selected_skill_path: Some("skills/code-review/skill.md".to_string()),
            reasons: vec![
                "matched by routing rules: intent=review".to_string(),
                "high confidence match (score=0.95)".to_string(),
            ],
            score: 0.95,
        };
        insta::assert_debug_snapshot!(d);
    }

    #[test]
    fn mcp_pre_guard_verdict_default() {
        let v = McpPreGuardVerdict::default();
        assert!(!v.blocked);
        assert!(v.reason.is_none());
    }

    #[test]
    fn constants_values() {
        assert_eq!(MAX_CONCURRENT_SUBAGENTS_LIMIT, 24);
        assert!(RFV_EXTERNAL_RESEARCH_SCHEMA_REL_PATH.ends_with(".json"));
    }

    #[test]
    fn mirror_host_enums_cover_canonical_hosts() {
        // Ensure HookObservationHost covers all formal hosts from RUNTIME_REGISTRY
        let canonical = framework_kernel::runtime_registry::HOST_HOME_DIRS;
        for host_dir in canonical {
            let host_id = host_dir.strip_prefix('.').unwrap_or(host_dir);
            assert!(
                HookObservationHost::from_host_id(host_id).is_some(),
                "HookObservationHost missing variant for canonical host: {host_id}"
            );
        }
        // Ensure PaperProseHookHost covers the same set
        for host_dir in canonical {
            let host_id = host_dir.strip_prefix('.').unwrap_or(host_dir);
            assert!(
                PaperProseHookHost::from_host_id(host_id).is_some(),
                "PaperProseHookHost missing variant for canonical host: {host_id}"
            );
        }
    }
}
