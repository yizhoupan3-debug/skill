//! host-projection hooks proxy layer.
//!
//! host-projection cannot depend on runtime-core (circular dep), so this module provides
//! function-pointer slots for runtime-core functionality that cursor_hooks / codex_hooks need.
//! The host application registers real implementations at startup via `register_*` functions.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

// ────────────────────────────────────────────────────────────────
// Mirror types (avoid dependency on runtime-core definitions)
// ────────────────────────────────────────────────────────────────

/// Mirror of `runtime_core::router_rs_observation::HookObservationHost`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookObservationHost {
    Cursor,
    Codex,
    ClaudeCode,
}
pub type HookObservationHostType = HookObservationHost;

/// Mirror of `runtime_core::paper_prose_hook::PaperProseHookHost`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaperProseHookHost {
    Cursor,
    Codex,
    Claude,
}
pub type PaperProseHookHostType = PaperProseHookHost;

impl PaperProseHookHost {
    pub fn from_codex_lifecycle_state_dir(_state_dir_leaf: &str) -> Self {
        Self::Codex
    }
}

/// Mirror of `runtime_core::ship_readiness::GoalReadiness`.
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

pub fn router_rs_cursor_hook_legacy_subtracted_events_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS")
}

pub fn router_rs_cursor_hook_silent_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_HOOK_SILENT")
        || router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_SILENT")
}

pub fn router_rs_cursor_hook_outbound_context_max_bytes() -> usize {
    let key_canonical = "ROUTER_RS_HOOK_OUTBOUND_CONTEXT_MAX_CHARS";
    let key_legacy = "ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS";
    parse_env_usize(key_canonical)
        .or_else(|| parse_env_usize(key_legacy))
        .unwrap_or(4096)
}

pub fn router_rs_cursor_review_fork_context_missing_infer_false_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE")
}

pub fn router_rs_cursor_autopilot_pre_goal_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_AUTOPILOT_PRE_GOAL_ENABLED")
        || router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED")
}

pub fn router_rs_cursor_hook_state_lock_retries() -> u32 {
    parse_env_u32("ROUTER_RS_CURSOR_HOOK_STATE_LOCK_RETRIES").unwrap_or(8)
}

pub fn router_rs_cursor_hook_state_file_sync_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_STATE_FILE_SYNC")
}

pub fn router_rs_cursor_hook_state_dir_sync_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_STATE_DIR_SYNC")
}

pub fn router_rs_cursor_review_pending_cycle_max() -> usize {
    parse_env_usize("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX").unwrap_or(3)
}

pub fn router_rs_cursor_review_gate_stop_max_nudges_cap() -> Option<u32> {
    #[cfg(test)]
    {
        let raw = std::env::var("ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES")
            .ok()
            .or_else(|| std::env::var("ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES").ok());
        if raw.is_none() {
            return None;
        }
    }
    parse_env_u32("ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES")
        .or_else(|| parse_env_u32("ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES"))
}

pub fn router_rs_cursor_pre_goal_strict_disk_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK")
}

pub fn router_rs_cursor_hook_state_fail_open_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_STATE_FAIL_OPEN")
}

pub fn router_rs_cursor_cargo_check_sync_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_CARGO_CHECK_SYNC")
}

pub fn router_rs_cursor_hook_state_legacy_full_sweep_enabled() -> bool {
    router_rs_env_enabled_default_false("ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP")
}

pub fn router_rs_cursor_hook_state_stale_sweep_days() -> u64 {
    parse_env_u64("ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS").unwrap_or(7)
}

pub fn router_rs_cursor_sessionstart_context_max_bytes() -> usize {
    parse_env_usize("ROUTER_RS_CURSOR_SESSIONSTART_CONTEXT_MAX_BYTES").unwrap_or(64 * 1024)
}

fn parse_env_usize(var: &str) -> Option<usize> {
    std::env::var(var)
        .ok()
        .and_then(|v| v.trim().parse().ok())
}

fn parse_env_u32(var: &str) -> Option<u32> {
    std::env::var(var)
        .ok()
        .and_then(|v| v.trim().parse().ok())
}

fn parse_env_u64(var: &str) -> Option<u64> {
    std::env::var(var)
        .ok()
        .and_then(|v| v.trim().parse().ok())
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
    MARK_HOOK_START.set(mark_start).ok();
    ADD_LOCK_WAIT_MS.set(add_lock_wait).ok();
    ADD_CARGO_CHECK_MS.set(add_cargo).ok();
    EMIT_HOOK_TIMING_LINE.set(emit_line).ok();
}

pub fn mark_hook_start() {
    MARK_HOOK_START.get().map(|f| f());
}

pub fn add_lock_wait_ms(ms: u64) {
    ADD_LOCK_WAIT_MS.get().map(|f| f(ms));
}

pub fn add_cargo_check_ms(ms: u64) {
    ADD_CARGO_CHECK_MS.get().map(|f| f(ms));
}

pub fn emit_hook_timing_line(event: &str) {
    EMIT_HOOK_TIMING_LINE.get().map(|f| f(event));
}

// ────────────────────────────────────────────────────────────────
// telemetry_emit: function-pointer proxies (OnceLock)
// ────────────────────────────────────────────────────────────────

static EMIT_HOOK_FIRED: OnceLock<fn(&str, &str)> = OnceLock::new();
static EMIT_TOOL_CALL: OnceLock<fn(&str, u64, bool)> = OnceLock::new();
static HOOK_ACTION_FROM_OPTIONAL_OUTPUT: OnceLock<fn(Option<&Value>) -> &'static str> = OnceLock::new();

pub fn register_telemetry(
    emit_hook_fired: fn(&str, &str),
    emit_tool_call: fn(&str, u64, bool),
    hook_action: fn(Option<&Value>) -> &'static str,
) {
    EMIT_HOOK_FIRED.set(emit_hook_fired).ok();
    EMIT_TOOL_CALL.set(emit_tool_call).ok();
    HOOK_ACTION_FROM_OPTIONAL_OUTPUT.set(hook_action).ok();
}

pub fn emit_hook_fired(hook_name: &str, action: &str) {
    EMIT_HOOK_FIRED.get().map(|f| f(hook_name, action));
}

pub fn emit_tool_call(tool: &str, duration_ms: u64, success: bool) {
    EMIT_TOOL_CALL.get().map(|f| f(tool, duration_ms, success));
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

static INIT_TRACKER: OnceLock<fn(&Path) -> Result<(), String>> = OnceLock::new();
static RECORD_TOOL_CALL: OnceLock<fn(&Path, &str, Option<&Value>) -> Result<(), String>> = OnceLock::new();
static READ_TRACKER_STATE: OnceLock<fn(&Path) -> Result<Value, String>> = OnceLock::new();

pub fn register_session_call_tracker(
    init: fn(&Path) -> Result<(), String>,
    record: fn(&Path, &str, Option<&Value>) -> Result<(), String>,
    read_state: fn(&Path) -> Result<Value, String>,
) {
    INIT_TRACKER.set(init).ok();
    RECORD_TOOL_CALL.set(record).ok();
    READ_TRACKER_STATE.set(read_state).ok();
}

pub fn init_tracker(repo_root: &Path) -> Result<(), String> {
    INIT_TRACKER
        .get()
        .map(|f| f(repo_root))
        .unwrap_or(Ok(()))
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

static BUILD_FRAMEWORK_CONTRACT: OnceLock<fn(&Path) -> Result<Value, String>> = OnceLock::new();
static TRY_APPEND_POST_TOOL_SHELL: OnceLock<fn(&Path, &Value, &str) -> Result<(), String>> = OnceLock::new();
static CLOSEOUT_ENFORCEMENT: OnceLock<fn() -> bool> = OnceLock::new();
static CLOSEOUT_RECORD_PATH: OnceLock<fn(&Path, &str) -> Result<PathBuf, String>> = OnceLock::new();
static EVALUATE_CLOSEOUT: OnceLock<fn(&Path, &str, &Path) -> Result<Value, String>> = OnceLock::new();
static FIRST_TASK_ID: OnceLock<fn(&Path) -> Option<String>> = OnceLock::new();
static EVIDENCE_APPEND: OnceLock<fn(Value) -> Result<Value, String>> = OnceLock::new();
static EXTRACT_DURATION: OnceLock<fn(&Value) -> Option<u64>> = OnceLock::new();
static POST_TOOL_SUCCEEDED: OnceLock<fn(&Value) -> bool> = OnceLock::new();
static CLOSEOUT_STOP_FOLLOWUP: OnceLock<fn(&Path, &str) -> Option<String>> = OnceLock::new();

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
    BUILD_FRAMEWORK_CONTRACT.set(build_contract).ok();
    TRY_APPEND_POST_TOOL_SHELL.set(append_shell).ok();
    CLOSEOUT_ENFORCEMENT.set(enforcement).ok();
    CLOSEOUT_RECORD_PATH.set(record_path).ok();
    EVALUATE_CLOSEOUT.set(eval_closeout).ok();
    FIRST_TASK_ID.set(first_task).ok();
    EVIDENCE_APPEND.set(evidence_append).ok();
    EXTRACT_DURATION.set(extract_duration).ok();
    POST_TOOL_SUCCEEDED.set(post_tool_ok).ok();
    CLOSEOUT_STOP_FOLLOWUP.set(closeout_followup).ok();
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

pub fn closeout_stop_followup_for_completion_text(
    repo_root: &Path,
    text: &str,
) -> Option<String> {
    CLOSEOUT_STOP_FOLLOWUP.get().map(|f| f(repo_root, text)).unwrap_or(None)
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
    ATTACH_OBSERVATION.set(attach).ok();
    STRIP_OBSERVATION.set(strip).ok();
}

pub fn attach_router_rs_observation(output: &mut Value, host: HookObservationHost) {
    ATTACH_OBSERVATION.get().map(|f| f(output, host));
}

pub fn strip_router_rs_observation(output: &mut Value) {
    STRIP_OBSERVATION.get().map(|f| f(output));
}

// ────────────────────────────────────────────────────────────────
// hook_outbound_protect: function-pointer proxies (OnceLock)
// ────────────────────────────────────────────────────────────────

static OUTBOUND_PROTECTED: OnceLock<fn(&str) -> bool> = OnceLock::new();
static TRUNCATE_OUTBOUND: OnceLock<fn(&str, usize, &str) -> String> = OnceLock::new();

pub fn register_hook_outbound_protect(
    is_protected: fn(&str) -> bool,
    truncate: fn(&str, usize, &str) -> String,
) {
    OUTBOUND_PROTECTED.set(is_protected).ok();
    TRUNCATE_OUTBOUND.set(truncate).ok();
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

// ────────────────────────────────────────────────────────────────
// hook_posttool_normalize: function-pointer proxy (OnceLock)
// ────────────────────────────────────────────────────────────────

static SYNTHETIC_POST_TOOL: OnceLock<fn(&Value) -> Value> = OnceLock::new();

pub fn register_hook_posttool_normalize(f: fn(&Value) -> Value) {
    SYNTHETIC_POST_TOOL.set(f).ok();
}

pub fn synthetic_post_tool_evidence_shape(event: &Value) -> Value {
    SYNTHETIC_POST_TOOL
        .get()
        .map(|f| f(event))
        .unwrap_or_else(|| serde_json::json!({}))
}

// ────────────────────────────────────────────────────────────────
// ship_readiness: function-pointer proxies (OnceLock)
// ────────────────────────────────────────────────────────────────

static EVAL_GOAL_READINESS: OnceLock<fn(&Path, &Value, &str) -> GoalReadiness> = OnceLock::new();
static GOAL_STOP_FOLLOWUP: OnceLock<fn(bool, bool, bool, u32) -> String> = OnceLock::new();

pub fn register_ship_readiness(
    evaluate: fn(&Path, &Value, &str) -> GoalReadiness,
    followup: fn(bool, bool, bool, u32) -> String,
) {
    EVAL_GOAL_READINESS.set(evaluate).ok();
    GOAL_STOP_FOLLOWUP.set(followup).ok();
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

static APPEND_PROSE: OnceLock<fn(&Path, &str, &mut Vec<String>, PaperProseHookHost)> = OnceLock::new();
static MERGE_PROSE: OnceLock<fn(&Path, &mut Value, &str, bool)> = OnceLock::new();
static APPEND_ADVERSARIAL: OnceLock<fn(&Path, &str, &mut Vec<String>, PaperProseHookHost)> = OnceLock::new();
static MERGE_ADVERSARIAL: OnceLock<fn(&Path, &mut Value, &str, bool)> = OnceLock::new();

pub fn register_paper_hooks(
    append_prose: fn(&Path, &str, &mut Vec<String>, PaperProseHookHost),
    merge_prose: fn(&Path, &mut Value, &str, bool),
    append_adversarial: fn(&Path, &str, &mut Vec<String>, PaperProseHookHost),
    merge_adversarial: fn(&Path, &mut Value, &str, bool),
) {
    APPEND_PROSE.set(append_prose).ok();
    MERGE_PROSE.set(merge_prose).ok();
    APPEND_ADVERSARIAL.set(append_adversarial).ok();
    MERGE_ADVERSARIAL.set(merge_adversarial).ok();
}

pub fn maybe_append_paper_prose_context(
    repo_root: &Path,
    prompt_text: &str,
    contexts: &mut Vec<String>,
    host: PaperProseHookHost,
) {
    APPEND_PROSE.get().map(|f| f(repo_root, prompt_text, contexts, host));
}

pub fn maybe_merge_paper_prose_before_submit(
    repo_root: &Path,
    output: &mut Value,
    prompt_text: &str,
    use_followup_message: bool,
) {
    MERGE_PROSE.get().map(|f| f(repo_root, output, prompt_text, use_followup_message));
}

pub fn maybe_append_paper_adversarial_context(
    repo_root: &Path,
    prompt_text: &str,
    contexts: &mut Vec<String>,
    host: PaperProseHookHost,
) {
    APPEND_ADVERSARIAL.get().map(|f| f(repo_root, prompt_text, contexts, host));
}

pub fn maybe_merge_paper_adversarial_before_submit(
    repo_root: &Path,
    output: &mut Value,
    prompt_text: &str,
    use_followup_message: bool,
) {
    MERGE_ADVERSARIAL.get().map(|f| f(repo_root, output, prompt_text, use_followup_message));
}

// ────────────────────────────────────────────────────────────────
// kernel_bootstrap: function-pointer proxy (OnceLock)
// ────────────────────────────────────────────────────────────────

static ENSURE_KERNEL: OnceLock<fn()> = OnceLock::new();

pub fn register_kernel_bootstrap(f: fn()) {
    ENSURE_KERNEL.set(f).ok();
}

pub fn ensure_kernel_bootstrap() {
    ENSURE_KERNEL.get().map(|f| f());
    // In test builds, install the test deps (tokenizer, review context probes)
    // as a fallback when no real kernel bootstrap is registered.
    #[cfg(test)]
    install_test_deps();
}

// ────────────────────────────────────────────────────────────────
// harness_operator_nudges: test-only proxy
// ────────────────────────────────────────────────────────────────

static HARNESS_NUDGE_TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

/// Test-only env lock. Replaces `harness_operator_nudges::harness_nudges_env_test_lock`.
#[cfg(test)]
pub fn harness_nudges_env_test_lock() -> std::sync::MutexGuard<'static, ()> {
    HARNESS_NUDGE_TEST_MUTEX
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap()
}

// ────────────────────────────────────────────────────────────────
// Test bootstrap: install tokenizer + review context probes
// ────────────────────────────────────────────────────────────────

/// Install a simple whitespace tokenizer and no-op review context probes so that
/// `core_policy::hook_common` functions work in host-projection tests.
/// This replaces the `kernel_bootstrap::ensure_kernel_bootstrap()` call that
/// runtime-core tests rely on.
#[cfg(test)]
pub fn install_test_deps() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        struct WhitespaceTokenizer;
        impl framework_kernel::TokenizerProvider for WhitespaceTokenizer {
            fn tokenize_query(&self, text: &str) -> Vec<String> {
                text.split_whitespace()
                    .map(|s| s.to_ascii_lowercase())
                    .collect()
            }
            fn has_parallel_review_candidate_context(
                &self,
                _query: &str,
                _tokens: &[String],
            ) -> bool {
                false
            }
        }
        framework_kernel::install_tokenizer_provider(Box::new(WhitespaceTokenizer));
        // Install no-op review context probes (the test version in core-policy is cfg(test)-only).
        core_policy::review_context_signals::install_review_context_probes(
            |_text, _tokens| false,
            |_text, _tokens| false,
        );
    });
}

// ────────────────────────────────────────────────────────────────
// Additional hooks needed by claude_code_hooks / mcp_stdio_harness
// (appended during host-projection hooks consolidation)
// ────────────────────────────────────────────────────────────────

// Framework Runtime (additional)
static RESOLVE_REPO_ROOT_ARG: OnceLock<fn(Option<&Path>) -> Result<PathBuf, String>> = OnceLock::new();
static CURRENT_LOCAL_TIMESTAMP: OnceLock<fn() -> String> = OnceLock::new();
static WRITE_FRAMEWORK_SESSION_ARTARTIFACTS: OnceLock<fn(Value) -> Result<Value, String>> = OnceLock::new();
static ROUTE_TASK_WITH_MANIFEST_FALLBACK: OnceLock<fn(&[routing_engine::route::SkillRecord], Option<&Path>, Option<&Path>, Option<&str>, &str, &str, bool, bool) -> Result<RouteDecision, String>> = OnceLock::new();
static BUILD_FRAMEWORK_RUNTIME_SNAPSHOT_ENVELOPE: OnceLock<fn(&Path, Option<&Path>, Option<&str>) -> Result<Value, String>> = OnceLock::new();
static BUILD_AUTOMATIC_CONTINUITY_CHECKPOINT_PAYLOAD: OnceLock<fn(&Path, &str, &str, Option<&str>, bool, bool) -> Value> = OnceLock::new();
static APPEND_EVIDENCE_INDEX: OnceLock<fn(&Path, Option<&str>, serde_json::Map<String, Value>) -> Result<(), String>> = OnceLock::new();
static HOOK_ACTION_FROM_OUTPUT: OnceLock<fn(&Value) -> &'static str> = OnceLock::new();
static CLOSEOUT_RECORD_SCHEMA_VERSION_FN: OnceLock<fn() -> &'static str> = OnceLock::new();
static CHECK_ANOMALIES: OnceLock<fn(&Path) -> Result<Vec<String>, String>> = OnceLock::new();

// Web Fetch Guard
static VALIDATE_AND_RESOLVE_WEB_FETCH_URL: OnceLock<fn(&str) -> Result<(String, Vec<String>), String>> = OnceLock::new();
static RESOLVE_WEB_FETCH_REDIRECT: OnceLock<fn(&str, &str) -> Result<String, String>> = OnceLock::new();
static RESOLVE_WEB_FETCH_ADDRESSES: OnceLock<fn(&str, u16) -> Result<Vec<String>, String>> = OnceLock::new();

// MCP Pre Guard
static EVALUATE_MCP_PRE_GUARD_SAFE: OnceLock<fn(&str, &Value, &Path) -> McpPreGuardVerdict> = OnceLock::new();

// Router Env Flags (additional)
static ROUTER_RS_SKIP_PRE_TOOL_USE_GUARD: OnceLock<fn() -> bool> = OnceLock::new();

pub fn register_framework_runtime_extra(
    resolve_repo_root_arg: fn(Option<&Path>) -> Result<PathBuf, String>,
    current_local_timestamp: fn() -> String,
    write_framework_session_artifacts: fn(Value) -> Result<Value, String>,
    route_task_with_manifest_fallback: fn(&[routing_engine::route::SkillRecord], Option<&Path>, Option<&Path>, Option<&str>, &str, &str, bool, bool) -> Result<RouteDecision, String>,
    build_framework_runtime_snapshot_envelope: fn(&Path, Option<&Path>, Option<&str>) -> Result<Value, String>,
    build_automatic_continuity_checkpoint_payload: fn(&Path, &str, &str, Option<&str>, bool, bool) -> Value,
    append_evidence_index: fn(&Path, Option<&str>, serde_json::Map<String, Value>) -> Result<(), String>,
    hook_action_from_output: fn(&Value) -> &'static str,
    closeout_record_schema_version: fn() -> &'static str,
    check_anomalies: fn(&Path) -> Result<Vec<String>, String>,
) {
    RESOLVE_REPO_ROOT_ARG.set(resolve_repo_root_arg).ok();
    CURRENT_LOCAL_TIMESTAMP.set(current_local_timestamp).ok();
    WRITE_FRAMEWORK_SESSION_ARTARTIFACTS.set(write_framework_session_artifacts).ok();
    ROUTE_TASK_WITH_MANIFEST_FALLBACK.set(route_task_with_manifest_fallback).ok();
    BUILD_FRAMEWORK_RUNTIME_SNAPSHOT_ENVELOPE.set(build_framework_runtime_snapshot_envelope).ok();
    BUILD_AUTOMATIC_CONTINUITY_CHECKPOINT_PAYLOAD.set(build_automatic_continuity_checkpoint_payload).ok();
    APPEND_EVIDENCE_INDEX.set(append_evidence_index).ok();
    HOOK_ACTION_FROM_OUTPUT.set(hook_action_from_output).ok();
    CLOSEOUT_RECORD_SCHEMA_VERSION_FN.set(closeout_record_schema_version).ok();
    CHECK_ANOMALIES.set(check_anomalies).ok();
}

pub fn register_web_fetch_guard_extra(
    validate_url: fn(&str) -> Result<(String, Vec<String>), String>,
    resolve_redirect: fn(&str, &str) -> Result<String, String>,
    resolve_addresses: fn(&str, u16) -> Result<Vec<String>, String>,
) {
    VALIDATE_AND_RESOLVE_WEB_FETCH_URL.set(validate_url).ok();
    RESOLVE_WEB_FETCH_REDIRECT.set(resolve_redirect).ok();
    RESOLVE_WEB_FETCH_ADDRESSES.set(resolve_addresses).ok();
}

pub fn register_mcp_pre_guard_extra(
    evaluate: fn(&str, &Value, &Path) -> McpPreGuardVerdict,
) {
    EVALUATE_MCP_PRE_GUARD_SAFE.set(evaluate).ok();
}

pub fn register_router_env_flags_extra(
    skip_pre_tool_use_guard: fn() -> bool,
) {
    ROUTER_RS_SKIP_PRE_TOOL_USE_GUARD.set(skip_pre_tool_use_guard).ok();
}

pub fn resolve_repo_root_arg(repo_root: Option<&Path>) -> Result<PathBuf, String> {
    RESOLVE_REPO_ROOT_ARG.get().map(|f| f(repo_root)).unwrap_or_else(|| {
        // Default: use CARGO_MANIFEST_DIR or current dir
        std::env::current_dir().map_err(|e| e.to_string())
    })
}

pub fn current_local_timestamp() -> String {
    CURRENT_LOCAL_TIMESTAMP.get().map(|f| f()).unwrap_or_else(|| "1970-01-01T00:00:00Z".into())
}

pub fn write_framework_session_artifacts(payload: Value) -> Result<Value, String> {
    WRITE_FRAMEWORK_SESSION_ARTARTIFACTS.get().map(|f| f(payload)).unwrap_or_else(|| Err("hooks not registered".into()))
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
        .map(|f| f(runtime_records, runtime_path, manifest_path, host_id, query, session_id, allow_overlay, first_turn))
        .unwrap_or_else(|| Err("hooks not registered".into()))
}

pub fn build_framework_runtime_snapshot_envelope(repo_root: &Path, artifact_root_override: Option<&Path>, task_id_override: Option<&str>) -> Result<Value, String> {
    BUILD_FRAMEWORK_RUNTIME_SNAPSHOT_ENVELOPE.get().map(|f| f(repo_root, artifact_root_override, task_id_override)).unwrap_or_else(|| Err("hooks not registered".into()))
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
        .map(|f| f(repo_root, task_line, summary_text, task_id, repointer_focus, update_registry_only_if_known))
        .unwrap_or(Value::Null)
}

pub fn check_anomalies(repo_root: &Path) -> Result<Vec<String>, String> {
    CHECK_ANOMALIES.get().map(|f| f(repo_root)).unwrap_or_else(|| Ok(vec![]))
}

pub fn append_evidence_index(repo_root: &Path, task_id: Option<&str>, entry: serde_json::Map<String, Value>) -> Result<(), String> {
    APPEND_EVIDENCE_INDEX.get().map(|f| f(repo_root, task_id, entry)).unwrap_or_else(|| Err("hooks not registered".into()))
}

pub fn hook_action_from_output(output: &Value) -> &'static str {
    HOOK_ACTION_FROM_OUTPUT.get().map(|f| f(output)).unwrap_or("unknown")
}

pub fn closeout_record_schema_version() -> &'static str {
    CLOSEOUT_RECORD_SCHEMA_VERSION_FN.get().map(|f| f()).unwrap_or("closeout-record-v1")
}

pub fn validate_and_resolve_web_fetch_url(url: &str) -> Result<(String, Vec<String>), String> {
    VALIDATE_AND_RESOLVE_WEB_FETCH_URL.get().map(|f| f(url)).unwrap_or_else(|| Err("hooks not registered".into()))
}

pub fn resolve_web_fetch_redirect(base: &str, location: &str) -> Result<String, String> {
    RESOLVE_WEB_FETCH_REDIRECT.get().map(|f| f(base, location)).unwrap_or_else(|| Err("hooks not registered".into()))
}

pub fn resolve_web_fetch_addresses(host: &str, port: u16) -> Result<Vec<String>, String> {
    RESOLVE_WEB_FETCH_ADDRESSES.get().map(|f| f(host, port)).unwrap_or_else(|| Err("hooks not registered".into()))
}

pub fn evaluate_mcp_pre_guard_safe(tool_name: &str, arguments: &Value, repo_root: &Path) -> McpPreGuardVerdict {
    EVALUATE_MCP_PRE_GUARD_SAFE.get().map(|f| f(tool_name, arguments, repo_root)).unwrap_or(McpPreGuardVerdict { blocked: false, reason: None })
}

pub fn router_rs_skip_pre_tool_use_guard() -> bool {
    ROUTER_RS_SKIP_PRE_TOOL_USE_GUARD.get().map(|f| f()).unwrap_or(false)
}
