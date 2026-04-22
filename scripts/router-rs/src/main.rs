use clap::Parser;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;
use strsim::jaro_winkler;

mod framework_profile;
mod framework_runtime;

use framework_runtime::{
    build_framework_contract_summary_envelope, build_framework_runtime_snapshot_envelope,
    resolve_repo_root_arg,
};
use framework_profile::{
    build_codex_artifact_bundle, build_profile_bundle, build_profile_bundle_with_legacy_alias,
    load_framework_profile,
};

const ROUTE_DECISION_SCHEMA_VERSION: &str = "router-rs-route-decision-v1";
const ROUTE_POLICY_SCHEMA_VERSION: &str = "router-rs-route-policy-v1";
const ROUTE_SNAPSHOT_SCHEMA_VERSION: &str = "router-rs-route-snapshot-v1";
const ROUTE_REPORT_SCHEMA_VERSION: &str = "router-rs-route-report-v1";
const ROUTE_AUTHORITY: &str = "rust-route-core";
const PROFILE_COMPILE_AUTHORITY: &str = "rust-route-compiler";
const EXECUTION_SCHEMA_VERSION: &str = "router-rs-execute-response-v1";
const EXECUTION_AUTHORITY: &str = "rust-execution-cli";
const EXECUTION_KERNEL_CONTRACT_MODE: &str = "rust-live-primary";
const EXECUTION_KERNEL_FALLBACK_POLICY: &str = "infrastructure-only-explicit";
const EXECUTION_KERNEL_DELEGATE_FAMILY: &str = "rust-cli";
const EXECUTION_KERNEL_DELEGATE_IMPL: &str = "router-rs";
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
    #[arg(long, default_value_t = false)]
    rollback_active: bool,
    #[arg(long)]
    python_route_snapshot_json: Option<String>,
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
    #[arg(long, default_value = "route-cli")]
    session_id: String,
    #[arg(long, default_value_t = true)]
    allow_overlay: bool,
    #[arg(long, default_value_t = true)]
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
    shadow_engine: Option<String>,
    mismatch: bool,
    mismatch_fields: Vec<String>,
    selected_skill_match: bool,
    overlay_skill_match: bool,
    layer_match: bool,
    score_bucket_match: bool,
    reasons_class_match: bool,
    rollback_active: bool,
    python: RouteDecisionSnapshotPayload,
    rust: RouteDecisionSnapshotPayload,
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
    rollback_active: bool,
    python_route_required: bool,
    diagnostic_python_lane: bool,
    primary_authority: String,
    route_result_engine: String,
    shadow_engine: Option<String>,
    diff_report_required: bool,
    verify_parity_required: bool,
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
    #[serde(alias = "rollback_to_python")]
    diagnostic_python_lane_active: Option<bool>,
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
            "choose only one output mode among --json, --route-json, --route-policy-json, --route-snapshot-json, --execute-json, --runtime-control-plane-json, --background-control-json, --describe-transport-json, --describe-handoff-json, --checkpoint-resume-manifest-json, --write-transport-binding-json, --write-checkpoint-resume-manifest-json, --runtime-observability-exporter-json, --runtime-observability-dashboard-json, --runtime-metric-record-json, --framework-runtime-snapshot-json, --framework-contract-summary-json, --route-report-json, --profile-json, and --profile-artifacts-json"
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
        let python_snapshot = serde_json::from_str::<RouteDecisionSnapshotPayload>(
            args.python_route_snapshot_json.as_deref().ok_or_else(|| {
                "--python-route-snapshot-json is required with --route-report-json".to_string()
            })?,
        )
        .map_err(|err| format!("parse python route snapshot failed: {err}"))?;
        let rust_snapshot = serde_json::from_str::<RouteDecisionSnapshotPayload>(
            args.rust_route_snapshot_json.as_deref().ok_or_else(|| {
                "--rust-route-snapshot-json is required with --route-report-json".to_string()
            })?,
        )
        .map_err(|err| format!("parse rust route snapshot failed: {err}"))?;
        let report =
            build_route_diff_report(mode, python_snapshot, rust_snapshot, args.rollback_active);
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
        let policy = build_route_policy(mode, args.rollback_active)?;
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
    python_snapshot: RouteDecisionSnapshotPayload,
    rust_snapshot: RouteDecisionSnapshotPayload,
    rollback_active: bool,
) -> RouteDiffReportPayload {
    let mut mismatch_fields = Vec::new();
    let selected_skill_match = python_snapshot.selected_skill == rust_snapshot.selected_skill;
    let overlay_skill_match = python_snapshot.overlay_skill == rust_snapshot.overlay_skill;
    let layer_match = python_snapshot.layer == rust_snapshot.layer;
    let score_bucket_match = python_snapshot.score_bucket == rust_snapshot.score_bucket;
    let reasons_class_match = python_snapshot.reasons_class == rust_snapshot.reasons_class;
    if !selected_skill_match {
        mismatch_fields.push("selected_skill".to_string());
    }
    if !overlay_skill_match {
        mismatch_fields.push("overlay_skill".to_string());
    }
    if !layer_match {
        mismatch_fields.push("layer".to_string());
    }
    if !score_bucket_match {
        mismatch_fields.push("score_bucket".to_string());
    }
    if !reasons_class_match {
        mismatch_fields.push("reasons_class".to_string());
    }

    RouteDiffReportPayload {
        report_schema_version: ROUTE_REPORT_SCHEMA_VERSION.to_string(),
        authority: ROUTE_AUTHORITY.to_string(),
        mode: mode.to_string(),
        primary_engine: if mode == "python" {
            "python".to_string()
        } else {
            "rust".to_string()
        },
        shadow_engine: Some(if mode == "python" {
            "rust".to_string()
        } else {
            "python".to_string()
        }),
        mismatch: !mismatch_fields.is_empty(),
        mismatch_fields,
        selected_skill_match,
        overlay_skill_match,
        layer_match,
        score_bucket_match,
        reasons_class_match,
        rollback_active,
        python: python_snapshot,
        rust: rust_snapshot,
    }
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
        "You are the Codex runtime executing through the Rust kernel slice.".to_string(),
        format!("Active owner skill: {}", payload.selected_skill),
        format!("Routing layer: {}", payload.layer),
    ];
    if let Some(overlay) = payload
        .overlay_skill
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        lines.push(format!("Active overlay skill: {overlay}"));
    }
    if let Some(route_engine) = payload
        .route_engine
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        lines.push(format!("Route engine: {route_engine}"));
    }
    if let Some(diagnostic_python_lane_active) = payload.diagnostic_python_lane_active {
        lines.push(format!(
            "Diagnostic python lane active: {diagnostic_python_lane_active}"
        ));
    }
    if !payload.reasons.is_empty() {
        lines.push("Routing reasons:".to_string());
        for reason in &payload.reasons {
            let reason = reason.trim();
            if !reason.is_empty() {
                lines.push(format!("- {reason}"));
            }
        }
    }
    if payload.selected_skill == "idea-to-plan" {
        lines.push("Repo-local planning delta: converge the strategy into outline.md, decision_log.md, assumptions.md, open_questions.md, plan_rubric.md, and code_list.md.".to_string());
        lines.push("Use checklist-writting instead when the route is already fixed and only execution decomposition remains.".to_string());
    }
    lines.push(
        "Respond as the selected skill and keep the execution aligned with the routed contract."
            .to_string(),
    );
    lines.join("\n")
}

fn build_steady_state_execution_kernel_metadata() -> Map<String, Value> {
    let mut metadata = Map::new();
    metadata.insert(
        "execution_kernel".to_string(),
        Value::String(EXECUTION_KERNEL_DELEGATE_IMPL.to_string()),
    );
    metadata.insert(
        "execution_kernel_authority".to_string(),
        Value::String(EXECUTION_AUTHORITY.to_string()),
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
    let mut metadata = build_steady_state_execution_kernel_metadata();
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
        "diagnostic_python_lane_active".to_string(),
        json!(payload.diagnostic_python_lane_active),
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
    let mut metadata = build_steady_state_execution_kernel_metadata();
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
        "diagnostic_python_lane_active".to_string(),
        json!(payload.diagnostic_python_lane_active),
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
                "projection": "python-thin-projection",
                "delegate_kind": "rust-route-adapter",
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

fn build_route_policy(
    mode: &str,
    rollback_requested: bool,
) -> Result<RouteExecutionPolicyPayload, String> {
    let normalized_mode = mode.trim().to_ascii_lowercase();
    let rollback_active = rollback_requested && normalized_mode == "rust";
    let base = RouteExecutionPolicyPayload {
        policy_schema_version: ROUTE_POLICY_SCHEMA_VERSION.to_string(),
        authority: ROUTE_AUTHORITY.to_string(),
        mode: normalized_mode.clone(),
        rollback_active,
        python_route_required: false,
        diagnostic_python_lane: false,
        primary_authority: "rust".to_string(),
        route_result_engine: "rust".to_string(),
        shadow_engine: None,
        diff_report_required: false,
        verify_parity_required: false,
    };
    let policy = match normalized_mode.as_str() {
        "python" => RouteExecutionPolicyPayload {
            python_route_required: true,
            primary_authority: "python".to_string(),
            route_result_engine: "python".to_string(),
            ..base
        },
        "shadow" => RouteExecutionPolicyPayload {
            diagnostic_python_lane: true,
            shadow_engine: Some("python".to_string()),
            diff_report_required: true,
            ..base
        },
        "verify" => RouteExecutionPolicyPayload {
            diagnostic_python_lane: true,
            shadow_engine: Some("python".to_string()),
            diff_report_required: true,
            verify_parity_required: true,
            ..base
        },
        "rust" if rollback_active => RouteExecutionPolicyPayload {
            diagnostic_python_lane: true,
            shadow_engine: Some("python".to_string()),
            diff_report_required: true,
            ..base
        },
        "rust" => base,
        _ => {
            return Err(format!(
                "unsupported route mode for --route-policy-json: {mode}"
            ))
        }
    };
    if (policy.diff_report_required || policy.verify_parity_required)
        && !policy.diagnostic_python_lane
    {
        return Err(
            "route policy declared diff/verify requirements outside the diagnostic lane"
                .to_string(),
        );
    }
    if policy.verify_parity_required && !policy.diff_report_required {
        return Err("route policy declared verify requirements without diff reporting".to_string());
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

#[cfg(test)]
mod tests {
    use super::*;

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
            diagnostic_python_lane_active: Some(false),
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
        let python_snapshot = build_route_snapshot(
            "python",
            "plan-to-code",
            Some("anti-laziness"),
            "L2",
            37.0,
            &["Trigger phrase matched: 直接做代码.".to_string()],
        );
        let rust_snapshot = build_route_snapshot(
            "rust",
            "plan-to-code",
            Some("anti-laziness"),
            "L2",
            39.0,
            &["Trigger phrase matched: 直接做代码.".to_string()],
        );

        let report = build_route_diff_report("shadow", python_snapshot, rust_snapshot, false);

        assert_eq!(report.report_schema_version, ROUTE_REPORT_SCHEMA_VERSION);
        assert_eq!(report.authority, ROUTE_AUTHORITY);
        assert_eq!(report.mode, "shadow");
        assert_eq!(report.primary_engine, "rust");
        assert_eq!(report.shadow_engine.as_deref(), Some("python"));
        assert!(report.selected_skill_match);
        assert!(report.overlay_skill_match);
        assert!(report.layer_match);
        assert!(report.score_bucket_match);
        assert!(report.reasons_class_match);
        assert!(!report.mismatch);
        assert!(report.mismatch_fields.is_empty());
    }

    #[test]
    fn route_policy_matches_mode_matrix() {
        let python = build_route_policy("python", false).expect("python policy");
        assert!(python.python_route_required);
        assert!(!python.diagnostic_python_lane);
        assert_eq!(python.primary_authority, "python");
        assert_eq!(python.route_result_engine, "python");
        assert!(python.shadow_engine.is_none());
        assert!(!python.diff_report_required);
        assert!(!python.verify_parity_required);

        let shadow = build_route_policy("shadow", false).expect("shadow policy");
        assert!(!shadow.python_route_required);
        assert!(shadow.diagnostic_python_lane);
        assert_eq!(shadow.primary_authority, "rust");
        assert_eq!(shadow.route_result_engine, "rust");
        assert_eq!(shadow.shadow_engine.as_deref(), Some("python"));
        assert!(shadow.diff_report_required);
        assert!(!shadow.verify_parity_required);

        let verify = build_route_policy("verify", false).expect("verify policy");
        assert!(!verify.python_route_required);
        assert!(verify.diagnostic_python_lane);
        assert_eq!(verify.primary_authority, "rust");
        assert_eq!(verify.route_result_engine, "rust");
        assert_eq!(verify.shadow_engine.as_deref(), Some("python"));
        assert!(verify.diff_report_required);
        assert!(verify.verify_parity_required);

        let rust = build_route_policy("rust", false).expect("rust policy");
        assert!(!rust.python_route_required);
        assert!(!rust.diagnostic_python_lane);
        assert_eq!(rust.primary_authority, "rust");
        assert_eq!(rust.route_result_engine, "rust");
        assert!(rust.shadow_engine.is_none());
        assert!(!rust.diff_report_required);
        assert!(!rust.rollback_active);

        let rollback = build_route_policy("rust", true).expect("rollback policy");
        assert!(!rollback.python_route_required);
        assert!(rollback.diagnostic_python_lane);
        assert_eq!(rollback.primary_authority, "rust");
        assert_eq!(rollback.route_result_engine, "rust");
        assert_eq!(rollback.shadow_engine.as_deref(), Some("python"));
        assert!(rollback.diff_report_required);
        assert!(rollback.rollback_active);
        assert!(!rollback.verify_parity_required);
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
        assert_eq!(response.metadata["execution_kernel"], "router-rs");
        assert_eq!(
            response.metadata["execution_kernel_authority"],
            EXECUTION_AUTHORITY
        );
        assert_eq!(
            response.metadata["diagnostic_python_lane_active"],
            Value::Bool(false)
        );
    }

    #[test]
    fn live_execute_prompt_builder_produces_rust_owned_contract_prompt() {
        let mut payload = sample_execute_request();
        payload.dry_run = false;
        payload.prompt_preview = None;

        let prompt = build_live_execute_prompt(&payload);

        assert!(prompt.contains("Rust kernel slice"));
        assert!(prompt.contains("Active owner skill: plan-to-code"));
        assert!(prompt.contains("Active overlay skill: rust-pro"));
        assert!(prompt.contains("Routing layer: L2"));
        assert!(prompt.contains("Route engine: rust"));
        assert!(prompt.contains("Diagnostic python lane active: false"));
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

        assert!(prompt.contains("Repo-local planning delta"));
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
        assert_eq!(response.metadata["execution_kernel"], "router-rs");
        assert_eq!(
            response.metadata["execution_kernel_authority"],
            EXECUTION_AUTHORITY
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
}
