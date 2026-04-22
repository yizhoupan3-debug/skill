#![recursion_limit = "256"]

use clap::{ArgAction, Parser};
use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use strsim::jaro_winkler;

mod framework_profile;
mod framework_runtime;

use framework_profile::{
    build_codex_artifact_bundle, build_profile_bundle, build_profile_bundle_with_legacy_alias,
    load_framework_profile,
};
use framework_runtime::{
    build_framework_contract_summary_envelope, build_framework_runtime_snapshot_envelope,
    resolve_repo_root_arg,
};

const ROUTE_DECISION_SCHEMA_VERSION: &str = "router-rs-route-decision-v1";
const ROUTE_POLICY_SCHEMA_VERSION: &str = "router-rs-route-policy-v1";
const ROUTE_SNAPSHOT_SCHEMA_VERSION: &str = "router-rs-route-snapshot-v1";
const ROUTE_REPORT_SCHEMA_VERSION: &str = "router-rs-route-report-v2";
const ROUTE_AUTHORITY: &str = "rust-route-core";
const PROFILE_COMPILE_AUTHORITY: &str = "rust-route-compiler";
const EXECUTION_SCHEMA_VERSION: &str = "router-rs-execute-response-v1";
const EXECUTION_METADATA_SCHEMA_VERSION: &str = "router-rs-execution-kernel-metadata-v1";
const EXECUTION_AUTHORITY: &str = "rust-execution-cli";
const EXECUTION_KERNEL_BRIDGE_KIND: &str = "rust-execution-kernel-slice";
const EXECUTION_KERNEL_BRIDGE_AUTHORITY: &str = "rust-execution-kernel-authority";
const EXECUTION_KERNEL_CONTRACT_MODE: &str = "rust-live-primary";
const EXECUTION_KERNEL_FALLBACK_POLICY: &str = "infrastructure-only-explicit";
const EXECUTION_KERNEL_DELEGATE_FAMILY: &str = "rust-cli";
const EXECUTION_KERNEL_DELEGATE_IMPL: &str = "router-rs";
const EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY: &str = "live_primary";
const EXECUTION_RESPONSE_SHAPE_DRY_RUN: &str = "dry_run";
const EXECUTION_PROMPT_PREVIEW_OWNER: &str = "rust-execution-cli";
const EXECUTION_MODEL_ID_SOURCE: &str = "aggregator-response.model";
const RUNTIME_CONTROL_PLANE_SCHEMA_VERSION: &str = "router-rs-runtime-control-plane-v1";
const RUNTIME_CONTROL_PLANE_AUTHORITY: &str = "rust-runtime-control-plane";
const SANDBOX_CONTROL_SCHEMA_VERSION: &str = "router-rs-sandbox-control-v1";
const SANDBOX_CONTROL_AUTHORITY: &str = "rust-sandbox-control";
const BACKGROUND_CONTROL_SCHEMA_VERSION: &str = "router-rs-background-control-v1";
const BACKGROUND_CONTROL_AUTHORITY: &str = "rust-background-control";
const TRACE_DESCRIPTOR_SCHEMA_VERSION: &str = "router-rs-trace-descriptor-v1";
const TRACE_DESCRIPTOR_AUTHORITY: &str = "rust-runtime-trace-descriptor";
const CHECKPOINT_RESUME_MANIFEST_SCHEMA_VERSION: &str = "router-rs-checkpoint-resume-manifest-v1";
const CHECKPOINT_RESUME_MANIFEST_AUTHORITY: &str = "rust-runtime-checkpoint-manifest";
const TRANSPORT_BINDING_WRITE_SCHEMA_VERSION: &str = "router-rs-transport-binding-write-v1";
const TRANSPORT_BINDING_WRITE_AUTHORITY: &str = "rust-runtime-transport-binding-writer";
const CHECKPOINT_MANIFEST_WRITE_SCHEMA_VERSION: &str = "router-rs-checkpoint-manifest-write-v1";
const CHECKPOINT_MANIFEST_WRITE_AUTHORITY: &str = "rust-runtime-checkpoint-manifest-writer";
const ATTACHED_RUNTIME_EVENT_ATTACH_AUTHORITY: &str = "rust-runtime-attached-event-transport";
const TRACE_STREAM_REPLAY_SCHEMA_VERSION: &str = "router-rs-trace-stream-replay-v1";
const TRACE_STREAM_INSPECT_SCHEMA_VERSION: &str = "router-rs-trace-stream-inspect-v1";
const TRACE_COMPACTION_DELTA_WRITE_SCHEMA_VERSION: &str =
    "router-rs-trace-compaction-delta-write-v1";
const TRACE_STREAM_IO_AUTHORITY: &str = "rust-runtime-trace-io";
const RUNTIME_OBSERVABILITY_EXPORTER_SCHEMA_VERSION: &str = "runtime-observability-exporter-v1";
const RUNTIME_OBSERVABILITY_METRIC_RECORD_SCHEMA_VERSION: &str =
    "runtime-observability-metric-record-v1";
const RUNTIME_OBSERVABILITY_METRIC_CATALOG_SCHEMA_VERSION: &str =
    "runtime-observability-metric-catalog-v1";
const RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION: &str = "runtime-observability-metrics-v1";
const RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION: &str = "runtime-observability-dashboard-v1";
const RUNTIME_OBSERVABILITY_SIGNAL_VOCABULARY: &str = "shared-runtime-v1";
const OVERLAY_ONLY_SKILLS: [&str; 4] = [
    "execution-audit-codex",
    "humanizer",
    "i18n-l10n",
    "iterative-optimizer",
];
const ARTIFACT_GATE_PHRASES: [&str; 12] = [
    "pdf",
    "docx",
    "xlsx",
    "ppt",
    "pptx",
    "excel",
    "spreadsheet",
    "word 文档",
    "word 文件",
    "表格",
    "工作簿",
    "幻灯片",
];

#[derive(Parser, Debug)]
#[command(name = "router-rs")]
#[command(about = "Fast Rust routing core for skill lookup")]
struct Cli {
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
    route_json: bool,
    #[arg(long)]
    route_policy_json: bool,
    #[arg(long)]
    route_snapshot_json: bool,
    #[arg(long)]
    execute_json: bool,
    #[arg(long)]
    runtime_control_plane_json: bool,
    #[arg(long)]
    sandbox_control_json: bool,
    #[arg(long)]
    background_control_json: bool,
    #[arg(long)]
    describe_transport_json: bool,
    #[arg(long)]
    describe_handoff_json: bool,
    #[arg(long)]
    checkpoint_resume_manifest_json: bool,
    #[arg(long)]
    write_transport_binding_json: bool,
    #[arg(long)]
    write_checkpoint_resume_manifest_json: bool,
    #[arg(long)]
    attach_runtime_event_transport_json: bool,
    #[arg(long)]
    subscribe_attached_runtime_events_json: bool,
    #[arg(long)]
    cleanup_attached_runtime_event_transport_json: bool,
    #[arg(long)]
    runtime_observability_exporter_json: bool,
    #[arg(long)]
    runtime_observability_metric_catalog_json: bool,
    #[arg(long)]
    runtime_observability_dashboard_json: bool,
    #[arg(long)]
    runtime_metric_record_json: bool,
    #[arg(long)]
    trace_stream_replay_json: bool,
    #[arg(long)]
    trace_stream_inspect_json: bool,
    #[arg(long)]
    write_trace_compaction_delta_json: bool,
    #[arg(long)]
    framework_runtime_snapshot_json: bool,
    #[arg(long)]
    framework_contract_summary_json: bool,
    #[arg(long)]
    profile_json: bool,
    #[arg(long)]
    profile_artifacts_json: bool,
    #[arg(long)]
    route_report_json: bool,
    #[arg(long)]
    include_legacy_alias_artifact: bool,
    #[arg(long)]
    route_mode: Option<String>,
    #[arg(long)]
    rust_route_snapshot_json: Option<String>,
    #[arg(long)]
    route_decision_json: Option<String>,
    #[arg(long)]
    route_snapshot_input_json: Option<String>,
    #[arg(long)]
    execute_input_json: Option<String>,
    #[arg(long)]
    sandbox_control_input_json: Option<String>,
    #[arg(long)]
    background_control_input_json: Option<String>,
    #[arg(long)]
    describe_transport_input_json: Option<String>,
    #[arg(long)]
    describe_handoff_input_json: Option<String>,
    #[arg(long)]
    checkpoint_resume_manifest_input_json: Option<String>,
    #[arg(long)]
    write_transport_binding_input_json: Option<String>,
    #[arg(long)]
    write_checkpoint_resume_manifest_input_json: Option<String>,
    #[arg(long)]
    attach_runtime_event_transport_input_json: Option<String>,
    #[arg(long)]
    subscribe_attached_runtime_events_input_json: Option<String>,
    #[arg(long)]
    cleanup_attached_runtime_event_transport_input_json: Option<String>,
    #[arg(long)]
    runtime_metric_record_input_json: Option<String>,
    #[arg(long)]
    trace_stream_replay_input_json: Option<String>,
    #[arg(long)]
    trace_stream_inspect_input_json: Option<String>,
    #[arg(long)]
    write_trace_compaction_delta_input_json: Option<String>,
    #[arg(long, default_value = "route-cli")]
    session_id: String,
    #[arg(long, default_value_t = true, action = ArgAction::Set, num_args = 1)]
    allow_overlay: bool,
    #[arg(long, default_value_t = true, action = ArgAction::Set, num_args = 1)]
    first_turn: bool,
}

#[derive(Debug, Clone)]
struct SkillRecord {
    slug: String,
    layer: String,
    owner: String,
    gate: String,
    priority: String,
    session_start: String,
    summary: String,
    health: f64,
    slug_lower: String,
    summary_lower: String,
    trigger_hints_lower: String,
    fuzzy_tokens: Vec<String>,
    gate_phrases: Vec<String>,
    trigger_hints: Vec<String>,
    name_tokens: HashSet<String>,
    keyword_tokens: HashSet<String>,
}

#[derive(Debug, Serialize)]
struct MatchRow {
    slug: String,
    layer: String,
    owner: String,
    gate: String,
    description: String,
    score: f64,
    matched_terms: usize,
    total_terms: usize,
}

#[derive(Debug, Clone)]
struct RouteCandidate {
    record: SkillRecord,
    score: f64,
    reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RecordsCacheKey {
    runtime_path: Option<PathBuf>,
    manifest_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct RecordsCacheEntry {
    runtime_mtime: Option<SystemTime>,
    manifest_mtime: Option<SystemTime>,
    records: Arc<Vec<SkillRecord>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RouteDecisionSnapshotPayload {
    engine: String,
    selected_skill: String,
    overlay_skill: Option<String>,
    layer: String,
    score: f64,
    score_bucket: String,
    reasons: Vec<String>,
    reasons_class: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RouteDiffReportPayload {
    report_schema_version: String,
    authority: String,
    mode: String,
    primary_engine: String,
    evidence_kind: String,
    strict_verification: bool,
    verification_passed: bool,
    verified_contract_fields: Vec<String>,
    contract_mismatch_fields: Vec<String>,
    route_snapshot: RouteDecisionSnapshotPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RouteDecision {
    decision_schema_version: String,
    authority: String,
    compile_authority: String,
    task: String,
    session_id: String,
    selected_skill: String,
    overlay_skill: Option<String>,
    layer: String,
    score: f64,
    reasons: Vec<String>,
    route_snapshot: RouteDecisionSnapshotPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RouteExecutionPolicyPayload {
    policy_schema_version: String,
    authority: String,
    mode: String,
    diagnostic_route_mode: String,
    primary_authority: String,
    route_result_engine: String,
    diagnostic_report_required: bool,
    strict_verification_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RouteSnapshotRequestPayload {
    engine: String,
    selected_skill: String,
    overlay_skill: Option<String>,
    layer: String,
    score: f64,
    reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RouteSnapshotEnvelopePayload {
    snapshot_schema_version: String,
    authority: String,
    route_snapshot: RouteDecisionSnapshotPayload,
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
    current_state: Option<String>,
    next_state: Option<String>,
    cleanup_failed: Option<bool>,
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
struct TraceStreamReplayRequestPayload {
    path: Option<String>,
    compaction_manifest_path: Option<String>,
    session_id: Option<String>,
    job_id: Option<String>,
    stream_scope_fields: Option<Vec<String>>,
    after_event_id: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TraceStreamInspectRequestPayload {
    path: Option<String>,
    compaction_manifest_path: Option<String>,
    session_id: Option<String>,
    job_id: Option<String>,
    stream_scope_fields: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TraceStreamReplayCursorPayload {
    event_id: Option<String>,
    event_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TraceStreamReplayResponsePayload {
    schema_version: String,
    authority: String,
    path: String,
    source_kind: String,
    event_count: usize,
    latest_event_id: Option<String>,
    latest_event_kind: Option<String>,
    latest_event_timestamp: Option<String>,
    latest_cursor: Option<Value>,
    after_event_id: Option<String>,
    window_start_index: usize,
    has_more: bool,
    next_cursor: Option<TraceStreamReplayCursorPayload>,
    events: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TraceStreamInspectResponsePayload {
    schema_version: String,
    authority: String,
    path: String,
    source_kind: String,
    event_count: usize,
    latest_event_id: Option<String>,
    latest_event_kind: Option<String>,
    latest_event_timestamp: Option<String>,
    latest_cursor: Option<Value>,
    recovery: Option<Value>,
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

#[derive(Debug, Clone, Deserialize)]
struct StdioJsonRequestPayload {
    id: Value,
    op: String,
    #[serde(default)]
    payload: Value,
}

#[derive(Debug, Clone, Serialize)]
struct StdioJsonResponsePayload {
    id: Value,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
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

impl SkillRecord {
    fn from_raw(raw: RawSkillRecord) -> Self {
        let RawSkillRecord {
            slug,
            layer,
            owner,
            gate,
            priority,
            session_start,
            summary,
            trigger_hints,
            health,
        } = raw;
        let slug_lower = normalize_text(&slug);
        let summary_lower = normalize_text(&summary);
        let trigger_hints_lower = normalize_text(&trigger_hints.join(" "));
        let mut fuzzy_source = String::with_capacity(
            slug_lower.len() + summary_lower.len() + trigger_hints_lower.len() + 2,
        );
        fuzzy_source.push_str(&slug_lower);
        fuzzy_source.push(' ');
        fuzzy_source.push_str(&trigger_hints_lower);
        fuzzy_source.push(' ');
        fuzzy_source.push_str(&summary_lower);

        let gate_phrases = gate_hint_phrases(&gate);
        let name_tokens = tokenize_query(&slug.replace('-', " "))
            .into_iter()
            .collect::<HashSet<_>>();
        let keyword_tokens = tokenize_query(&format!("{summary} {}", trigger_hints.join(" ")))
            .into_iter()
            .collect::<HashSet<_>>();

        Self {
            slug,
            layer,
            owner,
            gate,
            priority,
            session_start,
            summary,
            health,
            slug_lower,
            summary_lower,
            trigger_hints_lower,
            fuzzy_tokens: tokenize_query(&fuzzy_source),
            gate_phrases,
            trigger_hints,
            name_tokens,
            keyword_tokens,
        }
    }
}

#[derive(Debug)]
struct RawSkillRecord {
    slug: String,
    layer: String,
    owner: String,
    gate: String,
    priority: String,
    session_start: String,
    summary: String,
    trigger_hints: Vec<String>,
    health: f64,
}

fn main() -> Result<(), String> {
    let args = Cli::parse();
    if [
        args.json,
        args.stdio_json,
        args.route_json,
        args.route_policy_json,
        args.route_snapshot_json,
        args.execute_json,
        args.runtime_control_plane_json,
        args.sandbox_control_json,
        args.background_control_json,
        args.describe_transport_json,
        args.describe_handoff_json,
        args.checkpoint_resume_manifest_json,
        args.write_transport_binding_json,
        args.write_checkpoint_resume_manifest_json,
        args.attach_runtime_event_transport_json,
        args.subscribe_attached_runtime_events_json,
        args.cleanup_attached_runtime_event_transport_json,
        args.runtime_observability_exporter_json,
        args.runtime_observability_metric_catalog_json,
        args.runtime_observability_dashboard_json,
        args.runtime_metric_record_json,
        args.trace_stream_replay_json,
        args.trace_stream_inspect_json,
        args.write_trace_compaction_delta_json,
        args.framework_runtime_snapshot_json,
        args.framework_contract_summary_json,
        args.profile_json,
        args.profile_artifacts_json,
        args.route_report_json,
    ]
    .into_iter()
    .filter(|enabled| *enabled)
    .count()
        > 1
    {
        return Err(
            "choose only one output mode among --json, --stdio-json, --route-json, --route-policy-json, --route-snapshot-json, --execute-json, --runtime-control-plane-json, --sandbox-control-json, --background-control-json, --describe-transport-json, --describe-handoff-json, --checkpoint-resume-manifest-json, --write-transport-binding-json, --write-checkpoint-resume-manifest-json, --attach-runtime-event-transport-json, --subscribe-attached-runtime-events-json, --cleanup-attached-runtime-event-transport-json, --runtime-observability-exporter-json, --runtime-observability-metric-catalog-json, --runtime-observability-dashboard-json, --runtime-metric-record-json, --trace-stream-replay-json, --trace-stream-inspect-json, --write-trace-compaction-delta-json, --framework-runtime-snapshot-json, --framework-contract-summary-json, --route-report-json, --profile-json, and --profile-artifacts-json"
                .to_string(),
        );
    }

    if args.stdio_json {
        return run_stdio_json_loop();
    }

    if args.sandbox_control_json {
        let payload = serde_json::from_str::<SandboxControlRequestPayload>(
            args.sandbox_control_input_json.as_deref().ok_or_else(|| {
                "--sandbox-control-input-json is required with --sandbox-control-json".to_string()
            })?,
        )
        .map_err(|err| format!("parse sandbox control input failed: {err}"))?;
        println!(
            "{}",
            serde_json::to_string(&build_sandbox_control_response(payload)?)
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    if args.background_control_json {
        let payload = serde_json::from_str::<BackgroundControlRequestPayload>(
            args.background_control_input_json
                .as_deref()
                .ok_or_else(|| {
                    "--background-control-input-json is required with --background-control-json"
                        .to_string()
                })?,
        )
        .map_err(|err| format!("parse background control input failed: {err}"))?;
        println!(
            "{}",
            serde_json::to_string(&build_background_control_response(payload)?)
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    if args.describe_transport_json {
        let payload = serde_json::from_str::<Value>(
            args.describe_transport_input_json
                .as_deref()
                .ok_or_else(|| {
                    "--describe-transport-input-json is required with --describe-transport-json"
                        .to_string()
                })?,
        )
        .map_err(|err| format!("parse describe transport input failed: {err}"))?;
        println!(
            "{}",
            serde_json::to_string(&build_trace_transport_descriptor(payload)?)
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    if args.describe_handoff_json {
        let payload = serde_json::from_str::<Value>(
            args.describe_handoff_input_json.as_deref().ok_or_else(|| {
                "--describe-handoff-input-json is required with --describe-handoff-json".to_string()
            })?,
        )
        .map_err(|err| format!("parse describe handoff input failed: {err}"))?;
        println!(
            "{}",
            serde_json::to_string(&build_trace_handoff_descriptor(payload)?)
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    if args.checkpoint_resume_manifest_json {
        let payload = serde_json::from_str::<Value>(
            args.checkpoint_resume_manifest_input_json
                .as_deref()
                .ok_or_else(|| {
                    "--checkpoint-resume-manifest-input-json is required with --checkpoint-resume-manifest-json"
                        .to_string()
                })?,
        )
        .map_err(|err| format!("parse checkpoint resume manifest input failed: {err}"))?;
        println!(
            "{}",
            serde_json::to_string(&build_checkpoint_resume_manifest(payload)?)
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    if args.write_transport_binding_json {
        let payload = serde_json::from_str::<Value>(
            args.write_transport_binding_input_json
                .as_deref()
                .ok_or_else(|| {
                    "--write-transport-binding-input-json is required with --write-transport-binding-json"
                        .to_string()
                })?,
        )
        .map_err(|err| format!("parse write transport binding input failed: {err}"))?;
        println!(
            "{}",
            serde_json::to_string(&write_transport_binding_payload(payload)?)
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    if args.write_checkpoint_resume_manifest_json {
        let payload = serde_json::from_str::<Value>(
            args.write_checkpoint_resume_manifest_input_json
                .as_deref()
                .ok_or_else(|| {
                    "--write-checkpoint-resume-manifest-input-json is required with --write-checkpoint-resume-manifest-json"
                        .to_string()
                })?,
        )
        .map_err(|err| format!("parse write checkpoint resume manifest input failed: {err}"))?;
        println!(
            "{}",
            serde_json::to_string(&write_checkpoint_resume_manifest_payload(payload)?)
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    if args.attach_runtime_event_transport_json {
        let payload = serde_json::from_str::<Value>(
            args.attach_runtime_event_transport_input_json
                .as_deref()
                .ok_or_else(|| {
                    "--attach-runtime-event-transport-input-json is required with --attach-runtime-event-transport-json"
                        .to_string()
                })?,
        )
        .map_err(|err| format!("parse attach runtime event transport input failed: {err}"))?;
        println!(
            "{}",
            serde_json::to_string(&attach_runtime_event_transport(payload)?)
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    if args.subscribe_attached_runtime_events_json {
        let payload = serde_json::from_str::<Value>(
            args.subscribe_attached_runtime_events_input_json
                .as_deref()
                .ok_or_else(|| {
                    "--subscribe-attached-runtime-events-input-json is required with --subscribe-attached-runtime-events-json"
                        .to_string()
                })?,
        )
        .map_err(|err| format!("parse subscribe attached runtime events input failed: {err}"))?;
        println!(
            "{}",
            serde_json::to_string(&subscribe_attached_runtime_events(payload)?)
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    if args.cleanup_attached_runtime_event_transport_json {
        let payload = serde_json::from_str::<Value>(
            args.cleanup_attached_runtime_event_transport_input_json
                .as_deref()
                .ok_or_else(|| {
                    "--cleanup-attached-runtime-event-transport-input-json is required with --cleanup-attached-runtime-event-transport-json"
                        .to_string()
                })?,
        )
        .map_err(|err| format!("parse attached runtime event cleanup input failed: {err}"))?;
        println!(
            "{}",
            serde_json::to_string(&cleanup_attached_runtime_event_transport(payload)?)
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    if args.runtime_control_plane_json {
        println!(
            "{}",
            serde_json::to_string(&build_runtime_control_plane_payload())
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    if args.runtime_observability_exporter_json {
        println!(
            "{}",
            serde_json::to_string(&build_runtime_observability_exporter_descriptor())
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    if args.runtime_observability_metric_catalog_json {
        println!(
            "{}",
            serde_json::to_string(&build_runtime_observability_metric_catalog_payload())
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    if args.runtime_observability_dashboard_json {
        println!(
            "{}",
            serde_json::to_string(&runtime_observability_dashboard_schema())
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    if args.runtime_metric_record_json {
        let payload = serde_json::from_str::<Value>(
            args.runtime_metric_record_input_json
                .as_deref()
                .ok_or_else(|| {
                    "--runtime-metric-record-input-json is required with --runtime-metric-record-json"
                        .to_string()
                })?,
        )
        .map_err(|err| format!("parse runtime metric record input failed: {err}"))?;
        println!(
            "{}",
            serde_json::to_string(&build_runtime_metric_record(payload)?)
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    if args.trace_stream_replay_json {
        let payload = serde_json::from_str::<TraceStreamReplayRequestPayload>(
            args.trace_stream_replay_input_json
                .as_deref()
                .ok_or_else(|| {
                    "--trace-stream-replay-input-json is required with --trace-stream-replay-json"
                        .to_string()
                })?,
        )
        .map_err(|err| format!("parse trace stream replay input failed: {err}"))?;
        println!(
            "{}",
            serde_json::to_string(&replay_trace_stream(payload)?)
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    if args.trace_stream_inspect_json {
        let payload = serde_json::from_str::<TraceStreamInspectRequestPayload>(
            args.trace_stream_inspect_input_json
                .as_deref()
                .ok_or_else(|| {
                    "--trace-stream-inspect-input-json is required with --trace-stream-inspect-json"
                        .to_string()
                })?,
        )
        .map_err(|err| format!("parse trace stream inspect input failed: {err}"))?;
        println!(
            "{}",
            serde_json::to_string(&inspect_trace_stream(payload)?)
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    if args.write_trace_compaction_delta_json {
        let payload = serde_json::from_str::<TraceCompactionDeltaWriteRequestPayload>(
            args.write_trace_compaction_delta_input_json
                .as_deref()
                .ok_or_else(|| {
                    "--write-trace-compaction-delta-input-json is required with --write-trace-compaction-delta-json"
                        .to_string()
                })?,
        )
        .map_err(|err| format!("parse trace compaction delta write input failed: {err}"))?;
        println!(
            "{}",
            serde_json::to_string(&write_trace_compaction_delta(payload)?)
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    if args.framework_runtime_snapshot_json {
        let repo_root = resolve_repo_root_arg(args.repo_root.as_deref())?;
        println!(
            "{}",
            serde_json::to_string(&build_framework_runtime_snapshot_envelope(&repo_root)?)
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    if args.framework_contract_summary_json {
        let repo_root = resolve_repo_root_arg(args.repo_root.as_deref())?;
        println!(
            "{}",
            serde_json::to_string(&build_framework_contract_summary_envelope(&repo_root)?)
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    if args.profile_json {
        let profile_path = args
            .framework_profile
            .as_deref()
            .ok_or_else(|| "--framework-profile is required with --profile-json".to_string())?;
        let profile = load_framework_profile(profile_path)?;
        let bundle = if args.include_legacy_alias_artifact {
            build_profile_bundle_with_legacy_alias(&profile, true)?
        } else {
            build_profile_bundle(&profile)?
        };
        println!(
            "{}",
            serde_json::to_string(&bundle)
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    if args.profile_artifacts_json {
        let profile_path = args.framework_profile.as_deref().ok_or_else(|| {
            "--framework-profile is required with --profile-artifacts-json".to_string()
        })?;
        let profile = load_framework_profile(profile_path)?;
        let artifacts = build_codex_artifact_bundle(&profile, args.include_legacy_alias_artifact)?;
        println!(
            "{}",
            serde_json::to_string(&artifacts)
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    if args.route_report_json {
        let mode = args
            .route_mode
            .as_deref()
            .ok_or_else(|| "--route-mode is required with --route-report-json".to_string())?;
        let route_decision = args
            .route_decision_json
            .as_deref()
            .map(serde_json::from_str::<RouteDecision>)
            .transpose()
            .map_err(|err| format!("parse route decision contract failed: {err}"))?;
        let rust_snapshot = match args.rust_route_snapshot_json.as_deref() {
            Some(raw) => serde_json::from_str::<RouteDecisionSnapshotPayload>(raw)
                .map_err(|err| format!("parse rust route snapshot failed: {err}"))?,
            None => route_decision
                .as_ref()
                .map(|decision| decision.route_snapshot.clone())
                .ok_or_else(|| {
                    "--route-report-json requires --rust-route-snapshot-json or --route-decision-json"
                        .to_string()
                })?,
        };
        let report = build_route_diff_report(mode, rust_snapshot, route_decision.as_ref())?;
        println!(
            "{}",
            serde_json::to_string(&report)
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    if args.route_policy_json {
        let mode = args
            .route_mode
            .as_deref()
            .ok_or_else(|| "--route-mode is required with --route-policy-json".to_string())?;
        let policy = build_route_policy(mode)?;
        println!(
            "{}",
            serde_json::to_string(&policy)
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    if args.route_snapshot_json {
        let payload = serde_json::from_str::<RouteSnapshotRequestPayload>(
            args.route_snapshot_input_json.as_deref().ok_or_else(|| {
                "--route-snapshot-input-json is required with --route-snapshot-json".to_string()
            })?,
        )
        .map_err(|err| format!("parse route snapshot input failed: {err}"))?;
        let snapshot = build_route_snapshot(
            &payload.engine,
            &payload.selected_skill,
            payload.overlay_skill.as_deref(),
            &payload.layer,
            payload.score,
            &payload.reasons,
        );
        let envelope = RouteSnapshotEnvelopePayload {
            snapshot_schema_version: ROUTE_SNAPSHOT_SCHEMA_VERSION.to_string(),
            authority: ROUTE_AUTHORITY.to_string(),
            route_snapshot: snapshot,
        };
        println!(
            "{}",
            serde_json::to_string(&envelope)
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    if args.execute_json {
        let payload = serde_json::from_str::<ExecuteRequestPayload>(
            args.execute_input_json.as_deref().ok_or_else(|| {
                "--execute-input-json is required with --execute-json".to_string()
            })?,
        )
        .map_err(|err| format!("parse execute input failed: {err}"))?;
        let response = execute_request(payload)?;
        println!(
            "{}",
            serde_json::to_string(&response)
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    let records = load_records(args.runtime.as_deref(), args.manifest.as_deref())?;
    let query = args
        .query
        .as_deref()
        .ok_or_else(|| "missing --query".to_string())?;

    if args.route_json {
        let decision = route_task(
            &records,
            query,
            &args.session_id,
            args.allow_overlay,
            args.first_turn,
        )?;
        println!(
            "{}",
            serde_json::to_string(&decision)
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    let rows = search_skills(&records, query, args.limit);
    if args.json {
        println!(
            "{}",
            serde_json::to_string(&rows)
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    if rows.is_empty() {
        println!("No skills found matching: {}", query);
        return Ok(());
    }

    println!("Found {} matches for '{}':", rows.len(), query);
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
    Ok(())
}

fn run_stdio_json_loop() -> Result<(), String> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();
    for line_result in stdin.lock().lines() {
        let line = line_result.map_err(|err| format!("read stdio request failed: {err}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let response = handle_stdio_json_line(&line);
        let encoded = serde_json::to_string(&response)
            .map_err(|err| format!("serialize stdio response failed: {err}"))?;
        writeln!(stdout_lock, "{encoded}")
            .map_err(|err| format!("write stdio response failed: {err}"))?;
        stdout_lock
            .flush()
            .map_err(|err| format!("flush stdio response failed: {err}"))?;
    }
    Ok(())
}

fn handle_stdio_json_line(line: &str) -> StdioJsonResponsePayload {
    match serde_json::from_str::<StdioJsonRequestPayload>(line) {
        Ok(request) => match dispatch_stdio_json_request(&request.op, request.payload) {
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
        },
        Err(err) => StdioJsonResponsePayload {
            id: Value::Null,
            ok: false,
            payload: None,
            error: Some(format!("parse stdio request failed: {err}")),
        },
    }
}

fn dispatch_stdio_json_request(op: &str, payload: Value) -> Result<Value, String> {
    match op {
        "route" => dispatch_stdio_route(payload),
        "search_skills" => dispatch_stdio_search_skills(payload),
        "execute" => {
            let request = serde_json::from_value::<ExecuteRequestPayload>(payload)
                .map_err(|err| format!("parse execute input failed: {err}"))?;
            serde_json::to_value(execute_request(request)?)
                .map_err(|err| format!("serialize execute output failed: {err}"))
        }
        "route_report" => dispatch_stdio_route_report(payload),
        "route_policy" => dispatch_stdio_route_policy(payload),
        "route_snapshot" => dispatch_stdio_route_snapshot(payload),
        "compile_profile_bundle" => dispatch_stdio_compile_profile_bundle(payload),
        "compile_codex_profile_artifacts" => {
            dispatch_stdio_compile_codex_profile_artifacts(payload)
        }
        "runtime_control_plane" => Ok(build_runtime_control_plane_payload()),
        "sandbox_control" => {
            let request = serde_json::from_value::<SandboxControlRequestPayload>(payload)
                .map_err(|err| format!("parse sandbox control input failed: {err}"))?;
            serde_json::to_value(build_sandbox_control_response(request)?)
                .map_err(|err| format!("serialize sandbox control output failed: {err}"))
        }
        "runtime_observability_exporter_descriptor" => {
            serde_json::to_value(build_runtime_observability_exporter_descriptor()).map_err(|err| {
                format!("serialize runtime observability exporter output failed: {err}")
            })
        }
        "runtime_observability_metric_catalog" => serde_json::to_value(
            build_runtime_observability_metric_catalog_payload(),
        )
        .map_err(|err| {
            format!("serialize runtime observability metric catalog output failed: {err}")
        }),
        "runtime_observability_dashboard_schema" => {
            serde_json::to_value(runtime_observability_dashboard_schema()).map_err(|err| {
                format!("serialize runtime observability dashboard output failed: {err}")
            })
        }
        "runtime_metric_record" => serde_json::to_value(build_runtime_metric_record(payload)?)
            .map_err(|err| format!("serialize runtime metric record output failed: {err}")),
        "background_control" => {
            let request = serde_json::from_value::<BackgroundControlRequestPayload>(payload)
                .map_err(|err| format!("parse background control input failed: {err}"))?;
            serde_json::to_value(build_background_control_response(request)?)
                .map_err(|err| format!("serialize background control output failed: {err}"))
        }
        "describe_transport" => build_trace_transport_descriptor(payload),
        "describe_handoff" => build_trace_handoff_descriptor(payload),
        "checkpoint_resume_manifest" => build_checkpoint_resume_manifest(payload),
        "write_transport_binding" => write_transport_binding_payload(payload),
        "write_checkpoint_resume_manifest" => write_checkpoint_resume_manifest_payload(payload),
        "attach_runtime_event_transport" => attach_runtime_event_transport(payload),
        "subscribe_attached_runtime_events" => subscribe_attached_runtime_events(payload),
        "cleanup_attached_runtime_event_transport" => {
            cleanup_attached_runtime_event_transport(payload)
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
        "write_trace_compaction_delta" => {
            let request =
                serde_json::from_value::<TraceCompactionDeltaWriteRequestPayload>(payload)
                    .map_err(|err| format!("parse trace compaction delta input failed: {err}"))?;
            serde_json::to_value(write_trace_compaction_delta(request)?)
                .map_err(|err| format!("serialize trace compaction delta output failed: {err}"))
        }
        "framework_runtime_snapshot" => dispatch_stdio_framework_runtime_snapshot(payload),
        "framework_contract_summary" => dispatch_stdio_framework_contract_summary(payload),
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
    let runtime_path = optional_non_empty_string(&payload, "runtime_path").map(PathBuf::from);
    let manifest_path = optional_non_empty_string(&payload, "manifest_path").map(PathBuf::from);
    let records = load_records_cached_for_stdio(runtime_path.as_deref(), manifest_path.as_deref())?;
    serde_json::to_value(route_task(
        records.as_ref(),
        &query,
        &session_id,
        allow_overlay,
        first_turn,
    )?)
    .map_err(|err| format!("serialize route output failed: {err}"))
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
    let records = load_records_cached_for_stdio(runtime_path.as_deref(), manifest_path.as_deref())?;
    Ok(json!({
        "rows": search_skills(records.as_ref(), &query, limit),
    }))
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
    serde_json::to_value(build_framework_runtime_snapshot_envelope(Path::new(
        &repo_root,
    ))?)
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

fn dispatch_stdio_compile_profile_bundle(payload: Value) -> Result<Value, String> {
    let profile_path = required_non_empty_string(&payload, "profile_path", "stdio profile bundle")?;
    let include_legacy_alias_artifact = payload
        .get("include_legacy_alias_artifact")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let profile = load_framework_profile(Path::new(&profile_path))?;
    let bundle = if include_legacy_alias_artifact {
        build_profile_bundle_with_legacy_alias(&profile, true)?
    } else {
        build_profile_bundle(&profile)?
    };
    serde_json::to_value(bundle)
        .map_err(|err| format!("serialize profile bundle output failed: {err}"))
}

fn dispatch_stdio_compile_codex_profile_artifacts(payload: Value) -> Result<Value, String> {
    let profile_path =
        required_non_empty_string(&payload, "profile_path", "stdio codex profile artifacts")?;
    let include_legacy_alias_artifact = payload
        .get("include_legacy_alias_artifact")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let profile = load_framework_profile(Path::new(&profile_path))?;
    let artifacts = build_codex_artifact_bundle(&profile, include_legacy_alias_artifact)?;
    serde_json::to_value(artifacts)
        .map_err(|err| format!("serialize codex profile artifacts output failed: {err}"))
}

fn load_records(
    runtime_path: Option<&Path>,
    manifest_path: Option<&Path>,
) -> Result<Vec<SkillRecord>, String> {
    if let Some(path) = runtime_path {
        if path.exists() {
            let mut records = load_records_from_runtime(path)?;
            if let Some(manifest) = manifest_path {
                if manifest.exists() {
                    let meta = load_manifest_route_meta(manifest)?;
                    for record in &mut records {
                        if let Some((priority, session_start)) = meta.get(&record.slug) {
                            record.priority = priority.clone();
                            record.session_start = session_start.clone();
                        }
                    }
                }
            }
            return Ok(records);
        }
    }
    if let Some(path) = manifest_path {
        if path.exists() {
            return load_records_from_manifest(path);
        }
    }
    Err("No routing index found.".to_string())
}

fn records_cache_key(
    runtime_path: Option<&Path>,
    manifest_path: Option<&Path>,
) -> RecordsCacheKey {
    RecordsCacheKey {
        runtime_path: runtime_path.map(Path::to_path_buf),
        manifest_path: manifest_path.map(Path::to_path_buf),
    }
}

fn file_modified_at(path: Option<&Path>) -> Option<SystemTime> {
    path.and_then(|item| fs::metadata(item).ok()?.modified().ok())
}

fn records_cache() -> &'static Mutex<HashMap<RecordsCacheKey, RecordsCacheEntry>> {
    static RECORDS_CACHE: OnceLock<Mutex<HashMap<RecordsCacheKey, RecordsCacheEntry>>> =
        OnceLock::new();
    RECORDS_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn load_records_cached_for_stdio(
    runtime_path: Option<&Path>,
    manifest_path: Option<&Path>,
) -> Result<Arc<Vec<SkillRecord>>, String> {
    let key = records_cache_key(runtime_path, manifest_path);
    let runtime_mtime = file_modified_at(runtime_path);
    let manifest_mtime = file_modified_at(manifest_path);

    {
        let cache = records_cache()
            .lock()
            .map_err(|_| "route records cache lock poisoned".to_string())?;
        if let Some(entry) = cache.get(&key) {
            if entry.runtime_mtime == runtime_mtime && entry.manifest_mtime == manifest_mtime {
                return Ok(Arc::clone(&entry.records));
            }
        }
    }

    let records = Arc::new(load_records(runtime_path, manifest_path)?);
    let entry = RecordsCacheEntry {
        runtime_mtime,
        manifest_mtime,
        records: Arc::clone(&records),
    };
    let mut cache = records_cache()
        .lock()
        .map_err(|_| "route records cache lock poisoned".to_string())?;
    cache.insert(key, entry);
    Ok(records)
}

fn load_manifest_route_meta(path: &Path) -> Result<HashMap<String, (String, String)>, String> {
    let payload = read_json(path)?;
    let rows = payload
        .get("skills")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("manifest missing skills rows: {}", path.display()))?;
    let keys = payload
        .get("keys")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("manifest missing keys: {}", path.display()))?;

    let key_index = keys
        .iter()
        .enumerate()
        .filter_map(|(idx, key)| key.as_str().map(|raw| (raw.to_string(), idx)))
        .collect::<HashMap<_, _>>();

    let idx_slug = *key_index
        .get("slug")
        .ok_or_else(|| format!("manifest missing slug key: {}", path.display()))?;
    let idx_priority = key_index.get("priority").copied();
    let idx_session_start = key_index.get("session_start").copied();

    let mut meta = HashMap::new();
    for row in rows.iter().filter_map(Value::as_array) {
        if row.len() <= idx_slug {
            continue;
        }
        let slug = value_to_string(&row[idx_slug]);
        let priority = idx_priority
            .and_then(|idx| row.get(idx))
            .map(value_to_string)
            .unwrap_or_else(|| "P2".to_string());
        let session_start = idx_session_start
            .and_then(|idx| row.get(idx))
            .map(value_to_string)
            .unwrap_or_else(|| "n/a".to_string());
        meta.insert(slug, (priority, session_start));
    }
    Ok(meta)
}

fn load_records_from_runtime(path: &Path) -> Result<Vec<SkillRecord>, String> {
    let payload = read_json(path)?;
    let rows = payload
        .get("skills")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("runtime index missing skills rows: {}", path.display()))?;
    let keys = payload
        .get("keys")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("runtime index missing keys: {}", path.display()))?;

    let mut index: HashMap<String, usize> = HashMap::new();
    for (idx, key) in keys.iter().enumerate() {
        if let Some(raw) = key.as_str() {
            index.insert(raw.to_string(), idx);
        }
    }

    let idx_slug = *index
        .get("slug")
        .ok_or_else(|| format!("runtime index missing slug key: {}", path.display()))?;
    let idx_layer = *index
        .get("layer")
        .ok_or_else(|| format!("runtime index missing layer key: {}", path.display()))?;
    let idx_owner = *index
        .get("owner")
        .ok_or_else(|| format!("runtime index missing owner key: {}", path.display()))?;
    let idx_gate = *index
        .get("gate")
        .ok_or_else(|| format!("runtime index missing gate key: {}", path.display()))?;
    let idx_summary = *index
        .get("summary")
        .or_else(|| index.get("description"))
        .ok_or_else(|| format!("runtime index missing summary key: {}", path.display()))?;
    let idx_trigger_hints = *index
        .get("trigger_hints")
        .or_else(|| index.get("triggers"))
        .ok_or_else(|| {
            format!(
                "runtime index missing trigger_hints key: {}",
                path.display()
            )
        })?;
    let idx_health = *index
        .get("health")
        .ok_or_else(|| format!("runtime index missing health key: {}", path.display()))?;
    let idx_priority = index.get("priority").copied();
    let idx_session_start = index.get("session_start").copied();
    let required_max = *[
        idx_slug,
        idx_layer,
        idx_owner,
        idx_gate,
        idx_summary,
        idx_trigger_hints,
        idx_health,
    ]
    .iter()
    .max()
    .expect("required columns");

    rows.iter()
        .filter_map(Value::as_array)
        .filter(|row| row.len() > required_max)
        .map(|row| {
            Ok(SkillRecord::from_raw(RawSkillRecord {
                slug: value_to_string(&row[idx_slug]),
                layer: value_to_string(&row[idx_layer]),
                owner: value_to_string(&row[idx_owner]),
                gate: value_to_string(&row[idx_gate]),
                priority: idx_priority
                    .and_then(|idx| row.get(idx))
                    .map(value_to_string)
                    .unwrap_or_else(|| "P2".to_string()),
                session_start: idx_session_start
                    .and_then(|idx| row.get(idx))
                    .map(value_to_string)
                    .unwrap_or_else(|| "n/a".to_string()),
                summary: value_to_string(&row[idx_summary]),
                trigger_hints: value_to_string_list(&row[idx_trigger_hints]),
                health: value_to_f64(&row[idx_health]).unwrap_or(100.0),
            }))
        })
        .collect()
}

fn load_records_from_manifest(path: &Path) -> Result<Vec<SkillRecord>, String> {
    let payload = read_json(path)?;
    let rows = payload
        .get("skills")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("manifest missing skills rows: {}", path.display()))?;
    let keys = payload
        .get("keys")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("manifest missing keys: {}", path.display()))?;

    let key_index = keys
        .iter()
        .enumerate()
        .filter_map(|(idx, key)| key.as_str().map(|raw| (raw.to_string(), idx)))
        .collect::<HashMap<_, _>>();

    let idx_slug = *key_index
        .get("slug")
        .ok_or_else(|| format!("manifest missing slug key: {}", path.display()))?;
    let idx_layer = *key_index
        .get("layer")
        .ok_or_else(|| format!("manifest missing layer key: {}", path.display()))?;
    let idx_owner = *key_index
        .get("owner")
        .ok_or_else(|| format!("manifest missing owner key: {}", path.display()))?;
    let idx_gate = *key_index
        .get("gate")
        .ok_or_else(|| format!("manifest missing gate key: {}", path.display()))?;
    let idx_desc = *key_index
        .get("description")
        .or_else(|| key_index.get("summary"))
        .ok_or_else(|| format!("manifest missing description key: {}", path.display()))?;
    let idx_trigger_hints = *key_index
        .get("trigger_hints")
        .or_else(|| key_index.get("triggers"))
        .ok_or_else(|| format!("manifest missing trigger_hints key: {}", path.display()))?;
    let idx_health = *key_index
        .get("health")
        .ok_or_else(|| format!("manifest missing health key: {}", path.display()))?;
    let idx_priority = key_index.get("priority").copied();
    let idx_session_start = key_index.get("session_start").copied();
    let required_max = *[
        idx_slug,
        idx_layer,
        idx_owner,
        idx_gate,
        idx_desc,
        idx_trigger_hints,
        idx_health,
    ]
    .iter()
    .max()
    .expect("required columns");

    rows.iter()
        .filter_map(Value::as_array)
        .filter(|row| row.len() > required_max)
        .map(|row| {
            Ok(SkillRecord::from_raw(RawSkillRecord {
                slug: value_to_string(&row[idx_slug]),
                layer: value_to_string(&row[idx_layer]),
                owner: value_to_string(&row[idx_owner]),
                gate: value_to_string(&row[idx_gate]),
                priority: idx_priority
                    .and_then(|idx| row.get(idx))
                    .map(value_to_string)
                    .unwrap_or_else(|| "P2".to_string()),
                session_start: idx_session_start
                    .and_then(|idx| row.get(idx))
                    .map(value_to_string)
                    .unwrap_or_else(|| "n/a".to_string()),
                summary: value_to_string(&row[idx_desc]),
                trigger_hints: value_to_string_list(&row[idx_trigger_hints]),
                health: value_to_f64(&row[idx_health]).unwrap_or(100.0),
            }))
        })
        .collect()
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("failed reading {}: {err}", path.display()))?;
    serde_json::from_str(&text).map_err(|err| format!("failed parsing {}: {err}", path.display()))
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(raw) => raw.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn value_to_string_list(value: &Value) -> Vec<String> {
    match value {
        Value::Array(items) => items
            .iter()
            .map(value_to_string)
            .filter(|item| !item.trim().is_empty())
            .collect(),
        Value::Null => Vec::new(),
        _ => split_phrases(&value_to_string(value)),
    }
}

fn value_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.parse::<f64>().ok(),
        _ => None,
    }
}

fn normalize_text(text: &str) -> String {
    text.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn tokenize_query(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let lowered = normalize_text(text);
    let mut tokens = Vec::new();
    for capture in token_regex().find_iter(&lowered) {
        let token = capture.as_str().to_string();
        if seen.insert(token.clone()) {
            tokens.push(token);
        }
    }
    tokens
}

fn token_regex() -> &'static Regex {
    static TOKEN_REGEX: OnceLock<Regex> = OnceLock::new();
    TOKEN_REGEX.get_or_init(|| {
        Regex::new(r"[A-Za-z0-9.+#/-]+|[\u{4e00}-\u{9fff}]{2,}").expect("token regex")
    })
}

fn phrase_split_regex() -> &'static Regex {
    static PHRASE_SPLIT_REGEX: OnceLock<Regex> = OnceLock::new();
    PHRASE_SPLIT_REGEX.get_or_init(|| Regex::new(r"[,\n/|，]+").expect("phrase split regex"))
}

fn common_route_stop_tokens() -> &'static [&'static str] {
    &[
        "一个",
        "帮我",
        "帮我看",
        "我看",
        "先给",
        "给我",
        "给我一",
        "我一个",
        "写一",
        "写一个",
        "看这",
        "这张",
        "然后",
        "输出",
        "问题",
        "checklist",
        "skill",
        "路由",
    ]
}

fn is_meta_routing_task(query_text: &str) -> bool {
    (query_text.contains("skill") || query_text.contains("skill.md"))
        && ["路由", "触发", "routing", "router", "route"]
            .iter()
            .any(|marker| query_text.contains(marker))
}

fn wordlike_token_regex() -> &'static Regex {
    static WORDLIKE_TOKEN_REGEX: OnceLock<Regex> = OnceLock::new();
    WORDLIKE_TOKEN_REGEX
        .get_or_init(|| Regex::new(r"^[a-z0-9.+#/_-]+$").expect("wordlike token regex"))
}

fn tokenize_route_text(text: &str) -> Vec<String> {
    token_regex()
        .find_iter(&normalize_text(text))
        .map(|capture| capture.as_str().to_string())
        .collect()
}

fn split_phrases(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut phrases = Vec::new();
    for raw in phrase_split_regex().split(text) {
        let normalized = normalize_text(raw);
        if normalized.is_empty() || normalized == "none" {
            continue;
        }
        if seen.insert(normalized.clone()) {
            phrases.push(normalized);
        }
    }
    phrases
}

fn is_overlay_record(record: &SkillRecord) -> bool {
    normalize_text(&record.owner) == "overlay"
        || OVERLAY_ONLY_SKILLS.iter().any(|slug| slug == &record.slug)
}

fn can_be_primary_owner(record: &SkillRecord) -> bool {
    !matches!(normalize_text(&record.owner).as_str(), "gate" | "overlay")
}

fn phrase_token_matches(task_token: &str, phrase_token: &str) -> bool {
    if wordlike_token_regex().is_match(phrase_token) {
        task_token == phrase_token
    } else {
        task_token.contains(phrase_token)
    }
}

fn text_matches_phrase(task_tokens: &[String], phrase: &str) -> bool {
    let phrase_tokens = tokenize_route_text(phrase);
    if phrase_tokens.is_empty() {
        return false;
    }
    if phrase_tokens.len() == 1 {
        return task_tokens
            .iter()
            .any(|task_token| phrase_token_matches(task_token, &phrase_tokens[0]));
    }
    if phrase_tokens.len() > task_tokens.len() {
        return false;
    }
    for start in 0..=(task_tokens.len() - phrase_tokens.len()) {
        if phrase_tokens
            .iter()
            .enumerate()
            .all(|(offset, phrase_token)| {
                phrase_token_matches(&task_tokens[start + offset], phrase_token)
            })
        {
            return true;
        }
    }
    false
}

fn gate_hint_phrases(gate: &str) -> Vec<String> {
    match gate {
        "source" => vec![
            "官方".to_string(),
            "官方文档".to_string(),
            "文档".to_string(),
            "docs".to_string(),
            "readme".to_string(),
            "api".to_string(),
            "openai".to_string(),
            "github".to_string(),
            "look up".to_string(),
            "search".to_string(),
        ],
        "artifact" => ARTIFACT_GATE_PHRASES
            .iter()
            .map(|phrase| (*phrase).to_string())
            .collect(),
        "evidence" => vec![
            "报错".to_string(),
            "失败".to_string(),
            "崩".to_string(),
            "截图".to_string(),
            "渲染".to_string(),
            "日志".to_string(),
            "traceback".to_string(),
            "error".to_string(),
            "bug".to_string(),
            "why".to_string(),
            "为什么".to_string(),
        ],
        "delegation" => vec![
            "sidecar".to_string(),
            "subagent".to_string(),
            "delegation".to_string(),
            "并行 sidecar".to_string(),
            "子代理".to_string(),
            "主线程".to_string(),
            "local-supervisor".to_string(),
            "跨文件".to_string(),
            "长运行".to_string(),
        ],
        _ => Vec::new(),
    }
}

fn term_score(term: &str, record: &SkillRecord) -> f64 {
    if term == record.slug_lower {
        return 16.0;
    }
    if record.slug_lower.contains(term) {
        return 12.0;
    }
    if record.trigger_hints_lower.contains(term) {
        return 9.0;
    }
    if record.summary_lower.contains(term) {
        return 5.0;
    }

    if term.chars().count() >= 4 {
        let mut best: f64 = 0.0;
        for token in &record.fuzzy_tokens {
            let ratio = jaro_winkler(term, token);
            if ratio >= 0.84 {
                best = best.max(3.5 + ratio);
            }
        }
        return best;
    }

    0.0
}

fn search_skills(records: &[SkillRecord], query: &str, limit: usize) -> Vec<MatchRow> {
    let terms = tokenize_query(query);
    if terms.is_empty() {
        return Vec::new();
    }

    let required_matches = if terms.len() <= 2 {
        1
    } else {
        usize::max(2, ((terms.len() as f64) * 0.4).ceil() as usize)
    };

    let mut rows = Vec::new();

    for record in records {
        let mut matched_terms = 0usize;
        let mut score = 0.0f64;
        for term in &terms {
            let current = term_score(term, record);
            if current > 0.0 {
                matched_terms += 1;
                score += current;
            }
        }
        if matched_terms < required_matches {
            continue;
        }
        score += record.health.min(100.0) / 100.0;
        if normalize_text(&record.gate) != "none" {
            score += 0.25;
        }
        rows.push(MatchRow {
            slug: record.slug.clone(),
            layer: record.layer.clone(),
            owner: record.owner.clone(),
            gate: record.gate.clone(),
            description: record.summary.clone(),
            score: round2(score),
            matched_terms,
            total_terms: terms.len(),
        });
    }

    rows.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| right.matched_terms.cmp(&left.matched_terms))
            .then_with(|| left.slug.cmp(&right.slug))
    });
    rows.truncate(limit);
    rows
}

fn route_task(
    records: &[SkillRecord],
    query: &str,
    session_id: &str,
    allow_overlay: bool,
    first_turn: bool,
) -> Result<RouteDecision, String> {
    if records.is_empty() {
        return Err("No skill records available for route decision.".to_string());
    }
    let normalized_query = normalize_text(query);
    let query_token_list = tokenize_route_text(query);
    let query_tokens = query_token_list
        .iter()
        .filter(|token| !common_route_stop_tokens().contains(&token.as_str()))
        .cloned()
        .collect::<HashSet<String>>();

    let candidates = records
        .iter()
        .map(|record| {
            score_route_candidate(
                record,
                &normalized_query,
                &query_token_list,
                &query_tokens,
                first_turn,
            )
        })
        .collect::<Vec<_>>();
    let viable = candidates
        .into_iter()
        .filter(|candidate| candidate.score > 0.0)
        .collect::<Vec<_>>();

    if viable.is_empty() {
        let fallback = fallback_owner(records)?;
        return Ok(RouteDecision {
            decision_schema_version: ROUTE_DECISION_SCHEMA_VERSION.to_string(),
            authority: ROUTE_AUTHORITY.to_string(),
            compile_authority: PROFILE_COMPILE_AUTHORITY.to_string(),
            task: query.to_string(),
            session_id: session_id.to_string(),
            selected_skill: fallback.slug.clone(),
            overlay_skill: None,
            layer: fallback.layer.clone(),
            score: 0.0,
            reasons: vec![
                "No explicit keyword hit; fell back to highest-priority layer owner.".to_string(),
            ],
            route_snapshot: build_route_snapshot(
                "rust",
                &fallback.slug,
                None,
                &fallback.layer,
                0.0,
                &[
                    "No explicit keyword hit; fell back to highest-priority layer owner."
                        .to_string(),
                ],
            ),
        });
    }

    let selected = pick_owner(viable);
    let overlay = if allow_overlay {
        pick_overlay(records, &query_token_list, &selected.record)
    } else {
        None
    };

    let filtered_overlay = overlay
        .as_ref()
        .filter(|item| *item != &selected.record.slug)
        .cloned();

    Ok(RouteDecision {
        decision_schema_version: ROUTE_DECISION_SCHEMA_VERSION.to_string(),
        authority: ROUTE_AUTHORITY.to_string(),
        compile_authority: PROFILE_COMPILE_AUTHORITY.to_string(),
        task: query.to_string(),
        session_id: session_id.to_string(),
        selected_skill: selected.record.slug.clone(),
        overlay_skill: filtered_overlay,
        layer: selected.record.layer.clone(),
        score: round2(selected.score),
        route_snapshot: build_route_snapshot(
            "rust",
            &selected.record.slug,
            overlay
                .as_deref()
                .filter(|item| *item != selected.record.slug.as_str()),
            &selected.record.layer,
            round2(selected.score),
            &selected.reasons,
        ),
        reasons: selected.reasons,
    })
}

fn build_route_snapshot(
    engine: &str,
    selected_skill: &str,
    overlay_skill: Option<&str>,
    layer: &str,
    score: f64,
    reasons: &[String],
) -> RouteDecisionSnapshotPayload {
    RouteDecisionSnapshotPayload {
        engine: engine.to_string(),
        selected_skill: selected_skill.to_string(),
        overlay_skill: overlay_skill.map(|value| value.to_string()),
        layer: layer.to_string(),
        score,
        score_bucket: score_bucket(score),
        reasons: reasons.to_vec(),
        reasons_class: reasons_class(reasons),
    }
}

fn build_route_diff_report(
    mode: &str,
    rust_snapshot: RouteDecisionSnapshotPayload,
    route_decision: Option<&RouteDecision>,
) -> Result<RouteDiffReportPayload, String> {
    let normalized_mode = mode.trim().to_ascii_lowercase();
    let strict_verification = match normalized_mode.as_str() {
        "shadow" => false,
        "verify" => true,
        _ => {
            return Err(format!(
                "unsupported route mode for --route-report-json: {mode}"
            ))
        }
    };
    let (verified_contract_fields, contract_mismatch_fields) =
        compare_route_contract_to_snapshot(route_decision, &rust_snapshot);

    Ok(RouteDiffReportPayload {
        report_schema_version: ROUTE_REPORT_SCHEMA_VERSION.to_string(),
        authority: ROUTE_AUTHORITY.to_string(),
        mode: normalized_mode,
        primary_engine: "rust".to_string(),
        evidence_kind: "rust-owned-snapshot".to_string(),
        strict_verification,
        verification_passed: contract_mismatch_fields.is_empty(),
        verified_contract_fields,
        contract_mismatch_fields,
        route_snapshot: rust_snapshot,
    })
}

fn compare_route_contract_to_snapshot(
    route_decision: Option<&RouteDecision>,
    rust_snapshot: &RouteDecisionSnapshotPayload,
) -> (Vec<String>, Vec<String>) {
    let Some(route_decision) = route_decision else {
        return (Vec::new(), Vec::new());
    };

    let mut verified_fields = Vec::new();
    let mut mismatch_fields = Vec::new();

    let expected_fields = [
        (
            "engine",
            route_decision.route_snapshot.engine.as_str(),
            rust_snapshot.engine.as_str(),
        ),
        (
            "selected_skill",
            route_decision.selected_skill.as_str(),
            rust_snapshot.selected_skill.as_str(),
        ),
        (
            "layer",
            route_decision.layer.as_str(),
            rust_snapshot.layer.as_str(),
        ),
    ];
    for (field, expected, actual) in expected_fields {
        if expected == actual {
            verified_fields.push(field.to_string());
        } else {
            mismatch_fields.push(field.to_string());
        }
    }

    if route_decision.overlay_skill == rust_snapshot.overlay_skill {
        verified_fields.push("overlay_skill".to_string());
    } else {
        mismatch_fields.push("overlay_skill".to_string());
    }

    (verified_fields, mismatch_fields)
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

fn build_sandbox_control_response(
    payload: SandboxControlRequestPayload,
) -> Result<SandboxControlResponsePayload, String> {
    match payload.operation.as_str() {
        "transition" => {
            let current_state = payload
                .current_state
                .clone()
                .ok_or_else(|| "sandbox control transition requires current_state".to_string())?;
            let next_state = payload
                .next_state
                .clone()
                .ok_or_else(|| "sandbox control transition requires next_state".to_string())?;
            let allowed = matches!(
                (current_state.as_str(), next_state.as_str()),
                ("created", "warm")
                    | ("warm", "busy")
                    | ("busy", "draining")
                    | ("draining", "recycled")
                    | ("draining", "failed")
                    | ("warm", "failed")
                    | ("busy", "failed")
                    | ("recycled", "warm")
            );
            Ok(SandboxControlResponsePayload {
                schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
                authority: SANDBOX_CONTROL_AUTHORITY.to_string(),
                operation: payload.operation,
                current_state: Some(current_state.clone()),
                next_state: Some(next_state.clone()),
                allowed,
                resolved_state: Some(next_state.clone()),
                reason: if allowed {
                    "transition-accepted".to_string()
                } else {
                    "invalid-transition".to_string()
                },
                error: if allowed {
                    None
                } else {
                    Some(format!(
                        "invalid sandbox transition: {:?} -> {:?}",
                        current_state, next_state
                    ))
                },
            })
        }
        "cleanup" => {
            let current_state = payload
                .current_state
                .clone()
                .unwrap_or_else(|| "draining".to_string());
            let cleanup_failed = payload.cleanup_failed.unwrap_or(false);
            let resolved_state = if cleanup_failed { "failed" } else { "recycled" };
            let allowed = matches!(current_state.as_str(), "draining");
            Ok(SandboxControlResponsePayload {
                schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
                authority: SANDBOX_CONTROL_AUTHORITY.to_string(),
                operation: payload.operation,
                current_state: Some(current_state.clone()),
                next_state: Some(resolved_state.to_string()),
                allowed,
                resolved_state: Some(resolved_state.to_string()),
                reason: if !allowed {
                    "cleanup-invalid-state".to_string()
                } else if cleanup_failed {
                    "cleanup-failed".to_string()
                } else {
                    "cleanup-completed".to_string()
                },
                error: if allowed {
                    None
                } else {
                    Some(format!(
                        "invalid sandbox cleanup state: {:?} -> {:?}",
                        current_state, resolved_state
                    ))
                },
            })
        }
        other => Err(format!("unsupported sandbox control operation: {other}")),
    }
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
    lines.push("- Use plain Chinese unless the user asks otherwise.".to_string());
    lines.push("- Be brief, clear, and friendly.".to_string());
    lines.push(
        "- Keep the default reply to one short paragraph unless a list is truly needed."
            .to_string(),
    );
    lines.push("- Avoid internal runtime or routing jargon unless the user asks.".to_string());
    lines.push(
        "- If a technical term is necessary, explain it in simple words the first time."
            .to_string(),
    );
    if !payload.reasons.is_empty() {
        lines.push("Task cues:".to_string());
        for reason in &payload.reasons {
            let reason = reason.trim();
            if !reason.is_empty() {
                lines.push(format!("- {reason}"));
            }
        }
    }
    if payload.selected_skill == "idea-to-plan" {
        lines.push("Planning output: converge the strategy into outline.md, decision_log.md, assumptions.md, open_questions.md, plan_rubric.md, and code_list.md.".to_string());
        lines.push("Switch to checklist-writting only after the direction is fixed and the remaining work is execution breakdown.".to_string());
    }
    lines.push("Use the selected skill to solve the user's actual task.".to_string());
    lines.join("\n")
}

fn build_steady_state_execution_kernel_metadata(response_shape: &str) -> Map<String, Value> {
    let mut metadata = Map::new();
    metadata.insert(
        "execution_kernel_metadata_schema_version".to_string(),
        Value::String(EXECUTION_METADATA_SCHEMA_VERSION.to_string()),
    );
    metadata.insert(
        "execution_kernel".to_string(),
        Value::String(EXECUTION_KERNEL_BRIDGE_KIND.to_string()),
    );
    metadata.insert(
        "execution_kernel_authority".to_string(),
        Value::String(EXECUTION_KERNEL_BRIDGE_AUTHORITY.to_string()),
    );
    metadata.insert(
        "execution_kernel_contract_mode".to_string(),
        Value::String(EXECUTION_KERNEL_CONTRACT_MODE.to_string()),
    );
    metadata.insert(
        "execution_kernel_fallback_policy".to_string(),
        Value::String(EXECUTION_KERNEL_FALLBACK_POLICY.to_string()),
    );
    metadata.insert(
        "execution_kernel_in_process_replacement_complete".to_string(),
        Value::Bool(true),
    );
    metadata.insert(
        "execution_kernel_delegate".to_string(),
        Value::String(EXECUTION_KERNEL_DELEGATE_IMPL.to_string()),
    );
    metadata.insert(
        "execution_kernel_delegate_authority".to_string(),
        Value::String(EXECUTION_AUTHORITY.to_string()),
    );
    metadata.insert(
        "execution_kernel_delegate_family".to_string(),
        Value::String(EXECUTION_KERNEL_DELEGATE_FAMILY.to_string()),
    );
    metadata.insert(
        "execution_kernel_delegate_impl".to_string(),
        Value::String(EXECUTION_KERNEL_DELEGATE_IMPL.to_string()),
    );
    metadata.insert(
        "execution_kernel_live_primary".to_string(),
        Value::String(EXECUTION_KERNEL_DELEGATE_IMPL.to_string()),
    );
    metadata.insert(
        "execution_kernel_live_primary_authority".to_string(),
        Value::String(EXECUTION_AUTHORITY.to_string()),
    );
    metadata.insert("execution_kernel_live_fallback".to_string(), Value::Null);
    metadata.insert(
        "execution_kernel_live_fallback_authority".to_string(),
        Value::Null,
    );
    metadata.insert(
        "execution_kernel_live_fallback_enabled".to_string(),
        Value::Bool(false),
    );
    metadata.insert(
        "execution_kernel_live_fallback_mode".to_string(),
        Value::String("disabled".to_string()),
    );
    metadata.insert(
        "execution_kernel_response_shape".to_string(),
        Value::String(response_shape.to_string()),
    );
    metadata.insert(
        "execution_kernel_prompt_preview_owner".to_string(),
        Value::String(EXECUTION_PROMPT_PREVIEW_OWNER.to_string()),
    );
    metadata
}

fn build_execution_kernel_contracts_by_mode() -> Map<String, Value> {
    let mut contracts = Map::new();
    contracts.insert(
        EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY.to_string(),
        Value::Object(build_steady_state_execution_kernel_metadata(
            EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY,
        )),
    );
    contracts.insert(
        EXECUTION_RESPONSE_SHAPE_DRY_RUN.to_string(),
        Value::Object(build_steady_state_execution_kernel_metadata(
            EXECUTION_RESPONSE_SHAPE_DRY_RUN,
        )),
    );
    contracts
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
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|err| format!("build reqwest client failed: {err}"))?;
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

fn required_non_empty_string(payload: &Value, key: &str, context: &str) -> Result<String, String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .ok_or_else(|| format!("{context} requires non-empty {key}"))
}

fn optional_non_empty_string(payload: &Value, key: &str) -> Option<String> {
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
        "bridge_kind": optional_non_empty_string(payload, "bridge_kind").unwrap_or_else(|| "runtime_event_bridge".to_string()),
        "transport_family": optional_non_empty_string(payload, "transport_family").unwrap_or_else(|| "host-facing-bridge".to_string()),
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
        "cleanup_semantics": optional_non_empty_string(payload, "cleanup_semantics").unwrap_or_else(|| "bridge_cache_only".to_string()),
        "cleanup_preserves_replay": optional_bool(payload, "cleanup_preserves_replay").unwrap_or(true),
        "replay_reseed_supported": optional_bool(payload, "replay_reseed_supported").unwrap_or(true),
        "chunk_schema_version": optional_non_empty_string(payload, "chunk_schema_version").unwrap_or_else(|| "runtime-event-bridge-v1".to_string()),
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
    let tmp_path = path.with_file_name(format!("{file_name}.tmp"));
    fs::write(&tmp_path, serialized.as_bytes())
        .map_err(|err| format!("write temp payload {} failed: {err}", tmp_path.display()))?;
    fs::rename(&tmp_path, path)
        .map_err(|err| format!("replace payload {} failed: {err}", path.display()))?;
    Ok(serialized.len())
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

#[derive(Debug, Clone)]
enum ResolvedStorageBackend {
    Filesystem,
    Sqlite {
        db_path: PathBuf,
        storage_root: PathBuf,
    },
}

fn normalize_runtime_path(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("runtime attach path must be non-empty".to_string());
    }
    let candidate = PathBuf::from(trimmed);
    if candidate.is_absolute() {
        return Ok(candidate);
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(candidate))
        .map_err(|err| format!("resolve runtime attach path failed: {err}"))
}

fn normalize_optional_runtime_path(path: Option<String>) -> Result<Option<PathBuf>, String> {
    path.map(|value| normalize_runtime_path(&value)).transpose()
}

fn env_checkpoint_storage_db_path() -> Option<PathBuf> {
    std::env::var("CODEX_AGNO_CHECKPOINT_STORAGE_DB_FILE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn runtime_storage_db_name_candidates() -> Vec<String> {
    let mut ordered = Vec::new();
    let mut seen = HashSet::new();
    for candidate in [
        env_checkpoint_storage_db_path().and_then(|path| {
            path.file_name()
                .and_then(|value| value.to_str())
                .map(str::to_string)
        }),
        Some("runtime_checkpoint_store.sqlite3".to_string()),
    ]
    .into_iter()
    .flatten()
    {
        if seen.insert(candidate.clone()) {
            ordered.push(candidate);
        }
    }
    ordered
}

fn sqlite_connection(path: &Path) -> Result<Connection, String> {
    Connection::open(path).map_err(|err| {
        format!(
            "open sqlite runtime storage failed for {}: {err}",
            path.display()
        )
    })
}

fn sqlite_lookup_keys(path: &Path, storage_root: &Path) -> Result<(String, String), String> {
    let resolved_path = normalize_runtime_path(&path.display().to_string())?;
    let resolved_root = normalize_runtime_path(&storage_root.display().to_string())?;
    let stable_key = resolved_path
        .strip_prefix(&resolved_root)
        .map_err(|_| {
            format!(
                "sqlite runtime storage path {} must stay under storage root {}",
                resolved_path.display(),
                resolved_root.display()
            )
        })?
        .to_string_lossy()
        .replace('\\', "/");
    let legacy_key = resolved_path.display().to_string();
    Ok((stable_key, legacy_key))
}

fn sqlite_payload_exists(path: &Path, db_path: &Path, storage_root: &Path) -> Result<bool, String> {
    let (stable_key, legacy_key) = sqlite_lookup_keys(path, storage_root)?;
    let conn = sqlite_connection(db_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT 1 FROM runtime_storage_payloads WHERE payload_key = ?1 OR payload_key = ?2 LIMIT 1",
        )
        .map_err(|err| format!("prepare sqlite exists query failed: {err}"))?;
    let exists = stmt
        .query_row(params![stable_key, legacy_key], |row| row.get::<_, i64>(0))
        .optional()
        .map_err(|err| format!("run sqlite exists query failed: {err}"))?
        .is_some();
    Ok(exists)
}

fn sqlite_read_text(path: &Path, db_path: &Path, storage_root: &Path) -> Result<String, String> {
    let (stable_key, legacy_key) = sqlite_lookup_keys(path, storage_root)?;
    let conn = sqlite_connection(db_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT payload_text FROM runtime_storage_payloads WHERE payload_key = ?1 OR payload_key = ?2 LIMIT 1",
        )
        .map_err(|err| format!("prepare sqlite read query failed: {err}"))?;
    stmt.query_row(params![stable_key, legacy_key], |row| {
        row.get::<_, String>(0)
    })
    .map_err(|err| format!("read sqlite payload failed for {}: {err}", path.display()))
}

fn storage_artifact_exists(path: &Path, storage_backend: Option<&ResolvedStorageBackend>) -> bool {
    if path.exists() {
        return true;
    }
    match storage_backend {
        Some(ResolvedStorageBackend::Filesystem) => false,
        Some(ResolvedStorageBackend::Sqlite {
            db_path,
            storage_root,
        }) => sqlite_payload_exists(path, db_path, storage_root).unwrap_or(false),
        None => false,
    }
}

fn storage_read_text(
    path: &Path,
    storage_backend: Option<&ResolvedStorageBackend>,
) -> Result<String, String> {
    if path.exists() {
        return fs::read_to_string(path)
            .map_err(|err| format!("read artifact failed for {}: {err}", path.display()));
    }
    match storage_backend {
        Some(ResolvedStorageBackend::Filesystem) | None => {
            Err(format!("artifact does not exist: {}", path.display()))
        }
        Some(ResolvedStorageBackend::Sqlite {
            db_path,
            storage_root,
        }) => sqlite_read_text(path, db_path, storage_root),
    }
}

fn resolve_storage_backend(paths: &[PathBuf]) -> Option<ResolvedStorageBackend> {
    if paths.is_empty() {
        return None;
    }
    if paths.iter().any(|path| path.exists()) {
        return Some(ResolvedStorageBackend::Filesystem);
    }

    let mut roots = Vec::new();
    let mut seen_roots = HashSet::new();
    for path in paths {
        let mut candidates = Vec::new();
        let parent = path.parent();
        let parent_name = parent
            .and_then(|value| value.file_name())
            .and_then(|name| name.to_str());
        let grandparent = parent.and_then(Path::parent);
        let grandparent_name = grandparent
            .and_then(|value| value.file_name())
            .and_then(|name| name.to_str());

        if parent_name == Some("runtime_event_transports")
            || parent_name == Some("trace_compaction")
        {
            if let Some(root) = grandparent {
                candidates.push(root.to_path_buf());
            }
            if let Some(root) = grandparent.and_then(Path::parent) {
                candidates.push(root.to_path_buf());
            }
        }
        if grandparent_name == Some("trace_compaction") {
            if let Some(root) = grandparent.and_then(Path::parent) {
                candidates.push(root.to_path_buf());
            }
        }
        if let Some(parent) = path.parent() {
            candidates.push(parent.to_path_buf());
        }
        for candidate in candidates {
            let normalized = normalize_runtime_path(&candidate.display().to_string()).ok()?;
            if seen_roots.insert(normalized.clone()) {
                roots.push(normalized);
            }
        }
    }

    if let Some(db_path) = env_checkpoint_storage_db_path()
        .and_then(|path| normalize_runtime_path(&path.display().to_string()).ok())
        .filter(|path| path.is_absolute() && path.exists())
    {
        for root in &roots {
            let backend = ResolvedStorageBackend::Sqlite {
                db_path: db_path.clone(),
                storage_root: root.clone(),
            };
            if paths
                .iter()
                .any(|path| storage_artifact_exists(path, Some(&backend)))
            {
                return Some(backend);
            }
        }
    }

    let db_name_candidates = runtime_storage_db_name_candidates();
    for root in &roots {
        for db_name in &db_name_candidates {
            let db_path = root.join(db_name);
            if !db_path.exists() {
                continue;
            }
            let backend = ResolvedStorageBackend::Sqlite {
                db_path,
                storage_root: root.clone(),
            };
            if paths
                .iter()
                .any(|path| storage_artifact_exists(path, Some(&backend)))
            {
                return Some(backend);
            }
        }
    }

    None
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

fn normalize_attach_request(
    payload: &Value,
) -> Result<(Option<String>, Option<String>, Option<String>), String> {
    let binding_artifact_path = optional_non_empty_string(payload, "binding_artifact_path");
    let handoff_path = optional_non_empty_string(payload, "handoff_path");
    let resume_manifest_path = optional_non_empty_string(payload, "resume_manifest_path");
    let Some(attach_descriptor) = payload.get("attach_descriptor") else {
        return Ok((binding_artifact_path, handoff_path, resume_manifest_path));
    };
    if attach_descriptor.is_null() {
        return Ok((binding_artifact_path, handoff_path, resume_manifest_path));
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
    let _ = descriptor_mapping(attach_descriptor, "resolution")?;
    let resolved_mapping = descriptor_mapping(attach_descriptor, "resolved_artifacts")?
        .unwrap_or_else(|| {
            attach_descriptor
                .as_object()
                .expect("attach descriptor object")
        });
    let descriptor_binding = mapping_string(resolved_mapping, "binding_artifact_path")?;
    let descriptor_handoff = mapping_string(resolved_mapping, "handoff_path")?;
    let descriptor_resume = mapping_string(resolved_mapping, "resume_manifest_path")?;
    Ok((
        merge_attach_path_values(
            binding_artifact_path,
            descriptor_binding,
            "binding_artifact_path",
        )?,
        merge_attach_path_values(handoff_path, descriptor_handoff, "handoff_path")?,
        merge_attach_path_values(
            resume_manifest_path,
            descriptor_resume,
            "resume_manifest_path",
        )?,
    ))
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

fn attach_runtime_event_transport(payload: Value) -> Result<Value, String> {
    let (binding_artifact_path, handoff_path, resume_manifest_path) =
        normalize_attach_request(&payload)?;
    if binding_artifact_path.is_none() && handoff_path.is_none() && resume_manifest_path.is_none() {
        return Err(
            "External runtime event attach requires a binding artifact, handoff manifest, or resume manifest path."
                .to_string(),
        );
    }

    let binding_path = normalize_optional_runtime_path(binding_artifact_path)?;
    let handoff_file = normalize_optional_runtime_path(handoff_path)?;
    let resume_file = normalize_optional_runtime_path(resume_manifest_path)?;
    let mut binding_source = binding_path
        .as_ref()
        .map(|_| "explicit_request".to_string());
    let handoff_source = handoff_file
        .as_ref()
        .map(|_| "explicit_request".to_string());
    let mut resume_source = resume_file.as_ref().map(|_| "explicit_request".to_string());

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
    let attach_descriptor = json!({
        "schema_version": "runtime-event-attach-descriptor-v1",
        "attach_mode": "process_external_artifact_replay",
        "artifact_backend_family": artifact_backend_family,
        "source_transport_method": "describe_runtime_event_transport",
        "source_handoff_method": "describe_runtime_event_handoff",
        "attach_method": "attach_runtime_event_transport",
        "subscribe_method": "subscribe_attached_runtime_events",
        "cleanup_method": "cleanup_attached_runtime_event_transport",
        "resume_mode": resume_mode,
        "cleanup_semantics": "no_persisted_state",
        "attach_capabilities": {
            "artifact_replay": true,
            "live_remote_stream": false,
            "cleanup_preserves_replay": true,
        },
        "recommended_entrypoint": "describe_runtime_event_handoff",
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
        "artifact_backend_family": optional_non_empty_string(&transport, "binding_backend_family"),
        "source_handoff_method": "describe_runtime_event_handoff",
        "source_transport_method": "describe_runtime_event_transport",
        "attach_method": "attach_runtime_event_transport",
        "subscribe_method": "subscribe_attached_runtime_events",
        "cleanup_method": "cleanup_attached_runtime_event_transport",
        "resume_mode": optional_non_empty_string(&transport, "resume_mode"),
        "transport": transport,
        "handoff": handoff,
        "resume_manifest": resume_manifest,
        "binding_artifact_path": transport_path.as_ref().map(|path| path.display().to_string()),
        "handoff_path": handoff_file.as_ref().map(|path| path.display().to_string()),
        "resume_manifest_path": resolved_resume_file.as_ref().map(|path| path.display().to_string()),
        "trace_stream_path": trace_stream_path.display().to_string(),
        "replay_supported": true,
        "cleanup_semantics": "no_persisted_state",
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
        compaction_manifest_path: None,
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
    let next_cursor = replay.next_cursor.as_ref().and_then(|cursor| {
        let event_id = cursor.event_id.as_ref()?;
        events
            .iter()
            .find(|payload| {
                payload.get("event_id").and_then(Value::as_str) == Some(event_id.as_str())
            })
            .cloned()
    });
    Ok(json!({
        "schema_version": "runtime-event-bridge-v1",
        "session_id": session_id,
        "job_id": job_id,
        "events": events,
        "next_cursor": next_cursor,
        "has_more": has_more,
        "after_event_id": optional_non_empty_string(&payload, "after_event_id"),
        "heartbeat": if heartbeat && replay.events.is_empty() {
            json!({
                "schema_version": "runtime-event-bridge-heartbeat-v1",
                "kind": "bridge.heartbeat",
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

fn build_runtime_control_plane_payload() -> Value {
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
            "projection": "python-thin-projection",
            "delegate_kind": "rust-runtime-control-plane",
        },
        "prompt_builder": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "prompt-contract-projection",
            "projection": "python-thin-projection",
            "delegate_kind": "rust-execution-cli",
        },
        "middleware": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "middleware-policy-projection",
            "projection": "python-thin-projection",
            "delegate_kind": "rust-runtime-control-plane",
        },
        "state": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "durable-background-state",
            "projection": "python-thin-projection",
            "delegate_kind": "filesystem-state-store",
        },
        "trace": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "trace-and-handoff",
            "projection": "python-thin-projection",
            "delegate_kind": "filesystem-trace-store",
        },
        "memory": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "memory-lifecycle",
            "projection": "python-thin-projection",
            "delegate_kind": "fact-memory-store",
        },
        "checkpoint": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "checkpoint-artifact-projection",
            "projection": "python-thin-projection",
            "delegate_kind": "filesystem-checkpointer",
        },
        "execution": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "execution-kernel-control",
            "projection": "python-thin-projection",
            "delegate_kind": "rust-execution-kernel-slice",
            "kernel_contract": Value::Object(build_steady_state_execution_kernel_metadata(
                EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY,
            )),
            "kernel_contract_by_mode": Value::Object(build_execution_kernel_contracts_by_mode()),
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
            "kernel_live_fallback_kind": Value::Null,
            "kernel_live_fallback_authority": Value::Null,
            "kernel_live_fallback_family": Value::Null,
            "kernel_live_fallback_impl": Value::Null,
            "kernel_live_fallback_enabled": false,
            "kernel_live_fallback_mode": "disabled",
            "kernel_mode_support": ["dry_run", "live"],
            "execution_schema_version": EXECUTION_SCHEMA_VERSION,
            "sandbox_lifecycle_contract": {
                "schema_version": "runtime-sandbox-lifecycle-v1",
                "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
                "role": "sandbox-lifecycle-control",
                "projection": "python-thin-projection",
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
                "control_operations": ["transition", "cleanup"],
                "runtime_probe_dimensions": ["cpu", "memory", "wall_clock", "output_size"],
            },
        },
        "agent_factory": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "retired-compatibility-agent-contract-handle",
            "projection": "python-retired-request-surface",
            "delegate_kind": "execution-kernel-compatibility-agent-v1",
        },
        "background": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "background-orchestration",
            "projection": "python-thin-projection",
            "delegate_kind": "rust-background-control-policy",
            "orchestration_contract": {
                "schema_version": "runtime-background-orchestration-v1",
                "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
                "role": "background-orchestration-control",
                "projection": "python-thin-projection",
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
        "python_authority_default": false,
        "python_host_role": "thin-projection",
        "rustification_status": {
            "runtime_primary_owner": "rust-control-plane",
            "runtime_primary_owner_authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "python_runtime_role": "compatibility-host",
            "steady_state_python_allowed": false,
            "hot_path_projection_mode": "descriptor-driven",
        },
        "runtime_host": {
            "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
            "role": "runtime-orchestration",
            "projection": "python-thin-projection",
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
        },
        "services": services,
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
        "trace_bridge_schema_version": "runtime-event-bridge-v1",
        "trace_handoff_schema_version": "runtime-event-handoff-v1",
        "ownership_lane": "rust-contract-lane",
        "producer_owner": "rust-control-plane",
        "producer_authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
        "exporter_owner": "rust-control-plane",
        "exporter_authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
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

fn gsd_execution_markers() -> [&'static str; 6] {
    [
        "gsd",
        "get shit done",
        "推进到底",
        "别停",
        "直接干完",
        "一路做完",
    ]
}

fn build_route_policy(mode: &str) -> Result<RouteExecutionPolicyPayload, String> {
    let normalized_mode = mode.trim().to_ascii_lowercase();
    let base = RouteExecutionPolicyPayload {
        policy_schema_version: ROUTE_POLICY_SCHEMA_VERSION.to_string(),
        authority: ROUTE_AUTHORITY.to_string(),
        mode: normalized_mode.clone(),
        diagnostic_route_mode: "none".to_string(),
        primary_authority: "rust".to_string(),
        route_result_engine: "rust".to_string(),
        diagnostic_report_required: false,
        strict_verification_required: false,
    };
    let policy = match normalized_mode.as_str() {
        "shadow" => RouteExecutionPolicyPayload {
            diagnostic_route_mode: "shadow".to_string(),
            diagnostic_report_required: true,
            ..base
        },
        "verify" => RouteExecutionPolicyPayload {
            diagnostic_route_mode: "verify".to_string(),
            diagnostic_report_required: true,
            strict_verification_required: true,
            ..base
        },
        "rust" => base,
        _ => {
            return Err(format!(
                "unsupported route mode for --route-policy-json: {mode}"
            ))
        }
    };
    if policy.diagnostic_report_required && policy.diagnostic_route_mode == "none" {
        return Err(
            "route policy declared diagnostics outside the diagnostic route mode".to_string(),
        );
    }
    if policy.strict_verification_required && !policy.diagnostic_report_required {
        return Err(
            "route policy declared strict verification without diagnostic reporting".to_string(),
        );
    }
    Ok(policy)
}

fn score_route_candidate(
    record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    query_tokens: &HashSet<String>,
    first_turn: bool,
) -> RouteCandidate {
    let mut score = 0.0f64;
    let mut reasons = Vec::new();

    if record.slug == "systematic-debugging" && is_meta_routing_task(query_text) {
        return RouteCandidate {
            record: record.clone(),
            score: 0.0,
            reasons: vec![
                "Suppressed: meta-routing repair request should not be treated as a generic runtime-debugging gate."
                    .to_string(),
            ],
        };
    }

    if !record.slug_lower.is_empty() && query_text.contains(&record.slug_lower) {
        score += 100.0;
        reasons.push(format!("Exact skill name matched: {}.", record.slug));
    }

    let matched_gates = record
        .gate_phrases
        .iter()
        .filter(|phrase| text_matches_phrase(query_token_list, phrase))
        .cloned()
        .collect::<Vec<_>>();
    if !matched_gates.is_empty() {
        score += 18.0 + i32::min(12, ((matched_gates.len() - 1) as i32) * 6) as f64;
        reasons.push(format!(
            "Routing gate matched: {}.",
            matched_gates.join(", ")
        ));
    }

    let mut shared_name_tokens = record
        .name_tokens
        .iter()
        .filter(|token| query_tokens.contains(*token))
        .cloned()
        .collect::<Vec<_>>();
    shared_name_tokens.sort();
    if !shared_name_tokens.is_empty() {
        score += 14.0 + (shared_name_tokens.len() as f64) * 4.0;
        reasons.push(format!(
            "Name tokens matched: {}.",
            shared_name_tokens.join(", ")
        ));
    }

    let matched_trigger_hints = record
        .trigger_hints
        .iter()
        .filter(|phrase| {
            phrase.chars().count() >= 2
                && !common_route_stop_tokens().contains(&phrase.as_str())
                && text_matches_phrase(query_token_list, phrase)
        })
        .cloned()
        .collect::<Vec<_>>();
    if !matched_trigger_hints.is_empty() {
        score += (matched_trigger_hints.len() as f64) * 20.0;
        reasons.push(format!(
            "Trigger hint matched: {}.",
            matched_trigger_hints.join(", ")
        ));
    }

    let mut shared_keywords = record
        .keyword_tokens
        .iter()
        .filter(|token| query_tokens.contains(*token))
        .cloned()
        .collect::<Vec<_>>();
    shared_keywords.sort();
    if !shared_keywords.is_empty() {
        score += f64::min(24.0, (shared_keywords.len() as f64) * 3.0);
        reasons.push(format!(
            "Description keywords matched: {}.",
            shared_keywords
                .iter()
                .take(8)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    if first_turn && score > 0.0 {
        let session_start = normalize_text(&record.session_start);
        if session_start == "required" {
            score += 8.0;
            reasons.push("Session-start required boost applied (+8).".to_string());
        } else if session_start == "preferred" {
            score += 3.0;
            reasons.push("Session-start preferred boost applied (+3).".to_string());
        }
    }

    if normalize_text(&record.owner) == "gate" && score > 0.0 {
        score += 2.0;
    }

    if record.slug == "execution-controller-coding" {
        let controller_markers = [
            "高负载",
            "跨文件",
            "长运行",
            ".supervisor_state.json",
            "主线程",
            "系统指挥中心",
        ]
        .iter()
        .filter(|marker| query_text.contains(*marker))
        .cloned()
        .collect::<Vec<_>>();
        if !controller_markers.is_empty() {
            score += 24.0;
            reasons.push(format!(
                "Execution-controller boost applied: {}.",
                controller_markers.join(", ")
            ));
        }

        let gsd_markers = gsd_execution_markers()
            .iter()
            .filter(|marker| query_text.contains(*marker))
            .cloned()
            .collect::<Vec<_>>();
        if !gsd_markers.is_empty() {
            score += 26.0;
            reasons.push(format!(
                "GSD execution boost applied: {}.",
                gsd_markers.join(", ")
            ));
        }
    }

    if record.slug == "subagent-delegation" && score > 0.0 {
        let explicit_delegation = [
            "sidecar",
            "subagent",
            "delegation",
            "子代理",
            "并行 sidecar",
        ]
        .iter()
        .any(|marker| query_text.contains(*marker));
        let controller_markers = [
            "高负载",
            "跨文件",
            "长运行",
            ".supervisor_state.json",
            "主线程",
            "系统指挥中心",
        ]
        .iter()
        .any(|marker| query_text.contains(*marker));
        let gsd_posture = gsd_execution_markers()
            .iter()
            .any(|marker| query_text.contains(*marker));
        if controller_markers && !explicit_delegation {
            score *= 0.7;
            reasons.push(
                "Delegation-gate suppression applied: controller-orchestration signals dominate."
                    .to_string(),
            );
        }
        if gsd_posture && !explicit_delegation {
            score *= 0.45;
            reasons.push(
                "Delegation-gate suppression applied: gsd posture keeps the immediate blocker local."
                    .to_string(),
            );
        }
    }

    if record.slug == "visual-review" && score > 0.0 {
        let visual_evidence_markers = [
            "看图",
            "截图",
            "渲染",
            "render",
            "screenshot",
            "ui",
            "layout",
            "chart",
            "视觉",
        ];
        if !visual_evidence_markers
            .iter()
            .any(|marker| query_text.contains(marker))
        {
            return RouteCandidate {
                record: record.clone(),
                score: 0.0,
                reasons: vec![
                    "Suppressed: visual-review requires visible evidence, not a generic review token."
                        .to_string(),
                ],
            };
        }
    }

    if is_overlay_record(record) && score > 0.0 {
        score *= 0.15;
        reasons.push(format!(
            "Owner suppression applied: {} is overlay-only.",
            record.slug
        ));
    }

    RouteCandidate {
        record: record.clone(),
        score,
        reasons,
    }
}

fn fallback_owner(records: &[SkillRecord]) -> Result<&SkillRecord, String> {
    let primary_owners = records
        .iter()
        .filter(|record| can_be_primary_owner(record))
        .collect::<Vec<_>>();
    let pool = if primary_owners.is_empty() {
        records.iter().collect::<Vec<_>>()
    } else {
        primary_owners
    };
    pool.into_iter()
        .min_by(|left, right| {
            layer_rank(&left.layer)
                .cmp(&layer_rank(&right.layer))
                .then_with(|| priority_rank(&left.priority).cmp(&priority_rank(&right.priority)))
                .then_with(|| left.slug.cmp(&right.slug))
        })
        .ok_or_else(|| "No skill records available for fallback owner.".to_string())
}

fn pick_owner(candidates: Vec<RouteCandidate>) -> RouteCandidate {
    let mut gate_candidates = candidates
        .iter()
        .filter(|candidate| {
            normalize_text(&candidate.record.owner) == "gate"
                || normalize_text(&candidate.record.gate) != "none"
        })
        .cloned()
        .collect::<Vec<_>>();
    gate_candidates.sort_by(route_candidate_cmp);
    let mut owner_candidates = candidates
        .iter()
        .filter(|candidate| can_be_primary_owner(&candidate.record))
        .cloned()
        .collect::<Vec<_>>();
    owner_candidates.sort_by(route_candidate_cmp);
    let top_owner_score = owner_candidates
        .first()
        .map(|candidate| candidate.score)
        .unwrap_or(f64::NEG_INFINITY);
    if let Some(top_gate) = gate_candidates.first().cloned() {
        if top_gate.score >= 30.0 && top_gate.score >= top_owner_score {
            let mut selected = top_gate;
            selected
                .reasons
                .push("Prioritized via gate-before-owner precedence.".to_string());
            return selected;
        }
    }

    let owner_pool = if owner_candidates.is_empty() {
        candidates.clone()
    } else {
        owner_candidates.clone()
    };

    let mut layers = owner_pool
        .iter()
        .map(|candidate| candidate.record.layer.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    layers.sort_by_key(|layer| layer_rank(layer));

    for layer in layers {
        let mut layer_candidates = owner_pool
            .iter()
            .filter(|candidate| candidate.record.layer == layer)
            .cloned()
            .collect::<Vec<_>>();
        layer_candidates.sort_by(route_candidate_cmp);
        if let Some(top) = layer_candidates.first().cloned() {
            if top.score >= layer_threshold(&layer) {
                return top;
            }
        }
    }

    let mut fallback_pool = owner_pool;
    fallback_pool.sort_by(|left, right| {
        layer_rank(&left.record.layer)
            .cmp(&layer_rank(&right.record.layer))
            .then_with(|| {
                right
                    .score
                    .partial_cmp(&left.score)
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| {
                priority_rank(&left.record.priority).cmp(&priority_rank(&right.record.priority))
            })
            .then_with(|| left.record.slug.cmp(&right.record.slug))
    });
    fallback_pool.remove(0)
}

fn route_candidate_cmp(left: &RouteCandidate, right: &RouteCandidate) -> Ordering {
    right
        .score
        .partial_cmp(&left.score)
        .unwrap_or(Ordering::Equal)
        .then_with(|| {
            priority_rank(&left.record.priority).cmp(&priority_rank(&right.record.priority))
        })
        .then_with(|| left.record.slug.cmp(&right.record.slug))
}

fn pick_overlay(
    records: &[SkillRecord],
    query_tokens: &[String],
    selected_skill: &SkillRecord,
) -> Option<String> {
    let auto_anti_laziness = matches!(selected_skill.layer.as_str(), "L-1" | "L0" | "L1");
    let anti_laziness = records.iter().find(|record| record.slug == "anti-laziness");

    let mut ordered = records.to_vec();
    ordered.sort_by(|left, right| {
        layer_rank(&left.layer)
            .cmp(&layer_rank(&right.layer))
            .then_with(|| priority_rank(&left.priority).cmp(&priority_rank(&right.priority)))
            .then_with(|| left.slug.cmp(&right.slug))
    });

    for record in ordered {
        if record.slug == selected_skill.slug {
            continue;
        }
        if !is_overlay_record(&record) {
            continue;
        }
        let explicit_name_match = text_matches_phrase(query_tokens, &record.slug_lower);
        let explicit_trigger_match = record
            .trigger_hints
            .iter()
            .any(|phrase| phrase.chars().count() > 3 && text_matches_phrase(query_tokens, phrase));
        if explicit_name_match || explicit_trigger_match {
            return Some(record.slug.clone());
        }
    }

    if selected_skill.slug == "skill-developer-codex"
        && [
            "review",
            "framework-review",
            "routing-review",
            "审查",
            "审核",
        ]
        .iter()
        .any(|marker| text_matches_phrase(query_tokens, marker))
    {
        if let Some(skill) = records.iter().find(|record| record.slug == "code-review") {
            return Some(skill.slug.clone());
        }
    }

    if auto_anti_laziness {
        if let Some(skill) = anti_laziness {
            if skill.slug != selected_skill.slug {
                return Some(skill.slug.clone());
            }
        }
    }
    None
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn score_bucket(score: f64) -> String {
    let floor = ((score.max(0.0) / 10.0).floor() as i32) * 10;
    format!("{floor:02}-{ceiling:02}", ceiling = floor + 9)
}

fn reasons_class(reasons: &[String]) -> String {
    let mut normalized = reasons
        .iter()
        .map(|reason| normalize_text(reason))
        .filter(|reason| !reason.is_empty())
        .collect::<Vec<_>>();
    if normalized.is_empty() {
        return "none".to_string();
    }
    normalized.sort();
    normalized.dedup();
    normalized.join("|")
}

fn layer_rank(layer: &str) -> i32 {
    match layer {
        "L-1" => -1,
        "L0" => 0,
        "L1" => 1,
        "L2" => 2,
        "L3" => 3,
        "L4" => 4,
        _ => 99,
    }
}

fn priority_rank(priority: &str) -> i32 {
    match priority {
        "P0" => 0,
        "P1" => 1,
        "P2" => 2,
        "P3" => 3,
        _ => 99,
    }
}

fn layer_threshold(layer: &str) -> f64 {
    match layer {
        "L0" => 18.0,
        "L1" => 16.0,
        "L2" | "L3" => 14.0,
        _ => 15.0,
    }
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

fn trace_scope_fields<'a>(payload: &'a Option<Vec<String>>) -> Option<&'a [String]> {
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
    session_id: Option<&str>,
    job_id: Option<&str>,
    stream_scope_fields: &Option<Vec<String>>,
) -> Result<Vec<Map<String, Value>>, String> {
    let mut events = Vec::new();
    let storage_backend = resolve_storage_backend(&[path.to_path_buf()]);
    let raw_payload = storage_read_text(path, storage_backend.as_ref())?;

    for (line_number, raw_line) in raw_payload.lines().enumerate() {
        if raw_line.trim().is_empty() {
            continue;
        }
        let event_payload = hydrate_trace_event_object(
            trace_event_object(serde_json::from_str::<Value>(&raw_line).map_err(|err| {
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

fn load_compaction_recovery(
    manifest_path: &Path,
    session_id: Option<&str>,
    job_id: Option<&str>,
    stream_scope_fields: &Option<Vec<String>>,
) -> Result<ResolvedTraceSource, String> {
    let storage_backend = resolve_storage_backend(&[manifest_path.to_path_buf()]);
    let manifest_payload =
        serde_json::from_str::<Value>(&storage_read_text(manifest_path, storage_backend.as_ref())?)
            .map_err(|err| {
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
    let state_ref_uri = snapshot
        .get("state_ref")
        .and_then(Value::as_object)
        .and_then(|payload| payload.get("uri"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "compaction manifest is missing required recovery artifact refs.".to_string()
        })?;
    let artifact_index_uri = snapshot
        .get("artifact_index_ref")
        .and_then(Value::as_object)
        .and_then(|payload| payload.get("uri"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "compaction manifest is missing required recovery artifact refs.".to_string()
        })?;
    let state_path = PathBuf::from(state_ref_uri);
    let artifact_index_path = PathBuf::from(artifact_index_uri);
    if !storage_artifact_exists(&state_path, storage_backend.as_ref())
        || !storage_artifact_exists(&artifact_index_path, storage_backend.as_ref())
    {
        return Err(
            "Compaction recovery failed closed because a referenced artifact is missing."
                .to_string(),
        );
    }
    let state_payload =
        serde_json::from_str::<Value>(&storage_read_text(&state_path, storage_backend.as_ref())?)
            .map_err(|err| {
            format!(
                "parse compaction state failed for {}: {err}",
                state_path.display()
            )
        })?;
    let artifact_index_payload = serde_json::from_str::<Value>(&storage_read_text(
        &artifact_index_path,
        storage_backend.as_ref(),
    )?)
    .map_err(|err| {
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
        if storage_artifact_exists(delta_path, storage_backend.as_ref()) {
            let raw_delta_payload = storage_read_text(delta_path, storage_backend.as_ref())?;
            for (line_number, raw_line) in raw_delta_payload.lines().enumerate() {
                if raw_line.trim().is_empty() {
                    continue;
                }
                let delta_payload = serde_json::from_str::<Value>(&raw_line).map_err(|err| {
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

fn resolve_trace_source(
    path: Option<&str>,
    compaction_manifest_path: Option<&str>,
    session_id: Option<&str>,
    job_id: Option<&str>,
    stream_scope_fields: &Option<Vec<String>>,
) -> Result<ResolvedTraceSource, String> {
    if let Some(compaction_path) = compaction_manifest_path {
        return load_compaction_recovery(
            &PathBuf::from(compaction_path),
            session_id,
            job_id,
            stream_scope_fields,
        );
    }
    let path = path.ok_or_else(|| {
        "trace stream replay requires path or compaction_manifest_path".to_string()
    })?;
    let path_buf = PathBuf::from(path);
    let events = load_trace_stream_events(&path_buf, session_id, job_id, stream_scope_fields)?;
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

fn inspect_trace_stream(
    payload: TraceStreamInspectRequestPayload,
) -> Result<TraceStreamInspectResponsePayload, String> {
    let resolved = resolve_trace_source(
        payload.path.as_deref(),
        payload.compaction_manifest_path.as_deref(),
        payload.session_id.as_deref(),
        payload.job_id.as_deref(),
        &payload.stream_scope_fields,
    )?;

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
    })
}

fn replay_trace_stream(
    payload: TraceStreamReplayRequestPayload,
) -> Result<TraceStreamReplayResponsePayload, String> {
    let resolved = resolve_trace_source(
        payload.path.as_deref(),
        payload.compaction_manifest_path.as_deref(),
        payload.session_id.as_deref(),
        payload.job_id.as_deref(),
        &payload.stream_scope_fields,
    )?;
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

fn write_trace_compaction_delta(
    payload: TraceCompactionDeltaWriteRequestPayload,
) -> Result<TraceCompactionDeltaWriteResponsePayload, String> {
    let path = PathBuf::from(&payload.path);
    let serialized = serde_json::to_string(&payload.delta)
        .map_err(|err| format!("serialize trace compaction delta failed: {err}"))?
        + "\n";
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "create trace compaction delta parent failed for {}: {err}",
                parent.display()
            )
        })?;
    }
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|err| {
            format!(
                "open trace compaction delta failed for {}: {err}",
                path.display()
            )
        })?;
    file.write_all(serialized.as_bytes()).map_err(|err| {
        format!(
            "write trace compaction delta failed for {}: {err}",
            path.display()
        )
    })?;
    Ok(TraceCompactionDeltaWriteResponsePayload {
        schema_version: TRACE_COMPACTION_DELTA_WRITE_SCHEMA_VERSION.to_string(),
        authority: TRACE_STREAM_IO_AUTHORITY.to_string(),
        path: path.display().to_string(),
        bytes_written: serialized.as_bytes().len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/routing_route_fixtures.json")
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
    fn route_decision_fixture_expectations_hold() {
        let fixture = fixture_path();
        let records = load_records_from_manifest(&fixture).expect("load fixture records");
        let payload = read_json(&fixture).expect("read fixture");
        let cases = payload
            .get("cases")
            .and_then(Value::as_array)
            .expect("cases array");

        for case in cases {
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
                    .expect("selected_skill")
            );
            assert_eq!(
                decision.overlay_skill,
                expected
                    .get("overlay_skill")
                    .and_then(Value::as_str)
                    .map(|value| value.to_string())
            );
            assert_eq!(
                decision.layer,
                expected
                    .get("layer")
                    .and_then(Value::as_str)
                    .expect("expected layer")
            );
            assert_eq!(
                decision.route_snapshot.selected_skill,
                decision.selected_skill
            );
            assert_eq!(
                decision.route_snapshot.overlay_skill,
                decision.overlay_skill
            );
            assert_eq!(decision.route_snapshot.layer, decision.layer);
        }
    }

    #[test]
    fn route_diff_report_matches_shadow_compare_contract() {
        let rust_snapshot = build_route_snapshot(
            "rust",
            "plan-to-code",
            Some("anti-laziness"),
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
        assert_eq!(payload["python_authority_default"], Value::Bool(false));
        assert_eq!(
            payload["rustification_status"]["runtime_primary_owner"],
            Value::String("rust-control-plane".to_string())
        );
        assert_eq!(
            payload["rustification_status"]["python_runtime_role"],
            Value::String("compatibility-host".to_string())
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
            payload["services"]["execution"]["kernel_contract"]
                ["execution_kernel_live_fallback_enabled"],
            Value::Bool(false)
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
            payload["services"]["execution"]["kernel_live_delegate_authority"],
            Value::String("rust-execution-cli".to_string())
        );
        assert_eq!(
            payload["services"]["execution"]["kernel_live_fallback_enabled"],
            Value::Bool(false)
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
            payload["services"]["checkpoint"]["delegate_kind"],
            Value::String("filesystem-checkpointer".to_string())
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
    fn sandbox_control_accepts_known_edges_and_rejects_invalid_edges() {
        let accepted = build_sandbox_control_response(SandboxControlRequestPayload {
            schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
            operation: "transition".to_string(),
            current_state: Some("warm".to_string()),
            next_state: Some("busy".to_string()),
            cleanup_failed: None,
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
            cleanup_failed: None,
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
            next_state: None,
            cleanup_failed: Some(false),
        })
        .expect("cleanup recycled response");
        assert!(recycled.allowed);
        assert_eq!(recycled.reason, "cleanup-completed");
        assert_eq!(recycled.resolved_state.as_deref(), Some("recycled"));

        let failed = build_sandbox_control_response(SandboxControlRequestPayload {
            schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
            operation: "cleanup".to_string(),
            current_state: Some("draining".to_string()),
            next_state: None,
            cleanup_failed: Some(true),
        })
        .expect("cleanup failed response");
        assert!(failed.allowed);
        assert_eq!(failed.reason, "cleanup-failed");
        assert_eq!(failed.resolved_state.as_deref(), Some("failed"));
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
                "python",
                "plan-to-code",
                Some("anti-laziness"),
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
        assert_eq!(snapshot.route_snapshot.engine, "python");
        assert_eq!(snapshot.route_snapshot.selected_skill, "plan-to-code");
        assert_eq!(
            snapshot.route_snapshot.overlay_skill.as_deref(),
            Some("anti-laziness")
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
        assert_eq!(
            response.metadata["execution_kernel"],
            EXECUTION_KERNEL_BRIDGE_KIND
        );
        assert_eq!(
            response.metadata["execution_kernel_metadata_schema_version"],
            EXECUTION_METADATA_SCHEMA_VERSION
        );
        assert_eq!(
            response.metadata["execution_kernel_authority"],
            EXECUTION_KERNEL_BRIDGE_AUTHORITY
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
        assert!(prompt.contains("Be brief, clear, and friendly"));
        assert!(prompt.contains("Trigger phrase matched: 直接做代码."));
    }

    #[test]
    fn live_execute_prompt_builder_adds_idea_to_plan_contract() {
        let mut payload = sample_execute_request();
        payload.dry_run = false;
        payload.prompt_preview = None;
        payload.selected_skill = "idea-to-plan".to_string();
        payload.overlay_skill = Some("anti-laziness".to_string());
        payload.layer = "L-1".to_string();
        payload.reasons = vec!["Trigger hint matched: 先探索现状再提方案.".to_string()];

        let prompt = build_live_execute_prompt(&payload);

        assert!(prompt.contains("Planning output:"));
        assert!(prompt.contains("outline.md"));
        assert!(prompt.contains("decision_log.md"));
        assert!(prompt.contains("code_list.md"));
        assert!(prompt.contains("checklist-writting"));
        assert!(!prompt.contains("READ-ONLY planning route"));
        assert!(!prompt.contains("<proposed_plan>"));
    }

    #[test]
    fn live_execute_ignores_caller_supplied_prompt_preview() {
        let mut payload = sample_execute_request();
        payload.dry_run = false;
        payload.prompt_preview = Some("Python supplied live prompt".to_string());

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
            Some("Python supplied live prompt")
        );
        assert_eq!(
            response.metadata["execution_kernel"],
            EXECUTION_KERNEL_BRIDGE_KIND
        );
        assert_eq!(
            response.metadata["execution_kernel_authority"],
            EXECUTION_KERNEL_BRIDGE_AUTHORITY
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
            response.metadata["execution_kernel_live_fallback"],
            Value::Null
        );
        assert_eq!(
            response.metadata["execution_kernel_live_fallback_authority"],
            Value::Null
        );
        assert_eq!(
            response.metadata["execution_kernel_live_fallback_enabled"],
            Value::Bool(false)
        );
        assert_eq!(
            response.metadata["execution_kernel_live_fallback_mode"],
            "disabled"
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
            compaction_manifest_path: None,
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
            compaction_manifest_path: None,
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
            compaction_manifest_path: None,
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
        fs::write(
            &state_path,
            serde_json::to_string_pretty(&json!({
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
            .expect("serialize state"),
        )
        .expect("write state");
        fs::write(
            &artifact_index_path,
            serde_json::to_string_pretty(&json!([
                {
                    "schema_version": "runtime-trace-artifact-ref-v1",
                    "artifact_id": "art-state",
                    "kind": "state_ref",
                    "uri": state_path.display().to_string(),
                    "digest": "abc",
                    "size_bytes": 10
                }
            ]))
            .expect("serialize artifact index"),
        )
        .expect("write artifact index");
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
                        "digest": "artifact-digest",
                        "size_bytes": 10
                    },
                    "state_ref": {
                        "schema_version": "runtime-trace-artifact-ref-v1",
                        "artifact_id": "art-state",
                        "kind": "state_ref",
                        "uri": state_path.display().to_string(),
                        "digest": "state-digest",
                        "size_bytes": 10
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
            compaction_manifest_path: Some(manifest_path.display().to_string()),
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
            compaction_manifest_path: Some(manifest_path.display().to_string()),
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
}
