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

/// Mirror of `runtime_core::paper_prose_hook::PaperProseHookHost`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaperProseHookHost {
    Cursor,
    Codex,
    Claude,
}

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
