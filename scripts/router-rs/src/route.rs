use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

pub(crate) const ROUTE_DECISION_SCHEMA_VERSION: &str = "router-rs-route-decision-v1";
pub(crate) const SEARCH_RESULTS_SCHEMA_VERSION: &str = "router-rs-search-results-v1";
pub(crate) const ROUTE_POLICY_SCHEMA_VERSION: &str = "router-rs-route-policy-v1";
pub(crate) const ROUTE_SNAPSHOT_SCHEMA_VERSION: &str = "router-rs-route-snapshot-v1";
pub(crate) const ROUTE_REPORT_SCHEMA_VERSION: &str = "router-rs-route-report-v2";
pub(crate) const ROUTE_RESOLUTION_SCHEMA_VERSION: &str = "router-rs-route-resolution-v1";
pub(crate) const ROUTE_AUTHORITY: &str = "rust-route-core";
pub(crate) const PROFILE_COMPILE_AUTHORITY: &str = "rust-route-compiler";
const NO_SKILL_SELECTED: &str = "none";
const ARTIFACT_GATE_PHRASES: [&str; 16] = [
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
    "演示文稿",
    "presentation",
    "deck",
    "slide deck",
];
const PARALLEL_RECORD_SCAN_MIN: usize = 48;
#[cfg(test)]
const PARALLEL_EVAL_CASE_MIN: usize = 8;

#[derive(Debug, Clone)]
pub(crate) struct SkillRecord {
    pub(crate) slug: String,
    pub(crate) layer: String,
    pub(crate) owner: String,
    pub(crate) gate: String,
    pub(crate) priority: String,
    pub(crate) session_start: String,
    pub(crate) summary: String,
    pub(crate) slug_lower: String,
    pub(crate) owner_lower: String,
    pub(crate) gate_lower: String,
    pub(crate) session_start_lower: String,
    pub(crate) gate_phrases: Vec<String>,
    pub(crate) trigger_hints: Vec<String>,
    pub(crate) name_tokens: HashSet<String>,
    pub(crate) keyword_tokens: HashSet<String>,
    pub(crate) alias_tokens: HashSet<String>,
    pub(crate) do_not_use_tokens: HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MatchRow {
    pub(crate) slug: String,
    pub(crate) layer: String,
    pub(crate) owner: String,
    pub(crate) gate: String,
    pub(crate) description: String,
    pub(crate) score: f64,
    pub(crate) matched_terms: usize,
    pub(crate) total_terms: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SearchMatchRecordPayload {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) routing_layer: String,
    pub(crate) routing_gate: String,
    pub(crate) routing_owner: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SearchMatchPayload {
    pub(crate) record: SearchMatchRecordPayload,
    pub(crate) score: f64,
    pub(crate) matched_terms: usize,
    pub(crate) total_terms: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SearchResultsPayload {
    pub(crate) search_schema_version: String,
    pub(crate) authority: String,
    pub(crate) query: String,
    pub(crate) matches: Vec<SearchMatchPayload>,
}

#[derive(Debug, Clone)]
struct RouteCandidate<'a> {
    record: &'a SkillRecord,
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
pub(crate) struct RouteDecisionSnapshotPayload {
    pub(crate) engine: String,
    pub(crate) selected_skill: String,
    pub(crate) overlay_skill: Option<String>,
    pub(crate) layer: String,
    pub(crate) score: f64,
    pub(crate) score_bucket: String,
    pub(crate) reasons: Vec<String>,
    pub(crate) reasons_class: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RouteDiffReportPayload {
    pub(crate) report_schema_version: String,
    pub(crate) authority: String,
    pub(crate) mode: String,
    pub(crate) primary_engine: String,
    pub(crate) evidence_kind: String,
    pub(crate) strict_verification: bool,
    pub(crate) verification_passed: bool,
    pub(crate) verified_contract_fields: Vec<String>,
    pub(crate) contract_mismatch_fields: Vec<String>,
    pub(crate) route_snapshot: RouteDecisionSnapshotPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RouteDecision {
    pub(crate) decision_schema_version: String,
    pub(crate) authority: String,
    pub(crate) compile_authority: String,
    pub(crate) task: String,
    pub(crate) session_id: String,
    pub(crate) selected_skill: String,
    pub(crate) overlay_skill: Option<String>,
    #[serde(default = "default_route_context_payload")]
    pub(crate) route_context: RouteContextPayload,
    pub(crate) layer: String,
    pub(crate) score: f64,
    pub(crate) reasons: Vec<String>,
    pub(crate) route_snapshot: RouteDecisionSnapshotPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RouteContextPayload {
    pub(crate) execution_protocol: String,
    pub(crate) verification_required: bool,
    pub(crate) evidence_required: bool,
    pub(crate) supervisor_required: bool,
    pub(crate) delegation_candidate: bool,
    pub(crate) continue_safe_local_steps: bool,
    pub(crate) route_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RouteExecutionPolicyPayload {
    pub(crate) policy_schema_version: String,
    pub(crate) authority: String,
    pub(crate) mode: String,
    pub(crate) diagnostic_route_mode: String,
    pub(crate) primary_authority: String,
    pub(crate) route_result_engine: String,
    pub(crate) diagnostic_report_required: bool,
    pub(crate) strict_verification_required: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct InlineSkillRecordPayload {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    short_description: String,
    #[serde(default)]
    when_to_use: String,
    #[serde(default)]
    do_not_use: String,
    #[serde(default = "default_skill_layer")]
    routing_layer: String,
    #[serde(default = "default_skill_owner")]
    routing_owner: String,
    #[serde(default = "default_skill_gate")]
    routing_gate: String,
    #[serde(default = "default_skill_priority")]
    routing_priority: String,
    #[serde(default = "default_skill_session_start")]
    session_start: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default, alias = "trigger_phrases")]
    trigger_hints: Vec<String>,
    #[serde(default = "default_skill_health")]
    health: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RouteResolutionPayload {
    pub(crate) schema_version: String,
    pub(crate) authority: String,
    pub(crate) policy: RouteExecutionPolicyPayload,
    pub(crate) route_diagnostic_report: Option<RouteDiffReportPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RouteSnapshotRequestPayload {
    pub(crate) engine: String,
    pub(crate) selected_skill: String,
    pub(crate) overlay_skill: Option<String>,
    pub(crate) layer: String,
    pub(crate) score: f64,
    pub(crate) reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RouteSnapshotEnvelopePayload {
    pub(crate) snapshot_schema_version: String,
    pub(crate) authority: String,
    pub(crate) route_snapshot: RouteDecisionSnapshotPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(test)]
struct RoutingEvalCasePayload {
    id: Option<Value>,
    task: String,
    category: String,
    #[serde(default = "default_true")]
    first_turn: bool,
    expected_owner: Option<String>,
    expected_overlay: Option<String>,
    focus_skill: Option<String>,
    #[serde(default)]
    forbidden_owners: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(test)]
pub(crate) struct RoutingEvalCasesPayload {
    schema_version: String,
    #[serde(default)]
    cases: Vec<RoutingEvalCasePayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(test)]
pub(crate) struct RoutingEvalResultPayload {
    pub(crate) id: Option<Value>,
    category: String,
    task: String,
    focus_skill: Option<String>,
    pub(crate) selected_owner: String,
    pub(crate) selected_overlay: Option<String>,
    expected_owner: Option<String>,
    expected_overlay: Option<String>,
    forbidden_owners: Vec<String>,
    pub(crate) trigger_hit: bool,
    pub(crate) overtrigger: bool,
    pub(crate) owner_correct: bool,
    pub(crate) overlay_correct: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg(test)]
pub(crate) struct RoutingEvalMetricsPayload {
    pub(crate) case_count: usize,
    pub(crate) trigger_hit: usize,
    pub(crate) trigger_miss: usize,
    pub(crate) overtrigger: usize,
    pub(crate) owner_correct: usize,
    pub(crate) overlay_correct: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(test)]
pub(crate) struct RoutingEvalReportPayload {
    pub(crate) schema_version: String,
    pub(crate) metrics: RoutingEvalMetricsPayload,
    pub(crate) results: Vec<RoutingEvalResultPayload>,
}

#[cfg(test)]
struct EvaluatedRoutingCase {
    input_index: usize,
    result: RoutingEvalResultPayload,
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
            short_description,
            when_to_use,
            do_not_use,
            tags,
            trigger_hints,
            _health: _,
        } = raw;
        let slug_lower = normalize_text(&slug);
        let owner_lower = normalize_text(&owner);
        let gate_lower = normalize_text(&gate);
        let session_start_lower = normalize_text(&session_start);
        let alias_tokens = tags
            .iter()
            .flat_map(|tag| tokenize_query(tag))
            .collect::<HashSet<_>>();
        let do_not_use_tokens = tokenize_query(&do_not_use)
            .into_iter()
            .filter(|token| {
                !common_route_stop_tokens().contains(&token.as_str()) && token.len() > 2
            })
            .collect::<HashSet<_>>();
        let gate_phrases = gate_hint_phrases(&gate);
        let name_tokens = tokenize_query(&slug.replace('-', " "))
            .into_iter()
            .collect::<HashSet<_>>();
        let keyword_tokens = tokenize_query(&format!(
            "{summary} {short_description} {when_to_use} {} {}",
            trigger_hints.join(" "),
            tags.join(" ")
        ))
        .into_iter()
        .filter(|token| {
            !common_route_stop_tokens().contains(&token.as_str())
                && (token.chars().count() > 1 || token.chars().any(|ch| !ch.is_ascii()))
        })
        .collect::<HashSet<_>>();

        Self {
            slug,
            layer,
            owner,
            gate,
            priority,
            session_start,
            summary,
            slug_lower,
            owner_lower,
            gate_lower,
            session_start_lower,
            gate_phrases,
            trigger_hints,
            name_tokens,
            keyword_tokens,
            alias_tokens,
            do_not_use_tokens,
        }
    }
}

struct RawSkillRecord {
    slug: String,
    layer: String,
    owner: String,
    gate: String,
    priority: String,
    session_start: String,
    summary: String,
    short_description: String,
    when_to_use: String,
    do_not_use: String,
    tags: Vec<String>,
    trigger_hints: Vec<String>,
    _health: f64,
}

fn default_skill_layer() -> String {
    "L3".to_string()
}

fn default_skill_owner() -> String {
    "owner".to_string()
}

fn default_skill_gate() -> String {
    "none".to_string()
}

fn default_skill_priority() -> String {
    "P2".to_string()
}

fn default_skill_session_start() -> String {
    "n/a".to_string()
}

#[cfg(test)]
fn default_true() -> bool {
    true
}

fn default_skill_health() -> f64 {
    100.0
}

pub(crate) fn build_search_results_payload(
    query: &str,
    matches: Vec<MatchRow>,
) -> SearchResultsPayload {
    SearchResultsPayload {
        search_schema_version: SEARCH_RESULTS_SCHEMA_VERSION.to_string(),
        authority: ROUTE_AUTHORITY.to_string(),
        query: query.to_string(),
        matches: matches
            .into_iter()
            .map(|row| SearchMatchPayload {
                record: SearchMatchRecordPayload {
                    name: row.slug,
                    description: row.description,
                    routing_layer: row.layer,
                    routing_gate: row.gate,
                    routing_owner: row.owner,
                },
                score: row.score,
                matched_terms: row.matched_terms,
                total_terms: row.total_terms,
            })
            .collect(),
    }
}

pub(crate) fn load_records(
    runtime_path: Option<&Path>,
    manifest_path: Option<&Path>,
) -> Result<Vec<SkillRecord>, String> {
    if runtime_path.is_none() {
        if let Some(path) = manifest_path {
            if path.exists() {
                return load_records_from_manifest(path);
            }
        }
    }
    let default_runtime_path = default_runtime_path();
    let runtime_path = runtime_path.or(default_runtime_path.as_deref());
    if let Some(path) = runtime_path {
        if path.exists() {
            let mut records = load_records_from_runtime(path)?;
            if let Some(manifest) = manifest_path {
                if manifest.exists() {
                    let meta = load_manifest_route_meta(manifest)?;
                    apply_manifest_route_meta(&mut records, &meta);
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

pub(crate) fn load_inline_records(payload: &Value) -> Result<Vec<SkillRecord>, String> {
    let rows = payload
        .get("skills")
        .and_then(Value::as_array)
        .ok_or_else(|| "inline route requires a skills array".to_string())?;
    if rows.len() < PARALLEL_RECORD_SCAN_MIN {
        return rows.iter().map(inline_skill_record).collect();
    }
    rows.par_iter().map(inline_skill_record).collect()
}

fn inline_skill_record(row: &Value) -> Result<SkillRecord, String> {
    let skill = serde_json::from_value::<InlineSkillRecordPayload>(row.clone())
        .map_err(|err| format!("parse inline skill payload failed: {err}"))?;
    Ok(SkillRecord::from_raw(RawSkillRecord {
        slug: skill.name,
        layer: skill.routing_layer,
        owner: skill.routing_owner,
        gate: skill.routing_gate,
        priority: skill.routing_priority,
        session_start: skill.session_start,
        summary: skill.description,
        short_description: skill.short_description,
        when_to_use: skill.when_to_use,
        do_not_use: skill.do_not_use,
        tags: skill.tags,
        trigger_hints: skill.trigger_hints,
        _health: skill.health,
    }))
}

fn build_skill_record_from_indexed_row(row: &[Value], indexes: &RecordRowIndexes) -> SkillRecord {
    SkillRecord::from_raw(RawSkillRecord {
        slug: value_to_string(&row[indexes.slug]),
        layer: value_to_string(&row[indexes.layer]),
        owner: value_to_string(&row[indexes.owner]),
        gate: value_to_string(&row[indexes.gate]),
        priority: indexes
            .priority
            .and_then(|idx| row.get(idx))
            .map(value_to_string)
            .unwrap_or_else(|| "P2".to_string()),
        session_start: indexes
            .session_start
            .and_then(|idx| row.get(idx))
            .map(value_to_string)
            .unwrap_or_else(|| "n/a".to_string()),
        summary: value_to_string(&row[indexes.summary]),
        short_description: String::new(),
        when_to_use: String::new(),
        do_not_use: String::new(),
        tags: Vec::new(),
        trigger_hints: value_to_string_list(&row[indexes.trigger_hints]),
        _health: value_to_f64(&row[indexes.health]).unwrap_or(100.0),
    })
}

#[derive(Debug, Clone, Copy)]
struct RecordRowIndexes {
    slug: usize,
    layer: usize,
    owner: usize,
    gate: usize,
    summary: usize,
    trigger_hints: usize,
    health: usize,
    priority: Option<usize>,
    session_start: Option<usize>,
    required_max: usize,
}

impl RecordRowIndexes {
    fn from_required(
        required: [usize; 7],
        priority: Option<usize>,
        session_start: Option<usize>,
    ) -> Self {
        let [slug, layer, owner, gate, summary, trigger_hints, health] = required;
        let required_max = *required.iter().max().expect("required columns");
        Self {
            slug,
            layer,
            owner,
            gate,
            summary,
            trigger_hints,
            health,
            priority,
            session_start,
            required_max,
        }
    }
}

fn collect_skill_records_from_rows(rows: &[Value], indexes: RecordRowIndexes) -> Vec<SkillRecord> {
    let iter = || {
        rows.iter()
            .filter_map(Value::as_array)
            .filter(|row| row.len() > indexes.required_max)
            .map(|row| build_skill_record_from_indexed_row(row, &indexes))
            .collect::<Vec<_>>()
    };
    if rows.len() < PARALLEL_RECORD_SCAN_MIN {
        return iter();
    }
    rows.par_iter()
        .filter_map(Value::as_array)
        .filter(|row| row.len() > indexes.required_max)
        .map(|row| build_skill_record_from_indexed_row(row, &indexes))
        .collect()
}

fn apply_manifest_route_meta(
    records: &mut [SkillRecord],
    meta: &HashMap<String, (String, String)>,
) {
    if records.len() < PARALLEL_RECORD_SCAN_MIN {
        for record in records {
            if let Some((priority, session_start)) = meta.get(&record.slug) {
                record.priority = priority.clone();
                record.session_start = session_start.clone();
            }
        }
        return;
    }
    records.par_iter_mut().for_each(|record| {
        if let Some((priority, session_start)) = meta.get(&record.slug) {
            record.priority = priority.clone();
            record.session_start = session_start.clone();
        }
    });
}

fn default_runtime_path() -> Option<PathBuf> {
    Some(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("skills")
            .join("SKILL_ROUTING_RUNTIME.json"),
    )
}

fn effective_runtime_path(runtime_path: Option<&Path>) -> Option<PathBuf> {
    runtime_path
        .map(Path::to_path_buf)
        .or_else(default_runtime_path)
}

#[cfg(test)]
pub(crate) fn load_records_cached_for_stdio_with_default_runtime_path(
    default_runtime_path: &Path,
    manifest_path: Option<&Path>,
) -> Result<Arc<Vec<SkillRecord>>, String> {
    load_records_cached_for_stdio_resolved(Some(default_runtime_path), manifest_path)
}

fn records_cache_key(runtime_path: Option<&Path>, manifest_path: Option<&Path>) -> RecordsCacheKey {
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

pub(crate) fn load_records_cached_for_stdio(
    runtime_path: Option<&Path>,
    manifest_path: Option<&Path>,
) -> Result<Arc<Vec<SkillRecord>>, String> {
    let runtime_path = effective_runtime_path(runtime_path);
    let runtime_path = runtime_path.as_deref();
    load_records_cached_for_stdio_resolved(runtime_path, manifest_path)
}

fn load_records_cached_for_stdio_resolved(
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
    let indexes = RecordRowIndexes::from_required(
        [
            idx_slug,
            idx_layer,
            idx_owner,
            idx_gate,
            idx_summary,
            idx_trigger_hints,
            idx_health,
        ],
        idx_priority,
        idx_session_start,
    );

    Ok(collect_skill_records_from_rows(rows, indexes))
}

pub(crate) fn load_records_from_manifest(path: &Path) -> Result<Vec<SkillRecord>, String> {
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
    let indexes = RecordRowIndexes::from_required(
        [
            idx_slug,
            idx_layer,
            idx_owner,
            idx_gate,
            idx_desc,
            idx_trigger_hints,
            idx_health,
        ],
        idx_priority,
        idx_session_start,
    );

    Ok(collect_skill_records_from_rows(rows, indexes))
}

pub(crate) fn read_json(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("failed reading {}: {err}", path.display()))?;
    serde_json::from_str(&text).map_err(|err| format!("failed parsing {}: {err}", path.display()))
}

#[cfg(test)]
fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

pub(crate) fn value_to_string(value: &Value) -> String {
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
        "写",
        "做",
        "做一个",
        "部署",
        "文件",
        "看这",
        "这张",
        "然后",
        "输出",
        "问题",
        "a",
        "an",
        "and",
        "are",
        "as",
        "for",
        "in",
        "is",
        "of",
        "or",
        "the",
        "to",
        "with",
        "skill",
        "路由",
    ]
}

fn is_meta_routing_task(query_text: &str) -> bool {
    (query_text.contains("skill")
        || query_text.contains("skill.md")
        || query_text.contains("runtime")
        || query_text.contains("框架"))
        && [
            "路由",
            "触发",
            "routing",
            "router",
            "route",
            "系统",
            "入口",
            "抽象",
            "行为驱动",
            "第一性原理",
            "减法",
            "轻量化",
            "兼容层",
            "胶水层",
            "减少入口",
            "减入口",
            "不损害功能",
            "加重负担",
            "没有用",
            "runtime 轻量化",
            "讨论-规划-执行-验证",
        ]
        .iter()
        .any(|marker| query_text.contains(marker))
}

fn has_checklist_execution_context(query_text: &str) -> bool {
    query_text.contains("checklist")
        && ![
            "规范",
            "规范化",
            "normalize",
            "normalise",
            "serial",
            "parallel",
            "并行",
            "串行",
        ]
        .iter()
        .any(|marker| query_text.contains(marker))
        && [
            "执行",
            "一口气",
            "彻底",
            "落实",
            "按",
            "fix",
            "implement",
            "run",
            "do it",
        ]
        .iter()
        .any(|marker| query_text.contains(marker))
}

fn has_skill_creator_context(query_text: &str, query_token_list: &[String]) -> bool {
    (query_text.contains("skill") || query_text.contains("skill.md"))
        && [
            "创建",
            "新建",
            "写一个",
            "写个",
            "做一个",
            "做个",
            "create",
            "author",
            "scaffold",
            "update",
            "revise",
        ]
        .iter()
        .any(|marker| query_text.contains(marker) || text_matches_phrase(query_token_list, marker))
}

fn has_skill_installer_context(query_text: &str, query_token_list: &[String]) -> bool {
    query_text.contains("skill")
        && [
            "安装",
            "装一下",
            "装一个",
            "装个",
            "导入",
            "引入",
            "install",
            "installed",
            "curated",
            "github",
        ]
        .iter()
        .any(|marker| query_text.contains(marker) || text_matches_phrase(query_token_list, marker))
}

fn has_skill_framework_maintenance_context(query_text: &str, query_token_list: &[String]) -> bool {
    (query_text.contains("skill")
        || query_text.contains("skill.md")
        || query_text.contains("runtime")
        || query_text.contains("框架")
        || query_text.contains(".supervisor_state"))
        && [
            "不好用",
            "持续优化",
            "外部调研",
            "路由没触发",
            "触发不准",
            "优化 skill",
            "framework",
            "routing",
            "skill 系统",
            "skill系统",
            "轻量化",
            "兼容层",
            "胶水层",
            "减少入口",
            "减入口",
            "不损害功能",
            "加重负担",
            "没有用",
            "治理任务",
        ]
        .iter()
        .any(|marker| query_text.contains(marker) || text_matches_phrase(query_token_list, marker))
}

fn has_runtime_lightweighting_context(query_text: &str, query_token_list: &[String]) -> bool {
    [
        "runtime 轻量化",
        "轻量化",
        "兼容层",
        "胶水层",
        "减少入口",
        "减入口",
        "不损害功能",
        "加重负担",
        "没有用",
    ]
    .iter()
    .any(|marker| query_text.contains(marker) || text_matches_phrase(query_token_list, marker))
}

fn has_humanizer_context(query_text: &str, query_token_list: &[String]) -> bool {
    [
        "润色",
        "润色得自然",
        "自然一点",
        "改自然",
        "自然化",
        "文本精修",
        "表达优化",
        "去模板腔",
        "像人写的",
        "humanize",
        "aigc",
        "ai 味",
        "ai味",
        "ai 感",
        "逐句评估",
        "哪些句子",
        "普通说明",
        "说明文字",
        "普通写作",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

fn has_copywriting_context(query_text: &str, query_token_list: &[String]) -> bool {
    [
        "ux 微文案",
        "ux",
        "微文案",
        "空状态",
        "cta",
        "转化",
        "转化率",
        "点击创建",
        "创建项目",
        "广告词",
        "产品卖点",
        "落地页",
        "品牌故事",
        "copywriting",
        "in-app microcopy",
        "tagline",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
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
    record.owner_lower == "overlay"
}

fn can_be_primary_owner(record: &SkillRecord) -> bool {
    record.gate_lower == "none"
        && !framework_alias_requires_explicit_call(&record.slug)
        && !matches!(record.owner_lower.as_str(), "gate" | "overlay")
}

fn can_be_fallback_owner(record: &SkillRecord) -> bool {
    can_be_primary_owner(record)
        && !matches!(
            record.slug.as_str(),
            "coding-standards"
                | "error-handling-patterns"
                | "skill-framework-developer"
                | "plugin-creator"
                | "skill-creator"
                | "skill-installer"
        )
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
            "multiagent".to_string(),
            "multi-agent".to_string(),
            "多 agent".to_string(),
            "子代理".to_string(),
            "主线程".to_string(),
            "local-supervisor".to_string(),
            "跨文件".to_string(),
            "长运行".to_string(),
        ],
        _ => Vec::new(),
    }
}

pub(crate) fn search_skills(records: &[SkillRecord], query: &str, limit: usize) -> Vec<MatchRow> {
    if limit == 0 {
        return Vec::new();
    }
    let normalized_query = normalize_text(query);
    let query_token_list = tokenize_route_text(query);
    let query_tokens = query_token_list
        .iter()
        .filter(|token| !common_route_stop_tokens().contains(&token.as_str()))
        .cloned()
        .collect::<HashSet<String>>();
    if query_tokens.is_empty() && query_token_list.is_empty() {
        return Vec::new();
    }

    let score_record = |record: &SkillRecord| {
        let candidate = score_route_candidate(
            record,
            &normalized_query,
            &query_token_list,
            &query_tokens,
            true,
        );
        if candidate.score <= 0.0 {
            return None;
        }
        Some(MatchRow {
            slug: record.slug.clone(),
            layer: record.layer.clone(),
            owner: record.owner.clone(),
            gate: record.gate.clone(),
            description: record.summary.clone(),
            score: round2(candidate.score),
            matched_terms: candidate.reasons.len(),
            total_terms: query_tokens.len().max(query_token_list.len()),
        })
    };
    let mut rows = if records.len() < PARALLEL_RECORD_SCAN_MIN {
        records.iter().filter_map(score_record).collect::<Vec<_>>()
    } else {
        records
            .par_iter()
            .filter_map(score_record)
            .collect::<Vec<_>>()
    };

    rows.sort_unstable_by(|left, right| {
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

pub(crate) fn route_task(
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
    let route_context = build_route_context(&normalized_query, &query_token_list);

    if let Some(record) = records
        .iter()
        .find(|record| has_literal_framework_alias_call(&normalized_query, &record.slug))
    {
        let reasons =
            compact_route_reasons(&["Framework alias entrypoint matched explicitly.".to_string()]);
        return Ok(RouteDecision {
            decision_schema_version: ROUTE_DECISION_SCHEMA_VERSION.to_string(),
            authority: ROUTE_AUTHORITY.to_string(),
            compile_authority: PROFILE_COMPILE_AUTHORITY.to_string(),
            task: query.to_string(),
            session_id: session_id.to_string(),
            selected_skill: record.slug.clone(),
            overlay_skill: None,
            route_context,
            layer: record.layer.clone(),
            score: 100.0,
            route_snapshot: build_route_snapshot(
                "rust",
                &record.slug,
                None,
                &record.layer,
                100.0,
                &reasons,
            ),
            reasons,
        });
    }

    let score = |record| {
        score_route_candidate(
            record,
            &normalized_query,
            &query_token_list,
            &query_tokens,
            first_turn,
        )
    };
    let viable = if records.len() < PARALLEL_RECORD_SCAN_MIN {
        records
            .iter()
            .map(score)
            .filter(|candidate| candidate.score > 0.0)
            .collect::<Vec<_>>()
    } else {
        records
            .par_iter()
            .map(score)
            .filter(|candidate| candidate.score > 0.0)
            .collect::<Vec<_>>()
    };

    if viable.is_empty() {
        let fallback_reasons = compact_route_reasons(&[
            "No explicit skill hit; native runtime should proceed without loading a skill."
                .to_string(),
        ]);
        return Ok(RouteDecision {
            decision_schema_version: ROUTE_DECISION_SCHEMA_VERSION.to_string(),
            authority: ROUTE_AUTHORITY.to_string(),
            compile_authority: PROFILE_COMPILE_AUTHORITY.to_string(),
            task: query.to_string(),
            session_id: session_id.to_string(),
            selected_skill: NO_SKILL_SELECTED.to_string(),
            overlay_skill: None,
            route_context,
            layer: "runtime".to_string(),
            score: 0.0,
            reasons: fallback_reasons.clone(),
            route_snapshot: build_route_snapshot(
                "rust",
                NO_SKILL_SELECTED,
                None,
                "runtime",
                0.0,
                &fallback_reasons,
            ),
        });
    }
    if viable
        .iter()
        .all(|candidate| is_overlay_record(candidate.record))
    {
        let fallback_reasons = compact_route_reasons(&[
            "Only overlay signals matched; native runtime should proceed without loading a primary skill."
                .to_string(),
        ]);
        let _ = allow_overlay;
        return Ok(RouteDecision {
            decision_schema_version: ROUTE_DECISION_SCHEMA_VERSION.to_string(),
            authority: ROUTE_AUTHORITY.to_string(),
            compile_authority: PROFILE_COMPILE_AUTHORITY.to_string(),
            task: query.to_string(),
            session_id: session_id.to_string(),
            selected_skill: NO_SKILL_SELECTED.to_string(),
            overlay_skill: None,
            route_context,
            layer: "runtime".to_string(),
            score: 0.0,
            reasons: fallback_reasons.clone(),
            route_snapshot: build_route_snapshot(
                "rust",
                NO_SKILL_SELECTED,
                None,
                "runtime",
                0.0,
                &fallback_reasons,
            ),
        });
    }

    let selected = pick_owner(viable);
    let overlay = if allow_overlay {
        pick_overlay(
            records,
            &normalized_query,
            &query_token_list,
            selected.record,
        )
    } else {
        None
    };

    let filtered_overlay = overlay
        .as_ref()
        .filter(|item| *item != &selected.record.slug)
        .cloned();
    let compact_reasons = compact_route_reasons(&selected.reasons);

    Ok(RouteDecision {
        decision_schema_version: ROUTE_DECISION_SCHEMA_VERSION.to_string(),
        authority: ROUTE_AUTHORITY.to_string(),
        compile_authority: PROFILE_COMPILE_AUTHORITY.to_string(),
        task: query.to_string(),
        session_id: session_id.to_string(),
        selected_skill: selected.record.slug.clone(),
        overlay_skill: filtered_overlay,
        route_context,
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
            &compact_reasons,
        ),
        reasons: compact_reasons,
    })
}

pub(crate) fn literal_framework_alias_decision(
    records: &[SkillRecord],
    query: &str,
    session_id: &str,
) -> Option<RouteDecision> {
    let normalized_query = normalize_text(query);
    let query_token_list = tokenize_route_text(query);
    let route_context = build_route_context(&normalized_query, &query_token_list);
    let requested_slug = literal_framework_alias_slug(&normalized_query)?;
    let record = records
        .iter()
        .find(|record| record.slug == requested_slug)?;
    let reasons =
        compact_route_reasons(&["Framework alias entrypoint matched explicitly.".to_string()]);
    Some(RouteDecision {
        decision_schema_version: ROUTE_DECISION_SCHEMA_VERSION.to_string(),
        authority: ROUTE_AUTHORITY.to_string(),
        compile_authority: PROFILE_COMPILE_AUTHORITY.to_string(),
        task: query.to_string(),
        session_id: session_id.to_string(),
        selected_skill: record.slug.clone(),
        overlay_skill: None,
        route_context,
        layer: record.layer.clone(),
        score: 100.0,
        route_snapshot: build_route_snapshot(
            "rust",
            &record.slug,
            None,
            &record.layer,
            100.0,
            &reasons,
        ),
        reasons,
    })
}

fn literal_framework_alias_slug(query_text: &str) -> Option<&'static str> {
    query_text.split_whitespace().find_map(|part| {
        let term = part.trim_matches(|ch: char| {
            matches!(
                ch,
                '(' | ')'
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | '<'
                    | '>'
                    | ','
                    | '.'
                    | '!'
                    | '?'
                    | '，'
                    | '。'
                    | '：'
                    | '；'
                    | '"'
                    | '\''
                    | '`'
            )
        });
        match term {
            "$autopilot" | "/autopilot" => Some("autopilot"),
            "$deepinterview" | "/deepinterview" => Some("deepinterview"),
            "$team" | "/team" => Some("team"),
            _ => None,
        }
    })
}

pub(crate) fn build_route_snapshot(
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

fn default_route_context_payload() -> RouteContextPayload {
    RouteContextPayload {
        execution_protocol: "four_step".to_string(),
        verification_required: true,
        evidence_required: true,
        supervisor_required: false,
        delegation_candidate: false,
        continue_safe_local_steps: false,
        route_reason: "narrowest_domain_owner".to_string(),
    }
}

pub(crate) fn should_retry_with_manifest(decision: &RouteDecision) -> bool {
    decision.score < 35.0
        || (decision.selected_skill == "systematic-debugging" && decision.score < 35.0)
        || decision
            .reasons
            .iter()
            .any(|reason| reason.contains("fell back to highest-priority layer owner"))
        || decision.reasons.iter().any(|reason| {
            reason.contains("Fallback owner selected")
                || reason.contains("No explicit keyword hit")
                || reason.contains("No explicit skill hit")
        })
}

fn route_decision_is_no_hit(decision: &RouteDecision) -> bool {
    decision.score <= 0.0
        || decision.reasons.iter().any(|reason| {
            reason.contains("No explicit keyword hit")
                || reason.contains("No explicit skill hit")
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

pub(crate) fn should_accept_manifest_fallback(
    hot_decision: &RouteDecision,
    full_decision: &RouteDecision,
    should_retry: bool,
    explicit_manifest: bool,
) -> bool {
    if explicit_manifest && !route_decision_is_no_hit(hot_decision) {
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
        && matches!(full_decision.selected_skill.as_str(), "deepinterview");

    if full_decision.score <= 10.0
        && !matches!(full_decision.selected_skill.as_str(), "deepinterview")
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

fn build_route_context(query_text: &str, query_token_list: &[String]) -> RouteContextPayload {
    let completion_requested = completion_execution_markers().iter().any(|marker| {
        query_text.contains(*marker) || text_matches_phrase(query_token_list, marker)
    });
    let supervisor_required = supervisor_execution_markers().iter().any(|marker| {
        query_text.contains(*marker) || text_matches_phrase(query_token_list, marker)
    });
    let delegation_candidate = has_bounded_subagent_context(query_text, query_token_list)
        || has_team_orchestration_context(query_text, query_token_list)
        || has_parallel_review_candidate_context(query_text, query_token_list)
        || has_parallel_execution_context(query_text, query_token_list);
    let audit_requested = [
        "核查",
        "审查",
        "审核",
        "审计",
        "评审",
        "诊断",
        "有什么问题",
        "哪里错了",
        "audit",
        "review",
        "diagnose",
    ]
    .iter()
    .any(|marker| query_text.contains(*marker) || text_matches_phrase(query_token_list, marker));
    let implementation_requested = [
        "实现",
        "修复",
        "开发",
        "落地",
        "直接做代码",
        "implement",
        "fix",
        "code",
    ]
    .iter()
    .any(|marker| query_text.contains(*marker) || text_matches_phrase(query_token_list, marker));
    let route_reason = if supervisor_required {
        "explicit_supervisor_continuity"
    } else if delegation_candidate {
        "delegation_gate_candidate"
    } else if completion_requested {
        "completion_signal_context"
    } else {
        "narrowest_domain_owner"
    };

    RouteContextPayload {
        execution_protocol: if implementation_requested && !audit_requested {
            "implementation"
        } else if audit_requested {
            "audit"
        } else {
            "four_step"
        }
        .to_string(),
        verification_required: true,
        evidence_required: audit_requested || !implementation_requested,
        supervisor_required,
        delegation_candidate,
        continue_safe_local_steps: completion_requested,
        route_reason: route_reason.to_string(),
    }
}

pub(crate) fn build_route_diff_report(
    mode: &str,
    rust_snapshot: RouteDecisionSnapshotPayload,
    route_decision: Option<&RouteDecision>,
) -> Result<RouteDiffReportPayload, String> {
    let normalized_mode = mode.trim().to_ascii_lowercase();
    let strict_verification = match normalized_mode.as_str() {
        "shadow" => false,
        "verify" => true,
        _ => return Err(format!("unsupported route report mode: {mode}")),
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

pub(crate) fn build_route_resolution(
    mode: &str,
    route_decision: &RouteDecision,
) -> Result<RouteResolutionPayload, String> {
    let policy = build_route_policy(mode)?;
    let report = if policy.diagnostic_report_required {
        Some(build_route_diff_report(
            &policy.mode,
            route_decision.route_snapshot.clone(),
            Some(route_decision),
        )?)
    } else {
        None
    };
    if policy.strict_verification_required
        && report
            .as_ref()
            .map(|value| !value.verification_passed)
            .unwrap_or(false)
    {
        let mismatch_fields = report
            .as_ref()
            .map(|value| value.contract_mismatch_fields.join(", "))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "unknown".to_string());
        return Err(format!(
            "Rust verification route report detected contract drift: {mismatch_fields}."
        ));
    }
    Ok(RouteResolutionPayload {
        schema_version: ROUTE_RESOLUTION_SCHEMA_VERSION.to_string(),
        authority: ROUTE_AUTHORITY.to_string(),
        policy,
        route_diagnostic_report: report,
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

#[cfg(test)]
pub(crate) fn load_routing_eval_cases(path: &Path) -> Result<RoutingEvalCasesPayload, String> {
    let payload = read_json(path)?;
    let cases = serde_json::from_value::<RoutingEvalCasesPayload>(payload)
        .map_err(|err| format!("failed parsing {}: {err}", path.display()))?;
    if cases.schema_version != "routing-eval-cases-v1" {
        return Err(format!(
            "routing eval case file returned an unknown schema: {:?}",
            cases.schema_version
        ));
    }
    Ok(cases)
}

#[cfg(test)]
pub(crate) fn evaluate_routing_cases(
    records: &[SkillRecord],
    cases_payload: RoutingEvalCasesPayload,
) -> Result<RoutingEvalReportPayload, String> {
    let mut metrics = RoutingEvalMetricsPayload::default();
    let cases = cases_payload.cases;
    let evaluate_one = |(input_index, case): (usize, RoutingEvalCasePayload)| -> Result<Option<EvaluatedRoutingCase>, String> {
        let task = case.task.trim().to_string();
        if task.is_empty() {
            return Ok(None);
        }

        let session_suffix = case
            .id
            .as_ref()
            .map(value_to_string)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| (input_index + 1).to_string());
        let decision = route_task(
            records,
            &task,
            &format!("routing-eval::{session_suffix}"),
            true,
            case.first_turn,
        )?;
        let selected_owner = decision.selected_skill.clone();
        let selected_overlay = decision.overlay_skill.clone();

        let category = case.category.trim().to_string();
        let expected_owner = normalize_optional_text(case.expected_owner);
        let expected_overlay = normalize_optional_text(case.expected_overlay);
        let focus_skill = normalize_optional_text(case.focus_skill);
        let forbidden_owners = case
            .forbidden_owners
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<HashSet<_>>();

        let mut trigger_hit = false;
        let mut overtrigger = false;
        let owner_correct = expected_owner
            .as_ref()
            .map(|expected| expected == &selected_owner)
            .unwrap_or(false);
        let overlay_correct = match &expected_overlay {
            Some(expected) => Some(expected) == selected_overlay.as_ref(),
            None => selected_overlay.is_none(),
        };

        match category.as_str() {
            "should-trigger" => {
                trigger_hit = focus_skill
                    .as_ref()
                    .map(|focus| focus == &selected_owner)
                    .unwrap_or(false);
            }
            "should-not-trigger" => {
                overtrigger = forbidden_owners.contains(&selected_owner);
            }
            "wrong-owner-near-miss" | "gate-vs-owner-conflict" => {
                trigger_hit = focus_skill
                    .as_ref()
                    .map(|focus| focus == &selected_owner)
                    .unwrap_or(false);
                if forbidden_owners.contains(&selected_owner) {
                    overtrigger = true;
                }
            }
            _ => {}
        }

        let mut forbidden_owner_list = forbidden_owners.into_iter().collect::<Vec<_>>();
        forbidden_owner_list.sort();
        Ok(Some(EvaluatedRoutingCase {
            input_index,
            result: RoutingEvalResultPayload {
                id: case.id,
                category,
                task,
                focus_skill,
                selected_owner,
                selected_overlay,
                expected_owner,
                expected_overlay,
                forbidden_owners: forbidden_owner_list,
                trigger_hit,
                overtrigger,
                owner_correct,
                overlay_correct,
            },
        }))
    };

    let mut evaluated = if cases.len() < PARALLEL_EVAL_CASE_MIN {
        cases
            .into_iter()
            .enumerate()
            .map(evaluate_one)
            .collect::<Result<Vec<_>, _>>()?
    } else {
        cases
            .into_par_iter()
            .enumerate()
            .map(evaluate_one)
            .collect::<Vec<_>>()
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
    }
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    evaluated.sort_by_key(|row| row.input_index);

    let mut results = Vec::with_capacity(evaluated.len());
    for row in evaluated {
        metrics.case_count += 1;
        match row.result.category.as_str() {
            "should-trigger" | "wrong-owner-near-miss" | "gate-vs-owner-conflict" => {
                if row.result.trigger_hit {
                    metrics.trigger_hit += 1;
                } else {
                    metrics.trigger_miss += 1;
                }
            }
            _ => {}
        }
        if row.result.overtrigger {
            metrics.overtrigger += 1;
        }
        if row.result.owner_correct {
            metrics.owner_correct += 1;
        }
        if row.result.overlay_correct {
            metrics.overlay_correct += 1;
        }
        results.push(row.result);
    }

    Ok(RoutingEvalReportPayload {
        schema_version: "routing-eval-v1".to_string(),
        metrics,
        results,
    })
}

fn completion_execution_markers() -> [&'static str; 10] {
    [
        "gsd",
        "get shit done",
        "推进到底",
        "别停",
        "直接干完",
        "一路做完",
        "持续跑到收敛",
        "给验证证据",
        "给我验证证据",
        "验证证据",
    ]
}

fn supervisor_execution_markers() -> [&'static str; 9] {
    [
        ".supervisor_state.json",
        "共享 continuity",
        "shared continuity",
        "多 lane 集成",
        "主线程集成",
        "integration supervisor",
        "supervisor",
        "长运行",
        "状态持久化",
    ]
}

fn framework_alias_explicit_entrypoints(slug: &str) -> &'static [&'static str] {
    match slug {
        "autopilot" => &["/autopilot", "$autopilot"],
        "deepinterview" => &["/deepinterview", "$deepinterview"],
        "gitx" => &["/gitx", "$gitx", "gitx"],
        "team" => &["/team", "$team"],
        _ => &[],
    }
}

fn framework_alias_requires_explicit_call(slug: &str) -> bool {
    !framework_alias_explicit_entrypoints(slug).is_empty()
}

fn framework_alias_literal_entrypoints(slug: &str) -> &'static [&'static str] {
    framework_alias_explicit_entrypoints(slug)
}

fn has_literal_framework_alias_call(query_text: &str, slug: &str) -> bool {
    framework_alias_literal_entrypoints(slug)
        .iter()
        .any(|entrypoint| has_explicit_entrypoint_term(query_text, &normalize_text(entrypoint)))
}

fn has_explicit_entrypoint_term(query_text: &str, entrypoint: &str) -> bool {
    query_text.split_whitespace().any(|part| {
        part.trim_matches(|ch: char| {
            matches!(
                ch,
                '(' | ')'
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | '<'
                    | '>'
                    | ','
                    | '.'
                    | '!'
                    | '?'
                    | '，'
                    | '。'
                    | '：'
                    | '；'
                    | '"'
                    | '\''
                    | '`'
            )
        }) == entrypoint
    })
}

fn has_explicit_framework_alias_call(
    query_text: &str,
    query_token_list: &[String],
    slug: &str,
) -> bool {
    framework_alias_explicit_entrypoints(slug)
        .iter()
        .any(|entrypoint| {
            has_explicit_entrypoint_term(query_text, &normalize_text(entrypoint))
                || query_token_list.iter().any(|token| token == entrypoint)
        })
}

fn has_bounded_subagent_context(query_text: &str, query_token_list: &[String]) -> bool {
    [
        "sidecar",
        "sidecars",
        "subagent",
        "subagents",
        "delegation plan",
        "multiagent",
        "multi-agent",
        "多 agent",
        "多 agent 执行",
        "多 agent 路由",
        "bounded sidecar",
        "bounded sidecars",
        "bounded subagent",
        "bounded subagents",
        "subagent lane",
        "sidecar lane",
        "local-supervisor",
        "local-supervisor queue",
        "保留 sidecar 边界",
        "只切 sidecar",
        "并行 sidecar",
        "不实际 spawn",
        "stay local",
        "主线程保留",
        "保留主线程",
        "主线程集成",
        "lane-local output",
        "不创建 worker",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

fn has_token_budget_pressure(query_text: &str, query_token_list: &[String]) -> bool {
    [
        "token budget",
        "context budget",
        "token 开销",
        "token 成本",
        "降低 token",
        "压 token",
        "省 token",
        "缩上下文",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

fn has_team_negation_context(query_text: &str, query_token_list: &[String]) -> bool {
    [
        "不要 team",
        "不要进入 team",
        "不进 team",
        "不用 team",
        "无需 team",
        "not team",
        "without team",
        "不要 team orchestration",
        "只是 sidecar",
        "only sidecar",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

fn has_team_orchestration_context(query_text: &str, query_token_list: &[String]) -> bool {
    [
        "team orchestration",
        "team workflow",
        "team mode",
        "team supervisor",
        "worker lifecycle",
        "worker orchestration",
        "multi-worker",
        "multi worker",
        "parallel worker",
        "parallel workers",
        "disjoint files",
        "disjoint file",
        "disjoint write",
        "disjoint writes",
        "disjoint scope",
        "disjoint scopes",
        "disjoint write scope",
        "disjoint write scopes",
        "lane-local",
        "lane local",
        "lane-local delta",
        "worker write scope",
        "worker write scopes",
        "team 协作",
        "团队编排",
        "多 worker",
        "worker 生命周期",
        "supervisor-led",
        "supervisor led",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

fn has_parallel_execution_context(query_text: &str, query_token_list: &[String]) -> bool {
    let explicit_parallel = [
        "并行",
        "同时",
        "分头",
        "分路",
        "分三路",
        "多路",
        "多线",
        "多方向",
        "多个方向",
        "独立方向",
        "独立维度",
        "parallel",
        "concurrent",
        "in parallel",
        "split lanes",
        "split work",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    });
    if !explicit_parallel {
        return false;
    }

    let split_shape = [
        "三个方向",
        "三方向",
        "三个模块",
        "三模块",
        "多个模块",
        "多个假设",
        "多个独立",
        "前端",
        "后端",
        "测试",
        "api",
        "数据库",
        "ui",
        "安全",
        "性能",
        "架构",
        "实现",
        "策略",
        "验证",
        "frontend",
        "backend",
        "testing",
        "tests",
        "database",
        "security",
        "performance",
        "architecture",
        "implementation",
        "verification",
    ]
    .iter()
    .filter(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
    .count();

    split_shape >= 2
}

fn has_parallel_review_candidate_context(query_text: &str, query_token_list: &[String]) -> bool {
    let review_requested = [
        "review",
        "code review",
        "审查",
        "审核",
        "审计",
        "评审",
        "代码 review",
        "代码审查",
        "架构审查",
        "安全审查",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    });
    if !review_requested {
        return false;
    }

    let broad_or_independent = [
        "深度",
        "全面",
        "全量",
        "全仓",
        "仓库级",
        "整个仓库",
        "这个仓库",
        "repo-wide",
        "codebase-wide",
        "跨模块",
        "多模块",
        "多维",
        "多方向",
        "多假设",
        "第一性原理",
        "多余入口",
        "不必要抽象",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    });
    if !broad_or_independent {
        return false;
    }

    [
        "仓库",
        "repo",
        "codebase",
        "代码库",
        "架构",
        "architecture",
        "系统",
        "路由",
        "skill",
        "模块",
        "边界",
        "实现质量",
        "bug",
        "风险",
        "findings",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

fn paper_skill_requires_context(slug: &str) -> bool {
    matches!(
        slug,
        "paper-workbench" | "paper-reviewer" | "paper-reviser" | "paper-writing"
    )
}

fn has_paper_context(query_text: &str, query_token_list: &[String]) -> bool {
    [
        "paper",
        "manuscript",
        "论文",
        "稿子",
        "稿件",
        "摘要",
        "引言",
        "审稿意见",
        "reviewer comments",
        "rebuttal",
        "appendix",
        "claim",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

fn has_github_pr_context(query_text: &str, query_token_list: &[String]) -> bool {
    ["github", "gh", "pull request", "pr"].iter().any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

fn has_paper_review_revision_intent(query_text: &str, query_token_list: &[String]) -> bool {
    if !has_paper_context(query_text, query_token_list) {
        return false;
    }
    let review_markers = [
        "review",
        "reviewer comments",
        "review comments",
        "审稿意见",
        "评审意见",
    ];
    let revise_markers = ["改论文", "修改论文", "改稿", "修改稿", "进入修改", "直接改"];
    review_markers.iter().any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    }) && revise_markers.iter().any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

fn has_paper_direct_revision_context(query_text: &str, query_token_list: &[String]) -> bool {
    if !has_paper_context(query_text, query_token_list) {
        return false;
    }
    if [
        "该删就删",
        "藏到附录",
        "改到能投",
        "根据 reviewer comments 修改论文",
        "根据 reviewer comments 改论文",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    }) {
        return false;
    }
    [
        "别先给方案",
        "直接进入修改",
        "直接改稿",
        "不要再审",
        "只进改稿",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

fn has_paper_workbench_frontdoor_context(query_text: &str, query_token_list: &[String]) -> bool {
    if !has_paper_context(query_text, query_token_list) {
        return false;
    }
    [
        "整体推进这篇论文",
        "现在该审",
        "该审",
        "该改",
        "该补实验",
        "怎么处理",
        "先审再改",
        "改到能投",
        "该删就删",
        "藏到附录",
        "根据 reviewer comments 修改论文",
        "根据 reviewer comments 改论文",
        "能不能投",
        "整篇严审",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

fn has_paper_writing_context(query_text: &str, query_token_list: &[String]) -> bool {
    if !has_paper_context(query_text, query_token_list) {
        return false;
    }
    if has_paper_ref_first_workflow_context(query_text, query_token_list)
        || has_paper_review_judgment_context(query_text, query_token_list)
        || query_text.contains("别润色")
        || query_text.contains("不润色")
    {
        return false;
    }
    [
        "润色",
        "文字精修",
        "表达",
        "故事线",
        "重写摘要",
        "重写引言",
        "只改表达",
        "polish",
        "rewrite introduction",
        "rewrite abstract",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

fn has_paper_review_judgment_context(query_text: &str, query_token_list: &[String]) -> bool {
    if !has_paper_context(query_text, query_token_list) {
        return false;
    }
    [
        "paper review",
        "review paper",
        "审稿",
        "审一下",
        "严审",
        "投稿前",
        "能不能投",
        "投稿判断",
        "reviewer-style",
        "reviewer style",
        "外部调研",
        "查文献后审",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

fn has_paper_figure_layout_review_context(query_text: &str, query_token_list: &[String]) -> bool {
    if !has_paper_context(query_text, query_token_list) {
        return false;
    }
    let visual_markers = [
        "图表", "排版", "figure", "figures", "table", "tables", "layout",
    ];
    let review_markers = ["只看", "审", "review", "检查", "别检查别的维度"];
    visual_markers.iter().any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    }) && review_markers.iter().any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

fn has_paper_logic_evidence_review_context(query_text: &str, query_token_list: &[String]) -> bool {
    if !has_paper_context(query_text, query_token_list) {
        return false;
    }
    let logic_markers = [
        "claim",
        "claims",
        "evidence",
        "证据",
        "支撑",
        "实验支撑",
        "对齐",
        "够不够",
    ];
    let review_markers = ["看", "检查", "评估", "review", "审", "别润色"];
    logic_markers.iter().any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    }) && review_markers.iter().any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

fn has_paper_ref_first_workflow_context(query_text: &str, query_token_list: &[String]) -> bool {
    if !has_paper_context(query_text, query_token_list) {
        return false;
    }
    let ref_markers = [
        "下载ref",
        "目标期刊",
        "相近ref",
        "相近 ref",
        "reference corpus",
        "target journal",
    ];
    let story_or_write_markers = [
        "讲故事",
        "故事线",
        "写作套路",
        "重写摘要",
        "重写引言",
        "再写",
        "再帮我重写",
    ];
    ref_markers.iter().any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    }) && story_or_write_markers.iter().any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

fn has_design_reference_context(query_text: &str, query_token_list: &[String]) -> bool {
    [
        "参考源",
        "verified tokens",
        "品牌 token",
        "stripe",
        "linear",
        "apple",
        "vercel",
        "liquid glass motion",
        "产品风格映射",
        "borrowable cues",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

fn has_visual_evidence_review_context(query_text: &str, query_token_list: &[String]) -> bool {
    [
        "看图",
        "截图",
        "界面图",
        "视觉问题",
        "可读性审查",
        "重叠",
        "层级",
        "渲染",
        "rendered",
        "screenshot",
        "visual review",
        "ui overlap",
        "readability review",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

fn artifact_gate_matches_query(query_token_list: &[String]) -> bool {
    ARTIFACT_GATE_PHRASES
        .iter()
        .any(|phrase| text_matches_phrase(query_token_list, phrase))
}

fn artifact_gate_target_slug(query_token_list: &[String]) -> Option<&'static str> {
    const ARTIFACT_TARGETS: [(&str, &[&str]); 4] = [
        (
            "spreadsheets",
            &[
                "xlsx",
                "excel",
                "spreadsheet",
                "xls",
                "csv",
                "tsv",
                "sheet review",
                "工作簿",
            ],
        ),
        (
            "slides",
            &[
                "ppt",
                "pptx",
                "slides",
                "powerpoint",
                "presentation",
                "deck",
                "slide deck",
                "幻灯片",
                "演示文稿",
            ],
        ),
        ("doc", &["docx", "word 文档", "word 文件"]),
        ("pdf", &["pdf"]),
    ];

    ARTIFACT_TARGETS.iter().find_map(|(slug, phrases)| {
        phrases
            .iter()
            .any(|phrase| text_matches_phrase(query_token_list, phrase))
            .then_some(*slug)
    })
}

fn has_design_contract_context(query_text: &str, query_token_list: &[String]) -> bool {
    const MARKERS: [&str; 18] = [
        "design.md",
        "设计规范",
        "设计系统",
        "设计 token",
        "design token",
        "design tokens",
        "视觉身份",
        "视觉规范",
        "品牌风格",
        "品牌规范",
        "house style",
        "visual identity",
        "style contract",
        "统一设计规范",
        "统一视觉",
        "统一风格",
        "风格漂移",
        "根据 design.md",
    ];
    MARKERS.iter().any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

fn has_design_contract_negation_context(query_text: &str, query_token_list: &[String]) -> bool {
    const MARKERS: [&str; 10] = [
        "不需要设计系统",
        "不需要设计规范",
        "不用设计系统",
        "不用设计规范",
        "无需设计系统",
        "无需设计规范",
        "不要设计系统",
        "不要设计规范",
        "no design system",
        "without design system",
    ];
    MARKERS.iter().any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

fn has_design_output_audit_context(query_text: &str, query_token_list: &[String]) -> bool {
    [
        "设计审计",
        "设计验收",
        "验收结论",
        "风格漂移",
        "ai 味",
        "反模式",
        "drift",
        "anti-pattern",
        "audit produced",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

fn has_design_workflow_protocol_context(query_text: &str, query_token_list: &[String]) -> bool {
    [
        "设计工件协议",
        "设计工作流",
        "设计迭代协议",
        "design workflow",
        "design artifact protocol",
        "prompt 到 screenshot 到 verdict",
        "每轮都按这个工作流跑",
        "工作流跑",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

fn has_quick_artifact_context(query_text: &str, query_token_list: &[String]) -> bool {
    const MARKERS: [&str; 8] = [
        "快速", "普通", "简单", "临时", "quick", "simple", "draft", "utility",
    ];
    MARKERS.iter().any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

fn should_defer_to_artifact_gate(
    record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    if record.gate_lower != "none" || !artifact_gate_matches_query(query_token_list) {
        return false;
    }
    let explicit_entry = format!("${}", record.slug_lower);
    if query_text.contains(&explicit_entry) {
        return false;
    }
    record.session_start_lower == "n/a"
        && (record
            .name_tokens
            .iter()
            .any(|token| query_token_list.contains(token))
            || record
                .trigger_hints
                .iter()
                .any(|hint| text_matches_phrase(query_token_list, hint)))
}

fn should_suppress_non_target_artifact_gate(
    record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    if record.slug == "design-md"
        && has_design_contract_context(query_text, query_token_list)
        && !has_design_contract_negation_context(query_text, query_token_list)
    {
        return false;
    }
    record.gate_lower == "artifact"
        && !is_meta_routing_task(query_text)
        && artifact_gate_target_slug(query_token_list)
            .map(|target| record.slug != target)
            .unwrap_or(false)
}

fn should_prefer_design_contract_over_artifact(
    record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    record.slug == "slides"
        && has_design_contract_context(query_text, query_token_list)
        && !has_design_contract_negation_context(query_text, query_token_list)
}

pub(crate) fn build_route_policy(mode: &str) -> Result<RouteExecutionPolicyPayload, String> {
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
        _ => return Err(format!("unsupported route policy mode: {mode}")),
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

fn score_route_candidate<'a>(
    record: &'a SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    query_tokens: &HashSet<String>,
    first_turn: bool,
) -> RouteCandidate<'a> {
    let mut score = 0.0f64;
    let mut reasons = Vec::new();

    if record.slug == "systematic-debugging" && is_meta_routing_task(query_text) {
        return RouteCandidate {
            record,
            score: 0.0,
            reasons: vec![
                "Suppressed: meta-routing repair request should not be treated as a generic runtime-debugging gate."
                    .to_string(),
            ],
        };
    }
    if record.slug == "agent-swarm-orchestration"
        && is_meta_routing_task(query_text)
        && !has_parallel_execution_context(query_text, query_token_list)
        && !has_team_orchestration_context(query_text, query_token_list)
        && !has_bounded_subagent_context(query_text, query_token_list)
    {
        return RouteCandidate {
            record,
            score: 0.0,
            reasons: vec![
                "Suppressed: skill-system routing reviews stay on skill-framework-developer unless explicit parallel lanes are requested."
                    .to_string(),
            ],
        };
    }
    if record.gate_lower == "artifact" && is_meta_routing_task(query_text) {
        return RouteCandidate {
            record,
            score: 0.0,
            reasons: vec![
                "Suppressed: meta-routing repair request should not be treated as artifact work."
                    .to_string(),
            ],
        };
    }
    let _checklist_execution_context = has_checklist_execution_context(query_text);
    if record.slug == "skill-creator" && has_skill_creator_context(query_text, query_token_list) {
        score += 70.0;
        reasons.push(
            "Skill-creator boost applied: concrete skill authoring or SKILL.md revision wording detected."
                .to_string(),
        );
    }
    if record.slug == "skill-installer" && has_skill_installer_context(query_text, query_token_list)
    {
        score += 70.0;
        reasons.push(
            "Skill-installer boost applied: skill installation or import wording detected."
                .to_string(),
        );
    }
    if record.slug == "skill-framework-developer"
        && has_skill_framework_maintenance_context(query_text, query_token_list)
    {
        score += 70.0;
        reasons.push(
            "Skill-framework boost applied: skill-library maintenance or skill-quality repair wording detected."
                .to_string(),
        );
    }
    if record.slug == "documentation-engineering"
        && has_humanizer_context(query_text, query_token_list)
        && !has_paper_context(query_text, query_token_list)
    {
        score += 52.0;
        reasons.push(
            "Documentation-engineering polish boost applied: prose naturalization or sentence-level AI-flavor audit detected."
                .to_string(),
        );
    }
    if record.slug == "copywriting" && has_copywriting_context(query_text, query_token_list) {
        score += 56.0;
        reasons.push(
            "Copywriting boost applied: conversion-oriented UX or marketing copy wording detected."
                .to_string(),
        );
    }
    let literal_framework_alias = framework_alias_requires_explicit_call(&record.slug)
        && has_literal_framework_alias_call(query_text, &record.slug);
    let bounded_subagent_context = has_bounded_subagent_context(query_text, query_token_list);
    let team_negation_context = has_team_negation_context(query_text, query_token_list);
    let token_budget_pressure = has_token_budget_pressure(query_text, query_token_list);
    if record.slug == "team" && team_negation_context && !literal_framework_alias {
        return RouteCandidate {
            record,
            score: 0.0,
            reasons: vec![
                "Suppressed: query explicitly rejects team orchestration and should stay on bounded multi-agent lanes."
                    .to_string(),
            ],
        };
    }
    let explicit_framework_alias = framework_alias_requires_explicit_call(&record.slug)
        && has_explicit_framework_alias_call(query_text, query_token_list, &record.slug);
    let parallel_execution_context = has_parallel_execution_context(query_text, query_token_list);
    if record.slug == "agent-swarm-orchestration"
        && (bounded_subagent_context
            || has_team_orchestration_context(query_text, query_token_list)
            || has_parallel_review_candidate_context(query_text, query_token_list)
            || parallel_execution_context)
    {
        score += 60.0;
        reasons.push(
            "Agent-swarm boost applied: multi-agent delegation or worker orchestration wording detected."
                .to_string(),
        );
        if parallel_execution_context {
            score += 12.0;
            reasons.push(
                "Parallel-execution boost applied: independent lanes can run as bounded sidecars."
                    .to_string(),
            );
        }
        if has_parallel_review_candidate_context(query_text, query_token_list) {
            score += 10.0;
            reasons.push(
                "Parallel-review boost applied: broad review scope should run subagent admission before a single-lane review."
                    .to_string(),
            );
        }
        if bounded_subagent_context && token_budget_pressure {
            score += 8.0;
            reasons.push(
                "Token-budget boost applied: bounded sidecars fit prompt-budget pressure better than wider orchestration."
                    .to_string(),
            );
        }
    }
    if framework_alias_requires_explicit_call(&record.slug) && !explicit_framework_alias {
        return RouteCandidate {
            record,
            score: 0.0,
            reasons: vec![
                "Suppressed: framework alias skills only route from explicit /alias or $alias entrypoints."
                    .to_string(),
            ],
        };
    }
    if paper_skill_requires_context(&record.slug)
        && !has_paper_context(query_text, query_token_list)
    {
        return RouteCandidate {
            record,
            score: 0.0,
            reasons: vec![
                "Suppressed: paper skills require explicit paper or manuscript context."
                    .to_string(),
            ],
        };
    }
    if matches!(record.slug.as_str(), "gh-address-comments")
        && has_paper_context(query_text, query_token_list)
        && !has_github_pr_context(query_text, query_token_list)
    {
        return RouteCandidate {
            record,
            score: 0.0,
            reasons: vec![
                "Suppressed: paper review or revision requests without explicit GitHub/PR context should stay on paper lanes."
                    .to_string(),
            ],
        };
    }
    if record.slug == "design-md"
        && has_humanizer_context(query_text, query_token_list)
        && !has_design_contract_context(query_text, query_token_list)
    {
        return RouteCandidate {
            record,
            score: 0.0,
            reasons: vec![
                "Suppressed: prose naturalization should not route through the design artifact gate."
                    .to_string(),
            ],
        };
    }
    if record.slug == "native-app-debugging"
        && has_copywriting_context(query_text, query_token_list)
    {
        return RouteCandidate {
            record,
            score: 0.0,
            reasons: vec![
                "Suppressed: UX marketing or microcopy wording belongs to copywriting, not native-app debugging."
                    .to_string(),
            ],
        };
    }
    if should_defer_to_artifact_gate(record, query_text, query_token_list) {
        return RouteCandidate {
            record,
            score: 0.0,
            reasons: vec![
                "Suppressed: generic artifact intake should hit the artifact gate before a narrower owner."
                    .to_string(),
            ],
        };
    }
    if should_suppress_non_target_artifact_gate(record, query_text, query_token_list) {
        return RouteCandidate {
            record,
            score: 0.0,
            reasons: vec![
                "Suppressed: artifact wording targets a different canonical artifact gate."
                    .to_string(),
            ],
        };
    }
    if should_prefer_design_contract_over_artifact(record, query_text, query_token_list) {
        return RouteCandidate {
            record,
            score: 0.0,
            reasons: vec![
                "Suppressed: reusable design contract must precede slide authoring.".to_string(),
            ],
        };
    }
    if record.slug == "paper-workbench"
        && has_paper_ref_first_workflow_context(query_text, query_token_list)
    {
        score += 42.0;
        reasons.push(
            "Paper-workbench boost applied: target-journal ref-first manuscript workflow detected."
                .to_string(),
        );
    }
    if record.slug == "paper-workbench"
        && has_paper_workbench_frontdoor_context(query_text, query_token_list)
    {
        score += 54.0;
        reasons.push(
            "Paper-workbench boost applied: manuscript front-door workflow or next-step triage detected."
                .to_string(),
        );
    }
    if record.slug == "paper-workbench"
        && has_paper_review_judgment_context(query_text, query_token_list)
    {
        score += 36.0;
        reasons.push(
            "Paper-workbench boost applied: paper review judgment with optional external calibration detected."
                .to_string(),
        );
    }
    if record.slug == "paper-reviewer"
        && has_paper_figure_layout_review_context(query_text, query_token_list)
    {
        score += 38.0;
        reasons.push(
            "Paper-reviewer boost applied: figure/layout-only paper review slice detected."
                .to_string(),
        );
    }
    if record.slug == "paper-reviewer"
        && has_paper_logic_evidence_review_context(query_text, query_token_list)
    {
        score += 72.0;
        reasons.push(
            "Paper-reviewer boost applied: claim/evidence alignment review requested.".to_string(),
        );
    }
    if record.slug == "paper-reviewer"
        && has_paper_review_judgment_context(query_text, query_token_list)
        && query_text.contains("别润色")
    {
        score += 74.0;
        reasons.push(
            "Paper-reviewer boost applied: claim/evidence review-only paper judgment requested."
                .to_string(),
        );
    }
    if record.slug == "paper-reviser"
        && has_paper_direct_revision_context(query_text, query_token_list)
    {
        score += 82.0;
        reasons.push(
            "Paper-reviser boost applied: direct reviewer-comment manuscript revision requested."
                .to_string(),
        );
    }
    if record.slug == "paper-reviewer" && has_paper_writing_context(query_text, query_token_list) {
        return RouteCandidate {
            record,
            score: 0.0,
            reasons: vec![
                "Suppressed: bounded manuscript prose polish should route to paper-writing, not paper-reviewer."
                    .to_string(),
            ],
        };
    }
    if record.slug == "paper-writing" && has_paper_writing_context(query_text, query_token_list) {
        score += 40.0;
        reasons.push(
            "Paper-writing boost applied: bounded manuscript prose polish or storyline wording detected."
                .to_string(),
        );
    }
    if record.slug == "skill-framework-developer" && is_meta_routing_task(query_text) {
        score += 60.0;
        reasons.push(
            "Skill-framework boost applied: skill-system routing, behavior protocol, subtraction, or abstraction wording detected."
                .to_string(),
        );
    }
    if record.slug == "skill-framework-developer"
        && has_runtime_lightweighting_context(query_text, query_token_list)
    {
        score += 74.0;
        reasons.push(
            "Skill-framework boost applied: runtime lightweighting, compatibility-layer, glue-layer, or entrypoint-reduction wording detected."
                .to_string(),
        );
    }
    if record.slug == "design-md"
        && has_design_contract_negation_context(query_text, query_token_list)
    {
        return RouteCandidate {
            record,
            score: 0.0,
            reasons: vec![
                "Suppressed: query explicitly says a design-system contract is not needed."
                    .to_string(),
            ],
        };
    }
    let design_output_audit_context = has_design_output_audit_context(query_text, query_token_list);
    let design_workflow_protocol_context =
        has_design_workflow_protocol_context(query_text, query_token_list);
    if record.slug == "design-md" && design_output_audit_context {
        score += 44.0;
        reasons.push(
            "Design-md audit boost applied: UI drift, anti-pattern, or acceptance verdict wording detected."
                .to_string(),
        );
    }
    if record.slug == "design-md" && design_workflow_protocol_context {
        score += 44.0;
        reasons.push(
            "Design-md workflow boost applied: durable design artifact workflow wording detected."
                .to_string(),
        );
    }
    if record.slug == "design-md" && has_design_reference_context(query_text, query_token_list) {
        score += 74.0;
        reasons.push(
            "Design-md reference boost applied: named-product reference source grounding requested."
                .to_string(),
        );
    }
    if record.slug == "design-md"
        && has_design_contract_context(query_text, query_token_list)
        && !design_output_audit_context
        && !design_workflow_protocol_context
    {
        if has_quick_artifact_context(query_text, query_token_list) {
            score *= 0.65;
            reasons.push(
                "Design-md quick-task suppression applied: one-off artifact wording should not force a design contract."
                    .to_string(),
            );
        } else {
            score += 42.0;
            reasons.push(
                "Design-md boost applied: reusable visual contract or design-token wording detected."
                    .to_string(),
            );
        }
    }
    if explicit_framework_alias {
        score += 1000.0;
        reasons.push("Framework alias entrypoint matched explicitly.".to_string());
    }

    if !record.slug_lower.is_empty()
        && (text_matches_phrase(query_token_list, &record.slug_lower)
            || query_text.contains(&format!("${}", record.slug_lower)))
    {
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

    let mut alias_hits = record
        .alias_tokens
        .iter()
        .filter(|token| query_tokens.contains(*token))
        .cloned()
        .collect::<Vec<_>>();
    alias_hits.sort();
    if !alias_hits.is_empty() {
        score += 12.0 + (alias_hits.len() as f64) * 4.0;
        reasons.push(format!(
            "Skill alias hints matched: {}.",
            alias_hits
                .iter()
                .take(8)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    if first_turn && score > 0.0 {
        if record.session_start_lower == "required" {
            score += 8.0;
            reasons.push("Session-start required boost applied (+8).".to_string());
        } else if record.session_start_lower == "preferred" {
            score += 3.0;
            reasons.push("Session-start preferred boost applied (+3).".to_string());
        }
    }

    if record.owner_lower == "gate" && score > 0.0 {
        score += 2.0;
    }

    let visual_evidence_review_context =
        has_visual_evidence_review_context(query_text, query_token_list);
    let redesign_context = text_matches_phrase(query_token_list, "重新梳理")
        || text_matches_phrase(query_token_list, "改版")
        || text_matches_phrase(query_token_list, "redesign");

    if record.slug == "visual-review"
        && first_turn
        && visual_evidence_review_context
        && !redesign_context
    {
        score += 36.0;
        reasons.push(
            "Visual-review boost applied: visible UI evidence and concrete visual findings requested."
                .to_string(),
        );
    }

    if record.slug == "subagent-delegation" && score > 0.0 {
        if bounded_subagent_context {
            score += 22.0;
            reasons.push(
                "Bounded-sidecar boost applied: query prefers multi-agent sidecars without full team orchestration."
                    .to_string(),
            );
        }
        if bounded_subagent_context && token_budget_pressure {
            score += 8.0;
            reasons.push(
                "Token-budget boost applied: bounded sidecars fit prompt-budget pressure better than wider orchestration."
                    .to_string(),
            );
        }
        if team_negation_context {
            score += 16.0;
            reasons.push(
                "Team-negation boost applied: query says bounded multi-agent routing should avoid team."
                    .to_string(),
            );
        }
    }

    if record.slug == "team" && score > 0.0 && bounded_subagent_context && !explicit_framework_alias
    {
        score *= 0.2;
        reasons.push(
            "Team suppression applied: bounded sidecar wording prefers subagent-delegation over team."
                .to_string(),
        );
    }

    if record.slug == "team" && score > 0.0 && bounded_subagent_context && token_budget_pressure {
        score *= 0.6;
        reasons.push(
            "Team suppression applied: token-budget pressure favors bounded sidecars over full team orchestration."
                .to_string(),
        );
    }

    if record.slug == "team" && score > 0.0 && !explicit_framework_alias {
        score *= 0.25;
        reasons.push("Team suppression applied: team needs explicit entry.".to_string());
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
                record,
                score: 0.0,
                reasons: vec![
                    "Suppressed: visual-review requires visible evidence, not a generic review token."
                        .to_string(),
                ],
            };
        }
    }

    if !record.do_not_use_tokens.is_empty() && score > 0.0 {
        let negative_hits = record
            .do_not_use_tokens
            .iter()
            .filter(|token| query_tokens.contains(*token))
            .cloned()
            .collect::<Vec<_>>();
        if !negative_hits.is_empty() {
            let penalty = f64::min(score * 0.3, (negative_hits.len() as f64) * 5.0);
            score = f64::max(0.0, score - penalty);
            reasons.push(format!(
                "Do-not-use penalty applied: {}.",
                negative_hits
                    .into_iter()
                    .take(5)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    if record.slug == "paper-workbench"
        && has_paper_review_revision_intent(query_text, query_token_list)
    {
        score += 28.0;
        reasons.push(
            "Paper workbench boost applied: review-driven manuscript revision intent detected."
                .to_string(),
        );
    }

    if is_overlay_record(record) && score > 0.0 {
        score *= 0.15;
        reasons.push(format!(
            "Owner suppression applied: {} is overlay-only.",
            record.slug
        ));
    }
    RouteCandidate {
        record,
        score,
        reasons,
    }
}

fn pick_owner<'a>(candidates: Vec<RouteCandidate<'a>>) -> RouteCandidate<'a> {
    let mut owner_candidates = candidates
        .iter()
        .filter(|candidate| can_be_primary_owner(candidate.record))
        .cloned()
        .collect::<Vec<_>>();
    owner_candidates.sort_unstable_by(route_candidate_cmp);
    let top_owner_score = owner_candidates
        .first()
        .map(|candidate| candidate.score)
        .unwrap_or(f64::NEG_INFINITY);
    let top_gate = candidates
        .iter()
        .filter(|candidate| {
            candidate.record.owner_lower == "gate" || candidate.record.gate_lower != "none"
        })
        .min_by(|left, right| route_candidate_cmp(left, right))
        .cloned();
    if let Some(mut top_gate) = top_gate
        .as_ref()
        .filter(|candidate| {
            candidate.record.slug == "agent-swarm-orchestration" && candidate.score >= 60.0
        })
        .cloned()
    {
        top_gate.reasons.push(
            "Prioritized delegation gate before strong owner for broad parallel-review admission."
                .to_string(),
        );
        return top_gate;
    }
    if let Some(top_owner) = owner_candidates.first() {
        if top_owner.score >= 60.0 {
            return top_owner.clone();
        }
    }
    if let Some(mut top_gate) =
        top_gate.filter(|candidate| candidate.score >= 30.0 && candidate.score >= top_owner_score)
    {
        top_gate
            .reasons
            .push("Prioritized via gate-before-owner precedence.".to_string());
        return top_gate;
    }
    let owner_pool = if owner_candidates.is_empty() {
        candidates
            .iter()
            .filter(|candidate| !is_overlay_record(candidate.record))
            .cloned()
            .collect::<Vec<_>>()
    } else {
        owner_candidates.clone()
    };
    let owner_pool = if owner_pool.is_empty() {
        candidates
            .iter()
            .filter(|candidate| can_be_fallback_owner(candidate.record))
            .cloned()
            .collect::<Vec<_>>()
    } else {
        owner_pool
    };
    let owner_pool = if owner_pool.is_empty() {
        candidates.clone()
    } else {
        owner_pool
    };

    let mut layers = owner_pool
        .iter()
        .map(|candidate| candidate.record.layer.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    layers.sort_unstable_by_key(|layer| layer_rank(layer));

    for layer in layers {
        let mut layer_candidates = owner_pool
            .iter()
            .filter(|candidate| candidate.record.layer == layer)
            .cloned()
            .collect::<Vec<_>>();
        layer_candidates.sort_unstable_by(route_candidate_cmp);
        if let Some(top) = layer_candidates.first().cloned() {
            if top.score >= layer_threshold(&layer) {
                return top;
            }
        }
    }

    let mut fallback_pool = owner_pool;
    fallback_pool.sort_unstable_by(|left, right| {
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

fn route_candidate_cmp(left: &RouteCandidate<'_>, right: &RouteCandidate<'_>) -> Ordering {
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
    _query_text: &str,
    query_tokens: &[String],
    selected_skill: &SkillRecord,
) -> Option<String> {
    let mut ordered = records.iter().collect::<Vec<_>>();
    ordered.sort_unstable_by(|left, right| {
        layer_rank(&left.layer)
            .cmp(&layer_rank(&right.layer))
            .then_with(|| priority_rank(&left.priority).cmp(&priority_rank(&right.priority)))
            .then_with(|| left.slug.cmp(&right.slug))
    });

    for record in ordered {
        if record.slug == selected_skill.slug {
            continue;
        }
        if !is_overlay_record(record) {
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

    None
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn score_bucket(score: f64) -> String {
    let floor = ((score.max(0.0) / 10.0).floor() as i32) * 10;
    format!("{floor:02}-{ceiling:02}", ceiling = floor + 9)
}

fn compact_route_reasons(reasons: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut compact = Vec::new();
    for reason in reasons {
        let normalized = normalize_text(reason);
        if normalized.is_empty() || !seen.insert(normalized) {
            continue;
        }
        compact.push(reason.clone());
        if compact.len() >= 6 {
            break;
        }
    }
    compact
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
