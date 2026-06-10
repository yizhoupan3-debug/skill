//! host-projection hooks proxy layer.
//!
//! host-projection cannot depend on runtime-core (circular dep), so this module provides
//! function-pointer slots for runtime-core functionality that cursor_hooks / codex_hooks need.
//! The host application registers real implementations at startup via `register_*` functions.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, RwLock};

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
        .unwrap_or(8192)
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
// hook_timing: function-pointer proxies
// ────────────────────────────────────────────────────────────────

type FnMarkHookStart = fn();
type FnAddLockWaitMs = fn(u64);
type FnAddCargoCheckMs = fn(u64);
type FnEmitHookTimingLine = fn(&str);

static FN_MARK_HOOK_START: RwLock<Option<FnMarkHookStart>> = RwLock::new(None);
static FN_ADD_LOCK_WAIT_MS: RwLock<Option<FnAddLockWaitMs>> = RwLock::new(None);
static FN_ADD_CARGO_CHECK_MS: RwLock<Option<FnAddCargoCheckMs>> = RwLock::new(None);
static FN_EMIT_HOOK_TIMING_LINE: RwLock<Option<FnEmitHookTimingLine>> = RwLock::new(None);

pub fn register_hook_timing(
    mark_start: FnMarkHookStart,
    add_lock_wait: FnAddLockWaitMs,
    add_cargo: FnAddCargoCheckMs,
    emit_line: FnEmitHookTimingLine,
) {
    *FN_MARK_HOOK_START.write().unwrap() = Some(mark_start);
    *FN_ADD_LOCK_WAIT_MS.write().unwrap() = Some(add_lock_wait);
    *FN_ADD_CARGO_CHECK_MS.write().unwrap() = Some(add_cargo);
    *FN_EMIT_HOOK_TIMING_LINE.write().unwrap() = Some(emit_line);
}

pub fn mark_hook_start() {
    if let Some(f) = *FN_MARK_HOOK_START.read().unwrap() {
        f();
    }
}

pub fn add_lock_wait_ms(ms: u64) {
    if let Some(f) = *FN_ADD_LOCK_WAIT_MS.read().unwrap() {
        f(ms);
    }
}

pub fn add_cargo_check_ms(ms: u64) {
    if let Some(f) = *FN_ADD_CARGO_CHECK_MS.read().unwrap() {
        f(ms);
    }
}

pub fn emit_hook_timing_line(event: &str) {
    if let Some(f) = *FN_EMIT_HOOK_TIMING_LINE.read().unwrap() {
        f(event);
    }
}

// ────────────────────────────────────────────────────────────────
// telemetry_emit: function-pointer proxies
// ────────────────────────────────────────────────────────────────

type FnEmitHookFired = fn(&str, &str);
type FnEmitToolCall = fn(&str, u64, bool);
type FnHookActionFromOptionalOutput = fn(Option<&Value>) -> &'static str;

static FN_EMIT_HOOK_FIRED: RwLock<Option<FnEmitHookFired>> = RwLock::new(None);
static FN_EMIT_TOOL_CALL: RwLock<Option<FnEmitToolCall>> = RwLock::new(None);
static FN_HOOK_ACTION_FROM_OPTIONAL_OUTPUT: RwLock<Option<FnHookActionFromOptionalOutput>> =
    RwLock::new(None);

pub fn register_telemetry(
    emit_hook_fired: FnEmitHookFired,
    emit_tool_call: FnEmitToolCall,
    hook_action: FnHookActionFromOptionalOutput,
) {
    *FN_EMIT_HOOK_FIRED.write().unwrap() = Some(emit_hook_fired);
    *FN_EMIT_TOOL_CALL.write().unwrap() = Some(emit_tool_call);
    *FN_HOOK_ACTION_FROM_OPTIONAL_OUTPUT.write().unwrap() = Some(hook_action);
}

pub fn emit_hook_fired(hook_name: &str, action: &str) {
    if let Some(f) = *FN_EMIT_HOOK_FIRED.read().unwrap() {
        f(hook_name, action);
    }
}

pub fn emit_tool_call(tool: &str, duration_ms: u64, success: bool) {
    if let Some(f) = *FN_EMIT_TOOL_CALL.read().unwrap() {
        f(tool, duration_ms, success);
    }
}

pub fn hook_action_from_optional_output(output: Option<&Value>) -> &'static str {
    if let Some(f) = *FN_HOOK_ACTION_FROM_OPTIONAL_OUTPUT.read().unwrap() {
        return f(output);
    }
    "unknown"
}

// ────────────────────────────────────────────────────────────────
// session_call_tracker: function-pointer proxies
// ────────────────────────────────────────────────────────────────

type FnInitTracker = fn(&Path) -> Result<(), String>;
type FnRecordToolCall = fn(&Path, &str, Option<&Value>) -> Result<(), String>;
type FnReadTrackerState = fn(&Path) -> Result<Value, String>;

static FN_INIT_TRACKER: RwLock<Option<FnInitTracker>> = RwLock::new(None);
static FN_RECORD_TOOL_CALL: RwLock<Option<FnRecordToolCall>> = RwLock::new(None);
static FN_READ_TRACKER_STATE: RwLock<Option<FnReadTrackerState>> = RwLock::new(None);

pub fn register_session_call_tracker(
    init: FnInitTracker,
    record: FnRecordToolCall,
    read_state: FnReadTrackerState,
) {
    *FN_INIT_TRACKER.write().unwrap() = Some(init);
    *FN_RECORD_TOOL_CALL.write().unwrap() = Some(record);
    *FN_READ_TRACKER_STATE.write().unwrap() = Some(read_state);
}

pub fn init_tracker(repo_root: &Path) -> Result<(), String> {
    if let Some(f) = *FN_INIT_TRACKER.read().unwrap() {
        return f(repo_root);
    }
    Ok(())
}

pub fn record_tool_call(
    repo_root: &Path,
    tool_name: &str,
    cache_stats: Option<&Value>,
) -> Result<(), String> {
    if let Some(f) = *FN_RECORD_TOOL_CALL.read().unwrap() {
        return f(repo_root, tool_name, cache_stats);
    }
    Ok(())
}

pub fn read_tracker_state(repo_root: &Path) -> Result<Value, String> {
    if let Some(f) = *FN_READ_TRACKER_STATE.read().unwrap() {
        return f(repo_root);
    }
    Ok(serde_json::json!({}))
}

// ────────────────────────────────────────────────────────────────
// framework_runtime: function-pointer proxies
// ────────────────────────────────────────────────────────────────

type FnBuildFrameworkContractSummaryEnvelope = fn(&Path) -> Result<Value, String>;
type FnTryAppendPostToolShellEvidence = fn(&Path, &Value, &str) -> Result<(), String>;
type FnCloseoutProgrammaticEnforcementEnabled = fn() -> bool;
type FnCloseoutRecordPathForTask = fn(&Path, &str) -> Result<PathBuf, String>;
type FnEvaluateCloseoutRecordFileForTask = fn(&Path, &str, &Path) -> Result<Value, String>;
type FnFirstTaskIdFromRegistry = fn(&Path) -> Option<String>;
type FnFrameworkHookEvidenceAppend = fn(Value) -> Result<Value, String>;
type FnExtractPostToolDurationMs = fn(&Value) -> Option<u64>;
type FnPostToolCallSucceeded = fn(&Value) -> bool;
type FnCloseoutStopFollowupForCompletionText = fn(&Path, &str) -> Option<String>;

static FN_BUILD_FRAMEWORK_CONTRACT: RwLock<Option<FnBuildFrameworkContractSummaryEnvelope>> =
    RwLock::new(None);
static FN_TRY_APPEND_POST_TOOL_SHELL: RwLock<Option<FnTryAppendPostToolShellEvidence>> =
    RwLock::new(None);
static FN_CLOSEOUT_ENFORCEMENT: RwLock<Option<FnCloseoutProgrammaticEnforcementEnabled>> =
    RwLock::new(None);
static FN_CLOSEOUT_RECORD_PATH: RwLock<Option<FnCloseoutRecordPathForTask>> = RwLock::new(None);
static FN_EVALUATE_CLOSEOUT: RwLock<Option<FnEvaluateCloseoutRecordFileForTask>> = RwLock::new(None);
static FN_FIRST_TASK_ID: RwLock<Option<FnFirstTaskIdFromRegistry>> = RwLock::new(None);
static FN_EVIDENCE_APPEND: RwLock<Option<FnFrameworkHookEvidenceAppend>> = RwLock::new(None);
static FN_EXTRACT_DURATION: RwLock<Option<FnExtractPostToolDurationMs>> = RwLock::new(None);
static FN_POST_TOOL_SUCCEEDED: RwLock<Option<FnPostToolCallSucceeded>> = RwLock::new(None);
static FN_CLOSEOUT_STOP_FOLLOWUP: RwLock<Option<FnCloseoutStopFollowupForCompletionText>> =
    RwLock::new(None);

pub fn register_framework_runtime(
    build_contract: FnBuildFrameworkContractSummaryEnvelope,
    append_shell: FnTryAppendPostToolShellEvidence,
    enforcement: FnCloseoutProgrammaticEnforcementEnabled,
    record_path: FnCloseoutRecordPathForTask,
    eval_closeout: FnEvaluateCloseoutRecordFileForTask,
    first_task: FnFirstTaskIdFromRegistry,
    evidence_append: FnFrameworkHookEvidenceAppend,
    extract_duration: FnExtractPostToolDurationMs,
    post_tool_ok: FnPostToolCallSucceeded,
    closeout_followup: FnCloseoutStopFollowupForCompletionText,
) {
    *FN_BUILD_FRAMEWORK_CONTRACT.write().unwrap() = Some(build_contract);
    *FN_TRY_APPEND_POST_TOOL_SHELL.write().unwrap() = Some(append_shell);
    *FN_CLOSEOUT_ENFORCEMENT.write().unwrap() = Some(enforcement);
    *FN_CLOSEOUT_RECORD_PATH.write().unwrap() = Some(record_path);
    *FN_EVALUATE_CLOSEOUT.write().unwrap() = Some(eval_closeout);
    *FN_FIRST_TASK_ID.write().unwrap() = Some(first_task);
    *FN_EVIDENCE_APPEND.write().unwrap() = Some(evidence_append);
    *FN_EXTRACT_DURATION.write().unwrap() = Some(extract_duration);
    *FN_POST_TOOL_SUCCEEDED.write().unwrap() = Some(post_tool_ok);
    *FN_CLOSEOUT_STOP_FOLLOWUP.write().unwrap() = Some(closeout_followup);
}

pub fn build_framework_contract_summary_envelope(repo_root: &Path) -> Result<Value, String> {
    if let Some(f) = *FN_BUILD_FRAMEWORK_CONTRACT.read().unwrap() {
        return f(repo_root);
    }
    Err("framework_runtime not registered".into())
}

pub fn try_append_post_tool_shell_evidence(
    repo_root: &Path,
    event: &Value,
    kind: &str,
) -> Result<(), String> {
    if let Some(f) = *FN_TRY_APPEND_POST_TOOL_SHELL.read().unwrap() {
        return f(repo_root, event, kind);
    }
    Ok(())
}

pub fn closeout_programmatic_enforcement_enabled() -> bool {
    if let Some(f) = *FN_CLOSEOUT_ENFORCEMENT.read().unwrap() {
        return f();
    }
    false
}

pub fn closeout_record_path_for_task(repo_root: &Path, task_id: &str) -> Result<PathBuf, String> {
    if let Some(f) = *FN_CLOSEOUT_RECORD_PATH.read().unwrap() {
        return f(repo_root, task_id);
    }
    Err("framework_runtime not registered".into())
}

pub fn evaluate_closeout_record_file_for_task(
    repo_root: &Path,
    task_id: &str,
    record_path: &Path,
) -> Result<Value, String> {
    if let Some(f) = *FN_EVALUATE_CLOSEOUT.read().unwrap() {
        return f(repo_root, task_id, record_path);
    }
    Err("framework_runtime not registered".into())
}

pub fn first_task_id_from_registry(repo_root: &Path) -> Option<String> {
    if let Some(f) = *FN_FIRST_TASK_ID.read().unwrap() {
        return f(repo_root);
    }
    None
}

pub fn framework_hook_evidence_append(payload: Value) -> Result<Value, String> {
    if let Some(f) = *FN_EVIDENCE_APPEND.read().unwrap() {
        return f(payload);
    }
    Err("framework_runtime not registered".into())
}

pub fn extract_post_tool_duration_ms(event: &Value) -> Option<u64> {
    if let Some(f) = *FN_EXTRACT_DURATION.read().unwrap() {
        return f(event);
    }
    None
}

pub fn post_tool_call_succeeded(event: &Value) -> bool {
    if let Some(f) = *FN_POST_TOOL_SUCCEEDED.read().unwrap() {
        return f(event);
    }
    true
}

pub fn closeout_stop_followup_for_completion_text(
    repo_root: &Path,
    text: &str,
) -> Option<String> {
    if let Some(f) = *FN_CLOSEOUT_STOP_FOLLOWUP.read().unwrap() {
        return f(repo_root, text);
    }
    None
}

// ────────────────────────────────────────────────────────────────
// router_rs_observation: function-pointer proxies
// ────────────────────────────────────────────────────────────────

type FnAttachRouterRsObservation = fn(&mut Value, HookObservationHost);
type FnStripRouterRsObservation = fn(&mut Value);

static FN_ATTACH_OBSERVATION: RwLock<Option<FnAttachRouterRsObservation>> = RwLock::new(None);
static FN_STRIP_OBSERVATION: RwLock<Option<FnStripRouterRsObservation>> = RwLock::new(None);

pub fn register_router_rs_observation(
    attach: FnAttachRouterRsObservation,
    strip: FnStripRouterRsObservation,
) {
    *FN_ATTACH_OBSERVATION.write().unwrap() = Some(attach);
    *FN_STRIP_OBSERVATION.write().unwrap() = Some(strip);
}

pub fn attach_router_rs_observation(output: &mut Value, host: HookObservationHost) {
    if let Some(f) = *FN_ATTACH_OBSERVATION.read().unwrap() {
        f(output, host);
    }
}

pub fn strip_router_rs_observation(output: &mut Value) {
    if let Some(f) = *FN_STRIP_OBSERVATION.read().unwrap() {
        f(output);
    }
}

// ────────────────────────────────────────────────────────────────
// hook_outbound_protect: function-pointer proxies
// ────────────────────────────────────────────────────────────────

type FnHookOutboundLineProtected = fn(&str) -> bool;
type FnTruncateOutboundLines = fn(&str, usize, &str) -> String;

static FN_OUTBOUND_PROTECTED: RwLock<Option<FnHookOutboundLineProtected>> = RwLock::new(None);
static FN_TRUNCATE_OUTBOUND: RwLock<Option<FnTruncateOutboundLines>> = RwLock::new(None);

pub fn register_hook_outbound_protect(
    is_protected: FnHookOutboundLineProtected,
    truncate: FnTruncateOutboundLines,
) {
    *FN_OUTBOUND_PROTECTED.write().unwrap() = Some(is_protected);
    *FN_TRUNCATE_OUTBOUND.write().unwrap() = Some(truncate);
}

pub fn hook_outbound_line_is_framework_protected(line: &str) -> bool {
    if let Some(f) = *FN_OUTBOUND_PROTECTED.read().unwrap() {
        return f(line);
    }
    false
}

pub fn truncate_hook_outbound_lines_preserving(
    combined: &str,
    max_bytes: usize,
    suffix: &str,
) -> String {
    if let Some(f) = *FN_TRUNCATE_OUTBOUND.read().unwrap() {
        return f(combined, max_bytes, suffix);
    }
    combined.to_string()
}

// ────────────────────────────────────────────────────────────────
// hook_posttool_normalize: function-pointer proxy
// ────────────────────────────────────────────────────────────────

type FnSyntheticPostToolEvidenceShape = fn(&Value) -> Value;

static FN_SYNTHETIC_POST_TOOL: RwLock<Option<FnSyntheticPostToolEvidenceShape>> = RwLock::new(None);

pub fn register_hook_posttool_normalize(f: FnSyntheticPostToolEvidenceShape) {
    *FN_SYNTHETIC_POST_TOOL.write().unwrap() = Some(f);
}

pub fn synthetic_post_tool_evidence_shape(event: &Value) -> Value {
    if let Some(f) = *FN_SYNTHETIC_POST_TOOL.read().unwrap() {
        return f(event);
    }
    serde_json::json!({})
}

// ────────────────────────────────────────────────────────────────
// ship_readiness: function-pointer proxies
// ────────────────────────────────────────────────────────────────

type FnEvaluateGoalReadiness = fn(&Path, &Value, &str) -> GoalReadiness;
type FnGoalStopFollowupLine = fn(bool, bool, bool, u32) -> String;

static FN_EVAL_GOAL_READINESS: RwLock<Option<FnEvaluateGoalReadiness>> = RwLock::new(None);
static FN_GOAL_STOP_FOLLOWUP: RwLock<Option<FnGoalStopFollowupLine>> = RwLock::new(None);

pub fn register_ship_readiness(
    evaluate: FnEvaluateGoalReadiness,
    followup: FnGoalStopFollowupLine,
) {
    *FN_EVAL_GOAL_READINESS.write().unwrap() = Some(evaluate);
    *FN_GOAL_STOP_FOLLOWUP.write().unwrap() = Some(followup);
}

pub fn evaluate_goal_readiness_from_disk(
    repo_root: &Path,
    goal: &Value,
    task_id: &str,
) -> GoalReadiness {
    if let Some(f) = *FN_EVAL_GOAL_READINESS.read().unwrap() {
        return f(repo_root, goal, task_id);
    }
    GoalReadiness::default()
}

pub fn goal_stop_followup_line(
    contract: bool,
    progress: bool,
    verification: bool,
    goal_followup_count: u32,
) -> String {
    if let Some(f) = *FN_GOAL_STOP_FOLLOWUP.read().unwrap() {
        return f(contract, progress, verification, goal_followup_count);
    }
    String::new()
}

// ────────────────────────────────────────────────────────────────
// paper hooks: function-pointer proxies
// ────────────────────────────────────────────────────────────────

type FnMaybeAppendPaperContext = fn(&Path, &str, &mut Vec<String>, PaperProseHookHost);
type FnMaybeMergePaperBeforeSubmit = fn(&Path, &mut Value, &str, bool);

static FN_APPEND_PROSE: RwLock<Option<FnMaybeAppendPaperContext>> = RwLock::new(None);
static FN_MERGE_PROSE: RwLock<Option<FnMaybeMergePaperBeforeSubmit>> = RwLock::new(None);
static FN_APPEND_ADVERSARIAL: RwLock<Option<FnMaybeAppendPaperContext>> = RwLock::new(None);
static FN_MERGE_ADVERSARIAL: RwLock<Option<FnMaybeMergePaperBeforeSubmit>> = RwLock::new(None);

pub fn register_paper_hooks(
    append_prose: FnMaybeAppendPaperContext,
    merge_prose: FnMaybeMergePaperBeforeSubmit,
    append_adversarial: FnMaybeAppendPaperContext,
    merge_adversarial: FnMaybeMergePaperBeforeSubmit,
) {
    *FN_APPEND_PROSE.write().unwrap() = Some(append_prose);
    *FN_MERGE_PROSE.write().unwrap() = Some(merge_prose);
    *FN_APPEND_ADVERSARIAL.write().unwrap() = Some(append_adversarial);
    *FN_MERGE_ADVERSARIAL.write().unwrap() = Some(merge_adversarial);
}

pub fn maybe_append_paper_prose_context(
    repo_root: &Path,
    prompt_text: &str,
    contexts: &mut Vec<String>,
    host: PaperProseHookHost,
) {
    if let Some(f) = *FN_APPEND_PROSE.read().unwrap() {
        f(repo_root, prompt_text, contexts, host);
    }
}

pub fn maybe_merge_paper_prose_before_submit(
    repo_root: &Path,
    output: &mut Value,
    prompt_text: &str,
    use_followup_message: bool,
) {
    if let Some(f) = *FN_MERGE_PROSE.read().unwrap() {
        f(repo_root, output, prompt_text, use_followup_message);
    }
}

pub fn maybe_append_paper_adversarial_context(
    repo_root: &Path,
    prompt_text: &str,
    contexts: &mut Vec<String>,
    host: PaperProseHookHost,
) {
    if let Some(f) = *FN_APPEND_ADVERSARIAL.read().unwrap() {
        f(repo_root, prompt_text, contexts, host);
    }
}

pub fn maybe_merge_paper_adversarial_before_submit(
    repo_root: &Path,
    output: &mut Value,
    prompt_text: &str,
    use_followup_message: bool,
) {
    if let Some(f) = *FN_MERGE_ADVERSARIAL.read().unwrap() {
        f(repo_root, output, prompt_text, use_followup_message);
    }
}

// ────────────────────────────────────────────────────────────────
// kernel_bootstrap: function-pointer proxy
// ────────────────────────────────────────────────────────────────

type FnEnsureKernelBootstrap = fn();

static FN_ENSURE_KERNEL: RwLock<Option<FnEnsureKernelBootstrap>> = RwLock::new(None);

pub fn register_kernel_bootstrap(f: FnEnsureKernelBootstrap) {
    *FN_ENSURE_KERNEL.write().unwrap() = Some(f);
}

pub fn ensure_kernel_bootstrap() {
    if let Some(f) = *FN_ENSURE_KERNEL.read().unwrap() {
        f();
    }
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
        .unwrap_or_else(|poisoned| poisoned.into_inner())
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

        // Register test-only framework runtime hooks so cursor/codex/claude hooks tests
        // can exercise closeout enforcement, record path resolution, etc. without
        // depending on the real runtime-core registration.
        fn test_closeout_enforcement_enabled() -> bool {
            std::env::var("ROUTER_RS_CLOSEOUT_ENFORCEMENT")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false)
        }
        fn test_closeout_record_path(
            repo_root: &std::path::Path,
            task_id: &str,
        ) -> Result<std::path::PathBuf, String> {
            Ok(repo_root.join("artifacts/closeout").join(format!("{task_id}.json")))
        }
        fn test_evaluate_closeout_record(
            _repo_root: &std::path::Path,
            _task_id: &str,
            record_path: &std::path::Path,
        ) -> Result<Value, String> {
            let data = std::fs::read_to_string(record_path)
                .map_err(|e| format!("read closeout record: {e}"))?;
            let val: Value =
                serde_json::from_str(&data).map_err(|e| format!("parse closeout record: {e}"))?;
            // Simplified evaluation: allow if record has valid schema_version,
            // verification_status == "passed", and commands_run is non-empty.
            let schema_ok = val
                .get("schema_version")
                .and_then(Value::as_str)
                .map(|s| s == "closeout-record-v1")
                .unwrap_or(false);
            let verification_passed = val
                .get("verification_status")
                .and_then(Value::as_str)
                .map(|s| s == "passed")
                .unwrap_or(false);
            let has_commands = val
                .get("commands_run")
                .and_then(Value::as_array)
                .map(|a| !a.is_empty())
                .unwrap_or(false);
            let allowed = schema_ok && verification_passed && has_commands;
            Ok(serde_json::json!({
                "schema_version": "closeout-enforcement-response-v1",
                "authority": "closeout-enforcement",
                "task_id": val.get("task_id").and_then(Value::as_str).unwrap_or(""),
                "closeout_allowed": allowed,
                "claimed_completion": true,
                "violations": [],
                "missing_evidence": [],
                "verification_status": val.get("verification_status").and_then(Value::as_str).unwrap_or(""),
            }))
        }
        fn test_first_task_id_from_registry(repo_root: &std::path::Path) -> Option<String> {
            let reg_path = repo_root.join("artifacts/current/task_registry.json");
            let data = std::fs::read_to_string(reg_path).ok()?;
            let reg: Value = serde_json::from_str(&data).ok()?;
            reg.get("focus_task_id")
                .and_then(Value::as_str)
                .map(str::to_string)
        }
        fn test_build_contract(_repo_root: &std::path::Path) -> Result<Value, String> {
            Ok(serde_json::json!({}))
        }
        fn test_append_shell(
            _repo_root: &std::path::Path,
            _event: &Value,
            _kind: &str,
        ) -> Result<(), String> {
            Ok(())
        }
        fn test_evidence_append(payload: Value) -> Result<Value, String> {
            Ok(payload)
        }
        fn test_extract_duration(_event: &Value) -> Option<u64> {
            None
        }
        fn test_post_tool_ok(_event: &Value) -> bool {
            true
        }
        fn test_closeout_followup(
            repo_root: &std::path::Path,
            text: &str,
        ) -> Option<String> {
            if text.trim().is_empty() || !core_policy::hook_common::contains_completion_claim_token(text) {
                return None;
            }
            if !test_closeout_enforcement_enabled() {
                return None;
            }
            // Resolve task ID from task_registry.json.
            let tid = test_first_task_id_from_registry(repo_root)?;
            let record_path = test_closeout_record_path(repo_root, &tid).ok()?;
            if !record_path.is_file() {
                return Some(format!(
                    "CLOSEOUT_FOLLOWUP task_id={tid} reason=missing_record path={}\n\
请在完成态宣称前写入 closeout record 并通过评估。",
                    record_path.display()
                ));
            }
            let eval = test_evaluate_closeout_record(repo_root, &tid, &record_path).ok()?;
            if eval
                .get("closeout_allowed")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
            {
                return None;
            }
            Some(format!(
                "CLOSEOUT_FOLLOWUP task_id={tid} reason=evaluation_failed path={}",
                record_path.display()
            ))
        }
        register_framework_runtime(
            test_build_contract,
            test_append_shell,
            test_closeout_enforcement_enabled,
            test_closeout_record_path,
            test_evaluate_closeout_record,
            test_first_task_id_from_registry,
            test_evidence_append,
            test_extract_duration,
            test_post_tool_ok,
            test_closeout_followup,
        );

        // Register outbound truncation / protection hooks for tests.
        fn test_is_protected_line(line: &str) -> bool {
            let t = line.trim_start();
            t.contains("router-rs REVIEW_GATE")
                || t.starts_with("router-rs REVIEW_GATE detail")
                || t.contains("continuity_suppressed=")
                || t.contains("PAPER_PROSE_QUALITY_HOOK")
                || t.contains("PAPER_ADVERSARIAL_HOOK")
        }
        fn test_truncate_outbound(
            combined: &str,
            max_bytes: usize,
            suffix: &str,
        ) -> String {
            if combined.len() <= max_bytes {
                return combined.to_string();
            }
            let budget = max_bytes.saturating_sub(suffix.len());
            // Truncate at line boundary when possible.
            let mut end = budget.min(combined.len());
            // Find the last newline before the budget to avoid splitting lines.
            if let Some(nl) = combined[..end].rfind('\n') {
                end = nl;
            }
            // Ensure we don't land in the middle of a multi-byte UTF-8 char.
            while end > 0 && !combined.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}{}", &combined[..end], suffix)
        }
        register_hook_outbound_protect(test_is_protected_line, test_truncate_outbound);

        // Register ship readiness hooks (simplified for tests).
        fn test_evaluate_goal_readiness(
            repo_root: &std::path::Path,
            goal: &Value,
            task_id: &str,
        ) -> GoalReadiness {
            // Simplified evaluation: check goal text, non_goals, validation_commands,
            // done_when for contract; checkpoints/evidence for progress/verification.
            let has_goal_text = goal
                .get("goal")
                .and_then(Value::as_str)
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
            let has_non_goals = goal
                .get("non_goals")
                .and_then(Value::as_array)
                .map(|a| a.iter().any(|v| v.as_str().map(|s| !s.trim().is_empty()).unwrap_or(false)))
                .unwrap_or(false);
            let has_validation = goal
                .get("validation_commands")
                .and_then(Value::as_array)
                .map(|a| a.iter().any(|v| v.as_str().map(|s| !s.trim().is_empty()).unwrap_or(false)))
                .unwrap_or(false);
            let done_when_count = goal
                .get("done_when")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter(|v| v.as_str().map(|s| !s.trim().is_empty()).unwrap_or(false)).count())
                .unwrap_or(0);
            let contract = has_goal_text && has_non_goals && has_validation && done_when_count >= 2;

            let has_checkpoints = goal
                .get("checkpoints")
                .and_then(Value::as_array)
                .map(|a| !a.is_empty())
                .unwrap_or(false);
            // Check for evidence index on disk.
            let evidence_path = repo_root
                .join("artifacts/current")
                .join(task_id)
                .join("EVIDENCE_INDEX.json");
            let has_evidence = evidence_path.is_file();

            let status = goal
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("");
            let progress = has_checkpoints || has_evidence || status == "completed";
            // Verification is satisfied when evidence exists, status is completed,
            // or checkpoints exist (running goals with checkpoint history are considered verified).
            let verification = has_evidence || status == "completed"
                || (has_checkpoints && status == "running");

            GoalReadiness { contract, progress, verification }
        }
        fn test_goal_stop_followup(
            contract: bool,
            progress: bool,
            verification: bool,
            goal_followup_count: u32,
        ) -> String {
            let mut missing = Vec::new();
            if !contract { missing.push("goal_contract"); }
            if !progress { missing.push("checkpoint_progress"); }
            if !verification { missing.push("verification_or_blocker"); }
            let joined = missing.join(",");
            let mut line = format!("router-rs AG_FOLLOWUP missing_parts={joined}");
            if !contract {
                line.push_str(" primary_fix=goal_contract");
            } else if !progress {
                line.push_str(" primary_fix=checkpoint_progress");
            } else if !verification {
                line.push_str(" primary_fix=verification_or_blocker");
            }
            if goal_followup_count >= 3 {
                line.push_str(" | 已连续多轮 Stop 未满足门控；若确为小任务请直接单独一行 small_task");
            }
            line
        }
        register_ship_readiness(test_evaluate_goal_readiness, test_goal_stop_followup);

        // Register paper hooks with actual content injection for tests.
        // The builtin PAPER_PROSE_QUALITY_HOOK text is included at compile time.
        const PAPER_PROSE_BUILTIN: &str =
            include_str!("../../../configs/framework/PAPER_PROSE_QUALITY_HOOK.txt");

        fn prompt_signals_prose_work(text: &str) -> bool {
            // Simplified keyword detection for prose/edit signals.
            // Avoid false positives on programming terms like "abstract class".
            let lower = text.to_lowercase();
            let keywords = [
                "润色", "改稿", "论文", "latex", "manuscript",
                "proofread", "polish", "rewrite", "段落", "不通顺", "改这段",
                "sci", "prose", "写作", "初稿", "终稿", "提纲",
            ];
            if keywords.iter().any(|kw| lower.contains(kw)) {
                return true;
            }
            // "abstract" only triggers in prose context (with SCI/paper keywords).
            // "edit" alone is too generic (code edits).
            false
        }

        fn test_append_prose_context(
            _repo_root: &std::path::Path,
            prompt_text: &str,
            contexts: &mut Vec<String>,
            host: PaperProseHookHost,
        ) {
            if !super::hooks::router_rs_operator_inject_globally_enabled() {
                return;
            }
            let env_var = match host {
                PaperProseHookHost::Cursor => "ROUTER_RS_CURSOR_PAPER_PROSE_HOOK",
                PaperProseHookHost::Codex => "ROUTER_RS_CODEX_PAPER_PROSE_HOOK",
                PaperProseHookHost::Claude => "ROUTER_RS_CLAUDE_PAPER_PROSE_HOOK",
            };
            if !super::hooks::router_rs_env_enabled_default_true(env_var) {
                return;
            }
            if !prompt_signals_prose_work(prompt_text) {
                return;
            }
            let block = PAPER_PROSE_BUILTIN.trim().to_string();
            if !block.is_empty() {
                contexts.push(block);
            }
        }

        fn test_merge_prose_before_submit(
            _repo_root: &std::path::Path,
            output: &mut Value,
            prompt_text: &str,
            use_followup_message: bool,
        ) {
            if !super::hooks::router_rs_operator_inject_globally_enabled() {
                return;
            }
            if !super::hooks::router_rs_env_enabled_default_true("ROUTER_RS_CURSOR_PAPER_PROSE_HOOK") {
                return;
            }
            if !prompt_signals_prose_work(prompt_text) {
                return;
            }
            let block = PAPER_PROSE_BUILTIN.trim().to_string();
            if block.is_empty() {
                return;
            }
            let key = if use_followup_message {
                "followup_message"
            } else {
                "additional_context"
            };
            let existing = output
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if existing.contains("PAPER_PROSE_QUALITY_HOOK") {
                return;
            }
            let merged = if existing.is_empty() {
                block
            } else {
                format!("{existing}\n\n{block}")
            };
            output[key] = Value::String(merged);
        }

        fn test_append_adversarial_context(
            _repo_root: &std::path::Path,
            _prompt_text: &str,
            _contexts: &mut Vec<String>,
            _host: PaperProseHookHost,
        ) {
            // No-op for tests.
        }

        fn test_merge_adversarial_before_submit(
            _repo_root: &std::path::Path,
            _output: &mut Value,
            _prompt_text: &str,
            _use_followup_message: bool,
        ) {
            // No-op for tests.
        }

        register_paper_hooks(
            test_append_prose_context,
            test_merge_prose_before_submit,
            test_append_adversarial_context,
            test_merge_adversarial_before_submit,
        );

        // Register session call tracker hooks for tests.
        fn test_init_tracker(repo_root: &std::path::Path) -> Result<(), String> {
            let dir = repo_root.join("artifacts/current");
            std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {e}"))?;
            let path = dir.join("session_call_tracker.json");
            let state = serde_json::json!({
                "schema_version": "session-call-tracker-v1",
                "total_calls": 0,
                "per_tool": {},
            });
            std::fs::write(&path, serde_json::to_string_pretty(&state).unwrap())
                .map_err(|e| format!("write tracker: {e}"))?;
            Ok(())
        }
        fn test_record_tool_call(
            repo_root: &std::path::Path,
            tool_name: &str,
            _cache_stats: Option<&Value>,
        ) -> Result<(), String> {
            let path = repo_root.join("artifacts/current/session_call_tracker.json");
            let data = std::fs::read_to_string(&path).map_err(|e| format!("read tracker: {e}"))?;
            let mut state: Value =
                serde_json::from_str(&data).map_err(|e| format!("parse tracker: {e}"))?;
            let total = state["total_calls"].as_u64().unwrap_or(0) + 1;
            state["total_calls"] = serde_json::json!(total);
            let tool_key = tool_name.to_string();
            let per_tool = state["per_tool"].as_object_mut().unwrap();
            let count = per_tool.get(&tool_key).and_then(Value::as_u64).unwrap_or(0) + 1;
            per_tool.insert(tool_key, serde_json::json!(count));
            std::fs::write(&path, serde_json::to_string_pretty(&state).unwrap())
                .map_err(|e| format!("write tracker: {e}"))?;
            Ok(())
        }
        fn test_read_tracker_state(repo_root: &std::path::Path) -> Result<Value, String> {
            let path = repo_root.join("artifacts/current/session_call_tracker.json");
            let data = std::fs::read_to_string(&path).map_err(|e| format!("read tracker: {e}"))?;
            serde_json::from_str(&data).map_err(|e| format!("parse tracker: {e}"))
        }
        register_session_call_tracker(test_init_tracker, test_record_tool_call, test_read_tracker_state);

        // Register router_rs_observation hooks for tests.
        fn test_attach_observation(output: &mut Value, host: HookObservationHost) {
            let host_str = match host {
                HookObservationHost::Cursor => "cursor",
                HookObservationHost::Codex => "codex",
                HookObservationHost::ClaudeCode => "claude-code",
            };
            // Detect review gate advisory in followup_message.
            let followup = output
                .get("followup_message")
                .and_then(Value::as_str)
                .unwrap_or("");
            let decision = output.get("decision").and_then(Value::as_str);
            let gate = if followup.contains("REVIEW_GATE") {
                Some(serde_json::json!({
                    "code": "review_gate",
                    "blocking": true,
                    "human_prefix": "review_gate",
                }))
            } else if decision == Some("block") {
                Some(serde_json::json!({
                    "code": "block",
                    "blocking": true,
                    "human_prefix": "block",
                }))
            } else {
                None
            };
            let mut obs = serde_json::json!({
                "host": host_str,
            });
            if let Some(g) = gate {
                obs["gate"] = g;
            }
            if let Some(obj) = output.as_object_mut() {
                obj.insert("router_rs_observation".to_string(), obs);
            }
        }
        fn test_strip_observation(output: &mut Value) {
            if let Some(obj) = output.as_object_mut() {
                obj.remove("router_rs_observation");
            }
        }
        register_router_rs_observation(test_attach_observation, test_strip_observation);
    });
}

// ────────────────────────────────────────────────────────────────
// Additional hooks needed by claude_code_hooks / mcp_stdio_harness
// (appended during merge of Agent 1 + Agent 2 hooks)
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
