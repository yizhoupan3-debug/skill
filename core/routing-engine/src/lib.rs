//! B1 routing engine leaf modules: types, text normalization, fuzzy match, scoring weights.
//!
//! Orchestration (`route::routing`, signals, records) remains in `router-rs` until later phases.

pub mod fuzzy;
pub mod runtime_watch;
pub mod scoring_config;
pub mod text;
pub mod types;

pub use fuzzy::{
    fuzzy_fallback_score, trigram_similarity, FUZZY_FALLBACK_THRESHOLD, FUZZY_MIN_SIMILARITY,
};
pub use runtime_watch::{
    default_skill_routing_runtime_path, routing_runtime_watch, RoutingRuntimeWatch,
    RoutingTableSnapshot,
};
pub use scoring_config::{scoring_weights, ScoringWeights};
pub use text::{
    common_route_stop_tokens, read_json, tokenize_query, tokenize_route_text, value_to_string,
};
pub use types::{
    InlineSkillRecordPayload, MatchRow, RawSkillRecord, RecordRowIndexes, RecordsCacheEntry,
    RecordsCacheKey, RecordsCacheState, RouteCandidate, RouteContextPayload, RouteDecision,
    RouteDecisionSnapshotPayload, RouteDiffReportPayload, RouteExecutionPolicyPayload,
    RouteMetadataPatch, RouteResolutionPayload, RouteSnapshotEnvelopePayload,
    RouteSnapshotRequestPayload, SearchMatchPayload, SearchMatchRecordPayload,
    SearchResultsPayload, SkillRecord,
};
