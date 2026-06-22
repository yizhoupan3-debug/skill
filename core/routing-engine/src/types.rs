//! Route payload and record types.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRecord {
    pub slug: String,
    pub skill_path: Option<String>,
    pub layer: String,
    pub owner: String,
    pub gate: String,
    pub priority: String,
    pub session_start: String,
    pub summary: String,
    pub slug_lower: String,
    pub owner_lower: String,
    pub gate_lower: String,
    pub session_start_lower: String,
    pub gate_phrases: Vec<String>,
    pub trigger_hints: Vec<String>,
    pub name_tokens: HashSet<String>,
    pub keyword_tokens: HashSet<String>,
    pub alias_tokens: HashSet<String>,
    pub do_not_use_tokens: HashSet<String>,
    pub framework_alias_entrypoints: Vec<String>,
    pub metadata_positive_triggers: Vec<String>,
    pub host_platforms: Vec<String>,
    pub record_kind: String,
    pub primary_allowed: bool,
    pub fallback_policy_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchRow {
    pub slug: String,
    pub layer: String,
    pub owner: String,
    pub gate: String,
    pub description: String,
    pub score: f64,
    pub matched_terms: usize,
    pub total_terms: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMatchRecordPayload {
    pub name: String,
    pub description: String,
    pub routing_layer: String,
    pub routing_gate: String,
    pub routing_owner: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMatchPayload {
    pub record: SearchMatchRecordPayload,
    pub score: f64,
    pub matched_terms: usize,
    pub total_terms: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultsPayload {
    pub search_schema_version: String,
    pub authority: String,
    pub query: String,
    pub matches: Vec<SearchMatchPayload>,
}

#[derive(Debug, Clone)]
pub struct RouteCandidate<'a> {
    pub record: &'a SkillRecord,
    pub score: f64,
    pub reasons: Vec<String>,
    pub matched_token_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RecordsCacheKey {
    pub runtime_path: Option<PathBuf>,
    pub manifest_path: Option<PathBuf>,
    pub metadata_sidecar_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct RecordsCacheEntry {
    pub runtime_mtime: Option<SystemTime>,
    pub manifest_mtime: Option<SystemTime>,
    pub metadata_mtime: Option<SystemTime>,
    pub index_mtime: Option<SystemTime>,
    pub records: Arc<Vec<SkillRecord>>,
}

#[derive(Debug, Default)]
pub struct RecordsCacheState {
    pub map: HashMap<RecordsCacheKey, RecordsCacheEntry>,
    /// FIFO of admitted keys; used to evict oldest insertions when `map` exceeds
    /// [`RECORDS_CACHE_MAX_KEYS`]. Refreshes of an existing key do not enqueue again.
    pub fifo: VecDeque<RecordsCacheKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteDecisionSnapshotPayload {
    pub engine: String,
    pub selected_skill: String,
    pub overlay_skill: Option<String>,
    pub layer: String,
    pub score: f64,
    pub score_bucket: String,
    pub reasons: Vec<String>,
    pub matched_token_count: usize,
    pub reasons_class: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteDiffReportPayload {
    pub report_schema_version: String,
    pub authority: String,
    pub mode: String,
    pub primary_engine: String,
    pub evidence_kind: String,
    pub strict_verification: bool,
    pub verification_passed: bool,
    pub verified_contract_fields: Vec<String>,
    pub contract_mismatch_fields: Vec<String>,
    pub route_snapshot: RouteDecisionSnapshotPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteDecision {
    pub decision_schema_version: String,
    pub authority: String,
    pub compile_authority: String,
    pub task: String,
    pub session_id: String,
    pub selected_skill: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_skill_path: Option<String>,
    pub overlay_skill: Option<String>,
    #[serde(default = "default_route_context_payload")]
    pub route_context: RouteContextPayload,
    pub layer: String,
    pub score: f64,
    pub reasons: Vec<String>,
    pub matched_token_count: usize,
    #[serde(default)]
    pub fuzzy_match: bool,
    pub route_snapshot: RouteDecisionSnapshotPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteContextPayload {
    pub execution_protocol: String,
    pub verification_required: bool,
    pub evidence_required: bool,
    pub supervisor_required: bool,
    pub delegation_candidate: bool,
    pub continue_safe_local_steps: bool,
    pub route_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteExecutionPolicyPayload {
    pub policy_schema_version: String,
    pub authority: String,
    pub mode: String,
    pub diagnostic_route_mode: String,
    pub primary_authority: String,
    pub route_result_engine: String,
    pub diagnostic_report_required: bool,
    pub strict_verification_required: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InlineSkillRecordPayload {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub short_description: String,
    #[serde(default)]
    pub when_to_use: String,
    #[serde(default)]
    pub do_not_use: String,
    #[serde(default = "default_skill_layer")]
    pub routing_layer: String,
    #[serde(default = "default_skill_owner")]
    pub routing_owner: String,
    #[serde(default = "default_skill_gate")]
    pub routing_gate: String,
    #[serde(default = "default_skill_priority")]
    pub routing_priority: String,
    #[serde(default = "default_skill_session_start")]
    pub session_start: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub trigger_hints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteResolutionPayload {
    pub schema_version: String,
    pub authority: String,
    pub policy: RouteExecutionPolicyPayload,
    pub route_diagnostic_report: Option<RouteDiffReportPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteSnapshotRequestPayload {
    pub engine: String,
    pub selected_skill: String,
    pub overlay_skill: Option<String>,
    pub layer: String,
    pub score: f64,
    pub reasons: Vec<String>,
    pub matched_token_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteSnapshotEnvelopePayload {
    pub snapshot_schema_version: String,
    pub authority: String,
    pub route_snapshot: RouteDecisionSnapshotPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutingEvalCasePayload {
    pub id: Option<Value>,
    pub task: String,
    pub category: String,
    #[serde(default = "default_true")]
    pub first_turn: bool,
    pub expected_owner: Option<String>,
    pub expected_overlay: Option<String>,
    pub focus_skill: Option<String>,
    #[serde(default)]
    pub forbidden_owners: Vec<String>,
    /// When set, `evaluate_routing_cases` fails if `RouteDecision.layer` differs.
    #[serde(default)]
    pub expected_layer: Option<String>,
    /// When set, must match `RouteDecision.route_context` exactly.
    #[serde(default)]
    pub route_context: Option<RouteContextPayload>,
    /// Human-only fixture commentary; ignored by eval harness.
    #[serde(default)]
    pub notes: Option<String>,
    /// When set, filter hot records before `route_task` (aligns with `eval_route` / stdio route).
    #[serde(default)]
    pub host_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutingEvalCasesPayload {
    pub schema_version: String,
    #[serde(default)]
    pub cases: Vec<RoutingEvalCasePayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingEvalResultPayload {
    pub id: Option<Value>,
    pub category: String,
    pub task: String,
    pub focus_skill: Option<String>,
    pub selected_owner: String,
    pub selected_overlay: Option<String>,
    pub expected_owner: Option<String>,
    pub expected_overlay: Option<String>,
    pub forbidden_owners: Vec<String>,
    pub trigger_hit: bool,
    pub overtrigger: bool,
    pub owner_correct: bool,
    pub overlay_correct: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RoutingEvalMetricsPayload {
    pub case_count: usize,
    pub trigger_hit: usize,
    pub trigger_miss: usize,
    pub overtrigger: usize,
    pub owner_correct: usize,
    pub overlay_correct: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingEvalReportPayload {
    pub schema_version: String,
    pub metrics: RoutingEvalMetricsPayload,
    pub results: Vec<RoutingEvalResultPayload>,
}

pub struct EvaluatedRoutingCase {
    pub input_index: usize,
    pub result: RoutingEvalResultPayload,
}

pub struct RawSkillRecord {
    pub slug: String,
    pub skill_path: Option<String>,
    pub layer: String,
    pub owner: String,
    pub gate: String,
    pub priority: String,
    pub session_start: String,
    pub summary: String,
    pub short_description: String,
    pub when_to_use: String,
    pub do_not_use: String,
    pub tags: Vec<String>,
    pub trigger_hints: Vec<String>,
    pub host_platforms: Vec<String>,
    pub record_kind: String,
}

#[derive(Debug, Clone, Default)]
pub struct RouteMetadataPatch {
    pub priority: Option<String>,
    pub session_start: Option<String>,
    pub positive_triggers: Vec<String>,
    pub negative_triggers: Vec<String>,
    pub primary_allowed: Option<bool>,
    pub fallback_policy_mode: Option<String>,
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

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy)]
pub struct RecordRowIndexes {
    pub slug: usize,
    pub skill_path: Option<usize>,
    pub layer: usize,
    pub owner: usize,
    pub gate: usize,
    pub summary: usize,
    pub trigger_hints: usize,
    pub host_platforms: Option<usize>,
    pub record_kind: Option<usize>,
    pub priority: Option<usize>,
    pub session_start: Option<usize>,
    pub required_max: usize,
}

impl RecordRowIndexes {
    pub fn from_required(
        required: [usize; 6],
        priority: Option<usize>,
        session_start: Option<usize>,
    ) -> Self {
        let [slug, layer, owner, gate, summary, trigger_hints] = required;
        let required_max = *required.iter().max().expect("required columns");
        Self {
            slug,
            skill_path: None,
            layer,
            owner,
            gate,
            summary,
            trigger_hints,
            host_platforms: None,
            record_kind: None,
            priority,
            session_start,
            required_max,
        }
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
