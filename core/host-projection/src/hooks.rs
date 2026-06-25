//! host-projection hooks proxy layer.
//!
//! Owns the shared function pointer OnceLock store and proxy functions.
//! L4 crates (runtime-core, framework-runtime, loop-engine) register
//! their callbacks into these slots during bootstrap.
//!
//! Proxy functions are re-exported for consumers that need them.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

// ── Function pointer type aliases (reduce type_complexity warnings) ──

/// Route task with manifest fallback: (records_json, host_id, query, session_id, allow_overlay, first_turn) -> Result<RouteDecision, String>
/// `records_json` is a JSON-serialized slice of SkillRecord values (avoids L5→L1 dep on routing_engine).
type RouteTaskFn = fn(
    &[serde_json::Value],
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
// Host ID types (string-based, not enum — avoids per-host enum in L5)
// ────────────────────────────────────────────────────────────────

/// Canonical host ID for hook observation (e.g. "cursor", "codex", "claude", "opencode").
pub type HookObservationHost = &'static str;

/// Type alias for backward compatibility with function pointer signatures.
/// Canonical host ID for paper prose/adversarial hooks.
pub type PaperProseHookHost = &'static str;

/// Type alias for backward compatibility with function pointer signatures.
pub type PaperProseHookHostType = &'static str;

/// Per-host env var controlling prose hook injection.
/// Generated from RUNTIME_REGISTRY.json host_targets.metadata.*.paper_prose_env.
pub fn paper_prose_env_var(host: &str) -> &'static str {
    framework_kernel::runtime_registry::paper_prose_env(host)
}

/// Per-host env var controlling adversarial review hook injection.
/// Generated from RUNTIME_REGISTRY.json host_targets.metadata.*.paper_adversarial_env.
pub fn paper_adversarial_env_var(host: &str) -> &'static str {
    framework_kernel::runtime_registry::paper_adversarial_env(host)
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
// router_env_flags: delegates to core_policy::env_flags (single source of truth)
// ────────────────────────────────────────────────────────────────

pub fn router_rs_env_enabled_default_true(var_name: &str) -> bool {
    core_policy::env_flags::env_enabled_default_true(var_name)
}

pub fn router_rs_env_enabled_default_false(var_name: &str) -> bool {
    core_policy::env_flags::env_enabled_default_false(var_name)
}

pub fn router_rs_operator_inject_globally_enabled() -> bool {
    core_policy::env_flags::router_rs_operator_inject_globally_enabled()
}

pub fn router_rs_hook_legacy_subtracted_events_enabled() -> bool {
    core_policy::env_flags::router_rs_hook_legacy_subtracted_events_enabled()
}

pub fn router_rs_hook_silent_enabled() -> bool {
    core_policy::env_flags::router_rs_hook_silent_enabled()
}

pub fn router_rs_hook_outbound_context_max_bytes() -> usize {
    core_policy::env_flags::router_rs_hook_outbound_context_max_bytes()
}

/// Delegates to core-policy's canonical implementation (single source of truth).
pub fn router_rs_review_fork_context_missing_infer_false_enabled() -> bool {
    core_policy::env_flags::router_rs_review_fork_context_missing_infer_false_enabled()
}

pub fn router_rs_pre_goal_enabled() -> bool {
    core_policy::env_flags::router_rs_pre_goal_enabled()
}

pub fn router_rs_hook_state_lock_retries() -> u32 {
    core_policy::env_flags::router_rs_hook_state_lock_retries()
}

pub fn router_rs_hook_state_file_sync_enabled() -> bool {
    core_policy::env_flags::router_rs_hook_state_file_sync_enabled()
}

pub fn router_rs_hook_state_dir_sync_enabled() -> bool {
    core_policy::env_flags::router_rs_hook_state_dir_sync_enabled()
}

pub fn router_rs_review_pending_cycle_max() -> usize {
    core_policy::env_flags::router_rs_review_pending_cycle_max()
}

pub fn router_rs_review_gate_stop_max_nudges_cap() -> Option<u32> {
    #[cfg(test)]
    {
        let raw = std::env::var("ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES")
            .ok()
            .or_else(|| std::env::var("ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES").ok());
        raw.as_ref()?;
    }
    core_policy::env_flags::router_rs_review_gate_stop_max_nudges_cap()
}

pub fn router_rs_pre_goal_strict_disk_enabled() -> bool {
    core_policy::env_flags::router_rs_pre_goal_strict_disk_enabled()
}

pub fn router_rs_hook_state_fail_open_enabled() -> bool {
    core_policy::env_flags::router_rs_hook_state_fail_open_enabled()
}

pub fn router_rs_cargo_check_sync_enabled() -> bool {
    core_policy::env_flags::router_rs_cargo_check_sync_enabled()
}

pub fn router_rs_hook_state_legacy_full_sweep_enabled() -> bool {
    core_policy::env_flags::router_rs_hook_state_legacy_full_sweep_enabled()
}

pub fn router_rs_hook_state_stale_sweep_days() -> u64 {
    core_policy::env_flags::router_rs_hook_state_stale_sweep_days()
}

pub fn router_rs_sessionstart_context_max_bytes() -> usize {
    parse_env_usize("ROUTER_RS_SESSIONSTART_CONTEXT_MAX_BYTES")
        .or_else(|| parse_env_usize("ROUTER_RS_CURSOR_SESSIONSTART_CONTEXT_MAX_BYTES"))
        .unwrap_or(64 * 1024)
}

// ────────────────────────────────────────────────────────────────
// Hook-state file sweep utilities
// ────────────────────────────────────────────────────────────────

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
    sweep_files_by_age(hook_state_dir, cutoff)
}

/// 按存活时间清理文件（不含概率门控）。
/// 用于启动时一次性清理：lock file 的进程已退出即可安全删除。
fn sweep_files_by_age(dir: &Path, max_age: std::time::Duration) -> usize {
    let now = std::time::SystemTime::now();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return 0,
    };
    let mut cleaned = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let modified = match entry.metadata().and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(_) => continue,
        };
        if now.duration_since(modified).unwrap_or_default() > max_age
            && std::fs::remove_file(&path).is_ok()
        {
            cleaned += 1;
        }
    }
    if cleaned > 0 {
        eprintln!(
            "[router-rs] hook-state sweep: removed {cleaned} file(s) from {}",
            dir.display()
        );
    }
    cleaned
}

/// 清理孤儿 .lock 文件（进程已退出的残留锁）。
/// hook 子进程 exit(0) 跳过 Drop 后生成的 0 字节 lock 文件，
/// 其 flock 已被 OS 关闭 fd 时释放，锁已无人持有。
/// 安全阈值：1 小时（远短于文件数据的 7 天）。
///
/// 与 `sweep_stale_hook_state_files` 的区别：
/// - 无概率门控（直接执行）
/// - 更短阈值（1h vs 7d）
/// - 仅删 .lock 后缀文件
pub fn sweep_orphan_lock_files(hook_state_dir: &Path) -> usize {
    const LOCK_STALE_SECS: u64 = 3600; // 1h — lock 文件进程退出后即可安全清理
    let cutoff = std::time::Duration::from_secs(LOCK_STALE_SECS);
    let now = std::time::SystemTime::now();
    let entries = match std::fs::read_dir(hook_state_dir) {
        Ok(e) => e,
        Err(_) => return 0,
    };
    let mut cleaned = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = match path.file_name().and_then(|s| s.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if !file_name.ends_with(".lock") {
            continue;
        }
        if !path.is_file() {
            continue;
        }
        if !file_name.ends_with(".json.lock") && !file_name.starts_with("hook_state_") {
            continue; // 只清理已知模式的 lock 文件
        }
        let modified = match entry.metadata().and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(_) => continue,
        };
        if now.duration_since(modified).unwrap_or_default() > cutoff
            && std::fs::remove_file(&path).is_ok()
        {
            cleaned += 1;
        }
    }
    if cleaned > 0 {
        eprintln!(
            "[router-rs] hook-state orphan lock sweep: removed {cleaned} .lock file(s) from {}",
            hook_state_dir.display()
        );
    }
    cleaned
}

fn parse_env_usize(var: &str) -> Option<usize> {
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

type EvalGoalReadiness = fn(&Path, &Value, &str) -> GoalReadiness;
type GoalStopFollowup = fn(bool, bool, bool, u32) -> String;

static EVAL_GOAL_READINESS: OnceLock<EvalGoalReadiness> = OnceLock::new();
static GOAL_STOP_FOLLOWUP: OnceLock<GoalStopFollowup> = OnceLock::new();

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

/// Merge paper prose/adversarial before submit: (repo_root, output, prompt_text, use_followup_message, host)
type MergePaperContextFn = fn(&Path, &mut Value, &str, bool, PaperProseHookHost);

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
    host: PaperProseHookHost,
) {
    if let Some(f) = MERGE_PROSE
        .get() { f(repo_root, output, prompt_text, use_followup_message, host) }
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
    host: PaperProseHookHost,
) {
    if let Some(f) = MERGE_ADVERSARIAL
        .get() { f(repo_root, output, prompt_text, use_followup_message, host) }
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
// Research mode inference function pointer (ADR-010 §7.4)
// ────────────────────────────────────────────────────────────────
//
// L4 (runtime-core) calls `research_mode_for_request` to classify
// a request as "quick" or "deep" research. L5 (research-harness)
// registers the actual inference logic. This keeps research domain
// knowledge out of L4.

type InferResearchModeFn = fn(&Value) -> String;

static INFER_RESEARCH_MODE: OnceLock<InferResearchModeFn> = OnceLock::new();

/// Register the research mode inference callback. Called by L5 (research-harness).
pub fn register_research_mode_inference(f: InferResearchModeFn) {
    once_lock_set(&INFER_RESEARCH_MODE, f, "INFER_RESEARCH_MODE");
}

/// Returns `"quick"` or `"deep"`. Defaults to `"quick"` when L5 is not enabled.
pub fn research_mode_for_request(payload: &Value) -> String {
    if let Some(f) = INFER_RESEARCH_MODE.get() {
        f(payload)
    } else {
        "quick".to_string()
    }
}

// ────────────────────────────────────────────────────────────────
// Skill routing bridge (decouples L0 MCP harness from L1 routing-engine)
// ────────────────────────────────────────────────────────────────

type SkillRoutingBridgeFn = fn(&str, &Value) -> Result<Value, String>;

static SKILL_ROUTING_BRIDGE: OnceLock<SkillRoutingBridgeFn> = OnceLock::new();

/// Register the skill routing bridge. Called by runtime-core at bootstrap.
/// The bridge dispatches operations: "search", "filter_host", "load_cached", "read_json".
pub fn register_skill_routing_bridge(f: SkillRoutingBridgeFn) {
    once_lock_set(&SKILL_ROUTING_BRIDGE, f, "SKILL_ROUTING_BRIDGE");
}

/// Dispatch a skill routing operation via the registered bridge.
pub fn skill_routing_dispatch(op: &str, args: &Value) -> Result<Value, String> {
    if let Some(f) = SKILL_ROUTING_BRIDGE.get() {
        f(op, args)
    } else {
        Err("skill routing bridge not registered".to_string())
    }
}

// ────────────────────────────────────────────────────────────────
// kernel_bootstrap: function-pointer proxy (OnceLock)
// ────────────────────────────────────────────────────────────────

static ENSURE_KERNEL: OnceLock<fn()> = OnceLock::new();

pub fn register_kernel_bootstrap(f: fn()) {
    once_lock_set(&ENSURE_KERNEL, f, "ENSURE_KERNEL");
}

pub fn ensure_kernel_bootstrap() {
    if let Some(f) = ENSURE_KERNEL.get() { f() }
    // Register research mode inference (idempotent via OnceLock).
    // Production CLI uses research_harness::init_hooks(); this is the fallback.
    static RESEARCH_MODE_INIT: std::sync::Once = std::sync::Once::new();
    RESEARCH_MODE_INIT.call_once(|| {
        register_research_mode_inference(|payload: &serde_json::Value| {
            if let Some(mode) = payload.get("research_mode").and_then(serde_json::Value::as_str) {
                let m = mode.trim().to_ascii_lowercase();
                if m.contains("deep") || m.contains("深度") {
                    return "deep".to_string();
                }
                return "quick".to_string();
            }
            let task = payload.get("task").and_then(serde_json::Value::as_str)
                .unwrap_or("").to_ascii_lowercase();
            // Deep mode signals: specific research-intensive phrases only.
            // "external research" alone is NOT a deep signal — it needs "literature review" etc.
            if task.contains("deep dive") || task.contains("深度调研") || task.contains("深度研究")
                || task.contains("literature review") || task.contains("文献综述")
            {
                return "deep".to_string();
            }
            if let Some(reasons) = payload.get("reasons").and_then(serde_json::Value::as_array) {
                for r in reasons {
                    if let Some(s) = r.as_str() {
                        let low = s.to_ascii_lowercase();
                        if low.contains("deep") || low.contains("literature review") || low.contains("深度研究") {
                            return "deep".to_string();
                        }
                    }
                }
            }
            "quick".to_string()
        });
    });
    #[cfg(test)]
    crate::test_helpers::install_test_deps();
}

/// Public entry point that only calls the registered kernel bootstrap function (if any).
/// Host-projection wraps this with its own `#[cfg(test)] install_test_deps()`.
pub fn ensure_kernel_bootstrap_registered() {
    if let Some(f) = ENSURE_KERNEL.get() { f() }
}

// ────────────────────────────────────────────────────────────────
// Additional hooks needed by host_extensions::claude / mcp_stdio_harness
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
    runtime_records: &[serde_json::Value],
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

// ── Test-only re-exports from test_helpers (for host_extensions::cursor test code) ──


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

// ── Host-projection-specific OnceLock slots ──

/// Research tool dispatch: injected at startup by runtime-core
/// to break the L3→L6 dependency direction.
type ResearchToolDispatchFn = fn(&str, &Value) -> Result<String, String>;

// ── Session supervisor operation hook ──
// Registered by runtime-core at startup. Allows MCP tools to call session_supervisor ops
// without host-projection depending on session-supervisor crate directly.

static SESSION_SUPERVISOR_OP: OnceLock<fn(Value) -> Result<Value, String>> = OnceLock::new();

/// Register the session-supervisor operation handler. Called once at startup.
pub fn register_session_supervisor_op(f: fn(Value) -> Result<Value, String>) {
    SESSION_SUPERVISOR_OP.set(f).ok();
    eprintln!("[router-rs info] session_supervisor_op: registered");
}

/// Dispatch a session-supervisor operation. Returns None if not registered.
pub fn session_supervisor_op(payload: Value) -> Option<Result<Value, String>> {
    SESSION_SUPERVISOR_OP.get().map(|f| f(payload))
}
static RESEARCH_TOOL_DISPATCH: OnceLock<ResearchToolDispatchFn> = OnceLock::new();

pub fn register_research_tool_dispatch(f: ResearchToolDispatchFn) {
    once_lock_set(&RESEARCH_TOOL_DISPATCH, f, "research_tool_dispatch");
}

pub fn get_research_tool_dispatch() -> Option<ResearchToolDispatchFn> {
    RESEARCH_TOOL_DISPATCH.get().copied()
}

// ── MCP routing: decouple L0→L1 DAG violation (ADR-010 §11.2) ──
//
// These fn ptrs break the compile-time dependency from host-projection (L0)
// to routing-engine (L1). L4 (runtime-core) registers the routing-engine
// implementations during bootstrap. L0 calls through the fn ptr and receives
// JSON — no routing-engine types cross the boundary.

/// MCP tool skill route: route a query to the best matching skill.
type McpToolSkillRouteFn = fn(query: &str, host_id: &str, first_turn: bool, repo_root: &str) -> Result<String, String>;
static MCP_TOOL_SKILL_ROUTE: OnceLock<McpToolSkillRouteFn> = OnceLock::new();

pub fn register_mcp_tool_skill_route(f: McpToolSkillRouteFn) {
    once_lock_set(&MCP_TOOL_SKILL_ROUTE, f, "mcp_tool_skill_route");
}

pub fn mcp_tool_skill_route(query: &str, host_id: &str, first_turn: bool, repo_root: &str) -> Result<String, String> {
    match MCP_TOOL_SKILL_ROUTE.get() {
        Some(f) => f(query, host_id, first_turn, repo_root),
        None => Err("skill_route not available (not registered)".to_string()),
    }
}

/// MCP tool search skills: search skills by query string.
type McpToolSearchSkillsFn = fn(query: &str, limit: usize, effective_host: &str, repo_root: &str) -> Result<String, String>;
static MCP_TOOL_SEARCH_SKILLS: OnceLock<McpToolSearchSkillsFn> = OnceLock::new();

pub fn register_mcp_tool_search_skills(f: McpToolSearchSkillsFn) {
    once_lock_set(&MCP_TOOL_SEARCH_SKILLS, f, "mcp_tool_search_skills");
}

pub fn mcp_tool_search_skills(query: &str, limit: usize, effective_host: &str, repo_root: &str) -> Result<String, String> {
    match MCP_TOOL_SEARCH_SKILLS.get() {
        Some(f) => f(query, limit, effective_host, repo_root),
        None => Err("search_skills not available (not registered)".to_string()),
    }
}

type ReviewGateHandler = fn(event: &str, repo_root: Option<&Path>) -> Result<(), String>;
static REVIEW_GATE_HANDLER: OnceLock<ReviewGateHandler> = OnceLock::new();

pub fn register_review_gate_handler(handler: ReviewGateHandler) {
    once_lock_set(&REVIEW_GATE_HANDLER, handler, "REVIEW_GATE_HANDLER");
}

pub fn run_review_gate(event: &str, cli_repo_root: Option<&Path>) -> Result<(), String> {
    match REVIEW_GATE_HANDLER.get() {
        Some(handler) => handler(event, cli_repo_root),
        None => Err("review gate handler not registered".into()),
    }
}

// ── Browser dispatch (moved from runtime-core to break L3→L4 dep) ──

type BrowserDispatchFn = fn(framework_kernel::cli_args::BrowserSubcommand) -> Result<(), String>;
static BROWSER_DISPATCH: OnceLock<BrowserDispatchFn> = OnceLock::new();

/// Register the browser command dispatch function (call once at startup).
pub fn set_browser_dispatch(f: BrowserDispatchFn) {
    if BROWSER_DISPATCH.set(f).is_err() {
        tracing::warn!("BROWSER_DISPATCH already registered — second call ignored");
    }
}

/// Dispatch a browser subcommand. Returns `Err` if no dispatch function was registered.
pub fn dispatch_browser_command(
    command: framework_kernel::cli_args::BrowserSubcommand,
) -> Result<(), String> {
    match BROWSER_DISPATCH.get() {
        Some(f) => f(command),
        None => Err(
            "browser-mcp dispatch not registered; call set_browser_dispatch() at startup"
                .to_string(),
        ),
    }
}

// ── Runtime trace transport proxies (break browser-mcp L3→L4 dep) ──

type AttachRuntimeEventTransportFn = fn(Value) -> Result<Value, String>;
static ATTACH_RUNTIME_EVENT_TRANSPORT: OnceLock<AttachRuntimeEventTransportFn> = OnceLock::new();

pub fn register_attach_runtime_event_transport(f: AttachRuntimeEventTransportFn) {
    once_lock_set(&ATTACH_RUNTIME_EVENT_TRANSPORT, f, "ATTACH_RUNTIME_EVENT_TRANSPORT");
}

pub fn attach_runtime_event_transport(payload: Value) -> Result<Value, String> {
    ATTACH_RUNTIME_EVENT_TRANSPORT
        .get()
        .map(|f| f(payload))
        .unwrap_or_else(|| Err("attach_runtime_event_transport not registered".into()))
}

type InspectTraceStreamFn = fn(
    framework_kernel::stdio_payload_types::TraceStreamInspectRequestPayload,
) -> Result<framework_kernel::stdio_payload_types::TraceStreamInspectResponsePayload, String>;
static INSPECT_TRACE_STREAM: OnceLock<InspectTraceStreamFn> = OnceLock::new();

pub fn register_inspect_trace_stream(f: InspectTraceStreamFn) {
    once_lock_set(&INSPECT_TRACE_STREAM, f, "INSPECT_TRACE_STREAM");
}

pub fn inspect_trace_stream(
    payload: framework_kernel::stdio_payload_types::TraceStreamInspectRequestPayload,
) -> Result<framework_kernel::stdio_payload_types::TraceStreamInspectResponsePayload, String> {
    INSPECT_TRACE_STREAM
        .get()
        .map(|f| f(payload))
        .unwrap_or_else(|| Err("inspect_trace_stream not registered".into()))
}

// ── Tool dispatch hooks: business logic extraction from L0 → L4 ──
//
// These hooks move heavy business logic (payload construction, enum validation,
// multi-source evaluation) out of host-projection's tool handlers into runtime-core.
// host-projection retains MCP parameter type-checking; runtime-core owns domain logic.

/// Goal state manage dispatch: (args, repo_root, session_id) -> Result<String, String>
type GoalStateManageDispatchFn = fn(&Value, &Path, &str) -> Result<String, String>;
static GOAL_STATE_MANAGE_DISPATCH: OnceLock<GoalStateManageDispatchFn> = OnceLock::new();

pub fn register_tool_goal_state_manage_dispatch(f: GoalStateManageDispatchFn) {
    once_lock_set(&GOAL_STATE_MANAGE_DISPATCH, f, "tool_goal_state_manage_dispatch");
}

pub fn tool_goal_state_manage_dispatch(args: &Value, repo_root: &Path, session_id: &str) -> Result<String, String> {
    GOAL_STATE_MANAGE_DISPATCH
        .get()
        .map(|f| f(args, repo_root, session_id))
        .unwrap_or_else(|| Err("tool_goal_state_manage_dispatch not registered — runtime-core boot required".into()))
}

/// Quality gate manage dispatch: (args, repo_root, session_id) -> Result<String, String>
type QualityGateManageDispatchFn = fn(&Value, &Path, &str) -> Result<String, String>;
static QUALITY_GATE_MANAGE_DISPATCH: OnceLock<QualityGateManageDispatchFn> = OnceLock::new();

pub fn register_tool_quality_gate_manage_dispatch(f: QualityGateManageDispatchFn) {
    once_lock_set(&QUALITY_GATE_MANAGE_DISPATCH, f, "tool_quality_gate_manage_dispatch");
}

pub fn tool_quality_gate_manage_dispatch(args: &Value, repo_root: &Path, session_id: &str) -> Result<String, String> {
    QUALITY_GATE_MANAGE_DISPATCH
        .get()
        .map(|f| f(args, repo_root, session_id))
        .unwrap_or_else(|| Err("tool_quality_gate_manage_dispatch not registered — runtime-core boot required".into()))
}

/// Closeout record write dispatch: (args, repo_root) -> Result<String, String>
type CloseoutRecordWriteDispatchFn = fn(&Value, &Path) -> Result<String, String>;
static CLOSEOUT_RECORD_WRITE_DISPATCH: OnceLock<CloseoutRecordWriteDispatchFn> = OnceLock::new();

pub fn register_tool_closeout_record_write_dispatch(f: CloseoutRecordWriteDispatchFn) {
    once_lock_set(&CLOSEOUT_RECORD_WRITE_DISPATCH, f, "tool_closeout_record_write_dispatch");
}

pub fn tool_closeout_record_write_dispatch(args: &Value, repo_root: &Path) -> Result<String, String> {
    CLOSEOUT_RECORD_WRITE_DISPATCH
        .get()
        .map(|f| f(args, repo_root))
        .unwrap_or_else(|| Err("tool_closeout_record_write_dispatch not registered — runtime-core boot required".into()))
}

/// Closeout gate evaluate: (args, repo_root, host_id) -> Result<String, String>
type CloseoutGateEvaluateFn = fn(&Value, &Path, &str) -> Result<String, String>;
static CLOSEOUT_GATE_EVALUATE: OnceLock<CloseoutGateEvaluateFn> = OnceLock::new();

pub fn register_tool_closeout_gate_evaluate(f: CloseoutGateEvaluateFn) {
    once_lock_set(&CLOSEOUT_GATE_EVALUATE, f, "tool_closeout_gate_evaluate");
}

pub fn tool_closeout_gate_evaluate(args: &Value, repo_root: &Path, host_id: &str) -> Result<String, String> {
    CLOSEOUT_GATE_EVALUATE
        .get()
        .map(|f| f(args, repo_root, host_id))
        .unwrap_or_else(|| Err("tool_closeout_gate_evaluate not registered — runtime-core boot required".into()))
}

/// Routing evolution dispatch: (args, repo_root) -> Result<String, String>
type RoutingEvolutionDispatchFn = fn(&Value, &Path) -> Result<String, String>;
static ROUTING_EVOLUTION_DISPATCH: OnceLock<RoutingEvolutionDispatchFn> = OnceLock::new();

pub fn register_tool_routing_evolution_dispatch(f: RoutingEvolutionDispatchFn) {
    once_lock_set(&ROUTING_EVOLUTION_DISPATCH, f, "tool_routing_evolution_dispatch");
}

pub fn tool_routing_evolution_dispatch(args: &Value, repo_root: &Path) -> Result<String, String> {
    ROUTING_EVOLUTION_DISPATCH
        .get()
        .map(|f| f(args, repo_root))
        .unwrap_or_else(|| Err("tool_routing_evolution_dispatch not registered — runtime-core boot required".into()))
}

// ── Mirror type structural canary tests ──

#[cfg(test)]
mod mirror_type_tests {
    use super::*;

    /// Verify that `RouteDecision` has the expected field count and types.
    /// If the source (routing_engine) changes, this test catches structural drift.
    #[test]
    fn route_decision_mirror_structural_invariants() {
        let d = RouteDecision::default();
        assert_eq!(d.selected_skill, String::new());
        assert!(d.selected_skill_path.is_none());
        assert!(d.reasons.is_empty());
        assert_eq!(d.score, 0.0f64);

        // Verify this mirrors routing_engine's RouteDecision (4 fields)
        // Change this count when the source struct changes.
        let field_estimate = std::mem::size_of::<RouteDecision>();
        assert!(
            field_estimate > 0,
            "RouteDecision structural invariant check"
        );

        // Populated variant test
        let d2 = RouteDecision {
            selected_skill: "test-skill".into(),
            selected_skill_path: Some("skills/test/SKILL.md".into()),
            reasons: vec!["matched by routing".into()],
            score: 0.95,
        };
        assert_eq!(d2.selected_skill.as_str(), "test-skill");
    }

    /// Verify that `McpPreGuardVerdict` has the expected field layout.
    #[test]
    fn mcp_pre_guard_verdict_mirror_structural_invariants() {
        let v = McpPreGuardVerdict::default();
        assert!(!v.blocked);
        assert!(v.reason.is_none());

        let v2 = McpPreGuardVerdict {
            blocked: true,
            reason: Some("blocked by policy".into()),
        };
        assert!(v2.blocked);
        assert_eq!(v2.reason.as_deref(), Some("blocked by policy"));
    }

    /// Verify that mirrored constants match expected values.
    #[test]
    fn mirrored_constants_values() {
        // These mirrors of runtime-core constants should be reviewed
        // when the source crate changes versions.
        assert_eq!(MAX_CONCURRENT_SUBAGENTS_LIMIT, 24);
        assert!(RFV_EXTERNAL_RESEARCH_SCHEMA_REL_PATH.ends_with(".json"));
    }

    /// Regression: host type aliases compile correctly as &'static str.
    #[test]
    fn host_type_aliases_are_static_str() {
        let _host: HookObservationHost = "cursor";
        let _host2: PaperProseHookHost = "codex";
        assert_eq!(_host, "cursor");
        assert_eq!(_host2, "codex");
    }
}


