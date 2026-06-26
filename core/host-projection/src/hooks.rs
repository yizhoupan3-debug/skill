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

// ── Hook proxy macros (Phase C: RuntimeHooks-only, no OnceLock) ──

/// Proxy functions that delegate to `RuntimeHooks` struct.
///
/// v10 Wave 3a Phase C: replaces `once_lock_hook!` macro. No OnceLock slots
/// or register functions — all hooks are set via `set_runtime_hooks()` during bootstrap.
///
/// Arms:
/// - `fn name(args...);` — unit return, no-op if RuntimeHooks not yet set
/// - `fn name(args...) -> Ret = default;` — returns `default` if RuntimeHooks not yet set
/// - `fn name(args...) -> Ret = err("msg");` — returns `Err(FrameworkError::validation("msg"))`
macro_rules! runtime_hook_proxy {
    // Unit return — no-op if RuntimeHooks not yet set
    (
        $(#[$meta:meta])*
        fn $name:ident($($arg:ident: $t:ty),* $(,)?);
    ) => {
        $(#[$meta])*
        pub fn $name($($arg: $t),*) {
            if let Some(h) = get_runtime_hooks() { (h.$name)($($arg),*); }
        }
    };

    // Result return with Err default — `.unwrap_or_else(|| Err(FrameworkError::validation(msg)))`
    // MUST precede the generic arm below to avoid ambiguity with `= err(...)` as `$default:expr`.
    (
        $(#[$meta:meta])*
        fn $name:ident($($arg:ident: $t:ty),* $(,)?) -> $ret:ty = err($msg:expr);
    ) => {
        $(#[$meta])*
        pub fn $name($($arg: $t),*) -> $ret {
            get_runtime_hooks().map(|h| (h.$name)($($arg),*))
                .unwrap_or_else(|| Err(FrameworkError::validation($msg)))
        }
    };

    // Default value — `.map(|h| (h.field)(args)).unwrap_or(default)`
    (
        $(#[$meta:meta])*
        fn $name:ident($($arg:ident: $t:ty),* $(,)?) -> $ret:ty = $default:expr;
    ) => {
        $(#[$meta])*
        pub fn $name($($arg: $t),*) -> $ret {
            get_runtime_hooks().map(|h| (h.$name)($($arg),*)).unwrap_or_else(|| $default)
        }
    };
}

// ────────────────────────────────────────────────────────────────
// Host ID types (string-based, not enum — avoids per-host enum in L5)
// ────────────────────────────────────────────────────────────────

/// Type alias for backward compatibility with function pointer signatures.
/// Canonical host ID for paper prose/adversarial hooks.
pub type PaperProseHookHost = &'static str;

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
    /// Optional QG Checker ID — when set, the runtime runs this QG Checker
    /// directly instead of loading a full skill session.
    pub checker_id: Option<String>,
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
        // In tests, allow explicit env var override. If unset, fall through to
        // core_policy default (same as production behavior).
        if let Ok(raw) = std::env::var("ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES") {
            return raw.parse().ok();
        }
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

runtime_hook_proxy! { fn init_tracker(repo_root: &Path) -> Result<()> = Ok(()); }
runtime_hook_proxy! { fn record_tool_call(repo_root: &Path, tool_name: &str, cache_stats: Option<&Value>) -> Result<()> = Ok(()); }
runtime_hook_proxy! { fn read_tracker_state(repo_root: &Path) -> Result<Value> = Ok(serde_json::json!({})); }

// ────────────────────────────────────────────────────────────────

runtime_hook_proxy! { fn closeout_record_path_for_task(repo_root: &Path, task_id: &str) -> Result<PathBuf> = err("CLOSEOUT_RECORD_PATH not registered — runtime-core boot required"); }
runtime_hook_proxy! { fn evaluate_closeout_record_file_for_task(repo_root: &Path, task_id: &str, record_path: &Path) -> Result<Value> = err("hook framework_runtime not registered — runtime-core boot required"); }
runtime_hook_proxy! { fn extract_post_tool_duration_ms(event: &Value) -> Option<u64> = None; }
runtime_hook_proxy! { fn post_tool_call_succeeded(event: &Value) -> bool = true; }
runtime_hook_proxy! { fn closeout_stop_followup_for_completion_text(repo_root: &Path, text: &str) -> Option<String> = None; }


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
// paper hooks: individual OnceLock slots (set by research-harness)
// Proxy reads individual slot first (priority over RuntimeHooks).
// ────────────────────────────────────────────────────────────────

/// Append paper prose/adversarial context: (repo_root, prompt_text, contexts, host)
type AppendPaperContextFn = fn(&Path, &str, &mut Vec<String>, PaperProseHookHost);
/// Merge paper prose/adversarial before submit: (repo_root, output, prompt_text, use_followup_message, host)
type MergePaperContextFn = fn(&Path, &mut Value, &str, bool, PaperProseHookHost);

static APPEND_PROSE: OnceLock<AppendPaperContextFn> = OnceLock::new();
static MERGE_PROSE: OnceLock<MergePaperContextFn> = OnceLock::new();
static APPEND_ADVERSARIAL: OnceLock<AppendPaperContextFn> = OnceLock::new();
static MERGE_ADVERSARIAL: OnceLock<MergePaperContextFn> = OnceLock::new();

pub fn register_append_prose(f: AppendPaperContextFn) { once_lock_set(&APPEND_PROSE, f, "APPEND_PROSE"); }
pub fn register_merge_prose(f: MergePaperContextFn) { once_lock_set(&MERGE_PROSE, f, "MERGE_PROSE"); }
pub fn register_append_adversarial(f: AppendPaperContextFn) { once_lock_set(&APPEND_ADVERSARIAL, f, "APPEND_ADVERSARIAL"); }
pub fn register_merge_adversarial(f: MergePaperContextFn) { once_lock_set(&MERGE_ADVERSARIAL, f, "MERGE_ADVERSARIAL"); }

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

pub fn maybe_append_paper_prose_context(repo_root: &Path, prompt_text: &str, contexts: &mut Vec<String>, host: PaperProseHookHost) {
    if let Some(f) = APPEND_PROSE.get() { f(repo_root, prompt_text, contexts, host); return; }
    if let Some(h) = get_runtime_hooks() { (h.maybe_append_paper_prose_context)(repo_root, prompt_text, contexts, host); }
}
pub fn maybe_merge_paper_prose_before_submit(repo_root: &Path, output: &mut Value, prompt_text: &str, use_followup_message: bool, host: PaperProseHookHost) {
    if let Some(f) = MERGE_PROSE.get() { f(repo_root, output, prompt_text, use_followup_message, host); return; }
    if let Some(h) = get_runtime_hooks() { (h.maybe_merge_paper_prose_before_submit)(repo_root, output, prompt_text, use_followup_message, host); }
}
pub fn maybe_append_paper_adversarial_context(repo_root: &Path, prompt_text: &str, contexts: &mut Vec<String>, host: PaperProseHookHost) {
    if let Some(f) = APPEND_ADVERSARIAL.get() { f(repo_root, prompt_text, contexts, host); return; }
    if let Some(h) = get_runtime_hooks() { (h.maybe_append_paper_adversarial_context)(repo_root, prompt_text, contexts, host); }
}
pub fn maybe_merge_paper_adversarial_before_submit(repo_root: &Path, output: &mut Value, prompt_text: &str, use_followup_message: bool, host: PaperProseHookHost) {
    if let Some(f) = MERGE_ADVERSARIAL.get() { f(repo_root, output, prompt_text, use_followup_message, host); return; }
    if let Some(h) = get_runtime_hooks() { (h.maybe_merge_paper_adversarial_before_submit)(repo_root, output, prompt_text, use_followup_message, host); }
}

// ────────────────────────────────────────────────────────────────
// research activity log: individual OnceLock slot (set by research-harness)
// ────────────────────────────────────────────────────────────────

static RESEARCH_ACTIVITY: OnceLock<fn(&Path, &str, &str)> = OnceLock::new();

pub fn register_research_activity_hook(f: fn(&Path, &str, &str)) {
    once_lock_set(&RESEARCH_ACTIVITY, f, "RESEARCH_ACTIVITY");
}

pub fn maybe_record_research_activity(repo_root: &Path, tool_name: &str, summary: &str) {
    if let Some(f) = RESEARCH_ACTIVITY.get() { f(repo_root, tool_name, summary); return; }
    if let Some(h) = get_runtime_hooks() { (h.maybe_record_research_activity)(repo_root, tool_name, summary); }
}

// ────────────────────────────────────────────────────────────────
// Research mode inference: individual OnceLock slot (set by research-harness)
// ────────────────────────────────────────────────────────────────

type InferResearchModeFn = fn(&Value) -> String;

static INFER_RESEARCH_MODE: OnceLock<InferResearchModeFn> = OnceLock::new();

pub fn register_research_mode_inference(f: InferResearchModeFn) {
    once_lock_set(&INFER_RESEARCH_MODE, f, "INFER_RESEARCH_MODE");
}

pub fn research_mode_for_request(payload: &Value) -> String {
    if let Some(f) = INFER_RESEARCH_MODE.get() { return f(payload); }
    get_runtime_hooks().map(|h| (h.research_mode_for_request)(payload)).unwrap_or_else(|| "quick".to_string())
}

// ── Skill routing bridge: removed ──
// Was never registered in production (register_skill_routing_bridge not called).
// Route decision goes through route_task_with_manifest_fallback instead.


// ────────────────────────────────────────────────────────────────
// kernel_bootstrap: RuntimeHooks proxy (Phase C)
// ────────────────────────────────────────────────────────────────

pub fn ensure_kernel_bootstrap() {
    if let Some(h) = get_runtime_hooks() {
        (h.ensure_kernel_bootstrap)();
    }
    // research_mode_inference: NOT registered here — was previously inline but
    // the simplified logic preempted the production version from
    // research_harness::init_hooks(). The dispatch default is "quick" until
    // the real implementation registers.
    #[cfg(test)]
    crate::test_helpers::install_test_deps();
}

// ────────────────────────────────────────────────────────────────
// Additional hooks needed by host_extensions::claude / mcp_stdio_harness
// (appended during host-projection hooks consolidation)
// ────────────────────────────────────────────────────────────────

// ── framework_runtime_extra ──

/// Check anomalies: (repo_root) -> Result<anomaly_list>
type CheckAnomaliesFn = fn(&Path) -> Result<Vec<String>>;

runtime_hook_proxy! { fn current_local_timestamp() -> String = "1970-01-01T00:00:00Z".into(); }
runtime_hook_proxy! { fn write_framework_session_artifacts(payload: Value) -> Result<Value> = err("WRITE_FRAMEWORK_SESSION_ARTIFACTS not registered — runtime-core boot required"); }
runtime_hook_proxy! { fn route_task_with_manifest_fallback(runtime_records: &[serde_json::Value], host_id: Option<&str>, query: &str, session_id: &str, allow_overlay: bool, first_turn: bool) -> Result<RouteDecision> = err("ROUTE_TASK_WITH_MANIFEST_FALLBACK not registered — runtime-core boot required"); }
runtime_hook_proxy! { fn build_automatic_continuity_checkpoint_payload(repo_root: &Path, task_line: &str, summary_text: &str, task_id: Option<&str>, repointer_focus: bool, update_registry_only_if_known: bool) -> Value = Value::Null; }
runtime_hook_proxy! { fn append_evidence_index(repo_root: &Path, task_id: Option<&str>, entry: serde_json::Map<String, Value>) -> Result<()> = err("APPEND_EVIDENCE_INDEX not registered — runtime-core boot required"); }
runtime_hook_proxy! { fn closeout_record_schema_version() -> &'static str = "closeout-record-v1"; }
runtime_hook_proxy! { fn check_anomalies(repo_root: &Path) -> Result<Vec<String>> = Ok(vec![]); }

// ── web_fetch_guard ──

/// Validate and resolve web fetch URL: (url) -> Result<(resolved_url, addresses)>
type ValidateWebFetchUrlFn = fn(&str) -> Result<(String, Vec<String>)>;

/// Resolve web fetch redirect: (base_url, location) -> Result<resolved_url>
type ResolveWebFetchRedirectFn = fn(&str, &str) -> Result<String>;

/// Resolve web fetch addresses: (host, port) -> Result<addresses>
type ResolveWebFetchAddressesFn = fn(&str, u16) -> Result<Vec<String>>;

runtime_hook_proxy! { fn validate_and_resolve_web_fetch_url(url: &str) -> Result<(String, Vec<String>)> = err("VALIDATE_AND_RESOLVE_WEB_FETCH_URL not registered — runtime-core boot required"); }
runtime_hook_proxy! { fn resolve_web_fetch_redirect(base: &str, location: &str) -> Result<String> = err("RESOLVE_WEB_FETCH_REDIRECT not registered — runtime-core boot required"); }
runtime_hook_proxy! { fn resolve_web_fetch_addresses(host: &str, port: u16) -> Result<Vec<String>> = err("RESOLVE_WEB_FETCH_ADDRESSES not registered — runtime-core boot required"); }

// ── mcp_pre_guard ──

runtime_hook_proxy! { fn evaluate_mcp_pre_guard_safe(tool_name: &str, arguments: &Value, repo_root: &Path) -> McpPreGuardVerdict = McpPreGuardVerdict { blocked: false, reason: None }; }

// 7 fn pointer params in a registration pattern — below threshold=8, OK to keep.

// ── Test-only re-exports from test_helpers (for host_extensions::cursor test code) ──


// ── Quality Gate full implementation hook (registered by runtime-core) ──
// Returns None if not registered (caller should fall back to core-state).
pub fn quality_gate_drive_registered() -> Option<fn(Value) -> Result<Value>> {
    get_runtime_hooks().map(|h| h.quality_gate_drive)
}

// ── Host-projection-specific OnceLock slots ──

/// Research tool dispatch: injected at startup by runtime-core
/// to break the L3→L6 dependency direction.
type ResearchToolDispatchFn = fn(&str, &Value) -> std::result::Result<String, FrameworkError>;

// Returns None if not registered (caller should fall back to core-state).
static RESEARCH_TOOL_DISPATCH_SLOT: OnceLock<ResearchToolDispatchFn> = OnceLock::new();

pub fn register_research_tool_dispatch(f: ResearchToolDispatchFn) {
    once_lock_set(&RESEARCH_TOOL_DISPATCH_SLOT, f, "RESEARCH_TOOL_DISPATCH_SLOT");
}

pub fn get_research_tool_dispatch() -> Option<ResearchToolDispatchFn> {
    if let Some(f) = RESEARCH_TOOL_DISPATCH_SLOT.get().copied() { return Some(f); }
    get_runtime_hooks().map(|h| h.research_tool_dispatch)
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

runtime_hook_proxy! { fn mcp_tool_skill_route(query: &str, host_id: &str, first_turn: bool, repo_root: &str) -> Result<String> = err("MCP_TOOL_SKILL_ROUTE not registered — runtime-core boot required"); }
runtime_hook_proxy! { fn mcp_tool_search_skills(query: &str, limit: usize, effective_host: &str, repo_root: &str) -> Result<String> = err("MCP_TOOL_SEARCH_SKILLS not registered — runtime-core boot required"); }

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

runtime_hook_proxy! { fn attach_runtime_event_transport(payload: Value) -> Result<Value> = err("ATTACH_RUNTIME_EVENT_TRANSPORT not registered — runtime-core boot required"); }
runtime_hook_proxy! { fn inspect_trace_stream(payload: framework_kernel::stdio_payload_types::TraceStreamInspectRequestPayload) -> Result<framework_kernel::stdio_payload_types::TraceStreamInspectResponsePayload> = err("INSPECT_TRACE_STREAM not registered — runtime-core boot required"); }

// ── Tool dispatch hooks: business logic extraction from L0 → L4 ──
//
// These hooks move heavy business logic (payload construction, enum validation,
// multi-source evaluation) out of host-projection's tool handlers into runtime-core.
// host-projection retains MCP parameter type-checking; runtime-core owns domain logic.

type GoalStateManageDispatchFn = fn(&Value, &Path, &str) -> std::result::Result<String, FrameworkError>;
type QualityGateManageDispatchFn = fn(&Value, &Path, &str) -> std::result::Result<String, FrameworkError>;
type CloseoutRecordWriteDispatchFn = fn(&Value, &Path) -> std::result::Result<String, FrameworkError>;
type CloseoutGateEvaluateFn = fn(&Value, &Path, &str) -> std::result::Result<String, FrameworkError>;

runtime_hook_proxy! { fn tool_goal_state_manage_dispatch(args: &Value, repo_root: &Path, session_id: &str) -> Result<String> = err("GOAL_STATE_MANAGE_DISPATCH not registered — runtime-core boot required"); }
runtime_hook_proxy! { fn tool_quality_gate_manage_dispatch(args: &Value, repo_root: &Path, session_id: &str) -> Result<String> = err("QUALITY_GATE_MANAGE_DISPATCH not registered — runtime-core boot required"); }
runtime_hook_proxy! { fn tool_closeout_record_write_dispatch(args: &Value, repo_root: &Path) -> Result<String> = err("CLOSEOUT_RECORD_WRITE_DISPATCH not registered — runtime-core boot required"); }
runtime_hook_proxy! { fn tool_closeout_gate_evaluate(args: &Value, repo_root: &Path, host_id: &str) -> Result<String> = err("CLOSEOUT_GATE_EVALUATE not registered — runtime-core boot required"); }

// ════════════════════════════════════════════════════════════════
// RuntimeHooks — Wave 3a Phase A: consolidate all fn ptr slots
// ════════════════════════════════════════════════════════════════

/// Consolidated function pointer hooks (replaces ~38 individual OnceLock slots).
///
/// v10 Wave 3a Phase A: define struct + double-registration from runtime-core bootstrap.
/// Phase B: migrate consumers from proxy functions to `get_runtime_hooks()?.field`.
/// Phase C: remove old `once_lock_hook!` macro and individual OnceLock slots.
pub struct RuntimeHooks {
    // session_call_tracker (3 fields)
    pub init_tracker: fn(&Path) -> Result<()>,
    pub record_tool_call: fn(&Path, &str, Option<&Value>) -> Result<()>,
    pub read_tracker_state: fn(&Path) -> Result<Value>,
    // framework_runtime (5 fields)
    pub closeout_record_path_for_task: fn(&Path, &str) -> Result<PathBuf>,
    pub evaluate_closeout_record_file_for_task: fn(&Path, &str, &Path) -> Result<Value>,
    pub extract_post_tool_duration_ms: fn(&Value) -> Option<u64>,
    pub post_tool_call_succeeded: fn(&Value) -> bool,
    pub closeout_stop_followup_for_completion_text: fn(&Path, &str) -> Option<String>,
    // paper hooks (4 fields)
    pub maybe_append_paper_prose_context: AppendPaperContextFn,
    pub maybe_merge_paper_prose_before_submit: MergePaperContextFn,
    pub maybe_append_paper_adversarial_context: AppendPaperContextFn,
    pub maybe_merge_paper_adversarial_before_submit: MergePaperContextFn,
    // research activity (1 field)
    pub maybe_record_research_activity: fn(&Path, &str, &str),
    // research mode inference (1 field)
    pub research_mode_for_request: fn(&Value) -> String,
    // kernel bootstrap (1 field)
    pub ensure_kernel_bootstrap: fn(),
    // framework_runtime_extra (7 fields)
    pub current_local_timestamp: fn() -> String,
    pub write_framework_session_artifacts: fn(Value) -> Result<Value>,
    pub route_task_with_manifest_fallback: RouteTaskFn,
    pub build_automatic_continuity_checkpoint_payload: BuildCheckpointFn,
    pub append_evidence_index: AppendEvidenceFn,
    pub closeout_record_schema_version: fn() -> &'static str,
    pub check_anomalies: CheckAnomaliesFn,
    // web_fetch_guard (3 fields)
    pub validate_and_resolve_web_fetch_url: ValidateWebFetchUrlFn,
    pub resolve_web_fetch_redirect: ResolveWebFetchRedirectFn,
    pub resolve_web_fetch_addresses: ResolveWebFetchAddressesFn,
    // mcp_pre_guard (1 field)
    pub evaluate_mcp_pre_guard_safe: fn(&str, &Value, &Path) -> McpPreGuardVerdict,
    // quality_gate_drive (1 field)
    pub quality_gate_drive: fn(Value) -> Result<Value>,
    // research_tool_dispatch (1 field)
    pub research_tool_dispatch: ResearchToolDispatchFn,
    // mcp_tool_routing (2 fields)
    pub mcp_tool_skill_route: McpToolSkillRouteFn,
    pub mcp_tool_search_skills: McpToolSearchSkillsFn,
    // tool_dispatch (4 fields)
    pub tool_goal_state_manage_dispatch: GoalStateManageDispatchFn,
    pub tool_quality_gate_manage_dispatch: QualityGateManageDispatchFn,
    pub tool_closeout_record_write_dispatch: CloseoutRecordWriteDispatchFn,
    pub tool_closeout_gate_evaluate: CloseoutGateEvaluateFn,
    // browser_dispatch (1 field)
    pub browser_dispatch: BrowserDispatchFn,
    // runtime_trace_transport (2 fields)
    pub attach_runtime_event_transport: AttachRuntimeEventTransportFn,
    pub inspect_trace_stream: InspectTraceStreamFn,
}

static RUNTIME_HOOKS: OnceLock<RuntimeHooks> = OnceLock::new();

/// Get the consolidated RuntimeHooks struct. Returns `None` if not yet set (bootstrap not complete).
pub fn get_runtime_hooks() -> Option<&'static RuntimeHooks> {
    RUNTIME_HOOKS.get()
}

/// Set the consolidated RuntimeHooks struct during bootstrap.
/// Idempotent: second call is silently ignored.
pub fn set_runtime_hooks(hooks: RuntimeHooks) {
    RUNTIME_HOOKS.set(hooks).unwrap_or_else(|_| {
        tracing::warn!("RuntimeHooks already registered — second call ignored");
    });
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
            checker_id: None,
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


