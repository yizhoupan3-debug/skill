#![recursion_limit = "256"]

use chrono::Utc;
use clap::{ArgAction, Args, Parser, Subcommand};
use rayon::ThreadPoolBuilder;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod background_state;
mod browser_mcp;
mod cli_modes;
mod codex_hooks;
mod execution_contract;
mod framework_profile;
mod framework_runtime;
mod hook_policy;
mod host_integration;
mod route;
mod runtime_storage;
mod session_supervisor;
mod stdio_transport;
mod trace_runtime;

use background_state::handle_background_state_operation;
use browser_mcp::{
    resolve_browser_mcp_attach_artifact, run_browser_mcp_stdio_loop, BrowserAttachConfig,
};
use cli_modes::{dispatch_runtime_output_mode_stdio, handles_runtime_output_stdio_op};
use codex_hooks::{build_codex_hook_projection, run_codex_audit_hook, sync_host_entrypoints};
use execution_contract::{
    build_execution_contract_bundle, build_execution_kernel_contracts_by_mode,
    build_execution_kernel_metadata_contract, build_steady_state_execution_kernel_metadata,
    decode_execution_response_value, normalize_execution_kernel_contract_value,
    normalize_execution_kernel_metadata_contract_value,
    validate_execution_kernel_steady_state_metadata_value, EXECUTION_AUTHORITY,
    EXECUTION_MODEL_ID_SOURCE, EXECUTION_RESPONSE_SHAPE_DRY_RUN,
    EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY, EXECUTION_SCHEMA_VERSION,
};
use framework_profile::{
    build_codex_artifact_bundle, build_control_plane_contract_descriptors, build_profile_bundle,
    load_framework_profile,
};
use framework_runtime::{
    build_framework_alias_envelope, build_framework_contract_summary_envelope,
    build_framework_memory_policy_envelope, build_framework_memory_recall_envelope,
    build_framework_prompt_compression_envelope, build_framework_refresh_payload,
    build_framework_runtime_snapshot_envelope, build_framework_statusline, resolve_repo_root_arg,
    write_framework_session_artifacts, FrameworkAliasBuildOptions,
};
use hook_policy::{
    evaluate_hook_policy, evaluate_hook_policy_value, hook_policy_contract,
    HookPolicyEvaluateRequest,
};
use host_integration::run_host_integration_from_args;
use route::{
    build_route_diff_report, build_route_policy, build_route_resolution, build_route_snapshot,
    build_search_results_payload, literal_framework_alias_decision, load_inline_records,
    load_records, load_records_cached_for_stdio, load_records_from_manifest, route_task,
    search_skills, MatchRow, RouteDecision, RouteDecisionSnapshotPayload,
    RouteSnapshotEnvelopePayload, RouteSnapshotRequestPayload, SearchResultsPayload, SkillRecord,
    ROUTE_AUTHORITY, ROUTE_SNAPSHOT_SCHEMA_VERSION,
};
use runtime_storage::{
    build_checkpoint_control_plane_compiler_payload, resolve_storage_backend,
    runtime_backend_family_catalog_payload, runtime_backend_family_parity_payload,
    runtime_storage_operation, storage_artifact_exists, storage_read_text, ResolvedStorageBackend,
    RuntimeStorageRequestPayload,
};
use session_supervisor::handle_session_supervisor_operation;
#[cfg(test)]
use stdio_transport::{
    handle_stdio_json_line, DEFAULT_ROUTER_STDIO_POOL_SIZE, MAX_ROUTER_STDIO_POOL_SIZE,
};
use stdio_transport::{run_stdio_json_loop, runtime_concurrency_defaults_payload};
use stdio_transport::{StdioJsonRequestPayload, StdioJsonResponsePayload};
use trace_runtime::{
    compact_trace_stream, record_trace_event, TraceCompactRequestPayload,
    TraceRecordEventRequestPayload,
};

#[cfg(test)]
use execution_contract::{
    EXECUTION_KERNEL_AUTHORITY, EXECUTION_KERNEL_FALLBACK_POLICY, EXECUTION_KERNEL_KIND,
    EXECUTION_METADATA_CONTRACT_SCHEMA_VERSION, EXECUTION_METADATA_SCHEMA_VERSION,
    EXECUTION_PROMPT_PREVIEW_OWNER,
};
#[cfg(test)]
use framework_runtime::FRAMEWORK_ALIAS_SCHEMA_VERSION;
#[cfg(test)]
use route::{ROUTE_POLICY_SCHEMA_VERSION, ROUTE_REPORT_SCHEMA_VERSION};

const RUNTIME_CONTROL_PLANE_SCHEMA_VERSION: &str = "router-rs-runtime-control-plane-v1";
const RUNTIME_CONTROL_PLANE_AUTHORITY: &str = "rust-runtime-control-plane";
static WRITE_TEXT_PAYLOAD_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
const RUNTIME_INTEGRATOR_SCHEMA_VERSION: &str = "router-rs-runtime-integrator-v1";
const RUNTIME_INTEGRATOR_AUTHORITY: &str = "rust-runtime-integrator";
const SANDBOX_CONTROL_SCHEMA_VERSION: &str = "router-rs-sandbox-control-v1";
const SANDBOX_CONTROL_AUTHORITY: &str = "rust-sandbox-control";
const SANDBOX_EVENT_SCHEMA_VERSION: &str = "runtime-sandbox-event-v1";
const BACKGROUND_CONTROL_SCHEMA_VERSION: &str = "router-rs-background-control-v1";
const BACKGROUND_CONTROL_AUTHORITY: &str = "rust-background-control";
const TRACE_DESCRIPTOR_SCHEMA_VERSION: &str = "router-rs-trace-descriptor-v1";
const TRACE_DESCRIPTOR_AUTHORITY: &str = "rust-runtime-trace-descriptor";
const CHECKPOINT_RESUME_MANIFEST_SCHEMA_VERSION: &str = "router-rs-checkpoint-resume-manifest-v1";
const CHECKPOINT_RESUME_MANIFEST_AUTHORITY: &str = "rust-runtime-checkpoint-manifest";
const FRAMEWORK_REFRESH_SCHEMA_VERSION: &str = "router-rs-framework-refresh-v1";
const FRAMEWORK_REFRESH_CONFIRMATION: &str = "下一轮执行 prompt 已准备好，并且已经复制到剪贴板。";
const TRANSPORT_BINDING_WRITE_SCHEMA_VERSION: &str = "router-rs-transport-binding-write-v1";
const TRANSPORT_BINDING_WRITE_AUTHORITY: &str = "rust-runtime-transport-binding-writer";
const CHECKPOINT_MANIFEST_WRITE_SCHEMA_VERSION: &str = "router-rs-checkpoint-manifest-write-v1";
const CHECKPOINT_MANIFEST_WRITE_AUTHORITY: &str = "rust-runtime-checkpoint-manifest-writer";
const RUNTIME_STORAGE_SCHEMA_VERSION: &str = "router-rs-runtime-storage-v1";
const RUNTIME_STORAGE_AUTHORITY: &str = "rust-runtime-storage";
const ATTACHED_RUNTIME_EVENT_ATTACH_AUTHORITY: &str = "rust-runtime-attached-event-transport";
const TRACE_STREAM_REPLAY_SCHEMA_VERSION: &str = "router-rs-trace-stream-replay-v1";
const TRACE_STREAM_INSPECT_SCHEMA_VERSION: &str = "router-rs-trace-stream-inspect-v1";
const TRACE_COMPACTION_DELTA_WRITE_SCHEMA_VERSION: &str =
    "router-rs-trace-compaction-delta-write-v1";
const TRACE_METADATA_WRITE_SCHEMA_VERSION: &str = "router-rs-trace-metadata-write-v1";
const TRACE_STREAM_IO_AUTHORITY: &str = "rust-runtime-trace-io";
const TRACE_METADATA_WRITE_AUTHORITY: &str = "rust-runtime-trace-metadata-writer";
const RUNTIME_OBSERVABILITY_EXPORTER_SCHEMA_VERSION: &str = "runtime-observability-exporter-v1";
const RUNTIME_OBSERVABILITY_METRIC_RECORD_SCHEMA_VERSION: &str =
    "runtime-observability-metric-record-v1";
const RUNTIME_OBSERVABILITY_METRIC_CATALOG_SCHEMA_VERSION: &str =
    "runtime-observability-metric-catalog-v1";
const RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION: &str = "runtime-observability-metrics-v1";
const RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION: &str = "runtime-observability-dashboard-v1";
const RUNTIME_OBSERVABILITY_HEALTH_SNAPSHOT_SCHEMA_VERSION: &str =
    "runtime-observability-health-snapshot-v1";
const RUNTIME_OBSERVABILITY_SIGNAL_VOCABULARY: &str = "shared-runtime-v1";
const DEFAULT_MAX_CONCURRENT_SUBAGENTS: usize = 8;
const MAX_CONCURRENT_SUBAGENTS_LIMIT: usize = 32;
const DEFAULT_SUBAGENT_TIMEOUT_SECONDS: u64 = 900;
const DEFAULT_MAX_BACKGROUND_JOBS: usize = 16;
const MAX_BACKGROUND_JOBS_LIMIT: usize = 64;
const DEFAULT_BACKGROUND_JOB_TIMEOUT_SECONDS: u64 = 600;
const DEFAULT_COMPUTE_THREADS: usize = 0;
const MAX_COMPUTE_THREADS: usize = 64;

#[derive(Subcommand, Debug, Clone)]
enum RouterCommand {
    Route(RouteCommand),
    Search(SearchCommand),
    Framework {
        #[command(subcommand)]
        command: FrameworkCommand,
    },
    Codex {
        #[command(subcommand)]
        command: CodexCommand,
    },
    Trace {
        #[command(subcommand)]
        command: TraceCommand,
    },
    Storage {
        #[command(subcommand)]
        command: StorageCommand,
    },
    Browser {
        #[command(subcommand)]
        command: BrowserCommand,
    },
    Profile {
        #[command(subcommand)]
        command: ProfileCommand,
    },
    Migrate {
        #[command(subcommand)]
        command: MigrateCommand,
    },
    HookPolicy {
        #[command(subcommand)]
        command: HookPolicyCommand,
    },
}

#[derive(Args, Debug, Clone)]
struct RouteCommand {
    query: String,
    #[arg(long, default_value = "route-cli")]
    session_id: String,
    #[arg(long, default_value_t = true, action = ArgAction::Set, num_args = 1)]
    allow_overlay: bool,
    #[arg(long, default_value_t = true, action = ArgAction::Set, num_args = 1)]
    first_turn: bool,
    #[arg(long)]
    runtime: Option<PathBuf>,
    #[arg(long)]
    manifest: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
struct SearchCommand {
    query: String,
    #[arg(long, default_value_t = 5)]
    limit: usize,
    #[arg(long)]
    runtime: Option<PathBuf>,
    #[arg(long)]
    manifest: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}

#[derive(Subcommand, Debug, Clone)]
enum FrameworkCommand {
    Snapshot(FrameworkSnapshotCommand),
    ContractSummary(RepoRootCommand),
    MemoryRecall(FrameworkMemoryRecallCommand),
    MemoryPolicy(JsonInputCommand),
    PromptCompression(JsonInputCommand),
    Refresh(FrameworkRefreshCommand),
    Statusline(RepoRootCommand),
    SessionArtifactWrite(JsonInputCommand),
    Alias(FrameworkAliasCommand),
    HostIntegration(ForwardedArgsCommand),
    Contracts,
}

#[derive(Subcommand, Debug, Clone)]
enum CodexCommand {
    HookProjection,
    Sync(RepoRootCommand),
    Check(RepoRootCommand),
    Hook(CodexHookCommand),
    HostIntegration(ForwardedArgsCommand),
}

#[derive(Subcommand, Debug, Clone)]
enum TraceCommand {
    RecordEvent(JsonInputCommand),
    StreamReplay(JsonInputCommand),
    StreamInspect(JsonInputCommand),
    Compact(JsonInputCommand),
    WriteCompactionDelta(JsonInputCommand),
    WriteMetadata(JsonInputCommand),
}

#[derive(Subcommand, Debug, Clone)]
enum StorageCommand {
    Runtime(JsonInputCommand),
    CheckpointControlPlane(JsonInputCommand),
    BackendCatalog,
    BackendParity(StorageBackendParityCommand),
}

#[derive(Subcommand, Debug, Clone)]
enum BrowserCommand {
    McpStdio(BrowserMcpStdioCommand),
    ResolveAttachArtifact(BrowserResolveAttachCommand),
}

#[derive(Subcommand, Debug, Clone)]
enum ProfileCommand {
    Emit(ProfilePathCommand),
    Artifacts(ProfilePathCommand),
}

#[derive(Subcommand, Debug, Clone)]
enum MigrateCommand {
    ArchivedMemory(MigrateArchivedMemoryCommand),
    CurrentArtifactClutter(CurrentArtifactClutterCommand),
}

#[derive(Subcommand, Debug, Clone)]
enum HookPolicyCommand {
    Evaluate(JsonInputCommand),
    Contract,
}

#[derive(Args, Debug, Clone)]
struct RepoRootCommand {
    #[arg(long)]
    repo_root: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
struct JsonInputCommand {
    #[arg(long)]
    input_json: String,
}

#[derive(Args, Debug, Clone)]
struct FrameworkSnapshotCommand {
    #[arg(long)]
    repo_root: Option<PathBuf>,
    #[arg(long)]
    artifact_source_dir: Option<PathBuf>,
    #[arg(long)]
    task_id: Option<String>,
}

#[derive(Args, Debug, Clone)]
struct FrameworkMemoryRecallCommand {
    query: String,
    #[arg(long)]
    repo_root: Option<PathBuf>,
    #[arg(long, default_value_t = 5)]
    limit: usize,
    #[arg(long, default_value = "stable")]
    mode: String,
    #[arg(long)]
    memory_root: Option<PathBuf>,
    #[arg(long)]
    artifact_source_dir: Option<PathBuf>,
    #[arg(long)]
    task_id: Option<String>,
}

#[derive(Args, Debug, Clone)]
struct FrameworkRefreshCommand {
    #[arg(long)]
    repo_root: Option<PathBuf>,
    #[arg(long, default_value_t = 4)]
    max_lines: usize,
    #[arg(long)]
    verbose: bool,
}

#[derive(Args, Debug, Clone)]
struct FrameworkAliasCommand {
    alias: String,
    #[arg(long)]
    repo_root: Option<PathBuf>,
    #[arg(long, default_value_t = 4)]
    max_lines: usize,
    #[arg(long)]
    compact: bool,
    #[arg(long)]
    host_id: Option<String>,
}

#[derive(Args, Debug, Clone)]
struct CodexHookCommand {
    command: String,
    #[arg(long)]
    repo_root: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
struct ForwardedArgsCommand {
    #[arg(num_args = 1.., trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

#[derive(Args, Debug, Clone)]
struct StorageBackendParityCommand {
    #[arg(long)]
    store: Option<String>,
    #[arg(long)]
    checkpointer: Option<String>,
    #[arg(long)]
    trace: Option<String>,
    #[arg(long)]
    state: Option<String>,
}

#[derive(Args, Debug, Clone)]
struct BrowserMcpStdioCommand {
    #[arg(long)]
    repo_root: Option<PathBuf>,
    #[arg(long)]
    headless: Option<String>,
    #[arg(long)]
    runtime_attach_artifact_path: Option<String>,
    #[arg(long)]
    runtime_attach_descriptor_path: Option<String>,
}

#[derive(Args, Debug, Clone)]
struct BrowserResolveAttachCommand {
    #[arg(long)]
    repo_root: Option<PathBuf>,
    #[arg(long)]
    search_root: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
struct ProfilePathCommand {
    #[arg(long)]
    framework_profile: PathBuf,
    #[arg(long, default_value_t = false)]
    full: bool,
}

#[derive(Args, Debug, Clone)]
struct MigrateArchivedMemoryCommand {
    #[arg(long)]
    repo_root: Option<PathBuf>,
    #[arg(long)]
    memory_root: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
struct CurrentArtifactClutterCommand {
    active_task_id: String,
    #[arg(long)]
    repo_root: Option<PathBuf>,
}

#[derive(Parser, Debug)]
#[command(name = "router-rs")]
#[command(about = "Fast Rust routing core for skill lookup")]
#[command(override_usage = "router-rs <COMMAND>")]
#[command(
    help_template = "{about-section}\nUsage: {usage}\n\nCommands:\n{subcommands}\n\nUse `router-rs <command> --help` for command-specific options.\n"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<RouterCommand>,
    #[arg(long)]
    repo_root: Option<PathBuf>,
    #[arg(long)]
    query: Option<String>,
    #[arg(long, default_value_t = 5)]
    limit: usize,
    #[arg(long)]
    runtime: Option<PathBuf>,
    #[arg(long)]
    manifest: Option<PathBuf>,
    #[arg(long)]
    framework_profile: Option<PathBuf>,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    stdio_json: bool,
    #[arg(long)]
    stdio_max_concurrency: Option<usize>,
    #[arg(long)]
    compute_threads: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExecuteRequestPayload {
    schema_version: String,
    task: String,
    session_id: String,
    user_id: String,
    selected_skill: String,
    overlay_skill: Option<String>,
    layer: String,
    route_engine: Option<String>,
    diagnostic_route_mode: Option<String>,
    reasons: Vec<String>,
    prompt_preview: Option<String>,
    dry_run: bool,
    trace_event_count: usize,
    trace_output_path: Option<String>,
    default_output_tokens: usize,
    model_id: String,
    aggregator_base_url: String,
    aggregator_api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExecuteUsagePayload {
    input_tokens: usize,
    output_tokens: usize,
    total_tokens: usize,
    mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExecuteResponsePayload {
    execution_schema_version: String,
    authority: String,
    session_id: String,
    user_id: String,
    skill: String,
    overlay: Option<String>,
    live_run: bool,
    content: String,
    usage: ExecuteUsagePayload,
    prompt_preview: Option<String>,
    model_id: Option<String>,
    metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BackgroundControlRequestPayload {
    schema_version: String,
    operation: String,
    multitask_strategy: Option<String>,
    current_status: Option<String>,
    task_active: Option<bool>,
    task_done: Option<bool>,
    active_job_count: Option<usize>,
    capacity_limit: Option<usize>,
    attempt: Option<usize>,
    retry_count: Option<usize>,
    max_attempts: Option<usize>,
    backoff_base_seconds: Option<f64>,
    backoff_multiplier: Option<f64>,
    max_backoff_seconds: Option<f64>,
    requested_parallel_group_id: Option<String>,
    request_parallel_group_ids: Option<Vec<Option<String>>>,
    request_lane_ids: Option<Vec<Option<String>>>,
    lane_id_prefix: Option<String>,
    batch_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SandboxControlRequestPayload {
    schema_version: String,
    operation: String,
    sandbox_id: Option<String>,
    profile_id: Option<String>,
    current_state: Option<String>,
    next_state: Option<String>,
    cleanup_failed: Option<bool>,
    tool_category: Option<String>,
    capability_categories: Option<Vec<String>>,
    dedicated_profile: Option<bool>,
    budget_cpu: Option<f64>,
    budget_memory: Option<i64>,
    budget_wall_clock: Option<f64>,
    budget_output_size: Option<i64>,
    probe_cpu: Option<f64>,
    probe_memory: Option<i64>,
    probe_wall_clock: Option<f64>,
    probe_output_size: Option<i64>,
    error_kind: Option<String>,
    event_log_path: Option<String>,
    trace_event: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SandboxControlResponsePayload {
    schema_version: String,
    authority: String,
    operation: String,
    current_state: Option<String>,
    next_state: Option<String>,
    allowed: bool,
    resolved_state: Option<String>,
    reason: String,
    error: Option<String>,
    failure_reason: Option<String>,
    budget_violation: Option<String>,
    cleanup_required: Option<bool>,
    quarantined: Option<bool>,
    effective_capabilities: Option<Vec<String>>,
    sandbox_id: Option<String>,
    profile_id: Option<String>,
    event_schema_version: Option<String>,
    event_log_path: Option<String>,
    event_written: bool,
    event_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BackgroundControlEffectPlanPayload {
    next_step: String,
    terminal_status: Option<String>,
    resolved_status: Option<String>,
    finalize_immediately: Option<bool>,
    cancel_running_task: Option<bool>,
    next_retry_count: Option<usize>,
    backoff_seconds: Option<f64>,
    wait_timeout_seconds: Option<f64>,
    wait_poll_interval_seconds: Option<f64>,
    sleep_seconds: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BackgroundControlResponsePayload {
    schema_version: String,
    authority: String,
    operation: String,
    resolved_parallel_group_id: Option<String>,
    lane_ids: Option<Vec<String>>,
    normalized_multitask_strategy: Option<String>,
    supported_multitask_strategies: Vec<String>,
    strategy_supported: bool,
    accepted: Option<bool>,
    requires_takeover: Option<bool>,
    error: Option<String>,
    should_retry: Option<bool>,
    next_retry_count: Option<usize>,
    backoff_seconds: Option<f64>,
    terminal_status: Option<String>,
    resolved_status: Option<String>,
    finalize_immediately: Option<bool>,
    cancel_running_task: Option<bool>,
    reason: String,
    effect_plan: BackgroundControlEffectPlanPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TraceStreamReplayRequestPayload {
    pub(crate) path: Option<String>,
    pub(crate) event_stream_text: Option<String>,
    pub(crate) compaction_manifest_path: Option<String>,
    pub(crate) compaction_manifest_text: Option<String>,
    pub(crate) compaction_state_text: Option<String>,
    pub(crate) compaction_artifact_index_text: Option<String>,
    pub(crate) compaction_delta_text: Option<String>,
    pub(crate) session_id: Option<String>,
    pub(crate) job_id: Option<String>,
    pub(crate) stream_scope_fields: Option<Vec<String>>,
    pub(crate) after_event_id: Option<String>,
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TraceStreamInspectRequestPayload {
    pub(crate) path: Option<String>,
    pub(crate) event_stream_text: Option<String>,
    pub(crate) compaction_manifest_path: Option<String>,
    pub(crate) compaction_manifest_text: Option<String>,
    pub(crate) compaction_state_text: Option<String>,
    pub(crate) compaction_artifact_index_text: Option<String>,
    pub(crate) compaction_delta_text: Option<String>,
    pub(crate) session_id: Option<String>,
    pub(crate) job_id: Option<String>,
    pub(crate) stream_scope_fields: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TraceStreamReplayCursorPayload {
    pub(crate) event_id: Option<String>,
    pub(crate) event_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TraceStreamReplayResponsePayload {
    pub(crate) schema_version: String,
    pub(crate) authority: String,
    pub(crate) path: String,
    pub(crate) source_kind: String,
    pub(crate) event_count: usize,
    pub(crate) latest_event_id: Option<String>,
    pub(crate) latest_event_kind: Option<String>,
    pub(crate) latest_event_timestamp: Option<String>,
    pub(crate) latest_cursor: Option<Value>,
    pub(crate) after_event_id: Option<String>,
    pub(crate) window_start_index: usize,
    pub(crate) has_more: bool,
    pub(crate) next_cursor: Option<TraceStreamReplayCursorPayload>,
    pub(crate) events: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TraceStreamInspectResponsePayload {
    pub(crate) schema_version: String,
    pub(crate) authority: String,
    pub(crate) path: String,
    pub(crate) source_kind: String,
    pub(crate) event_count: usize,
    pub(crate) latest_event_id: Option<String>,
    pub(crate) latest_event_kind: Option<String>,
    pub(crate) latest_event_timestamp: Option<String>,
    pub(crate) latest_cursor: Option<Value>,
    pub(crate) recovery: Option<Value>,
    pub(crate) reroute_count: usize,
    pub(crate) retry_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TraceCompactionDeltaWriteRequestPayload {
    path: String,
    delta: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TraceCompactionDeltaWriteResponsePayload {
    schema_version: String,
    authority: String,
    path: String,
    bytes_written: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TraceMetadataWriteRequestPayload {
    output_path: String,
    #[serde(default)]
    mirror_paths: Vec<String>,
    #[serde(default = "default_true")]
    write_outputs: bool,
    task: String,
    #[serde(default)]
    matched_skills: Vec<String>,
    owner: String,
    gate: String,
    overlay: Option<String>,
    reroute_count: Option<usize>,
    retry_count: Option<usize>,
    #[serde(default)]
    artifact_paths: Vec<String>,
    verification_status: String,
    session_id: Option<String>,
    job_id: Option<String>,
    event_stream_path: Option<String>,
    event_stream_text: Option<String>,
    compaction_manifest_path: Option<String>,
    compaction_manifest_text: Option<String>,
    compaction_state_text: Option<String>,
    compaction_artifact_index_text: Option<String>,
    compaction_delta_text: Option<String>,
    stream_scope_fields: Option<Vec<String>>,
    framework_version: Option<String>,
    metadata_schema_version: Option<String>,
    routing_runtime_version: Option<u64>,
    runtime_path: Option<String>,
    ts: Option<String>,
    trace_event_schema_version: Option<String>,
    trace_event_sink_schema_version: Option<String>,
    parallel_group: Option<Value>,
    supervisor_projection: Option<Value>,
    control_plane: Option<Value>,
    stream: Option<Value>,
    events: Option<Vec<Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TraceMetadataWriteResponsePayload {
    schema_version: String,
    authority: String,
    output_path: String,
    mirror_paths: Vec<String>,
    bytes_written: usize,
    routing_runtime_version: u64,
    payload_text: String,
}

fn print_json_value<T: Serialize>(payload: &T) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string(payload).map_err(|err| format!("serialize output failed: {err}"))?
    );
    Ok(())
}

fn parse_json_input<T>(raw: &str, context: &str) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_str(raw).map_err(|err| format!("parse {context} input failed: {err}"))
}

fn should_retry_with_manifest(decision: &RouteDecision) -> bool {
    decision.score < 35.0
        || (decision.selected_skill == "systematic-debugging" && decision.score < 35.0)
        || decision
            .reasons
            .iter()
            .any(|reason| reason.contains("fell back to highest-priority layer owner"))
        || decision.reasons.iter().any(|reason| {
            reason.contains("Fallback owner selected") || reason.contains("No explicit keyword hit")
        })
}

fn route_decision_is_no_hit(decision: &RouteDecision) -> bool {
    decision.score <= 0.0
        || decision.reasons.iter().any(|reason| {
            reason.contains("No explicit keyword hit")
                || reason.contains("fell back to highest-priority layer owner")
                || reason.contains("Only overlay signals matched")
        })
}

fn route_reason_terms(decision: &RouteDecision) -> Vec<String> {
    decision
        .reasons
        .iter()
        .filter_map(|reason| reason.split_once(':').map(|(_, terms)| terms))
        .flat_map(|terms| terms.trim_end_matches('.').split(','))
        .map(|term| term.trim().to_ascii_lowercase())
        .filter(|term| !term.is_empty())
        .collect()
}

fn has_non_generic_manifest_signal(decision: &RouteDecision) -> bool {
    const GENERIC_FULL_MANIFEST_TERMS: [&str; 5] =
        ["runtime", "debug", "backend", "review", "plan"];

    if decision.reasons.iter().any(|reason| {
        reason.contains("Exact skill name matched")
            || reason.contains("Framework alias entrypoint matched explicitly")
    }) {
        return true;
    }

    let terms = route_reason_terms(decision);
    if terms.iter().any(|term| term.contains("架构")) {
        return true;
    }
    !terms.is_empty()
        && terms.iter().any(|term| {
            !GENERIC_FULL_MANIFEST_TERMS
                .iter()
                .any(|generic| term == generic)
        })
}

fn should_accept_manifest_fallback(
    hot_decision: &RouteDecision,
    full_decision: &RouteDecision,
    should_retry: bool,
    explicit_manifest: bool,
) -> bool {
    if explicit_manifest {
        return full_decision.score > hot_decision.score
            || (full_decision.score == hot_decision.score
                && full_decision.selected_skill != hot_decision.selected_skill)
            || (full_decision.selected_skill == hot_decision.selected_skill
                && full_decision.overlay_skill.is_some()
                && hot_decision.overlay_skill.is_none());
    }

    if full_decision.selected_skill == hot_decision.selected_skill
        && full_decision.overlay_skill.is_some()
        && hot_decision.overlay_skill.is_none()
    {
        return true;
    }

    if !should_retry
        || !(route_decision_is_no_hit(hot_decision)
            || hot_decision.score < 25.0
            || (hot_decision.score < 35.0
                && matches!(
                    hot_decision.selected_skill.as_str(),
                    "agent-swarm-orchestration" | "doc" | "design-md" | "pdf" | "sentry"
                ))
            || hot_decision.selected_skill == "systematic-debugging")
    {
        if full_decision.score >= hot_decision.score + 8.0
            && has_non_generic_manifest_signal(full_decision)
        {
            return true;
        }
        return false;
    }

    let low_score_review_fallback = full_decision.score >= 20.0
        && matches!(
            full_decision.selected_skill.as_str(),
            "architect-review" | "code-review"
        );

    if full_decision.score <= 10.0
        && !matches!(
            full_decision.selected_skill.as_str(),
            "architect-review" | "code-review"
        )
    {
        return false;
    }

    if !low_score_review_fallback && !has_non_generic_manifest_signal(full_decision) {
        return false;
    }

    (full_decision.score > hot_decision.score
        || (full_decision.score == hot_decision.score
            && full_decision.selected_skill != hot_decision.selected_skill))
        || low_score_review_fallback
}

fn repo_root_from_cargo_manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn manifest_fallback_path(
    runtime_path: Option<&Path>,
    manifest_path: Option<&Path>,
) -> Result<Option<PathBuf>, String> {
    if let Some(path) = manifest_path {
        if path.exists() {
            return Ok(Some(path.to_path_buf()));
        }
        return Err(format!("manifest path does not exist: {}", path.display()));
    }
    let fallback = runtime_path
        .and_then(Path::parent)
        .map(|parent| parent.join("SKILL_MANIFEST.json"))
        .filter(|path| path.exists())
        .or_else(|| {
            Some(
                repo_root_from_cargo_manifest_dir()
                    .join("skills")
                    .join("SKILL_MANIFEST.json"),
            )
            .filter(|path| path.exists())
        });
    Ok(fallback)
}

fn route_task_with_manifest_fallback(
    runtime_records: &[SkillRecord],
    runtime_path: Option<&Path>,
    manifest_path: Option<&Path>,
    query: &str,
    session_id: &str,
    allow_overlay: bool,
    first_turn: bool,
) -> Result<RouteDecision, String> {
    if let Some(decision) = literal_framework_alias_decision(runtime_records, query, session_id) {
        return Ok(decision);
    }
    let hot_decision = route_task(
        runtime_records,
        query,
        session_id,
        allow_overlay,
        first_turn,
    )?;
    let should_retry = should_retry_with_manifest(&hot_decision);
    let Some(fallback_path) = manifest_fallback_path(runtime_path, manifest_path)? else {
        return Ok(hot_decision);
    };
    let full_records = load_records_from_manifest(&fallback_path)?;
    if full_records.len() <= runtime_records.len() {
        return Ok(hot_decision);
    }
    let full_decision = route_task(&full_records, query, session_id, allow_overlay, first_turn)?;
    if should_accept_manifest_fallback(
        &hot_decision,
        &full_decision,
        should_retry,
        manifest_path.is_some(),
    ) {
        return Ok(full_decision);
    }
    Ok(hot_decision)
}

fn dispatch_router_command(command: RouterCommand) -> Result<(), String> {
    match command {
        RouterCommand::Route(command) => {
            let records = load_records(command.runtime.as_deref(), command.manifest.as_deref())?;
            let decision = route_task_with_manifest_fallback(
                &records,
                command.runtime.as_deref(),
                command.manifest.as_deref(),
                &command.query,
                &command.session_id,
                command.allow_overlay,
                command.first_turn,
            )?;
            print_json_value(&decision)
        }
        RouterCommand::Search(command) => {
            let manifest_path =
                manifest_fallback_path(command.runtime.as_deref(), command.manifest.as_deref())?;
            let records = if let Some(path) = manifest_path.as_deref() {
                load_records_from_manifest(path)?
            } else {
                load_records(command.runtime.as_deref(), command.manifest.as_deref())?
            };
            let rows = search_skills(&records, &command.query, command.limit);
            let payload = build_search_results_payload(&command.query, rows.clone());
            if command.json {
                return print_json_value(&payload);
            }
            print_search_results(&command.query, &payload, rows);
            Ok(())
        }
        RouterCommand::Framework { command } => dispatch_framework_command(command),
        RouterCommand::Codex { command } => dispatch_codex_command(command),
        RouterCommand::Trace { command } => dispatch_trace_command(command),
        RouterCommand::Storage { command } => dispatch_storage_command(command),
        RouterCommand::Browser { command } => dispatch_browser_command(command),
        RouterCommand::Profile { command } => dispatch_profile_command(command),
        RouterCommand::Migrate { command } => dispatch_migrate_command(command),
        RouterCommand::HookPolicy { command } => dispatch_hook_policy_command(command),
    }
}

fn dispatch_framework_command(command: FrameworkCommand) -> Result<(), String> {
    match command {
        FrameworkCommand::Snapshot(command) => {
            let repo_root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            print_json_value(&build_framework_runtime_snapshot_envelope(
                &repo_root,
                command.artifact_source_dir.as_deref(),
                command.task_id.as_deref(),
            )?)
        }
        FrameworkCommand::ContractSummary(command) => {
            let repo_root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            print_json_value(&build_framework_contract_summary_envelope(&repo_root)?)
        }
        FrameworkCommand::MemoryRecall(command) => {
            let repo_root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            print_json_value(&build_framework_memory_recall_envelope(
                &repo_root,
                &command.query,
                command.limit,
                &command.mode,
                command.memory_root.as_deref(),
                command.artifact_source_dir.as_deref(),
                command.task_id.as_deref(),
            )?)
        }
        FrameworkCommand::MemoryPolicy(command) => {
            let payload =
                parse_json_input::<Value>(&command.input_json, "framework memory policy")?;
            print_json_value(&build_framework_memory_policy_envelope(payload)?)
        }
        FrameworkCommand::PromptCompression(command) => {
            let payload =
                parse_json_input::<Value>(&command.input_json, "framework prompt compression")?;
            print_json_value(&build_framework_prompt_compression_envelope(payload)?)
        }
        FrameworkCommand::Refresh(command) => {
            let repo_root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            let refresh_payload =
                build_framework_refresh_payload(&repo_root, command.max_lines, command.verbose)?;
            let prompt = refresh_payload
                .get("prompt")
                .and_then(Value::as_str)
                .ok_or_else(|| "framework refresh payload is missing prompt".to_string())?;
            let clipboard = copy_text_to_clipboard(prompt)?;
            let mut refresh = refresh_payload
                .as_object()
                .cloned()
                .ok_or_else(|| "framework refresh payload must be an object".to_string())?;
            refresh.insert(
                "confirmation".to_string(),
                Value::String(FRAMEWORK_REFRESH_CONFIRMATION.to_string()),
            );
            refresh.insert("clipboard".to_string(), clipboard);
            print_json_value(&json!({
                "schema_version": FRAMEWORK_REFRESH_SCHEMA_VERSION,
                "authority": framework_runtime::FRAMEWORK_RUNTIME_AUTHORITY,
                "refresh": Value::Object(refresh),
            }))
        }
        FrameworkCommand::Statusline(command) => {
            let repo_root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            println!("{}", build_framework_statusline(&repo_root)?);
            Ok(())
        }
        FrameworkCommand::SessionArtifactWrite(command) => {
            let payload =
                parse_json_input::<Value>(&command.input_json, "framework session artifact write")?;
            print_json_value(&write_framework_session_artifacts(payload)?)
        }
        FrameworkCommand::Alias(command) => {
            let repo_root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            print_json_value(&build_framework_alias_envelope(
                &repo_root,
                &command.alias,
                FrameworkAliasBuildOptions {
                    max_lines: command.max_lines,
                    compact: command.compact,
                    host_id: command.host_id.as_deref(),
                },
            )?)
        }
        FrameworkCommand::HostIntegration(command) => {
            let payload = run_host_integration_from_args(&command.args)?;
            print_json_value(&payload)
        }
        FrameworkCommand::Contracts => {
            print_json_value(&build_control_plane_contract_descriptors())
        }
    }
}

fn dispatch_codex_command(command: CodexCommand) -> Result<(), String> {
    match command {
        CodexCommand::HookProjection => print_json_value(&build_codex_hook_projection()),
        CodexCommand::Sync(command) => {
            let repo_root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            print_json_value(&sync_host_entrypoints(&repo_root, true)?)
        }
        CodexCommand::Check(command) => {
            let repo_root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            print_json_value(&sync_host_entrypoints(&repo_root, false)?)
        }
        CodexCommand::Hook(command) => {
            let repo_root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            if let Some(payload) = run_codex_audit_hook(&command.command, &repo_root)? {
                print_json_value(&payload)?;
            }
            Ok(())
        }
        CodexCommand::HostIntegration(command) => {
            let payload = run_host_integration_from_args(&command.args)?;
            print_json_value(&payload)
        }
    }
}

fn dispatch_trace_command(command: TraceCommand) -> Result<(), String> {
    match command {
        TraceCommand::RecordEvent(command) => {
            let payload = parse_json_input::<TraceRecordEventRequestPayload>(
                &command.input_json,
                "trace record event",
            )?;
            print_json_value(&record_trace_event(payload)?)
        }
        TraceCommand::StreamReplay(command) => {
            let payload = parse_json_input::<TraceStreamReplayRequestPayload>(
                &command.input_json,
                "trace stream replay",
            )?;
            print_json_value(&replay_trace_stream(payload)?)
        }
        TraceCommand::StreamInspect(command) => {
            let payload = parse_json_input::<TraceStreamInspectRequestPayload>(
                &command.input_json,
                "trace stream inspect",
            )?;
            print_json_value(&inspect_trace_stream(payload)?)
        }
        TraceCommand::Compact(command) => {
            let payload = parse_json_input::<TraceCompactRequestPayload>(
                &command.input_json,
                "trace compact",
            )?;
            print_json_value(&compact_trace_stream(payload)?)
        }
        TraceCommand::WriteCompactionDelta(command) => {
            let payload = parse_json_input::<TraceCompactionDeltaWriteRequestPayload>(
                &command.input_json,
                "trace compaction delta write",
            )?;
            print_json_value(&write_trace_compaction_delta(payload)?)
        }
        TraceCommand::WriteMetadata(command) => {
            let payload = parse_json_input::<TraceMetadataWriteRequestPayload>(
                &command.input_json,
                "trace metadata write",
            )?;
            print_json_value(&write_trace_metadata(payload)?)
        }
    }
}

fn dispatch_storage_command(command: StorageCommand) -> Result<(), String> {
    match command {
        StorageCommand::Runtime(command) => {
            let payload = parse_json_input::<RuntimeStorageRequestPayload>(
                &command.input_json,
                "runtime storage",
            )?;
            print_json_value(&runtime_storage_operation(payload)?)
        }
        StorageCommand::CheckpointControlPlane(command) => {
            let payload =
                parse_json_input::<Value>(&command.input_json, "runtime checkpoint control plane")?;
            print_json_value(&build_checkpoint_control_plane_compiler_payload(payload)?)
        }
        StorageCommand::BackendCatalog => {
            print_json_value(&runtime_backend_family_catalog_payload())
        }
        StorageCommand::BackendParity(command) => {
            print_json_value(&runtime_backend_family_parity_payload(
                command.store.as_deref(),
                command.checkpointer.as_deref(),
                command.trace.as_deref(),
                command.state.as_deref(),
            )?)
        }
    }
}

fn dispatch_browser_command(command: BrowserCommand) -> Result<(), String> {
    match command {
        BrowserCommand::McpStdio(command) => run_browser_mcp_stdio_loop(
            command.repo_root.as_deref(),
            BrowserAttachConfig::from_cli_and_env(
                command.runtime_attach_descriptor_path,
                command.runtime_attach_artifact_path,
                command.headless,
            ),
        ),
        BrowserCommand::ResolveAttachArtifact(command) => {
            let repo_root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            let Some(path) =
                resolve_browser_mcp_attach_artifact(&repo_root, command.search_root.as_deref())
            else {
                return Err("no browser-mcp runtime attach artifact candidates found".to_string());
            };
            println!("{path}");
            Ok(())
        }
    }
}

fn dispatch_profile_command(command: ProfileCommand) -> Result<(), String> {
    match command {
        ProfileCommand::Emit(command) => {
            let profile = load_framework_profile(&command.framework_profile)?;
            print_json_value(&build_profile_bundle(&profile)?)
        }
        ProfileCommand::Artifacts(command) => {
            let profile = load_framework_profile(&command.framework_profile)?;
            print_json_value(&build_codex_artifact_bundle(&profile, command.full)?)
        }
    }
}

fn dispatch_migrate_command(command: MigrateCommand) -> Result<(), String> {
    match command {
        MigrateCommand::ArchivedMemory(command) => {
            let repo_root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            let memory_root = command
                .memory_root
                .unwrap_or_else(|| repo_root.join(".codex").join("memory"));
            print_json_value(&framework_runtime::archive_pre_cutover_memory_surfaces(
                &memory_root,
            )?)
        }
        MigrateCommand::CurrentArtifactClutter(command) => {
            let repo_root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            let payload = run_host_integration_from_args(&[
                "migrate-current-artifact-clutter".to_string(),
                "--repo-root".to_string(),
                repo_root.display().to_string(),
                "--active-task-id".to_string(),
                command.active_task_id,
            ])?;
            print_json_value(&payload)
        }
    }
}

fn dispatch_hook_policy_command(command: HookPolicyCommand) -> Result<(), String> {
    match command {
        HookPolicyCommand::Evaluate(command) => {
            let payload = parse_json_input::<HookPolicyEvaluateRequest>(
                &command.input_json,
                "hook policy evaluate",
            )?;
            print_json_value(&evaluate_hook_policy(payload)?)
        }
        HookPolicyCommand::Contract => print_json_value(&hook_policy_contract()),
    }
}

fn print_search_results(query: &str, payload: &SearchResultsPayload, rows: Vec<MatchRow>) {
    if payload.matches.is_empty() {
        println!("No skills found matching: {}", query);
        return;
    }

    println!("Found {} matches for '{}':", payload.matches.len(), query);
    println!();
    println!(
        "{:<30} | {:<5} | {:<10} | {:<6} | Description",
        "Skill", "Layer", "Gate", "Score"
    );
    println!("{}", "-".repeat(120));
    for row in rows {
        let mut description = row.description.clone();
        if description.chars().count() > 60 {
            description = description.chars().take(57).collect::<String>() + "...";
        }
        println!(
            "{:<30} | {:<5} | {:<10} | {:<6.2} | {}",
            row.slug, row.layer, row.gate, row.score, description
        );
    }
}

fn background_effect_plan(next_step: &str) -> BackgroundControlEffectPlanPayload {
    BackgroundControlEffectPlanPayload {
        next_step: next_step.to_string(),
        terminal_status: None,
        resolved_status: None,
        finalize_immediately: None,
        cancel_running_task: None,
        next_retry_count: None,
        backoff_seconds: None,
        wait_timeout_seconds: None,
        wait_poll_interval_seconds: None,
        sleep_seconds: None,
    }
}

fn default_trace_metadata_schema_version() -> String {
    "trace-metadata-v2".to_string()
}

fn default_true() -> bool {
    true
}

fn default_trace_framework_version() -> String {
    "phase1".to_string()
}

fn timestamp_now() -> String {
    Utc::now().to_rfc3339()
}

fn append_io_lock() -> &'static Mutex<()> {
    static APPEND_IO_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    APPEND_IO_LOCK.get_or_init(|| Mutex::new(()))
}

fn append_text_with_process_lock(path: &Path, payload: &str, context: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "create {context} parent failed for {}: {err}",
                parent.display()
            )
        })?;
    }
    let _guard = append_io_lock()
        .lock()
        .map_err(|_| format!("{context} append lock poisoned"))?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| format!("open {context} append failed for {}: {err}", path.display()))?;
    file.write_all(payload.as_bytes()).map_err(|err| {
        format!(
            "write {context} append failed for {}: {err}",
            path.display()
        )
    })
}

fn default_trace_runtime_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("skills")
        .join("SKILL_ROUTING_RUNTIME.json")
}

fn load_trace_routing_runtime_version(runtime_path: Option<&str>) -> u64 {
    let resolved_path = runtime_path
        .map(PathBuf::from)
        .unwrap_or_else(default_trace_runtime_path);
    let raw = match fs::read_to_string(&resolved_path) {
        Ok(value) => value,
        Err(_) => return 1,
    };
    match serde_json::from_str::<Value>(&raw) {
        Ok(Value::Object(payload)) => payload.get("version").and_then(Value::as_u64).unwrap_or(1),
        _ => 1,
    }
}

fn trace_source_explicitly_provided(payload: &TraceMetadataWriteRequestPayload) -> bool {
    payload.event_stream_path.is_some()
        || payload.event_stream_text.is_some()
        || payload.compaction_manifest_path.is_some()
        || payload.compaction_manifest_text.is_some()
}

fn write_trace_metadata(
    payload: TraceMetadataWriteRequestPayload,
) -> Result<TraceMetadataWriteResponsePayload, String> {
    let routing_runtime_version = payload
        .routing_runtime_version
        .unwrap_or_else(|| load_trace_routing_runtime_version(payload.runtime_path.as_deref()));
    let metadata_schema_version = payload
        .metadata_schema_version
        .as_deref()
        .map(str::to_string)
        .unwrap_or_else(default_trace_metadata_schema_version);
    let framework_version = payload
        .framework_version
        .as_deref()
        .map(str::to_string)
        .unwrap_or_else(default_trace_framework_version);
    let timestamp = payload.ts.clone().unwrap_or_else(timestamp_now);
    let should_resolve_trace = payload.events.is_none()
        || payload.stream.is_none()
        || payload.reroute_count.is_none()
        || payload.retry_count.is_none();
    let resolved_trace = if trace_source_explicitly_provided(&payload) {
        Some(resolve_trace_source(
            TraceSourceRequest::from_metadata_payload(&payload),
        )?)
    } else if should_resolve_trace {
        resolve_trace_source(TraceSourceRequest::from_metadata_payload(&payload)).ok()
    } else {
        None
    };
    let resolved_events = payload.events.clone().unwrap_or_else(|| {
        resolved_trace
            .as_ref()
            .map(|trace| {
                trace
                    .events
                    .iter()
                    .cloned()
                    .map(Value::Object)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });
    let reroute_count = payload.reroute_count.unwrap_or_else(|| {
        resolved_trace
            .as_ref()
            .map(|trace| trace_reroute_count(&trace.events))
            .unwrap_or(0)
    });
    let retry_count = payload.retry_count.unwrap_or_else(|| {
        resolved_trace
            .as_ref()
            .map(|trace| trace_retry_count(&trace.events))
            .unwrap_or(0)
    });
    let resolved_stream = payload.stream.clone().or_else(|| {
        resolved_trace
            .as_ref()
            .map(|trace| build_trace_stream_metadata(trace, payload.control_plane.as_ref()))
    });

    let mut document = Map::new();
    document.insert("version".to_string(), json!(1));
    document.insert(
        "schema_version".to_string(),
        json!(metadata_schema_version.clone()),
    );
    document.insert(
        "metadata_schema_version".to_string(),
        json!(metadata_schema_version),
    );
    document.insert("ts".to_string(), json!(timestamp));
    document.insert("task".to_string(), json!(payload.task));
    document.insert("framework_version".to_string(), json!(framework_version));
    document.insert(
        "routing_runtime_version".to_string(),
        json!(routing_runtime_version),
    );
    document.insert("matched_skills".to_string(), json!(payload.matched_skills));
    document.insert(
        "decision".to_string(),
        json!({
            "owner": payload.owner,
            "gate": payload.gate,
            "overlay": payload.overlay,
        }),
    );
    document.insert("reroute_count".to_string(), json!(reroute_count));
    document.insert("retry_count".to_string(), json!(retry_count));
    document.insert("artifact_paths".to_string(), json!(payload.artifact_paths));
    document.insert(
        "verification_status".to_string(),
        json!(payload.verification_status),
    );
    if let Some(value) = payload.trace_event_schema_version {
        document.insert("trace_event_schema_version".to_string(), json!(value));
    }
    if let Some(value) = payload.trace_event_sink_schema_version {
        document.insert("trace_event_sink_schema_version".to_string(), json!(value));
    }
    if let Some(value) = payload.parallel_group {
        document.insert("parallel_group".to_string(), value);
    }
    if let Some(value) = payload.supervisor_projection {
        document.insert("supervisor_projection".to_string(), value);
    }
    if let Some(value) = payload.control_plane {
        document.insert("control_plane".to_string(), value);
    }
    if let Some(value) = resolved_stream {
        document.insert("stream".to_string(), value);
    }
    if !resolved_events.is_empty() {
        document.insert("events".to_string(), Value::Array(resolved_events));
    }

    let serialized = serde_json::to_string_pretty(&Value::Object(document))
        .map_err(|err| format!("serialize trace metadata failed: {err}"))?
        + "\n";
    if payload.write_outputs {
        let outputs =
            std::iter::once(payload.output_path.clone()).chain(payload.mirror_paths.clone());
        for output in outputs {
            let path = PathBuf::from(&output);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|err| {
                    format!(
                        "create trace metadata parent failed for {}: {err}",
                        parent.display()
                    )
                })?;
            }
            fs::write(&path, &serialized).map_err(|err| {
                format!("write trace metadata failed for {}: {err}", path.display())
            })?;
        }
    }

    Ok(TraceMetadataWriteResponsePayload {
        schema_version: TRACE_METADATA_WRITE_SCHEMA_VERSION.to_string(),
        authority: TRACE_METADATA_WRITE_AUTHORITY.to_string(),
        output_path: payload.output_path,
        mirror_paths: payload.mirror_paths,
        bytes_written: serialized.len(),
        routing_runtime_version,
        payload_text: serialized,
    })
}

fn main() -> Result<(), String> {
    let args = Cli::parse();
    configure_compute_parallelism(args.compute_threads)?;
    if let Some(command) = args.command.clone() {
        return dispatch_router_command(command);
    }

    if args.stdio_json {
        return run_stdio_json_loop(args.stdio_max_concurrency);
    }

    Err("missing router-rs command; use `router-rs --help` for canonical subcommands".to_string())
}

pub(crate) fn dispatch_stdio_json_request_payload(
    request: StdioJsonRequestPayload,
) -> StdioJsonResponsePayload {
    match dispatch_stdio_json_request(&request.op, request.payload) {
        Ok(payload) => StdioJsonResponsePayload {
            id: request.id,
            ok: true,
            payload: Some(payload),
            error: None,
        },
        Err(error) => StdioJsonResponsePayload {
            id: request.id,
            ok: false,
            payload: None,
            error: Some(error),
        },
    }
}

fn dispatch_stdio_json_request(op: &str, payload: Value) -> Result<Value, String> {
    if handles_runtime_output_stdio_op(op) {
        let Some(result) = dispatch_runtime_output_mode_stdio(op, payload) else {
            return Err(format!("runtime output mode dispatch drifted for {op}"));
        };
        return result;
    }
    match op {
        "route" => dispatch_stdio_route(payload),
        "search_skills" => dispatch_stdio_search_skills(payload),
        "hook_policy" => evaluate_hook_policy_value(payload),
        "concurrency_defaults" => serde_json::to_value(runtime_concurrency_defaults_payload())
            .map_err(|err| format!("serialize concurrency defaults output failed: {err}")),
        "execute" => {
            let request = serde_json::from_value::<ExecuteRequestPayload>(payload)
                .map_err(|err| format!("parse execute input failed: {err}"))?;
            serde_json::to_value(execute_request(request)?)
                .map_err(|err| format!("serialize execute output failed: {err}"))
        }
        "execution_contract_bundle" => Ok(Value::Object(build_execution_contract_bundle())),
        "normalize_execution_kernel_metadata_contract" => {
            if payload.as_object().is_some_and(Map::is_empty) || payload.is_null() {
                normalize_execution_kernel_metadata_contract_value(None)
            } else {
                normalize_execution_kernel_metadata_contract_value(Some(&payload))
            }
        }
        "normalize_execution_kernel_contract" => {
            let kernel_contract = payload.get("kernel_contract").ok_or_else(|| {
                "execution-kernel contract payload is missing kernel_contract.".to_string()
            })?;
            let response_shape = payload.get("response_shape").and_then(Value::as_str);
            normalize_execution_kernel_contract_value(kernel_contract, response_shape)
        }
        "validate_execution_kernel_steady_state_metadata" => {
            let metadata = payload.get("metadata").ok_or_else(|| {
                "execution-kernel validation payload is missing metadata.".to_string()
            })?;
            let kernel_contract = payload.get("kernel_contract");
            let response_shape = payload.get("response_shape").and_then(Value::as_str);
            validate_execution_kernel_steady_state_metadata_value(
                metadata,
                kernel_contract,
                response_shape,
            )
        }
        "decode_execution_response" => {
            let execution_payload = payload.get("payload").ok_or_else(|| {
                "execution response decode payload is missing payload.".to_string()
            })?;
            let kernel_contract = payload.get("kernel_contract");
            let dry_run = payload.get("dry_run").and_then(Value::as_bool);
            decode_execution_response_value(execution_payload, kernel_contract, dry_run)
        }
        "route_report" => dispatch_stdio_route_report(payload),
        "route_resolution" => dispatch_stdio_route_resolution(payload),
        "route_policy" => dispatch_stdio_route_policy(payload),
        "route_snapshot" => dispatch_stdio_route_snapshot(payload),
        "compile_profile_bundle" => dispatch_stdio_compile_profile_bundle(payload),
        "compile_codex_profile_artifacts" => {
            dispatch_stdio_compile_codex_profile_artifacts(payload)
        }
        "sandbox_control" => {
            let request = serde_json::from_value::<SandboxControlRequestPayload>(payload)
                .map_err(|err| format!("parse sandbox control input failed: {err}"))?;
            serde_json::to_value(build_sandbox_control_response(request)?)
                .map_err(|err| format!("serialize sandbox control output failed: {err}"))
        }
        "runtime_observability_health_snapshot" => {
            serde_json::to_value(build_runtime_observability_health_snapshot()).map_err(|err| {
                format!("serialize runtime observability health snapshot output failed: {err}")
            })
        }
        "background_control" => {
            let request = serde_json::from_value::<BackgroundControlRequestPayload>(payload)
                .map_err(|err| format!("parse background control input failed: {err}"))?;
            serde_json::to_value(build_background_control_response(request)?)
                .map_err(|err| format!("serialize background control output failed: {err}"))
        }
        "background_state" => handle_background_state_operation(payload),
        "session_supervisor" => handle_session_supervisor_operation(payload),
        "describe_transport" => build_trace_transport_descriptor(payload),
        "describe_handoff" => build_trace_handoff_descriptor(payload),
        "checkpoint_resume_manifest" => build_checkpoint_resume_manifest(payload),
        "runtime_checkpoint_control_plane" => {
            build_checkpoint_control_plane_compiler_payload(payload)
        }
        "write_transport_binding" => write_transport_binding_payload(payload),
        "write_checkpoint_resume_manifest" => write_checkpoint_resume_manifest_payload(payload),
        "attach_runtime_event_transport" => attach_runtime_event_transport(payload),
        "subscribe_attached_runtime_events" => subscribe_attached_runtime_events(payload),
        "cleanup_attached_runtime_event_transport" => {
            cleanup_attached_runtime_event_transport(payload)
        }
        "runtime_storage" => {
            let request = serde_json::from_value::<RuntimeStorageRequestPayload>(payload)
                .map_err(|err| format!("parse runtime storage input failed: {err}"))?;
            serde_json::to_value(runtime_storage_operation(request)?)
                .map_err(|err| format!("serialize runtime storage output failed: {err}"))
        }
        "trace_record_event" => {
            let request = serde_json::from_value(payload)
                .map_err(|err| format!("parse trace record event input failed: {err}"))?;
            serde_json::to_value(record_trace_event(request)?)
                .map_err(|err| format!("serialize trace record event output failed: {err}"))
        }
        "trace_stream_replay" => {
            let request = serde_json::from_value::<TraceStreamReplayRequestPayload>(payload)
                .map_err(|err| format!("parse trace stream replay input failed: {err}"))?;
            serde_json::to_value(replay_trace_stream(request)?)
                .map_err(|err| format!("serialize trace stream replay output failed: {err}"))
        }
        "trace_stream_inspect" => {
            let request = serde_json::from_value::<TraceStreamInspectRequestPayload>(payload)
                .map_err(|err| format!("parse trace stream inspect input failed: {err}"))?;
            serde_json::to_value(inspect_trace_stream(request)?)
                .map_err(|err| format!("serialize trace stream inspect output failed: {err}"))
        }
        "trace_compact" => {
            let request = serde_json::from_value(payload)
                .map_err(|err| format!("parse trace compact input failed: {err}"))?;
            serde_json::to_value(compact_trace_stream(request)?)
                .map_err(|err| format!("serialize trace compact output failed: {err}"))
        }
        "write_trace_compaction_delta" => {
            let request =
                serde_json::from_value::<TraceCompactionDeltaWriteRequestPayload>(payload)
                    .map_err(|err| format!("parse trace compaction delta input failed: {err}"))?;
            serde_json::to_value(write_trace_compaction_delta(request)?)
                .map_err(|err| format!("serialize trace compaction delta output failed: {err}"))
        }
        "write_trace_metadata" => {
            let request = serde_json::from_value::<TraceMetadataWriteRequestPayload>(payload)
                .map_err(|err| format!("parse trace metadata write input failed: {err}"))?;
            serde_json::to_value(write_trace_metadata(request)?)
                .map_err(|err| format!("serialize trace metadata output failed: {err}"))
        }
        "framework_runtime_snapshot" => dispatch_stdio_framework_runtime_snapshot(payload),
        "framework_contract_summary" => dispatch_stdio_framework_contract_summary(payload),
        "framework_memory_recall" => dispatch_stdio_framework_memory_recall(payload),
        "framework_memory_policy" => build_framework_memory_policy_envelope(payload),
        "framework_prompt_compression" => build_framework_prompt_compression_envelope(payload),
        "framework_session_artifact_write" => write_framework_session_artifacts(payload),
        "framework_alias" => dispatch_stdio_framework_alias(payload),
        "control_plane_contracts" => {
            serde_json::to_value(build_control_plane_contract_descriptors())
                .map_err(|err| format!("serialize control plane contracts output failed: {err}"))
        }
        _ => Err(format!("unsupported stdio operation: {op}")),
    }
}

fn dispatch_stdio_route(payload: Value) -> Result<Value, String> {
    let query = required_non_empty_string(&payload, "query", "stdio route")?;
    let session_id = optional_non_empty_string(&payload, "session_id")
        .unwrap_or_else(|| "route-cli".to_string());
    let allow_overlay = payload
        .get("allow_overlay")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let first_turn = payload
        .get("first_turn")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let owned_inline_records = if payload.get("skills").is_some() {
        Some(load_inline_records(&payload)?)
    } else {
        None
    };
    let runtime_path = optional_non_empty_string(&payload, "runtime_path").map(PathBuf::from);
    let manifest_path = optional_non_empty_string(&payload, "manifest_path").map(PathBuf::from);
    let cached_records;
    let using_inline_records = owned_inline_records.is_some();
    let records: &[SkillRecord] = if let Some(items) = owned_inline_records.as_ref() {
        items.as_slice()
    } else {
        cached_records =
            load_records_cached_for_stdio(runtime_path.as_deref(), manifest_path.as_deref())?;
        cached_records.as_ref()
    };
    let decision = if using_inline_records {
        route_task(records, &query, &session_id, allow_overlay, first_turn)?
    } else {
        route_task_with_manifest_fallback(
            records,
            runtime_path.as_deref(),
            manifest_path.as_deref(),
            &query,
            &session_id,
            allow_overlay,
            first_turn,
        )?
    };
    serde_json::to_value(decision).map_err(|err| format!("serialize route output failed: {err}"))
}

fn dispatch_stdio_search_skills(payload: Value) -> Result<Value, String> {
    let query = required_non_empty_string(&payload, "query", "stdio search_skills")?;
    let limit = payload
        .get("limit")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(5);
    let runtime_path = optional_non_empty_string(&payload, "runtime_path").map(PathBuf::from);
    let manifest_path = optional_non_empty_string(&payload, "manifest_path").map(PathBuf::from);
    let manifest_fallback =
        manifest_fallback_path(runtime_path.as_deref(), manifest_path.as_deref())?;
    let owned_records;
    let cached_records;
    let records: &[SkillRecord] = if let Some(path) = manifest_fallback.as_deref() {
        owned_records = load_records_from_manifest(path)?;
        owned_records.as_slice()
    } else {
        cached_records =
            load_records_cached_for_stdio(runtime_path.as_deref(), manifest_path.as_deref())?;
        cached_records.as_ref()
    };
    let matches = search_skills(records, &query, limit);
    let resolved = build_search_results_payload(&query, matches);
    serde_json::to_value(resolved).map_err(|err| format!("serialize search output failed: {err}"))
}

fn dispatch_stdio_route_report(payload: Value) -> Result<Value, String> {
    let mode = required_non_empty_string(&payload, "mode", "stdio route report")?;
    let route_decision = payload
        .get("route_decision")
        .cloned()
        .filter(|value| !value.is_null())
        .map(serde_json::from_value::<RouteDecision>)
        .transpose()
        .map_err(|err| format!("parse route decision contract failed: {err}"))?;
    let rust_snapshot = match payload.get("rust_route_snapshot").cloned() {
        Some(raw) if !raw.is_null() => serde_json::from_value::<RouteDecisionSnapshotPayload>(raw)
            .map_err(|err| format!("parse rust route snapshot failed: {err}"))?,
        _ => route_decision
            .as_ref()
            .map(|decision| decision.route_snapshot.clone())
            .ok_or_else(|| {
                "route_report requires rust_route_snapshot or route_decision".to_string()
            })?,
    };
    serde_json::to_value(build_route_diff_report(
        &mode,
        rust_snapshot,
        route_decision.as_ref(),
    )?)
    .map_err(|err| format!("serialize route report output failed: {err}"))
}

fn dispatch_stdio_route_resolution(payload: Value) -> Result<Value, String> {
    let mode = required_non_empty_string(&payload, "mode", "stdio route resolution")?;
    let decision_value = payload
        .get("route_decision")
        .cloned()
        .ok_or_else(|| "route_resolution requires route_decision".to_string())?;
    let decision = serde_json::from_value::<RouteDecision>(decision_value)
        .map_err(|err| format!("parse route resolution input failed: {err}"))?;
    serde_json::to_value(build_route_resolution(&mode, &decision)?)
        .map_err(|err| format!("serialize route resolution output failed: {err}"))
}

fn dispatch_stdio_route_policy(payload: Value) -> Result<Value, String> {
    let mode = required_non_empty_string(&payload, "mode", "stdio route policy")?;
    serde_json::to_value(build_route_policy(&mode)?)
        .map_err(|err| format!("serialize route policy output failed: {err}"))
}

fn dispatch_stdio_route_snapshot(payload: Value) -> Result<Value, String> {
    let request = serde_json::from_value::<RouteSnapshotRequestPayload>(payload)
        .map_err(|err| format!("parse route snapshot input failed: {err}"))?;
    let snapshot = build_route_snapshot(
        &request.engine,
        &request.selected_skill,
        request.overlay_skill.as_deref(),
        &request.layer,
        request.score,
        &request.reasons,
    );
    serde_json::to_value(RouteSnapshotEnvelopePayload {
        snapshot_schema_version: ROUTE_SNAPSHOT_SCHEMA_VERSION.to_string(),
        authority: ROUTE_AUTHORITY.to_string(),
        route_snapshot: snapshot,
    })
    .map_err(|err| format!("serialize route snapshot output failed: {err}"))
}

fn dispatch_stdio_framework_runtime_snapshot(payload: Value) -> Result<Value, String> {
    let repo_root =
        required_non_empty_string(&payload, "repo_root", "stdio framework runtime snapshot")?;
    let artifact_root = optional_non_empty_string(&payload, "artifact_source_dir");
    let task_id = optional_non_empty_string(&payload, "task_id");
    serde_json::to_value(build_framework_runtime_snapshot_envelope(
        Path::new(&repo_root),
        artifact_root.as_deref().map(Path::new),
        task_id.as_deref(),
    )?)
    .map_err(|err| format!("serialize framework runtime snapshot output failed: {err}"))
}

fn dispatch_stdio_framework_contract_summary(payload: Value) -> Result<Value, String> {
    let repo_root =
        required_non_empty_string(&payload, "repo_root", "stdio framework contract summary")?;
    serde_json::to_value(build_framework_contract_summary_envelope(Path::new(
        &repo_root,
    ))?)
    .map_err(|err| format!("serialize framework contract summary output failed: {err}"))
}

fn dispatch_stdio_framework_memory_recall(payload: Value) -> Result<Value, String> {
    let repo_root =
        required_non_empty_string(&payload, "repo_root", "stdio framework memory recall")?;
    let query = payload.get("query").and_then(Value::as_str).unwrap_or("");
    let max_items = payload
        .get("top")
        .or_else(|| payload.get("max_items"))
        .or_else(|| payload.get("limit"))
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(8);
    let mode = payload
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("stable");
    let memory_root = payload
        .get("memory_root")
        .and_then(Value::as_str)
        .map(Path::new);
    let artifact_source_dir = payload
        .get("artifact_source_dir")
        .and_then(Value::as_str)
        .map(Path::new);
    let task_id = payload.get("task_id").and_then(Value::as_str);
    serde_json::to_value(build_framework_memory_recall_envelope(
        Path::new(&repo_root),
        query,
        max_items,
        mode,
        memory_root,
        artifact_source_dir,
        task_id,
    )?)
    .map_err(|err| format!("serialize framework memory recall output failed: {err}"))
}

fn dispatch_stdio_framework_alias(payload: Value) -> Result<Value, String> {
    let repo_root = required_non_empty_string(&payload, "repo_root", "stdio framework alias")?;
    let alias_name = required_non_empty_string(&payload, "alias", "stdio framework alias")?;
    let max_lines = payload
        .get("max_lines")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(4);
    let compact = payload
        .get("compact")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let host_id = payload.get("host_id").and_then(Value::as_str);
    serde_json::to_value(build_framework_alias_envelope(
        Path::new(&repo_root),
        &alias_name,
        FrameworkAliasBuildOptions {
            max_lines,
            compact,
            host_id,
        },
    )?)
    .map_err(|err| format!("serialize framework alias output failed: {err}"))
}

fn dispatch_stdio_compile_profile_bundle(payload: Value) -> Result<Value, String> {
    let profile_path = required_non_empty_string(&payload, "profile_path", "stdio profile bundle")?;
    let profile = load_framework_profile(Path::new(&profile_path))?;
    let bundle = build_profile_bundle(&profile)?;
    serde_json::to_value(bundle)
        .map_err(|err| format!("serialize profile bundle output failed: {err}"))
}

fn dispatch_stdio_compile_codex_profile_artifacts(payload: Value) -> Result<Value, String> {
    let profile_path =
        required_non_empty_string(&payload, "profile_path", "stdio codex profile artifacts")?;
    let full = optional_bool(&payload, "full").unwrap_or(false);
    let profile = load_framework_profile(Path::new(&profile_path))?;
    let artifacts = build_codex_artifact_bundle(&profile, full)?;
    serde_json::to_value(artifacts)
        .map_err(|err| format!("serialize codex profile artifacts output failed: {err}"))
}

fn configure_compute_parallelism(override_value: Option<usize>) -> Result<(), String> {
    let Some(thread_count) = override_value
        .or_else(|| env_usize("ROUTER_RS_COMPUTE_THREADS"))
        .filter(|value| *value > 0)
        .map(|value| value.clamp(1, MAX_COMPUTE_THREADS))
    else {
        return Ok(());
    };
    ThreadPoolBuilder::new()
        .num_threads(thread_count)
        .build_global()
        .map_err(|err| format!("configure compute thread pool failed: {err}"))
}

fn env_usize(name: &str) -> Option<usize> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
}

fn execute_request(payload: ExecuteRequestPayload) -> Result<ExecuteResponsePayload, String> {
    let prompt_preview = payload
        .prompt_preview
        .clone()
        .filter(|value| !value.trim().is_empty());
    if payload.dry_run {
        let dry_run_prompt_preview =
            Some(prompt_preview.unwrap_or_else(|| build_live_execute_prompt(&payload)));
        return Ok(build_dry_run_execute_response(
            &payload,
            dry_run_prompt_preview,
        ));
    }
    let live_prompt_preview = build_live_execute_prompt(&payload);
    if payload.aggregator_base_url.trim().is_empty() {
        return Err("router-rs execute requires a non-empty aggregator_base_url".to_string());
    }
    if payload.aggregator_api_key.trim().is_empty() {
        return Err("router-rs execute requires a non-empty aggregator_api_key".to_string());
    }
    let live_result = perform_live_execute(&payload, &live_prompt_preview)?;
    Ok(build_live_execute_response(
        &payload,
        Some(live_prompt_preview),
        live_result,
    ))
}

fn normalize_multitask_strategy(strategy: Option<&str>) -> String {
    strategy.unwrap_or("reject").trim().to_lowercase()
}

fn compute_backoff_seconds(
    base: f64,
    multiplier: f64,
    retry_count: usize,
    maximum: Option<f64>,
) -> f64 {
    if retry_count == 0 || base <= 0.0 {
        return 0.0;
    }
    let normalized_multiplier = if multiplier > 0.0 { multiplier } else { 1.0 };
    let mut delay = base * normalized_multiplier.powi((retry_count.saturating_sub(1)) as i32);
    if let Some(maximum) = maximum {
        delay = delay.min(maximum);
    }
    delay
}

fn next_background_parallel_group_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("pgroup_{nanos:x}")
}

fn build_background_control_response(
    payload: BackgroundControlRequestPayload,
) -> Result<BackgroundControlResponsePayload, String> {
    let supported_multitask_strategies = vec!["interrupt".to_string(), "reject".to_string()];
    match payload.operation.as_str() {
        "batch-plan" => {
            let batch_size = payload.batch_size.unwrap_or(0);
            if batch_size == 0 {
                let mut effect_plan = background_effect_plan("reject");
                effect_plan.terminal_status = Some("failed".to_string());
                return Ok(BackgroundControlResponsePayload {
                    schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                    authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                    operation: payload.operation,
                    resolved_parallel_group_id: None,
                    lane_ids: None,
                    normalized_multitask_strategy: None,
                    supported_multitask_strategies,
                    strategy_supported: true,
                    accepted: Some(false),
                    requires_takeover: Some(false),
                    error: Some(
                        "enqueue_background_batch requires at least one request.".to_string(),
                    ),
                    should_retry: None,
                    next_retry_count: None,
                    backoff_seconds: None,
                    terminal_status: Some("failed".to_string()),
                    resolved_status: None,
                    finalize_immediately: Some(true),
                    cancel_running_task: Some(false),
                    reason: "batch-plan-empty".to_string(),
                    effect_plan,
                });
            }

            let requested_group_id = payload
                .requested_parallel_group_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string());
            let mut request_group_ids = HashSet::new();
            if let Some(values) = payload.request_parallel_group_ids.as_ref() {
                for value in values {
                    if let Some(group_id) = value
                        .as_deref()
                        .map(str::trim)
                        .filter(|candidate| !candidate.is_empty())
                    {
                        request_group_ids.insert(group_id.to_string());
                    }
                }
            }
            if request_group_ids.len() > 1 {
                let mut effect_plan = background_effect_plan("reject");
                effect_plan.terminal_status = Some("failed".to_string());
                return Ok(BackgroundControlResponsePayload {
                    schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                    authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                    operation: payload.operation,
                    resolved_parallel_group_id: None,
                    lane_ids: None,
                    normalized_multitask_strategy: None,
                    supported_multitask_strategies,
                    strategy_supported: true,
                    accepted: Some(false),
                    requires_takeover: Some(false),
                    error: Some(
                        "enqueue_background_batch requires one consistent parallel_group_id across the whole batch."
                            .to_string(),
                    ),
                    should_retry: None,
                    next_retry_count: None,
                    backoff_seconds: None,
                    terminal_status: Some("failed".to_string()),
                    resolved_status: None,
                    finalize_immediately: Some(true),
                    cancel_running_task: Some(false),
                    reason: "batch-plan-misaligned-parallel-group".to_string(),
                    effect_plan,
                });
            }
            if let Some(requested) = requested_group_id.as_ref() {
                if let Some(existing) = request_group_ids.iter().next() {
                    if existing != requested {
                        let mut effect_plan = background_effect_plan("reject");
                        effect_plan.terminal_status = Some("failed".to_string());
                        return Ok(BackgroundControlResponsePayload {
                            schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                            authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                            operation: payload.operation,
                            resolved_parallel_group_id: None,
                            lane_ids: None,
                            normalized_multitask_strategy: None,
                            supported_multitask_strategies,
                            strategy_supported: true,
                            accepted: Some(false),
                            requires_takeover: Some(false),
                            error: Some(
                                "enqueue_background_batch requires one consistent parallel_group_id across the whole batch."
                                    .to_string(),
                            ),
                            should_retry: None,
                            next_retry_count: None,
                            backoff_seconds: None,
                            terminal_status: Some("failed".to_string()),
                            resolved_status: None,
                            finalize_immediately: Some(true),
                            cancel_running_task: Some(false),
                            reason: "batch-plan-misaligned-parallel-group".to_string(),
                            effect_plan,
                        });
                    }
                }
            }

            let resolved_parallel_group_id = requested_group_id
                .or_else(|| request_group_ids.into_iter().next())
                .unwrap_or_else(next_background_parallel_group_id);
            let lane_id_prefix = payload
                .lane_id_prefix
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("lane");
            let lane_ids = (0..batch_size)
                .map(|index| {
                    payload
                        .request_lane_ids
                        .as_ref()
                        .and_then(|values| values.get(index))
                        .and_then(|value| value.as_deref())
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| format!("{lane_id_prefix}-{}", index + 1))
                })
                .collect::<Vec<_>>();
            let effect_plan = background_effect_plan("plan_batch");
            Ok(BackgroundControlResponsePayload {
                schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                operation: payload.operation,
                resolved_parallel_group_id: Some(resolved_parallel_group_id),
                lane_ids: Some(lane_ids),
                normalized_multitask_strategy: None,
                supported_multitask_strategies,
                strategy_supported: true,
                accepted: Some(true),
                requires_takeover: Some(false),
                error: None,
                should_retry: None,
                next_retry_count: None,
                backoff_seconds: None,
                terminal_status: None,
                resolved_status: None,
                finalize_immediately: Some(false),
                cancel_running_task: Some(false),
                reason: "batch-plan-resolved".to_string(),
                effect_plan,
            })
        }
        "enqueue" => {
            let normalized_multitask_strategy =
                normalize_multitask_strategy(payload.multitask_strategy.as_deref());
            let strategy_supported = supported_multitask_strategies
                .iter()
                .any(|strategy| strategy == &normalized_multitask_strategy);
            if !strategy_supported {
                let mut effect_plan = background_effect_plan("reject");
                effect_plan.terminal_status = Some("failed".to_string());
                return Ok(BackgroundControlResponsePayload {
                    schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                    authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                    operation: payload.operation,
                    resolved_parallel_group_id: None,
                    lane_ids: None,
                    normalized_multitask_strategy: Some(normalized_multitask_strategy),
                    supported_multitask_strategies,
                    strategy_supported: false,
                    accepted: Some(false),
                    requires_takeover: Some(false),
                    error: Some(format!(
                        "Unsupported multitask strategy: {}. Supported strategies: interrupt, reject",
                        payload.multitask_strategy.as_deref().unwrap_or("reject")
                    )),
                    should_retry: None,
                    next_retry_count: None,
                    backoff_seconds: None,
                    terminal_status: None,
                    resolved_status: None,
                    finalize_immediately: None,
                    cancel_running_task: None,
                    reason: "invalid-multitask-strategy".to_string(),
                    effect_plan,
                });
            }
            let active_job_count = payload.active_job_count.unwrap_or(0);
            let capacity_limit = payload.capacity_limit.unwrap_or(0);
            if active_job_count >= capacity_limit {
                let mut effect_plan = background_effect_plan("reject");
                effect_plan.terminal_status = Some("failed".to_string());
                return Ok(BackgroundControlResponsePayload {
                    schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                    authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                    operation: payload.operation,
                    resolved_parallel_group_id: None,
                    lane_ids: None,
                    normalized_multitask_strategy: Some(normalized_multitask_strategy.clone()),
                    supported_multitask_strategies,
                    strategy_supported: true,
                    accepted: Some(false),
                    requires_takeover: Some(normalized_multitask_strategy == "interrupt"),
                    error: Some(format!(
                        "Too many admitted background jobs ({}/{})",
                        active_job_count, capacity_limit
                    )),
                    should_retry: None,
                    next_retry_count: None,
                    backoff_seconds: None,
                    terminal_status: None,
                    resolved_status: None,
                    finalize_immediately: None,
                    cancel_running_task: None,
                    reason: "capacity-rejected".to_string(),
                    effect_plan,
                });
            }
            let effect_plan = background_effect_plan("admit");
            Ok(BackgroundControlResponsePayload {
                schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                operation: payload.operation,
                resolved_parallel_group_id: None,
                lane_ids: None,
                normalized_multitask_strategy: Some(normalized_multitask_strategy.clone()),
                supported_multitask_strategies,
                strategy_supported: true,
                accepted: Some(true),
                requires_takeover: Some(normalized_multitask_strategy == "interrupt"),
                error: None,
                should_retry: None,
                next_retry_count: None,
                backoff_seconds: None,
                terminal_status: None,
                resolved_status: None,
                finalize_immediately: None,
                cancel_running_task: None,
                reason: "accepted".to_string(),
                effect_plan,
            })
        }
        "interrupt" => {
            let current_status = payload
                .current_status
                .unwrap_or_else(|| "queued".to_string());
            let task_active = payload.task_active.unwrap_or(false);
            let task_done = payload.task_done.unwrap_or(false);
            let finalize_immediately =
                matches!(current_status.as_str(), "queued" | "retry_scheduled")
                    || !task_active
                    || task_done;
            let mut effect_plan = if finalize_immediately {
                background_effect_plan("finalize_interrupted")
            } else {
                background_effect_plan("request_interrupt")
            };
            effect_plan.finalize_immediately = Some(finalize_immediately);
            effect_plan.cancel_running_task =
                Some(!finalize_immediately && task_active && !task_done);
            effect_plan.resolved_status = Some("interrupt_requested".to_string());
            effect_plan.terminal_status = Some(if finalize_immediately {
                "interrupted".to_string()
            } else {
                "interrupt_requested".to_string()
            });
            Ok(BackgroundControlResponsePayload {
                schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                operation: payload.operation,
                resolved_parallel_group_id: None,
                lane_ids: None,
                normalized_multitask_strategy: None,
                supported_multitask_strategies,
                strategy_supported: true,
                accepted: None,
                requires_takeover: None,
                error: None,
                should_retry: None,
                next_retry_count: None,
                backoff_seconds: None,
                terminal_status: Some(if finalize_immediately {
                    "interrupted".to_string()
                } else {
                    "interrupt_requested".to_string()
                }),
                resolved_status: Some("interrupt_requested".to_string()),
                finalize_immediately: Some(finalize_immediately),
                cancel_running_task: Some(!finalize_immediately && task_active && !task_done),
                reason: if finalize_immediately {
                    "interrupt-finalized".to_string()
                } else {
                    "interrupt-cancel-running-task".to_string()
                },
                effect_plan,
            })
        }
        "claim" => {
            let current_status = payload
                .current_status
                .unwrap_or_else(|| "queued".to_string());
            if matches!(
                current_status.as_str(),
                "interrupt_requested" | "interrupted"
            ) {
                let mut effect_plan = background_effect_plan("finalize_interrupted");
                effect_plan.finalize_immediately = Some(true);
                effect_plan.terminal_status = Some("interrupted".to_string());
                effect_plan.resolved_status = Some("interrupted".to_string());
                return Ok(BackgroundControlResponsePayload {
                    schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                    authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                    operation: payload.operation,
                    resolved_parallel_group_id: None,
                    lane_ids: None,
                    normalized_multitask_strategy: None,
                    supported_multitask_strategies,
                    strategy_supported: true,
                    accepted: None,
                    requires_takeover: None,
                    error: None,
                    should_retry: None,
                    next_retry_count: None,
                    backoff_seconds: None,
                    terminal_status: Some("interrupted".to_string()),
                    resolved_status: Some("interrupted".to_string()),
                    finalize_immediately: Some(true),
                    cancel_running_task: Some(false),
                    reason: "claim-suppressed-interrupted".to_string(),
                    effect_plan,
                });
            }
            if matches!(
                current_status.as_str(),
                "completed" | "failed" | "retry_exhausted"
            ) {
                let mut effect_plan = background_effect_plan("finalize_terminal");
                effect_plan.finalize_immediately = Some(true);
                effect_plan.terminal_status = Some(current_status.clone());
                effect_plan.resolved_status = Some(current_status.clone());
                return Ok(BackgroundControlResponsePayload {
                    schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                    authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                    operation: payload.operation,
                    resolved_parallel_group_id: None,
                    lane_ids: None,
                    normalized_multitask_strategy: None,
                    supported_multitask_strategies,
                    strategy_supported: true,
                    accepted: None,
                    requires_takeover: None,
                    error: None,
                    should_retry: None,
                    next_retry_count: None,
                    backoff_seconds: None,
                    terminal_status: Some(current_status.clone()),
                    resolved_status: Some(current_status),
                    finalize_immediately: Some(true),
                    cancel_running_task: Some(false),
                    reason: "claim-suppressed-terminal".to_string(),
                    effect_plan,
                });
            }
            let mut effect_plan = background_effect_plan("claim_execution");
            effect_plan.finalize_immediately = Some(false);
            effect_plan.resolved_status = Some("running".to_string());
            Ok(BackgroundControlResponsePayload {
                schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                operation: payload.operation,
                resolved_parallel_group_id: None,
                lane_ids: None,
                normalized_multitask_strategy: None,
                supported_multitask_strategies,
                strategy_supported: true,
                accepted: None,
                requires_takeover: None,
                error: None,
                should_retry: None,
                next_retry_count: None,
                backoff_seconds: None,
                terminal_status: None,
                resolved_status: Some("running".to_string()),
                finalize_immediately: Some(false),
                cancel_running_task: Some(false),
                reason: "claim-running".to_string(),
                effect_plan,
            })
        }
        "complete" => {
            let mut effect_plan = background_effect_plan("finalize_completed");
            effect_plan.finalize_immediately = Some(true);
            effect_plan.terminal_status = Some("completed".to_string());
            effect_plan.resolved_status = Some("completed".to_string());
            Ok(BackgroundControlResponsePayload {
                schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                operation: payload.operation,
                resolved_parallel_group_id: None,
                lane_ids: None,
                normalized_multitask_strategy: None,
                supported_multitask_strategies,
                strategy_supported: true,
                accepted: None,
                requires_takeover: None,
                error: None,
                should_retry: None,
                next_retry_count: None,
                backoff_seconds: None,
                terminal_status: Some("completed".to_string()),
                resolved_status: Some("completed".to_string()),
                finalize_immediately: Some(true),
                cancel_running_task: Some(false),
                reason: "complete-finalized".to_string(),
                effect_plan,
            })
        }
        "completion-race" => {
            let current_status = payload
                .current_status
                .unwrap_or_else(|| "running".to_string());
            let lost_race = matches!(
                current_status.as_str(),
                "interrupt_requested" | "interrupted"
            );
            let terminal_status = if lost_race {
                "interrupted"
            } else {
                "completed"
            };
            let mut effect_plan = if lost_race {
                background_effect_plan("finalize_interrupted")
            } else {
                background_effect_plan("finalize_completed")
            };
            effect_plan.finalize_immediately = Some(true);
            effect_plan.terminal_status = Some(terminal_status.to_string());
            effect_plan.resolved_status = Some(terminal_status.to_string());
            Ok(BackgroundControlResponsePayload {
                schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                operation: payload.operation,
                resolved_parallel_group_id: None,
                lane_ids: None,
                normalized_multitask_strategy: None,
                supported_multitask_strategies,
                strategy_supported: true,
                accepted: None,
                requires_takeover: None,
                error: None,
                should_retry: None,
                next_retry_count: None,
                backoff_seconds: None,
                terminal_status: Some(terminal_status.to_string()),
                resolved_status: Some(terminal_status.to_string()),
                finalize_immediately: Some(true),
                cancel_running_task: Some(false),
                reason: if lost_race {
                    "completion-race-lost".to_string()
                } else {
                    "completion-race-won".to_string()
                },
                effect_plan,
            })
        }
        "retry-claim" => {
            let current_status = payload
                .current_status
                .unwrap_or_else(|| "retry_scheduled".to_string());
            let interrupted = matches!(
                current_status.as_str(),
                "interrupt_requested" | "interrupted"
            );
            let terminal_status = if interrupted {
                "interrupted"
            } else {
                "retry_claimed"
            };
            let mut effect_plan = if interrupted {
                background_effect_plan("finalize_interrupted")
            } else {
                background_effect_plan("claim_retry")
            };
            effect_plan.finalize_immediately = Some(interrupted);
            effect_plan.terminal_status = Some(terminal_status.to_string());
            effect_plan.resolved_status = Some(terminal_status.to_string());
            Ok(BackgroundControlResponsePayload {
                schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                operation: payload.operation,
                resolved_parallel_group_id: None,
                lane_ids: None,
                normalized_multitask_strategy: None,
                supported_multitask_strategies,
                strategy_supported: true,
                accepted: None,
                requires_takeover: None,
                error: None,
                should_retry: None,
                next_retry_count: None,
                backoff_seconds: None,
                terminal_status: Some(terminal_status.to_string()),
                resolved_status: Some(terminal_status.to_string()),
                finalize_immediately: Some(interrupted),
                cancel_running_task: Some(false),
                reason: if interrupted {
                    "retry-claim-interrupted".to_string()
                } else {
                    "retry-claim-granted".to_string()
                },
                effect_plan,
            })
        }
        "interrupt-finalize" => {
            let mut effect_plan = background_effect_plan("finalize_interrupted");
            effect_plan.finalize_immediately = Some(true);
            effect_plan.terminal_status = Some("interrupted".to_string());
            effect_plan.resolved_status = Some("interrupted".to_string());
            Ok(BackgroundControlResponsePayload {
                schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                operation: payload.operation,
                resolved_parallel_group_id: None,
                lane_ids: None,
                normalized_multitask_strategy: None,
                supported_multitask_strategies,
                strategy_supported: true,
                accepted: None,
                requires_takeover: None,
                error: None,
                should_retry: None,
                next_retry_count: None,
                backoff_seconds: None,
                terminal_status: Some("interrupted".to_string()),
                resolved_status: Some("interrupted".to_string()),
                finalize_immediately: Some(true),
                cancel_running_task: Some(false),
                reason: "interrupt-finalized".to_string(),
                effect_plan,
            })
        }
        "retry" => {
            let attempt = payload.attempt.unwrap_or(1).max(1);
            let retry_count = payload.retry_count.unwrap_or(0);
            let max_attempts = payload.max_attempts.unwrap_or(1).max(1);
            if attempt >= max_attempts {
                let mut effect_plan = background_effect_plan("finalize_terminal");
                effect_plan.terminal_status = Some(if max_attempts > 1 {
                    "retry_exhausted".to_string()
                } else {
                    "failed".to_string()
                });
                return Ok(BackgroundControlResponsePayload {
                    schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                    authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                    operation: payload.operation,
                    resolved_parallel_group_id: None,
                    lane_ids: None,
                    normalized_multitask_strategy: None,
                    supported_multitask_strategies,
                    strategy_supported: true,
                    accepted: None,
                    requires_takeover: None,
                    error: None,
                    should_retry: Some(false),
                    next_retry_count: Some(retry_count),
                    backoff_seconds: Some(0.0),
                    terminal_status: Some(if max_attempts > 1 {
                        "retry_exhausted".to_string()
                    } else {
                        "failed".to_string()
                    }),
                    resolved_status: None,
                    finalize_immediately: None,
                    cancel_running_task: None,
                    reason: "attempt-budget-exhausted".to_string(),
                    effect_plan,
                });
            }
            let next_retry_count = retry_count + 1;
            let backoff_seconds = compute_backoff_seconds(
                payload.backoff_base_seconds.unwrap_or(0.0),
                payload.backoff_multiplier.unwrap_or(2.0),
                next_retry_count,
                payload.max_backoff_seconds,
            );
            let mut effect_plan = background_effect_plan("schedule_retry");
            effect_plan.next_retry_count = Some(next_retry_count);
            effect_plan.backoff_seconds = Some(backoff_seconds);
            effect_plan.terminal_status = Some("retry_scheduled".to_string());
            Ok(BackgroundControlResponsePayload {
                schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                operation: payload.operation,
                resolved_parallel_group_id: None,
                lane_ids: None,
                normalized_multitask_strategy: None,
                supported_multitask_strategies,
                strategy_supported: true,
                accepted: None,
                requires_takeover: None,
                error: None,
                should_retry: Some(true),
                next_retry_count: Some(next_retry_count),
                backoff_seconds: Some(backoff_seconds),
                terminal_status: Some("retry_scheduled".to_string()),
                resolved_status: None,
                finalize_immediately: None,
                cancel_running_task: None,
                reason: "retry-scheduled".to_string(),
                effect_plan,
            })
        }
        "session-release" => {
            let mut effect_plan = background_effect_plan("wait_for_release");
            effect_plan.wait_timeout_seconds = Some(5.0);
            effect_plan.wait_poll_interval_seconds = Some(0.01);
            Ok(BackgroundControlResponsePayload {
                schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                operation: payload.operation,
                resolved_parallel_group_id: None,
                lane_ids: None,
                normalized_multitask_strategy: None,
                supported_multitask_strategies,
                strategy_supported: true,
                accepted: None,
                requires_takeover: None,
                error: None,
                should_retry: None,
                next_retry_count: None,
                backoff_seconds: None,
                terminal_status: None,
                resolved_status: None,
                finalize_immediately: None,
                cancel_running_task: None,
                reason: "session-release-wait".to_string(),
                effect_plan,
            })
        }
        other => Err(format!("unsupported background control operation: {other}")),
    }
}

fn sandbox_transition_allowed(current_state: &str, next_state: &str) -> bool {
    matches!(
        (current_state, next_state),
        ("created", "warm")
            | ("warm", "busy")
            | ("busy", "draining")
            | ("draining", "recycled")
            | ("draining", "failed")
            | ("warm", "failed")
            | ("busy", "failed")
            | ("recycled", "warm")
    )
}

#[allow(clippy::too_many_arguments)]
fn sandbox_response(
    request: &SandboxControlRequestPayload,
    current_state: Option<String>,
    next_state: Option<String>,
    allowed: bool,
    resolved_state: Option<String>,
    reason: &str,
    error: Option<String>,
    failure_reason: Option<String>,
    budget_violation: Option<String>,
    cleanup_required: Option<bool>,
    quarantined: Option<bool>,
    effective_capabilities: Option<Vec<String>>,
    event_kind: Option<&str>,
) -> SandboxControlResponsePayload {
    SandboxControlResponsePayload {
        schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
        authority: SANDBOX_CONTROL_AUTHORITY.to_string(),
        operation: request.operation.clone(),
        current_state,
        next_state,
        allowed,
        resolved_state,
        reason: reason.to_string(),
        error,
        failure_reason,
        budget_violation,
        cleanup_required,
        quarantined,
        effective_capabilities,
        sandbox_id: request.sandbox_id.clone(),
        profile_id: request.profile_id.clone(),
        event_schema_version: Some(SANDBOX_EVENT_SCHEMA_VERSION.to_string()),
        event_log_path: request.event_log_path.clone(),
        event_written: false,
        event_kind: event_kind.map(|value| value.to_string()),
    }
}

fn maybe_record_sandbox_event(
    response: &mut SandboxControlResponsePayload,
    request: &SandboxControlRequestPayload,
) -> Result<(), String> {
    if request.trace_event != Some(true) {
        return Ok(());
    }
    let path = request
        .event_log_path
        .as_deref()
        .ok_or_else(|| "sandbox event tracing requires event_log_path".to_string())?;
    let event = json!({
        "schema_version": SANDBOX_EVENT_SCHEMA_VERSION,
        "authority": SANDBOX_CONTROL_AUTHORITY,
        "ts": Utc::now().to_rfc3339(),
        "kind": response.event_kind,
        "operation": response.operation,
        "sandbox_id": response.sandbox_id,
        "profile_id": response.profile_id,
        "current_state": response.current_state,
        "next_state": response.next_state,
        "resolved_state": response.resolved_state,
        "allowed": response.allowed,
        "reason": response.reason,
        "failure_reason": response.failure_reason,
        "budget_violation": response.budget_violation,
        "cleanup_required": response.cleanup_required,
        "quarantined": response.quarantined,
        "effective_capabilities": response.effective_capabilities,
    });
    let serialized = serde_json::to_string(&event)
        .map_err(|err| format!("serialize sandbox event failed: {err}"))?
        + "\n";
    let path = PathBuf::from(path);
    append_text_with_process_lock(&path, &serialized, "sandbox event log")?;
    response.event_log_path = Some(path.display().to_string());
    response.event_written = true;
    Ok(())
}

fn build_sandbox_control_response(
    payload: SandboxControlRequestPayload,
) -> Result<SandboxControlResponsePayload, String> {
    let mut response = match payload.operation.as_str() {
        "transition" => {
            let current_state = payload
                .current_state
                .clone()
                .ok_or_else(|| "sandbox control transition requires current_state".to_string())?;
            let next_state = payload
                .next_state
                .clone()
                .ok_or_else(|| "sandbox control transition requires next_state".to_string())?;
            let allowed = sandbox_transition_allowed(&current_state, &next_state);
            sandbox_response(
                &payload,
                Some(current_state.clone()),
                Some(next_state.clone()),
                allowed,
                Some(next_state.clone()),
                if allowed {
                    "transition-accepted"
                } else {
                    "invalid-transition"
                },
                if allowed {
                    None
                } else {
                    Some(format!(
                        "invalid sandbox transition: {:?} -> {:?}",
                        current_state, next_state
                    ))
                },
                None,
                None,
                None,
                None,
                payload.capability_categories.clone(),
                None,
            )
        }
        "cleanup" => {
            let current_state = payload
                .current_state
                .clone()
                .unwrap_or_else(|| "draining".to_string());
            let cleanup_failed = payload.cleanup_failed.unwrap_or(false);
            let resolved_state = if cleanup_failed { "failed" } else { "recycled" };
            let allowed = matches!(current_state.as_str(), "draining");
            sandbox_response(
                &payload,
                Some(current_state.clone()),
                Some(resolved_state.to_string()),
                allowed,
                Some(resolved_state.to_string()),
                if !allowed {
                    "cleanup-invalid-state"
                } else if cleanup_failed {
                    "cleanup-failed"
                } else {
                    "cleanup-completed"
                },
                if allowed {
                    None
                } else {
                    Some(format!(
                        "invalid sandbox cleanup state: {:?} -> {:?}",
                        current_state, resolved_state
                    ))
                },
                payload
                    .error_kind
                    .clone()
                    .or_else(|| cleanup_failed.then(|| "cleanup_failed".to_string())),
                None,
                Some(false),
                Some(cleanup_failed),
                payload.capability_categories.clone(),
                Some(if cleanup_failed {
                    "sandbox.cleanup_failed"
                } else {
                    "sandbox.cleanup_completed"
                }),
            )
        }
        "admit" => {
            let current_state = payload
                .current_state
                .clone()
                .unwrap_or_else(|| "warm".to_string());
            let categories = payload.capability_categories.clone().unwrap_or_default();
            let tool_category = payload
                .tool_category
                .clone()
                .unwrap_or_else(|| "workspace_mutating".to_string());
            let dedicated_profile = payload.dedicated_profile.unwrap_or(false);
            let failure_reason = if categories.is_empty() {
                Some("policy_violation:missing_capability_declaration".to_string())
            } else if let Some(unknown) = categories.iter().find(|category| {
                !matches!(
                    category.as_str(),
                    "read_only" | "workspace_mutating" | "networked" | "high_risk"
                )
            }) {
                Some(format!("policy_violation:unknown_capability:{unknown}"))
            } else if !matches!(
                tool_category.as_str(),
                "read_only" | "workspace_mutating" | "networked" | "high_risk"
            ) {
                Some(format!(
                    "policy_violation:unknown_tool_category:{tool_category}"
                ))
            } else if !categories.iter().any(|category| category == &tool_category) {
                Some(format!(
                    "policy_violation:capability_denied:{tool_category}"
                ))
            } else if tool_category == "high_risk" && !dedicated_profile {
                Some("policy_violation:high_risk_requires_dedicated_profile".to_string())
            } else if payload.budget_cpu.unwrap_or(0.0) <= 0.0 {
                Some("budget_admission_failed:cpu_non_positive".to_string())
            } else if payload.budget_memory.unwrap_or(0) <= 0 {
                Some("budget_admission_failed:memory_non_positive".to_string())
            } else if payload.budget_wall_clock.unwrap_or(0.0) <= 0.0 {
                Some("budget_admission_failed:wall_clock_non_positive".to_string())
            } else if payload.budget_output_size.unwrap_or(0) <= 0 {
                Some("budget_admission_failed:output_size_non_positive".to_string())
            } else {
                None
            };
            if let Some(reason) = failure_reason.or_else(|| {
                (!sandbox_transition_allowed(&current_state, "busy")).then(|| {
                    format!("invalid sandbox admission state: {current_state:?} -> \"busy\"")
                })
            }) {
                sandbox_response(
                    &payload,
                    Some(current_state.clone()),
                    Some("failed".to_string()),
                    false,
                    Some("failed".to_string()),
                    "admission-rejected",
                    Some(reason.clone()),
                    Some(reason),
                    None,
                    Some(false),
                    Some(true),
                    Some(categories.clone()),
                    Some("sandbox.failed"),
                )
            } else {
                sandbox_response(
                    &payload,
                    Some(current_state.clone()),
                    Some("busy".to_string()),
                    true,
                    Some("busy".to_string()),
                    "admission-accepted",
                    None,
                    None,
                    None,
                    Some(false),
                    Some(false),
                    Some(categories.clone()),
                    Some("sandbox.execution_started"),
                )
            }
        }
        "execution_result" => {
            let current_state = payload
                .current_state
                .clone()
                .unwrap_or_else(|| "busy".to_string());
            let budget_violation = [
                (
                    "cpu_exceeded",
                    payload
                        .probe_cpu
                        .zip(payload.budget_cpu)
                        .is_some_and(|(observed, limit)| observed > limit),
                ),
                (
                    "memory_exceeded",
                    payload
                        .probe_memory
                        .zip(payload.budget_memory)
                        .is_some_and(|(observed, limit)| observed > limit),
                ),
                (
                    "wall_clock_exceeded",
                    payload
                        .probe_wall_clock
                        .zip(payload.budget_wall_clock)
                        .is_some_and(|(observed, limit)| observed > limit),
                ),
                (
                    "output_size_exceeded",
                    payload
                        .probe_output_size
                        .zip(payload.budget_output_size)
                        .is_some_and(|(observed, limit)| observed > limit),
                ),
            ]
            .into_iter()
            .find_map(|(reason, exceeded)| exceeded.then(|| reason.to_string()));
            if let Some(reason) = payload.error_kind.clone() {
                let resolved_state = if reason == "wall_clock_exceeded" {
                    "draining"
                } else {
                    "failed"
                };
                sandbox_response(
                    &payload,
                    Some(current_state.clone()),
                    Some(resolved_state.to_string()),
                    sandbox_transition_allowed(&current_state, resolved_state),
                    Some(resolved_state.to_string()),
                    if resolved_state == "draining" {
                        "execution-timeout"
                    } else {
                        "execution-failed"
                    },
                    Some(reason.clone()),
                    Some(reason),
                    None,
                    Some(resolved_state == "draining"),
                    Some(resolved_state == "failed"),
                    payload.capability_categories.clone(),
                    Some(if resolved_state == "draining" {
                        "sandbox.timeout"
                    } else {
                        "sandbox.failed"
                    }),
                )
            } else if let Some(violation) = budget_violation {
                sandbox_response(
                    &payload,
                    Some(current_state.clone()),
                    Some("draining".to_string()),
                    sandbox_transition_allowed(&current_state, "draining"),
                    Some("draining".to_string()),
                    "budget-exceeded",
                    Some(violation.clone()),
                    Some(violation.clone()),
                    Some(violation),
                    Some(true),
                    Some(false),
                    payload.capability_categories.clone(),
                    Some("sandbox.budget_exceeded"),
                )
            } else {
                sandbox_response(
                    &payload,
                    Some(current_state.clone()),
                    Some("draining".to_string()),
                    sandbox_transition_allowed(&current_state, "draining"),
                    Some("draining".to_string()),
                    "execution-completed",
                    None,
                    None,
                    None,
                    Some(true),
                    Some(false),
                    payload.capability_categories.clone(),
                    Some("sandbox.execution_completed"),
                )
            }
        }
        other => return Err(format!("unsupported sandbox control operation: {other}")),
    };
    maybe_record_sandbox_event(&mut response, &payload)?;
    Ok(response)
}

fn build_live_execute_prompt(payload: &ExecuteRequestPayload) -> String {
    let mut lines = vec![
        "Help with the user's request directly. The route is already chosen, so stay on it."
            .to_string(),
        format!("Primary focus: {}", payload.selected_skill),
    ];
    if let Some(overlay) = payload
        .overlay_skill
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        lines.push(format!("Extra guidance: {overlay}"));
    }
    lines.push("How to reply:".to_string());
    lines.push("- Lead with the answer or result.".to_string());
    lines.push(
        "- Use plain Chinese unless the user asks otherwise, and keep the wording natural."
            .to_string(),
    );
    lines.push("- Keep the default reply short; only use a list when the content is naturally list-shaped.".to_string());
    lines.push("- For closeouts, say what was done, what effect was achieved, and what needs to happen next or that the work is finished.".to_string());
    lines.push("- Do not default to file inventories, evidence dumps, or step-by-step process retellings unless the user asks for them.".to_string());
    let prompt_reasons = payload
        .reasons
        .iter()
        .map(|reason| reason.trim())
        .filter(|reason| !reason.is_empty())
        .take(5)
        .collect::<Vec<_>>();
    if !prompt_reasons.is_empty() {
        lines.push("Task cues:".to_string());
        for reason in prompt_reasons {
            lines.push(format!("- {reason}"));
        }
    }
    if payload.selected_skill == "idea-to-plan" {
        lines.push("Planning output: converge the strategy into outline.md, decision_log.md, assumptions.md, open_questions.md, plan_rubric.md, and code_list.md.".to_string());
        lines.push("Switch to plan-to-code only after the direction is fixed and the remaining work is execution breakdown.".to_string());
    }
    lines.push("Use the selected skill to solve the user's actual task.".to_string());
    lines.join("\n")
}

fn build_dry_run_execute_response(
    payload: &ExecuteRequestPayload,
    prompt_preview: Option<String>,
) -> ExecuteResponsePayload {
    let prompt = prompt_preview.clone().unwrap_or_default();
    let input_tokens = estimate_tokens(&format!("{}\n{}", payload.task, prompt));
    let output_tokens = payload.default_output_tokens.min(96);
    let content = format!(
        "[dry-run] Routed to `{}` on {}. Session `{}` is ready for Rust-owned execution.",
        payload.selected_skill, payload.layer, payload.session_id
    );
    let mut metadata =
        build_steady_state_execution_kernel_metadata(EXECUTION_RESPONSE_SHAPE_DRY_RUN);
    metadata.insert(
        "reason".to_string(),
        Value::String("router-rs returned a deterministic dry-run payload.".to_string()),
    );
    metadata.insert(
        "trace_event_count".to_string(),
        json!(payload.trace_event_count),
    );
    metadata.insert(
        "trace_output_path".to_string(),
        json!(payload.trace_output_path),
    );
    metadata.insert(
        "execution_mode".to_string(),
        Value::String("dry_run".to_string()),
    );
    metadata.insert("route_engine".to_string(), json!(payload.route_engine));
    metadata.insert(
        "diagnostic_route_mode".to_string(),
        json!(payload.diagnostic_route_mode),
    );
    ExecuteResponsePayload {
        execution_schema_version: EXECUTION_SCHEMA_VERSION.to_string(),
        authority: EXECUTION_AUTHORITY.to_string(),
        session_id: payload.session_id.clone(),
        user_id: payload.user_id.clone(),
        skill: payload.selected_skill.clone(),
        overlay: payload.overlay_skill.clone(),
        live_run: false,
        content,
        usage: ExecuteUsagePayload {
            input_tokens,
            output_tokens,
            total_tokens: input_tokens + output_tokens,
            mode: "estimated".to_string(),
        },
        prompt_preview,
        model_id: None,
        metadata: Value::Object(metadata),
    }
}

#[derive(Debug)]
struct LiveExecuteResult {
    content: String,
    model_id: Option<String>,
    run_id: Option<String>,
    status: Option<String>,
    input_tokens: usize,
    output_tokens: usize,
    total_tokens: usize,
}

fn perform_live_execute(
    payload: &ExecuteRequestPayload,
    prompt_preview: &str,
) -> Result<LiveExecuteResult, String> {
    let endpoint = normalize_chat_completions_endpoint(&payload.aggregator_base_url);
    let client = live_execute_http_client()?;
    let mut messages = Vec::new();
    if !prompt_preview.trim().is_empty() {
        messages.push(serde_json::json!({
            "role": "system",
            "content": prompt_preview,
        }));
    }
    messages.push(serde_json::json!({
        "role": "user",
        "content": payload.task,
    }));
    let request_body = serde_json::json!({
        "model": payload.model_id,
        "messages": messages,
        "max_tokens": payload.default_output_tokens,
    });
    let response = client
        .post(endpoint)
        .bearer_auth(payload.aggregator_api_key.as_str())
        .json(&request_body)
        .send()
        .map_err(|err| format!("router-rs live execute request failed: {err}"))?;
    let status = response.status();
    let response_body = response
        .text()
        .map_err(|err| format!("read router-rs live execute response failed: {err}"))?;
    if !status.is_success() {
        return Err(format!(
            "router-rs live execute returned HTTP {}: {}",
            status.as_u16(),
            truncate_for_error(&response_body)
        ));
    }
    let payload = serde_json::from_str::<Value>(&response_body)
        .map_err(|err| format!("parse router-rs live execute response failed: {err}"))?;
    let content = extract_chat_completion_content(&payload)?;
    let usage = payload.get("usage").and_then(Value::as_object);
    let input_tokens = usage
        .and_then(|usage| usage.get("prompt_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| estimate_tokens(&content) as u64) as usize;
    let output_tokens = usage
        .and_then(|usage| usage.get("completion_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| estimate_tokens(&content) as u64) as usize;
    let total_tokens = usage
        .and_then(|usage| usage.get("total_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or((input_tokens + output_tokens) as u64) as usize;
    Ok(LiveExecuteResult {
        content,
        model_id: payload
            .get("model")
            .and_then(Value::as_str)
            .map(|value| value.to_string()),
        run_id: payload
            .get("id")
            .and_then(Value::as_str)
            .map(|value| value.to_string()),
        status: payload
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("finish_reason"))
            .and_then(Value::as_str)
            .map(|value| value.to_string()),
        input_tokens,
        output_tokens,
        total_tokens,
    })
}

fn live_execute_http_client() -> Result<&'static reqwest::blocking::Client, String> {
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client);
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|err| format!("build reqwest client failed: {err}"))?;
    let _ = CLIENT.set(client);
    CLIENT
        .get()
        .ok_or_else(|| "build reqwest client failed: client cache was not initialized".to_string())
}

fn build_live_execute_response(
    payload: &ExecuteRequestPayload,
    prompt_preview: Option<String>,
    live_result: LiveExecuteResult,
) -> ExecuteResponsePayload {
    let mut metadata =
        build_steady_state_execution_kernel_metadata(EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY);
    metadata.insert("run_id".to_string(), json!(live_result.run_id));
    metadata.insert("status".to_string(), json!(live_result.status));
    metadata.insert(
        "trace_event_count".to_string(),
        json!(payload.trace_event_count),
    );
    metadata.insert(
        "trace_output_path".to_string(),
        json!(payload.trace_output_path),
    );
    metadata.insert(
        "execution_mode".to_string(),
        Value::String("live".to_string()),
    );
    metadata.insert("route_engine".to_string(), json!(payload.route_engine));
    metadata.insert(
        "diagnostic_route_mode".to_string(),
        json!(payload.diagnostic_route_mode),
    );
    metadata.insert(
        "execution_kernel_model_id_source".to_string(),
        Value::String(EXECUTION_MODEL_ID_SOURCE.to_string()),
    );
    ExecuteResponsePayload {
        execution_schema_version: EXECUTION_SCHEMA_VERSION.to_string(),
        authority: EXECUTION_AUTHORITY.to_string(),
        session_id: payload.session_id.clone(),
        user_id: payload.user_id.clone(),
        skill: payload.selected_skill.clone(),
        overlay: payload.overlay_skill.clone(),
        live_run: true,
        content: live_result.content,
        usage: ExecuteUsagePayload {
            input_tokens: live_result.input_tokens,
            output_tokens: live_result.output_tokens,
            total_tokens: live_result.total_tokens,
            mode: "live".to_string(),
        },
        prompt_preview,
        model_id: live_result.model_id.clone(),
        metadata: Value::Object(metadata),
    }
}

pub(crate) fn required_non_empty_string(
    payload: &Value,
    key: &str,
    context: &str,
) -> Result<String, String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .ok_or_else(|| format!("{context} requires non-empty {key}"))
}

fn copy_text_to_clipboard(text: &str) -> Result<Value, String> {
    if let Ok(path_value) = std::env::var("ROUTER_RS_CLIPBOARD_PATH") {
        let path = PathBuf::from(path_value);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("create clipboard file parent failed: {err}"))?;
        }
        fs::write(&path, text).map_err(|err| format!("write clipboard file failed: {err}"))?;
        return Ok(json!({
            "backend": "file",
            "target_path": path.display().to_string(),
        }));
    }

    let mut child = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("launch pbcopy failed: {err}"))?;
    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| "pbcopy stdin is unavailable".to_string())?;
        stdin
            .write_all(text.as_bytes())
            .map_err(|err| format!("write pbcopy stdin failed: {err}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|err| format!("wait for pbcopy failed: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "pbcopy exited with a non-zero status".to_string()
        } else {
            format!("pbcopy failed: {stderr}")
        });
    }
    Ok(json!({
        "backend": "pbcopy",
    }))
}

pub(crate) fn optional_non_empty_string(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

fn optional_bool(payload: &Value, key: &str) -> Option<bool> {
    payload.get(key).and_then(Value::as_bool)
}

fn nested_value<'a>(payload: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = payload;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn nested_non_empty_string(payload: &Value, path: &[&str]) -> Option<String> {
    nested_value(payload, path)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

fn nested_bool(payload: &Value, path: &[&str]) -> Option<bool> {
    nested_value(payload, path).and_then(Value::as_bool)
}

fn build_attach_target_payload(
    session_id: &str,
    job_id: Option<&str>,
    endpoint_kind: &str,
    subscribe_method: &str,
    describe_method: &str,
    cleanup_method: &str,
    handoff_method: &str,
) -> Value {
    serde_json::json!({
        "endpoint_kind": endpoint_kind,
        "subscribe_method": subscribe_method,
        "describe_method": describe_method,
        "cleanup_method": cleanup_method,
        "handoff_method": handoff_method,
        "session_id": session_id,
        "job_id": job_id,
    })
}

fn build_replay_anchor_payload(
    latest_cursor: Value,
    resume_mode: &str,
    replay_supported: bool,
) -> Value {
    serde_json::json!({
        "anchor_kind": "trace_replay_cursor",
        "cursor_schema_version": "runtime-trace-cursor-v1",
        "resume_mode": resume_mode,
        "latest_cursor": latest_cursor,
        "replay_supported": replay_supported,
    })
}

fn build_transport_health_payload(payload: &Value) -> Value {
    if payload.get("transport_health").is_some() {
        return payload
            .get("transport_health")
            .cloned()
            .unwrap_or(Value::Null);
    }
    if payload.get("control_plane").is_none() {
        return Value::Null;
    }
    serde_json::json!({
        "backend_family": nested_non_empty_string(payload, &["control_plane", "backend_family"]),
        "supports_atomic_replace": nested_bool(payload, &["control_plane", "supports_atomic_replace"]),
        "supports_compaction": nested_bool(payload, &["control_plane", "supports_compaction"]),
        "supports_snapshot_delta": nested_bool(payload, &["control_plane", "supports_snapshot_delta"]),
        "supports_remote_event_transport": nested_bool(payload, &["control_plane", "supports_remote_event_transport"]),
    })
}

fn build_trace_transport_payload(
    payload: &Value,
    session_id: String,
    job_id: Option<String>,
) -> Value {
    let stream_key = job_id.as_deref().unwrap_or(session_id.as_str());
    let endpoint_kind = optional_non_empty_string(payload, "endpoint_kind")
        .unwrap_or_else(|| "runtime_method".to_string());
    let subscribe_method = optional_non_empty_string(payload, "subscribe_method")
        .unwrap_or_else(|| "subscribe_runtime_events".to_string());
    let describe_method = optional_non_empty_string(payload, "describe_method")
        .unwrap_or_else(|| "describe_runtime_event_transport".to_string());
    let cleanup_method = optional_non_empty_string(payload, "cleanup_method")
        .unwrap_or_else(|| "cleanup_runtime_events".to_string());
    let handoff_method = optional_non_empty_string(payload, "handoff_method")
        .unwrap_or_else(|| "describe_runtime_event_handoff".to_string());
    let resume_mode = optional_non_empty_string(payload, "resume_mode")
        .unwrap_or_else(|| "after_event_id".to_string());
    let replay_supported = optional_bool(payload, "replay_supported").unwrap_or(true);
    let latest_cursor = payload.get("latest_cursor").cloned().unwrap_or(Value::Null);
    let binding_backend_family = optional_non_empty_string(payload, "binding_backend_family")
        .or_else(|| nested_non_empty_string(payload, &["control_plane", "backend_family"]));
    let control_plane_authority = optional_non_empty_string(payload, "control_plane_authority")
        .or_else(|| {
            nested_non_empty_string(payload, &["control_plane", "trace_service", "authority"])
        });
    let control_plane_role = optional_non_empty_string(payload, "control_plane_role")
        .or_else(|| nested_non_empty_string(payload, &["control_plane", "trace_service", "role"]));
    let control_plane_projection = optional_non_empty_string(payload, "control_plane_projection")
        .or_else(|| {
            nested_non_empty_string(payload, &["control_plane", "trace_service", "projection"])
        });
    let control_plane_delegate_kind =
        optional_non_empty_string(payload, "control_plane_delegate_kind").or_else(|| {
            nested_non_empty_string(
                payload,
                &["control_plane", "trace_service", "delegate_kind"],
            )
        });
    let attach_target = build_attach_target_payload(
        &session_id,
        job_id.as_deref(),
        &endpoint_kind,
        &subscribe_method,
        &describe_method,
        &cleanup_method,
        &handoff_method,
    );
    let replay_anchor =
        build_replay_anchor_payload(latest_cursor.clone(), &resume_mode, replay_supported);

    serde_json::json!({
        "schema_version": "runtime-event-transport-v1",
        "stream_id": format!("stream::{stream_key}"),
        "session_id": session_id,
        "job_id": job_id,
        "transport_contract_kind": optional_non_empty_string(payload, "transport_contract_kind").unwrap_or_else(|| "runtime_event_stream".to_string()),
        "transport_family": optional_non_empty_string(payload, "transport_family").unwrap_or_else(|| "host-facing-transport".to_string()),
        "transport_kind": optional_non_empty_string(payload, "transport_kind").unwrap_or_else(|| "poll".to_string()),
        "endpoint_kind": endpoint_kind,
        "ownership_lane": optional_non_empty_string(payload, "ownership_lane").unwrap_or_else(|| "rust-contract-lane".to_string()),
        "producer_owner": optional_non_empty_string(payload, "producer_owner").unwrap_or_else(|| "rust-control-plane".to_string()),
        "producer_authority": optional_non_empty_string(payload, "producer_authority").unwrap_or_else(|| RUNTIME_CONTROL_PLANE_AUTHORITY.to_string()),
        "exporter_owner": optional_non_empty_string(payload, "exporter_owner").unwrap_or_else(|| "rust-control-plane".to_string()),
        "exporter_authority": optional_non_empty_string(payload, "exporter_authority").unwrap_or_else(|| RUNTIME_CONTROL_PLANE_AUTHORITY.to_string()),
        "remote_capable": optional_bool(payload, "remote_capable")
            .or_else(|| nested_bool(payload, &["control_plane", "supports_remote_event_transport"]))
            .unwrap_or(true),
        "remote_attach_supported": optional_bool(payload, "remote_attach_supported")
            .or_else(|| nested_bool(payload, &["control_plane", "supports_remote_event_transport"]))
            .unwrap_or(true),
        "handoff_supported": optional_bool(payload, "handoff_supported").unwrap_or(true),
        "handoff_method": handoff_method,
        "subscribe_method": subscribe_method,
        "cleanup_method": cleanup_method,
        "describe_method": describe_method,
        "handoff_kind": optional_non_empty_string(payload, "handoff_kind").unwrap_or_else(|| "artifact_handoff".to_string()),
        "binding_refresh_mode": optional_non_empty_string(payload, "binding_refresh_mode").unwrap_or_else(|| "describe_or_checkpoint".to_string()),
        "binding_artifact_format": optional_non_empty_string(payload, "binding_artifact_format").unwrap_or_else(|| "json".to_string()),
        "binding_backend_family": binding_backend_family,
        "binding_artifact_path": optional_non_empty_string(payload, "binding_artifact_path"),
        "resume_mode": resume_mode,
        "heartbeat_supported": optional_bool(payload, "heartbeat_supported").unwrap_or(true),
        "cleanup_semantics": optional_non_empty_string(payload, "cleanup_semantics").unwrap_or_else(|| "stream_cache_only".to_string()),
        "cleanup_preserves_replay": optional_bool(payload, "cleanup_preserves_replay").unwrap_or(true),
        "replay_reseed_supported": optional_bool(payload, "replay_reseed_supported").unwrap_or(true),
        "chunk_schema_version": optional_non_empty_string(payload, "chunk_schema_version").unwrap_or_else(|| "runtime-event-stream-v1".to_string()),
        "cursor_schema_version": optional_non_empty_string(payload, "cursor_schema_version").unwrap_or_else(|| "runtime-trace-cursor-v1".to_string()),
        "latest_cursor": latest_cursor,
        "replay_supported": replay_supported,
        "attach_target": payload.get("attach_target").cloned().unwrap_or(attach_target),
        "replay_anchor": payload.get("replay_anchor").cloned().unwrap_or(replay_anchor),
        "control_plane_authority": control_plane_authority,
        "control_plane_role": control_plane_role,
        "control_plane_projection": control_plane_projection,
        "control_plane_delegate_kind": control_plane_delegate_kind,
        "transport_health": build_transport_health_payload(payload),
    })
}

fn build_trace_transport_descriptor(payload: Value) -> Result<Value, String> {
    let session_id = required_non_empty_string(&payload, "session_id", "describe transport")?;
    let job_id = optional_non_empty_string(&payload, "job_id");
    Ok(serde_json::json!({
        "schema_version": TRACE_DESCRIPTOR_SCHEMA_VERSION,
        "authority": TRACE_DESCRIPTOR_AUTHORITY,
        "transport": build_trace_transport_payload(&payload, session_id, job_id),
    }))
}

fn build_trace_handoff_descriptor(payload: Value) -> Result<Value, String> {
    let session_id = required_non_empty_string(&payload, "session_id", "describe handoff")?;
    let job_id = optional_non_empty_string(&payload, "job_id");
    let transport_source = payload.get("transport").cloned().unwrap_or(Value::Null);
    let transport_session_id = optional_non_empty_string(&transport_source, "session_id")
        .unwrap_or_else(|| session_id.clone());
    let transport_job_id =
        optional_non_empty_string(&transport_source, "job_id").or_else(|| job_id.clone());
    let transport = if transport_source.is_object() {
        build_trace_transport_payload(
            &transport_source,
            transport_session_id.clone(),
            transport_job_id.clone(),
        )
    } else {
        build_trace_transport_payload(&payload, session_id.clone(), job_id.clone())
    };
    let checkpoint_backend_family =
        optional_non_empty_string(&payload, "checkpoint_backend_family")
            .or_else(|| optional_non_empty_string(&transport, "binding_backend_family"))
            .or_else(|| nested_non_empty_string(&payload, &["control_plane", "backend_family"]))
            .unwrap_or_else(|| "filesystem".to_string());
    let trace_stream_path = optional_non_empty_string(&payload, "trace_stream_path");
    let resume_manifest_path = optional_non_empty_string(&payload, "resume_manifest_path");
    let recovery_artifacts = payload
        .get("recovery_artifacts")
        .cloned()
        .filter(Value::is_array)
        .unwrap_or_else(|| {
            let mut ordered: Vec<String> = Vec::new();
            if let Some(path) = optional_non_empty_string(&transport, "binding_artifact_path") {
                ordered.push(path);
            }
            if let Some(path) = resume_manifest_path.clone() {
                ordered.push(path);
            }
            if let Some(path) = trace_stream_path.clone() {
                ordered.push(path);
            }
            serde_json::json!(ordered)
        });

    Ok(serde_json::json!({
        "schema_version": TRACE_DESCRIPTOR_SCHEMA_VERSION,
        "authority": TRACE_DESCRIPTOR_AUTHORITY,
        "handoff": {
            "schema_version": "runtime-event-handoff-v1",
            "stream_id": transport.get("stream_id").cloned().unwrap_or_else(|| Value::String(format!("stream::{}", transport_job_id.as_deref().unwrap_or(transport_session_id.as_str())))),
            "session_id": session_id,
            "job_id": job_id,
            "checkpoint_backend_family": checkpoint_backend_family,
            "trace_stream_path": trace_stream_path,
            "resume_manifest_path": resume_manifest_path,
            "remote_attach_strategy": optional_non_empty_string(&payload, "remote_attach_strategy").unwrap_or_else(|| "transport_descriptor_then_replay".to_string()),
            "cleanup_preserves_replay": transport.get("cleanup_preserves_replay").and_then(Value::as_bool).unwrap_or(true),
            "attach_target": transport.get("attach_target").cloned().unwrap_or(Value::Null),
            "replay_anchor": transport.get("replay_anchor").cloned().unwrap_or(Value::Null),
            "recovery_artifacts": recovery_artifacts,
            "control_plane": payload.get("control_plane").cloned().unwrap_or(Value::Null),
            "transport": transport,
        },
    }))
}

fn build_checkpoint_resume_manifest(payload: Value) -> Result<Value, String> {
    let session_id =
        required_non_empty_string(&payload, "session_id", "checkpoint resume manifest")?;
    let job_id = optional_non_empty_string(&payload, "job_id");
    let status =
        optional_non_empty_string(&payload, "status").unwrap_or_else(|| "running".to_string());
    let generation = payload
        .get("generation")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let mut resume_manifest = serde_json::json!({
        "schema_version": "runtime-resume-manifest-v1",
        "session_id": session_id,
        "job_id": job_id,
        "status": status,
        "generation": generation,
        "trace_output_path": optional_non_empty_string(&payload, "trace_output_path"),
        "trace_stream_path": optional_non_empty_string(&payload, "trace_stream_path"),
        "event_transport_path": optional_non_empty_string(&payload, "event_transport_path"),
        "background_state_path": optional_non_empty_string(&payload, "background_state_path"),
        "latest_cursor": payload.get("latest_cursor").cloned().unwrap_or(Value::Null),
        "artifact_paths": payload
            .get("artifact_paths")
            .cloned()
            .filter(Value::is_array)
            .unwrap_or_else(|| serde_json::json!([])),
        "parallel_group": payload.get("parallel_group").cloned().unwrap_or(Value::Null),
        "supervisor_projection": payload.get("supervisor_projection").cloned().unwrap_or(Value::Null),
        "control_plane": payload.get("control_plane").cloned().unwrap_or(Value::Null),
    });
    if let Some(updated_at) = optional_non_empty_string(&payload, "updated_at") {
        if let Some(map) = resume_manifest.as_object_mut() {
            map.insert("updated_at".to_string(), Value::String(updated_at));
        }
    }
    Ok(serde_json::json!({
        "schema_version": CHECKPOINT_RESUME_MANIFEST_SCHEMA_VERSION,
        "authority": CHECKPOINT_RESUME_MANIFEST_AUTHORITY,
        "resume_manifest": resume_manifest,
    }))
}

fn write_json_payload(path: &Path, payload: &Value) -> Result<usize, String> {
    let serialized = format!(
        "{}\n",
        serde_json::to_string_pretty(payload)
            .map_err(|err| format!("serialize persisted payload failed: {err}"))?
    );
    write_text_payload(path, &serialized)
}

fn write_transport_binding_payload(payload: Value) -> Result<Value, String> {
    let path = required_non_empty_string(&payload, "path", "write transport binding")?;
    let session_id = required_non_empty_string(&payload, "session_id", "write transport binding")?;
    let job_id = optional_non_empty_string(&payload, "job_id");
    let transport = build_trace_transport_payload(&payload, session_id, job_id);
    let bytes_written = write_json_payload(Path::new(&path), &transport)?;
    Ok(serde_json::json!({
        "schema_version": TRANSPORT_BINDING_WRITE_SCHEMA_VERSION,
        "authority": TRANSPORT_BINDING_WRITE_AUTHORITY,
        "path": path,
        "bytes_written": bytes_written,
    }))
}

fn write_checkpoint_resume_manifest_payload(payload: Value) -> Result<Value, String> {
    let path = required_non_empty_string(&payload, "path", "write checkpoint resume manifest")?;
    let manifest = build_checkpoint_resume_manifest(payload)?
        .get("resume_manifest")
        .cloned()
        .ok_or_else(|| "checkpoint resume manifest payload missing resume_manifest".to_string())?;
    let bytes_written = write_json_payload(Path::new(&path), &manifest)?;
    Ok(serde_json::json!({
        "schema_version": CHECKPOINT_MANIFEST_WRITE_SCHEMA_VERSION,
        "authority": CHECKPOINT_MANIFEST_WRITE_AUTHORITY,
        "path": path,
        "bytes_written": bytes_written,
    }))
}

pub(crate) fn write_text_payload(path: &Path, payload: &str) -> Result<usize, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "create parent directory for {} failed: {err}",
                path.display()
            )
        })?;
    }
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("persist path {} has no file name", path.display()))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = WRITE_TEXT_PAYLOAD_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let thread_id = format!("{:?}", std::thread::current().id())
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>();
    let tmp_path = path.with_file_name(format!(
        "{file_name}.{}.{}.{}.{}.tmp",
        std::process::id(),
        thread_id,
        nonce,
        sequence
    ));
    fs::write(&tmp_path, payload.as_bytes())
        .map_err(|err| format!("write temp payload {} failed: {err}", tmp_path.display()))?;
    fs::rename(&tmp_path, path).map_err(|err| {
        let _ = fs::remove_file(&tmp_path);
        format!("replace payload {} failed: {err}", path.display())
    })?;
    Ok(payload.len())
}

fn descriptor_mapping<'a>(
    attach_descriptor: &'a Value,
    field_name: &str,
) -> Result<Option<&'a Map<String, Value>>, String> {
    match attach_descriptor.get(field_name) {
        None => Ok(None),
        Some(Value::Object(map)) => Ok(Some(map)),
        Some(_) => Err(format!(
            "External runtime event attach descriptor field {field_name:?} must be a mapping."
        )),
    }
}

fn mapping_string(
    mapping: &Map<String, Value>,
    field_name: &str,
) -> Result<Option<String>, String> {
    match mapping.get(field_name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(format!(
            "External runtime event attach descriptor field {field_name:?} must be a string."
        )),
    }
}

fn merge_attach_path_values(
    explicit_value: Option<String>,
    descriptor_value: Option<String>,
    field_name: &str,
) -> Result<Option<String>, String> {
    match (explicit_value, descriptor_value) {
        (None, descriptor) => Ok(descriptor),
        (Some(explicit), None) => Ok(Some(explicit)),
        (Some(explicit), Some(descriptor)) if explicit == descriptor => Ok(Some(explicit)),
        (Some(_), Some(_)) => Err(format!(
            "External runtime event attach received conflicting {field_name:?} values between direct args and attach_descriptor."
        )),
    }
}

struct NormalizedAttachRequest {
    binding_artifact_path: Option<String>,
    handoff_path: Option<String>,
    resume_manifest_path: Option<String>,
    trace_stream_path: Option<String>,
    binding_artifact_resolution: Option<String>,
    handoff_resolution: Option<String>,
    resume_manifest_resolution: Option<String>,
}

fn normalize_attach_request(payload: &Value) -> Result<NormalizedAttachRequest, String> {
    let explicit_binding_artifact_path =
        optional_non_empty_string(payload, "binding_artifact_path");
    let explicit_handoff_path = optional_non_empty_string(payload, "handoff_path");
    let explicit_resume_manifest_path = optional_non_empty_string(payload, "resume_manifest_path");
    let Some(attach_descriptor) = payload.get("attach_descriptor") else {
        return Ok(NormalizedAttachRequest {
            binding_artifact_path: explicit_binding_artifact_path.clone(),
            handoff_path: explicit_handoff_path.clone(),
            resume_manifest_path: explicit_resume_manifest_path.clone(),
            trace_stream_path: None,
            binding_artifact_resolution: explicit_binding_artifact_path
                .as_ref()
                .map(|_| "explicit_request".to_string()),
            handoff_resolution: explicit_handoff_path
                .as_ref()
                .map(|_| "explicit_request".to_string()),
            resume_manifest_resolution: explicit_resume_manifest_path
                .as_ref()
                .map(|_| "explicit_request".to_string()),
        });
    };
    if attach_descriptor.is_null() {
        return Ok(NormalizedAttachRequest {
            binding_artifact_path: explicit_binding_artifact_path.clone(),
            handoff_path: explicit_handoff_path.clone(),
            resume_manifest_path: explicit_resume_manifest_path.clone(),
            trace_stream_path: None,
            binding_artifact_resolution: explicit_binding_artifact_path
                .as_ref()
                .map(|_| "explicit_request".to_string()),
            handoff_resolution: explicit_handoff_path
                .as_ref()
                .map(|_| "explicit_request".to_string()),
            resume_manifest_resolution: explicit_resume_manifest_path
                .as_ref()
                .map(|_| "explicit_request".to_string()),
        });
    }
    if !attach_descriptor.is_object() {
        return Err("External runtime event attach descriptor must be a mapping.".to_string());
    }
    let schema_version = attach_descriptor
        .get("schema_version")
        .and_then(Value::as_str);
    if let Some(schema_version) = schema_version {
        if schema_version != "runtime-event-attach-descriptor-v1" {
            return Err(format!(
                "Unsupported runtime event attach descriptor schema: {schema_version:?}"
            ));
        }
    }
    let attach_mode = attach_descriptor.get("attach_mode").and_then(Value::as_str);
    if let Some(attach_mode) = attach_mode {
        if attach_mode != "process_external_artifact_replay" {
            return Err(format!(
                "Unsupported runtime event attach mode: {attach_mode:?}"
            ));
        }
    }
    let expected_scalars = [
        (
            "source_transport_method",
            "describe_runtime_event_transport",
        ),
        ("source_handoff_method", "describe_runtime_event_handoff"),
        ("attach_method", "attach_runtime_event_transport"),
        ("subscribe_method", "subscribe_attached_runtime_events"),
        ("cleanup_method", "cleanup_attached_runtime_event_transport"),
        ("resume_mode", "after_event_id"),
    ];
    for (field_name, expected) in expected_scalars {
        if let Some(value) = attach_descriptor.get(field_name).and_then(Value::as_str) {
            if value != expected {
                return Err(format!(
                    "External runtime event attach descriptor must use {field_name}={expected:?}."
                ));
            }
        }
    }
    if let Some(capabilities) = descriptor_mapping(attach_descriptor, "attach_capabilities")? {
        if capabilities.get("artifact_replay").and_then(Value::as_bool) != Some(true) {
            return Err(
                "External runtime event attach descriptor must advertise attach_capabilities.artifact_replay=True."
                    .to_string(),
            );
        }
        if !matches!(
            capabilities
                .get("live_remote_stream")
                .and_then(Value::as_bool),
            None | Some(false)
        ) {
            return Err(
                "External runtime event attach descriptor must advertise attach_capabilities.live_remote_stream=False."
                    .to_string(),
            );
        }
        if !matches!(
            capabilities
                .get("cleanup_preserves_replay")
                .and_then(Value::as_bool),
            None | Some(true)
        ) {
            return Err(
                "External runtime event attach descriptor must advertise attach_capabilities.cleanup_preserves_replay=True."
                    .to_string(),
            );
        }
    }
    let _ = descriptor_mapping(attach_descriptor, "requested_artifacts")?;
    let resolution_mapping = descriptor_mapping(attach_descriptor, "resolution")?;
    let resolved_mapping = descriptor_mapping(attach_descriptor, "resolved_artifacts")?
        .unwrap_or_else(|| {
            attach_descriptor
                .as_object()
                .expect("attach descriptor object")
        });
    let descriptor_binding = mapping_string(resolved_mapping, "binding_artifact_path")?;
    let descriptor_handoff = mapping_string(resolved_mapping, "handoff_path")?;
    let descriptor_resume = mapping_string(resolved_mapping, "resume_manifest_path")?;
    let descriptor_trace_stream = mapping_string(resolved_mapping, "trace_stream_path")?;
    let binding_artifact_path = merge_attach_path_values(
        explicit_binding_artifact_path.clone(),
        descriptor_binding,
        "binding_artifact_path",
    )?;
    let handoff_path = merge_attach_path_values(
        explicit_handoff_path.clone(),
        descriptor_handoff,
        "handoff_path",
    )?;
    let resume_manifest_path = merge_attach_path_values(
        explicit_resume_manifest_path.clone(),
        descriptor_resume,
        "resume_manifest_path",
    )?;
    Ok(NormalizedAttachRequest {
        binding_artifact_path,
        handoff_path,
        resume_manifest_path,
        trace_stream_path: descriptor_trace_stream,
        binding_artifact_resolution: if explicit_binding_artifact_path.is_some() {
            Some("explicit_request".to_string())
        } else {
            resolution_mapping
                .as_ref()
                .map(|mapping| mapping_string(mapping, "binding_artifact_path"))
                .transpose()?
                .flatten()
        },
        handoff_resolution: if explicit_handoff_path.is_some() {
            Some("explicit_request".to_string())
        } else {
            resolution_mapping
                .as_ref()
                .map(|mapping| mapping_string(mapping, "handoff_path"))
                .transpose()?
                .flatten()
        },
        resume_manifest_resolution: if explicit_resume_manifest_path.is_some() {
            Some("explicit_request".to_string())
        } else {
            resolution_mapping
                .as_ref()
                .map(|mapping| mapping_string(mapping, "resume_manifest_path"))
                .transpose()?
                .flatten()
        },
    })
}

fn require_requested_artifact(
    path: &Option<PathBuf>,
    storage_backend: Option<&ResolvedStorageBackend>,
    field_name: &str,
) -> Result<(), String> {
    if let Some(path) = path {
        if !storage_artifact_exists(path, storage_backend) {
            return Err(format!(
                "External runtime event attach requested {field_name:?} that does not exist: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn load_json_artifact(
    path: &Option<PathBuf>,
    storage_backend: Option<&ResolvedStorageBackend>,
) -> Result<Option<Value>, String> {
    let Some(path) = path else {
        return Ok(None);
    };
    if !storage_artifact_exists(path, storage_backend) {
        return Ok(None);
    }
    serde_json::from_str::<Value>(&storage_read_text(path, storage_backend)?)
        .map(Some)
        .map_err(|err| {
            format!(
                "parse runtime attach artifact failed for {}: {err}",
                path.display()
            )
        })
}

fn json_path(value: &Value, key: &str) -> Result<Option<PathBuf>, String> {
    normalize_optional_runtime_path(optional_non_empty_string(value, key))
}

fn nested_json_path(value: &Value, path: &[&str]) -> Result<Option<PathBuf>, String> {
    normalize_optional_runtime_path(nested_non_empty_string(value, path))
}

fn normalize_optional_runtime_path(value: Option<String>) -> Result<Option<PathBuf>, String> {
    value
        .map(|path| {
            let candidate = PathBuf::from(path.trim());
            if candidate.as_os_str().is_empty() {
                return Err("runtime attach path must be non-empty".to_string());
            }
            if candidate.is_absolute() {
                Ok(candidate)
            } else {
                std::env::current_dir()
                    .map(|cwd| cwd.join(candidate))
                    .map_err(|err| format!("resolve runtime attach path failed: {err}"))
            }
        })
        .transpose()
}

fn normalize_path_for_compare(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

fn infer_resume_manifest_path(binding_artifact_path: &Path) -> PathBuf {
    let candidates = [
        binding_artifact_path
            .parent()
            .and_then(Path::parent)
            .map(|parent| parent.join("TRACE_RESUME_MANIFEST.json")),
        binding_artifact_path
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .map(|parent| parent.join("TRACE_RESUME_MANIFEST.json")),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|candidate| candidate.exists())
        .unwrap_or_else(|| {
            binding_artifact_path
                .parent()
                .and_then(Path::parent)
                .map(|parent| parent.join("TRACE_RESUME_MANIFEST.json"))
                .unwrap_or_else(|| PathBuf::from("TRACE_RESUME_MANIFEST.json"))
        })
}

fn infer_trace_stream_from_binding_artifact(
    binding_artifact_path: Option<&Path>,
    storage_backend: Option<&ResolvedStorageBackend>,
) -> Option<PathBuf> {
    let binding_artifact_path = binding_artifact_path?;
    let candidates = [
        binding_artifact_path
            .parent()
            .and_then(Path::parent)
            .map(|parent| parent.join("TRACE_EVENTS.jsonl")),
        binding_artifact_path
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .map(|parent| parent.join("TRACE_EVENTS.jsonl")),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|candidate| storage_artifact_exists(candidate, storage_backend))
}

fn validate_attached_runtime_alignment(
    transport: &Value,
    handoff: Option<&Value>,
    resume_manifest: Option<&Value>,
    binding_artifact_path: Option<&Path>,
    resume_manifest_path: Option<&Path>,
    storage_backend: Option<&ResolvedStorageBackend>,
) -> Result<(), String> {
    let transport_stream_id = optional_non_empty_string(transport, "stream_id");
    let transport_session_id = optional_non_empty_string(transport, "session_id");
    let transport_job_id = optional_non_empty_string(transport, "job_id");

    if let Some(handoff) = handoff {
        if optional_non_empty_string(handoff, "stream_id") != transport_stream_id {
            return Err(
                "External runtime event attach rejected mismatched transport/handoff stream ids."
                    .to_string(),
            );
        }
        if optional_non_empty_string(handoff, "session_id") != transport_session_id
            || optional_non_empty_string(handoff, "job_id") != transport_job_id
        {
            return Err(
                "External runtime event attach rejected mismatched transport/handoff stream scope."
                    .to_string(),
            );
        }
        if let (Some(binding_artifact_path), Some(handoff_binding_path)) = (
            binding_artifact_path,
            nested_json_path(handoff, &["transport", "binding_artifact_path"])?,
        ) {
            if normalize_path_for_compare(&handoff_binding_path)
                != normalize_path_for_compare(binding_artifact_path)
            {
                return Err("External runtime event attach rejected mismatched transport/handoff binding artifact paths.".to_string());
            }
        }
        if let (Some(resume_manifest_path), Some(handoff_resume_manifest_path)) = (
            resume_manifest_path,
            json_path(handoff, "resume_manifest_path")?,
        ) {
            if normalize_path_for_compare(&handoff_resume_manifest_path)
                != normalize_path_for_compare(resume_manifest_path)
            {
                return Err("External runtime event attach rejected mismatched handoff/resume manifest paths.".to_string());
            }
        }
    }

    if let Some(resume_manifest) = resume_manifest {
        if optional_non_empty_string(resume_manifest, "session_id") != transport_session_id
            || optional_non_empty_string(resume_manifest, "job_id") != transport_job_id
        {
            return Err(
                "External runtime event attach rejected mismatched transport/resume stream scope."
                    .to_string(),
            );
        }
        if let (Some(binding_artifact_path), Some(resume_binding_path)) = (
            binding_artifact_path,
            json_path(resume_manifest, "event_transport_path")?,
        ) {
            if normalize_path_for_compare(&resume_binding_path)
                != normalize_path_for_compare(binding_artifact_path)
            {
                return Err("External runtime event attach rejected mismatched transport/resume binding artifact paths.".to_string());
            }
        }
        if let (Some(_handoff), Some(handoff_trace_stream_path), Some(resume_trace_stream_path)) = (
            handoff,
            handoff
                .map(|value| json_path(value, "trace_stream_path"))
                .transpose()?
                .flatten(),
            json_path(resume_manifest, "trace_stream_path")?,
        ) {
            if normalize_path_for_compare(&handoff_trace_stream_path)
                != normalize_path_for_compare(&resume_trace_stream_path)
            {
                return Err("External runtime event attach rejected mismatched handoff/resume trace stream paths.".to_string());
            }
        }
    }

    let binding_trace_stream_path =
        infer_trace_stream_from_binding_artifact(binding_artifact_path, storage_backend);
    if let (Some(binding_trace_stream_path), Some(_handoff), Some(handoff_trace_stream_path)) = (
        binding_trace_stream_path.as_ref(),
        handoff,
        handoff
            .map(|value| json_path(value, "trace_stream_path"))
            .transpose()?
            .flatten(),
    ) {
        if normalize_path_for_compare(&handoff_trace_stream_path)
            != normalize_path_for_compare(binding_trace_stream_path)
        {
            return Err("External runtime event attach rejected mismatched binding/handoff trace stream paths.".to_string());
        }
    }
    if let (
        Some(binding_trace_stream_path),
        Some(_resume_manifest),
        Some(resume_trace_stream_path),
    ) = (
        binding_trace_stream_path.as_ref(),
        resume_manifest,
        resume_manifest
            .map(|value| json_path(value, "trace_stream_path"))
            .transpose()?
            .flatten(),
    ) {
        if normalize_path_for_compare(&resume_trace_stream_path)
            != normalize_path_for_compare(binding_trace_stream_path)
        {
            return Err("External runtime event attach rejected mismatched binding/resume trace stream paths.".to_string());
        }
    }
    Ok(())
}

fn trace_stream_resolution(
    handoff: Option<&Value>,
    resume_manifest: Option<&Value>,
    binding_artifact_path: Option<&Path>,
    storage_backend: Option<&ResolvedStorageBackend>,
) -> Result<Option<(PathBuf, String)>, String> {
    if let Some(handoff) = handoff {
        if let Some(path) = json_path(handoff, "trace_stream_path")? {
            return Ok(Some((path, "handoff_manifest".to_string())));
        }
    }
    if let Some(resume_manifest) = resume_manifest {
        if let Some(path) = json_path(resume_manifest, "trace_stream_path")? {
            return Ok(Some((path, "resume_manifest".to_string())));
        }
    }
    if let Some(path) =
        infer_trace_stream_from_binding_artifact(binding_artifact_path, storage_backend)
    {
        return Ok(Some((path, "binding_artifact_adjacency".to_string())));
    }
    Ok(None)
}

pub(crate) fn attach_runtime_event_transport(payload: Value) -> Result<Value, String> {
    let normalized_request = normalize_attach_request(&payload)?;
    let binding_artifact_path = normalized_request.binding_artifact_path;
    let handoff_path = normalized_request.handoff_path;
    let resume_manifest_path = normalized_request.resume_manifest_path;
    let descriptor_trace_stream_path =
        normalize_optional_runtime_path(normalized_request.trace_stream_path)?;
    if binding_artifact_path.is_none() && handoff_path.is_none() && resume_manifest_path.is_none() {
        return Err(
            "External runtime event attach requires a binding artifact, handoff manifest, or resume manifest path."
                .to_string(),
        );
    }

    let binding_path = normalize_optional_runtime_path(binding_artifact_path)?;
    let handoff_file = normalize_optional_runtime_path(handoff_path)?;
    let resume_file = normalize_optional_runtime_path(resume_manifest_path)?;
    let mut binding_source = normalized_request.binding_artifact_resolution;
    let handoff_source = normalized_request.handoff_resolution;
    let mut resume_source = normalized_request.resume_manifest_resolution;

    let requested_paths = [
        binding_path.clone(),
        handoff_file.clone(),
        resume_file.clone(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let storage_backend = resolve_storage_backend(&requested_paths);
    require_requested_artifact(
        &binding_path,
        storage_backend.as_ref(),
        "binding_artifact_path",
    )?;
    require_requested_artifact(&handoff_file, storage_backend.as_ref(), "handoff_path")?;
    require_requested_artifact(
        &resume_file,
        storage_backend.as_ref(),
        "resume_manifest_path",
    )?;

    let handoff = load_json_artifact(&handoff_file, storage_backend.as_ref())?;
    let mut resume_manifest = load_json_artifact(&resume_file, storage_backend.as_ref())?;
    let mut resolved_resume_file = resume_file.clone();

    if resume_manifest.is_none() {
        if let Some(handoff_resume_path) = handoff
            .as_ref()
            .map(|payload| json_path(payload, "resume_manifest_path"))
            .transpose()?
            .flatten()
        {
            if storage_artifact_exists(&handoff_resume_path, storage_backend.as_ref()) {
                resolved_resume_file = Some(handoff_resume_path.clone());
                resume_manifest =
                    load_json_artifact(&Some(handoff_resume_path), storage_backend.as_ref())?;
                resume_source = Some("handoff_manifest".to_string());
            }
        }
    }

    let mut transport_path = binding_path.clone();
    if transport_path.is_none() {
        if let Some(resume_transport_path) = resume_manifest
            .as_ref()
            .map(|payload| json_path(payload, "event_transport_path"))
            .transpose()?
            .flatten()
        {
            if storage_artifact_exists(&resume_transport_path, storage_backend.as_ref()) {
                transport_path = Some(resume_transport_path);
                binding_source = Some("resume_manifest".to_string());
            }
        }
    }
    if transport_path.is_none() {
        if let Some(handoff_transport_path) = handoff
            .as_ref()
            .map(|payload| nested_json_path(payload, &["transport", "binding_artifact_path"]))
            .transpose()?
            .flatten()
        {
            if storage_artifact_exists(&handoff_transport_path, storage_backend.as_ref()) {
                transport_path = Some(handoff_transport_path);
                binding_source = Some("handoff_transport".to_string());
            }
        }
    }

    if transport_path.is_none() && handoff.is_none() {
        return Err(
            "External runtime event attach could not resolve a transport binding artifact from the provided manifests."
                .to_string(),
        );
    }

    let transport = if let Some(transport_path) = transport_path.as_ref() {
        load_json_artifact(&Some(transport_path.clone()), storage_backend.as_ref())?.ok_or_else(
            || "External runtime event attach could not load a transport descriptor.".to_string(),
        )?
    } else {
        handoff
            .as_ref()
            .and_then(|payload| payload.get("transport").cloned())
            .ok_or_else(|| {
                "External runtime event attach could not load a transport descriptor.".to_string()
            })?
    };

    if resume_manifest.is_none() {
        if let Some(transport_path) = transport_path.as_ref() {
            let inferred_resume_path = infer_resume_manifest_path(transport_path);
            if storage_artifact_exists(&inferred_resume_path, storage_backend.as_ref()) {
                resolved_resume_file = Some(inferred_resume_path.clone());
                resume_manifest =
                    load_json_artifact(&Some(inferred_resume_path), storage_backend.as_ref())?;
            }
        }
    }

    validate_attached_runtime_alignment(
        &transport,
        handoff.as_ref(),
        resume_manifest.as_ref(),
        transport_path.as_deref(),
        resolved_resume_file.as_deref(),
        storage_backend.as_ref(),
    )?;

    let Some((trace_stream_path, trace_stream_source)) = trace_stream_resolution(
        handoff.as_ref(),
        resume_manifest.as_ref(),
        transport_path.as_deref(),
        storage_backend.as_ref(),
    )?
    else {
        return Err(
            "External runtime event replay requires a handoff or resume manifest with trace_stream_path, or a filesystem binding artifact adjacent to TRACE_EVENTS.jsonl."
                .to_string(),
        );
    };
    if let Some(descriptor_trace_stream_path) = descriptor_trace_stream_path.as_ref() {
        if normalize_path_for_compare(descriptor_trace_stream_path)
            != normalize_path_for_compare(&trace_stream_path)
        {
            return Err(
                "External runtime event attach descriptor must already match canonical 'resolved_artifacts.trace_stream_path'."
                    .to_string(),
            );
        }
    }
    if !storage_artifact_exists(&trace_stream_path, storage_backend.as_ref()) {
        return Err(format!(
            "External runtime event replay trace stream not found: {}",
            trace_stream_path.display()
        ));
    }

    let resume_mode = optional_non_empty_string(&transport, "resume_mode")
        .unwrap_or_else(|| "after_event_id".to_string());
    let artifact_backend_family = optional_non_empty_string(&transport, "binding_backend_family")
        .unwrap_or_else(|| "filesystem".to_string());
    let source_transport_method = "describe_runtime_event_transport";
    let source_handoff_method = "describe_runtime_event_handoff";
    let attach_method = "attach_runtime_event_transport";
    let subscribe_method = "subscribe_attached_runtime_events";
    let cleanup_method = "cleanup_attached_runtime_event_transport";
    let cleanup_semantics = "no_persisted_state";
    let recommended_entrypoint = "describe_runtime_event_handoff";
    let attach_descriptor = json!({
        "schema_version": "runtime-event-attach-descriptor-v1",
        "attach_mode": "process_external_artifact_replay",
        "artifact_backend_family": artifact_backend_family.clone(),
        "source_transport_method": source_transport_method,
        "source_handoff_method": source_handoff_method,
        "attach_method": attach_method,
        "subscribe_method": subscribe_method,
        "cleanup_method": cleanup_method,
        "resume_mode": resume_mode.clone(),
        "cleanup_semantics": cleanup_semantics,
        "attach_capabilities": {
            "artifact_replay": true,
            "live_remote_stream": false,
            "cleanup_preserves_replay": true,
        },
        "recommended_entrypoint": recommended_entrypoint,
        "requested_artifacts": {
            "binding_artifact_path": transport_path.as_ref().map(|path| path.display().to_string()),
            "handoff_path": handoff_file.as_ref().map(|path| path.display().to_string()),
            "resume_manifest_path": resolved_resume_file.as_ref().map(|path| path.display().to_string()),
        },
        "resolved_artifacts": {
            "binding_artifact_path": transport_path.as_ref().map(|path| path.display().to_string()),
            "handoff_path": handoff_file.as_ref().map(|path| path.display().to_string()),
            "resume_manifest_path": resolved_resume_file.as_ref().map(|path| path.display().to_string()),
            "trace_stream_path": trace_stream_path.display().to_string(),
        },
        "resolution": {
            "binding_artifact_path": binding_source,
            "handoff_path": handoff_source,
            "resume_manifest_path": resume_source,
            "trace_stream_path": trace_stream_source,
        },
    });

    Ok(json!({
        "attach_mode": "process_external_artifact_replay",
        "artifact_backend_family": artifact_backend_family,
        "source_handoff_method": source_handoff_method,
        "source_transport_method": source_transport_method,
        "attach_method": attach_method,
        "subscribe_method": subscribe_method,
        "cleanup_method": cleanup_method,
        "resume_mode": resume_mode,
        "transport": transport,
        "handoff": handoff,
        "resume_manifest": resume_manifest,
        "binding_artifact_path": transport_path.as_ref().map(|path| path.display().to_string()),
        "handoff_path": handoff_file.as_ref().map(|path| path.display().to_string()),
        "resume_manifest_path": resolved_resume_file.as_ref().map(|path| path.display().to_string()),
        "trace_stream_path": trace_stream_path.display().to_string(),
        "replay_supported": true,
        "cleanup_semantics": cleanup_semantics,
        "cleanup_preserves_replay": true,
        "authority": ATTACHED_RUNTIME_EVENT_ATTACH_AUTHORITY,
        "attach_descriptor": attach_descriptor,
    }))
}

fn subscribe_attached_runtime_events(payload: Value) -> Result<Value, String> {
    let attached = attach_runtime_event_transport(payload.clone())?;
    let transport = attached
        .get("transport")
        .ok_or_else(|| "attached runtime transport payload missing transport".to_string())?;
    let session_id = optional_non_empty_string(transport, "session_id")
        .ok_or_else(|| "attached runtime transport payload missing session_id".to_string())?;
    let job_id = optional_non_empty_string(transport, "job_id");
    let trace_stream_path = attached
        .get("trace_stream_path")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "attached runtime transport payload missing trace_stream_path".to_string()
        })?;
    let replay = replay_trace_stream(TraceStreamReplayRequestPayload {
        path: Some(trace_stream_path.to_string()),
        event_stream_text: None,
        compaction_manifest_path: None,
        compaction_manifest_text: None,
        compaction_state_text: None,
        compaction_artifact_index_text: None,
        compaction_delta_text: None,
        session_id: Some(session_id.clone()),
        job_id: job_id.clone(),
        stream_scope_fields: None,
        after_event_id: optional_non_empty_string(&payload, "after_event_id"),
        limit: payload
            .get("limit")
            .and_then(Value::as_u64)
            .map(|value| value as usize),
    })?;
    let heartbeat = optional_bool(&payload, "heartbeat").unwrap_or(false);
    let events = replay.events.clone();
    let has_more = replay.has_more;
    let next_cursor = serde_json::to_value(&replay.next_cursor)
        .map_err(|err| format!("serialize attached runtime cursor failed: {err}"))?;
    Ok(json!({
        "schema_version": "runtime-event-stream-v1",
        "session_id": session_id,
        "job_id": job_id,
        "events": events,
        "next_cursor": next_cursor,
        "has_more": has_more,
        "after_event_id": optional_non_empty_string(&payload, "after_event_id"),
        "heartbeat": if heartbeat && replay.events.is_empty() {
            json!({
                "schema_version": "runtime-event-stream-heartbeat-v1",
                "kind": "runtime.stream.heartbeat",
                "status": "idle",
            })
        } else {
            Value::Null
        },
    }))
}

fn cleanup_attached_runtime_event_transport(payload: Value) -> Result<Value, String> {
    let attached = attach_runtime_event_transport(payload)?;
    Ok(json!({
        "authority": ATTACHED_RUNTIME_EVENT_ATTACH_AUTHORITY,
        "cleanup_semantics": "no_persisted_state",
        "cleanup_preserves_replay": true,
        "cleanup_method": "cleanup_attached_runtime_event_transport",
        "binding_artifact_path": attached.get("binding_artifact_path").cloned().unwrap_or(Value::Null),
        "trace_stream_path": attached.get("trace_stream_path").cloned().unwrap_or(Value::Null),
    }))
}

pub(crate) fn build_runtime_control_plane_payload() -> Value {
    let concurrency_defaults = runtime_concurrency_defaults_payload();
    let services = serde_json::json!({
        "router": {
            "authority": ROUTE_AUTHORITY,
            "role": "route-selection",
            "projection": "rust-owned-live-route",
            "delegate_kind": "rust-route-core",
        },
        "skill_loader": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "skill-registry-projection",
            "projection": "rust-native-projection",
            "delegate_kind": "rust-runtime-control-plane",
        },
        "prompt_builder": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "prompt-contract-projection",
            "projection": "rust-native-projection",
            "delegate_kind": "rust-execution-cli",
        },
        "middleware": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "middleware-policy-projection",
            "projection": "rust-native-projection",
            "delegate_kind": "rust-runtime-control-plane",
            "subagent_limit_contract": {
                "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
                "owner": "rust-runtime-control-plane",
                "projection": "rust-native-projection",
                "limit_owner": "rust-control-plane",
                "max_concurrent_subagents": concurrency_defaults.max_concurrent_subagents,
                "max_concurrent_subagents_limit": concurrency_defaults.max_concurrent_subagents_limit,
                "timeout_seconds": concurrency_defaults.subagent_timeout_seconds,
                "enforcement_mode": "rust-owned-policy-native-enforced",
            },
        },
        "state": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "durable-background-state",
            "projection": "rust-native-projection",
            "delegate_kind": "filesystem-state-store",
        },
        "trace": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "trace-and-handoff",
            "projection": "rust-native-projection",
            "delegate_kind": "filesystem-trace-store",
        },
        "memory": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "memory-lifecycle",
            "projection": "rust-native-projection",
            "delegate_kind": "fact-memory-store",
        },
        "checkpoint": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "checkpoint-artifact-projection",
            "projection": "rust-native-projection",
            "delegate_kind": "filesystem-checkpointer",
            "backend_family_catalog": runtime_backend_family_catalog_payload(),
            "backend_family_parity": runtime_backend_family_parity_payload(
                Some("filesystem"),
                Some("filesystem"),
                Some("filesystem"),
                Some("filesystem"),
            ).expect("default backend family parity is valid"),
        },
        "execution": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "execution-kernel-control",
            "projection": "rust-native-projection",
            "delegate_kind": "rust-execution-kernel-slice",
            "kernel_contract": Value::Object(build_steady_state_execution_kernel_metadata(
                EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY,
            )),
            "kernel_contract_by_mode": Value::Object(build_execution_kernel_contracts_by_mode()),
            "kernel_metadata_contract": build_execution_kernel_metadata_contract(),
            "kernel_adapter_kind": "rust-execution-kernel-slice",
            "kernel_authority": "rust-execution-kernel-authority",
            "kernel_owner_family": "rust",
            "kernel_owner_impl": "execution-kernel-slice",
            "kernel_contract_mode": "rust-live-primary",
            "kernel_replace_ready": true,
            "kernel_in_process_replacement_complete": true,
            "kernel_live_backend_family": "rust-cli",
            "kernel_live_backend_impl": "router-rs",
            "kernel_live_delegate_kind": "router-rs",
            "kernel_live_delegate_authority": "rust-execution-cli",
            "kernel_live_delegate_family": "rust-cli",
            "kernel_live_delegate_impl": "router-rs",
            "kernel_live_delegate_mode": "rust-primary",
            "kernel_mode_support": ["dry_run", "live"],
            "execution_schema_version": EXECUTION_SCHEMA_VERSION,
            "sandbox_lifecycle_contract": {
                "schema_version": "runtime-sandbox-lifecycle-v1",
                "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
                "role": "sandbox-lifecycle-control",
                "projection": "rust-native-projection",
                "delegate_kind": "rust-runtime-control-plane",
                "lifecycle_states": [
                    "created",
                    "warm",
                    "busy",
                    "draining",
                    "recycled",
                    "failed"
                ],
                "allowed_transitions": [
                    ["busy", "draining"],
                    ["busy", "failed"],
                    ["created", "warm"],
                    ["draining", "failed"],
                    ["draining", "recycled"],
                    ["recycled", "warm"],
                    ["warm", "busy"],
                    ["warm", "failed"]
                ],
                "capability_categories": [
                    "read_only",
                    "workspace_mutating",
                    "networked",
                    "high_risk"
                ],
                "cleanup_mode": "async-drain-and-recycle",
                "event_log_artifact": "runtime_sandbox_events.jsonl",
                "event_schema_version": SANDBOX_EVENT_SCHEMA_VERSION,
                "event_tracing": {
                    "request_flag": "trace_event",
                    "path_field": "event_log_path",
                    "response_flag": "event_written",
                    "effective_capabilities_field": "effective_capabilities"
                },
                "control_operations": ["transition", "cleanup", "admit", "execution_result"],
                "runtime_probe_dimensions": ["cpu", "memory", "wall_clock", "output_size"],
            },
        },
        "background": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "background-orchestration",
            "projection": "rust-native-projection",
            "delegate_kind": "rust-background-control-policy",
            "orchestration_contract": {
                "schema_version": "runtime-background-orchestration-v1",
                "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
                "role": "background-orchestration-control",
                "projection": "rust-native-projection",
                "delegate_kind": "rust-background-control-policy",
                "policy_schema_version": BACKGROUND_CONTROL_SCHEMA_VERSION,
                "queue_model": "bounded-async-host",
                "session_takeover_model": "state-store-lease-arbitration",
                "state_artifact": "runtime_background_jobs.json",
                "active_statuses": [
                    "queued",
                    "running",
                    "interrupt_requested",
                    "retry_scheduled",
                    "retry_claimed"
                ],
                "terminal_statuses": [
                    "completed",
                    "failed",
                    "interrupted",
                    "retry_exhausted"
                ],
                "policy_operations": [
                    "batch-plan",
                    "enqueue",
                    "claim",
                    "interrupt",
                    "interrupt-finalize",
                    "retry",
                    "retry-claim",
                    "complete",
                    "completion-race",
                    "session-release"
                ],
                "max_background_jobs": concurrency_defaults.max_background_jobs,
                "max_background_jobs_limit": concurrency_defaults.max_background_jobs_limit,
                "background_job_timeout_seconds": concurrency_defaults.background_job_timeout_seconds,
                "admission_owner": "rust-background-control-policy",
                "queue_concurrency_owner": "rust-control-plane",
            },
        },
    });
    let rust_owned_service_count = services
        .as_object()
        .map(|service_map| service_map.len())
        .unwrap_or(0);

    serde_json::json!({
        "schema_version": RUNTIME_CONTROL_PLANE_SCHEMA_VERSION,
        "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
        "default_route_mode": "rust",
        "default_route_authority": ROUTE_AUTHORITY,
        "runtime_status": {
            "runtime_primary_owner": "rust-control-plane",
            "runtime_primary_owner_authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "hot_path_projection_mode": "descriptor-driven",
            "framework_runtime_replacement": "router-rs::framework_runtime",
            "framework_runtime_replacement_authority": framework_runtime::FRAMEWORK_RUNTIME_AUTHORITY,
        },
        "runtime_host": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "runtime-orchestration",
            "projection": "rust-native-projection",
            "delegate_kind": "rust-runtime-control-plane",
            "startup_order": ["router", "state", "trace", "memory", "execution", "background"],
            "shutdown_order": ["background", "execution", "memory", "trace", "state", "router"],
            "health_sections": [
                "router",
                "state",
                "trace",
                "memory",
                "execution_environment",
                "background",
                "checkpoint"
            ],
            "rust_owned_service_count": rust_owned_service_count,
            "concurrency_contract": {
                "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
                "owner": "rust-control-plane",
                "router_stdio_pool_owner": "rust-control-plane",
                "router_stdio_pool_default_size": concurrency_defaults.router_stdio.default_pool_size,
                "router_stdio_pool_max_size": concurrency_defaults.router_stdio.max_pool_size,
                "router_stdio_pool_env_keys": concurrency_defaults.router_stdio.env_keys,
                "router_stdio_pool_scheduling": concurrency_defaults.router_stdio.scheduling,
                "router_stdio_backpressure": concurrency_defaults.router_stdio.backpressure,
                "stdio_max_concurrency_arg": concurrency_defaults.router_stdio.stdio_max_concurrency_arg,
                "request_concurrency_field": concurrency_defaults.router_stdio.request_concurrency_field,
                "compute_threads_owner": "rust-control-plane",
                "compute_threads_default": concurrency_defaults.compute.default_threads,
                "compute_threads_max": concurrency_defaults.compute.max_threads,
                "compute_threads_env_keys": concurrency_defaults.compute.env_keys,
                "compute_threads_arg": concurrency_defaults.compute.cli_arg,
                "compute_threads_scheduling": concurrency_defaults.compute.scheduling,
                "max_background_jobs": concurrency_defaults.max_background_jobs,
                "max_background_jobs_limit": concurrency_defaults.max_background_jobs_limit,
                "max_concurrent_subagents": concurrency_defaults.max_concurrent_subagents,
                "max_concurrent_subagents_limit": concurrency_defaults.max_concurrent_subagents_limit,
                "background_job_timeout_seconds": concurrency_defaults.background_job_timeout_seconds,
                "subagent_timeout_seconds": concurrency_defaults.subagent_timeout_seconds,
            },
        },
        "services": services,
    })
}

fn build_runtime_integrator_payload() -> Value {
    let control_plane = build_runtime_control_plane_payload();
    let runtime_host = control_plane
        .get("runtime_host")
        .cloned()
        .unwrap_or(Value::Null);
    let services = control_plane
        .get("services")
        .cloned()
        .unwrap_or(Value::Null);
    let runtime_status = control_plane
        .get("runtime_status")
        .cloned()
        .unwrap_or(Value::Null);
    let concurrency_contract = runtime_host
        .get("concurrency_contract")
        .cloned()
        .unwrap_or(Value::Null);
    let subagent_limit_contract = services
        .get("middleware")
        .and_then(Value::as_object)
        .and_then(|middleware| middleware.get("subagent_limit_contract"))
        .cloned()
        .unwrap_or(Value::Null);
    let observability_exporter = build_runtime_observability_exporter_descriptor();
    let observability_metric_catalog = build_runtime_observability_metric_catalog_payload();
    let observability_dashboard = runtime_observability_dashboard_schema();
    json!({
        "schema_version": RUNTIME_INTEGRATOR_SCHEMA_VERSION,
        "authority": RUNTIME_INTEGRATOR_AUTHORITY,
        "mode": "rust-owned-thin-orchestration",
        "control_plane": control_plane,
        "runtime_host": runtime_host,
        "services": services,
        "runtime_status": runtime_status,
        "concurrency_contract": concurrency_contract,
        "subagent_limit_contract": subagent_limit_contract,
        "observability": {
            "schema_version": RUNTIME_OBSERVABILITY_HEALTH_SNAPSHOT_SCHEMA_VERSION,
            "ownership_lane": observability_exporter["ownership_lane"].clone(),
            "metric_catalog_version": observability_exporter["metric_catalog_version"].clone(),
            "dashboard_schema_version": observability_dashboard["schema_version"].clone(),
            "resource_dimensions": observability_dashboard["resource_dimensions"].clone(),
            "metric_catalog_schema_version": observability_metric_catalog["schema_version"].clone(),
            "metric_names": observability_metric_catalog["metrics"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|metric| metric.get("metric_name").cloned())
                .collect::<Vec<Value>>(),
            "dashboard_panel_count": observability_dashboard["panels"]
                .as_array()
                .map(|items| items.len())
                .unwrap_or(0),
            "dashboard_alert_count": observability_dashboard["alerts"]
                .as_array()
                .map(|items| items.len())
                .unwrap_or(0),
            "exporter": observability_exporter,
        },
    })
}

fn runtime_observability_resource_dimensions() -> Vec<&'static str> {
    vec![
        "service.name",
        "service.version",
        "runtime.instance.id",
        "route_engine_mode",
    ]
}

fn runtime_observability_base_dimensions() -> Vec<&'static str> {
    vec![
        "runtime.job_id",
        "runtime.session_id",
        "runtime.attempt",
        "runtime.worker_id",
        "runtime.generation",
        "runtime.schema_version",
    ]
}

fn runtime_observability_dashboard_dimensions() -> Vec<String> {
    runtime_observability_resource_dimensions()
        .into_iter()
        .chain(runtime_observability_base_dimensions())
        .map(|value| value.to_string())
        .collect()
}

fn runtime_observability_metric_catalog() -> Vec<Value> {
    let base_dimensions = runtime_observability_dashboard_dimensions();
    vec![
        json!({
            "intent": "route mismatch rate",
            "metric_name": "runtime.route_mismatch_total",
            "metric_type": "counter",
            "unit": "1",
            "base_dimensions": base_dimensions.clone(),
            "dashboard_derivation": "rate(route_mismatch_total) / rate(route_evaluation_total)",
        }),
        json!({
            "intent": "replay resume success rate",
            "metric_name": "runtime.replay_resume_success_total",
            "metric_type": "counter",
            "unit": "1",
            "base_dimensions": base_dimensions.clone(),
            "dashboard_derivation": "rate(replay_resume_success_total) / rate(replay_resume_attempt_total)",
        }),
        json!({
            "intent": "lease takeover latency",
            "metric_name": "runtime.lease_takeover_latency_ms",
            "metric_type": "histogram",
            "unit": "ms",
            "base_dimensions": base_dimensions.clone(),
            "dashboard_derivation": "p50 / p95 / p99",
        }),
        json!({
            "intent": "interrupt completion latency",
            "metric_name": "runtime.interrupt_completion_latency_ms",
            "metric_type": "histogram",
            "unit": "ms",
            "base_dimensions": base_dimensions.clone(),
            "dashboard_derivation": "p50 / p95 / p99",
        }),
        json!({
            "intent": "compression offload rate",
            "metric_name": "runtime.compression_offload_total",
            "metric_type": "counter",
            "unit": "1",
            "base_dimensions": base_dimensions.clone(),
            "dashboard_derivation": "rate(compression_offload_total) / rate(compression_candidate_total)",
        }),
        json!({
            "intent": "sandbox timeout rate",
            "metric_name": "runtime.sandbox_timeout_total",
            "metric_type": "counter",
            "unit": "1",
            "base_dimensions": base_dimensions,
            "dashboard_derivation": "rate(sandbox_timeout_total) / rate(sandbox_execution_total)",
        }),
    ]
}

fn build_runtime_observability_metric_catalog_payload() -> Value {
    let metrics = runtime_observability_metric_catalog()
        .into_iter()
        .map(|metric| {
            let mut metric_object = metric;
            if let Some(base_dimensions) = metric_object.get("base_dimensions").cloned() {
                if let Some(object) = metric_object.as_object_mut() {
                    object.remove("base_dimensions");
                    object.insert("dimensions".to_string(), base_dimensions);
                }
            }
            metric_object
        })
        .collect::<Vec<Value>>();

    json!({
        "schema_version": RUNTIME_OBSERVABILITY_METRIC_CATALOG_SCHEMA_VERSION,
        "metric_catalog_version": RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION,
        "resource_dimensions": runtime_observability_resource_dimensions(),
        "base_dimensions": runtime_observability_base_dimensions(),
        "metrics": metrics,
    })
}

fn build_runtime_observability_exporter_descriptor() -> Value {
    json!({
        "schema_version": RUNTIME_OBSERVABILITY_EXPORTER_SCHEMA_VERSION,
        "metric_catalog_version": RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION,
        "dashboard_schema_version": RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION,
        "signal_vocabulary": RUNTIME_OBSERVABILITY_SIGNAL_VOCABULARY,
        "export_path": "jsonl-plus-otel",
        "jsonl_sink_schema_version": "runtime-event-sink-v1",
        "trace_stream_schema_version": "runtime-event-stream-v1",
        "trace_handoff_schema_version": "runtime-event-handoff-v1",
        "ownership_lane": "rust-contract-lane",
        "producer_owner": "rust-control-plane",
        "producer_authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
        "exporter_owner": "rust-control-plane",
        "exporter_authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
    })
}

fn build_runtime_observability_health_snapshot() -> Value {
    let exporter = build_runtime_observability_exporter_descriptor();
    let dashboard = runtime_observability_dashboard_schema();
    let catalog = build_runtime_observability_metric_catalog_payload();
    let metric_names = catalog
        .get("metrics")
        .and_then(Value::as_array)
        .map(|metrics| {
            metrics
                .iter()
                .filter_map(|metric| metric.get("metric_name").cloned())
                .collect::<Vec<Value>>()
        })
        .unwrap_or_default();
    let dashboard_panel_count = dashboard
        .get("panels")
        .and_then(Value::as_array)
        .map(|panels| panels.len())
        .unwrap_or(0);
    let dashboard_alert_count = dashboard
        .get("alerts")
        .and_then(Value::as_array)
        .map(|alerts| alerts.len())
        .unwrap_or(0);

    json!({
        "schema_version": RUNTIME_OBSERVABILITY_HEALTH_SNAPSHOT_SCHEMA_VERSION,
        "ownership_lane": exporter["ownership_lane"].clone(),
        "metric_catalog_version": exporter["metric_catalog_version"].clone(),
        "dashboard_schema_version": dashboard["schema_version"].clone(),
        "resource_dimensions": dashboard["resource_dimensions"].clone(),
        "metric_catalog_schema_version": catalog["schema_version"].clone(),
        "metric_names": metric_names,
        "dashboard_panel_count": dashboard_panel_count,
        "dashboard_alert_count": dashboard_alert_count,
        "exporter": exporter,
    })
}

fn runtime_observability_dashboard_schema() -> Value {
    json!({
        "schema_version": RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION,
        "title": "Runtime Observability",
        "resource_dimensions": runtime_observability_dashboard_dimensions(),
        "panels": [
            {
                "name": "Route mismatch rate",
                "metric": "runtime.route_mismatch_total",
                "visualization": "timeseries",
                "group_by": ["service.name", "service.version", "route_engine_mode"],
            },
            {
                "name": "Replay resume success rate",
                "metric": "runtime.replay_resume_success_total",
                "visualization": "timeseries",
                "group_by": ["service.name", "service.version", "runtime.session_id"],
            },
            {
                "name": "Lease takeover latency",
                "metric": "runtime.lease_takeover_latency_ms",
                "visualization": "histogram",
                "group_by": ["service.name", "service.version", "runtime.worker_id"],
            },
            {
                "name": "Interrupt completion latency",
                "metric": "runtime.interrupt_completion_latency_ms",
                "visualization": "histogram",
                "group_by": ["service.name", "service.version", "runtime.session_id"],
            },
            {
                "name": "Compression offload rate",
                "metric": "runtime.compression_offload_total",
                "visualization": "timeseries",
                "group_by": ["service.name", "service.version", "runtime.generation"],
            },
            {
                "name": "Sandbox timeout rate",
                "metric": "runtime.sandbox_timeout_total",
                "visualization": "timeseries",
                "group_by": ["service.name", "service.version", "runtime.worker_id"],
            }
        ],
        "alerts": [
            {
                "name": "route-mismatch-burst",
                "metric": "runtime.route_mismatch_total",
                "severity": "warning",
            },
            {
                "name": "lease-takeover-latency-regression",
                "metric": "runtime.lease_takeover_latency_ms",
                "severity": "critical",
            },
            {
                "name": "sandbox-timeout-spike",
                "metric": "runtime.sandbox_timeout_total",
                "severity": "warning",
            }
        ],
    })
}

fn build_runtime_metric_record(payload: Value) -> Result<Value, String> {
    let metric_name = required_non_empty_string(&payload, "metric_name", "runtime metric record")?;
    let spec = runtime_observability_metric_catalog()
        .into_iter()
        .find(|entry| {
            entry.get("metric_name").and_then(Value::as_str) == Some(metric_name.as_str())
        })
        .ok_or_else(|| format!("unsupported runtime metric: {metric_name}"))?;

    let value = payload
        .get("value")
        .cloned()
        .ok_or_else(|| "runtime metric record requires a numeric value".to_string())?;
    let numeric_value = value
        .as_f64()
        .ok_or_else(|| "runtime metric record requires a numeric value".to_string())?;
    if !numeric_value.is_finite() {
        return Err("metric value must be finite".to_string());
    }

    let service_name =
        required_non_empty_string(&payload, "service_name", "runtime metric record")?;
    let service_version =
        required_non_empty_string(&payload, "service_version", "runtime metric record")?;
    let runtime_instance_id =
        required_non_empty_string(&payload, "runtime_instance_id", "runtime metric record")?;
    let route_engine_mode =
        required_non_empty_string(&payload, "route_engine_mode", "runtime metric record")?;
    let job_id = required_non_empty_string(&payload, "job_id", "runtime metric record")?;
    let session_id = required_non_empty_string(&payload, "session_id", "runtime metric record")?;
    let worker_id = required_non_empty_string(&payload, "worker_id", "runtime metric record")?;
    let generation = required_non_empty_string(&payload, "generation", "runtime metric record")?;
    let attempt = payload
        .get("attempt")
        .and_then(Value::as_i64)
        .ok_or_else(|| "runtime metric record requires integer field attempt".to_string())?;
    if attempt < 0 {
        return Err(
            "runtime metric record requires non-negative integer field attempt".to_string(),
        );
    }

    Ok(json!({
        "schema_version": RUNTIME_OBSERVABILITY_METRIC_RECORD_SCHEMA_VERSION,
        "metric_name": metric_name,
        "metric_type": spec.get("metric_type").cloned().unwrap_or(Value::Null),
        "unit": spec.get("unit").cloned().unwrap_or(Value::Null),
        "value": value,
        "resource_attributes": {
            "service.name": service_name,
            "service.version": service_version,
            "runtime.instance.id": runtime_instance_id,
            "route_engine_mode": route_engine_mode,
        },
        "dimensions": {
            "runtime.job_id": job_id,
            "runtime.session_id": session_id,
            "runtime.attempt": attempt,
            "runtime.worker_id": worker_id,
            "runtime.generation": generation,
            "runtime.schema_version": RUNTIME_OBSERVABILITY_METRIC_RECORD_SCHEMA_VERSION,
            "runtime.stage": "runtime.metric",
            "runtime.status": "ok",
        },
        "ownership": build_runtime_observability_exporter_descriptor(),
    }))
}

fn normalize_chat_completions_endpoint(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/chat/completions") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/chat/completions")
    }
}

fn truncate_for_error(raw: &str) -> String {
    let compact = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= 240 {
        compact
    } else {
        compact.chars().take(237).collect::<String>() + "..."
    }
}

fn extract_chat_completion_content(payload: &Value) -> Result<String, String> {
    let message_content = payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .ok_or_else(|| {
            "router-rs live execute response missing choices[0].message.content".to_string()
        })?;

    if let Some(content) = message_content.as_str() {
        return Ok(content.to_string());
    }

    if let Some(parts) = message_content.as_array() {
        let joined = parts
            .iter()
            .filter_map(|part| {
                part.get("text")
                    .and_then(Value::as_str)
                    .map(|value| value.to_string())
                    .or_else(|| {
                        part.get("content")
                            .and_then(Value::as_str)
                            .map(|value| value.to_string())
                    })
            })
            .collect::<Vec<_>>()
            .join("");
        if !joined.is_empty() {
            return Ok(joined);
        }
    }

    Err("router-rs live execute response content had an unsupported shape".to_string())
}

fn estimate_tokens(text: &str) -> usize {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return 0;
    }
    trimmed.chars().count().div_ceil(4)
}

fn extract_trace_event_payload(payload: Value) -> Result<Value, String> {
    let event_payload = match payload {
        Value::Object(mut object) => match object.remove("event") {
            Some(Value::Object(event)) => Value::Object(event),
            Some(other) => {
                return Err(format!(
                    "trace stream line contained non-object event wrapper: {other}"
                ))
            }
            None => Value::Object(object),
        },
        other => {
            return Err(format!(
                "trace stream line must decode to a JSON object: {other}"
            ))
        }
    };
    Ok(event_payload)
}

fn trace_event_object(payload: Value) -> Result<Map<String, Value>, String> {
    match extract_trace_event_payload(payload)? {
        Value::Object(object) => Ok(object),
        other => Err(format!(
            "trace stream payload must resolve to a JSON object: {other}"
        )),
    }
}

fn trace_event_string_field(payload: &Map<String, Value>, field: &str) -> Option<String> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn trace_event_usize_field(payload: &Map<String, Value>, field: &str) -> Option<usize> {
    payload
        .get(field)
        .and_then(|value| value.as_u64().map(|number| number as usize))
}

fn build_trace_cursor(generation: usize, seq: usize, event_id: &str) -> String {
    format!("g{generation}:s{seq}:{event_id}")
}

fn hydrate_trace_event_object(
    mut payload: Map<String, Value>,
    line_number: usize,
) -> Map<String, Value> {
    let seq = trace_event_usize_field(&payload, "seq").unwrap_or(line_number);
    let generation = trace_event_usize_field(&payload, "generation").unwrap_or(0);
    let event_id = trace_event_string_field(&payload, "event_id")
        .unwrap_or_else(|| format!("evt_replay_{line_number:06}"));
    let cursor = trace_event_string_field(&payload, "cursor")
        .unwrap_or_else(|| build_trace_cursor(generation, seq, &event_id));

    payload
        .entry("seq".to_string())
        .or_insert_with(|| json!(seq));
    payload
        .entry("generation".to_string())
        .or_insert_with(|| json!(generation));
    payload
        .entry("event_id".to_string())
        .or_insert_with(|| Value::String(event_id));
    payload
        .entry("cursor".to_string())
        .or_insert_with(|| Value::String(cursor));
    payload
        .entry("status".to_string())
        .or_insert_with(|| Value::String("ok".to_string()));
    payload
        .entry("schema_version".to_string())
        .or_insert_with(|| Value::String("runtime-trace-v2".to_string()));
    Value::Object(payload)
        .as_object()
        .cloned()
        .unwrap_or_default()
}

fn trace_event_matches_scope(
    payload: &Map<String, Value>,
    session_id: Option<&str>,
    job_id: Option<&str>,
    stream_scope_fields: Option<&[String]>,
) -> bool {
    let session_scoped = stream_scope_fields
        .map(|fields| fields.iter().any(|field| field == "session_id"))
        .unwrap_or(true);
    let job_scoped = stream_scope_fields
        .map(|fields| fields.iter().any(|field| field == "job_id"))
        .unwrap_or(true);
    if session_scoped {
        if let Some(expected_session_id) = session_id {
            if trace_event_string_field(payload, "session_id").as_deref()
                != Some(expected_session_id)
            {
                return false;
            }
        }
    }
    if job_scoped {
        if let Some(expected_job_id) = job_id {
            if trace_event_string_field(payload, "job_id").as_deref() != Some(expected_job_id) {
                return false;
            }
        }
    }
    true
}

fn trace_scope_fields(payload: &Option<Vec<String>>) -> Option<&[String]> {
    payload.as_deref().filter(|fields| !fields.is_empty())
}

fn trace_event_matches_request_scope(
    payload: &Map<String, Value>,
    session_id: Option<&str>,
    job_id: Option<&str>,
    stream_scope_fields: &Option<Vec<String>>,
) -> bool {
    trace_event_matches_scope(
        payload,
        session_id,
        job_id,
        trace_scope_fields(stream_scope_fields),
    )
}

fn load_trace_stream_events(
    path: &Path,
    event_stream_text: Option<&str>,
    session_id: Option<&str>,
    job_id: Option<&str>,
    stream_scope_fields: &Option<Vec<String>>,
) -> Result<Vec<Map<String, Value>>, String> {
    let mut events = Vec::new();
    let raw_payload = match event_stream_text {
        Some(value) => value.to_string(),
        None => {
            let storage_backend = resolve_storage_backend(&[path.to_path_buf()]);
            storage_read_text(path, storage_backend.as_ref())?
        }
    };

    for (line_number, raw_line) in raw_payload.lines().enumerate() {
        if raw_line.trim().is_empty() {
            continue;
        }
        let event_payload = hydrate_trace_event_object(
            trace_event_object(serde_json::from_str::<Value>(raw_line).map_err(|err| {
                format!("parse trace stream line {} failed: {err}", line_number + 1)
            })?)?,
            line_number + 1,
        );
        if trace_event_matches_request_scope(
            &event_payload,
            session_id,
            job_id,
            stream_scope_fields,
        ) {
            events.push(event_payload);
        }
    }
    Ok(events)
}

fn latest_cursor_from_trace_event(payload: &Map<String, Value>) -> Option<Value> {
    let session_id = trace_event_string_field(payload, "session_id")?;
    let seq = trace_event_usize_field(payload, "seq")?;
    let generation = trace_event_usize_field(payload, "generation").unwrap_or(0);
    let event_id = trace_event_string_field(payload, "event_id")?;
    let cursor = trace_event_string_field(payload, "cursor")
        .unwrap_or_else(|| build_trace_cursor(generation, seq, &event_id));
    Some(json!({
        "schema_version": "runtime-trace-cursor-v1",
        "session_id": session_id,
        "job_id": trace_event_string_field(payload, "job_id"),
        "generation": generation,
        "seq": seq,
        "event_id": event_id,
        "cursor": cursor,
    }))
}

fn compaction_delta_to_trace_event(
    payload: Value,
    line_number: usize,
) -> Result<Map<String, Value>, String> {
    let object = payload.as_object().cloned().ok_or_else(|| {
        format!(
            "trace compaction delta line {} must decode to a JSON object",
            line_number
        )
    })?;
    let generation = trace_event_usize_field(&object, "generation").unwrap_or(0);
    let seq = trace_event_usize_field(&object, "seq")
        .ok_or_else(|| format!("trace compaction delta line {line_number} missing seq"))?;
    let applies_to = object
        .get("applies_to")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("trace compaction delta line {line_number} missing applies_to"))?;
    let session_id = applies_to
        .get("session_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!("trace compaction delta line {line_number} missing applies_to.session_id")
        })?;
    let payload_object = object
        .get("payload")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("trace compaction delta line {line_number} missing payload"))?;
    let event_id = payload_object
        .get("event_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("evt_replay_{line_number:06}"));
    let cursor = payload_object
        .get("cursor")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| build_trace_cursor(generation, seq, &event_id));

    let mut event = Map::new();
    event.insert("event_id".to_string(), Value::String(event_id));
    event.insert("seq".to_string(), json!(seq));
    event.insert("generation".to_string(), json!(generation));
    event.insert("cursor".to_string(), Value::String(cursor));
    event.insert(
        "ts".to_string(),
        object
            .get("ts")
            .cloned()
            .unwrap_or_else(|| Value::String(String::new())),
    );
    event.insert(
        "session_id".to_string(),
        Value::String(session_id.to_string()),
    );
    event.insert(
        "job_id".to_string(),
        applies_to.get("job_id").cloned().unwrap_or(Value::Null),
    );
    event.insert(
        "kind".to_string(),
        object
            .get("kind")
            .cloned()
            .unwrap_or_else(|| Value::String(String::new())),
    );
    event.insert(
        "stage".to_string(),
        payload_object
            .get("stage")
            .cloned()
            .unwrap_or_else(|| Value::String("background".to_string())),
    );
    event.insert(
        "status".to_string(),
        payload_object
            .get("status")
            .cloned()
            .unwrap_or_else(|| Value::String("ok".to_string())),
    );
    event.insert(
        "payload".to_string(),
        payload_object
            .get("payload")
            .cloned()
            .unwrap_or_else(|| json!({})),
    );
    event.insert(
        "schema_version".to_string(),
        Value::String("runtime-trace-v2".to_string()),
    );
    Ok(event)
}

fn validate_compaction_artifact_digest(
    artifact_ref: &Map<String, Value>,
    payload_text: &str,
    label: &str,
) -> Result<(), String> {
    let expected = artifact_ref
        .get("digest")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("compaction {label} artifact ref is missing digest"))?;
    let actual = sha256_hex(payload_text.as_bytes());
    if expected != actual {
        return Err(format!(
            "Compaction recovery failed closed because {label} artifact digest mismatched."
        ));
    }
    Ok(())
}

fn sha256_hex(payload: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(payload);
    format!("{:x}", hasher.finalize())
}

struct ResolvedTraceSource {
    path: PathBuf,
    source_kind: &'static str,
    events: Vec<Map<String, Value>>,
    latest_cursor: Option<Value>,
    latest_event_id: Option<String>,
    latest_event_kind: Option<String>,
    latest_event_timestamp: Option<String>,
    recovery: Option<Value>,
}

struct TraceSourceRequest<'a> {
    path: Option<&'a str>,
    event_stream_text: Option<&'a str>,
    compaction_manifest_path: Option<&'a str>,
    compaction_manifest_text: Option<&'a str>,
    compaction_state_text: Option<&'a str>,
    compaction_artifact_index_text: Option<&'a str>,
    compaction_delta_text: Option<&'a str>,
    session_id: Option<&'a str>,
    job_id: Option<&'a str>,
    stream_scope_fields: &'a Option<Vec<String>>,
}

struct CompactionRecoveryRequest<'a> {
    manifest_path: &'a Path,
    manifest_text: Option<&'a str>,
    state_text: Option<&'a str>,
    artifact_index_text: Option<&'a str>,
    delta_text: Option<&'a str>,
    session_id: Option<&'a str>,
    job_id: Option<&'a str>,
    stream_scope_fields: &'a Option<Vec<String>>,
}

impl<'a> TraceSourceRequest<'a> {
    fn from_metadata_payload(payload: &'a TraceMetadataWriteRequestPayload) -> Self {
        Self {
            path: payload.event_stream_path.as_deref(),
            event_stream_text: payload.event_stream_text.as_deref(),
            compaction_manifest_path: payload.compaction_manifest_path.as_deref(),
            compaction_manifest_text: payload.compaction_manifest_text.as_deref(),
            compaction_state_text: payload.compaction_state_text.as_deref(),
            compaction_artifact_index_text: payload.compaction_artifact_index_text.as_deref(),
            compaction_delta_text: payload.compaction_delta_text.as_deref(),
            session_id: payload.session_id.as_deref(),
            job_id: payload.job_id.as_deref(),
            stream_scope_fields: &payload.stream_scope_fields,
        }
    }

    fn from_inspect_payload(payload: &'a TraceStreamInspectRequestPayload) -> Self {
        Self {
            path: payload.path.as_deref(),
            event_stream_text: payload.event_stream_text.as_deref(),
            compaction_manifest_path: payload.compaction_manifest_path.as_deref(),
            compaction_manifest_text: payload.compaction_manifest_text.as_deref(),
            compaction_state_text: payload.compaction_state_text.as_deref(),
            compaction_artifact_index_text: payload.compaction_artifact_index_text.as_deref(),
            compaction_delta_text: payload.compaction_delta_text.as_deref(),
            session_id: payload.session_id.as_deref(),
            job_id: payload.job_id.as_deref(),
            stream_scope_fields: &payload.stream_scope_fields,
        }
    }

    fn from_replay_payload(payload: &'a TraceStreamReplayRequestPayload) -> Self {
        Self {
            path: payload.path.as_deref(),
            event_stream_text: payload.event_stream_text.as_deref(),
            compaction_manifest_path: payload.compaction_manifest_path.as_deref(),
            compaction_manifest_text: payload.compaction_manifest_text.as_deref(),
            compaction_state_text: payload.compaction_state_text.as_deref(),
            compaction_artifact_index_text: payload.compaction_artifact_index_text.as_deref(),
            compaction_delta_text: payload.compaction_delta_text.as_deref(),
            session_id: payload.session_id.as_deref(),
            job_id: payload.job_id.as_deref(),
            stream_scope_fields: &payload.stream_scope_fields,
        }
    }
}

fn load_compaction_recovery(
    request: CompactionRecoveryRequest<'_>,
) -> Result<ResolvedTraceSource, String> {
    let CompactionRecoveryRequest {
        manifest_path,
        manifest_text,
        state_text,
        artifact_index_text,
        delta_text,
        session_id,
        job_id,
        stream_scope_fields,
    } = request;
    let storage_backend = resolve_storage_backend(&[manifest_path.to_path_buf()]);
    let manifest_raw = match manifest_text {
        Some(value) => value.to_string(),
        None => storage_read_text(manifest_path, storage_backend.as_ref())?,
    };
    let manifest_payload = serde_json::from_str::<Value>(&manifest_raw).map_err(|err| {
        format!(
            "parse compaction manifest failed for {}: {err}",
            manifest_path.display()
        )
    })?;
    let manifest = manifest_payload.as_object().ok_or_else(|| {
        format!(
            "compaction manifest must decode to a JSON object: {}",
            manifest_path.display()
        )
    })?;
    let snapshot = manifest
        .get("latest_stable_snapshot")
        .and_then(Value::as_object)
        .ok_or_else(|| "compaction manifest is missing latest_stable_snapshot".to_string())?;
    let state_ref = snapshot
        .get("state_ref")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "compaction manifest is missing required recovery artifact refs.".to_string()
        })?;
    let artifact_index_ref = snapshot
        .get("artifact_index_ref")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "compaction manifest is missing required recovery artifact refs.".to_string()
        })?;
    let state_ref_uri = state_ref
        .get("uri")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "compaction manifest is missing required recovery artifact refs.".to_string()
        })?;
    let artifact_index_uri = artifact_index_ref
        .get("uri")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "compaction manifest is missing required recovery artifact refs.".to_string()
        })?;
    let state_path = PathBuf::from(state_ref_uri);
    let artifact_index_path = PathBuf::from(artifact_index_uri);
    if state_text.is_none()
        && artifact_index_text.is_none()
        && (!storage_artifact_exists(&state_path, storage_backend.as_ref())
            || !storage_artifact_exists(&artifact_index_path, storage_backend.as_ref()))
    {
        return Err(
            "Compaction recovery failed closed because a referenced artifact is missing."
                .to_string(),
        );
    }
    let state_raw = match state_text {
        Some(value) => value.to_string(),
        None => storage_read_text(&state_path, storage_backend.as_ref())?,
    };
    validate_compaction_artifact_digest(state_ref, &state_raw, "state_ref")?;
    let state_payload = serde_json::from_str::<Value>(&state_raw).map_err(|err| {
        format!(
            "parse compaction state failed for {}: {err}",
            state_path.display()
        )
    })?;
    let artifact_index_raw = match artifact_index_text {
        Some(value) => value.to_string(),
        None => storage_read_text(&artifact_index_path, storage_backend.as_ref())?,
    };
    validate_compaction_artifact_digest(
        artifact_index_ref,
        &artifact_index_raw,
        "artifact_index_ref",
    )?;
    let artifact_index_payload =
        serde_json::from_str::<Value>(&artifact_index_raw).map_err(|err| {
            format!(
                "parse compaction artifact index failed for {}: {err}",
                artifact_index_path.display()
            )
        })?;

    let delta_path = manifest
        .get("delta_path")
        .and_then(Value::as_str)
        .map(PathBuf::from);
    let mut deltas = Vec::new();
    let mut events = Vec::new();
    if let Some(delta_path) = delta_path.as_ref() {
        let raw_delta_payload = delta_text.map(str::to_string).or_else(|| {
            if storage_artifact_exists(delta_path, storage_backend.as_ref()) {
                storage_read_text(delta_path, storage_backend.as_ref()).ok()
            } else {
                None
            }
        });
        if let Some(raw_delta_payload) = raw_delta_payload {
            for (line_number, raw_line) in raw_delta_payload.lines().enumerate() {
                if raw_line.trim().is_empty() {
                    continue;
                }
                let delta_payload = serde_json::from_str::<Value>(raw_line).map_err(|err| {
                    format!(
                        "parse compaction delta line {} failed: {err}",
                        line_number + 1
                    )
                })?;
                let event_payload =
                    compaction_delta_to_trace_event(delta_payload.clone(), line_number + 1)?;
                if trace_event_matches_request_scope(
                    &event_payload,
                    session_id,
                    job_id,
                    stream_scope_fields,
                ) {
                    deltas.push(delta_payload);
                    events.push(event_payload);
                }
            }
        }
    }

    let latest_cursor = events
        .last()
        .and_then(latest_cursor_from_trace_event)
        .or_else(|| state_payload.get("latest_cursor").cloned());
    let latest_event = events.last().cloned().or_else(|| {
        state_payload
            .get("latest_event")
            .and_then(Value::as_object)
            .cloned()
    });
    let latest_event_id = latest_cursor
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|payload| payload.get("event_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            latest_event
                .as_ref()
                .and_then(|payload| trace_event_string_field(payload, "event_id"))
        });
    let latest_event_kind = latest_event
        .as_ref()
        .and_then(|payload| trace_event_string_field(payload, "kind"));
    let latest_event_timestamp = latest_event
        .as_ref()
        .and_then(|payload| trace_event_string_field(payload, "ts"));
    let latest_recoverable_generation = latest_cursor
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|payload| payload.get("generation"))
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .or_else(|| {
            manifest
                .get("active_generation")
                .and_then(Value::as_u64)
                .map(|value| value as usize)
        })
        .unwrap_or(0);
    let recovery = Some(json!({
        "schema_version": "runtime-trace-compaction-recovery-v1",
        "session_id": manifest.get("session_id").cloned().unwrap_or(Value::Null),
        "job_id": manifest.get("job_id").cloned().unwrap_or(Value::Null),
        "latest_recoverable_generation": latest_recoverable_generation,
        "snapshot": Value::Object(snapshot.clone()),
        "deltas": deltas,
        "artifact_index": artifact_index_payload,
        "state": state_payload,
        "latest_cursor": latest_cursor.clone(),
    }));
    Ok(ResolvedTraceSource {
        path: manifest_path.to_path_buf(),
        source_kind: "compaction_manifest",
        events,
        latest_cursor,
        latest_event_id,
        latest_event_kind,
        latest_event_timestamp,
        recovery,
    })
}

fn resolve_trace_source(request: TraceSourceRequest<'_>) -> Result<ResolvedTraceSource, String> {
    if request.compaction_manifest_path.is_some() || request.compaction_manifest_text.is_some() {
        let compaction_path = request
            .compaction_manifest_path
            .unwrap_or("<inline-compaction-manifest>");
        return load_compaction_recovery(CompactionRecoveryRequest {
            manifest_path: &PathBuf::from(compaction_path),
            manifest_text: request.compaction_manifest_text,
            state_text: request.compaction_state_text,
            artifact_index_text: request.compaction_artifact_index_text,
            delta_text: request.compaction_delta_text,
            session_id: request.session_id,
            job_id: request.job_id,
            stream_scope_fields: request.stream_scope_fields,
        });
    }
    let path = request
        .path
        .or(if request.event_stream_text.is_some() {
            Some("<inline-trace-stream>")
        } else {
            None
        })
        .ok_or_else(|| {
            "trace stream replay requires path, event_stream_text, compaction_manifest_path, or compaction_manifest_text".to_string()
        })?;
    let path_buf = PathBuf::from(path);
    let events = load_trace_stream_events(
        &path_buf,
        request.event_stream_text,
        request.session_id,
        request.job_id,
        request.stream_scope_fields,
    )?;
    let latest_event = events.last();
    Ok(ResolvedTraceSource {
        path: path_buf,
        source_kind: "trace_stream",
        latest_cursor: latest_event.and_then(latest_cursor_from_trace_event),
        latest_event_id: latest_event
            .and_then(|payload| trace_event_string_field(payload, "event_id")),
        latest_event_kind: latest_event
            .and_then(|payload| trace_event_string_field(payload, "kind")),
        latest_event_timestamp: latest_event
            .and_then(|payload| trace_event_string_field(payload, "ts")),
        recovery: None,
        events,
    })
}

pub(crate) fn inspect_trace_stream(
    payload: TraceStreamInspectRequestPayload,
) -> Result<TraceStreamInspectResponsePayload, String> {
    let resolved = resolve_trace_source(TraceSourceRequest::from_inspect_payload(&payload))?;
    let reroute_count = trace_reroute_count(&resolved.events);
    let retry_count = trace_retry_count(&resolved.events);

    Ok(TraceStreamInspectResponsePayload {
        schema_version: TRACE_STREAM_INSPECT_SCHEMA_VERSION.to_string(),
        authority: TRACE_STREAM_IO_AUTHORITY.to_string(),
        path: resolved.path.display().to_string(),
        source_kind: resolved.source_kind.to_string(),
        event_count: resolved.events.len(),
        latest_event_id: resolved.latest_event_id,
        latest_event_kind: resolved.latest_event_kind,
        latest_event_timestamp: resolved.latest_event_timestamp,
        latest_cursor: resolved.latest_cursor,
        recovery: resolved.recovery,
        reroute_count,
        retry_count,
    })
}

pub(crate) fn replay_trace_stream(
    payload: TraceStreamReplayRequestPayload,
) -> Result<TraceStreamReplayResponsePayload, String> {
    let resolved = resolve_trace_source(TraceSourceRequest::from_replay_payload(&payload))?;
    let after_event_id = payload.after_event_id.clone();
    let limit = payload.limit.unwrap_or(usize::MAX);
    let mut anchor_found = after_event_id.is_none();
    let mut anchor_index = None;
    let mut next_cursor = None;
    let mut events = Vec::new();

    for (current_index, event_payload) in resolved.events.iter().enumerate() {
        let event_id = trace_event_string_field(event_payload, "event_id");
        if !anchor_found {
            if event_id.as_deref() == after_event_id.as_deref() {
                anchor_found = true;
                anchor_index = Some(current_index);
                continue;
            }
            continue;
        }
        if events.len() >= limit {
            continue;
        }
        next_cursor = Some(TraceStreamReplayCursorPayload {
            event_id: event_id.clone(),
            event_index: current_index,
        });
        events.push(Value::Object(event_payload.clone()));
    }

    if after_event_id.is_some() && !anchor_found {
        return Err(format!(
            "Unknown event id for stream resume: {}",
            after_event_id.unwrap_or_default()
        ));
    }

    let window_start_index = anchor_index.map_or(0, |index| index + 1);
    let has_more = resolved.events.len() > window_start_index + events.len();
    Ok(TraceStreamReplayResponsePayload {
        schema_version: TRACE_STREAM_REPLAY_SCHEMA_VERSION.to_string(),
        authority: TRACE_STREAM_IO_AUTHORITY.to_string(),
        path: resolved.path.display().to_string(),
        source_kind: resolved.source_kind.to_string(),
        event_count: resolved.events.len(),
        latest_event_id: resolved.latest_event_id,
        latest_event_kind: resolved.latest_event_kind,
        latest_event_timestamp: resolved.latest_event_timestamp,
        latest_cursor: resolved.latest_cursor,
        after_event_id,
        window_start_index,
        has_more,
        next_cursor,
        events,
    })
}

fn trace_reroute_count(events: &[Map<String, Value>]) -> usize {
    events
        .iter()
        .filter(|event| {
            trace_event_string_field(event, "kind").as_deref() == Some("route.selected")
        })
        .count()
        .saturating_sub(1)
}

fn trace_retry_count(events: &[Map<String, Value>]) -> usize {
    events
        .iter()
        .filter(|event| trace_event_string_field(event, "kind").as_deref() == Some("run.failed"))
        .count()
}

fn build_trace_stream_metadata(
    trace: &ResolvedTraceSource,
    control_plane: Option<&Value>,
) -> Value {
    let latest = trace.events.last();
    let mut stream = Map::new();
    stream.insert(
        "generation".to_string(),
        json!(latest
            .and_then(|event| trace_event_usize_field(event, "generation"))
            .unwrap_or(0)),
    );
    stream.insert("replay_supported".to_string(), Value::Bool(true));
    stream.insert("event_stream_supported".to_string(), Value::Bool(true));
    stream.insert(
        "event_stream_schema_version".to_string(),
        Value::String("runtime-event-stream-v1".to_string()),
    );
    if let Some(control_plane) = control_plane.and_then(Value::as_object) {
        let field_map = [
            ("authority", "control_plane_authority"),
            ("role", "control_plane_role"),
            ("projection", "control_plane_projection"),
            ("delegate_kind", "control_plane_delegate_kind"),
            ("ownership_lane", "ownership_lane"),
            ("producer_owner", "producer_owner"),
            ("producer_authority", "producer_authority"),
            ("exporter_owner", "exporter_owner"),
            ("exporter_authority", "exporter_authority"),
            ("transport_family", "transport_family"),
            ("resume_mode", "resume_mode"),
            ("stream_scope_fields", "stream_scope_fields"),
            ("cleanup_scope_fields", "cleanup_scope_fields"),
        ];
        for (source, target) in field_map {
            if let Some(value) = control_plane.get(source) {
                stream.insert(target.to_string(), value.clone());
            }
        }
    }
    stream.insert(
        "event_stream_path".to_string(),
        Value::String(trace.path.display().to_string()),
    );
    stream.insert("compaction_manifest_path".to_string(), Value::Null);
    stream.insert("event_count".to_string(), json!(trace.events.len()));
    stream.insert(
        "latest_seq".to_string(),
        json!(latest
            .and_then(|event| trace_event_usize_field(event, "seq"))
            .unwrap_or(0)),
    );
    stream.insert(
        "latest_event_id".to_string(),
        trace
            .latest_event_id
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    stream.insert(
        "latest_cursor".to_string(),
        trace.latest_cursor.clone().unwrap_or(Value::Null),
    );
    Value::Object(stream)
}

fn write_trace_compaction_delta(
    payload: TraceCompactionDeltaWriteRequestPayload,
) -> Result<TraceCompactionDeltaWriteResponsePayload, String> {
    let path = PathBuf::from(&payload.path);
    let serialized = serde_json::to_string(&payload.delta)
        .map_err(|err| format!("serialize trace compaction delta failed: {err}"))?
        + "\n";
    append_text_with_process_lock(&path, &serialized, "trace compaction delta")?;
    Ok(TraceCompactionDeltaWriteResponsePayload {
        schema_version: TRACE_COMPACTION_DELTA_WRITE_SCHEMA_VERSION.to_string(),
        authority: TRACE_STREAM_IO_AUTHORITY.to_string(),
        path: path.display().to_string(),
        bytes_written: serialized.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::{
        evaluate_routing_cases, load_records_cached_for_stdio_with_default_runtime_path,
        load_routing_eval_cases, read_json, value_to_string,
    };
    use std::sync::Arc;
    use std::thread::{sleep, spawn};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/routing_route_fixtures.json")
    }

    fn routing_eval_case_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/routing_eval_cases.json")
    }

    fn assert_routing_eval_cases_match<F>(label: &str, mut route_case: F)
    where
        F: FnMut(&str, &str, bool) -> Result<RouteDecision, String>,
    {
        let payload = read_json(&routing_eval_case_path()).expect("read routing eval fixture");
        let cases = payload
            .get("cases")
            .and_then(Value::as_array)
            .expect("routing eval cases array");
        let mut failures = Vec::new();

        for (index, case) in cases.iter().enumerate() {
            let id = case
                .get("id")
                .map(value_to_string)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| (index + 1).to_string());
            let task = case
                .get("task")
                .and_then(Value::as_str)
                .expect("routing eval task");
            let first_turn = case
                .get("first_turn")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let decision = route_case(task, &format!("routing-eval::{label}::{id}"), first_turn)
                .unwrap_or_else(|err| panic!("route eval {label}/{id} failed: {err}"));

            if let Some(expected_owner) = case.get("expected_owner").and_then(Value::as_str) {
                if decision.selected_skill != expected_owner {
                    failures.push(format!(
                        "{id}: expected owner {expected_owner}, got {} (score {})",
                        decision.selected_skill, decision.score
                    ));
                }
            }

            let expected_overlay = case
                .get("expected_overlay")
                .and_then(Value::as_str)
                .map(|value| value.to_string());
            if decision.overlay_skill != expected_overlay {
                failures.push(format!(
                    "{id}: expected overlay {:?}, got {:?} (owner {}, score {})",
                    expected_overlay,
                    decision.overlay_skill,
                    decision.selected_skill,
                    decision.score
                ));
            }

            if case
                .get("forbidden_owners")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .any(|forbidden| forbidden == decision.selected_skill)
                })
                .unwrap_or(false)
            {
                failures.push(format!(
                    "{id}: selected forbidden owner {} (score {})",
                    decision.selected_skill, decision.score
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{label} routing eval strict failures:\n{}",
            failures.join("\n")
        );
    }

    fn sample_execute_request() -> ExecuteRequestPayload {
        ExecuteRequestPayload {
            schema_version: "router-rs-execute-request-v1".to_string(),
            task: "帮我继续推进 Rust kernel".to_string(),
            session_id: "execute-session".to_string(),
            user_id: "tester".to_string(),
            selected_skill: "plan-to-code".to_string(),
            overlay_skill: Some("rust-pro".to_string()),
            layer: "L2".to_string(),
            route_engine: Some("rust".to_string()),
            diagnostic_route_mode: Some("none".to_string()),
            reasons: vec!["Trigger phrase matched: 直接做代码.".to_string()],
            prompt_preview: Some("Keep the kernel Rust-first.".to_string()),
            dry_run: true,
            trace_event_count: 6,
            trace_output_path: Some("/tmp/TRACE_METADATA.json".to_string()),
            default_output_tokens: 512,
            model_id: "gpt-5.4".to_string(),
            aggregator_base_url: "http://127.0.0.1:20128/v1".to_string(),
            aggregator_api_key: "test-key".to_string(),
        }
    }

    fn temp_trace_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("router-rs-{name}-{nonce}.jsonl"))
    }

    fn temp_json_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("router-rs-{name}-{nonce}.json"))
    }

    fn temp_dir_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("router-rs-{name}-{nonce}"))
    }

    fn write_text_fixture(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create fixture parent");
        }
        fs::write(path, content).expect("write text fixture");
    }

    fn write_runtime_fixture(path: &Path, slug: &str) {
        fs::write(
            path,
            json!({
                "keys": ["slug", "layer", "owner", "gate", "summary", "trigger_hints", "health", "priority", "session_start"],
                "skills": [[slug, "L2", "primary", "none", format!("{slug} summary"), ["trigger"], 100.0, "P1", "always"]]
            })
            .to_string(),
        )
        .expect("write runtime fixture");
    }

    fn write_manifest_fixture(path: &Path, slug: &str, priority: &str) {
        fs::write(
            path,
            json!({
                "keys": ["slug", "description", "layer", "owner", "gate", "trigger_hints", "health", "priority", "session_start"],
                "skills": [[slug, format!("{slug} manifest"), "L2", "primary", "none", ["trigger"], 100.0, priority, "always"]]
            })
            .to_string(),
        )
        .expect("write manifest fixture");
    }

    #[test]
    fn stdio_request_dispatches_route_policy_payload() {
        let response =
            handle_stdio_json_line(r#"{"id":1,"op":"route_policy","payload":{"mode":"verify"}}"#);
        assert!(response.ok);
        assert_eq!(response.id, json!(1));
        assert_eq!(
            response.payload.expect("payload")["policy_schema_version"],
            json!(ROUTE_POLICY_SCHEMA_VERSION)
        );
    }

    #[test]
    fn stdio_request_dispatches_hook_policy_payload() {
        let response = handle_stdio_json_line(
            r#"{"id":1,"op":"hook_policy","payload":{"operation":"validation-categories","command":"python3 -m json.tool .claude/settings.json"}}"#,
        );
        assert!(response.ok, "{}", response.error.unwrap_or_default());
        let payload = response.payload.expect("payload");
        assert_eq!(payload["categories"], json!(["config", "json"]));
    }

    #[test]
    fn stdio_request_dispatches_concurrency_defaults_payload() {
        let response =
            handle_stdio_json_line(r#"{"id":1,"op":"concurrency_defaults","payload":{}}"#);
        assert!(response.ok);
        let payload = response.payload.expect("payload");
        assert_eq!(
            payload["router_stdio"]["default_pool_size"],
            json!(DEFAULT_ROUTER_STDIO_POOL_SIZE)
        );
        assert_eq!(
            payload["router_stdio"]["max_pool_size"],
            json!(MAX_ROUTER_STDIO_POOL_SIZE)
        );
        assert_eq!(
            payload["max_background_jobs"],
            json!(DEFAULT_MAX_BACKGROUND_JOBS)
        );
        assert_eq!(
            payload["max_concurrent_subagents"],
            json!(DEFAULT_MAX_CONCURRENT_SUBAGENTS)
        );
    }

    #[test]
    fn stdio_request_dispatches_execute_payload() {
        let payload =
            serde_json::to_string(&sample_execute_request()).expect("serialize execute payload");
        let response = handle_stdio_json_line(&format!(
            "{{\"id\":3,\"op\":\"execute\",\"payload\":{payload}}}"
        ));
        assert!(response.ok);
        assert_eq!(response.id, json!(3));
        let payload = response.payload.expect("payload");
        assert_eq!(
            payload["execution_schema_version"],
            json!(EXECUTION_SCHEMA_VERSION)
        );
        assert_eq!(payload["authority"], json!(EXECUTION_AUTHORITY));
        assert_eq!(payload["live_run"], json!(false));
    }

    #[test]
    fn framework_refresh_copies_compact_prompt_to_configured_file() {
        let repo_root = std::env::temp_dir().join(format!(
            "router-rs-refresh-fixture-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock before epoch")
                .as_nanos()
        ));
        let memory_root = repo_root.join(".codex").join("memory");
        let task_root = repo_root
            .join("artifacts")
            .join("current")
            .join("active-bootstrap-repair-20260418210000");
        fs::create_dir_all(&memory_root).expect("create memory root");
        fs::create_dir_all(&task_root).expect("create task root");
        fs::write(
            task_root.join("SESSION_SUMMARY.md"),
            "- task: active bootstrap repair\n- phase: implementation\n- status: in_progress\n",
        )
        .expect("write session summary");
        fs::write(
            task_root.join("NEXT_ACTIONS.json"),
            r#"{"next_actions":["Patch classifier","Run MCP regression tests"]}"#,
        )
        .expect("write next actions");
        fs::write(task_root.join("EVIDENCE_INDEX.json"), r#"{"artifacts":[]}"#)
            .expect("write evidence index");
        fs::write(
            task_root.join("TRACE_METADATA.json"),
            r#"{"task":"active bootstrap repair","matched_skills":["plan-to-code","skill-framework-developer"]}"#,
        )
        .expect("write trace metadata");
        fs::create_dir_all(repo_root.join("artifacts").join("current"))
            .expect("create current root");
        fs::write(
            repo_root.join("artifacts").join("current").join("active_task.json"),
            r#"{"task_id":"active-bootstrap-repair-20260418210000","task":"active bootstrap repair"}"#,
        )
        .expect("write active task");
        fs::write(
            repo_root.join(".supervisor_state.json"),
            r#"{
                "task_id":"active-bootstrap-repair-20260418210000",
                "task_summary":"active bootstrap repair",
                "active_phase":"implementation",
                "verification":{"verification_status":"in_progress"},
                "continuity":{"story_state":"active","resume_allowed":true},
                "primary_owner":"skill-framework-developer",
                "execution_contract":{
                    "goal":"Repair stale bootstrap injection",
                    "scope":["scripts/router-rs/src/framework_runtime.rs"],
                    "acceptance_criteria":["completed tasks never appear as current execution"]
                },
                "blockers":{"open_blockers":["Need regression coverage"]}
            }"#,
        )
        .expect("write supervisor state");
        fs::write(
            memory_root.join("MEMORY.md"),
            "# 项目长期记忆\n\n## Active Patterns\n\n- AP-1: Externalize task state\n",
        )
        .expect("write memory");

        let clipboard_path = repo_root.join("clipboard.txt");
        std::env::set_var("ROUTER_RS_CLIPBOARD_PATH", &clipboard_path);
        let refresh =
            build_framework_refresh_payload(&repo_root, 6, false).expect("build refresh payload");
        let prompt = refresh
            .get("prompt")
            .and_then(Value::as_str)
            .expect("refresh prompt");
        let clipboard = copy_text_to_clipboard(prompt).expect("copy prompt");
        std::env::remove_var("ROUTER_RS_CLIPBOARD_PATH");

        let copied = fs::read_to_string(&clipboard_path).expect("read clipboard file");
        assert_eq!(clipboard["backend"], json!("file"));
        assert!(copied.contains("继续当前仓库，先看这些恢复锚点："));
        assert!(copied.contains("先做："));
        assert!(copied.contains("按既定串并行分工直接开始执行。"));
        assert!(!copied.contains("当前上下文："));
        assert!(!copied.contains("必须先做的下一步："));

        let _ = fs::remove_dir_all(&repo_root);
    }

    #[test]
    fn framework_refresh_completed_task_uses_plain_closeout_wording() {
        let repo_root = std::env::temp_dir().join(format!(
            "router-rs-refresh-completed-fixture-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock before epoch")
                .as_nanos()
        ));
        let memory_root = repo_root.join(".codex").join("memory");
        let task_root = repo_root
            .join("artifacts")
            .join("current")
            .join("completed-rerun-20260423");
        fs::create_dir_all(&memory_root).expect("create memory root");
        fs::create_dir_all(&task_root).expect("create task root");
        fs::create_dir_all(repo_root.join("artifacts").join("current"))
            .expect("create current root");
        fs::write(
            task_root.join("SESSION_SUMMARY.md"),
            "- task: bounded rerun\n- phase: closeout\n- status: completed\n",
        )
        .expect("write session summary");
        fs::write(
            task_root.join("NEXT_ACTIONS.json"),
            r#"{"next_actions":[]}"#,
        )
        .expect("write next actions");
        fs::write(task_root.join("EVIDENCE_INDEX.json"), r#"{"artifacts":[]}"#)
            .expect("write evidence index");
        fs::write(
            task_root.join("TRACE_METADATA.json"),
            r#"{"task":"bounded rerun","verification_status":"completed"}"#,
        )
        .expect("write trace metadata");
        fs::write(
            repo_root
                .join("artifacts")
                .join("current")
                .join("active_task.json"),
            r#"{"task_id":"completed-rerun-20260423","task":"bounded rerun"}"#,
        )
        .expect("write active task");
        fs::write(
            repo_root.join(".supervisor_state.json"),
            r#"{
                "task_id":"completed-rerun-20260423",
                "task_summary":"bounded rerun",
                "active_phase":"closeout",
                "verification":{"verification_status":"completed","last_verification_summary":"262 passed"},
                "continuity":{"story_state":"completed","resume_allowed":false},
                "next_actions":[],
                "execution_contract":{"goal":"Re-run bounded verification"}
            }"#,
        )
        .expect("write supervisor state");
        fs::write(memory_root.join("MEMORY.md"), "# 项目长期记忆\n").expect("write memory");

        let refresh =
            build_framework_refresh_payload(&repo_root, 6, false).expect("build refresh payload");
        let prompt = refresh
            .get("prompt")
            .and_then(Value::as_str)
            .expect("refresh prompt");

        assert!(prompt.contains("最近一轮已经收尾："));
        assert!(prompt.contains("- bounded rerun"));
        assert!(prompt.contains("- 结果已经稳定，可以直接按已完成上下文来看。"));
        assert!(prompt.contains("- 如果还要继续相关工作，先新开一个 standalone task"));
        assert!(prompt.contains("先看这些恢复锚点："));
        assert!(!prompt.contains("剩余："));
        assert!(!prompt.contains("先做："));
        assert!(!prompt.contains("按既定串并行分工直接开始执行。"));
        assert!(!prompt.contains("Keep this task only as recent-completed context"));
        assert!(!prompt.contains("Start a new standalone task before resuming related work"));

        let _ = fs::remove_dir_all(&repo_root);
    }

    #[test]
    fn framework_statusline_uses_rust_runtime_view() {
        let repo_root = temp_dir_path("framework-statusline");
        let task_id = "statusline-task-20260424120000";
        let task_root = repo_root.join("artifacts").join("current").join(task_id);
        write_text_fixture(
            &task_root.join("SESSION_SUMMARY.md"),
            "# SESSION_SUMMARY\n\n- task: Validate status line\n- phase: integration\n- status: in_progress\n",
        );
        write_text_fixture(
            &task_root.join("NEXT_ACTIONS.json"),
            &json!({"next_actions": ["Ship it"]}).to_string(),
        );
        write_text_fixture(
            &task_root.join("EVIDENCE_INDEX.json"),
            &json!({"artifacts": []}).to_string(),
        );
        write_text_fixture(
            &task_root.join("TRACE_METADATA.json"),
            &json!({"matched_skills": ["plan-to-code", "skill-framework-developer"]}).to_string(),
        );
        write_text_fixture(
            &repo_root
                .join("artifacts")
                .join("current")
                .join("active_task.json"),
            &json!({"task_id": task_id, "task": "Validate status line"}).to_string(),
        );
        write_text_fixture(
            &repo_root
                .join("artifacts")
                .join("current")
                .join("focus_task.json"),
            &json!({"task_id": task_id, "task": "Validate status line"}).to_string(),
        );
        write_text_fixture(
            &repo_root
                .join("artifacts")
                .join("current")
                .join("task_registry.json"),
            &json!({
                "schema_version": "task-registry-v1",
                "focus_task_id": task_id,
                "tasks": [
                    {
                        "task_id": task_id,
                        "task": "Validate status line",
                        "phase": "integration",
                        "status": "in_progress",
                        "resume_allowed": true
                    }
                ]
            })
            .to_string(),
        );
        write_text_fixture(
            &repo_root.join(".supervisor_state.json"),
            &json!({
                "task_id": task_id,
                "task_summary": "Validate status line",
                "active_phase": "integration",
                "verification": {"verification_status": "in_progress"},
                "continuity": {"story_state": "active", "resume_allowed": true}
            })
            .to_string(),
        );

        let statusline = build_framework_statusline(&repo_root).expect("build statusline");

        assert!(statusline.contains("task=Validate status line"));
        assert!(statusline.contains("next=/refresh"));
        assert!(statusline.contains("integration/in_progress"));
        assert!(statusline.contains("route=plan-to-code+1"));
        assert!(statusline.contains("others=0"));
        assert!(statusline.contains("resumable=0"));
        assert!(statusline.contains("git=nogit"));
        let _ = fs::remove_dir_all(&repo_root);
    }

    #[test]
    fn framework_snapshot_missing_recovery_anchors_is_not_resumable() {
        let repo_root = temp_dir_path("framework-missing-recovery-anchors");
        let current_root = repo_root.join("artifacts").join("current");
        write_text_fixture(
            &current_root.join("EVIDENCE_INDEX.json"),
            &json!({"artifacts": []}).to_string(),
        );

        let snapshot =
            build_framework_runtime_snapshot_envelope(&repo_root, None, None).expect("snapshot");
        let continuity = &snapshot["runtime_snapshot"]["continuity"];
        let missing_anchors = continuity["missing_recovery_anchors"]
            .as_array()
            .expect("missing anchors array");

        assert_eq!(continuity["state"], json!("missing"));
        assert_eq!(continuity["can_resume"], json!(false));
        assert_eq!(continuity["current_execution"], Value::Null);
        assert!(missing_anchors.contains(&json!("SESSION_SUMMARY")));
        assert!(missing_anchors.contains(&json!("NEXT_ACTIONS")));
        assert!(missing_anchors.contains(&json!("TRACE_METADATA")));

        let _ = fs::remove_dir_all(&repo_root);
    }

    #[test]
    fn framework_session_writer_materializes_complete_focus_continuity() {
        let repo_root = temp_dir_path("framework-session-writer-continuity");
        let output_dir = repo_root.join("artifacts").join("current");
        let payload = json!({
            "repo_root": repo_root,
            "output_dir": output_dir,
            "task_id": "continuity-polish-20260424120000",
            "task": "continuity polish",
            "phase": "implementation",
            "status": "in_progress",
            "summary": "Make Rust continuity recoverable without manual mirror repair.",
            "focus": true,
            "next_actions": ["Run targeted tests"],
            "matched_skills": ["plan-to-code", "rust-pro"],
            "execution_contract": {
                "goal": "Improve continuity artifacts",
                "acceptance_criteria": ["writer emits all recovery anchors"]
            },
            "blockers": ["none"]
        });

        let result = write_framework_session_artifacts(payload).expect("write artifacts");
        let task_id = result["task_id"].as_str().expect("task id");
        let task_root = repo_root.join("artifacts").join("current").join(task_id);

        for path in [
            task_root.join("SESSION_SUMMARY.md"),
            task_root.join("NEXT_ACTIONS.json"),
            task_root.join("EVIDENCE_INDEX.json"),
            task_root.join("TRACE_METADATA.json"),
            task_root.join("CONTINUITY_JOURNAL.json"),
            repo_root.join(".supervisor_state.json"),
            repo_root.join("artifacts/current/active_task.json"),
            repo_root.join("artifacts/current/focus_task.json"),
            repo_root.join("artifacts/current/task_registry.json"),
        ] {
            assert!(path.is_file(), "missing {}", path.display());
        }

        let snapshot =
            build_framework_runtime_snapshot_envelope(&repo_root, None, None).expect("snapshot");
        let runtime = &snapshot["runtime_snapshot"];
        assert_eq!(runtime["active_task_id"], json!(task_id));
        assert_eq!(runtime["continuity"]["state"], json!("active"));
        assert_eq!(runtime["continuity"]["can_resume"], json!(true));
        assert_eq!(runtime["continuity"]["missing_recovery_anchors"], json!([]));

        let supervisor = serde_json::from_str::<Value>(
            &fs::read_to_string(repo_root.join(".supervisor_state.json")).expect("read supervisor"),
        )
        .expect("parse supervisor");
        assert_eq!(supervisor["continuity"]["resume_allowed"], json!(true));
        assert_eq!(
            supervisor["verification"]["verification_status"],
            json!("in_progress")
        );
        assert_eq!(
            supervisor["trace_metadata"]["matched_skills"],
            json!(["plan-to-code", "rust-pro"])
        );
        assert_eq!(
            supervisor["artifact_refs"]["task_root"],
            json!(task_root.display().to_string())
        );
        let active_pointer = serde_json::from_str::<Value>(
            &fs::read_to_string(repo_root.join("artifacts/current/active_task.json"))
                .expect("read active pointer"),
        )
        .expect("parse active pointer");
        assert_eq!(
            active_pointer["session_summary"],
            json!(task_root.join("SESSION_SUMMARY.md").display().to_string())
        );
        for path in [
            repo_root.join("SESSION_SUMMARY.md"),
            repo_root.join("NEXT_ACTIONS.json"),
            repo_root.join("EVIDENCE_INDEX.json"),
            repo_root.join("TRACE_METADATA.json"),
            repo_root.join("CONTINUITY_JOURNAL.json"),
            repo_root.join("artifacts/current/SESSION_SUMMARY.md"),
            repo_root.join("artifacts/current/NEXT_ACTIONS.json"),
            repo_root.join("artifacts/current/EVIDENCE_INDEX.json"),
            repo_root.join("artifacts/current/TRACE_METADATA.json"),
            repo_root.join("artifacts/current/CONTINUITY_JOURNAL.json"),
        ] {
            assert!(!path.exists(), "unexpected mirror {}", path.display());
        }
        let journal = serde_json::from_str::<Value>(
            &fs::read_to_string(task_root.join("CONTINUITY_JOURNAL.json")).expect("read journal"),
        )
        .expect("parse journal");
        assert_eq!(journal["schema_version"], json!("continuity-journal-v1"));
        assert_eq!(journal["checkpoint_count"], json!(1));
        assert!(journal["latest_checkpoint_id"]
            .as_str()
            .is_some_and(|value| value.len() == 64));
        assert!(
            journal["checkpoints"][0]["artifact_hashes"]["supervisor_state"]
                .as_str()
                .is_some_and(|value| value.len() == 64)
        );

        let _ = fs::remove_dir_all(&repo_root);
    }

    #[test]
    fn framework_session_artifact_write_rejects_stale_focus_update() {
        let repo_root = temp_dir_path("framework-session-cas");
        let output_dir = repo_root.join("artifacts").join("current");
        let first = write_framework_session_artifacts(json!({
            "repo_root": repo_root,
            "output_dir": output_dir,
            "task_id": "cas-task",
            "task": "CAS task",
            "phase": "implementation",
            "status": "in_progress",
            "summary": "Initial write.",
            "focus": true,
            "next_actions": ["Continue"]
        }))
        .expect("first write");
        assert_eq!(first["task_id"], json!("cas-task"));

        let focus_path = repo_root.join("artifacts/current/focus_task.json");
        let stale_hash = framework_runtime::hash_file_for_test(&focus_path).expect("focus hash");
        write_text_fixture(
            &focus_path,
            r#"{"task_id":"other-task","task":"Other task","updated_at":"2026-04-25T00:00:00+08:00"}"#,
        );

        let err = write_framework_session_artifacts(json!({
            "repo_root": repo_root,
            "output_dir": output_dir,
            "task_id": "cas-task",
            "task": "CAS task",
            "phase": "implementation",
            "status": "in_progress",
            "summary": "Stale write.",
            "focus": true,
            "expected_focus_task_hash": stale_hash,
            "next_actions": ["Continue"]
        }))
        .expect_err("stale focus update should fail");
        assert!(err.contains("stale focus task pointer update rejected"));

        let focus = read_json(&focus_path).expect("read focus");
        assert_eq!(focus["task_id"], json!("other-task"));
        let _ = fs::remove_dir_all(&repo_root);
    }

    #[test]
    fn framework_session_artifact_write_preserves_existing_roundtrip() {
        let repo_root = temp_dir_path("framework-session-cas-roundtrip");
        let output_dir = repo_root.join("artifacts").join("current");
        let first = write_framework_session_artifacts(json!({
            "repo_root": repo_root,
            "output_dir": output_dir,
            "task_id": "cas-roundtrip",
            "task": "CAS roundtrip",
            "phase": "implementation",
            "status": "in_progress",
            "summary": "Initial write.",
            "focus": true,
            "next_actions": ["Continue"]
        }))
        .expect("first write");
        assert_eq!(first["task_id"], json!("cas-roundtrip"));

        let active_path = repo_root.join("artifacts/current/active_task.json");
        let focus_path = repo_root.join("artifacts/current/focus_task.json");
        let supervisor_path = repo_root.join(".supervisor_state.json");
        let active_hash = framework_runtime::hash_file_for_test(&active_path).expect("active hash");
        let focus_hash = framework_runtime::hash_file_for_test(&focus_path).expect("focus hash");
        let supervisor_hash =
            framework_runtime::hash_file_for_test(&supervisor_path).expect("supervisor hash");

        let second = write_framework_session_artifacts(json!({
            "repo_root": repo_root,
            "output_dir": output_dir,
            "task_id": "cas-roundtrip",
            "task": "CAS roundtrip",
            "phase": "validation",
            "status": "passed",
            "summary": "Validated write.",
            "focus": true,
            "expected_active_task_hash": active_hash,
            "expected_focus_task_hash": focus_hash,
            "expected_supervisor_state_hash": supervisor_hash,
            "next_actions": []
        }))
        .expect("roundtrip write");
        assert_eq!(second["task_id"], json!("cas-roundtrip"));

        let supervisor = read_json(&supervisor_path).expect("read supervisor");
        assert_eq!(supervisor["active_phase"], json!("validation"));
        assert_eq!(
            supervisor["verification"]["verification_status"],
            json!("passed")
        );
        let _ = fs::remove_dir_all(&repo_root);
    }

    #[test]
    fn task_registry_normalization_dedupes_and_limits_old_tasks() {
        let repo_root = temp_dir_path("framework-registry-compact");
        let current_root = repo_root.join("artifacts").join("current");
        let mut tasks = Vec::new();
        for index in 0..140 {
            tasks.push(json!({
                "task_id": format!("task-{index:03}"),
                "task": format!("Task {index:03}"),
                "updated_at": format!("2026-04-24T12:{:02}:00+08:00", index % 60),
                "status": "completed",
                "phase": "closeout",
                "resume_allowed": false
            }));
        }
        tasks.push(json!({
            "task_id": "focus-task",
            "task": "Focus task",
            "updated_at": "2026-04-24T13:00:00+08:00",
            "status": "in_progress",
            "phase": "implementation",
            "resume_allowed": true
        }));
        write_text_fixture(
            &current_root.join("task_registry.json"),
            &json!({
                "schema_version": "task-registry-v1",
                "focus_task_id": "focus-task",
                "tasks": tasks
            })
            .to_string(),
        );

        let changed = framework_runtime::write_framework_session_artifacts(json!({
            "repo_root": repo_root,
            "output_dir": current_root,
            "task_id": "focus-task",
            "task": "Focus task",
            "phase": "implementation",
            "status": "in_progress",
            "focus": true,
            "next_actions": ["Continue"]
        }))
        .expect("write focused task");
        assert_eq!(changed["task_id"], json!("focus-task"));

        let registry = serde_json::from_str::<Value>(
            &fs::read_to_string(current_root.join("task_registry.json")).expect("read registry"),
        )
        .expect("parse registry");
        let tasks = registry["tasks"].as_array().expect("tasks");
        assert_eq!(tasks.len(), 128);
        assert_eq!(registry["truncated"], json!(true));
        assert_eq!(registry["focus_task_id"], json!("focus-task"));
        assert_eq!(tasks[0]["task_id"], json!("focus-task"));
        assert_eq!(registry["recoverable_task_count"], json!(1));

        let _ = fs::remove_dir_all(&repo_root);
    }

    #[test]
    fn framework_alias_builds_compact_autopilot_payload() {
        let repo_root = std::env::temp_dir().join(format!(
            "router-rs-alias-fixture-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock before epoch")
                .as_nanos()
        ));
        let task_root = repo_root
            .join("artifacts")
            .join("current")
            .join("active-bootstrap-repair-20260418210000");
        fs::create_dir_all(&task_root).expect("create task root");
        fs::create_dir_all(repo_root.join("artifacts").join("current"))
            .expect("create current root");
        fs::write(
            task_root.join("SESSION_SUMMARY.md"),
            "- task: active bootstrap repair\n- phase: implementation\n- status: in_progress\n",
        )
        .expect("write session summary");
        fs::write(
            task_root.join("NEXT_ACTIONS.json"),
            r#"{"next_actions":["Patch classifier","Run MCP regression tests"]}"#,
        )
        .expect("write next actions");
        fs::write(task_root.join("EVIDENCE_INDEX.json"), r#"{"artifacts":[]}"#)
            .expect("write evidence index");
        fs::write(
            task_root.join("TRACE_METADATA.json"),
            r#"{"task":"active bootstrap repair","matched_skills":["plan-to-code"]}"#,
        )
        .expect("write trace metadata");
        fs::write(
            repo_root.join("artifacts").join("current").join("active_task.json"),
            r#"{"task_id":"active-bootstrap-repair-20260418210000","task":"active bootstrap repair"}"#,
        )
        .expect("write active task");
        fs::write(
            repo_root.join(".supervisor_state.json"),
            r#"{
                "task_id":"active-bootstrap-repair-20260418210000",
                "task_summary":"active bootstrap repair",
                "active_phase":"implementation",
                "verification":{"verification_status":"in_progress"},
                "continuity":{"story_state":"active","resume_allowed":true},
                "execution_contract":{"acceptance_criteria":["completed tasks never appear as current execution"]}
            }"#,
        )
        .expect("write supervisor state");

        let payload = build_framework_alias_envelope(
            &repo_root,
            "autopilot",
            FrameworkAliasBuildOptions {
                max_lines: 4,
                compact: false,
                host_id: None,
            },
        )
        .expect("build alias payload");
        let alias = payload
            .get("alias")
            .and_then(Value::as_object)
            .expect("alias payload");
        let prompt = alias
            .get("entry_prompt")
            .and_then(Value::as_str)
            .expect("entry prompt");

        assert_eq!(
            payload["schema_version"],
            json!(FRAMEWORK_ALIAS_SCHEMA_VERSION)
        );
        assert_eq!(alias["name"], json!("autopilot"));
        assert_eq!(alias["host_entrypoint"], json!("$autopilot"));
        assert_eq!(alias["compact"], json!(false));
        assert!(prompt.contains("进入 autopilot"));
        assert!(prompt.contains("本地 Rust"));
        assert!(prompt.contains("路由："));
        assert_eq!(
            alias["state_machine"]["current_state"],
            json!("resume_requires_repair")
        );
        assert_eq!(
            alias["state_machine"]["recommended_action"],
            json!("repair_continuity_then_resume")
        );
        assert_eq!(alias["state_machine"]["evidence_missing"], json!(true));
        assert_eq!(
            alias["entry_contract"]["context"]["execution_readiness"],
            json!("needs_recovery")
        );
        assert_eq!(
            alias["entry_contract"]["route_rules"][0],
            json!("模糊需求 -> `idea-to-plan`")
        );

        let _ = fs::remove_dir_all(&repo_root);
    }

    #[test]
    fn framework_alias_builds_compact_deepinterview_payload() {
        let repo_root = std::env::temp_dir().join(format!(
            "router-rs-deepinterview-alias-fixture-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock before epoch")
                .as_nanos()
        ));
        let task_root = repo_root
            .join("artifacts")
            .join("current")
            .join("active-bootstrap-repair-20260418210000");
        fs::create_dir_all(&task_root).expect("create task root");
        fs::create_dir_all(repo_root.join("artifacts").join("current"))
            .expect("create current root");
        fs::write(
            task_root.join("SESSION_SUMMARY.md"),
            "- task: active bootstrap repair\n- phase: implementation\n- status: in_progress\n",
        )
        .expect("write session summary");
        fs::write(
            task_root.join("NEXT_ACTIONS.json"),
            r#"{"next_actions":["Patch classifier","Run MCP regression tests"]}"#,
        )
        .expect("write next actions");
        fs::write(task_root.join("EVIDENCE_INDEX.json"), r#"{"artifacts":[]}"#)
            .expect("write evidence index");
        fs::write(
            task_root.join("TRACE_METADATA.json"),
            r#"{"task":"active bootstrap repair","matched_skills":["code-review"]}"#,
        )
        .expect("write trace metadata");
        fs::write(
            repo_root.join("artifacts").join("current").join("active_task.json"),
            r#"{"task_id":"active-bootstrap-repair-20260418210000","task":"active bootstrap repair"}"#,
        )
        .expect("write active task");
        fs::write(
            repo_root.join(".supervisor_state.json"),
            r#"{
                "task_id":"active-bootstrap-repair-20260418210000",
                "task_summary":"active bootstrap repair",
                "active_phase":"implementation",
                "verification":{"verification_status":"in_progress"},
                "continuity":{"story_state":"active","resume_allowed":true},
                "execution_contract":{"acceptance_criteria":["completed tasks never appear as current execution"]}
            }"#,
        )
        .expect("write supervisor state");

        let payload = build_framework_alias_envelope(
            &repo_root,
            "deepinterview",
            FrameworkAliasBuildOptions {
                max_lines: 5,
                compact: false,
                host_id: None,
            },
        )
        .expect("build alias payload");
        let alias = payload
            .get("alias")
            .and_then(Value::as_object)
            .expect("alias payload");
        let prompt = alias
            .get("entry_prompt")
            .and_then(Value::as_str)
            .expect("entry prompt");

        assert_eq!(
            payload["schema_version"],
            json!(FRAMEWORK_ALIAS_SCHEMA_VERSION)
        );
        assert_eq!(alias["name"], json!("deepinterview"));
        assert_eq!(alias["host_entrypoint"], json!("$deepinterview"));
        assert_eq!(alias["compact"], json!(false));
        assert_eq!(alias["canonical_owner"], json!("code-review"));
        assert_eq!(
            alias["state_machine"]["handoff"]["rules"][1]["target"],
            json!("autopilot")
        );
        assert_eq!(
            alias["entry_contract"]["route_rules"][0],
            json!("主 owner -> `code-review`")
        );
        assert!(prompt.contains("进入 deepinterview"));
        assert!(prompt.contains("每轮只问一个问题"));
        assert!(prompt.contains("review lanes ->"));

        let _ = fs::remove_dir_all(&repo_root);
    }

    #[test]
    fn framework_alias_builds_compact_team_payload() {
        let repo_root = std::env::temp_dir().join(format!(
            "router-rs-team-alias-fixture-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock before epoch")
                .as_nanos()
        ));
        let task_root = repo_root
            .join("artifacts")
            .join("current")
            .join("active-bootstrap-repair-20260418210000");
        fs::create_dir_all(&task_root).expect("create task root");
        fs::create_dir_all(repo_root.join("artifacts").join("current"))
            .expect("create current root");
        fs::write(
            task_root.join("SESSION_SUMMARY.md"),
            "- task: active bootstrap repair\n- phase: implementation\n- status: in_progress\n",
        )
        .expect("write session summary");
        fs::write(
            task_root.join("NEXT_ACTIONS.json"),
            r#"{"next_actions":["Patch classifier","Run MCP regression tests"]}"#,
        )
        .expect("write next actions");
        fs::write(task_root.join("EVIDENCE_INDEX.json"), r#"{"artifacts":[]}"#)
            .expect("write evidence index");
        fs::write(
            task_root.join("TRACE_METADATA.json"),
            r#"{"task":"active bootstrap repair","matched_skills":["plan-to-code"]}"#,
        )
        .expect("write trace metadata");
        fs::write(
            repo_root.join("artifacts").join("current").join("active_task.json"),
            r#"{"task_id":"active-bootstrap-repair-20260418210000","task":"active bootstrap repair"}"#,
        )
        .expect("write active task");
        fs::write(
            repo_root.join(".supervisor_state.json"),
            r#"{
                "task_id":"active-bootstrap-repair-20260418210000",
                "task_summary":"active bootstrap repair",
                "active_phase":"implementation",
                "verification":{"verification_status":"in_progress"},
                "continuity":{"story_state":"active","resume_allowed":true},
                "execution_contract":{"acceptance_criteria":["completed tasks never appear as current execution"]}
            }"#,
        )
        .expect("write supervisor state");

        let payload = build_framework_alias_envelope(
            &repo_root,
            "team",
            FrameworkAliasBuildOptions {
                max_lines: 5,
                compact: false,
                host_id: None,
            },
        )
        .expect("build alias payload");
        let alias = payload
            .get("alias")
            .and_then(Value::as_object)
            .expect("alias payload");
        let prompt = alias
            .get("entry_prompt")
            .and_then(Value::as_str)
            .expect("entry prompt");

        assert_eq!(
            payload["schema_version"],
            json!(FRAMEWORK_ALIAS_SCHEMA_VERSION)
        );
        assert_eq!(alias["name"], json!("team"));
        assert_eq!(alias["host_entrypoint"], json!("$team"));
        assert_eq!(alias["compact"], json!(false));
        assert_eq!(alias["canonical_owner"], json!("plan-to-code"));
        assert_eq!(
            alias["state_machine"]["handoff"]["rules"][1]["target"],
            json!("agent-swarm-orchestration")
        );
        assert_eq!(
            alias["entry_contract"]["route_rules"][0],
            json!("主 owner -> `plan-to-code`")
        );
        assert!(prompt.contains("进入 team"));
        assert!(prompt.contains("bounded subagent lane -> `agent-swarm-orchestration`"));
        assert!(prompt.contains("worker write scope -> `lane-local-delta-only`"));

        let _ = fs::remove_dir_all(&repo_root);
    }

    #[test]
    fn framework_alias_compact_payload_omits_duplicate_prompt_fields() {
        let repo_root = std::env::temp_dir().join(format!(
            "router-rs-compact-alias-fixture-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock before epoch")
                .as_nanos()
        ));
        let task_root = repo_root
            .join("artifacts")
            .join("current")
            .join("active-bootstrap-repair-20260418210000");
        fs::create_dir_all(&task_root).expect("create task root");
        fs::create_dir_all(repo_root.join("artifacts").join("current"))
            .expect("create current root");
        fs::write(
            task_root.join("SESSION_SUMMARY.md"),
            "- task: active bootstrap repair\n- phase: implementation\n- status: in_progress\n",
        )
        .expect("write session summary");
        fs::write(
            task_root.join("NEXT_ACTIONS.json"),
            r#"{"next_actions":["Patch classifier","Run MCP regression tests"]}"#,
        )
        .expect("write next actions");
        fs::write(task_root.join("EVIDENCE_INDEX.json"), r#"{"artifacts":[]}"#)
            .expect("write evidence index");
        fs::write(
            task_root.join("TRACE_METADATA.json"),
            r#"{"task":"active bootstrap repair","matched_skills":["plan-to-code"]}"#,
        )
        .expect("write trace metadata");
        fs::write(
            repo_root.join("artifacts").join("current").join("active_task.json"),
            r#"{"task_id":"active-bootstrap-repair-20260418210000","task":"active bootstrap repair"}"#,
        )
        .expect("write active task");
        fs::write(
            repo_root.join(".supervisor_state.json"),
            r#"{
                "task_id":"active-bootstrap-repair-20260418210000",
                "task_summary":"active bootstrap repair",
                "active_phase":"implementation",
                "verification":{"verification_status":"in_progress"},
                "continuity":{"story_state":"active","resume_allowed":true},
                "execution_contract":{"acceptance_criteria":["completed tasks never appear as current execution"]}
            }"#,
        )
        .expect("write supervisor state");

        let payload = build_framework_alias_envelope(
            &repo_root,
            "autopilot",
            FrameworkAliasBuildOptions {
                max_lines: 3,
                compact: true,
                host_id: None,
            },
        )
        .expect("build alias payload");
        let alias = payload
            .get("alias")
            .and_then(Value::as_object)
            .expect("alias payload");

        assert_eq!(alias["compact"], json!(true));
        assert!(alias.get("entry_prompt").is_none());
        assert!(alias.get("entry_prompt_token_estimate").is_none());
        assert!(alias.get("upstream_source").is_none());
        assert_eq!(alias["state_machine"]["evidence_missing"], json!(true));
        assert_eq!(
            alias["entry_contract"]["context"]["execution_readiness"],
            json!("needs_recovery")
        );
        assert_eq!(
            alias["state_machine"]["required_anchors"],
            json!([
                "SESSION_SUMMARY",
                "NEXT_ACTIONS",
                "TRACE_METADATA",
                "SUPERVISOR_STATE"
            ])
        );
        assert!(alias["state_machine"]["resume"].get("task").is_none());

        let _ = fs::remove_dir_all(&repo_root);
    }

    #[test]
    fn framework_memory_recall_with_artifact_override_keeps_repo_supervisor_anchor() {
        let repo_root = std::env::temp_dir().join(format!(
            "router-rs-memory-override-fixture-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock before epoch")
                .as_nanos()
        ));
        let isolated_artifacts = repo_root.join("isolated-artifacts");
        let isolated_task_id = "isolated-task-20260418220000";
        let repo_task_root = repo_root
            .join("artifacts")
            .join("current")
            .join("repo-task-20260418210000");
        let isolated_task_root = isolated_artifacts.join("current").join(isolated_task_id);
        fs::create_dir_all(&repo_task_root).expect("create repo task root");
        fs::create_dir_all(&isolated_task_root).expect("create isolated task root");
        fs::create_dir_all(repo_root.join(".codex").join("memory")).expect("create memory root");
        fs::write(
            repo_task_root.join("SESSION_SUMMARY.md"),
            "- task: repo default task\n- phase: implementation\n- status: in_progress\n",
        )
        .expect("write repo session summary");
        fs::write(
            repo_task_root.join("NEXT_ACTIONS.json"),
            r#"{"next_actions":["Continue repo default task"]}"#,
        )
        .expect("write repo next actions");
        fs::write(
            repo_task_root.join("EVIDENCE_INDEX.json"),
            r#"{"artifacts":[]}"#,
        )
        .expect("write repo evidence index");
        fs::write(
            repo_task_root.join("TRACE_METADATA.json"),
            r#"{"task":"repo default task","matched_skills":["plan-to-code"]}"#,
        )
        .expect("write repo trace metadata");
        fs::write(
            isolated_task_root.join("SESSION_SUMMARY.md"),
            "- task: isolated active task\n- phase: implementation\n- status: in_progress\n",
        )
        .expect("write isolated session summary");
        fs::write(
            isolated_task_root.join("NEXT_ACTIONS.json"),
            r#"{"next_actions":["Continue isolated active task"]}"#,
        )
        .expect("write isolated next actions");
        fs::write(
            isolated_task_root.join("EVIDENCE_INDEX.json"),
            r#"{"artifacts":[]}"#,
        )
        .expect("write isolated evidence index");
        fs::write(
            isolated_task_root.join("TRACE_METADATA.json"),
            r#"{"task":"isolated active task","matched_skills":["plan-to-code"]}"#,
        )
        .expect("write isolated trace metadata");
        fs::write(
            repo_root.join(".codex").join("memory").join("MEMORY.md"),
            "# 项目长期记忆\n",
        )
        .expect("write memory");
        fs::write(
            repo_root.join(".supervisor_state.json"),
            format!(
                r#"{{
                "task_id":"{isolated_task_id}",
                "task_summary":"isolated active task",
                "active_phase":"implementation",
                "verification":{{"verification_status":"in_progress"}},
                "continuity":{{"story_state":"active","resume_allowed":true}}
            }}"#
            ),
        )
        .expect("write supervisor state");

        let payload = build_framework_memory_recall_envelope(
            &repo_root,
            "isolated active task",
            8,
            "active",
            None,
            Some(&isolated_artifacts),
            Some(isolated_task_id),
        )
        .expect("build memory recall");
        let memory_recall = payload
            .get("memory_recall")
            .and_then(Value::as_object)
            .expect("memory recall payload");

        let prompt_payload = &memory_recall["prompt_payload"];
        assert_eq!(
            prompt_payload["continuity"]["task"],
            json!("isolated active task")
        );
        assert_eq!(
            memory_recall["diagnostics"]["source_artifacts"]["root_anchor"]["supervisor_state"],
            json!(repo_root
                .join(".supervisor_state.json")
                .display()
                .to_string())
        );
        assert_eq!(
            memory_recall["diagnostics"]["source_artifacts"]["artifact_lanes"]["bootstrap"],
            json!(isolated_artifacts
                .join("bootstrap")
                .join("<task_id>")
                .display()
                .to_string())
        );

        let _ = fs::remove_dir_all(&repo_root);
    }

    #[test]
    fn stdio_request_rejects_unknown_operations() {
        let response =
            handle_stdio_json_line(r#"{"id":"req-1","op":"not-supported","payload":{}}"#);
        assert!(!response.ok);
        assert_eq!(response.id, json!("req-1"));
        assert!(response
            .error
            .expect("error")
            .contains("unsupported stdio operation"));
    }

    #[test]
    fn stdio_request_dispatches_route_snapshot_payload() {
        let response = handle_stdio_json_line(
            r#"{"id":2,"op":"route_snapshot","payload":{"engine":"rust","selected_skill":"router","overlay_skill":null,"layer":"L2","score":42.0,"reasons":["matched"]}}"#,
        );
        assert!(response.ok);
        assert_eq!(response.id, json!(2));
        let payload = response.payload.expect("payload");
        assert_eq!(
            payload["snapshot_schema_version"],
            json!(ROUTE_SNAPSHOT_SCHEMA_VERSION)
        );
        assert_eq!(payload["route_snapshot"]["selected_skill"], json!("router"));
    }

    #[test]
    fn stdio_route_supports_inline_skill_catalog_and_token_budget_bias() {
        let response = handle_stdio_json_line(
            r#"{"id":4,"op":"route","payload":{"query":"这是多阶段任务，但只要 bounded sidecar，保留主线程集成，降低 token 开销，不要 team orchestration","session_id":"inline-route","allow_overlay":true,"first_turn":true,"skills":[{"name":"agent-swarm-orchestration","description":"Decide whether work should stay local, use bounded sidecars, or escalate to team orchestration.","routing_layer":"L0","routing_owner":"gate","routing_gate":"delegation","routing_priority":"P1","trigger_hints":["subagent","sidecar","delegation"]},{"name":"team","description":"Supervisor-led worker lifecycle with integration qa cleanup and resume phases.","routing_layer":"L0","routing_owner":"owner","routing_gate":"none","routing_priority":"P1","trigger_hints":["team orchestration","supervisor","worker lifecycle","integration","qa","cleanup"]},{"name":"code-review","description":"Review code with structured findings.","routing_layer":"L1","routing_owner":"overlay","routing_gate":"none","routing_priority":"P1"}]}}"#,
        );
        assert!(response.ok, "{:?}", response.error);
        let payload = response.payload.expect("payload");
        assert_eq!(
            payload["selected_skill"],
            json!("agent-swarm-orchestration")
        );
        assert_eq!(payload["overlay_skill"], Value::Null);
        let reasons = payload["reasons"]
            .as_array()
            .expect("route reasons array")
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>();
        assert!(reasons
            .iter()
            .any(|reason| reason.contains("Token-budget boost applied")));
    }

    #[test]
    fn runtime_storage_operation_round_trips_filesystem_payload() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("router-rs-runtime-storage-{nonce}.txt"));
        let _ = fs::remove_file(&path);

        let write = runtime_storage_operation(RuntimeStorageRequestPayload {
            operation: "write_text".to_string(),
            path: path.display().to_string(),
            backend_family: "filesystem".to_string(),
            sqlite_db_path: None,
            storage_root: None,
            payload_text: Some("alpha".to_string()),
            expected_sha256: None,
        })
        .expect("write payload");
        assert_eq!(write.schema_version, RUNTIME_STORAGE_SCHEMA_VERSION);
        assert_eq!(write.authority, RUNTIME_STORAGE_AUTHORITY);
        assert!(write.exists);
        assert_eq!(write.bytes_written, Some(5));
        assert_eq!(
            write.backend_capabilities["supports_atomic_replace"],
            json!(true)
        );
        assert_eq!(
            write.payload_sha256.as_deref(),
            Some("8ed3f6ad685b959ead7022518e1af76cd816f8e8ec7ccdda1ed4018e8f2223f8")
        );

        let append = runtime_storage_operation(RuntimeStorageRequestPayload {
            operation: "append_text".to_string(),
            path: path.display().to_string(),
            backend_family: "filesystem".to_string(),
            sqlite_db_path: None,
            storage_root: None,
            payload_text: Some("-beta".to_string()),
            expected_sha256: None,
        })
        .expect("append payload");
        assert!(append.exists);
        assert_eq!(append.bytes_written, Some(5));
        assert_eq!(
            append.payload_sha256.as_deref(),
            Some("a8b405ab6f00d98196baf634c9d1cb02b03a801770775effca822c7abe8cf432")
        );

        let read = runtime_storage_operation(RuntimeStorageRequestPayload {
            operation: "read_text".to_string(),
            path: path.display().to_string(),
            backend_family: "filesystem".to_string(),
            sqlite_db_path: None,
            storage_root: None,
            payload_text: None,
            expected_sha256: Some(
                "a8b405ab6f00d98196baf634c9d1cb02b03a801770775effca822c7abe8cf432".to_string(),
            ),
        })
        .expect("read payload");
        assert_eq!(read.payload_text.as_deref(), Some("alpha-beta"));
        assert_eq!(read.verified, Some(true));
        assert_eq!(
            read.payload_sha256.as_deref(),
            Some("a8b405ab6f00d98196baf634c9d1cb02b03a801770775effca822c7abe8cf432")
        );

        let verify = runtime_storage_operation(RuntimeStorageRequestPayload {
            operation: "verify_text".to_string(),
            path: path.display().to_string(),
            backend_family: "filesystem".to_string(),
            sqlite_db_path: None,
            storage_root: None,
            payload_text: None,
            expected_sha256: Some(
                "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            ),
        })
        .expect("verify payload");
        assert_eq!(verify.verified, Some(false));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn runtime_storage_operation_round_trips_sqlite_payload() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("router-rs-runtime-storage-root-{nonce}"));
        let db_path = root.join("runtime_checkpoint_store.sqlite3");
        let artifact_path = root.join("runtime-data").join("TRACE_RESUME_MANIFEST.json");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create sqlite root");

        let write = runtime_storage_operation(RuntimeStorageRequestPayload {
            operation: "write_text".to_string(),
            path: artifact_path.display().to_string(),
            backend_family: "sqlite".to_string(),
            sqlite_db_path: Some(db_path.display().to_string()),
            storage_root: Some(root.display().to_string()),
            payload_text: Some("{\"status\":\"ok\"}".to_string()),
            expected_sha256: None,
        })
        .expect("sqlite write payload");
        assert_eq!(write.backend_family, "sqlite");
        assert_eq!(
            write.backend_capabilities["supports_sqlite_wal"],
            json!(true)
        );
        assert_eq!(
            write.sqlite_db_path.as_deref(),
            Some(db_path.display().to_string().as_str())
        );
        assert_eq!(
            write.storage_root.as_deref(),
            Some(root.display().to_string().as_str())
        );
        assert!(db_path.exists());

        let read = runtime_storage_operation(RuntimeStorageRequestPayload {
            operation: "read_text".to_string(),
            path: artifact_path.display().to_string(),
            backend_family: "sqlite".to_string(),
            sqlite_db_path: Some(db_path.display().to_string()),
            storage_root: Some(root.display().to_string()),
            payload_text: None,
            expected_sha256: None,
        })
        .expect("sqlite read payload");
        assert_eq!(read.payload_text.as_deref(), Some("{\"status\":\"ok\"}"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn runtime_checkpoint_control_plane_normalizes_backend_family_catalog() {
        let root = temp_dir_path("checkpoint-control-plane");
        let response = build_checkpoint_control_plane_compiler_payload(json!({
            "control_plane_descriptor": {
                "schema_version": "router-rs-runtime-control-plane-v1",
                "authority": "rust-runtime-control-plane",
                "services": {
                    "trace": {
                        "authority": "rust-runtime-control-plane",
                        "role": "trace-and-handoff",
                        "projection": "rust-native-projection",
                        "delegate_kind": "filesystem-trace-store"
                    },
                    "state": {
                        "authority": "rust-runtime-control-plane",
                        "role": "durable-background-state",
                        "projection": "rust-native-projection",
                        "delegate_kind": "filesystem-state-store"
                    }
                }
            },
            "capabilities": {
                "backend_family": "sqlite3",
                "store_backend_family": "sqlite",
                "trace_backend_family": "sqlite",
                "state_backend_family": "sqlite"
            },
            "paths": {
                "trace_output_path": root.join("TRACE_METADATA.json").display().to_string(),
                "event_stream_path": root.join("TRACE_EVENTS.jsonl").display().to_string(),
                "resume_manifest_path": root.join("TRACE_RESUME_MANIFEST.json").display().to_string(),
                "background_state_path": root.join("runtime_background_jobs.json").display().to_string(),
                "event_transport_dir": root.join("runtime_event_transports").display().to_string()
            }
        }))
        .expect("checkpoint control plane");
        let control_plane = &response["checkpoint_control_plane"];

        assert_eq!(control_plane["backend_family"], json!("sqlite"));
        assert_eq!(
            control_plane["trace_service"]["delegate_kind"],
            json!("filesystem-trace-store")
        );
        assert_eq!(
            control_plane["state_service"]["delegate_kind"],
            json!("filesystem-state-store")
        );
        assert_eq!(control_plane["supports_compaction"], json!(true));
        assert_eq!(control_plane["supports_snapshot_delta"], json!(true));
        assert_eq!(control_plane["supports_consistent_append"], json!(true));
        assert_eq!(control_plane["supports_sqlite_wal"], json!(true));
        assert_eq!(
            control_plane["backend_family_catalog"]["strongest_local_backend_family"],
            json!("sqlite")
        );
        assert_eq!(
            control_plane["backend_family_parity"]["aligned"],
            json!(true)
        );
        assert_eq!(
            control_plane["backend_family_parity"]["compaction_eligible"],
            json!(true)
        );
    }

    #[test]
    fn runtime_checkpoint_control_plane_rejects_mixed_backend_families() {
        let root = temp_dir_path("checkpoint-control-plane-mismatch");
        let err = build_checkpoint_control_plane_compiler_payload(json!({
            "capabilities": {
                "backend_family": "sqlite",
                "store_backend_family": "filesystem"
            },
            "paths": {
                "background_state_path": root.join("runtime_background_jobs.json").display().to_string(),
                "event_transport_dir": root.join("runtime_event_transports").display().to_string()
            }
        }))
        .expect_err("mixed backend families should fail closed");

        assert!(err.contains("backend family mismatch"));
    }

    #[test]
    fn stdio_route_cache_reuses_records_until_runtime_changes() {
        let runtime_path = temp_json_path("routing-runtime");
        let manifest_path = temp_json_path("routing-manifest");
        write_runtime_fixture(&runtime_path, "alpha");
        write_manifest_fixture(&manifest_path, "alpha", "P1");

        let first = load_records_cached_for_stdio(Some(&runtime_path), Some(&manifest_path))
            .expect("first cache load");
        let second = load_records_cached_for_stdio(Some(&runtime_path), Some(&manifest_path))
            .expect("second cache load");
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(first[0].slug, "alpha");

        sleep(Duration::from_millis(20));
        write_runtime_fixture(&runtime_path, "beta");

        let third = load_records_cached_for_stdio(Some(&runtime_path), Some(&manifest_path))
            .expect("reload after runtime change");
        assert!(!Arc::ptr_eq(&second, &third));
        assert_eq!(third[0].slug, "beta");

        let _ = fs::remove_file(runtime_path);
        let _ = fs::remove_file(manifest_path);
    }

    #[test]
    fn route_records_cache_refreshes_default_runtime_path() {
        let repo_root = temp_dir_path("routing-default-runtime");
        let skills_dir = repo_root.join("skills");
        fs::create_dir_all(&skills_dir).expect("create skills dir");
        let runtime_path = skills_dir.join("SKILL_ROUTING_RUNTIME.json");
        write_runtime_fixture(&runtime_path, "default-alpha");

        let first = load_records_cached_for_stdio_with_default_runtime_path(&runtime_path, None)
            .expect("first default load");
        let second = load_records_cached_for_stdio_with_default_runtime_path(&runtime_path, None)
            .expect("second default load");
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(first[0].slug, "default-alpha");

        sleep(Duration::from_millis(20));
        write_runtime_fixture(&runtime_path, "default-beta");

        let third = load_records_cached_for_stdio_with_default_runtime_path(&runtime_path, None)
            .expect("refreshed default load");
        assert!(!Arc::ptr_eq(&second, &third));
        assert_eq!(third[0].slug, "default-beta");

        let _ = fs::remove_dir_all(repo_root);
    }

    #[test]
    fn route_decision_fixture_expectations_hold() {
        let fixture = fixture_path();
        let records = load_records(None, Some(&fixture)).expect("load fixture records");
        let payload = read_json(&fixture).expect("read fixture");
        let cases = payload
            .get("cases")
            .and_then(Value::as_array)
            .expect("cases array");

        for case in cases {
            let case_name = case
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("<unnamed>");
            let query = case
                .get("query")
                .and_then(Value::as_str)
                .expect("case query");
            let expected = case.get("expected").expect("case expected");
            let allow_overlay = case
                .get("allow_overlay")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let first_turn = case
                .get("first_turn")
                .and_then(Value::as_bool)
                .unwrap_or(true);

            let decision = route_task(
                &records,
                query,
                "fixture-session",
                allow_overlay,
                first_turn,
            )
            .expect("route task");

            assert_eq!(
                decision.selected_skill,
                expected
                    .get("selected_skill")
                    .and_then(Value::as_str)
                    .expect("selected_skill"),
                "selected_skill mismatch for {case_name}"
            );
            assert_eq!(
                decision.overlay_skill,
                expected
                    .get("overlay_skill")
                    .and_then(Value::as_str)
                    .map(|value| value.to_string()),
                "overlay_skill mismatch for {case_name}: {:?}",
                decision.reasons
            );
            assert_eq!(
                decision.layer,
                expected
                    .get("layer")
                    .and_then(Value::as_str)
                    .expect("expected layer"),
                "layer mismatch for {case_name}"
            );
            assert_eq!(
                decision.route_snapshot.selected_skill, decision.selected_skill,
                "snapshot selected_skill mismatch for {case_name}"
            );
            assert_eq!(
                decision.route_snapshot.overlay_skill, decision.overlay_skill,
                "snapshot overlay_skill mismatch for {case_name}"
            );
            assert_eq!(
                decision.route_snapshot.layer, decision.layer,
                "snapshot layer mismatch for {case_name}"
            );
            if let Some(expected_route_context) = expected.get("route_context") {
                assert_eq!(
                    serde_json::to_value(&decision.route_context).expect("serialize route context"),
                    expected_route_context.clone(),
                    "route_context mismatch for {case_name}"
                );
            }
        }
    }

    #[test]
    fn routing_eval_report_matches_expected_baseline() {
        let manifest_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_MANIFEST.json");
        let records = load_records_from_manifest(&manifest_path).expect("load routing records");
        let cases =
            load_routing_eval_cases(&routing_eval_case_path()).expect("load routing eval cases");
        let report = evaluate_routing_cases(&records, cases).expect("evaluate routing cases");

        assert_eq!(report.schema_version, "routing-eval-v1");
        assert_eq!(report.metrics.case_count, 73);
        assert_eq!(report.metrics.overtrigger, 0);
        assert_routing_eval_cases_match("manifest", |task, session_id, first_turn| {
            route_task(&records, task, session_id, true, first_turn)
        });
    }

    #[test]
    fn routing_eval_runtime_fallback_matches_expected_baseline() {
        let runtime_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../skills/SKILL_ROUTING_RUNTIME.json");
        let records = load_records(Some(&runtime_path), None).expect("load hot runtime records");

        assert_routing_eval_cases_match("runtime-fallback", |task, session_id, first_turn| {
            route_task_with_manifest_fallback(
                &records,
                Some(&runtime_path),
                None,
                task,
                session_id,
                true,
                first_turn,
            )
        });
    }

    #[test]
    fn framework_command_aliases_require_literal_entrypoints() {
        let runtime_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../skills/SKILL_ROUTING_RUNTIME.json");
        let records = load_records(Some(&runtime_path), None).expect("load hot runtime records");
        assert!(records.iter().any(|record| record.slug == "autopilot"));
        assert!(records.iter().any(|record| record.slug == "team"));

        let autopilot = route_task_with_manifest_fallback(
            &records,
            Some(&runtime_path),
            None,
            "$autopilot",
            "alias-autopilot",
            true,
            true,
        )
        .expect("route explicit autopilot alias");
        assert_eq!(autopilot.selected_skill, "autopilot");

        let team = route_task_with_manifest_fallback(
            &records,
            Some(&runtime_path),
            None,
            "$team",
            "alias-team",
            true,
            true,
        )
        .expect("route explicit team alias");
        assert_eq!(team.selected_skill, "team");

        let natural_language_team = route_task_with_manifest_fallback(
            &records,
            Some(&runtime_path),
            None,
            "需要 team orchestration 多 agent 执行",
            "natural-language-team",
            true,
            true,
        )
        .expect("route natural language team ask");
        assert_eq!(
            natural_language_team.selected_skill,
            "agent-swarm-orchestration"
        );

        for (query, forbidden) in [
            ("autopilot", "autopilot"),
            ("team", "team"),
            ("fix this bug", "systematic-debugging"),
            ("make a plan", "idea-to-plan"),
            ("write a small helper function", "plan-to-code"),
            ("ordinary skill question", "skill-framework-developer"),
        ] {
            let decision = route_task_with_manifest_fallback(
                &records,
                Some(&runtime_path),
                None,
                query,
                &format!("negative-{forbidden}"),
                true,
                true,
            )
            .unwrap_or_else(|err| panic!("route negative case {query}: {err}"));
            assert_ne!(
                decision.selected_skill, forbidden,
                "generic query {query:?} should not select {forbidden}"
            );
        }
    }

    #[test]
    fn search_uses_route_scorer_for_framework_review() {
        let manifest_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_MANIFEST.json");
        let records = load_records_from_manifest(&manifest_path).expect("load routing records");

        let rows = search_skills(&records, "现在的路由系统，用减法原理review一下", 5);

        assert_eq!(
            rows.first().map(|row| row.slug.as_str()),
            Some("skill-framework-developer")
        );
        assert!(!rows.iter().any(|row| row.slug == "paper-reviewer"));
    }

    #[test]
    fn generic_xlsx_intake_hits_spreadsheet_gate_first() {
        let manifest_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_MANIFEST.json");
        let records = load_records_from_manifest(&manifest_path).expect("load routing records");

        let decision = route_task(
            &records,
            "整理这个 xlsx 表格",
            "artifact-gate-test",
            true,
            true,
        )
        .expect("route task");

        assert_eq!(decision.selected_skill, "spreadsheets");
    }

    #[test]
    fn route_diff_report_matches_shadow_compare_contract() {
        let rust_snapshot = build_route_snapshot(
            "rust",
            "plan-to-code",
            Some("code-review"),
            "L2",
            39.0,
            &["Trigger phrase matched: 直接做代码.".to_string()],
        );

        let report = build_route_diff_report("shadow", rust_snapshot, None).expect("shadow report");

        assert_eq!(report.report_schema_version, ROUTE_REPORT_SCHEMA_VERSION);
        assert_eq!(report.authority, ROUTE_AUTHORITY);
        assert_eq!(report.mode, "shadow");
        assert_eq!(report.primary_engine, "rust");
        assert_eq!(report.evidence_kind, "rust-owned-snapshot");
        assert!(!report.strict_verification);
        assert!(report.verification_passed);
        assert!(report.verified_contract_fields.is_empty());
        assert!(report.contract_mismatch_fields.is_empty());
        assert_eq!(report.route_snapshot.engine, "rust");
    }

    #[test]
    fn route_policy_matches_mode_matrix() {
        let shadow = build_route_policy("shadow").expect("shadow policy");
        assert_eq!(shadow.diagnostic_route_mode, "shadow");
        assert_eq!(shadow.primary_authority, "rust");
        assert_eq!(shadow.route_result_engine, "rust");
        assert!(shadow.diagnostic_report_required);
        assert!(!shadow.strict_verification_required);

        let verify = build_route_policy("verify").expect("verify policy");
        assert_eq!(verify.diagnostic_route_mode, "verify");
        assert_eq!(verify.primary_authority, "rust");
        assert_eq!(verify.route_result_engine, "rust");
        assert!(verify.diagnostic_report_required);
        assert!(verify.strict_verification_required);

        let rust = build_route_policy("rust").expect("rust policy");
        assert_eq!(rust.diagnostic_route_mode, "none");
        assert_eq!(rust.primary_authority, "rust");
        assert_eq!(rust.route_result_engine, "rust");
        assert!(!rust.diagnostic_report_required);
        assert!(!rust.strict_verification_required);

        let unsupported = build_route_policy("python").expect_err("unsupported route mode");
        assert!(unsupported.contains("unsupported route policy mode"));
    }

    #[test]
    fn runtime_control_plane_payload_is_rust_owned() {
        let payload = build_runtime_control_plane_payload();

        assert_eq!(
            payload["schema_version"],
            Value::String(RUNTIME_CONTROL_PLANE_SCHEMA_VERSION.to_string())
        );
        assert_eq!(
            payload["authority"],
            Value::String(RUNTIME_CONTROL_PLANE_AUTHORITY.to_string())
        );
        assert_eq!(
            payload["default_route_mode"],
            Value::String("rust".to_string())
        );
        assert_eq!(
            payload["default_route_authority"],
            Value::String(ROUTE_AUTHORITY.to_string())
        );
        assert_eq!(
            payload["runtime_status"]["runtime_primary_owner"],
            Value::String("rust-control-plane".to_string())
        );
        assert_eq!(
            payload["runtime_status"]["hot_path_projection_mode"],
            Value::String("descriptor-driven".to_string())
        );
        assert!(payload["runtime_status"]
            .get("framework_runtime_package_status")
            .is_none());
        assert_eq!(
            payload["runtime_status"]["framework_runtime_replacement"],
            Value::String("router-rs::framework_runtime".to_string())
        );
        assert_eq!(
            payload["runtime_host"]["role"],
            Value::String("runtime-orchestration".to_string())
        );
        assert_eq!(
            payload["runtime_host"]["startup_order"][0],
            Value::String("router".to_string())
        );
        assert_eq!(
            payload["runtime_host"]["concurrency_contract"]["router_stdio_pool_default_size"],
            json!(DEFAULT_ROUTER_STDIO_POOL_SIZE)
        );
        assert_eq!(
            payload["runtime_host"]["concurrency_contract"]["router_stdio_pool_max_size"],
            json!(MAX_ROUTER_STDIO_POOL_SIZE)
        );
        assert_eq!(
            payload["services"]["middleware"]["subagent_limit_contract"]
                ["max_concurrent_subagents_limit"],
            json!(MAX_CONCURRENT_SUBAGENTS_LIMIT)
        );
        assert_eq!(
            payload["runtime_host"]["shutdown_order"][0],
            Value::String("background".to_string())
        );
        assert_eq!(
            payload["services"]["execution"]["delegate_kind"],
            Value::String("rust-execution-kernel-slice".to_string())
        );
        assert_eq!(
            payload["services"]["execution"]["kernel_live_backend_impl"],
            Value::String("router-rs".to_string())
        );
        assert_eq!(
            payload["services"]["execution"]["kernel_contract"]["execution_kernel_delegate_impl"],
            Value::String("router-rs".to_string())
        );
        assert_eq!(
            payload["services"]["execution"]["kernel_contract"]
                ["execution_kernel_metadata_schema_version"],
            Value::String(EXECUTION_METADATA_SCHEMA_VERSION.to_string())
        );
        assert_eq!(
            payload["services"]["execution"]["kernel_contract"]["execution_kernel_fallback_policy"],
            Value::String(EXECUTION_KERNEL_FALLBACK_POLICY.to_string())
        );
        assert_eq!(
            payload["services"]["execution"]["kernel_contract"]["execution_kernel_response_shape"],
            Value::String(EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY.to_string())
        );
        assert_eq!(
            payload["services"]["execution"]["kernel_contract_by_mode"]
                [EXECUTION_RESPONSE_SHAPE_DRY_RUN]["execution_kernel_response_shape"],
            Value::String(EXECUTION_RESPONSE_SHAPE_DRY_RUN.to_string())
        );
        assert_eq!(
            payload["services"]["execution"]["kernel_contract_by_mode"]
                [EXECUTION_RESPONSE_SHAPE_DRY_RUN]["execution_kernel_prompt_preview_owner"],
            Value::String(EXECUTION_PROMPT_PREVIEW_OWNER.to_string())
        );
        assert_eq!(
            payload["services"]["execution"]["kernel_metadata_contract"]["schema_version"],
            Value::String(EXECUTION_METADATA_CONTRACT_SCHEMA_VERSION.to_string())
        );
        assert_eq!(
            payload["services"]["execution"]["kernel_metadata_contract"]["authority"],
            Value::String(EXECUTION_KERNEL_AUTHORITY.to_string())
        );
        assert_eq!(
            payload["services"]["execution"]["kernel_metadata_contract"]["runtime_fields"]
                ["live_primary_required"][2],
            Value::String("execution_mode".to_string())
        );
        assert_eq!(
            payload["services"]["execution"]["kernel_metadata_contract"]["runtime_fields"]
                ["live_primary_passthrough"][1],
            Value::String("diagnostic_route_mode".to_string())
        );
        assert_eq!(
            payload["services"]["execution"]["kernel_metadata_contract"]["defaults"]
                ["live_primary_model_id_source"],
            Value::String(EXECUTION_MODEL_ID_SOURCE.to_string())
        );
        assert_eq!(
            payload["services"]["execution"]["kernel_live_delegate_authority"],
            Value::String("rust-execution-cli".to_string())
        );
        assert_eq!(
            payload["services"]["execution"]["sandbox_lifecycle_contract"]["schema_version"],
            Value::String("runtime-sandbox-lifecycle-v1".to_string())
        );
        assert_eq!(
            payload["services"]["execution"]["sandbox_lifecycle_contract"]["cleanup_mode"],
            Value::String("async-drain-and-recycle".to_string())
        );
        assert_eq!(
            payload["services"]["execution"]["sandbox_lifecycle_contract"]["control_operations"][1],
            Value::String("cleanup".to_string())
        );
        assert_eq!(
            payload["services"]["execution"]["sandbox_lifecycle_contract"]["control_operations"][2],
            Value::String("admit".to_string())
        );
        assert_eq!(
            payload["services"]["execution"]["sandbox_lifecycle_contract"]["event_schema_version"],
            Value::String(SANDBOX_EVENT_SCHEMA_VERSION.to_string())
        );
        assert_eq!(
            payload["services"]["execution"]["sandbox_lifecycle_contract"]["event_tracing"]
                ["response_flag"],
            Value::String("event_written".to_string())
        );
        assert_eq!(
            payload["services"]["checkpoint"]["delegate_kind"],
            Value::String("filesystem-checkpointer".to_string())
        );
        assert_eq!(
            payload["services"]["checkpoint"]["backend_family_catalog"]
                ["strongest_local_backend_family"],
            Value::String("sqlite".to_string())
        );
        assert!(
            !payload["services"]["checkpoint"]["backend_family_catalog"]["families"]
                .as_array()
                .expect("backend family catalog")
                .iter()
                .any(|family| family["backend_family"] == "memory")
        );
        assert_eq!(
            payload["services"]["checkpoint"]["backend_family_catalog"]
                ["test_only_backend_families"][0],
            Value::String("memory".to_string())
        );
        assert_eq!(
            payload["services"]["checkpoint"]["backend_family_parity"]["aligned"],
            Value::Bool(true)
        );
        assert_eq!(
            payload["services"]["background"]["authority"],
            Value::String(RUNTIME_CONTROL_PLANE_AUTHORITY.to_string())
        );
        assert_eq!(
            payload["services"]["background"]["delegate_kind"],
            Value::String("rust-background-control-policy".to_string())
        );
        assert_eq!(
            payload["services"]["background"]["orchestration_contract"]["policy_schema_version"],
            Value::String(BACKGROUND_CONTROL_SCHEMA_VERSION.to_string())
        );
        assert_eq!(
            payload["services"]["background"]["orchestration_contract"]["active_statuses"][4],
            Value::String("retry_claimed".to_string())
        );
        assert_eq!(
            payload["services"]["background"]["orchestration_contract"]["policy_operations"][0],
            Value::String("batch-plan".to_string())
        );
        assert_eq!(
            payload["services"]["background"]["orchestration_contract"]["policy_operations"][5],
            Value::String("retry".to_string())
        );
        assert!(payload["services"].get("agent_factory").is_none());
    }

    fn execution_kernel_contract_shape_fields(shape: &Value) -> Vec<String> {
        let object = shape.as_object().expect("contract shape object");
        let mut keys: Vec<String> = object.keys().cloned().collect();
        keys.sort_unstable();
        keys
    }

    #[test]
    fn execution_kernel_metadata_shape_consistency_regression_for_primary_and_dry_run() {
        let contracts = build_execution_kernel_contracts_by_mode();
        let live_primary = contracts
            .get(EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY)
            .expect("live primary contract");
        let dry_run = contracts
            .get(EXECUTION_RESPONSE_SHAPE_DRY_RUN)
            .expect("dry run contract");
        let base_fields = execution_kernel_contract_shape_fields(live_primary);
        assert_eq!(base_fields, execution_kernel_contract_shape_fields(dry_run));
        assert_eq!(
            live_primary["execution_kernel_response_shape"],
            Value::String(EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY.to_string())
        );
        assert_eq!(
            dry_run["execution_kernel_response_shape"],
            Value::String(EXECUTION_RESPONSE_SHAPE_DRY_RUN.to_string())
        );
        assert_eq!(
            live_primary["execution_kernel_prompt_preview_owner"],
            Value::String(EXECUTION_PROMPT_PREVIEW_OWNER.to_string())
        );
        assert_eq!(
            dry_run["execution_kernel_prompt_preview_owner"],
            Value::String(EXECUTION_PROMPT_PREVIEW_OWNER.to_string())
        );
        assert_eq!(contracts.len(), 2);
    }

    #[test]
    fn execution_kernel_metadata_contract_is_rust_owned() {
        let contract = build_execution_kernel_metadata_contract();

        assert_eq!(
            contract["schema_version"],
            Value::String(EXECUTION_METADATA_CONTRACT_SCHEMA_VERSION.to_string())
        );
        assert_eq!(
            contract["steady_state_fields"][0],
            Value::String("execution_kernel_metadata_schema_version".to_string())
        );
        assert_eq!(
            contract["runtime_fields"]["shared"],
            json!(["trace_event_count", "trace_output_path"])
        );
        assert_eq!(
            contract["defaults"]["supported_response_shapes"],
            json!([
                EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY,
                EXECUTION_RESPONSE_SHAPE_DRY_RUN,
            ])
        );
    }

    #[test]
    fn runtime_observability_exporter_descriptor_is_rust_owned() {
        let payload = build_runtime_observability_exporter_descriptor();

        assert_eq!(
            payload["schema_version"],
            Value::String(RUNTIME_OBSERVABILITY_EXPORTER_SCHEMA_VERSION.to_string())
        );
        assert_eq!(
            payload["metric_catalog_version"],
            Value::String(RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION.to_string())
        );
        assert_eq!(
            payload["dashboard_schema_version"],
            Value::String(RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION.to_string())
        );
        assert_eq!(
            payload["producer_authority"],
            Value::String(RUNTIME_CONTROL_PLANE_AUTHORITY.to_string())
        );
        assert_eq!(
            payload["exporter_authority"],
            Value::String(RUNTIME_CONTROL_PLANE_AUTHORITY.to_string())
        );
        assert_eq!(
            payload["export_path"],
            Value::String("jsonl-plus-otel".to_string())
        );
    }

    #[test]
    fn runtime_observability_dashboard_and_metric_record_follow_contract() {
        let catalog = build_runtime_observability_metric_catalog_payload();
        let metrics = catalog["metrics"].as_array().expect("metric catalog array");
        assert_eq!(
            catalog["schema_version"],
            Value::String(RUNTIME_OBSERVABILITY_METRIC_CATALOG_SCHEMA_VERSION.to_string())
        );
        assert_eq!(
            catalog["metric_catalog_version"],
            Value::String(RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION.to_string())
        );
        assert!(metrics
            .iter()
            .all(|metric| metric.get("dimensions").is_some()));
        assert!(metrics
            .iter()
            .all(|metric| metric.get("base_dimensions").is_none()));

        let dashboard = runtime_observability_dashboard_schema();
        let resource_dimensions = dashboard["resource_dimensions"]
            .as_array()
            .expect("resource dimensions array");
        assert_eq!(
            dashboard["schema_version"],
            Value::String(RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION.to_string())
        );
        assert!(resource_dimensions
            .iter()
            .any(|value| value == "service.name"));
        assert!(resource_dimensions
            .iter()
            .any(|value| value == "runtime.generation"));
        assert!(resource_dimensions
            .iter()
            .any(|value| value == "runtime.schema_version"));

        let record = build_runtime_metric_record(json!({
            "metric_name": "runtime.route_mismatch_total",
            "value": 3,
            "service_name": "codex-runtime",
            "service_version": "v1",
            "runtime_instance_id": "runtime-123",
            "route_engine_mode": "rust",
            "job_id": "job-1",
            "session_id": "session-1",
            "attempt": 2,
            "worker_id": "worker-7",
            "generation": "gen-a",
        }))
        .expect("metric record");
        assert_eq!(
            record["schema_version"],
            Value::String(RUNTIME_OBSERVABILITY_METRIC_RECORD_SCHEMA_VERSION.to_string())
        );
        assert_eq!(record["metric_type"], Value::String("counter".to_string()));
        assert_eq!(record["unit"], Value::String("1".to_string()));
        assert_eq!(
            record["dimensions"]["runtime.stage"],
            Value::String("runtime.metric".to_string())
        );
        assert_eq!(
            record["dimensions"]["runtime.status"],
            Value::String("ok".to_string())
        );
        assert_eq!(
            record["dimensions"]["runtime.schema_version"],
            Value::String(RUNTIME_OBSERVABILITY_METRIC_RECORD_SCHEMA_VERSION.to_string())
        );
        assert_eq!(
            record["ownership"]["exporter_authority"],
            Value::String(RUNTIME_CONTROL_PLANE_AUTHORITY.to_string())
        );

        let err = build_runtime_metric_record(json!({
            "metric_name": "runtime.unknown_total",
            "value": 1,
            "service_name": "codex-runtime",
            "service_version": "v1",
            "runtime_instance_id": "runtime-123",
            "route_engine_mode": "rust",
            "job_id": "job-1",
            "session_id": "session-1",
            "attempt": 1,
            "worker_id": "worker-7",
            "generation": "gen-a",
        }))
        .expect_err("unknown metric should fail closed");
        assert_eq!(err, "unsupported runtime metric: runtime.unknown_total");

        let err = build_runtime_metric_record(json!({
            "metric_name": "runtime.route_mismatch_total",
            "value": 1,
            "service_name": "codex-runtime",
            "service_version": "v1",
            "runtime_instance_id": "runtime-123",
            "route_engine_mode": "rust",
            "job_id": "job-1",
            "session_id": "session-1",
            "attempt": -1,
            "worker_id": "worker-7",
            "generation": "gen-a",
        }))
        .expect_err("negative attempts should fail closed");
        assert_eq!(
            err,
            "runtime metric record requires non-negative integer field attempt"
        );
    }

    #[test]
    fn runtime_observability_health_snapshot_is_rust_owned() {
        let payload = build_runtime_observability_health_snapshot();

        assert_eq!(
            payload["schema_version"],
            Value::String(RUNTIME_OBSERVABILITY_HEALTH_SNAPSHOT_SCHEMA_VERSION.to_string())
        );
        assert_eq!(
            payload["metric_catalog_schema_version"],
            Value::String(RUNTIME_OBSERVABILITY_METRIC_CATALOG_SCHEMA_VERSION.to_string())
        );
        assert_eq!(
            payload["dashboard_schema_version"],
            Value::String(RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION.to_string())
        );
        assert_eq!(
            payload["metric_catalog_version"],
            Value::String(RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION.to_string())
        );
        assert_eq!(
            payload["dashboard_panel_count"],
            Value::Number(serde_json::Number::from(6))
        );
        assert_eq!(
            payload["dashboard_alert_count"],
            Value::Number(serde_json::Number::from(3))
        );
        let metric_names = payload["metric_names"].as_array().expect("metric names");
        assert_eq!(metric_names.len(), 6);
        assert!(metric_names
            .iter()
            .any(|value| value == "runtime.route_mismatch_total"));
        assert_eq!(
            payload["exporter"]["exporter_authority"],
            Value::String(RUNTIME_CONTROL_PLANE_AUTHORITY.to_string())
        );
    }

    #[test]
    fn sandbox_control_accepts_known_edges_and_rejects_invalid_edges() {
        let accepted = build_sandbox_control_response(SandboxControlRequestPayload {
            schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
            operation: "transition".to_string(),
            current_state: Some("warm".to_string()),
            next_state: Some("busy".to_string()),
            ..sandbox_control_request_defaults()
        })
        .expect("accepted transition");
        assert_eq!(accepted.authority, SANDBOX_CONTROL_AUTHORITY);
        assert!(accepted.allowed);
        assert_eq!(accepted.reason, "transition-accepted");
        assert_eq!(accepted.resolved_state.as_deref(), Some("busy"));

        let rejected = build_sandbox_control_response(SandboxControlRequestPayload {
            schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
            operation: "transition".to_string(),
            current_state: Some("busy".to_string()),
            next_state: Some("warm".to_string()),
            ..sandbox_control_request_defaults()
        })
        .expect("rejected transition");
        assert!(!rejected.allowed);
        assert_eq!(rejected.reason, "invalid-transition");
        assert_eq!(
            rejected.error.as_deref(),
            Some("invalid sandbox transition: \"busy\" -> \"warm\"")
        );
    }

    #[test]
    fn sandbox_control_cleanup_resolves_recycled_and_failed_targets() {
        let recycled = build_sandbox_control_response(SandboxControlRequestPayload {
            schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
            operation: "cleanup".to_string(),
            current_state: Some("draining".to_string()),
            cleanup_failed: Some(false),
            ..sandbox_control_request_defaults()
        })
        .expect("cleanup recycled response");
        assert!(recycled.allowed);
        assert_eq!(recycled.reason, "cleanup-completed");
        assert_eq!(recycled.resolved_state.as_deref(), Some("recycled"));

        let failed = build_sandbox_control_response(SandboxControlRequestPayload {
            schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
            operation: "cleanup".to_string(),
            current_state: Some("draining".to_string()),
            cleanup_failed: Some(true),
            ..sandbox_control_request_defaults()
        })
        .expect("cleanup failed response");
        assert!(failed.allowed);
        assert_eq!(failed.reason, "cleanup-failed");
        assert_eq!(failed.resolved_state.as_deref(), Some("failed"));
    }

    #[test]
    fn sandbox_control_records_durable_event_when_requested() {
        let path = temp_trace_path("sandbox-events");
        let response = build_sandbox_control_response(SandboxControlRequestPayload {
            schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
            operation: "admit".to_string(),
            sandbox_id: Some("sandbox-1".to_string()),
            profile_id: Some("workspace".to_string()),
            current_state: Some("warm".to_string()),
            tool_category: Some("workspace_mutating".to_string()),
            capability_categories: Some(vec![
                "read_only".to_string(),
                "workspace_mutating".to_string(),
            ]),
            budget_cpu: Some(1.0),
            budget_memory: Some(1024),
            budget_wall_clock: Some(5.0),
            budget_output_size: Some(4096),
            event_log_path: Some(path.display().to_string()),
            trace_event: Some(true),
            ..sandbox_control_request_defaults()
        })
        .expect("sandbox event response");

        assert!(response.allowed);
        assert!(response.event_written);
        assert_eq!(
            response.event_schema_version.as_deref(),
            Some(SANDBOX_EVENT_SCHEMA_VERSION)
        );
        assert_eq!(
            response.effective_capabilities,
            Some(vec![
                "read_only".to_string(),
                "workspace_mutating".to_string()
            ])
        );

        let line = fs::read_to_string(&path).expect("sandbox event log");
        let event: Value = serde_json::from_str(line.trim()).expect("sandbox event json");
        assert_eq!(
            event["schema_version"],
            Value::String(SANDBOX_EVENT_SCHEMA_VERSION.to_string())
        );
        assert_eq!(
            event["kind"],
            Value::String("sandbox.execution_started".to_string())
        );
        assert_eq!(event["sandbox_id"], Value::String("sandbox-1".to_string()));
        assert_eq!(
            event["effective_capabilities"][1],
            Value::String("workspace_mutating".to_string())
        );
    }

    #[test]
    fn sandbox_event_append_preserves_jsonl_records_under_concurrency() {
        let event_path = temp_trace_path("sandbox-events-concurrent");
        let mut workers = Vec::new();
        for seq in 0..32 {
            let path = event_path.clone();
            workers.push(spawn(move || {
                build_sandbox_control_response(SandboxControlRequestPayload {
                    schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
                    operation: "admit".to_string(),
                    sandbox_id: Some(format!("sandbox-{seq}")),
                    profile_id: Some("workspace".to_string()),
                    current_state: Some("warm".to_string()),
                    tool_category: Some("read_only".to_string()),
                    capability_categories: Some(vec!["read_only".to_string()]),
                    budget_cpu: Some(1.0),
                    budget_memory: Some(1024),
                    budget_wall_clock: Some(5.0),
                    budget_output_size: Some(4096),
                    event_log_path: Some(path.display().to_string()),
                    trace_event: Some(true),
                    ..sandbox_control_request_defaults()
                })
                .expect("sandbox event response");
            }));
        }
        for worker in workers {
            worker.join().expect("join sandbox worker");
        }

        let persisted = fs::read_to_string(&event_path).expect("read sandbox jsonl");
        let lines = persisted.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 32);
        let mut seen = HashSet::new();
        for line in lines {
            let event = serde_json::from_str::<Value>(line).expect("parse sandbox jsonl");
            seen.insert(
                event["sandbox_id"]
                    .as_str()
                    .expect("sandbox id")
                    .to_string(),
            );
        }
        assert_eq!(seen.len(), 32);

        fs::remove_file(&event_path).expect("cleanup sandbox path");
    }

    #[test]
    fn sandbox_control_rejects_admission_from_invalid_state() {
        let response = build_sandbox_control_response(SandboxControlRequestPayload {
            schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
            operation: "admit".to_string(),
            current_state: Some("failed".to_string()),
            tool_category: Some("read_only".to_string()),
            capability_categories: Some(vec!["read_only".to_string()]),
            budget_cpu: Some(1.0),
            budget_memory: Some(1024),
            budget_wall_clock: Some(5.0),
            budget_output_size: Some(4096),
            ..sandbox_control_request_defaults()
        })
        .expect("sandbox invalid admission response");

        assert!(!response.allowed);
        assert_eq!(response.reason, "admission-rejected");
        assert_eq!(response.resolved_state.as_deref(), Some("failed"));
        assert_eq!(response.quarantined, Some(true));
        assert_eq!(
            response.failure_reason.as_deref(),
            Some("invalid sandbox admission state: \"failed\" -> \"busy\"")
        );
    }

    fn sandbox_control_request_defaults() -> SandboxControlRequestPayload {
        SandboxControlRequestPayload {
            schema_version: String::new(),
            operation: String::new(),
            sandbox_id: None,
            profile_id: None,
            current_state: None,
            next_state: None,
            cleanup_failed: None,
            tool_category: None,
            capability_categories: None,
            dedicated_profile: None,
            budget_cpu: None,
            budget_memory: None,
            budget_wall_clock: None,
            budget_output_size: None,
            probe_cpu: None,
            probe_memory: None,
            probe_wall_clock: None,
            probe_output_size: None,
            error_kind: None,
            event_log_path: None,
            trace_event: None,
        }
    }

    fn background_control_request_defaults() -> BackgroundControlRequestPayload {
        BackgroundControlRequestPayload {
            schema_version: String::new(),
            operation: String::new(),
            multitask_strategy: None,
            current_status: None,
            task_active: None,
            task_done: None,
            active_job_count: None,
            capacity_limit: None,
            attempt: None,
            retry_count: None,
            max_attempts: None,
            backoff_base_seconds: None,
            backoff_multiplier: None,
            max_backoff_seconds: None,
            requested_parallel_group_id: None,
            request_parallel_group_ids: None,
            request_lane_ids: None,
            lane_id_prefix: None,
            batch_size: None,
        }
    }

    #[test]
    fn background_control_enqueue_rejects_invalid_strategy_and_capacity() {
        let invalid = build_background_control_response(BackgroundControlRequestPayload {
            schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
            operation: "enqueue".to_string(),
            multitask_strategy: Some("pause".to_string()),
            current_status: None,
            task_active: None,
            task_done: None,
            active_job_count: Some(0),
            capacity_limit: Some(4),
            attempt: None,
            retry_count: None,
            max_attempts: None,
            backoff_base_seconds: None,
            backoff_multiplier: None,
            max_backoff_seconds: None,
            ..background_control_request_defaults()
        })
        .expect("invalid strategy response");
        assert_eq!(invalid.authority, BACKGROUND_CONTROL_AUTHORITY);
        assert!(!invalid.strategy_supported);
        assert_eq!(invalid.accepted, Some(false));

        let capacity = build_background_control_response(BackgroundControlRequestPayload {
            schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
            operation: "enqueue".to_string(),
            multitask_strategy: Some("interrupt".to_string()),
            current_status: None,
            task_active: None,
            task_done: None,
            active_job_count: Some(2),
            capacity_limit: Some(2),
            attempt: None,
            retry_count: None,
            max_attempts: None,
            backoff_base_seconds: None,
            backoff_multiplier: None,
            max_backoff_seconds: None,
            ..background_control_request_defaults()
        })
        .expect("capacity response");
        assert!(capacity.strategy_supported);
        assert_eq!(capacity.accepted, Some(false));
        assert_eq!(capacity.requires_takeover, Some(true));
        assert_eq!(capacity.reason, "capacity-rejected");
    }

    #[test]
    fn background_control_batch_plan_resolves_group_and_lane_assignments() {
        let planned = build_background_control_response(BackgroundControlRequestPayload {
            schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
            operation: "batch-plan".to_string(),
            requested_parallel_group_id: Some("pgroup-contract".to_string()),
            request_parallel_group_ids: Some(vec![
                Some("pgroup-contract".to_string()),
                Some("pgroup-contract".to_string()),
            ]),
            request_lane_ids: Some(vec![Some("lane-a".to_string()), None]),
            lane_id_prefix: Some("lane".to_string()),
            batch_size: Some(2),
            ..background_control_request_defaults()
        })
        .expect("batch plan response");
        assert_eq!(planned.accepted, Some(true));
        assert_eq!(
            planned.resolved_parallel_group_id.as_deref(),
            Some("pgroup-contract")
        );
        assert_eq!(
            planned.lane_ids,
            Some(vec!["lane-a".to_string(), "lane-2".to_string()])
        );
        assert_eq!(planned.reason, "batch-plan-resolved");
        assert_eq!(planned.effect_plan.next_step, "plan_batch");

        let rejected = build_background_control_response(BackgroundControlRequestPayload {
            schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
            operation: "batch-plan".to_string(),
            request_parallel_group_ids: Some(vec![
                Some("pgroup-a".to_string()),
                Some("pgroup-b".to_string()),
            ]),
            batch_size: Some(2),
            ..background_control_request_defaults()
        })
        .expect("rejected batch plan response");
        assert_eq!(rejected.accepted, Some(false));
        assert_eq!(rejected.reason, "batch-plan-misaligned-parallel-group");
        assert_eq!(
            rejected.error.as_deref(),
            Some(
                "enqueue_background_batch requires one consistent parallel_group_id across the whole batch."
            )
        );
    }

    #[test]
    fn background_control_retry_computes_backoff_and_terminal_status() {
        let retry = build_background_control_response(BackgroundControlRequestPayload {
            schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
            operation: "retry".to_string(),
            multitask_strategy: None,
            current_status: None,
            task_active: None,
            task_done: None,
            active_job_count: None,
            capacity_limit: None,
            attempt: Some(1),
            retry_count: Some(0),
            max_attempts: Some(2),
            backoff_base_seconds: Some(0.5),
            backoff_multiplier: Some(2.0),
            max_backoff_seconds: Some(1.0),
            ..background_control_request_defaults()
        })
        .expect("retry response");
        assert_eq!(retry.should_retry, Some(true));
        assert_eq!(retry.next_retry_count, Some(1));
        assert_eq!(retry.backoff_seconds, Some(0.5));
        assert_eq!(retry.terminal_status.as_deref(), Some("retry_scheduled"));
        assert_eq!(retry.effect_plan.next_step, "schedule_retry");
        assert_eq!(retry.effect_plan.next_retry_count, Some(1));
        assert_eq!(retry.effect_plan.backoff_seconds, Some(0.5));

        let exhausted = build_background_control_response(BackgroundControlRequestPayload {
            schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
            operation: "retry".to_string(),
            multitask_strategy: None,
            current_status: None,
            task_active: None,
            task_done: None,
            active_job_count: None,
            capacity_limit: None,
            attempt: Some(2),
            retry_count: Some(1),
            max_attempts: Some(2),
            backoff_base_seconds: Some(0.5),
            backoff_multiplier: Some(2.0),
            max_backoff_seconds: Some(1.0),
            ..background_control_request_defaults()
        })
        .expect("retry exhausted response");
        assert_eq!(exhausted.should_retry, Some(false));
        assert_eq!(
            exhausted.terminal_status.as_deref(),
            Some("retry_exhausted")
        );
        assert_eq!(exhausted.effect_plan.next_step, "finalize_terminal");
    }

    #[test]
    fn background_control_interrupt_resolves_finalize_and_cancel_paths() {
        let queued = build_background_control_response(BackgroundControlRequestPayload {
            schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
            operation: "interrupt".to_string(),
            multitask_strategy: None,
            current_status: Some("queued".to_string()),
            task_active: Some(false),
            task_done: Some(false),
            active_job_count: None,
            capacity_limit: None,
            attempt: None,
            retry_count: None,
            max_attempts: None,
            backoff_base_seconds: None,
            backoff_multiplier: None,
            max_backoff_seconds: None,
            ..background_control_request_defaults()
        })
        .expect("queued interrupt response");
        assert_eq!(
            queued.resolved_status.as_deref(),
            Some("interrupt_requested")
        );
        assert_eq!(queued.finalize_immediately, Some(true));
        assert_eq!(queued.cancel_running_task, Some(false));
        assert_eq!(queued.terminal_status.as_deref(), Some("interrupted"));
        assert_eq!(queued.effect_plan.next_step, "finalize_interrupted");

        let running = build_background_control_response(BackgroundControlRequestPayload {
            schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
            operation: "interrupt".to_string(),
            multitask_strategy: None,
            current_status: Some("running".to_string()),
            task_active: Some(true),
            task_done: Some(false),
            active_job_count: None,
            capacity_limit: None,
            attempt: None,
            retry_count: None,
            max_attempts: None,
            backoff_base_seconds: None,
            backoff_multiplier: None,
            max_backoff_seconds: None,
            ..background_control_request_defaults()
        })
        .expect("running interrupt response");
        assert_eq!(running.finalize_immediately, Some(false));
        assert_eq!(running.cancel_running_task, Some(true));
        assert_eq!(
            running.terminal_status.as_deref(),
            Some("interrupt_requested")
        );
        assert_eq!(running.effect_plan.next_step, "request_interrupt");
    }

    #[test]
    fn background_control_claim_resolves_running_and_suppressed_paths() {
        let queued = build_background_control_response(BackgroundControlRequestPayload {
            schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
            operation: "claim".to_string(),
            multitask_strategy: None,
            current_status: Some("queued".to_string()),
            task_active: Some(false),
            task_done: Some(false),
            active_job_count: None,
            capacity_limit: None,
            attempt: None,
            retry_count: None,
            max_attempts: None,
            backoff_base_seconds: None,
            backoff_multiplier: None,
            max_backoff_seconds: None,
            ..background_control_request_defaults()
        })
        .expect("queued claim response");
        assert_eq!(queued.resolved_status.as_deref(), Some("running"));
        assert_eq!(queued.reason, "claim-running");
        assert_eq!(queued.effect_plan.next_step, "claim_execution");

        let interrupted = build_background_control_response(BackgroundControlRequestPayload {
            schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
            operation: "claim".to_string(),
            multitask_strategy: None,
            current_status: Some("interrupt_requested".to_string()),
            task_active: Some(false),
            task_done: Some(false),
            active_job_count: None,
            capacity_limit: None,
            attempt: None,
            retry_count: None,
            max_attempts: None,
            backoff_base_seconds: None,
            backoff_multiplier: None,
            max_backoff_seconds: None,
            ..background_control_request_defaults()
        })
        .expect("interrupt claim response");
        assert_eq!(interrupted.terminal_status.as_deref(), Some("interrupted"));
        assert_eq!(interrupted.reason, "claim-suppressed-interrupted");
        assert_eq!(interrupted.effect_plan.next_step, "finalize_interrupted");
    }

    #[test]
    fn background_control_complete_and_completion_race_resolve_terminal_status() {
        let complete = build_background_control_response(BackgroundControlRequestPayload {
            schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
            operation: "complete".to_string(),
            multitask_strategy: None,
            current_status: Some("running".to_string()),
            task_active: Some(false),
            task_done: Some(true),
            active_job_count: None,
            capacity_limit: None,
            attempt: None,
            retry_count: None,
            max_attempts: None,
            backoff_base_seconds: None,
            backoff_multiplier: None,
            max_backoff_seconds: None,
            ..background_control_request_defaults()
        })
        .expect("complete response");
        assert_eq!(complete.terminal_status.as_deref(), Some("completed"));
        assert_eq!(complete.resolved_status.as_deref(), Some("completed"));
        assert_eq!(complete.effect_plan.next_step, "finalize_completed");

        let race_won = build_background_control_response(BackgroundControlRequestPayload {
            schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
            operation: "completion-race".to_string(),
            multitask_strategy: None,
            current_status: Some("running".to_string()),
            task_active: Some(false),
            task_done: Some(true),
            active_job_count: None,
            capacity_limit: None,
            attempt: None,
            retry_count: None,
            max_attempts: None,
            backoff_base_seconds: None,
            backoff_multiplier: None,
            max_backoff_seconds: None,
            ..background_control_request_defaults()
        })
        .expect("completion race won response");
        assert_eq!(race_won.terminal_status.as_deref(), Some("completed"));
        assert_eq!(race_won.reason, "completion-race-won");
        assert_eq!(race_won.effect_plan.next_step, "finalize_completed");

        let race_lost = build_background_control_response(BackgroundControlRequestPayload {
            schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
            operation: "completion-race".to_string(),
            multitask_strategy: None,
            current_status: Some("interrupt_requested".to_string()),
            task_active: Some(false),
            task_done: Some(true),
            active_job_count: None,
            capacity_limit: None,
            attempt: None,
            retry_count: None,
            max_attempts: None,
            backoff_base_seconds: None,
            backoff_multiplier: None,
            max_backoff_seconds: None,
            ..background_control_request_defaults()
        })
        .expect("completion race lost response");
        assert_eq!(race_lost.terminal_status.as_deref(), Some("interrupted"));
        assert_eq!(race_lost.resolved_status.as_deref(), Some("interrupted"));
        assert_eq!(race_lost.reason, "completion-race-lost");
        assert_eq!(race_lost.effect_plan.next_step, "finalize_interrupted");
    }

    #[test]
    fn background_control_retry_claim_and_interrupt_finalize_cover_retry_lifecycle() {
        let claimed = build_background_control_response(BackgroundControlRequestPayload {
            schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
            operation: "retry-claim".to_string(),
            multitask_strategy: None,
            current_status: Some("retry_scheduled".to_string()),
            task_active: Some(false),
            task_done: Some(false),
            active_job_count: None,
            capacity_limit: None,
            attempt: None,
            retry_count: None,
            max_attempts: None,
            backoff_base_seconds: None,
            backoff_multiplier: None,
            max_backoff_seconds: None,
            ..background_control_request_defaults()
        })
        .expect("retry claim response");
        assert_eq!(claimed.terminal_status.as_deref(), Some("retry_claimed"));
        assert_eq!(claimed.resolved_status.as_deref(), Some("retry_claimed"));
        assert_eq!(claimed.finalize_immediately, Some(false));
        assert_eq!(claimed.effect_plan.next_step, "claim_retry");

        let interrupted = build_background_control_response(BackgroundControlRequestPayload {
            schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
            operation: "retry-claim".to_string(),
            multitask_strategy: None,
            current_status: Some("interrupt_requested".to_string()),
            task_active: Some(false),
            task_done: Some(false),
            active_job_count: None,
            capacity_limit: None,
            attempt: None,
            retry_count: None,
            max_attempts: None,
            backoff_base_seconds: None,
            backoff_multiplier: None,
            max_backoff_seconds: None,
            ..background_control_request_defaults()
        })
        .expect("retry claim interrupted response");
        assert_eq!(interrupted.terminal_status.as_deref(), Some("interrupted"));
        assert_eq!(interrupted.resolved_status.as_deref(), Some("interrupted"));
        assert_eq!(interrupted.reason, "retry-claim-interrupted");
        assert_eq!(interrupted.effect_plan.next_step, "finalize_interrupted");

        let finalize = build_background_control_response(BackgroundControlRequestPayload {
            schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
            operation: "interrupt-finalize".to_string(),
            multitask_strategy: None,
            current_status: Some("interrupt_requested".to_string()),
            task_active: Some(false),
            task_done: Some(true),
            active_job_count: None,
            capacity_limit: None,
            attempt: None,
            retry_count: None,
            max_attempts: None,
            backoff_base_seconds: None,
            backoff_multiplier: None,
            max_backoff_seconds: None,
            ..background_control_request_defaults()
        })
        .expect("interrupt finalize response");
        assert_eq!(finalize.terminal_status.as_deref(), Some("interrupted"));
        assert_eq!(finalize.resolved_status.as_deref(), Some("interrupted"));
        assert_eq!(finalize.reason, "interrupt-finalized");
        assert_eq!(finalize.effect_plan.next_step, "finalize_interrupted");
    }

    #[test]
    fn background_control_session_release_exposes_wait_plan() {
        let release = build_background_control_response(BackgroundControlRequestPayload {
            schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
            operation: "session-release".to_string(),
            multitask_strategy: None,
            current_status: None,
            task_active: None,
            task_done: None,
            active_job_count: None,
            capacity_limit: None,
            attempt: None,
            retry_count: None,
            max_attempts: None,
            backoff_base_seconds: None,
            backoff_multiplier: None,
            max_backoff_seconds: None,
            ..background_control_request_defaults()
        })
        .expect("session release response");
        assert_eq!(release.reason, "session-release-wait");
        assert_eq!(release.effect_plan.next_step, "wait_for_release");
        assert_eq!(release.effect_plan.wait_timeout_seconds, Some(5.0));
        assert_eq!(release.effect_plan.wait_poll_interval_seconds, Some(0.01));
    }

    #[test]
    fn route_snapshot_builder_normalizes_score_bucket_and_reasons_class() {
        let snapshot = RouteSnapshotEnvelopePayload {
            snapshot_schema_version: ROUTE_SNAPSHOT_SCHEMA_VERSION.to_string(),
            authority: ROUTE_AUTHORITY.to_string(),
            route_snapshot: build_route_snapshot(
                "rust",
                "plan-to-code",
                Some("code-review"),
                "L2",
                39.4,
                &[
                    " Trigger phrase matched: 直接做代码. ".to_string(),
                    "trigger phrase matched: 直接做代码.".to_string(),
                ],
            ),
        };

        assert_eq!(
            snapshot.snapshot_schema_version,
            ROUTE_SNAPSHOT_SCHEMA_VERSION
        );
        assert_eq!(snapshot.authority, ROUTE_AUTHORITY);
        assert_eq!(snapshot.route_snapshot.engine, "rust");
        assert_eq!(snapshot.route_snapshot.selected_skill, "plan-to-code");
        assert_eq!(
            snapshot.route_snapshot.overlay_skill.as_deref(),
            Some("code-review")
        );
        assert_eq!(snapshot.route_snapshot.score_bucket, "30-39");
        assert_eq!(
            snapshot.route_snapshot.reasons_class,
            "trigger phrase matched: 直接做代码."
        );
    }

    #[test]
    fn execute_request_dry_run_returns_rust_owned_contract() {
        let response = execute_request(sample_execute_request()).expect("execute response");

        assert_eq!(response.execution_schema_version, EXECUTION_SCHEMA_VERSION);
        assert_eq!(response.authority, EXECUTION_AUTHORITY);
        assert!(!response.live_run);
        assert_eq!(response.skill, "plan-to-code");
        assert_eq!(response.overlay.as_deref(), Some("rust-pro"));
        assert_eq!(response.usage.mode, "estimated");
        assert_eq!(response.model_id, None);
        assert_eq!(response.metadata["execution_kernel"], EXECUTION_KERNEL_KIND);
        assert_eq!(
            response.metadata["execution_kernel_metadata_schema_version"],
            EXECUTION_METADATA_SCHEMA_VERSION
        );
        assert_eq!(
            response.metadata["execution_kernel_authority"],
            EXECUTION_KERNEL_AUTHORITY
        );
        assert_eq!(
            response.metadata["execution_kernel_response_shape"],
            EXECUTION_RESPONSE_SHAPE_DRY_RUN
        );
        assert_eq!(
            response.metadata["execution_kernel_prompt_preview_owner"],
            EXECUTION_PROMPT_PREVIEW_OWNER
        );
        assert_eq!(
            response.metadata["diagnostic_route_mode"],
            Value::String("none".to_string())
        );
    }

    #[test]
    fn live_execute_prompt_builder_produces_rust_owned_contract_prompt() {
        let mut payload = sample_execute_request();
        payload.dry_run = false;
        payload.prompt_preview = None;

        let prompt = build_live_execute_prompt(&payload);

        assert!(prompt.contains("Help with the user's request directly."));
        assert!(prompt.contains("Primary focus: plan-to-code"));
        assert!(prompt.contains("Extra guidance: rust-pro"));
        assert!(prompt.contains("How to reply:"));
        assert!(prompt.contains("Lead with the answer or result."));
        assert!(prompt.contains(
            "Use plain Chinese unless the user asks otherwise, and keep the wording natural."
        ));
        assert!(prompt.contains("Keep the default reply short; only use a list when the content is naturally list-shaped."));
        assert!(prompt.contains("Trigger phrase matched: 直接做代码."));
    }

    #[test]
    fn live_execute_prompt_builder_caps_task_cues_to_five_lines() {
        let mut payload = sample_execute_request();
        payload.dry_run = false;
        payload.prompt_preview = None;
        payload.reasons = vec![
            "cue-1".to_string(),
            "cue-2".to_string(),
            "cue-3".to_string(),
            "cue-4".to_string(),
            "cue-5".to_string(),
            "cue-6".to_string(),
        ];

        let prompt = build_live_execute_prompt(&payload);

        assert!(prompt.contains("- cue-1"));
        assert!(prompt.contains("- cue-2"));
        assert!(prompt.contains("- cue-3"));
        assert!(prompt.contains("- cue-4"));
        assert!(prompt.contains("- cue-5"));
        assert!(!prompt.contains("- cue-6"));
    }

    #[test]
    fn live_execute_prompt_builder_adds_idea_to_plan_contract() {
        let mut payload = sample_execute_request();
        payload.dry_run = false;
        payload.prompt_preview = None;
        payload.selected_skill = "idea-to-plan".to_string();
        payload.overlay_skill = Some("code-review".to_string());
        payload.layer = "L-1".to_string();
        payload.reasons = vec!["Trigger hint matched: 先探索现状再提方案.".to_string()];

        let prompt = build_live_execute_prompt(&payload);

        assert!(prompt.contains("Planning output:"));
        assert!(prompt.contains("outline.md"));
        assert!(prompt.contains("decision_log.md"));
        assert!(prompt.contains("code_list.md"));
        assert!(prompt.contains("plan-to-code"));
        assert!(!prompt.contains("READ-ONLY planning route"));
        assert!(!prompt.contains("<proposed_plan>"));
    }

    #[test]
    fn live_execute_ignores_caller_supplied_prompt_preview() {
        let mut payload = sample_execute_request();
        payload.dry_run = false;
        payload.prompt_preview = Some("Native supplied live prompt".to_string());

        let prompt = build_live_execute_prompt(&payload);
        let response = build_live_execute_response(
            &payload,
            Some(prompt.clone()),
            LiveExecuteResult {
                content: "router-rs content".to_string(),
                model_id: Some("gpt-5.4".to_string()),
                run_id: Some("run-1".to_string()),
                status: Some("stop".to_string()),
                input_tokens: 21,
                output_tokens: 13,
                total_tokens: 34,
            },
        );

        assert_eq!(response.prompt_preview.as_deref(), Some(prompt.as_str()));
        assert_ne!(
            response.prompt_preview.as_deref(),
            Some("Native supplied live prompt")
        );
        assert_eq!(response.metadata["execution_kernel"], EXECUTION_KERNEL_KIND);
        assert_eq!(
            response.metadata["execution_kernel_authority"],
            EXECUTION_KERNEL_AUTHORITY
        );
        assert_eq!(
            response.metadata["execution_kernel_metadata_schema_version"],
            EXECUTION_METADATA_SCHEMA_VERSION
        );
        assert_eq!(
            response.metadata["execution_kernel_delegate_family"],
            "rust-cli"
        );
        assert_eq!(
            response.metadata["execution_kernel_delegate_impl"],
            "router-rs"
        );
        assert_eq!(
            response.metadata["execution_kernel_live_primary"],
            "router-rs"
        );
        assert_eq!(
            response.metadata["execution_kernel_live_primary_authority"],
            EXECUTION_AUTHORITY
        );
        assert_eq!(
            response.metadata["execution_kernel_response_shape"],
            EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY
        );
        assert_eq!(
            response.metadata["execution_kernel_prompt_preview_owner"],
            EXECUTION_PROMPT_PREVIEW_OWNER
        );
        assert_eq!(
            response.metadata["execution_kernel_model_id_source"],
            EXECUTION_MODEL_ID_SOURCE
        );
    }

    #[test]
    fn extract_chat_completion_content_accepts_string_and_part_arrays() {
        let string_payload = serde_json::json!({
            "choices": [{"message": {"content": "hello from router-rs"}}]
        });
        let parts_payload = serde_json::json!({
            "choices": [{
                "message": {
                    "content": [
                        {"text": "hello "},
                        {"content": "from "},
                        {"text": "router-rs"}
                    ]
                }
            }]
        });

        assert_eq!(
            extract_chat_completion_content(&string_payload).expect("string content"),
            "hello from router-rs"
        );
        assert_eq!(
            extract_chat_completion_content(&parts_payload).expect("parts content"),
            "hello from router-rs"
        );
    }

    #[test]
    fn live_execute_http_client_is_process_cached() {
        let first = live_execute_http_client().expect("first client");
        let second = live_execute_http_client().expect("second client");

        assert!(std::ptr::eq(first, second));
    }

    #[test]
    fn trace_stream_replay_unwraps_wrapped_events_and_supports_resume() {
        let trace_path = temp_trace_path("trace-replay");
        fs::write(
            &trace_path,
            concat!(
                "{\"sink_schema_version\":\"runtime-trace-sink-v2\",\"event\":{\"event_id\":\"evt-1\",\"kind\":\"job.started\",\"ts\":\"2026-04-22T10:00:00.000Z\"}}\n",
                "{\"event_id\":\"evt-2\",\"kind\":\"job.completed\",\"ts\":\"2026-04-22T10:00:01.000Z\"}\n"
            ),
        )
        .expect("write trace stream");

        let replay = replay_trace_stream(TraceStreamReplayRequestPayload {
            path: Some(trace_path.display().to_string()),
            event_stream_text: None,
            compaction_manifest_path: None,
            compaction_manifest_text: None,
            compaction_state_text: None,
            compaction_artifact_index_text: None,
            compaction_delta_text: None,
            session_id: None,
            job_id: None,
            stream_scope_fields: None,
            after_event_id: Some("evt-1".to_string()),
            limit: Some(10),
        })
        .expect("replay trace stream");

        assert_eq!(replay.schema_version, TRACE_STREAM_REPLAY_SCHEMA_VERSION);
        assert_eq!(replay.authority, TRACE_STREAM_IO_AUTHORITY);
        assert_eq!(replay.event_count, 2);
        assert_eq!(replay.source_kind, "trace_stream");
        assert_eq!(replay.events.len(), 1);
        assert_eq!(
            replay.events[0]["event_id"],
            Value::String("evt-2".to_string())
        );
        assert_eq!(
            replay.events[0]["kind"],
            Value::String("job.completed".to_string())
        );
        assert!(!replay.has_more);
        assert_eq!(
            replay.next_cursor.expect("next cursor").event_id.as_deref(),
            Some("evt-2")
        );
        assert!(replay.latest_cursor.is_none());

        fs::remove_file(&trace_path).expect("cleanup trace stream");
    }

    #[test]
    fn trace_stream_inspect_reports_latest_event_metadata() {
        let trace_path = temp_trace_path("trace-inspect");
        fs::write(
            &trace_path,
            concat!(
                "{\"event_id\":\"evt-1\",\"kind\":\"job.started\",\"ts\":\"2026-04-22T10:00:00.000Z\"}\n",
                "{\"sink_schema_version\":\"runtime-trace-sink-v2\",\"event\":{\"event_id\":\"evt-2\",\"kind\":\"job.completed\",\"ts\":\"2026-04-22T10:00:01.000Z\"}}\n"
            ),
        )
        .expect("write trace stream");

        let summary = inspect_trace_stream(TraceStreamInspectRequestPayload {
            path: Some(trace_path.display().to_string()),
            event_stream_text: None,
            compaction_manifest_path: None,
            compaction_manifest_text: None,
            compaction_state_text: None,
            compaction_artifact_index_text: None,
            compaction_delta_text: None,
            session_id: None,
            job_id: None,
            stream_scope_fields: None,
        })
        .expect("inspect trace stream");

        assert_eq!(summary.schema_version, TRACE_STREAM_INSPECT_SCHEMA_VERSION);
        assert_eq!(summary.authority, TRACE_STREAM_IO_AUTHORITY);
        assert_eq!(summary.source_kind, "trace_stream");
        assert_eq!(summary.event_count, 2);
        assert_eq!(summary.latest_event_id.as_deref(), Some("evt-2"));
        assert_eq!(summary.latest_event_kind.as_deref(), Some("job.completed"));
        assert_eq!(
            summary.latest_event_timestamp.as_deref(),
            Some("2026-04-22T10:00:01.000Z")
        );
        assert!(summary.latest_cursor.is_none());

        fs::remove_file(&trace_path).expect("cleanup trace stream");
    }

    #[test]
    fn trace_stream_replay_filters_by_scope_and_hydrates_cursor_fields() {
        let trace_path = temp_trace_path("trace-scope");
        fs::write(
            &trace_path,
            concat!(
                "{\"sink_schema_version\":\"runtime-trace-sink-v2\",\"event\":{\"session_id\":\"session-1\",\"job_id\":\"job-1\",\"event_id\":\"evt-1\",\"kind\":\"job.started\",\"stage\":\"background\",\"ts\":\"2026-04-22T10:00:00.000Z\"}}\n",
                "{\"session_id\":\"session-1\",\"job_id\":\"job-2\",\"event_id\":\"evt-2\",\"kind\":\"job.completed\",\"stage\":\"background\",\"ts\":\"2026-04-22T10:00:01.000Z\"}\n"
            ),
        )
        .expect("write trace stream");

        let replay = replay_trace_stream(TraceStreamReplayRequestPayload {
            path: Some(trace_path.display().to_string()),
            event_stream_text: None,
            compaction_manifest_path: None,
            compaction_manifest_text: None,
            compaction_state_text: None,
            compaction_artifact_index_text: None,
            compaction_delta_text: None,
            session_id: Some("session-1".to_string()),
            job_id: Some("job-1".to_string()),
            stream_scope_fields: None,
            after_event_id: None,
            limit: Some(10),
        })
        .expect("replay scoped trace stream");

        assert_eq!(replay.event_count, 1);
        assert_eq!(replay.events.len(), 1);
        assert_eq!(
            replay.events[0]["event_id"],
            Value::String("evt-1".to_string())
        );
        assert_eq!(replay.events[0]["seq"], json!(1));
        assert_eq!(replay.events[0]["generation"], json!(0));
        assert_eq!(
            replay.events[0]["cursor"],
            Value::String("g0:s1:evt-1".to_string())
        );
        assert_eq!(
            replay.latest_cursor.expect("latest cursor")["cursor"],
            Value::String("g0:s1:evt-1".to_string())
        );

        fs::remove_file(&trace_path).expect("cleanup trace stream");
    }

    #[test]
    fn attach_runtime_event_transport_preserves_resume_manifest_resolution_on_descriptor_roundtrip()
    {
        let binding_artifact_path = temp_json_path("attach-transport");
        let resume_manifest_path = temp_json_path("attach-resume-manifest");
        let trace_stream_path = temp_trace_path("attach-trace-stream");

        fs::write(
            &binding_artifact_path,
            serde_json::to_string_pretty(&json!({
                "stream_id": "stream-attach-roundtrip",
                "session_id": "session-attach-roundtrip",
                "job_id": "job-attach-roundtrip",
                "binding_backend_family": "filesystem",
                "resume_mode": "after_event_id"
            }))
            .expect("serialize binding artifact"),
        )
        .expect("write binding artifact");
        fs::write(&trace_stream_path, "").expect("write empty trace stream");
        fs::write(
            &resume_manifest_path,
            serde_json::to_string_pretty(&json!({
                "session_id": "session-attach-roundtrip",
                "job_id": "job-attach-roundtrip",
                "event_transport_path": binding_artifact_path.display().to_string(),
                "trace_stream_path": trace_stream_path.display().to_string()
            }))
            .expect("serialize resume manifest"),
        )
        .expect("write resume manifest");

        let attached = attach_runtime_event_transport(json!({
            "resume_manifest_path": resume_manifest_path.display().to_string()
        }))
        .expect("attach via resume manifest");
        let attach_descriptor = attached
            .get("attach_descriptor")
            .cloned()
            .expect("attach descriptor");
        assert_eq!(
            attach_descriptor["resolution"]["binding_artifact_path"],
            Value::String("resume_manifest".to_string())
        );
        assert_eq!(
            attach_descriptor["resolution"]["resume_manifest_path"],
            Value::String("explicit_request".to_string())
        );

        let roundtrip = attach_runtime_event_transport(json!({
            "attach_descriptor": attach_descriptor
        }))
        .expect("attach via descriptor roundtrip");
        assert_eq!(
            roundtrip["attach_descriptor"]["resolution"]["binding_artifact_path"],
            Value::String("resume_manifest".to_string())
        );
        assert_eq!(
            roundtrip["attach_descriptor"]["resolution"]["resume_manifest_path"],
            Value::String("explicit_request".to_string())
        );
        assert_eq!(
            roundtrip["binding_artifact_path"],
            Value::String(binding_artifact_path.display().to_string())
        );
        assert_eq!(
            roundtrip["resume_manifest_path"],
            Value::String(resume_manifest_path.display().to_string())
        );

        fs::remove_file(&binding_artifact_path).expect("cleanup binding artifact");
        fs::remove_file(&resume_manifest_path).expect("cleanup resume manifest");
        fs::remove_file(&trace_stream_path).expect("cleanup trace stream");
    }

    #[test]
    fn attach_runtime_event_transport_reads_sqlite_resume_manifest_trace_stream() {
        let root = temp_json_path("attach-sqlite-root")
            .with_extension("")
            .join("runtime-data");
        let db_path = root.join("runtime_checkpoint_store.sqlite3");
        let binding_artifact_path = root
            .join("runtime_event_transports")
            .join("session-sqlite__job-sqlite.json");
        let resume_manifest_path = root.join("TRACE_RESUME_MANIFEST.json");
        let trace_stream_path = root.join("TRACE_EVENTS.jsonl");

        fs::create_dir_all(binding_artifact_path.parent().expect("binding parent"))
            .expect("create sqlite fixture dir");
        let conn = rusqlite::Connection::open(&db_path).expect("open sqlite fixture");
        conn.execute(
            "CREATE TABLE runtime_storage_payloads (payload_key TEXT PRIMARY KEY, payload_text TEXT NOT NULL)",
            [],
        )
        .expect("create runtime storage payload table");
        for (path, payload) in [
            (
                binding_artifact_path.clone(),
                serde_json::to_string_pretty(&json!({
                    "schema_version": "runtime-event-transport-v1",
                    "stream_id": "stream::job-sqlite",
                    "session_id": "session-sqlite",
                    "job_id": "job-sqlite",
                    "binding_backend_family": "sqlite",
                    "resume_mode": "after_event_id",
                    "cleanup_preserves_replay": true
                }))
                .expect("serialize binding"),
            ),
            (
                resume_manifest_path.clone(),
                serde_json::to_string_pretty(&json!({
                    "schema_version": "runtime-resume-manifest-v1",
                    "session_id": "session-sqlite",
                    "job_id": "job-sqlite",
                    "event_transport_path": binding_artifact_path.display().to_string(),
                    "trace_stream_path": trace_stream_path.display().to_string(),
                    "updated_at": "2026-04-23T00:00:01+00:00"
                }))
                .expect("serialize resume"),
            ),
            (
                trace_stream_path.clone(),
                "{\"event_id\":\"evt-sqlite-1\",\"kind\":\"job.started\",\"ts\":\"2026-04-23T00:00:00.000Z\"}\n".to_string(),
            ),
        ] {
            let stable_key = path
                .strip_prefix(&root)
                .expect("path under sqlite root")
                .to_string_lossy()
                .replace('\\', "/");
            conn.execute(
                "INSERT OR REPLACE INTO runtime_storage_payloads (payload_key, payload_text) VALUES (?1, ?2)",
                rusqlite::params![stable_key, payload],
            )
            .expect("insert sqlite fixture payload");
        }
        drop(conn);

        let attached = attach_runtime_event_transport(json!({
            "resume_manifest_path": resume_manifest_path.display().to_string()
        }))
        .expect("attach via sqlite resume manifest");
        assert_eq!(
            attached["artifact_backend_family"],
            Value::String("sqlite".to_string())
        );
        assert_eq!(
            attached["trace_stream_path"],
            Value::String(trace_stream_path.display().to_string())
        );

        let replay = replay_trace_stream(TraceStreamReplayRequestPayload {
            path: Some(trace_stream_path.display().to_string()),
            event_stream_text: None,
            compaction_manifest_path: None,
            compaction_manifest_text: None,
            compaction_state_text: None,
            compaction_artifact_index_text: None,
            compaction_delta_text: None,
            session_id: None,
            job_id: None,
            stream_scope_fields: None,
            after_event_id: None,
            limit: Some(10),
        })
        .expect("replay sqlite trace stream");
        assert_eq!(replay.event_count, 1);
        assert_eq!(
            replay.events[0]["event_id"],
            Value::String("evt-sqlite-1".to_string())
        );

        fs::remove_dir_all(root.parent().expect("fixture parent")).expect("cleanup sqlite fixture");
    }

    #[test]
    fn background_state_operation_persists_control_plane_projection_and_health() {
        let state_path = temp_json_path("background-state-filesystem");
        let response = handle_background_state_operation(json!({
            "schema_version": "router-rs-background-state-request-v1",
            "operation": "apply_mutation",
            "state_path": state_path.display().to_string(),
            "backend_family": "filesystem",
            "control_plane_descriptor": {
                "schema_version": "router-rs-runtime-control-plane-v1",
                "authority": "rust-runtime-control-plane",
                "services": {
                    "state": {
                        "authority": "rust-runtime-control-plane",
                        "role": "durable-background-state",
                        "projection": "rust-native-projection",
                        "delegate_kind": "filesystem-state-store"
                    },
                    "trace": {
                        "authority": "rust-runtime-control-plane",
                        "role": "trace-and-handoff",
                        "projection": "rust-native-projection",
                        "delegate_kind": "filesystem-trace-store"
                    }
                }
            },
            "job_id": "job-filesystem-1",
            "mutation": {
                "status": "queued",
                "session_id": "session-filesystem-1"
            }
        }))
        .expect("filesystem background state response");

        assert_eq!(
            response["schema_version"],
            Value::String("router-rs-background-state-store-v1".to_string())
        );
        assert_eq!(
            response["authority"],
            Value::String("rust-background-state-store".to_string())
        );
        assert_eq!(
            response["health"]["runtime_control_plane_authority"],
            Value::String("rust-runtime-control-plane".to_string())
        );
        assert_eq!(
            response["health"]["runtime_control_plane_schema_version"],
            Value::String("router-rs-runtime-control-plane-v1".to_string())
        );
        assert_eq!(
            response["health"]["control_plane_projection"],
            Value::String("rust-native-projection".to_string())
        );
        assert_eq!(
            response["health"]["control_plane_delegate_kind"],
            Value::String("filesystem-state-store".to_string())
        );
        assert_eq!(
            response["health"]["backend_family"],
            Value::String("filesystem".to_string())
        );
        assert_eq!(
            response["health"]["supports_atomic_replace"],
            Value::Bool(true)
        );
        assert_eq!(
            response["health"]["supports_compaction"],
            Value::Bool(false)
        );
        assert_eq!(
            response["health"]["supports_snapshot_delta"],
            Value::Bool(false)
        );
        assert_eq!(
            response["health"]["supports_remote_event_transport"],
            Value::Bool(true)
        );
        assert_eq!(
            response["health"]["supports_consistent_append"],
            Value::Bool(true)
        );
        assert_eq!(
            response["health"]["supports_sqlite_wal"],
            Value::Bool(false)
        );

        let persisted = read_json(&state_path).expect("read persisted state");
        assert_eq!(
            persisted["control_plane"]["authority"],
            Value::String("rust-runtime-control-plane".to_string())
        );
        assert_eq!(
            persisted["control_plane"]["projection"],
            Value::String("rust-native-projection".to_string())
        );
        assert_eq!(
            persisted["control_plane"]["delegate_kind"],
            Value::String("filesystem-state-store".to_string())
        );
        assert_eq!(
            persisted["control_plane"]["supports_atomic_replace"],
            Value::Bool(true)
        );
        assert_eq!(
            persisted["control_plane"]["supports_consistent_append"],
            Value::Bool(true)
        );
        assert_eq!(
            persisted["jobs"][0]["status"],
            Value::String("queued".to_string())
        );

        let recovered = handle_background_state_operation(json!({
            "schema_version": "router-rs-background-state-request-v1",
            "operation": "snapshot",
            "state_path": state_path.display().to_string(),
            "backend_family": "filesystem"
        }))
        .expect("recovered background state snapshot");
        assert_eq!(
            recovered["health"]["control_plane_delegate_kind"],
            Value::String("filesystem-state-store".to_string())
        );
        assert_eq!(
            recovered["state"]["jobs"][0]["job_id"],
            Value::String("job-filesystem-1".to_string())
        );

        fs::remove_file(&state_path).expect("cleanup filesystem background state");
    }

    #[test]
    fn background_state_operation_compacts_terminal_jobs_over_capacity() {
        let state_path = temp_json_path("background-state-capacity");
        for (job_id, status) in [
            ("job-1", "completed"),
            ("job-2", "failed"),
            ("job-3", "queued"),
        ] {
            handle_background_state_operation(json!({
                "schema_version": "router-rs-background-state-request-v1",
                "operation": "apply_mutation",
                "state_path": state_path.display().to_string(),
                "backend_family": "filesystem",
                "job_id": job_id,
                "mutation": {"status": status}
            }))
            .expect("write background state fixture");
        }

        let response = handle_background_state_operation(json!({
            "schema_version": "router-rs-background-state-request-v1",
            "operation": "snapshot",
            "state_path": state_path.display().to_string(),
            "backend_family": "filesystem",
            "capacity_limit": 2
        }))
        .expect("capacity-compacted snapshot");
        let jobs = response["state"]["jobs"].as_array().expect("jobs");
        assert_eq!(jobs.len(), 2);
        assert!(jobs
            .iter()
            .any(|job| job["job_id"] == Value::String("job-3".to_string())));
        assert_eq!(response["health"]["max_background_jobs"], json!(16));
        assert_eq!(response["health"]["max_background_jobs_limit"], json!(64));

        fs::remove_file(&state_path).expect("cleanup capacity background state");
    }

    #[test]
    fn background_state_operation_reports_sqlite_backend_capabilities() {
        let temp_dir = temp_json_path("background-state-sqlite-root")
            .parent()
            .expect("temp root parent")
            .join(format!(
                "router-rs-bg-sqlite-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("clock before epoch")
                    .as_nanos()
            ));
        fs::create_dir_all(&temp_dir).expect("create sqlite temp dir");
        let canonical_temp_dir = temp_dir
            .canonicalize()
            .expect("canonicalize sqlite temp dir");
        let state_path = canonical_temp_dir.join("runtime_background_jobs.json");
        let sqlite_db_path = canonical_temp_dir.join("runtime_background_jobs.sqlite");

        let response = handle_background_state_operation(json!({
            "schema_version": "router-rs-background-state-request-v1",
            "operation": "apply_mutation",
            "state_path": state_path.display().to_string(),
            "backend_family": "sqlite",
            "sqlite_db_path": sqlite_db_path.display().to_string(),
            "control_plane_descriptor": {
                "schema_version": "router-rs-runtime-control-plane-v1",
                "authority": "rust-runtime-control-plane",
                "services": {
                    "state": {
                        "authority": "rust-runtime-control-plane",
                        "role": "durable-background-state",
                        "projection": "rust-native-projection",
                        "delegate_kind": "filesystem-state-store"
                    }
                }
            },
            "job_id": "job-sqlite-1",
            "mutation": {
                "status": "completed",
                "session_id": "session-sqlite-1"
            }
        }))
        .expect("sqlite background state response");

        assert_eq!(
            response["health"]["control_plane_delegate_kind"],
            Value::String("sqlite-state-store".to_string())
        );
        assert_eq!(
            response["health"]["backend_family"],
            Value::String("sqlite".to_string())
        );
        assert_eq!(
            response["health"]["supports_atomic_replace"],
            Value::Bool(true)
        );
        assert_eq!(response["health"]["supports_compaction"], Value::Bool(true));
        assert_eq!(
            response["health"]["supports_snapshot_delta"],
            Value::Bool(true)
        );
        assert_eq!(
            response["health"]["supports_remote_event_transport"],
            Value::Bool(true)
        );
        assert_eq!(
            response["health"]["supports_consistent_append"],
            Value::Bool(true)
        );
        assert_eq!(response["health"]["supports_sqlite_wal"], Value::Bool(true));
        assert!(!state_path.exists());
        assert!(sqlite_db_path.exists());

        let recovered = handle_background_state_operation(json!({
            "schema_version": "router-rs-background-state-request-v1",
            "operation": "snapshot",
            "state_path": state_path.display().to_string(),
            "backend_family": "sqlite",
            "sqlite_db_path": sqlite_db_path.display().to_string()
        }))
        .expect("recovered sqlite background state snapshot");
        assert_eq!(
            recovered["state"]["jobs"][0]["job_id"],
            Value::String("job-sqlite-1".to_string())
        );
        assert_eq!(
            recovered["health"]["control_plane_delegate_kind"],
            Value::String("sqlite-state-store".to_string())
        );

        fs::remove_dir_all(&canonical_temp_dir).expect("cleanup sqlite background state dir");
    }

    #[test]
    fn background_state_arbitration_dispatch_requires_explicit_operation() {
        let state_path = temp_json_path("background-state-arbitration-dispatch");

        handle_background_state_operation(json!({
            "schema_version": "router-rs-background-state-request-v1",
            "operation": "apply_mutation",
            "state_path": state_path.display().to_string(),
            "backend_family": "filesystem",
            "job_id": "job-1",
            "mutation": {
                "status": "running",
                "session_id": "shared-session"
            }
        }))
        .expect("seed active owner");

        let reserved = handle_background_state_operation(json!({
            "schema_version": "router-rs-background-state-request-v1",
            "operation": "arbitrate_session_takeover",
            "arbitration_operation": "reserve",
            "state_path": state_path.display().to_string(),
            "backend_family": "filesystem",
            "session_id": "shared-session",
            "incoming_job_id": "job-2"
        }))
        .expect("dispatch reserve arbitration");
        assert_eq!(reserved["takeover"]["operation"], json!("reserve"));
        assert_eq!(reserved["takeover"]["outcome"], json!("pending"));

        let missing = handle_background_state_operation(json!({
            "schema_version": "router-rs-background-state-request-v1",
            "operation": "arbitrate_session_takeover",
            "state_path": state_path.display().to_string(),
            "backend_family": "filesystem",
            "session_id": "shared-session",
            "incoming_job_id": "job-3"
        }))
        .expect_err("missing arbitration operation should fail closed");
        assert_eq!(
            missing,
            "Background state arbitration is missing arbitration_operation."
        );

        fs::remove_file(&state_path).expect("cleanup arbitration dispatch state");
    }

    #[test]
    fn background_state_operation_arbitrates_takeover_across_persisted_roundtrip() {
        let state_path = temp_json_path("background-state-takeover");
        let control_plane_descriptor = json!({
            "schema_version": "router-rs-runtime-control-plane-v1",
            "authority": "rust-runtime-control-plane",
            "services": {
                "state": {
                    "authority": "rust-runtime-control-plane",
                    "role": "durable-background-state",
                    "projection": "rust-native-projection",
                    "delegate_kind": "filesystem-state-store"
                }
            }
        });

        handle_background_state_operation(json!({
            "schema_version": "router-rs-background-state-request-v1",
            "operation": "apply_mutation",
            "state_path": state_path.display().to_string(),
            "backend_family": "filesystem",
            "control_plane_descriptor": control_plane_descriptor,
            "job_id": "job-1",
            "mutation": {
                "status": "running",
                "session_id": "shared-session",
                "claimed_by": "job-1"
            }
        }))
        .expect("seed active owner");

        let reserved = handle_background_state_operation(json!({
            "schema_version": "router-rs-background-state-request-v1",
            "operation": "reserve",
            "state_path": state_path.display().to_string(),
            "backend_family": "filesystem",
            "session_id": "shared-session",
            "incoming_job_id": "job-2"
        }))
        .expect("reserve takeover");
        assert_eq!(
            reserved["takeover"]["outcome"],
            Value::String("pending".to_string())
        );
        assert_eq!(reserved["takeover"]["changed"], Value::Bool(true));
        assert_eq!(
            reserved["takeover"]["previous_active_job_id"],
            Value::String("job-1".to_string())
        );
        assert_eq!(
            reserved["takeover"]["pending_job_id"],
            Value::String("job-2".to_string())
        );
        assert_eq!(reserved["health"]["pending_session_takeovers"], json!(1));

        let completed = handle_background_state_operation(json!({
            "schema_version": "router-rs-background-state-request-v1",
            "operation": "apply_mutation",
            "state_path": state_path.display().to_string(),
            "backend_family": "filesystem",
            "job_id": "job-1",
            "mutation": {
                "status": "completed",
                "session_id": "shared-session",
                "claimed_by": "job-1"
            }
        }))
        .expect("complete previous owner");
        assert_eq!(completed["health"]["active_job_count"], json!(0));

        let claimed = handle_background_state_operation(json!({
            "schema_version": "router-rs-background-state-request-v1",
            "operation": "claim",
            "state_path": state_path.display().to_string(),
            "backend_family": "filesystem",
            "session_id": "shared-session",
            "incoming_job_id": "job-2"
        }))
        .expect("claim takeover");
        assert_eq!(
            claimed["takeover"]["outcome"],
            Value::String("claimed".to_string())
        );
        assert_eq!(claimed["takeover"]["changed"], Value::Bool(true));
        assert_eq!(
            claimed["takeover"]["active_job_id"],
            Value::String("job-2".to_string())
        );
        assert_eq!(claimed["takeover"]["pending_job_id"], Value::Null);
        assert_eq!(claimed["health"]["pending_session_takeovers"], json!(0));

        let active = handle_background_state_operation(json!({
            "schema_version": "router-rs-background-state-request-v1",
            "operation": "get_active_job",
            "state_path": state_path.display().to_string(),
            "backend_family": "filesystem",
            "session_id": "shared-session"
        }))
        .expect("get active job after claim");
        assert_eq!(active["active_job_id"], Value::String("job-2".to_string()));

        let persisted = read_json(&state_path).expect("read persisted takeover state");
        assert_eq!(persisted["pending_session_takeovers"], Value::Array(vec![]));
        assert_eq!(
            persisted["active_sessions"],
            Value::Array(vec![json!({
                "session_id": "shared-session",
                "job_id": "job-2"
            })])
        );

        fs::remove_file(&state_path).expect("cleanup takeover background state");
    }

    #[test]
    fn background_state_operation_release_keeps_current_owner_when_only_pending_takeover_exists() {
        let state_path = temp_json_path("background-state-release");

        handle_background_state_operation(json!({
            "schema_version": "router-rs-background-state-request-v1",
            "operation": "apply_mutation",
            "state_path": state_path.display().to_string(),
            "backend_family": "filesystem",
            "job_id": "job-1",
            "mutation": {
                "status": "running",
                "session_id": "shared-session"
            }
        }))
        .expect("seed release owner");

        handle_background_state_operation(json!({
            "schema_version": "router-rs-background-state-request-v1",
            "operation": "reserve",
            "state_path": state_path.display().to_string(),
            "backend_family": "filesystem",
            "session_id": "shared-session",
            "incoming_job_id": "job-2"
        }))
        .expect("seed pending takeover");

        let released = handle_background_state_operation(json!({
            "schema_version": "router-rs-background-state-request-v1",
            "operation": "release",
            "state_path": state_path.display().to_string(),
            "backend_family": "filesystem",
            "session_id": "shared-session",
            "incoming_job_id": "job-2"
        }))
        .expect("release pending takeover");
        assert_eq!(
            released["takeover"]["outcome"],
            Value::String("released".to_string())
        );
        assert_eq!(released["takeover"]["changed"], Value::Bool(true));
        assert_eq!(
            released["takeover"]["active_job_id"],
            Value::String("job-1".to_string())
        );
        assert_eq!(released["takeover"]["pending_job_id"], Value::Null);
        assert_eq!(released["health"]["pending_session_takeovers"], json!(0));

        let active = handle_background_state_operation(json!({
            "schema_version": "router-rs-background-state-request-v1",
            "operation": "get_active_job",
            "state_path": state_path.display().to_string(),
            "backend_family": "filesystem",
            "session_id": "shared-session"
        }))
        .expect("get active job after release");
        assert_eq!(active["active_job_id"], Value::String("job-1".to_string()));

        fs::remove_file(&state_path).expect("cleanup release background state");
    }

    #[test]
    fn trace_compaction_inspect_and_replay_read_snapshot_plus_deltas() {
        let temp_root = temp_trace_path("trace-compaction");
        let trace_root = temp_root.parent().expect("temp root parent").join(
            temp_root
                .file_stem()
                .expect("temp root stem")
                .to_string_lossy()
                .to_string(),
        );
        let manifest_path = trace_root.join("stream.manifest.json");
        let delta_path = trace_root.join("stream.deltas.jsonl");
        let artifact_dir = trace_root.join("artifacts");
        let state_path = artifact_dir.join("stream.state.json");
        let artifact_index_path = artifact_dir.join("stream.artifacts.json");
        fs::create_dir_all(&artifact_dir).expect("create artifact dir");
        let state_text = serde_json::to_string_pretty(&json!({
            "session_id": "session-compact",
            "job_id": "job-compact",
            "latest_cursor": {
                "schema_version": "runtime-trace-cursor-v1",
                "session_id": "session-compact",
                "job_id": "job-compact",
                "generation": 0,
                "seq": 2,
                "event_id": "evt-snapshot",
                "cursor": "g0:s2:evt-snapshot"
            },
            "latest_event": {
                "event_id": "evt-snapshot",
                "kind": "job.progress",
                "stage": "background",
                "status": "ok",
                "ts": "2026-04-22T10:00:00.000Z"
            }
        }))
        .expect("serialize state");
        fs::write(&state_path, &state_text).expect("write state");
        let state_digest = sha256_hex(state_text.as_bytes());
        let artifact_index_text = serde_json::to_string_pretty(&json!([
            {
                "schema_version": "runtime-trace-artifact-ref-v1",
                "artifact_id": "art-state",
                "kind": "state_ref",
                "uri": state_path.display().to_string(),
                "digest": state_digest,
                "size_bytes": state_text.len()
            }
        ]))
        .expect("serialize artifact index");
        fs::write(&artifact_index_path, &artifact_index_text).expect("write artifact index");
        let artifact_index_digest = sha256_hex(artifact_index_text.as_bytes());
        fs::write(
            &delta_path,
            concat!(
                "{\"schema_version\":\"runtime-trace-compaction-delta-v1\",\"generation\":1,\"delta_id\":\"delta-1\",\"parent_snapshot_id\":\"snap-1\",\"seq\":1,\"ts\":\"2026-04-22T10:00:01.000Z\",\"kind\":\"job.resumed\",\"payload\":{\"event_id\":\"evt-1\",\"cursor\":\"g1:s1:evt-1\",\"stage\":\"background\",\"status\":\"ok\",\"payload\":{\"step\":3}},\"artifact_refs\":[],\"applies_to\":{\"session_id\":\"session-compact\",\"job_id\":\"job-compact\"}}\n",
                "{\"schema_version\":\"runtime-trace-compaction-delta-v1\",\"generation\":1,\"delta_id\":\"delta-2\",\"parent_snapshot_id\":\"snap-1\",\"seq\":2,\"ts\":\"2026-04-22T10:00:02.000Z\",\"kind\":\"job.completed\",\"payload\":{\"event_id\":\"evt-2\",\"cursor\":\"g1:s2:evt-2\",\"stage\":\"background\",\"status\":\"ok\",\"payload\":{\"step\":4}},\"artifact_refs\":[],\"applies_to\":{\"session_id\":\"session-compact\",\"job_id\":\"job-compact\"}}\n"
            ),
        )
        .expect("write deltas");
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&json!({
                "schema_version": "runtime-trace-compaction-manifest-v1",
                "session_id": "session-compact",
                "job_id": "job-compact",
                "backend_family": "filesystem",
                "compaction_supported": true,
                "snapshot_delta_supported": true,
                "latest_stable_snapshot": {
                    "schema_version": "runtime-trace-compaction-snapshot-v1",
                    "generation": 0,
                    "snapshot_id": "snap-1",
                    "session_id": "session-compact",
                    "job_id": "job-compact",
                    "state_digest": "state-digest",
                    "artifact_index_ref": {
                        "schema_version": "runtime-trace-artifact-ref-v1",
                        "artifact_id": "art-index",
                        "kind": "artifact_index_ref",
                        "uri": artifact_index_path.display().to_string(),
                        "digest": artifact_index_digest,
                        "size_bytes": artifact_index_text.len()
                    },
                    "state_ref": {
                        "schema_version": "runtime-trace-artifact-ref-v1",
                        "artifact_id": "art-state",
                        "kind": "state_ref",
                        "uri": state_path.display().to_string(),
                        "digest": state_digest,
                        "size_bytes": state_text.len()
                    }
                },
                "active_generation": 1,
                "active_parent_snapshot_id": "snap-1",
                "manifest_path": manifest_path.display().to_string(),
                "delta_path": delta_path.display().to_string(),
                "artifact_index_path": artifact_index_path.display().to_string(),
                "state_path": state_path.display().to_string()
            }))
            .expect("serialize manifest"),
        )
        .expect("write manifest");

        let summary = inspect_trace_stream(TraceStreamInspectRequestPayload {
            path: None,
            event_stream_text: None,
            compaction_manifest_path: Some(manifest_path.display().to_string()),
            compaction_manifest_text: None,
            compaction_state_text: None,
            compaction_artifact_index_text: None,
            compaction_delta_text: None,
            session_id: Some("session-compact".to_string()),
            job_id: Some("job-compact".to_string()),
            stream_scope_fields: None,
        })
        .expect("inspect compaction manifest");
        assert_eq!(summary.source_kind, "compaction_manifest");
        assert_eq!(summary.event_count, 2);
        assert_eq!(summary.latest_event_id.as_deref(), Some("evt-2"));
        assert_eq!(
            summary.recovery.expect("recovery")["latest_recoverable_generation"],
            json!(1)
        );

        let replay = replay_trace_stream(TraceStreamReplayRequestPayload {
            path: None,
            event_stream_text: None,
            compaction_manifest_path: Some(manifest_path.display().to_string()),
            compaction_manifest_text: None,
            compaction_state_text: None,
            compaction_artifact_index_text: None,
            compaction_delta_text: None,
            session_id: Some("session-compact".to_string()),
            job_id: Some("job-compact".to_string()),
            stream_scope_fields: None,
            after_event_id: Some("evt-1".to_string()),
            limit: Some(10),
        })
        .expect("replay compaction manifest");
        assert_eq!(replay.source_kind, "compaction_manifest");
        assert_eq!(replay.events.len(), 1);
        assert_eq!(
            replay.events[0]["event_id"],
            Value::String("evt-2".to_string())
        );

        fs::remove_dir_all(&trace_root).expect("cleanup compaction root");
    }

    #[test]
    fn trace_compaction_recovery_fails_closed_on_artifact_digest_mismatch() {
        let temp_root = temp_trace_path("trace-compaction-digest-mismatch");
        let trace_root = temp_root.parent().expect("temp root parent").join(
            temp_root
                .file_stem()
                .expect("temp root stem")
                .to_string_lossy()
                .to_string(),
        );
        let manifest_path = trace_root.join("stream.manifest.json");
        let artifact_dir = trace_root.join("artifacts");
        let state_path = artifact_dir.join("stream.state.json");
        let artifact_index_path = artifact_dir.join("stream.artifacts.json");
        fs::create_dir_all(&artifact_dir).expect("create digest mismatch artifact dir");
        let state_text = serde_json::to_string_pretty(&json!({
            "session_id": "session-compact",
            "job_id": "job-compact",
            "latest_cursor": {
                "schema_version": "runtime-trace-cursor-v1",
                "session_id": "session-compact",
                "job_id": "job-compact",
                "generation": 0,
                "seq": 1,
                "event_id": "evt-snapshot",
                "cursor": "g0:s1:evt-snapshot"
            }
        }))
        .expect("serialize digest mismatch state");
        fs::write(&state_path, &state_text).expect("write digest mismatch state");
        let artifact_index_text = "[]";
        fs::write(&artifact_index_path, artifact_index_text)
            .expect("write digest mismatch artifact index");
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&json!({
                "schema_version": "runtime-trace-compaction-manifest-v1",
                "session_id": "session-compact",
                "job_id": "job-compact",
                "latest_stable_snapshot": {
                    "schema_version": "runtime-trace-compaction-snapshot-v1",
                    "generation": 0,
                    "snapshot_id": "snap-1",
                    "session_id": "session-compact",
                    "job_id": "job-compact",
                    "state_ref": {
                        "schema_version": "runtime-trace-artifact-ref-v1",
                        "artifact_id": "art-state",
                        "kind": "state_ref",
                        "uri": state_path.display().to_string(),
                        "digest": "not-the-real-digest",
                        "size_bytes": state_text.len()
                    },
                    "artifact_index_ref": {
                        "schema_version": "runtime-trace-artifact-ref-v1",
                        "artifact_id": "art-index",
                        "kind": "artifact_index_ref",
                        "uri": artifact_index_path.display().to_string(),
                        "digest": sha256_hex(artifact_index_text.as_bytes()),
                        "size_bytes": artifact_index_text.len()
                    }
                },
                "active_generation": 1,
                "active_parent_snapshot_id": "snap-1",
                "manifest_path": manifest_path.display().to_string(),
                "artifact_index_path": artifact_index_path.display().to_string(),
                "state_path": state_path.display().to_string()
            }))
            .expect("serialize digest mismatch manifest"),
        )
        .expect("write digest mismatch manifest");

        let err = inspect_trace_stream(TraceStreamInspectRequestPayload {
            path: None,
            event_stream_text: None,
            compaction_manifest_path: Some(manifest_path.display().to_string()),
            compaction_manifest_text: None,
            compaction_state_text: None,
            compaction_artifact_index_text: None,
            compaction_delta_text: None,
            session_id: Some("session-compact".to_string()),
            job_id: Some("job-compact".to_string()),
            stream_scope_fields: None,
        })
        .expect_err("digest mismatch must fail closed");

        assert_eq!(
            err,
            "Compaction recovery failed closed because state_ref artifact digest mismatched."
        );

        fs::remove_dir_all(&trace_root).expect("cleanup digest mismatch compaction root");
    }

    #[test]
    fn write_trace_compaction_delta_appends_one_jsonl_line() {
        let delta_path = temp_trace_path("trace-delta-write");
        let response = write_trace_compaction_delta(TraceCompactionDeltaWriteRequestPayload {
            path: delta_path.display().to_string(),
            delta: json!({
                "schema_version": "runtime-trace-compaction-delta-v1",
                "delta_id": "delta-1",
                "seq": 1
            }),
        })
        .expect("write delta");

        assert_eq!(
            response.schema_version,
            TRACE_COMPACTION_DELTA_WRITE_SCHEMA_VERSION
        );
        assert_eq!(response.authority, TRACE_STREAM_IO_AUTHORITY);
        assert_eq!(response.path, delta_path.display().to_string());
        assert!(response.bytes_written > 0);
        let persisted = fs::read_to_string(&delta_path).expect("read delta");
        assert!(persisted.contains("\"delta_id\":\"delta-1\""));

        fs::remove_file(&delta_path).expect("cleanup delta path");
    }

    #[test]
    fn trace_append_preserves_jsonl_records_under_concurrency() {
        let trace_path = temp_trace_path("trace-record-event-concurrent");
        let mut workers = Vec::new();
        for seq in 0..32 {
            let path = trace_path.clone();
            workers.push(spawn(move || {
                record_trace_event(TraceRecordEventRequestPayload {
                    path: Some(path.display().to_string()),
                    write_outputs: true,
                    sink_schema_version: "runtime-trace-sink-v2".to_string(),
                    event_schema_version: "runtime-trace-v2".to_string(),
                    generation: 1,
                    seq,
                    session_id: "concurrent-trace".to_string(),
                    job_id: None,
                    kind: "test.event".to_string(),
                    stage: "append".to_string(),
                    status: "ok".to_string(),
                    payload: Map::new(),
                    compaction_manifest_path: None,
                    compaction_manifest_text: None,
                })
                .expect("record trace event");
            }));
        }
        for worker in workers {
            worker.join().expect("join trace worker");
        }

        let persisted = fs::read_to_string(&trace_path).expect("read trace jsonl");
        let lines = persisted.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 32);
        let mut seen = HashSet::new();
        for line in lines {
            let record = serde_json::from_str::<Value>(line).expect("parse trace jsonl");
            seen.insert(record["event"]["seq"].as_u64().expect("seq"));
        }
        assert_eq!(seen.len(), 32);

        fs::remove_file(&trace_path).expect("cleanup trace path");
    }

    #[test]
    fn stdio_request_dispatches_write_trace_compaction_delta_payload() {
        let delta_path = temp_trace_path("trace-delta-write-stdio");
        let response = handle_stdio_json_line(&format!(
            "{{\"id\":2,\"op\":\"write_trace_compaction_delta\",\"payload\":{{\"path\":\"{}\",\"delta\":{{\"schema_version\":\"runtime-trace-compaction-delta-v1\",\"delta_id\":\"delta-stdio\",\"seq\":2}}}}}}",
            delta_path.display()
        ));
        assert!(response.ok);
        assert_eq!(response.id, json!(2));
        assert_eq!(
            response.payload.expect("payload")["schema_version"],
            json!(TRACE_COMPACTION_DELTA_WRITE_SCHEMA_VERSION)
        );
        let persisted = fs::read_to_string(&delta_path).expect("read stdio delta");
        assert!(persisted.contains("\"delta_id\":\"delta-stdio\""));
        fs::remove_file(&delta_path).expect("cleanup stdio delta path");
    }

    #[test]
    fn write_trace_metadata_persists_primary_and_mirror_outputs() {
        let output_path = temp_json_path("trace-metadata-write");
        let mirror_path = output_path
            .parent()
            .expect("output parent")
            .join("artifacts")
            .join("current")
            .join("TRACE_METADATA.json");
        let response = write_trace_metadata(TraceMetadataWriteRequestPayload {
            output_path: output_path.display().to_string(),
            mirror_paths: vec![mirror_path.display().to_string()],
            write_outputs: true,
            task: "trace metadata rustification".to_string(),
            matched_skills: vec!["plan-to-code".to_string()],
            owner: "plan-to-code".to_string(),
            gate: "none".to_string(),
            overlay: None,
            reroute_count: Some(0),
            retry_count: Some(1),
            artifact_paths: vec!["artifacts/current/SESSION_SUMMARY.md".to_string()],
            verification_status: "passed".to_string(),
            session_id: None,
            job_id: None,
            event_stream_path: None,
            event_stream_text: None,
            compaction_manifest_path: None,
            compaction_manifest_text: None,
            compaction_state_text: None,
            compaction_artifact_index_text: None,
            compaction_delta_text: None,
            stream_scope_fields: None,
            framework_version: Some("phase1".to_string()),
            metadata_schema_version: Some("trace-metadata-v2".to_string()),
            routing_runtime_version: Some(9),
            runtime_path: None,
            ts: Some("2026-04-23T00:00:00Z".to_string()),
            trace_event_schema_version: None,
            trace_event_sink_schema_version: None,
            parallel_group: None,
            supervisor_projection: None,
            control_plane: None,
            stream: None,
            events: None,
        })
        .expect("write trace metadata");

        assert_eq!(response.schema_version, TRACE_METADATA_WRITE_SCHEMA_VERSION);
        assert_eq!(response.authority, TRACE_METADATA_WRITE_AUTHORITY);
        assert_eq!(response.output_path, output_path.display().to_string());
        assert_eq!(response.routing_runtime_version, 9);
        assert!(response.payload_text.contains("\"version\": 1"));
        let primary = fs::read_to_string(&output_path).expect("read primary trace metadata");
        let mirror = fs::read_to_string(&mirror_path).expect("read mirror trace metadata");
        assert_eq!(primary, mirror);
        assert!(primary.contains("\"schema_version\": \"trace-metadata-v2\""));
        assert!(primary.contains("\"task\": \"trace metadata rustification\""));

        fs::remove_file(&output_path).expect("cleanup primary trace metadata");
        fs::remove_file(&mirror_path).expect("cleanup mirror trace metadata");
        fs::remove_dir_all(
            mirror_path
                .parent()
                .and_then(Path::parent)
                .expect("cleanup mirror root"),
        )
        .expect("cleanup mirror directories");
    }

    #[test]
    fn stdio_request_dispatches_write_trace_metadata_payload() {
        let output_path = temp_json_path("trace-metadata-write-stdio");
        let response = handle_stdio_json_line(&format!(
            "{{\"id\":3,\"op\":\"write_trace_metadata\",\"payload\":{{\"output_path\":\"{}\",\"task\":\"trace metadata stdio\",\"matched_skills\":[\"plan-to-code\"],\"owner\":\"plan-to-code\",\"gate\":\"none\",\"overlay\":null,\"reroute_count\":0,\"retry_count\":0,\"artifact_paths\":[],\"verification_status\":\"passed\",\"metadata_schema_version\":\"trace-metadata-v2\",\"routing_runtime_version\":11}}}}",
            output_path.display()
        ));
        assert!(response.ok);
        assert_eq!(response.id, json!(3));
        assert_eq!(
            response.payload.expect("payload")["schema_version"],
            json!(TRACE_METADATA_WRITE_SCHEMA_VERSION)
        );
        let persisted = fs::read_to_string(&output_path).expect("read stdio trace metadata");
        assert!(persisted.contains("\"routing_runtime_version\": 11"));
        fs::remove_file(&output_path).expect("cleanup stdio trace metadata");
    }

    #[test]
    fn write_trace_metadata_fails_closed_for_explicit_bad_trace_source() {
        let output_path = temp_json_path("trace-metadata-bad-source");
        let missing_trace_path = temp_trace_path("trace-metadata-missing-source");
        let response = write_trace_metadata(TraceMetadataWriteRequestPayload {
            output_path: output_path.display().to_string(),
            mirror_paths: Vec::new(),
            write_outputs: true,
            task: "trace metadata missing source".to_string(),
            matched_skills: Vec::new(),
            owner: "plan-to-code".to_string(),
            gate: "none".to_string(),
            overlay: None,
            reroute_count: Some(0),
            retry_count: Some(0),
            artifact_paths: Vec::new(),
            verification_status: "passed".to_string(),
            session_id: None,
            job_id: None,
            event_stream_path: Some(missing_trace_path.display().to_string()),
            event_stream_text: None,
            compaction_manifest_path: None,
            compaction_manifest_text: None,
            compaction_state_text: None,
            compaction_artifact_index_text: None,
            compaction_delta_text: None,
            stream_scope_fields: None,
            framework_version: None,
            metadata_schema_version: Some("trace-metadata-v2".to_string()),
            routing_runtime_version: Some(11),
            runtime_path: None,
            ts: Some("2026-04-23T00:00:00Z".to_string()),
            trace_event_schema_version: None,
            trace_event_sink_schema_version: None,
            parallel_group: None,
            supervisor_projection: None,
            control_plane: None,
            stream: None,
            events: Some(Vec::new()),
        });

        assert!(response.is_err());
        assert!(!output_path.exists());
    }

    #[test]
    fn write_text_payload_uses_unique_temp_paths_under_concurrency() {
        let output_path = temp_json_path("atomic-write-concurrent");
        let mut workers = Vec::new();
        for index in 0..32 {
            let path = output_path.clone();
            workers.push(spawn(move || {
                write_text_payload(&path, &format!("payload-{index}"))
                    .expect("concurrent atomic write");
            }));
        }
        for worker in workers {
            worker.join().expect("join writer");
        }

        let persisted = fs::read_to_string(&output_path).expect("read final payload");
        assert!(persisted.starts_with("payload-"));
        let tmp_entries = fs::read_dir(output_path.parent().expect("output parent"))
            .expect("read temp dir")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.file_name().to_string_lossy().starts_with(
                    output_path
                        .file_name()
                        .expect("file name")
                        .to_string_lossy()
                        .as_ref(),
                ) && entry.file_name().to_string_lossy().ends_with(".tmp")
            })
            .count();
        assert_eq!(tmp_entries, 0);

        fs::remove_file(&output_path).expect("cleanup concurrent write output");
    }

    #[test]
    fn subscribe_attached_runtime_events_returns_cursor_not_event_payload() {
        let binding_artifact_path = temp_json_path("subscribe-transport");
        let resume_manifest_path = temp_json_path("subscribe-resume-manifest");
        let trace_stream_path = temp_trace_path("subscribe-trace-stream");

        fs::write(
            &binding_artifact_path,
            serde_json::to_string_pretty(&json!({
                "schema_version": "runtime-event-transport-v1",
                "stream_id": "stream::job-subscribe",
                "session_id": "session-subscribe",
                "job_id": "job-subscribe",
                "binding_backend_family": "filesystem",
                "resume_mode": "after_event_id"
            }))
            .expect("serialize binding artifact"),
        )
        .expect("write binding artifact");
        fs::write(
            &resume_manifest_path,
            serde_json::to_string_pretty(&json!({
                "schema_version": "runtime-resume-manifest-v1",
                "session_id": "session-subscribe",
                "job_id": "job-subscribe",
                "event_transport_path": binding_artifact_path.display().to_string(),
                "trace_stream_path": trace_stream_path.display().to_string()
            }))
            .expect("serialize resume manifest"),
        )
        .expect("write resume manifest");
        fs::write(
            &trace_stream_path,
            concat!(
                "{\"event_id\":\"evt-1\",\"kind\":\"job.started\",\"session_id\":\"session-subscribe\",\"job_id\":\"job-subscribe\"}\n",
                "{\"event_id\":\"evt-2\",\"kind\":\"job.completed\",\"session_id\":\"session-subscribe\",\"job_id\":\"job-subscribe\"}\n"
            ),
        )
        .expect("write trace stream");

        let response = subscribe_attached_runtime_events(json!({
            "resume_manifest_path": resume_manifest_path.display().to_string(),
            "after_event_id": "evt-1",
            "limit": 1
        }))
        .expect("subscribe attached events");

        assert_eq!(response["events"].as_array().expect("events").len(), 1);
        assert_eq!(
            response["next_cursor"],
            json!({"event_id": "evt-2", "event_index": 1})
        );
        assert_eq!(response["next_cursor"]["kind"], Value::Null);

        fs::remove_file(&binding_artifact_path).expect("cleanup binding artifact");
        fs::remove_file(&resume_manifest_path).expect("cleanup resume manifest");
        fs::remove_file(&trace_stream_path).expect("cleanup trace stream");
    }
}
