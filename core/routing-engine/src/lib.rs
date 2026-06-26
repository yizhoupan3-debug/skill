#![deny(clippy::unwrap_used, clippy::expect_used)]
//! B1 routing engine: leaf modules, hook registry, and route orchestration.
//!
//! This crate provides skill routing (search, score, decide) with zero internal
//! path dependencies. Runtime-core injects host-specific behavior via `hooks::register_hooks`.

pub mod fuzzy;
pub mod hooks;
pub mod route;
pub mod runtime_watch;
pub mod scoring_config;
pub mod text;
pub mod types;

pub use fuzzy::{FUZZY_MIN_SIMILARITY, fuzzy_fallback_score};
pub use runtime_watch::{
    RoutingRuntimeWatch, RoutingTableSnapshot, default_skill_routing_runtime_path,
    routing_runtime_watch,
};
pub use scoring_config::{ScoringWeights, scoring_weights};
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
