//! Route payload and record types.
use serde::{Deserialize, Serialize};
#[cfg(test)]
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub(crate) struct SkillRecord {
    pub(crate) slug: String,
    pub(crate) skill_path: Option<String>,
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
    pub(crate) framework_alias_entrypoints: Vec<String>,
    pub(crate) metadata_positive_triggers: Vec<String>,
    pub(crate) host_platforms: Vec<String>,
    pub(crate) record_kind: String,
    pub(crate) primary_allowed: bool,
    pub(crate) fallback_policy_mode: String,
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
pub(crate) struct RouteCandidate<'a> {
    pub(crate) record: &'a SkillRecord,
    pub(crate) score: f64,
    pub(crate) reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct RecordsCacheKey {
    pub(crate) runtime_path: Option<PathBuf>,
    pub(crate) manifest_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub(crate) struct RecordsCacheEntry {
    pub(crate) runtime_mtime: Option<SystemTime>,
    pub(crate) manifest_mtime: Option<SystemTime>,
    pub(crate) metadata_mtime: Option<SystemTime>,
    pub(crate) records: Arc<Vec<SkillRecord>>,
}

#[derive(Debug, Default)]
pub(crate) struct RecordsCacheState {
    pub(crate) map: HashMap<RecordsCacheKey, RecordsCacheEntry>,
    /// FIFO of admitted keys; used to evict oldest insertions when `map` exceeds
    /// [`RECORDS_CACHE_MAX_KEYS`]. Refreshes of an existing key do not enqueue again.
    pub(crate) fifo: VecDeque<RecordsCacheKey>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) selected_skill_path: Option<String>,
    pub(crate) overlay_skill: Option<String>,
    #[serde(default = "default_route_context_payload")]
    pub(crate) route_context: RouteContextPayload,
    pub(crate) layer: String,
    pub(crate) score: f64,
    pub(crate) reasons: Vec<String>,
    pub(crate) route_snapshot: RouteDecisionSnapshotPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
pub(crate) struct InlineSkillRecordPayload {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) description: String,
    #[serde(default)]
    pub(crate) short_description: String,
    #[serde(default)]
    pub(crate) when_to_use: String,
    #[serde(default)]
    pub(crate) do_not_use: String,
    #[serde(default = "default_skill_layer")]
    pub(crate) routing_layer: String,
    #[serde(default = "default_skill_owner")]
    pub(crate) routing_owner: String,
    #[serde(default = "default_skill_gate")]
    pub(crate) routing_gate: String,
    #[serde(default = "default_skill_priority")]
    pub(crate) routing_priority: String,
    #[serde(default = "default_skill_session_start")]
    pub(crate) session_start: String,
    #[serde(default)]
    pub(crate) tags: Vec<String>,
    #[serde(default, alias = "trigger_phrases")]
    pub(crate) trigger_hints: Vec<String>,
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
#[serde(deny_unknown_fields)]
#[cfg(test)]
pub(crate) struct RoutingEvalCasePayload {
    pub(crate) id: Option<Value>,
    pub(crate) task: String,
    pub(crate) category: String,
    #[serde(default = "default_true")]
    pub(crate) first_turn: bool,
    pub(crate) expected_owner: Option<String>,
    pub(crate) expected_overlay: Option<String>,
    pub(crate) focus_skill: Option<String>,
    #[serde(default)]
    pub(crate) forbidden_owners: Vec<String>,
    /// When set, `evaluate_routing_cases` fails if `RouteDecision.layer` differs.
    #[serde(default)]
    pub(crate) expected_layer: Option<String>,
    /// When set, must match `RouteDecision.route_context` exactly.
    #[serde(default)]
    pub(crate) route_context: Option<RouteContextPayload>,
    /// Human-only fixture commentary; ignored by eval harness.
    #[serde(default)]
    pub(crate) notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg(test)]
pub(crate) struct RoutingEvalCasesPayload {
    pub(crate) schema_version: String,
    #[serde(default)]
    pub(crate) cases: Vec<RoutingEvalCasePayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(test)]
pub(crate) struct RoutingEvalResultPayload {
    pub(crate) id: Option<Value>,
    pub(crate) category: String,
    pub(crate) task: String,
    pub(crate) focus_skill: Option<String>,
    pub(crate) selected_owner: String,
    pub(crate) selected_overlay: Option<String>,
    pub(crate) expected_owner: Option<String>,
    pub(crate) expected_overlay: Option<String>,
    pub(crate) forbidden_owners: Vec<String>,
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
pub(crate) struct EvaluatedRoutingCase {
    pub(crate) input_index: usize,
    pub(crate) result: RoutingEvalResultPayload,
}

pub(crate) struct RawSkillRecord {
    pub(crate) slug: String,
    pub(crate) skill_path: Option<String>,
    pub(crate) layer: String,
    pub(crate) owner: String,
    pub(crate) gate: String,
    pub(crate) priority: String,
    pub(crate) session_start: String,
    pub(crate) summary: String,
    pub(crate) short_description: String,
    pub(crate) when_to_use: String,
    pub(crate) do_not_use: String,
    pub(crate) tags: Vec<String>,
    pub(crate) trigger_hints: Vec<String>,
    pub(crate) host_platforms: Vec<String>,
    pub(crate) record_kind: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RouteMetadataPatch {
    pub(crate) priority: Option<String>,
    pub(crate) session_start: Option<String>,
    pub(crate) positive_triggers: Vec<String>,
    pub(crate) negative_triggers: Vec<String>,
    pub(crate) primary_allowed: Option<bool>,
    pub(crate) fallback_policy_mode: Option<String>,
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

#[derive(Debug, Clone, Copy)]
pub(crate) struct RecordRowIndexes {
    pub(crate) slug: usize,
    pub(crate) skill_path: Option<usize>,
    pub(crate) layer: usize,
    pub(crate) owner: usize,
    pub(crate) gate: usize,
    pub(crate) summary: usize,
    pub(crate) trigger_hints: usize,
    pub(crate) host_platforms: Option<usize>,
    pub(crate) record_kind: Option<usize>,
    pub(crate) priority: Option<usize>,
    pub(crate) session_start: Option<usize>,
    pub(crate) required_max: usize,
}

impl RecordRowIndexes {
    pub(crate) fn from_required(
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
