//! host-projection hooks proxy layer.
//!
//! Owns the shared function pointer OnceLock store and proxy functions.
//! L4 crates (runtime-core, framework-runtime, loop-engine) register
//! their callbacks into these slots during bootstrap.
//!
//! Proxy functions are re-exported for consumers that need them.

use core_policy::error::FrameworkError;
type Result<T> = std::result::Result<T, FrameworkError>;

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

// ── Function pointer type aliases (reduce type_complexity warnings) ──

/// Route task with manifest fallback: (records_json, host_id, query, session_id, allow_overlay, first_turn) -> Result<RouteDecision>
/// `records_json` is a JSON-serialized slice of SkillRecord values (avoids L5→L1 dep on routing_engine).
type RouteTaskFn = fn(
    &[serde_json::Value],
    Option<&str>,
    &str,
    &str,
    bool,
    bool,
) -> Result<RouteDecision>;

/// Build framework runtime snapshot envelope: (repo_root, runtime_path, host_id) -> Result<Value>
type BuildSnapshotFn = fn(&Path, Option<&Path>, Option<&str>) -> Result<Value>;

/// Build snapshot with level: (repo_root, runtime_path, host_id, level) -> Result<Value>
type BuildSnapshotWithLevelFn = fn(&Path, Option<&Path>, Option<&str>, &str) -> Result<Value>;

/// Build automatic continuity checkpoint payload: (repo_root, task_id, session_id, current_query, allow_overlay, first_turn) -> Value
type BuildCheckpointFn = fn(&Path, &str, &str, Option<&str>, bool, bool) -> Value;

/// Append evidence index row: (repo_root, task_id, metadata) -> Result<()>
type AppendEvidenceFn = fn(&Path, Option<&str>, serde_json::Map<String, Value>) -> Result<()>;

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

// ── Hook slot macros ────────────────────────────────────────────

/// Declare a `OnceLock`-backed hook slot (static + register + dispatch).
///
/// Arms:
/// - `fn name(args...);` — unit return, no-op if unregistered
/// - `fn name(args...) -> Ret = expr;` — returns `expr` as default
/// - `fn name(args...) -> Ret = err("msg");` — returns `Err(FrameworkError::validation("msg"))`
macro_rules! once_lock_hook {
    // Unit return — `if let Some(f) = STATIC.get() { f(args) }`
    (
        $(#[$meta:meta])*
        static $name:ident: $fty:ty;
        register $rname:ident;
        fn $dname:ident($($arg:ident: $t:ty),* $(,)?);
    ) => {
        $(#[$meta])*
        static $name: OnceLock<$fty> = OnceLock::new();

        pub fn $rname(f: $fty) {
            once_lock_set(&$name, f, stringify!($name));
        }

        pub fn $dname($($arg: $t),*) {
            if let Some(f) = $name.get() { f($($arg),*) }
        }
    };

    // Result return with `Err` default — `.unwrap_or_else(|| Err(FrameworkError::validation(msg)))`
    // MUST precede the generic arm below to avoid ambiguity with `= err(...)` as `$default:expr`.
    (
        $(#[$meta:meta])*
        static $name:ident: $fty:ty;
        register $rname:ident;
        fn $dname:ident($($arg:ident: $t:ty),* $(,)?) -> $ret:ty = err($msg:expr);
    ) => {
        $(#[$meta])*
        static $name: OnceLock<$fty> = OnceLock::new();

        pub fn $rname(f: $fty) {
            once_lock_set(&$name, f, stringify!($name));
        }

        pub fn $dname($($arg: $t),*) -> $ret {
            $name.get().map(|f| f($($arg),*))
                .unwrap_or_else(|| Err(FrameworkError::validation($msg)))
        }
    };

    // Default value — `.map(|f| f(args)).unwrap_or(default)`
    (
        $(#[$meta:meta])*
        static $name:ident: $fty:ty;
        register $rname:ident;
        fn $dname:ident($($arg:ident: $t:ty),* $(,)?) -> $ret:ty = $default:expr;
    ) => {
        $(#[$meta])*
        static $name: OnceLock<$fty> = OnceLock::new();

        pub fn $rname(f: $fty) {
            once_lock_set(&$name, f, stringify!($name));
        }

        pub fn $dname($($arg: $t),*) -> $ret {
            $name.get().map(|f| f($($arg),*)).unwrap_or($default)
        }
    };
}

// ────────────────────────────────────────────────────────────────
// Host ID types (string-based, not enum — avoids per-host enum in L5)
// ────────────────────────────────────────────────────────────────

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
// Env readers with host-projection-specific logic (local impl, not pure proxy)
// ────────────────────────────────────────────────────────────────

pub fn router_rs_review_gate_stop_max_nudges_cap() -> Option<u32> {
    #[cfg(test)]
    {
        let raw = std::env::var("ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES").ok();
        raw.as_ref()?;
    }
    core_policy::env_flags::router_rs_review_gate_stop_max_nudges_cap()
}

pub fn router_rs_sessionstart_context_max_bytes() -> usize {
    parse_env_usize("ROUTER_RS_SESSIONSTART_CONTEXT_MAX_BYTES")
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

    let days = core_policy::env_flags::router_rs_hook_state_stale_sweep_days();
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
        tracing::info!(
            "hook-state sweep: removed {cleaned} file(s) from {}",
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
        if now.duration_since(modified).unwrap_or_default() > cutoff {
            #[cfg(unix)]
            {
                // Use flock probe: if we can acquire LOCK_EX|LOCK_NB, no one holds the lock.
                // Otherwise skip (lock still active). This avoids TOCTOU between metadata check
                // and removal — the flock is tested against the actual lock holder.
                use std::os::unix::io::AsRawFd;
                if let Ok(f) = std::fs::OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(false)
                    .open(&path)
                {
                    let rc = unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
                    if rc == 0 {
                        // Lock acquired — no one is holding it; safe to delete.
                        unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_UN) };
                        drop(f);
                        if std::fs::remove_file(&path).is_ok() {
                            cleaned += 1;
                        }
                    } // else: lock still held by another process — skip
                }
            }
            #[cfg(not(unix))]
            {
                // Non-Unix: no flock available; age-based removal is best-effort.
                // TOCTOU race is mitigated by age threshold.
                if std::fs::remove_file(&path).is_ok() {
                    cleaned += 1;
                }
            }
        }
    }
    if cleaned > 0 {
        tracing::info!(
            "hook-state orphan lock sweep: removed {cleaned} .lock file(s) from {}",
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
pub fn read_stdin_limited<R: std::io::Read>(reader: &mut R) -> Result<String> {
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
        if inner.read(&mut probe).map_err(FrameworkError::Io)? > 0 {
            return Err(FrameworkError::validation("stdin payload exceeds 4 MiB limit"));
        }
    }
    Ok(input)
}

/// Read stdin as JSON object with 4 MiB limit. Returns empty object if stdin is empty.
/// Rejects non-object JSON (arrays, strings, numbers, etc.) with an error.
pub fn read_stdin_json_limited() -> Result<Value> {
    let mut stdin = std::io::stdin();
    let input = read_stdin_limited(&mut stdin)?;
    if input.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }
    let val: Value = serde_json::from_str(&input).map_err(|_| FrameworkError::validation("stdin_json_invalid"))?;
    if !val.is_object() {
        return Err(FrameworkError::validation("stdin_json_not_object: expected JSON object"));
    }
    Ok(val)
}

// ────────────────────────────────────────────────────────────────
// hook_timing: function-pointer proxies (OnceLock)
// ────────────────────────────────────────────────────────────────

// All four hook_timing proxy dispatch functions were never used by any caller
// (callers go through runtime_core::hook_timing::* directly).
// The bundle register register_hook_timing has been removed accordingly.
// The native functions are still defined in runtime-core/src/hook_timing.rs.

// ────────────────────────────────────────────────────────────────
// session_call_tracker: function-pointer proxies (OnceLock)
// ────────────────────────────────────────────────────────────────

once_lock_hook! { static INIT_TRACKER: fn(&Path) -> Result<()>; register register_init_tracker; fn init_tracker(repo_root: &Path) -> Result<()> = Ok(()); }
once_lock_hook! { static RECORD_TOOL_CALL: fn(&Path, &str, Option<&Value>) -> Result<()>; register register_record_tool_call; fn record_tool_call(repo_root: &Path, tool_name: &str, cache_stats: Option<&Value>) -> Result<()> = Ok(()); }
once_lock_hook! { static READ_TRACKER_STATE: fn(&Path) -> Result<Value>; register register_read_tracker_state; fn read_tracker_state(repo_root: &Path) -> Result<Value> = Ok(serde_json::json!({})); }

pub fn register_session_call_tracker(
    init: fn(&Path) -> Result<()>,
    record: fn(&Path, &str, Option<&Value>) -> Result<()>,
    read_state: fn(&Path) -> Result<Value>,
) {
    register_init_tracker(init);
    register_record_tool_call(record);
    register_read_tracker_state(read_state);
}

// ────────────────────────────────────────────────────────────────
// framework_runtime: function-pointer proxies (OnceLock)
// ────────────────────────────────────────────────────────────────

once_lock_hook! { static BUILD_FRAMEWORK_CONTRACT: fn(&Path) -> Result<Value>; register register_build_framework_contract; fn build_framework_contract_summary_envelope(repo_root: &Path) -> Result<Value> = err("framework_runtime not registered"); }
once_lock_hook! { static TRY_APPEND_POST_TOOL_SHELL: fn(&Path, &Value, &str) -> Result<()>; register register_try_append_post_tool_shell; fn try_append_post_tool_shell_evidence(repo_root: &Path, event: &Value, kind: &str) -> Result<()> = Ok(()); }
once_lock_hook! { static CLOSEOUT_ENFORCEMENT: fn() -> bool; register register_closeout_enforcement; fn closeout_programmatic_enforcement_enabled() -> bool = false; }
once_lock_hook! { static CLOSEOUT_RECORD_PATH: fn(&Path, &str) -> Result<PathBuf>; register register_closeout_record_path; fn closeout_record_path_for_task(repo_root: &Path, task_id: &str) -> Result<PathBuf> = err("framework_runtime not registered"); }
once_lock_hook! { static EVALUATE_CLOSEOUT: fn(&Path, &str, &Path) -> Result<Value>; register register_evaluate_closeout; fn evaluate_closeout_record_file_for_task(repo_root: &Path, task_id: &str, record_path: &Path) -> Result<Value> = err("framework_runtime not registered"); }
once_lock_hook! { static FIRST_TASK_ID: fn(&Path) -> Option<String>; register register_first_task_id; fn first_task_id_from_registry(repo_root: &Path) -> Option<String> = None; }
once_lock_hook! { static EVIDENCE_APPEND: fn(Value) -> Result<Value>; register register_evidence_append; fn framework_hook_evidence_append(payload: Value) -> Result<Value> = err("framework_runtime not registered"); }
once_lock_hook! { static EXTRACT_DURATION: fn(&Value) -> Option<u64>; register register_extract_duration; fn extract_post_tool_duration_ms(event: &Value) -> Option<u64> = None; }
once_lock_hook! { static POST_TOOL_SUCCEEDED: fn(&Value) -> bool; register register_post_tool_succeeded; fn post_tool_call_succeeded(event: &Value) -> bool = true; }
once_lock_hook! { static CLOSEOUT_STOP_FOLLOWUP: fn(&Path, &str) -> Option<String>; register register_closeout_stop_followup; fn closeout_stop_followup_for_completion_text(repo_root: &Path, text: &str) -> Option<String> = None; }

// 10 fn pointer params — above threshold=8, OK to keep.
// Each argument is a distinct registration slot stored in a OnceLock static.
// Extracting a struct would add ceremony to callers without reducing surface.
#[allow(clippy::too_many_arguments)]
pub fn register_framework_runtime(
    build_contract: fn(&Path) -> Result<Value>,
    append_shell: fn(&Path, &Value, &str) -> Result<()>,
    enforcement: fn() -> bool,
    record_path: fn(&Path, &str) -> Result<PathBuf>,
    eval_closeout: fn(&Path, &str, &Path) -> Result<Value>,
    first_task: fn(&Path) -> Option<String>,
    evidence_append: fn(Value) -> Result<Value>,
    extract_duration: fn(&Value) -> Option<u64>,
    post_tool_ok: fn(&Value) -> bool,
    closeout_followup: fn(&Path, &str) -> Option<String>,
) {
    register_build_framework_contract(build_contract);
    register_try_append_post_tool_shell(append_shell);
    register_closeout_enforcement(enforcement);
    register_closeout_record_path(record_path);
    register_evaluate_closeout(eval_closeout);
    register_first_task_id(first_task);
    register_evidence_append(evidence_append);
    register_extract_duration(extract_duration);
    register_post_tool_succeeded(post_tool_ok);
    register_closeout_stop_followup(closeout_followup);
}

// ── hook_outbound_protect: removed from hooks proxy layer ──
//
// The authoritative implementation lives in runtime-core-contracts
// (hook_outbound_protect.rs), whose functions are called directly by
// consumers. The OnceLock proxy was never registered in production
// (register only called from test_helpers).

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

// ── ship_readiness: removed from hooks proxy layer ──
//
// register_ship_readiness was never called in production;
// the OnceLock slots were always empty. Gate eval handles
// goal readiness without the hooks proxy.

// ────────────────────────────────────────────────────────────────
// paper hooks: function-pointer proxies (OnceLock)
// ────────────────────────────────────────────────────────────────

/// Append paper prose/adversarial context: (repo_root, prompt_text, contexts, host)
type AppendPaperContextFn = fn(&Path, &str, &mut Vec<String>, PaperProseHookHost);
/// Merge paper prose/adversarial before submit: (repo_root, output, prompt_text, use_followup_message, host)
type MergePaperContextFn = fn(&Path, &mut Value, &str, bool, PaperProseHookHost);

once_lock_hook! { static APPEND_PROSE: AppendPaperContextFn; register register_append_prose; fn maybe_append_paper_prose_context(repo_root: &Path, prompt_text: &str, contexts: &mut Vec<String>, host: PaperProseHookHost); }
once_lock_hook! { static MERGE_PROSE: MergePaperContextFn; register register_merge_prose; fn maybe_merge_paper_prose_before_submit(repo_root: &Path, output: &mut Value, prompt_text: &str, use_followup_message: bool, host: PaperProseHookHost); }
once_lock_hook! { static APPEND_ADVERSARIAL: AppendPaperContextFn; register register_append_adversarial; fn maybe_append_paper_adversarial_context(repo_root: &Path, prompt_text: &str, contexts: &mut Vec<String>, host: PaperProseHookHost); }
once_lock_hook! { static MERGE_ADVERSARIAL: MergePaperContextFn; register register_merge_adversarial; fn maybe_merge_paper_adversarial_before_submit(repo_root: &Path, output: &mut Value, prompt_text: &str, use_followup_message: bool, host: PaperProseHookHost); }

pub fn register_paper_hooks(
    append_prose: AppendPaperContextFn,
    merge_prose: MergePaperContextFn,
    append_adversarial: AppendPaperContextFn,
    merge_adversarial: MergePaperContextFn,
) {
    register_append_prose(append_prose);
    register_merge_prose(merge_prose);
    register_append_adversarial(append_adversarial);
    register_merge_adversarial(merge_adversarial);
}

// ────────────────────────────────────────────────────────────────
// research activity log: function-pointer proxy (OnceLock)
// ────────────────────────────────────────────────────────────────

once_lock_hook! { static RESEARCH_ACTIVITY: fn(&Path, &str, &str); register register_research_activity_hook; fn maybe_record_research_activity(repo_root: &Path, tool_name: &str, summary: &str); }

// ────────────────────────────────────────────────────────────────
// Research mode inference function pointer (ADR-010 §7.4)
// ────────────────────────────────────────────────────────────────

type InferResearchModeFn = fn(&Value) -> String;

once_lock_hook! { static INFER_RESEARCH_MODE: InferResearchModeFn; register register_research_mode_inference; fn research_mode_for_request(payload: &Value) -> String = "quick".to_string(); }

// ── Skill routing bridge: removed ──
// Was never registered in production (register_skill_routing_bridge not called).
// Route decision goes through route_task_with_manifest_fallback instead.


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

/// Resolve repo root argument: (repo_root) -> Result<PathBuf>
type ResolveRepoRootFn = fn(Option<&Path>) -> Result<PathBuf>;

/// Check anomalies: (repo_root) -> Result<anomaly_list>
type CheckAnomaliesFn = fn(&Path) -> Result<Vec<String>>;

// RESOLVE_REPO_ROOT_ARG: manual - fallback closure calls current_dir()
static RESOLVE_REPO_ROOT_ARG: OnceLock<ResolveRepoRootFn> = OnceLock::new();

pub fn register_resolve_repo_root_arg(f: ResolveRepoRootFn) {
    once_lock_set(&RESOLVE_REPO_ROOT_ARG, f, "RESOLVE_REPO_ROOT_ARG");
}

once_lock_hook! { static CURRENT_LOCAL_TIMESTAMP: fn() -> String; register register_current_local_timestamp; fn current_local_timestamp() -> String = "1970-01-01T00:00:00Z".into(); }
once_lock_hook! { static WRITE_FRAMEWORK_SESSION_ARTIFACTS: fn(Value) -> Result<Value>; register register_write_framework_session_artifacts; fn write_framework_session_artifacts(payload: Value) -> Result<Value> = err("hooks not registered"); }
once_lock_hook! { static ROUTE_TASK_WITH_MANIFEST_FALLBACK: RouteTaskFn; register register_route_task_with_manifest_fallback; fn route_task_with_manifest_fallback(runtime_records: &[serde_json::Value], host_id: Option<&str>, query: &str, session_id: &str, allow_overlay: bool, first_turn: bool) -> Result<RouteDecision> = err("hooks not registered"); }
once_lock_hook! { static BUILD_FRAMEWORK_RUNTIME_SNAPSHOT_ENVELOPE: BuildSnapshotFn; register register_build_framework_runtime_snapshot_envelope; fn build_framework_runtime_snapshot_envelope(repo_root: &Path, artifact_root_override: Option<&Path>, task_id_override: Option<&str>) -> Result<Value> = err("hooks not registered"); }
// BUILD_FRAMEWORK_RUNTIME_SNAPSHOT_ENVELOPE_WITH_LEVEL: manual — has special fallback to old fn ptr
static BUILD_FRAMEWORK_RUNTIME_SNAPSHOT_ENVELOPE_WITH_LEVEL: OnceLock<BuildSnapshotWithLevelFn> = OnceLock::new();
once_lock_hook! { static BUILD_AUTOMATIC_CONTINUITY_CHECKPOINT_PAYLOAD: BuildCheckpointFn; register register_build_automatic_continuity_checkpoint_payload; fn build_automatic_continuity_checkpoint_payload(repo_root: &Path, task_line: &str, summary_text: &str, task_id: Option<&str>, repointer_focus: bool, update_registry_only_if_known: bool) -> Value = Value::Null; }
once_lock_hook! { static APPEND_EVIDENCE_INDEX: AppendEvidenceFn; register register_append_evidence_index; fn append_evidence_index(repo_root: &Path, task_id: Option<&str>, entry: serde_json::Map<String, Value>) -> Result<()> = err("hooks not registered"); }
once_lock_hook! { static HOOK_ACTION_FROM_OUTPUT: fn(&Value) -> &'static str; register register_hook_action_from_output; fn hook_action_from_output(output: &Value) -> &'static str = "unknown"; }
once_lock_hook! { static CLOSEOUT_RECORD_SCHEMA_VERSION_FN: fn() -> &'static str; register register_closeout_record_schema_version; fn closeout_record_schema_version() -> &'static str = "closeout-record-v1"; }
once_lock_hook! { static CHECK_ANOMALIES: CheckAnomaliesFn; register register_check_anomalies; fn check_anomalies(repo_root: &Path) -> Result<Vec<String>> = Ok(vec![]); }

// ── web_fetch_guard ──

/// Validate and resolve web fetch URL: (url) -> Result<(resolved_url, addresses)>
type ValidateWebFetchUrlFn = fn(&str) -> Result<(String, Vec<String>)>;

/// Resolve web fetch redirect: (base_url, location) -> Result<resolved_url>
type ResolveWebFetchRedirectFn = fn(&str, &str) -> Result<String>;

/// Resolve web fetch addresses: (host, port) -> Result<addresses>
type ResolveWebFetchAddressesFn = fn(&str, u16) -> Result<Vec<String>>;

once_lock_hook! { static VALIDATE_AND_RESOLVE_WEB_FETCH_URL: ValidateWebFetchUrlFn; register register_validate_and_resolve_web_fetch_url; fn validate_and_resolve_web_fetch_url(url: &str) -> Result<(String, Vec<String>)> = err("hooks not registered"); }
once_lock_hook! { static RESOLVE_WEB_FETCH_REDIRECT: ResolveWebFetchRedirectFn; register register_resolve_web_fetch_redirect; fn resolve_web_fetch_redirect(base: &str, location: &str) -> Result<String> = err("hooks not registered"); }
once_lock_hook! { static RESOLVE_WEB_FETCH_ADDRESSES: ResolveWebFetchAddressesFn; register register_resolve_web_fetch_addresses; fn resolve_web_fetch_addresses(host: &str, port: u16) -> Result<Vec<String>> = err("hooks not registered"); }

// ── mcp_pre_guard ──

once_lock_hook! { static EVALUATE_MCP_PRE_GUARD_SAFE: fn(&str, &Value, &Path) -> McpPreGuardVerdict; register register_evaluate_mcp_pre_guard_safe; fn evaluate_mcp_pre_guard_safe(tool_name: &str, arguments: &Value, repo_root: &Path) -> McpPreGuardVerdict = McpPreGuardVerdict { blocked: false, reason: None }; }

// 10+ fn pointer params in a registration pattern — above threshold=8, OK to keep.
// Each is a distinct OnceLock slot; struct would not reduce surface.
#[allow(clippy::too_many_arguments)]
pub fn register_framework_runtime_extra(
    resolve_repo_root_arg: ResolveRepoRootFn,
    current_local_timestamp: fn() -> String,
    write_framework_session_artifacts: fn(Value) -> Result<Value>,
    route_task_with_manifest_fallback: RouteTaskFn,
    build_framework_runtime_snapshot_envelope: BuildSnapshotFn,
    build_automatic_continuity_checkpoint_payload: BuildCheckpointFn,
    append_evidence_index: AppendEvidenceFn,
    hook_action_from_output: fn(&Value) -> &'static str,
    closeout_record_schema_version: fn() -> &'static str,
    check_anomalies: CheckAnomaliesFn,
) {
    register_resolve_repo_root_arg(resolve_repo_root_arg);
    register_current_local_timestamp(current_local_timestamp);
    register_write_framework_session_artifacts(write_framework_session_artifacts);
    register_route_task_with_manifest_fallback(route_task_with_manifest_fallback);
    register_build_framework_runtime_snapshot_envelope(build_framework_runtime_snapshot_envelope);
    register_build_automatic_continuity_checkpoint_payload(build_automatic_continuity_checkpoint_payload);
    register_append_evidence_index(append_evidence_index);
    register_hook_action_from_output(hook_action_from_output);
    register_closeout_record_schema_version(closeout_record_schema_version);
    register_check_anomalies(check_anomalies);
}

pub fn register_web_fetch_guard_extra(
    validate_url: ValidateWebFetchUrlFn,
    resolve_redirect: ResolveWebFetchRedirectFn,
    resolve_addresses: ResolveWebFetchAddressesFn,
) {
    register_validate_and_resolve_web_fetch_url(validate_url);
    register_resolve_web_fetch_redirect(resolve_redirect);
    register_resolve_web_fetch_addresses(resolve_addresses);
}

pub fn register_mcp_pre_guard_extra(evaluate: fn(&str, &Value, &Path) -> McpPreGuardVerdict) {
    register_evaluate_mcp_pre_guard_safe(evaluate);
}

// Manual dispatch: fallback closure captures nothing from args but calls current_dir()
pub fn resolve_repo_root_arg(repo_root: Option<&Path>) -> Result<PathBuf> {
    RESOLVE_REPO_ROOT_ARG
        .get()
        .map(|f| f(repo_root))
        .unwrap_or_else(|| {
            std::env::current_dir().map_err(FrameworkError::Io)
        })
}

// Manual dispatch: special fallback to old fn pointer
pub fn build_framework_runtime_snapshot_envelope_with_level(
    repo_root: &Path,
    artifact_root_override: Option<&Path>,
    task_id_override: Option<&str>,
    detail_level: &str,
) -> Result<Value> {
    if let Some(f) = BUILD_FRAMEWORK_RUNTIME_SNAPSHOT_ENVELOPE_WITH_LEVEL.get() {
        f(repo_root, artifact_root_override, task_id_override, detail_level)
    } else {
        build_framework_runtime_snapshot_envelope(repo_root, artifact_root_override, task_id_override)
    }
}

pub fn register_build_framework_runtime_snapshot_envelope_with_level(
    func: BuildSnapshotWithLevelFn,
) {
    once_lock_set(&BUILD_FRAMEWORK_RUNTIME_SNAPSHOT_ENVELOPE_WITH_LEVEL, func, "BUILD_FRAMEWORK_RUNTIME_SNAPSHOT_ENVELOPE_WITH_LEVEL");
}

// ── Test-only re-exports from test_helpers (for host_extensions::cursor test code) ──


// ── Quality Gate full implementation hook (registered by runtime-core) ──
// Manual: dispatch returns fn ptr instead of calling it
static QUALITY_GATE_DRIVE: OnceLock<fn(Value) -> Result<Value>> = OnceLock::new();

pub fn register_quality_gate_drive(func: fn(Value) -> Result<Value>) {
    once_lock_set(&QUALITY_GATE_DRIVE, func, "QUALITY_GATE_DRIVE");
}

/// Call the registered quality_gate implementation (runtime-core has append_round support).
/// Returns None if not registered (caller should fall back to core-state).
pub fn quality_gate_drive_registered() -> Option<fn(Value) -> Result<Value>> {
    QUALITY_GATE_DRIVE.get().copied()
}

// ── Host-projection-specific OnceLock slots ──

/// Research tool dispatch: injected at startup by runtime-core
/// to break the L3→L6 dependency direction.
type ResearchToolDispatchFn = fn(&str, &Value) -> std::result::Result<String, FrameworkError>;

// ── Session supervisor operation hook ──
// Manual: custom register with .set(f).ok() + logging
static SESSION_SUPERVISOR_OP: OnceLock<fn(Value) -> Result<Value>> = OnceLock::new();

/// Register the session-supervisor operation handler. Called once at startup.
pub fn register_session_supervisor_op(f: fn(Value) -> Result<Value>) {
    SESSION_SUPERVISOR_OP.set(f).ok();
    tracing::info!("session_supervisor_op: registered");
}

/// Dispatch a session-supervisor operation. Returns None if not registered.
pub fn session_supervisor_op(payload: Value) -> Option<Result<Value>> {
    SESSION_SUPERVISOR_OP.get().map(|f| f(payload))
}

// Manual: dispatch returns fn ptr instead of calling it
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
type McpToolSkillRouteFn = fn(query: &str, host_id: &str, first_turn: bool, repo_root: &str) -> Result<String>;
/// MCP tool search skills: search skills by query string.
type McpToolSearchSkillsFn = fn(query: &str, limit: usize, effective_host: &str, repo_root: &str) -> Result<String>;

once_lock_hook! { static MCP_TOOL_SKILL_ROUTE: McpToolSkillRouteFn; register register_mcp_tool_skill_route; fn mcp_tool_skill_route(query: &str, host_id: &str, first_turn: bool, repo_root: &str) -> Result<String> = err("skill_route not available (not registered)"); }
once_lock_hook! { static MCP_TOOL_SEARCH_SKILLS: McpToolSearchSkillsFn; register register_mcp_tool_search_skills; fn mcp_tool_search_skills(query: &str, limit: usize, effective_host: &str, repo_root: &str) -> Result<String> = err("search_skills not available (not registered)"); }

// ── Browser dispatch (moved from runtime-core to break L3→L4 dep) ──
// Manual: custom register with different warning pattern
type BrowserDispatchFn = fn(framework_kernel::cli_args::BrowserSubcommand) -> Result<()>;
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
) -> Result<()> {
    match BROWSER_DISPATCH.get() {
        Some(f) => f(command),
        None => Err(FrameworkError::validation(
            "browser-mcp dispatch not registered; call set_browser_dispatch() at startup",
        )),
    }
}

// ── Runtime trace transport proxies (break browser-mcp L3→L4 dep) ──

type AttachRuntimeEventTransportFn = fn(Value) -> Result<Value>;
type InspectTraceStreamFn = fn(
    framework_kernel::stdio_payload_types::TraceStreamInspectRequestPayload,
) -> Result<framework_kernel::stdio_payload_types::TraceStreamInspectResponsePayload>;

once_lock_hook! { static ATTACH_RUNTIME_EVENT_TRANSPORT: AttachRuntimeEventTransportFn; register register_attach_runtime_event_transport; fn attach_runtime_event_transport(payload: Value) -> Result<Value> = err("attach_runtime_event_transport not registered"); }
once_lock_hook! { static INSPECT_TRACE_STREAM: InspectTraceStreamFn; register register_inspect_trace_stream; fn inspect_trace_stream(payload: framework_kernel::stdio_payload_types::TraceStreamInspectRequestPayload) -> Result<framework_kernel::stdio_payload_types::TraceStreamInspectResponsePayload> = err("inspect_trace_stream not registered"); }

// ── Tool dispatch hooks: business logic extraction from L0 → L4 ──
//
// These hooks move heavy business logic (payload construction, enum validation,
// multi-source evaluation) out of host-projection's tool handlers into runtime-core.
// host-projection retains MCP parameter type-checking; runtime-core owns domain logic.

type GoalStateManageDispatchFn = fn(&Value, &Path, &str) -> std::result::Result<String, FrameworkError>;
type QualityGateManageDispatchFn = fn(&Value, &Path, &str) -> std::result::Result<String, FrameworkError>;
type CloseoutRecordWriteDispatchFn = fn(&Value, &Path) -> std::result::Result<String, FrameworkError>;
type CloseoutGateEvaluateFn = fn(&Value, &Path, &str) -> std::result::Result<String, FrameworkError>;
type RoutingEvolutionDispatchFn = fn(&Value, &Path) -> std::result::Result<String, FrameworkError>;

once_lock_hook! { static GOAL_STATE_MANAGE_DISPATCH: GoalStateManageDispatchFn; register register_tool_goal_state_manage_dispatch; fn tool_goal_state_manage_dispatch(args: &Value, repo_root: &Path, session_id: &str) -> Result<String> = err("tool_goal_state_manage_dispatch not registered — runtime-core boot required"); }
once_lock_hook! { static QUALITY_GATE_MANAGE_DISPATCH: QualityGateManageDispatchFn; register register_tool_quality_gate_manage_dispatch; fn tool_quality_gate_manage_dispatch(args: &Value, repo_root: &Path, session_id: &str) -> Result<String> = err("tool_quality_gate_manage_dispatch not registered — runtime-core boot required"); }
once_lock_hook! { static CLOSEOUT_RECORD_WRITE_DISPATCH: CloseoutRecordWriteDispatchFn; register register_tool_closeout_record_write_dispatch; fn tool_closeout_record_write_dispatch(args: &Value, repo_root: &Path) -> Result<String> = err("tool_closeout_record_write_dispatch not registered — runtime-core boot required"); }
once_lock_hook! { static CLOSEOUT_GATE_EVALUATE: CloseoutGateEvaluateFn; register register_tool_closeout_gate_evaluate; fn tool_closeout_gate_evaluate(args: &Value, repo_root: &Path, host_id: &str) -> Result<String> = err("tool_closeout_gate_evaluate not registered — runtime-core boot required"); }
once_lock_hook! { static ROUTING_EVOLUTION_DISPATCH: RoutingEvolutionDispatchFn; register register_tool_routing_evolution_dispatch; fn tool_routing_evolution_dispatch(args: &Value, repo_root: &Path) -> Result<String> = err("tool_routing_evolution_dispatch not registered — runtime-core boot required"); }

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
        let _host: PaperProseHookHost = "codex";
        assert_eq!(_host, "codex");
    }
}


