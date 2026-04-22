use clap::{ArgAction, Parser};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;
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
const TRACE_STREAM_REPLAY_SCHEMA_VERSION: &str = "router-rs-trace-stream-replay-v1";
const TRACE_STREAM_INSPECT_SCHEMA_VERSION: &str = "router-rs-trace-stream-inspect-v1";
const TRACE_COMPACTION_DELTA_WRITE_SCHEMA_VERSION: &str =
    "router-rs-trace-compaction-delta-write-v1";
const TRACE_STREAM_IO_AUTHORITY: &str = "rust-runtime-trace-io";
const RUNTIME_OBSERVABILITY_EXPORTER_SCHEMA_VERSION: &str = "runtime-observability-exporter-v1";
const RUNTIME_OBSERVABILITY_METRIC_RECORD_SCHEMA_VERSION: &str =
    "runtime-observability-metric-record-v1";
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
    runtime_observability_exporter_json: bool,
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
    route_snapshot_input_json: Option<String>,
    #[arg(long)]
    execute_input_json: Option<String>,
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
    route_snapshot: RouteDecisionSnapshotPayload,
}

#[derive(Debug, Serialize)]
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
        args.route_json,
        args.route_policy_json,
        args.route_snapshot_json,
        args.execute_json,
        args.runtime_control_plane_json,
        args.background_control_json,
        args.describe_transport_json,
        args.describe_handoff_json,
        args.checkpoint_resume_manifest_json,
        args.write_transport_binding_json,
        args.write_checkpoint_resume_manifest_json,
        args.runtime_observability_exporter_json,
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
            "choose only one output mode among --json, --route-json, --route-policy-json, --route-snapshot-json, --execute-json, --runtime-control-plane-json, --background-control-json, --describe-transport-json, --describe-handoff-json, --checkpoint-resume-manifest-json, --write-transport-binding-json, --write-checkpoint-resume-manifest-json, --runtime-observability-exporter-json, --runtime-observability-dashboard-json, --runtime-metric-record-json, --trace-stream-replay-json, --trace-stream-inspect-json, --write-trace-compaction-delta-json, --framework-runtime-snapshot-json, --framework-contract-summary-json, --route-report-json, --profile-json, and --profile-artifacts-json"
                .to_string(),
        );
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
        let rust_snapshot = serde_json::from_str::<RouteDecisionSnapshotPayload>(
            args.rust_route_snapshot_json.as_deref().ok_or_else(|| {
                "--rust-route-snapshot-json is required with --route-report-json".to_string()
            })?,
        )
        .map_err(|err| format!("parse rust route snapshot failed: {err}"))?;
        let report = build_route_diff_report(mode, rust_snapshot)?;
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

    Ok(RouteDiffReportPayload {
        report_schema_version: ROUTE_REPORT_SCHEMA_VERSION.to_string(),
        authority: ROUTE_AUTHORITY.to_string(),
        mode: normalized_mode,
        primary_engine: "rust".to_string(),
        evidence_kind: "rust-owned-snapshot".to_string(),
        strict_verification,
        verification_passed: true,
        route_snapshot: rust_snapshot,
    })
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

fn build_background_control_response(
    payload: BackgroundControlRequestPayload,
) -> Result<BackgroundControlResponsePayload, String> {
    let supported_multitask_strategies = vec!["interrupt".to_string(), "reject".to_string()];
    match payload.operation.as_str() {
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
        "complete" => {
            let mut effect_plan = background_effect_plan("finalize_completed");
            effect_plan.finalize_immediately = Some(true);
            effect_plan.terminal_status = Some("completed".to_string());
            effect_plan.resolved_status = Some("completed".to_string());
            Ok(BackgroundControlResponsePayload {
                schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
                authority: BACKGROUND_CONTROL_AUTHORITY.to_string(),
                operation: payload.operation,
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

fn build_runtime_control_plane_payload() -> Value {
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
        "services": {
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
            },
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
            if trace_event_string_field(payload, "session_id").as_deref() != Some(expected_session_id) {
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

fn trace_scope_fields<'a>(
    payload: &'a Option<Vec<String>>,
) -> Option<&'a [String]> {
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
    let file = fs::File::open(path)
        .map_err(|err| format!("open trace stream failed for {}: {err}", path.display()))?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();

    for (line_number, line) in reader.lines().enumerate() {
        let raw_line = line.map_err(|err| {
            format!(
                "read trace stream failed at line {}: {err}",
                line_number + 1
            )
        })?;
        if raw_line.trim().is_empty() {
            continue;
        }
        let event_payload =
            hydrate_trace_event_object(
                trace_event_object(serde_json::from_str::<Value>(&raw_line).map_err(|err| {
                    format!("parse trace stream line {} failed: {err}", line_number + 1)
                })?)?,
                line_number + 1,
            );
        if trace_event_matches_request_scope(&event_payload, session_id, job_id, stream_scope_fields) {
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
        .ok_or_else(|| format!("trace compaction delta line {line_number} missing applies_to.session_id"))?;
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
    let manifest_payload = serde_json::from_str::<Value>(
        &fs::read_to_string(manifest_path)
            .map_err(|err| format!("open compaction manifest failed for {}: {err}", manifest_path.display()))?,
    )
    .map_err(|err| format!("parse compaction manifest failed for {}: {err}", manifest_path.display()))?;
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
        .ok_or_else(|| "compaction manifest is missing required recovery artifact refs.".to_string())?;
    let artifact_index_uri = snapshot
        .get("artifact_index_ref")
        .and_then(Value::as_object)
        .and_then(|payload| payload.get("uri"))
        .and_then(Value::as_str)
        .ok_or_else(|| "compaction manifest is missing required recovery artifact refs.".to_string())?;
    let state_path = PathBuf::from(state_ref_uri);
    let artifact_index_path = PathBuf::from(artifact_index_uri);
    if !state_path.exists() || !artifact_index_path.exists() {
        return Err(
            "Compaction recovery failed closed because a referenced artifact is missing.".to_string()
        );
    }
    let state_payload = serde_json::from_str::<Value>(
        &fs::read_to_string(&state_path)
            .map_err(|err| format!("open compaction state failed for {}: {err}", state_path.display()))?,
    )
    .map_err(|err| format!("parse compaction state failed for {}: {err}", state_path.display()))?;
    let artifact_index_payload = serde_json::from_str::<Value>(
        &fs::read_to_string(&artifact_index_path).map_err(|err| {
            format!(
                "open compaction artifact index failed for {}: {err}",
                artifact_index_path.display()
            )
        })?,
    )
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
        if delta_path.exists() {
            let file = fs::File::open(delta_path)
                .map_err(|err| format!("open compaction delta stream failed for {}: {err}", delta_path.display()))?;
            let reader = BufReader::new(file);
            for (line_number, line) in reader.lines().enumerate() {
                let raw_line = line.map_err(|err| {
                    format!(
                        "read compaction delta stream failed at line {}: {err}",
                        line_number + 1
                    )
                })?;
                if raw_line.trim().is_empty() {
                    continue;
                }
                let delta_payload = serde_json::from_str::<Value>(&raw_line).map_err(|err| {
                    format!("parse compaction delta line {} failed: {err}", line_number + 1)
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
        latest_event_id: latest_event.and_then(|payload| trace_event_string_field(payload, "event_id")),
        latest_event_kind: latest_event.and_then(|payload| trace_event_string_field(payload, "kind")),
        latest_event_timestamp: latest_event.and_then(|payload| trace_event_string_field(payload, "ts")),
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
        .map_err(|err| format!("open trace compaction delta failed for {}: {err}", path.display()))?;
    file.write_all(serialized.as_bytes())
        .map_err(|err| format!("write trace compaction delta failed for {}: {err}", path.display()))?;
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

        let report = build_route_diff_report("shadow", rust_snapshot).expect("shadow report");

        assert_eq!(report.report_schema_version, ROUTE_REPORT_SCHEMA_VERSION);
        assert_eq!(report.authority, ROUTE_AUTHORITY);
        assert_eq!(report.mode, "shadow");
        assert_eq!(report.primary_engine, "rust");
        assert_eq!(report.evidence_kind, "rust-owned-snapshot");
        assert!(!report.strict_verification);
        assert!(report.verification_passed);
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
            payload["services"]["execution"]["delegate_kind"],
            Value::String("rust-execution-kernel-slice".to_string())
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
        })
        .expect("capacity response");
        assert!(capacity.strategy_supported);
        assert_eq!(capacity.accepted, Some(false));
        assert_eq!(capacity.requires_takeover, Some(true));
        assert_eq!(capacity.reason, "capacity-rejected");
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
        assert_eq!(replay.events[0]["event_id"], Value::String("evt-1".to_string()));
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
        assert_eq!(replay.events[0]["event_id"], Value::String("evt-2".to_string()));

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
}
