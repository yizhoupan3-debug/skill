//! host-projection hooks proxy layer.
//!
//! Owns the shared `RuntimeHooks` struct (replaces ~38 individual OnceLock slots).
//! L4 crates (runtime-core, framework-runtime, goal-engine) register
//! their callbacks via `set_runtime_hooks()` during bootstrap.
//! L5 crates (research-harness) extend specific fields via `modify_runtime_hooks()`.
//!
//! Proxy functions are re-exported for consumers that need them.

use core_errors::FrameworkError;
type Result<T> = std::result::Result<T, FrameworkError>;

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

// ── Function pointer type aliases (reduce type_complexity warnings) ──

/// Build automatic continuity checkpoint payload: (repo_root, task_id, session_id, current_query, allow_overlay, first_turn) -> Value
type BuildCheckpointFn = fn(&Path, &str, &str, Option<&str>, bool, bool) -> Value;

/// Append evidence index row: (repo_root, task_id, metadata) -> Result<()>
type AppendEvidenceFn = fn(&Path, Option<&str>, serde_json::Map<String, Value>) -> Result<()>;

// ── Hook proxy macros (RuntimeHooks-only) ──

/// Proxy functions that delegate to `RuntimeHooks` struct.
///
/// v10 Wave 3a Phase C ✅: replaces `once_lock_hook!` macro + all individual
/// OnceLock slots. All hooks are set via `set_runtime_hooks()` during bootstrap;
/// L5 crates extend specific fields via `modify_runtime_hooks()`.
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
    framework_core::runtime_registry::paper_prose_env(host)
}

/// Per-host env var controlling adversarial review hook injection.
/// Generated from RUNTIME_REGISTRY.json host_targets.metadata.*.paper_adversarial_env.
pub fn paper_adversarial_env_var(host: &str) -> &'static str {
    framework_core::runtime_registry::paper_adversarial_env(host)
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
    framework_core::env_flags::router_rs_review_gate_stop_max_nudges_cap()
}

// ────────────────────────────────────────────────────────────────
// Hook-state file sweep utilities
// ────────────────────────────────────────────────────────────────

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
            return Err(FrameworkError::validation(
                "stdin payload exceeds 4 MiB limit",
            ));
        }
    }
    Ok(input)
}

// ────────────────────────────────────────────────────────────────
// hook_timing: function-pointer proxies (OnceLock)
// ────────────────────────────────────────────────────────────────

// All four hook_timing proxy dispatch functions were never used by any caller
// (callers go through runtime_core::hook_timing::* directly).
// The bundle register register_hook_timing has been removed accordingly.
// The native functions are still defined in runtime-core/src/hook_timing.rs.

// ────────────────────────────────────────────────────────────────

runtime_hook_proxy! { fn closeout_record_path_for_task(repo_root: &Path, task_id: &str) -> Result<PathBuf> = err("CLOSEOUT_RECORD_PATH not registered — runtime-core boot required"); }
runtime_hook_proxy! { fn evaluate_closeout_record_file_for_task(repo_root: &Path, task_id: &str, record_path: &Path) -> Result<Value> = err("hook framework_runtime not registered — runtime-core boot required"); }
runtime_hook_proxy! { fn extract_post_tool_duration_ms(event: &Value) -> Option<u64> = None; }
runtime_hook_proxy! { fn post_tool_call_succeeded(event: &Value) -> bool = true; }
runtime_hook_proxy! { fn closeout_stop_followup_for_completion_text(repo_root: &Path, text: &str) -> Option<String> = None; }

// ── hook_outbound_protect: removed from hooks proxy layer ──
//
// The authoritative implementation lived in the now-deleted runtime-core-contracts
// crate (hook_outbound_protect.rs). Functions are called directly by consumers.
// The OnceLock proxy was never registered in production.

// hook_posttool_normalize: default policy (register removed — was never called in production)
// ────────────────────────────────────────────────────────────────

#[cfg(not(test))]
pub fn synthetic_post_tool_evidence_shape(_event: &Value) -> Value {
    serde_json::json!({})
}

#[cfg(test)]
pub use crate::test_helpers::{
    register_hook_posttool_normalize, synthetic_post_tool_evidence_shape,
};

// ── ship_readiness: removed from hooks proxy layer ──
//
// register_ship_readiness was never called in production;
// the OnceLock slots were always empty. Gate eval handles
// goal readiness without the hooks proxy.

// ────────────────────────────────────────────────────────────────
// paper hooks + research activity (RuntimeHooks-only, Phase C ✅)
// ────────────────────────────────────────────────────────────────

/// Append paper prose/adversarial context: (repo_root, prompt_text, contexts, host)
type AppendPaperContextFn = fn(&Path, &str, &mut Vec<String>, PaperProseHookHost);
/// Merge paper prose/adversarial before submit: (repo_root, output, prompt_text, use_followup_message, host)
type MergePaperContextFn = fn(&Path, &mut Value, &str, bool, PaperProseHookHost);

runtime_hook_proxy! { fn maybe_append_paper_prose_context(repo_root: &Path, prompt_text: &str, contexts: &mut Vec<String>, host: PaperProseHookHost); }
runtime_hook_proxy! { fn maybe_merge_paper_prose_before_submit(repo_root: &Path, output: &mut Value, prompt_text: &str, use_followup_message: bool, host: PaperProseHookHost); }
runtime_hook_proxy! { fn maybe_append_paper_adversarial_context(repo_root: &Path, prompt_text: &str, contexts: &mut Vec<String>, host: PaperProseHookHost); }
runtime_hook_proxy! { fn maybe_merge_paper_adversarial_before_submit(repo_root: &Path, output: &mut Value, prompt_text: &str, use_followup_message: bool, host: PaperProseHookHost); }

runtime_hook_proxy! { fn maybe_record_research_activity(repo_root: &Path, tool_name: &str, summary: &str); }

// ── Skill routing bridge: removed ──
// Was never registered in production (register_skill_routing_bridge not called).
// ────────────────────────────────────────────────────────────────
// kernel_bootstrap: RuntimeHooks proxy (Phase C)
// ────────────────────────────────────────────────────────────────

pub fn ensure_kernel_bootstrap() {
    if let Some(h) = get_runtime_hooks() {
        (h.ensure_kernel_bootstrap)();
    }
    #[cfg(test)]
    crate::test_helpers::install_test_deps();
}

// ────────────────────────────────────────────────────────────────
// Additional hooks needed by host_extensions::claude / mcp_stdio_harness
// (appended during host-projection hooks consolidation)
// ────────────────────────────────────────────────────────────────

// ── framework_runtime_extra ──

runtime_hook_proxy! { fn current_local_timestamp() -> String = "1970-01-01T00:00:00Z".into(); }
runtime_hook_proxy! { fn write_framework_session_artifacts(payload: Value) -> Result<Value> = err("WRITE_FRAMEWORK_SESSION_ARTIFACTS not registered — runtime-core boot required"); }
runtime_hook_proxy! { fn append_evidence_index(repo_root: &Path, task_id: Option<&str>, entry: serde_json::Map<String, Value>) -> Result<()> = err("APPEND_EVIDENCE_INDEX not registered — runtime-core boot required"); }
runtime_hook_proxy! { fn closeout_record_schema_version() -> &'static str = "closeout-record-v1"; }

// ── web_fetch_guard ──

/// Validate and resolve web fetch URL: (url) -> Result<(resolved_url, addresses)>
type ValidateWebFetchUrlFn = fn(&str) -> Result<(String, Vec<String>)>;

/// Resolve web fetch redirect: (base_url, location) -> Result<(resolved_url, addresses)>
type ResolveWebFetchRedirectFn = fn(&str, &str) -> Result<(String, Vec<String>)>;

/// Resolve web fetch addresses: (host, port) -> Result<addresses>
type ResolveWebFetchAddressesFn = fn(&str, u16) -> Result<Vec<String>>;

runtime_hook_proxy! { fn validate_and_resolve_web_fetch_url(url: &str) -> Result<(String, Vec<String>)> = err("VALIDATE_AND_RESOLVE_WEB_FETCH_URL not registered — runtime-core boot required"); }
runtime_hook_proxy! { fn resolve_web_fetch_redirect(base: &str, location: &str) -> Result<(String, Vec<String>)> = err("RESOLVE_WEB_FETCH_REDIRECT not registered — runtime-core boot required"); }
runtime_hook_proxy! { fn resolve_web_fetch_addresses(host: &str, port: u16) -> Result<Vec<String>> = err("RESOLVE_WEB_FETCH_ADDRESSES not registered — runtime-core boot required"); }

// ── mcp_pre_guard ──

runtime_hook_proxy! { fn evaluate_mcp_pre_guard_safe(tool_name: &str, arguments: &Value, repo_root: &Path) -> McpPreGuardVerdict = McpPreGuardVerdict { blocked: true, reason: Some("MCP pre-guard not initialized — rejected by default".to_string()) }; }

// 7 fn pointer params in a registration pattern — below threshold=8, OK to keep.

// ── Test-only re-exports from test_helpers (for host_extensions::cursor test code) ──

/// Research tool dispatch: injected at startup by runtime-core
/// to break the L3→L6 dependency direction.
type ResearchToolDispatchFn = fn(&str, &Value) -> std::result::Result<String, FrameworkError>;

/// Get the registered research tool dispatch function from RuntimeHooks.
pub(crate) fn get_research_tool_dispatch() -> Option<ResearchToolDispatchFn> {
    get_runtime_hooks().map(|h| h.research_tool_dispatch)
}

// ── MCP routing: decouple L0→L1 DAG violation (ADR-010 §11.2) ──
//
// These fn ptrs break the compile-time dependency from host-projection (L0)
// to routing-engine (L1). L4 (runtime-core) registers the routing-engine
// implementations during bootstrap. L0 calls through the fn ptr and receives
// JSON — no routing-engine types cross the boundary.

/// MCP tool skill route: route a query to the best matching skill.
type McpToolSkillRouteFn =
    fn(query: &str, host_id: &str, first_turn: bool, repo_root: &str) -> Result<String>;

runtime_hook_proxy! { fn mcp_tool_skill_route(query: &str, host_id: &str, first_turn: bool, repo_root: &str) -> Result<String> = err("MCP_TOOL_SKILL_ROUTE not registered — runtime-core boot required"); }
// ── Browser dispatch (via RuntimeHooks, set via modify_runtime_hooks) ──
type BrowserDispatchFn = fn(framework_core::cli_args::BrowserSubcommand) -> Result<()>;

/// Dispatch a browser subcommand. Returns `Err` if no dispatch function was registered.
pub fn dispatch_browser_command(
    command: framework_core::cli_args::BrowserSubcommand,
) -> Result<()> {
    get_runtime_hooks()
        .map(|h| (h.browser_dispatch)(command))
        .unwrap_or_else(|| {
            Err(FrameworkError::validation(
                "browser-mcp dispatch not registered; set via modify_runtime_hooks() at startup",
            ))
        })
}

// ── Runtime trace transport proxies (break browser-mcp L3→L4 dep) ──

type AttachRuntimeEventTransportFn = fn(Value) -> Result<Value>;
type InspectTraceStreamFn =
    fn(
        framework_core::stdio_payload_types::TraceStreamInspectRequestPayload,
    ) -> Result<framework_core::stdio_payload_types::TraceStreamInspectResponsePayload>;

runtime_hook_proxy! { fn attach_runtime_event_transport(payload: Value) -> Result<Value> = err("ATTACH_RUNTIME_EVENT_TRANSPORT not registered — runtime-core boot required"); }
runtime_hook_proxy! { fn inspect_trace_stream(payload: framework_core::stdio_payload_types::TraceStreamInspectRequestPayload) -> Result<framework_core::stdio_payload_types::TraceStreamInspectResponsePayload> = err("INSPECT_TRACE_STREAM not registered — runtime-core boot required"); }

// ── Tool dispatch hooks: business logic extraction from L0 → L4 ──
//
// These hooks move heavy business logic (payload construction, enum validation,
// multi-source evaluation) out of host-projection's tool handlers into runtime-core.
// host-projection retains MCP parameter type-checking; runtime-core owns domain logic.

type GoalStateManageDispatchFn =
    fn(&Value, &Path, &str) -> std::result::Result<String, FrameworkError>;
type CloseoutRecordWriteDispatchFn =
    fn(&Value, &Path) -> std::result::Result<String, FrameworkError>;
type CloseoutGateEvaluateFn =
    fn(&Value, &Path, &str) -> std::result::Result<String, FrameworkError>;

runtime_hook_proxy! { fn tool_goal_state_manage_dispatch(args: &Value, repo_root: &Path, session_id: &str) -> Result<String> = err("GOAL_STATE_MANAGE_DISPATCH not registered — runtime-core boot required"); }
runtime_hook_proxy! { fn tool_closeout_record_write_dispatch(args: &Value, repo_root: &Path) -> Result<String> = err("CLOSEOUT_RECORD_WRITE_DISPATCH not registered — runtime-core boot required"); }
runtime_hook_proxy! { fn tool_closeout_gate_evaluate(args: &Value, repo_root: &Path, host_id: &str) -> Result<String> = err("CLOSEOUT_GATE_EVALUATE not registered — runtime-core boot required"); }

// ════════════════════════════════════════════════════════════════
// RuntimeHooks — Wave 3a ✅ all phases complete
// ════════════════════════════════════════════════════════════════

/// Consolidated function pointer hooks (replaces ~38 individual OnceLock slots).
///
/// v10 Wave 3a Phase A: define struct + double-registration from runtime-core bootstrap.
/// Phase B: migrate consumers from proxy functions to `get_runtime_hooks()?.field`.
/// Phase C ✅: all individual OnceLock slots removed; L5 crates use
/// `modify_runtime_hooks()` to extend research-specific fields after bootstrap.
#[derive(Clone, Copy)]
pub struct RuntimeHooks {
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
    // kernel bootstrap (1 field)
    pub ensure_kernel_bootstrap: fn(),
    // framework_runtime_extra (5 fields)
    pub current_local_timestamp: fn() -> String,
    pub write_framework_session_artifacts: fn(Value) -> Result<Value>,
    pub build_automatic_continuity_checkpoint_payload: BuildCheckpointFn,
    pub append_evidence_index: AppendEvidenceFn,
    pub closeout_record_schema_version: fn() -> &'static str,
    // web_fetch_guard (3 fields)
    pub validate_and_resolve_web_fetch_url: ValidateWebFetchUrlFn,
    pub resolve_web_fetch_redirect: ResolveWebFetchRedirectFn,
    pub resolve_web_fetch_addresses: ResolveWebFetchAddressesFn,
    // mcp_pre_guard (1 field)
    pub evaluate_mcp_pre_guard_safe: fn(&str, &Value, &Path) -> McpPreGuardVerdict,
    // research_tool_dispatch (1 field)
    pub research_tool_dispatch: ResearchToolDispatchFn,
    // mcp_tool_routing (1 field)
    pub mcp_tool_skill_route: McpToolSkillRouteFn,
    pub tool_goal_state_manage_dispatch: GoalStateManageDispatchFn,
    pub tool_closeout_record_write_dispatch: CloseoutRecordWriteDispatchFn,
    pub tool_closeout_gate_evaluate: CloseoutGateEvaluateFn,
    // browser_dispatch (1 field)
    pub browser_dispatch: BrowserDispatchFn,
    // runtime_trace_transport (2 fields)
    pub attach_runtime_event_transport: AttachRuntimeEventTransportFn,
    pub inspect_trace_stream: InspectTraceStreamFn,
}

/// Safe defaults for all RuntimeHooks fields.
///
/// Provides no-op or error-returning defaults that mirror the proxy fallback
/// behavior. Useful for tests and for constructing a partial RuntimeHooks
/// that gets overridden via `modify_runtime_hooks()` after bootstrap.
impl Default for RuntimeHooks {
    fn default() -> Self {
        Self {
            // framework_runtime (5 fields)
            closeout_record_path_for_task: |_, _| {
                Err(FrameworkError::validation(
                    "CLOSEOUT_RECORD_PATH not registered",
                ))
            },
            evaluate_closeout_record_file_for_task: |_, _, _| {
                Err(FrameworkError::validation(
                    "evaluate_closeout_record_file_for_task not registered",
                ))
            },
            extract_post_tool_duration_ms: |_| None,
            post_tool_call_succeeded: |_| true,
            closeout_stop_followup_for_completion_text: |_, _| None,
            // paper hooks (4 fields) — no-op defaults
            maybe_append_paper_prose_context: |_, _, _, _| {},
            maybe_merge_paper_prose_before_submit: |_, _, _, _, _| {},
            maybe_append_paper_adversarial_context: |_, _, _, _| {},
            maybe_merge_paper_adversarial_before_submit: |_, _, _, _, _| {},
            // research activity (1 field) — no-op default
            maybe_record_research_activity: |_, _, _| {},
            // kernel bootstrap (1 field)
            ensure_kernel_bootstrap: || {},
            // framework_runtime_extra (5 fields)
            current_local_timestamp: || "1970-01-01T00:00:00Z".to_string(),
            write_framework_session_artifacts: |_| {
                Err(FrameworkError::validation(
                    "WRITE_FRAMEWORK_SESSION_ARTIFACTS not registered",
                ))
            },
            build_automatic_continuity_checkpoint_payload: |_, _, _, _, _, _| Value::Null,
            append_evidence_index: |_, _, _| {
                Err(FrameworkError::validation(
                    "APPEND_EVIDENCE_INDEX not registered",
                ))
            },
            closeout_record_schema_version: || "closeout-record-v1",
            // web_fetch_guard (3 fields)
            validate_and_resolve_web_fetch_url: |_| {
                Err(FrameworkError::validation(
                    "VALIDATE_AND_RESOLVE_WEB_FETCH_URL not registered",
                ))
            },
            resolve_web_fetch_redirect: |_, _| {
                Err(FrameworkError::validation(
                    "RESOLVE_WEB_FETCH_REDIRECT not registered",
                ))
            },
            resolve_web_fetch_addresses: |_, _| {
                Err(FrameworkError::validation(
                    "RESOLVE_WEB_FETCH_ADDRESSES not registered",
                ))
            },
            // mcp_pre_guard (1 field) — blocked by default
            evaluate_mcp_pre_guard_safe: |_, _, _| McpPreGuardVerdict {
                blocked: true,
                reason: Some(
                    "MCP pre-guard not initialized — rejected by default".to_string(),
                ),
            },
            // research_tool_dispatch (1 field)
            research_tool_dispatch: |_, _| {
                Err(FrameworkError::validation(
                    "research_tool_dispatch not registered",
                ))
            },
            // mcp_tool_routing (1 field)
            mcp_tool_skill_route: |_, _, _, _| {
                Err(FrameworkError::validation(
                    "MCP_TOOL_SKILL_ROUTE not registered",
                ))
            },
            // tool_dispatch (3 fields)
            tool_goal_state_manage_dispatch: |_, _, _| {
                Err(FrameworkError::validation(
                    "GOAL_STATE_MANAGE_DISPATCH not registered",
                ))
            },
            tool_closeout_record_write_dispatch: |_, _| {
                Err(FrameworkError::validation(
                    "CLOSEOUT_RECORD_WRITE_DISPATCH not registered",
                ))
            },
            tool_closeout_gate_evaluate: |_, _, _| {
                Err(FrameworkError::validation(
                    "CLOSEOUT_GATE_EVALUATE not registered",
                ))
            },
            // browser_dispatch (1 field)
            browser_dispatch: |_| {
                Err(FrameworkError::validation(
                    "browser-mcp dispatch not registered",
                ))
            },
            // runtime_trace_transport (2 fields)
            attach_runtime_event_transport: |_| {
                Err(FrameworkError::validation(
                    "ATTACH_RUNTIME_EVENT_TRANSPORT not registered",
                ))
            },
            inspect_trace_stream: |_| {
                Err(FrameworkError::validation(
                    "INSPECT_TRACE_STREAM not registered",
                ))
            },
        }
    }
}

static RUNTIME_HOOKS: Mutex<Option<RuntimeHooks>> = Mutex::new(None);

/// Get the consolidated RuntimeHooks struct. Returns `None` if not yet set (bootstrap not complete).
pub(crate) fn get_runtime_hooks() -> Option<RuntimeHooks> {
    let guard = match RUNTIME_HOOKS.lock() {
        Ok(g) => g,
        Err(e) => {
            tracing::error!("RUNTIME_HOOKS mutex poisoned — bootstrap thread panicked: {e}");
            e.into_inner()
        }
    };
    guard.clone()
}

/// Set the consolidated RuntimeHooks struct during bootstrap.
/// Second call from L5 (research-harness) replaces the struct with research-specific fields.
pub fn set_runtime_hooks(hooks: RuntimeHooks) {
    if let Ok(mut guard) = RUNTIME_HOOKS.lock() {
        let was_set = guard.is_some();
        *guard = Some(hooks);
        if was_set {
            tracing::debug!("RuntimeHooks replaced (L5 research-harness extension)");
        }
    }
}

/// Modify specific fields of the already-initialized RuntimeHooks struct in-place.
/// Used by L5 crates (research-harness) and browser-mcp dispatch to override
/// specific hook implementations after `runtime_core::init_hooks()`.
pub fn modify_runtime_hooks(f: impl FnOnce(&mut RuntimeHooks)) {
    if let Ok(mut guard) = RUNTIME_HOOKS.lock() {
        if let Some(hooks) = guard.as_mut() {
            f(hooks);
        }
    }
}

// ── Mirror type structural canary tests ──

#[cfg(test)]
mod mirror_type_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    /// Verify that mirrored constants match expected values.
    #[test]
    fn mirrored_constants_values() {
        assert_eq!(MAX_CONCURRENT_SUBAGENTS_LIMIT, 24);
    }

    /// Regression: host type aliases compile correctly as &'static str.
    #[test]
    fn host_type_aliases_are_static_str() {
        let _host: PaperProseHookHost = "codex";
        assert_eq!(_host, "codex");
    }
}
