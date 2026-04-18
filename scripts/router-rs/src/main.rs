use clap::Parser;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;
use strsim::jaro_winkler;

mod framework_profile;

use framework_profile::{
    build_codex_artifact_bundle, build_profile_bundle, build_profile_bundle_with_legacy_alias,
    load_framework_profile,
};

const ROUTE_DECISION_SCHEMA_VERSION: &str = "router-rs-route-decision-v1";
const ROUTE_POLICY_SCHEMA_VERSION: &str = "router-rs-route-policy-v1";
const ROUTE_SNAPSHOT_SCHEMA_VERSION: &str = "router-rs-route-snapshot-v1";
const ROUTE_AUTHORITY: &str = "rust-route-core";
const PROFILE_COMPILE_AUTHORITY: &str = "rust-route-compiler";
const EXECUTION_SCHEMA_VERSION: &str = "router-rs-execute-response-v1";
const EXECUTION_AUTHORITY: &str = "rust-execution-cli";
const RUNTIME_CONTROL_PLANE_SCHEMA_VERSION: &str = "router-rs-runtime-control-plane-v1";
const RUNTIME_CONTROL_PLANE_AUTHORITY: &str = "rust-runtime-control-plane";
const OVERLAY_ONLY_SKILLS: [&str; 4] = [
    "execution-audit-codex",
    "humanizer",
    "i18n-l10n",
    "iterative-optimizer",
];
const ARTIFACT_GATE_HINTS: [&str; 7] = ["pdf", "docx", "word 文档", "xlsx", "excel", "ppt", "pptx"];
const SOURCE_GATE_HINTS: [&str; 7] = ["官方", "官方文档", "docs", "readme", "api", "openai", "github"];
const EVIDENCE_GATE_HINTS: [&str; 11] = [
    "报错",
    "失败",
    "崩",
    "截图",
    "渲染",
    "日志",
    "traceback",
    "error",
    "bug",
    "why",
    "为什么",
];
const DELEGATION_GATE_HINTS: [&str; 9] = [
    "sidecar",
    "subagent",
    "delegation",
    "并行",
    "子代理",
    "主线程",
    "local-supervisor",
    "跨文件",
    "长运行",
];

#[derive(Parser, Debug)]
#[command(name = "router-rs")]
#[command(about = "Fast Rust routing core for skill lookup")]
struct Cli {
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
    triggers_lower: String,
    fuzzy_tokens: Vec<String>,
    gate_phrases: Vec<String>,
    trigger_phrases: Vec<String>,
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
    rollback_to_python: Option<bool>,
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
            triggers,
            health,
        } = raw;
        let slug_lower = normalize_text(&slug);
        let summary_lower = normalize_text(&summary);
        let triggers_lower = normalize_text(&triggers);
        let mut fuzzy_source = String::with_capacity(
            slug_lower.len() + summary_lower.len() + triggers_lower.len() + 2,
        );
        fuzzy_source.push_str(&slug_lower);
        fuzzy_source.push(' ');
        fuzzy_source.push_str(&triggers_lower);
        fuzzy_source.push(' ');
        fuzzy_source.push_str(&summary_lower);

        let gate_phrases = gate_default_phrases(&gate).unwrap_or_else(|| split_phrases(&gate));
        let trigger_phrases = split_phrases(&triggers);
        let name_tokens = tokenize_query(&slug.replace('-', " "))
            .into_iter()
            .collect::<HashSet<_>>();
        let keyword_tokens = tokenize_query(&format!("{summary} {triggers}"))
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
            triggers_lower,
            fuzzy_tokens: tokenize_query(&fuzzy_source),
            gate_phrases,
            trigger_phrases,
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
    triggers: String,
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
            "choose only one output mode among --json, --route-json, --route-policy-json, --route-snapshot-json, --execute-json, --runtime-control-plane-json, --route-report-json, --profile-json, and --profile-artifacts-json"
                .to_string(),
        );
    }

    if args.runtime_control_plane_json {
        println!(
            "{}",
            serde_json::to_string(&build_runtime_control_plane_payload())
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
    let idx_triggers = *index
        .get("triggers")
        .ok_or_else(|| format!("runtime index missing triggers key: {}", path.display()))?;
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
        idx_triggers,
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
                triggers: value_to_string(&row[idx_triggers]),
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
    let idx_triggers = *key_index
        .get("triggers")
        .ok_or_else(|| format!("manifest missing triggers key: {}", path.display()))?;
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
        idx_triggers,
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
                triggers: value_to_string(&row[idx_triggers]),
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

fn gate_default_phrases(gate: &str) -> Option<Vec<String>> {
    let phrases = match normalize_text(gate).as_str() {
        "artifact" => ARTIFACT_GATE_HINTS.as_slice(),
        "source" => SOURCE_GATE_HINTS.as_slice(),
        "evidence" => EVIDENCE_GATE_HINTS.as_slice(),
        "delegation" => DELEGATION_GATE_HINTS.as_slice(),
        _ => return None,
    };
    Some(phrases.iter().map(|phrase| normalize_text(phrase)).collect())
}

fn is_overlay_record(record: &SkillRecord) -> bool {
    normalize_text(&record.owner) == "overlay"
        || OVERLAY_ONLY_SKILLS.iter().any(|slug| slug == &record.slug)
}

fn can_be_primary_owner(record: &SkillRecord) -> bool {
    let owner = normalize_text(&record.owner);
    owner != "gate" && owner != "overlay"
}

fn phrase_token_matches(task_token: &str, phrase_token: &str) -> bool {
    let word_like = phrase_token
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ".+#/_-".contains(ch));
    if word_like {
        task_token == phrase_token
    } else {
        task_token.contains(phrase_token)
    }
}

fn text_matches_phrase(query_tokens: &[String], phrase: &str) -> bool {
    let phrase_tokens = tokenize_query(phrase);
    if phrase_tokens.is_empty() {
        return false;
    }
    if phrase_tokens.len() == 1 {
        return query_tokens
            .iter()
            .any(|task_token| phrase_token_matches(task_token, &phrase_tokens[0]));
    }
    if query_tokens.len() < phrase_tokens.len() {
        return false;
    }
    for start in 0..=(query_tokens.len() - phrase_tokens.len()) {
        if phrase_tokens.iter().enumerate().all(|(offset, phrase_token)| {
            phrase_token_matches(&query_tokens[start + offset], phrase_token)
        }) {
            return true;
        }
    }
    false
}

fn term_score(term: &str, record: &SkillRecord) -> f64 {
    if term == record.slug_lower {
        return 16.0;
    }
    if record.slug_lower.contains(term) {
        return 12.0;
    }
    if record.triggers_lower.contains(term) {
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
    let query_tokens = tokenize_query(query)
        .into_iter()
        .collect::<HashSet<String>>();

    let candidates = records
        .iter()
        .map(|record| score_route_candidate(record, &normalized_query, &query_tokens, first_turn))
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
        pick_overlay(records, &normalized_query, &selected.record)
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
        mode: mode.to_string(),
        primary_engine: if mode == "shadow" || rollback_active {
            "python".to_string()
        } else {
            "rust".to_string()
        },
        shadow_engine: Some(if mode == "shadow" || rollback_active {
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
    if let Some(rollback_to_python) = payload.rollback_to_python {
        lines.push(format!("Rollback to python: {rollback_to_python}"));
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
    lines.push(
        "Respond as the selected skill and keep the execution aligned with the routed contract."
            .to_string(),
    );
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
        metadata: serde_json::json!({
            "reason": "router-rs returned a deterministic dry-run payload.",
            "trace_event_count": payload.trace_event_count,
            "trace_output_path": payload.trace_output_path,
            "execution_kernel": "router-rs",
            "execution_kernel_authority": EXECUTION_AUTHORITY,
            "execution_mode": "dry_run",
            "route_engine": payload.route_engine,
            "rollback_to_python": payload.rollback_to_python,
        }),
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
        metadata: serde_json::json!({
            "run_id": live_result.run_id,
            "status": live_result.status,
            "trace_event_count": payload.trace_event_count,
            "trace_output_path": payload.trace_output_path,
            "execution_kernel": "router-rs",
            "execution_kernel_authority": EXECUTION_AUTHORITY,
            "execution_kernel_delegate_family": "rust-cli",
            "execution_kernel_delegate_impl": "router-rs",
            "execution_kernel_live_primary": "router-rs",
            "execution_kernel_live_primary_authority": EXECUTION_AUTHORITY,
            "execution_kernel_live_fallback": Value::Null,
            "execution_kernel_live_fallback_authority": Value::Null,
            "execution_kernel_live_fallback_enabled": false,
            "execution_kernel_live_fallback_mode": "disabled",
            "execution_mode": "live",
            "route_engine": payload.route_engine,
            "rollback_to_python": payload.rollback_to_python,
        }),
    }
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
                "role": "compatibility-fallback-factory",
                "projection": "python-compatibility-only",
                "delegate_kind": "python-agno-fallback",
            },
            "background": {
                "authority": RUNTIME_CONTROL_PLANE_AUTHORITY,
                "role": "background-orchestration",
                "projection": "python-thin-projection",
                "delegate_kind": "asyncio-background-supervisor",
            },
        },
    })
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
        primary_authority: "rust".to_string(),
        route_result_engine: "rust".to_string(),
        shadow_engine: None,
        diff_report_required: false,
        verify_parity_required: false,
    };
    match normalized_mode.as_str() {
        "python" => Ok(RouteExecutionPolicyPayload {
            python_route_required: true,
            primary_authority: "python".to_string(),
            route_result_engine: "python".to_string(),
            ..base
        }),
        "shadow" => Ok(RouteExecutionPolicyPayload {
            python_route_required: true,
            primary_authority: "python".to_string(),
            route_result_engine: "python".to_string(),
            shadow_engine: Some("rust".to_string()),
            diff_report_required: true,
            ..base
        }),
        "verify" => Ok(RouteExecutionPolicyPayload {
            python_route_required: true,
            shadow_engine: Some("python".to_string()),
            diff_report_required: true,
            verify_parity_required: true,
            ..base
        }),
        "rust" if rollback_active => Ok(RouteExecutionPolicyPayload {
            python_route_required: true,
            primary_authority: "python".to_string(),
            route_result_engine: "python".to_string(),
            shadow_engine: Some("rust".to_string()),
            diff_report_required: true,
            ..base
        }),
        "rust" => Ok(base),
        _ => Err(format!(
            "unsupported route mode for --route-policy-json: {mode}"
        )),
    }
}

fn score_route_candidate(
    record: &SkillRecord,
    query_text: &str,
    query_tokens: &HashSet<String>,
    first_turn: bool,
) -> RouteCandidate {
    let mut score = 0.0f64;
    let mut reasons = Vec::new();

    let query_token_list = tokenize_query(query_text);

    if !record.slug_lower.is_empty() && text_matches_phrase(&query_token_list, &record.slug) {
        score += 100.0;
        reasons.push(format!("Exact skill name matched: {}.", record.slug));
    }

    let matched_gates = record
        .gate_phrases
        .iter()
        .filter(|phrase| text_matches_phrase(&query_token_list, phrase))
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

    let matched_trigger_phrases = record
        .trigger_phrases
        .iter()
        .filter(|phrase| phrase.chars().count() >= 2 && text_matches_phrase(&query_token_list, phrase))
        .cloned()
        .collect::<Vec<_>>();
    if !matched_trigger_phrases.is_empty() {
        score += (matched_trigger_phrases.len() as f64) * 20.0;
        reasons.push(format!(
            "Trigger phrase matched: {}.",
            matched_trigger_phrases.join(", ")
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
    records
        .iter()
        .filter(|record| can_be_primary_owner(record))
        .min_by(|left, right| {
            layer_rank(&left.layer)
                .cmp(&layer_rank(&right.layer))
                .then_with(|| priority_rank(&left.priority).cmp(&priority_rank(&right.priority)))
                .then_with(|| left.slug.cmp(&right.slug))
        })
        .or_else(|| {
            records.iter().min_by(|left, right| {
                layer_rank(&left.layer)
                    .cmp(&layer_rank(&right.layer))
                    .then_with(|| priority_rank(&left.priority).cmp(&priority_rank(&right.priority)))
                    .then_with(|| left.slug.cmp(&right.slug))
            })
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
    let owner_candidates = candidates
        .iter()
        .filter(|candidate| can_be_primary_owner(&candidate.record))
        .cloned()
        .collect::<Vec<_>>();
    let top_owner_score = owner_candidates
        .iter()
        .map(|candidate| candidate.score)
        .fold(f64::NEG_INFINITY, f64::max);
    if let Some(top_gate) = gate_candidates.first().cloned() {
        if top_gate.score >= 30.0 && top_gate.score >= top_owner_score {
            let mut selected = top_gate;
            selected
                .reasons
                .push("Prioritized via gate-before-owner precedence.".to_string());
            return selected;
        }
    }

    let mut layers = owner_candidates
        .iter()
        .map(|candidate| candidate.record.layer.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    layers.sort_by_key(|layer| layer_rank(layer));

    for layer in layers {
        let mut layer_candidates = owner_candidates
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

    let mut fallback_candidates = owner_candidates;
    if fallback_candidates.is_empty() {
        fallback_candidates = candidates;
    }
    fallback_candidates.sort_by(|left, right| {
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
    fallback_candidates.remove(0)
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
    query_text: &str,
    selected_skill: &SkillRecord,
) -> Option<String> {
    let auto_anti_laziness = matches!(selected_skill.layer.as_str(), "L-1" | "L0" | "L1");
    let anti_laziness = records.iter().find(|record| record.slug == "anti-laziness");
    let query_tokens = tokenize_query(query_text);

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
        let explicit_name_match = text_matches_phrase(&query_tokens, &record.slug)
            || text_matches_phrase(&query_tokens, &record.slug.replace('-', " "));
        let explicit_trigger_match = record
            .trigger_phrases
            .iter()
            .any(|phrase| phrase.chars().count() > 3 && text_matches_phrase(&query_tokens, phrase));
        if explicit_name_match || explicit_trigger_match {
            return Some(record.slug.clone());
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
            rollback_to_python: Some(false),
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

        assert_eq!(report.mode, "shadow");
        assert_eq!(report.primary_engine, "python");
        assert_eq!(report.shadow_engine.as_deref(), Some("rust"));
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
        assert_eq!(python.primary_authority, "python");
        assert_eq!(python.route_result_engine, "python");
        assert!(python.shadow_engine.is_none());
        assert!(!python.diff_report_required);
        assert!(!python.verify_parity_required);

        let shadow = build_route_policy("shadow", false).expect("shadow policy");
        assert!(shadow.python_route_required);
        assert_eq!(shadow.primary_authority, "python");
        assert_eq!(shadow.route_result_engine, "python");
        assert_eq!(shadow.shadow_engine.as_deref(), Some("rust"));
        assert!(shadow.diff_report_required);
        assert!(!shadow.verify_parity_required);

        let verify = build_route_policy("verify", false).expect("verify policy");
        assert!(verify.python_route_required);
        assert_eq!(verify.primary_authority, "rust");
        assert_eq!(verify.route_result_engine, "rust");
        assert_eq!(verify.shadow_engine.as_deref(), Some("python"));
        assert!(verify.diff_report_required);
        assert!(verify.verify_parity_required);

        let rust = build_route_policy("rust", false).expect("rust policy");
        assert!(!rust.python_route_required);
        assert_eq!(rust.primary_authority, "rust");
        assert_eq!(rust.route_result_engine, "rust");
        assert!(rust.shadow_engine.is_none());
        assert!(!rust.diff_report_required);
        assert!(!rust.rollback_active);

        let rollback = build_route_policy("rust", true).expect("rollback policy");
        assert!(rollback.python_route_required);
        assert_eq!(rollback.primary_authority, "python");
        assert_eq!(rollback.route_result_engine, "python");
        assert_eq!(rollback.shadow_engine.as_deref(), Some("rust"));
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
        assert_eq!(payload["default_route_mode"], Value::String("rust".to_string()));
        assert_eq!(payload["default_route_authority"], Value::String(ROUTE_AUTHORITY.to_string()));
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
        assert!(prompt.contains("Rollback to python: false"));
        assert!(prompt.contains("Trigger phrase matched: 直接做代码."));
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
        assert_eq!(response.metadata["execution_kernel_delegate_impl"], "router-rs");
        assert_eq!(response.metadata["execution_kernel_live_primary"], "router-rs");
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
