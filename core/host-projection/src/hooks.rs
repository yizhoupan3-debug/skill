//! Hook registry for host-projection dependency injection.
//!
//! host-projection depends on core-state, core-policy, and framework-kernel directly.
//! It cannot depend on runtime-core (circular dependency). The functions in this module
//! allow runtime-core to register callbacks that host-projection's migrated modules need
//! at runtime (framework_runtime, telemetry, hook_timing, session_call_tracker, etc.).
//!
//! Pattern: mirrors routing-engine's `hooks.rs` — `OnceLock<HostProjectionHooks>` +
//! `register_hooks()` + per-hook accessor functions with safe defaults.
//!
//! All hooks are **optional** — unregistered hooks return safe defaults.
//! The host crate (runtime-core) should call `register_hooks` once at startup.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

// ══════════════════════════════════════════════════════════════════════════════
// Mirror types (avoid pulling runtime-core's heavy deps into host-projection)
// ══════════════════════════════════════════════════════════════════════════════

/// Mirrors `paper_prose_hook::PaperProseHookHost`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaperProseHookHostType {
    Claude,
    Cursor,
    Codex,
}

/// Mirrors `router_rs_observation::HookObservationHost`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookObservationHostType {
    ClaudeCode,
    Cursor,
    Codex,
}

/// Mirrors `mcp_pre_guard::McpPreGuardVerdict`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpPreGuardVerdictType {
    pub blocked: bool,
    pub reason: Option<String>,
}

/// Mirrors `session_call_tracker::CacheStats`.
#[derive(Debug, Clone, Default)]
pub struct SessionCallCacheStats {
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Goal readiness verdict for closeout gate evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalReadinessType {
    Ready,
    NotReady,
    Blocked,
}

// ══════════════════════════════════════════════════════════════════════════════
// Function-pointer types
// ══════════════════════════════════════════════════════════════════════════════

// ── Framework Runtime hooks ──
type AppendEvidenceIndexFn =
    fn(&Path, Option<&str>, serde_json::Map<String, Value>) -> Result<(), String>;
type CloseoutProgrammaticEnforcementEnabledFn = fn() -> bool;
type CloseoutRecordPathForTaskFn = fn(&Path, &str) -> Result<PathBuf, String>;
type CloseoutStopFollowupForCompletionTextFn = fn(&Path, &str) -> Option<String>;
type CloseoutRecordSchemaVersionFn = fn() -> &'static str;
type CurrentLocalTimestampFn = fn() -> String;
type EvaluateCloseoutRecordFileForTaskFn = fn(&Path, &str, &Path) -> Result<Value, String>;
type ExtractPostToolDurationMsFn = fn(&Value) -> Option<u64>;
type FirstTaskIdFromRegistryFn = fn(&Path) -> Option<String>;
type FrameworkHookEvidenceAppendFn = fn(Value) -> Result<Value, String>;
type PostToolCallSucceededFn = fn(&Value) -> bool;
type ResolveRepoRootArgFn = fn(Option<&Path>) -> Result<PathBuf, String>;
type WriteFrameworkSessionArtifactsFn = fn(Value) -> Result<Value, String>;
type BuildFrameworkRuntimeSnapshotEnvelopeFn =
    fn(&Path, Option<&Path>, Option<&str>) -> Result<Value, String>;
type BuildAutomaticContinuityCheckpointPayloadFn =
    fn(&Path, &str, &str, Option<&str>, bool, bool) -> Value;

// ── Routing ──
type RouteTaskWithManifestFallbackFn = fn(
    &[routing_engine::types::SkillRecord],
    Option<&Path>,
    Option<&Path>,
    Option<&str>,
    &str,
    &str,
    bool,
    bool,
) -> Result<routing_engine::types::RouteDecision, String>;

// ── Hook timing ──
type MarkHookStartFn = fn();
type AddLockWaitMsFn = fn(u64);
type AddCargoCheckMsFn = fn(u64);
type EmitHookTimingLineFn = fn(&str);

// ── Telemetry ──
type EmitHookFiredFn = fn(&str, &str);
type EmitToolCallFn = fn(&str, u64, bool);
type HookActionFromOptionalOutputFn = fn(Option<&Value>) -> &'static str;
type HookActionFromOutputFn = fn(&Value) -> &'static str;

// ── Session call tracker ──
type InitTrackerFn = fn(&Path) -> Result<(), String>;
type ReadTrackerStateFn = fn(&Path) -> Result<Value, String>;
type RecordToolCallFn = fn(&Path, &str, Option<SessionCallCacheStats>) -> Result<(), String>;
type CheckAnomaliesFn = fn(&Path) -> Result<Vec<String>, String>;

// ── Router env flags (subset used by migrated files) ──
type RouterEnvBoolFn = fn() -> bool;
type RouterEnvUsizeFn = fn() -> usize;
type RouterEnvU32Fn = fn() -> u32;
type RouterEnvU64Fn = fn() -> u64;
type RouterEnvOptionU32Fn = fn() -> Option<u32>;

// ── Paper hooks ──
type MaybeAppendPaperProseContextFn =
    fn(&Path, &str, &mut Vec<String>, PaperProseHookHostType);
type MaybeAppendPaperAdversarialContextFn =
    fn(&Path, &str, &mut Vec<String>, PaperProseHookHostType);

// ── Observation ──
type AttachRouterRsObservationFn = fn(&mut Value, HookObservationHostType);

// ── Kernel ──
type EnsureKernelBootstrapFn = fn();

// ── MCP pre-guard ──
type EvaluateMcpPreGuardSafeFn = fn(&str, &Value, &Path) -> McpPreGuardVerdictType;

// ── Web fetch guard ──
type WebFetchValidateFn = fn(&str) -> Result<(String, Vec<String>), String>;
type WebFetchRedirectFn = fn(&str, &str) -> Result<String, String>;
type WebFetchAddressesFn = fn(&str, u16) -> Result<Vec<String>, String>;

// ── RFV loop ──
type FrameworkRfvLoopFn = fn(Value) -> Result<Value, String>;

// ── Autopilot goal (re-exports from core-state, but hook for uniformity) ──
type FrameworkGoalDriveFn = fn(Value) -> Result<Value, String>;
type ReadGoalStateFn = fn(&Path) -> Result<Value, String>;

// ══════════════════════════════════════════════════════════════════════════════
// HostProjectionHooks struct
// ══════════════════════════════════════════════════════════════════════════════

pub struct HostProjectionHooks {
    // ── Framework Runtime ──
    append_evidence_index: AppendEvidenceIndexFn,
    closeout_programmatic_enforcement_enabled: CloseoutProgrammaticEnforcementEnabledFn,
    closeout_record_path_for_task: CloseoutRecordPathForTaskFn,
    closeout_stop_followup_for_completion_text: CloseoutStopFollowupForCompletionTextFn,
    closeout_record_schema_version: CloseoutRecordSchemaVersionFn,
    current_local_timestamp: CurrentLocalTimestampFn,
    evaluate_closeout_record_file_for_task: EvaluateCloseoutRecordFileForTaskFn,
    extract_post_tool_duration_ms: ExtractPostToolDurationMsFn,
    first_task_id_from_registry: FirstTaskIdFromRegistryFn,
    framework_hook_evidence_append: FrameworkHookEvidenceAppendFn,
    post_tool_call_succeeded: PostToolCallSucceededFn,
    resolve_repo_root_arg: ResolveRepoRootArgFn,
    write_framework_session_artifacts: WriteFrameworkSessionArtifactsFn,
    build_framework_runtime_snapshot_envelope: BuildFrameworkRuntimeSnapshotEnvelopeFn,
    build_automatic_continuity_checkpoint_payload: BuildAutomaticContinuityCheckpointPayloadFn,
    route_task_with_manifest_fallback: RouteTaskWithManifestFallbackFn,

    // ── Hook timing ──
    mark_hook_start: MarkHookStartFn,
    add_lock_wait_ms: AddLockWaitMsFn,
    add_cargo_check_ms: AddCargoCheckMsFn,
    emit_hook_timing_line: EmitHookTimingLineFn,

    // ── Telemetry ──
    emit_hook_fired: EmitHookFiredFn,
    emit_tool_call: EmitToolCallFn,
    hook_action_from_optional_output: HookActionFromOptionalOutputFn,
    hook_action_from_output: HookActionFromOutputFn,

    // ── Session call tracker ──
    init_tracker: InitTrackerFn,
    read_tracker_state: ReadTrackerStateFn,
    record_tool_call: RecordToolCallFn,
    check_anomalies: CheckAnomaliesFn,

    // ── Router env flags ──
    router_rs_skip_pre_tool_use_guard: RouterEnvBoolFn,
    router_rs_task_ledger_flock_enabled: RouterEnvBoolFn,
    router_rs_hook_timing_enabled: RouterEnvBoolFn,
    router_rs_continuity_post_tool_evidence_enabled: RouterEnvBoolFn,
    router_rs_cursor_hook_outbound_context_max_bytes: RouterEnvUsizeFn,
    router_rs_session_call_tracker_tool_keys_max: RouterEnvUsizeFn,
    router_rs_rfv_max_rounds_cap: RouterEnvU64Fn,
    router_rs_cursor_hook_state_lock_retries: RouterEnvU32Fn,
    router_rs_cursor_hook_state_stale_sweep_days: RouterEnvU64Fn,
    router_rs_cursor_review_gate_stop_max_nudges_cap: RouterEnvOptionU32Fn,
    router_rs_operator_inject_globally_enabled: RouterEnvBoolFn,
    router_rs_cursor_hook_silent_enabled: RouterEnvBoolFn,
    router_rs_cursor_sessionstart_context_max_bytes: RouterEnvUsizeFn,

    // ── Paper hooks ──
    maybe_append_paper_prose_context: MaybeAppendPaperProseContextFn,
    maybe_append_paper_adversarial_context: MaybeAppendPaperAdversarialContextFn,

    // ── Observation ──
    attach_router_rs_observation: AttachRouterRsObservationFn,

    // ── Kernel ──
    ensure_kernel_bootstrap: EnsureKernelBootstrapFn,

    // ── MCP pre-guard ──
    evaluate_mcp_pre_guard_safe: EvaluateMcpPreGuardSafeFn,

    // ── Web fetch guard ──
    validate_and_resolve_web_fetch_url: WebFetchValidateFn,
    resolve_web_fetch_redirect: WebFetchRedirectFn,
    resolve_web_fetch_addresses: WebFetchAddressesFn,

    // ── RFV loop ──
    framework_rfv_loop: FrameworkRfvLoopFn,

    // ── Autopilot goal ──
    framework_goal_drive: FrameworkGoalDriveFn,
    read_goal_state: ReadGoalStateFn,
}

// ══════════════════════════════════════════════════════════════════════════════
// Global state + registration
// ══════════════════════════════════════════════════════════════════════════════

static HOOKS: OnceLock<HostProjectionHooks> = OnceLock::new();

/// Register all host-projection hooks. Should be called once from runtime-core at startup.
/// Returns `Err` if hooks were already registered.
#[allow(clippy::too_many_arguments)]
pub fn register_hooks(hooks: HostProjectionHooks) -> Result<(), &'static str> {
    HOOKS
        .set(hooks)
        .map_err(|_| "host-projection hooks already registered")
}

/// Convenience: build a `HostProjectionHooks` with all required function pointers.
/// Use this from runtime-core's initialization code.
pub fn build_hooks(
    // Framework Runtime
    append_evidence_index: AppendEvidenceIndexFn,
    closeout_programmatic_enforcement_enabled: CloseoutProgrammaticEnforcementEnabledFn,
    closeout_record_path_for_task: CloseoutRecordPathForTaskFn,
    closeout_stop_followup_for_completion_text: CloseoutStopFollowupForCompletionTextFn,
    closeout_record_schema_version: CloseoutRecordSchemaVersionFn,
    current_local_timestamp: CurrentLocalTimestampFn,
    evaluate_closeout_record_file_for_task: EvaluateCloseoutRecordFileForTaskFn,
    extract_post_tool_duration_ms: ExtractPostToolDurationMsFn,
    first_task_id_from_registry: FirstTaskIdFromRegistryFn,
    framework_hook_evidence_append: FrameworkHookEvidenceAppendFn,
    post_tool_call_succeeded: PostToolCallSucceededFn,
    resolve_repo_root_arg: ResolveRepoRootArgFn,
    write_framework_session_artifacts: WriteFrameworkSessionArtifactsFn,
    build_framework_runtime_snapshot_envelope: BuildFrameworkRuntimeSnapshotEnvelopeFn,
    build_automatic_continuity_checkpoint_payload: BuildAutomaticContinuityCheckpointPayloadFn,
    route_task_with_manifest_fallback: RouteTaskWithManifestFallbackFn,
    // Hook timing
    mark_hook_start: MarkHookStartFn,
    add_lock_wait_ms: AddLockWaitMsFn,
    add_cargo_check_ms: AddCargoCheckMsFn,
    emit_hook_timing_line: EmitHookTimingLineFn,
    // Telemetry
    emit_hook_fired: EmitHookFiredFn,
    emit_tool_call: EmitToolCallFn,
    hook_action_from_optional_output: HookActionFromOptionalOutputFn,
    hook_action_from_output: HookActionFromOutputFn,
    // Session call tracker
    init_tracker: InitTrackerFn,
    read_tracker_state: ReadTrackerStateFn,
    record_tool_call: RecordToolCallFn,
    check_anomalies: CheckAnomaliesFn,
    // Router env flags
    router_rs_skip_pre_tool_use_guard: RouterEnvBoolFn,
    router_rs_task_ledger_flock_enabled: RouterEnvBoolFn,
    router_rs_hook_timing_enabled: RouterEnvBoolFn,
    router_rs_continuity_post_tool_evidence_enabled: RouterEnvBoolFn,
    router_rs_cursor_hook_outbound_context_max_bytes: RouterEnvUsizeFn,
    router_rs_session_call_tracker_tool_keys_max: RouterEnvUsizeFn,
    router_rs_rfv_max_rounds_cap: RouterEnvU64Fn,
    router_rs_cursor_hook_state_lock_retries: RouterEnvU32Fn,
    router_rs_cursor_hook_state_stale_sweep_days: RouterEnvU64Fn,
    router_rs_cursor_review_gate_stop_max_nudges_cap: RouterEnvOptionU32Fn,
    router_rs_operator_inject_globally_enabled: RouterEnvBoolFn,
    router_rs_cursor_hook_silent_enabled: RouterEnvBoolFn,
    router_rs_cursor_sessionstart_context_max_bytes: RouterEnvUsizeFn,
    // Paper hooks
    maybe_append_paper_prose_context: MaybeAppendPaperProseContextFn,
    maybe_append_paper_adversarial_context: MaybeAppendPaperAdversarialContextFn,
    // Observation
    attach_router_rs_observation: AttachRouterRsObservationFn,
    // Kernel
    ensure_kernel_bootstrap: EnsureKernelBootstrapFn,
    // MCP pre-guard
    evaluate_mcp_pre_guard_safe: EvaluateMcpPreGuardSafeFn,
    // Web fetch guard
    validate_and_resolve_web_fetch_url: WebFetchValidateFn,
    resolve_web_fetch_redirect: WebFetchRedirectFn,
    resolve_web_fetch_addresses: WebFetchAddressesFn,
    // RFV loop
    framework_rfv_loop: FrameworkRfvLoopFn,
    // Autopilot goal
    framework_goal_drive: FrameworkGoalDriveFn,
    read_goal_state: ReadGoalStateFn,
) -> HostProjectionHooks {
    HostProjectionHooks {
        append_evidence_index,
        closeout_programmatic_enforcement_enabled,
        closeout_record_path_for_task,
        closeout_stop_followup_for_completion_text,
        closeout_record_schema_version,
        current_local_timestamp,
        evaluate_closeout_record_file_for_task,
        extract_post_tool_duration_ms,
        first_task_id_from_registry,
        framework_hook_evidence_append,
        post_tool_call_succeeded,
        resolve_repo_root_arg,
        write_framework_session_artifacts,
        build_framework_runtime_snapshot_envelope,
        build_automatic_continuity_checkpoint_payload,
        route_task_with_manifest_fallback,
        mark_hook_start,
        add_lock_wait_ms,
        add_cargo_check_ms,
        emit_hook_timing_line,
        emit_hook_fired,
        emit_tool_call,
        hook_action_from_optional_output,
        hook_action_from_output,
        init_tracker,
        read_tracker_state,
        record_tool_call,
        check_anomalies,
        router_rs_skip_pre_tool_use_guard,
        router_rs_task_ledger_flock_enabled,
        router_rs_hook_timing_enabled,
        router_rs_continuity_post_tool_evidence_enabled,
        router_rs_cursor_hook_outbound_context_max_bytes,
        router_rs_session_call_tracker_tool_keys_max,
        router_rs_rfv_max_rounds_cap,
        router_rs_cursor_hook_state_lock_retries,
        router_rs_cursor_hook_state_stale_sweep_days,
        router_rs_cursor_review_gate_stop_max_nudges_cap,
        router_rs_operator_inject_globally_enabled,
        router_rs_cursor_hook_silent_enabled,
        router_rs_cursor_sessionstart_context_max_bytes,
        maybe_append_paper_prose_context,
        maybe_append_paper_adversarial_context,
        attach_router_rs_observation,
        ensure_kernel_bootstrap,
        evaluate_mcp_pre_guard_safe,
        validate_and_resolve_web_fetch_url,
        resolve_web_fetch_redirect,
        resolve_web_fetch_addresses,
        framework_rfv_loop,
        framework_goal_drive,
        read_goal_state,
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Public accessors (safe defaults when hooks are not registered)
// ══════════════════════════════════════════════════════════════════════════════

// ── Framework Runtime ──

pub fn append_evidence_index(
    repo_root: &Path,
    task_id: Option<&str>,
    entry: serde_json::Map<String, Value>,
) -> Result<(), String> {
    HOOKS
        .get()
        .map(|h| (h.append_evidence_index)(repo_root, task_id, entry))
        .unwrap_or(Ok(()))
}

pub fn closeout_programmatic_enforcement_enabled() -> bool {
    HOOKS
        .get()
        .map(|h| (h.closeout_programmatic_enforcement_enabled)())
        .unwrap_or(false)
}

pub fn closeout_record_path_for_task(repo_root: &Path, task_id: &str) -> Result<PathBuf, String> {
    HOOKS
        .get()
        .map(|h| (h.closeout_record_path_for_task)(repo_root, task_id))
        .unwrap_or_else(|| Err("hooks not registered".into()))
}

pub fn closeout_stop_followup_for_completion_text(
    repo_root: &Path,
    completion_text: &str,
) -> Option<String> {
    HOOKS
        .get()
        .and_then(|h| (h.closeout_stop_followup_for_completion_text)(repo_root, completion_text))
}

pub fn closeout_record_schema_version() -> &'static str {
    HOOKS
        .get()
        .map(|h| (h.closeout_record_schema_version)())
        .unwrap_or("closeout-record-v1")
}

pub fn current_local_timestamp() -> String {
    HOOKS
        .get()
        .map(|h| (h.current_local_timestamp)())
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".into())
}

pub fn evaluate_closeout_record_file_for_task(
    repo_root: &Path,
    task_id: &str,
    record_path: &Path,
) -> Result<Value, String> {
    HOOKS
        .get()
        .map(|h| (h.evaluate_closeout_record_file_for_task)(repo_root, task_id, record_path))
        .unwrap_or_else(|| Err("hooks not registered".into()))
}

pub fn extract_post_tool_duration_ms(payload: &Value) -> Option<u64> {
    HOOKS
        .get()
        .and_then(|h| (h.extract_post_tool_duration_ms)(payload))
}

pub fn first_task_id_from_registry(repo_root: &Path) -> Option<String> {
    HOOKS
        .get()
        .and_then(|h| (h.first_task_id_from_registry)(repo_root))
}

pub fn framework_hook_evidence_append(payload: Value) -> Result<Value, String> {
    HOOKS
        .get()
        .map(|h| (h.framework_hook_evidence_append)(payload.clone()))
        .unwrap_or(Ok(payload))
}

pub fn post_tool_call_succeeded(payload: &Value) -> bool {
    HOOKS
        .get()
        .map(|h| (h.post_tool_call_succeeded)(payload))
        .unwrap_or(false)
}

pub fn resolve_repo_root_arg(cli_repo_root: Option<&Path>) -> Result<PathBuf, String> {
    HOOKS
        .get()
        .map(|h| (h.resolve_repo_root_arg)(cli_repo_root))
        .unwrap_or_else(|| Err("hooks not registered".into()))
}

pub fn write_framework_session_artifacts(payload: Value) -> Result<Value, String> {
    HOOKS
        .get()
        .map(|h| (h.write_framework_session_artifacts)(payload))
        .unwrap_or_else(|| Err("hooks not registered".into()))
}

pub fn build_framework_runtime_snapshot_envelope(
    repo_root: &Path,
    artifact_root_override: Option<&Path>,
    task_id_override: Option<&str>,
) -> Result<Value, String> {
    HOOKS
        .get()
        .map(|h| (h.build_framework_runtime_snapshot_envelope)(repo_root, artifact_root_override, task_id_override))
        .unwrap_or_else(|| Err("hooks not registered".into()))
}

pub fn build_automatic_continuity_checkpoint_payload(
    repo_root: &Path,
    task_line: &str,
    summary_text: &str,
    task_id: Option<&str>,
    repointer_focus: bool,
    update_registry_only_if_known: bool,
) -> Value {
    HOOKS
        .get()
        .map(|h| (h.build_automatic_continuity_checkpoint_payload)(
            repo_root, task_line, summary_text, task_id, repointer_focus, update_registry_only_if_known,
        ))
        .unwrap_or(Value::Null)
}

pub fn route_task_with_manifest_fallback(
    runtime_records: &[routing_engine::types::SkillRecord],
    runtime_path: Option<&Path>,
    manifest_path: Option<&Path>,
    host_id: Option<&str>,
    query: &str,
    session_id: &str,
    allow_overlay: bool,
    first_turn: bool,
) -> Result<routing_engine::types::RouteDecision, String> {
    HOOKS
        .get()
        .map(|h| (h.route_task_with_manifest_fallback)(
            runtime_records, runtime_path, manifest_path, host_id,
            query, session_id, allow_overlay, first_turn,
        ))
        .unwrap_or_else(|| Err("hooks not registered".into()))
}

// ── Hook timing ──

pub fn mark_hook_start() {
    if let Some(h) = HOOKS.get() {
        (h.mark_hook_start)();
    }
}

pub fn add_lock_wait_ms(ms: u64) {
    if let Some(h) = HOOKS.get() {
        (h.add_lock_wait_ms)(ms);
    }
}

pub fn add_cargo_check_ms(ms: u64) {
    if let Some(h) = HOOKS.get() {
        (h.add_cargo_check_ms)(ms);
    }
}

pub fn emit_hook_timing_line(label: &str) {
    if let Some(h) = HOOKS.get() {
        (h.emit_hook_timing_line)(label);
    }
}

// ── Telemetry ──

pub fn emit_hook_fired(event: &str, action: &str) {
    if let Some(h) = HOOKS.get() {
        (h.emit_hook_fired)(event, action);
    }
}

pub fn emit_tool_call(tool_name: &str, duration_ms: u64, succeeded: bool) {
    if let Some(h) = HOOKS.get() {
        (h.emit_tool_call)(tool_name, duration_ms, succeeded);
    }
}

pub fn hook_action_from_optional_output(output: Option<&Value>) -> &'static str {
    HOOKS
        .get()
        .map(|h| (h.hook_action_from_optional_output)(output))
        .unwrap_or("unknown")
}

pub fn hook_action_from_output(output: &Value) -> &'static str {
    HOOKS
        .get()
        .map(|h| (h.hook_action_from_output)(output))
        .unwrap_or("unknown")
}

// ── Session call tracker ──

pub fn init_tracker(repo_root: &Path) -> Result<(), String> {
    HOOKS
        .get()
        .map(|h| (h.init_tracker)(repo_root))
        .unwrap_or(Ok(()))
}

pub fn read_tracker_state(repo_root: &Path) -> Result<Value, String> {
    HOOKS
        .get()
        .map(|h| (h.read_tracker_state)(repo_root))
        .unwrap_or_else(|| Err("hooks not registered".into()))
}

pub fn record_tool_call(
    repo_root: &Path,
    tool_name: &str,
    cache_stats: Option<SessionCallCacheStats>,
) -> Result<(), String> {
    HOOKS
        .get()
        .map(|h| (h.record_tool_call)(repo_root, tool_name, cache_stats))
        .unwrap_or(Ok(()))
}

pub fn check_anomalies(repo_root: &Path) -> Result<Vec<String>, String> {
    HOOKS
        .get()
        .map(|h| (h.check_anomalies)(repo_root))
        .unwrap_or(Ok(Vec::new()))
}

// ── Router env flags ──

pub fn router_rs_skip_pre_tool_use_guard() -> bool {
    HOOKS
        .get()
        .map(|h| (h.router_rs_skip_pre_tool_use_guard)())
        .unwrap_or(false)
}

pub fn router_rs_task_ledger_flock_enabled() -> bool {
    HOOKS
        .get()
        .map(|h| (h.router_rs_task_ledger_flock_enabled)())
        .unwrap_or(false)
}

pub fn router_rs_hook_timing_enabled() -> bool {
    HOOKS
        .get()
        .map(|h| (h.router_rs_hook_timing_enabled)())
        .unwrap_or(false)
}

pub fn router_rs_continuity_post_tool_evidence_enabled() -> bool {
    HOOKS
        .get()
        .map(|h| (h.router_rs_continuity_post_tool_evidence_enabled)())
        .unwrap_or(false)
}

pub fn router_rs_cursor_hook_outbound_context_max_bytes() -> usize {
    HOOKS
        .get()
        .map(|h| (h.router_rs_cursor_hook_outbound_context_max_bytes)())
        .unwrap_or(4096)
}

pub fn router_rs_session_call_tracker_tool_keys_max() -> usize {
    HOOKS
        .get()
        .map(|h| (h.router_rs_session_call_tracker_tool_keys_max)())
        .unwrap_or(64)
}

pub fn router_rs_rfv_max_rounds_cap() -> u64 {
    HOOKS
        .get()
        .map(|h| (h.router_rs_rfv_max_rounds_cap)())
        .unwrap_or(10)
}

pub fn router_rs_cursor_hook_state_lock_retries() -> u32 {
    HOOKS
        .get()
        .map(|h| (h.router_rs_cursor_hook_state_lock_retries)())
        .unwrap_or(3)
}

pub fn router_rs_cursor_hook_state_stale_sweep_days() -> u64 {
    HOOKS
        .get()
        .map(|h| (h.router_rs_cursor_hook_state_stale_sweep_days)())
        .unwrap_or(30)
}

pub fn router_rs_cursor_review_gate_stop_max_nudges_cap() -> Option<u32> {
    HOOKS
        .get()
        .and_then(|h| (h.router_rs_cursor_review_gate_stop_max_nudges_cap)())
}

pub fn router_rs_operator_inject_globally_enabled() -> bool {
    HOOKS
        .get()
        .map(|h| (h.router_rs_operator_inject_globally_enabled)())
        .unwrap_or(false)
}

pub fn router_rs_cursor_hook_silent_enabled() -> bool {
    HOOKS
        .get()
        .map(|h| (h.router_rs_cursor_hook_silent_enabled)())
        .unwrap_or(false)
}

pub fn router_rs_cursor_sessionstart_context_max_bytes() -> usize {
    HOOKS
        .get()
        .map(|h| (h.router_rs_cursor_sessionstart_context_max_bytes)())
        .unwrap_or(4096)
}

// ── Paper hooks ──

pub fn maybe_append_paper_prose_context(
    repo_root: &Path,
    prompt: &str,
    out_lines: &mut Vec<String>,
    host: PaperProseHookHostType,
) {
    if let Some(h) = HOOKS.get() {
        (h.maybe_append_paper_prose_context)(repo_root, prompt, out_lines, host);
    }
}

pub fn maybe_append_paper_adversarial_context(
    repo_root: &Path,
    prompt: &str,
    out_lines: &mut Vec<String>,
    host: PaperProseHookHostType,
) {
    if let Some(h) = HOOKS.get() {
        (h.maybe_append_paper_adversarial_context)(repo_root, prompt, out_lines, host);
    }
}

// ── Observation ──

pub fn attach_router_rs_observation(output: &mut Value, host: HookObservationHostType) {
    if let Some(h) = HOOKS.get() {
        (h.attach_router_rs_observation)(output, host);
    }
}

// ── Kernel ──

pub fn ensure_kernel_bootstrap() {
    if let Some(h) = HOOKS.get() {
        (h.ensure_kernel_bootstrap)();
    }
}

// ── MCP pre-guard ──

pub fn evaluate_mcp_pre_guard_safe(
    tool_name: &str,
    arguments: &Value,
    repo_root: &Path,
) -> McpPreGuardVerdictType {
    HOOKS
        .get()
        .map(|h| (h.evaluate_mcp_pre_guard_safe)(tool_name, arguments, repo_root))
        .unwrap_or(McpPreGuardVerdictType {
            blocked: false,
            reason: None,
        })
}

// ── Web fetch guard ──

pub fn validate_and_resolve_web_fetch_url(
    url: &str,
) -> Result<(String, Vec<String>), String> {
    HOOKS
        .get()
        .map(|h| (h.validate_and_resolve_web_fetch_url)(url))
        .unwrap_or_else(|| Err("hooks not registered".into()))
}

pub fn resolve_web_fetch_redirect(
    base: &str,
    location: &str,
) -> Result<String, String> {
    HOOKS
        .get()
        .map(|h| (h.resolve_web_fetch_redirect)(base, location))
        .unwrap_or_else(|| Err("hooks not registered".into()))
}

pub fn resolve_web_fetch_addresses(
    host: &str,
    port: u16,
) -> Result<Vec<String>, String> {
    HOOKS
        .get()
        .map(|h| (h.resolve_web_fetch_addresses)(host, port))
        .unwrap_or_else(|| Err("hooks not registered".into()))
}

// ── RFV loop ──

pub fn framework_rfv_loop(payload: Value) -> Result<Value, String> {
    HOOKS
        .get()
        .map(|h| (h.framework_rfv_loop)(payload))
        .unwrap_or_else(|| Err("hooks not registered".into()))
}

// ── Autopilot goal ──

pub fn framework_goal_drive(payload: Value) -> Result<Value, String> {
    HOOKS
        .get()
        .map(|h| (h.framework_goal_drive)(payload))
        .unwrap_or_else(|| Err("hooks not registered".into()))
}

pub fn read_goal_state(repo_root: &Path) -> Result<Value, String> {
    HOOKS
        .get()
        .map(|h| (h.read_goal_state)(repo_root))
        .unwrap_or_else(|| Err("hooks not registered".into()))
}
