//! Skill routing: record loading, scoring, and route decisions.
#![allow(unused_imports)] // `pub use` re-exports are used outside this module.

mod aliases;
mod constants;
pub use routing_engine::fuzzy;
pub use routing_engine::scoring_config;
pub use routing_engine::text;
pub use routing_engine::types;
mod eval;
mod gate_hints;
mod metadata_tests;
mod search_regression_tests;
mod nl_route_adjustments;
mod policy;
pub mod records;
pub mod routing;
mod scoring;
pub use scoring_config::scoring_weights;
mod signal_cache;
mod signals;
mod skill_record;
pub use fuzzy::{fuzzy_fallback_score, trigram_similarity, FUZZY_FALLBACK_THRESHOLD, FUZZY_MIN_SIMILARITY};

pub use constants::{
    PROFILE_COMPILE_AUTHORITY, ROUTE_AUTHORITY, ROUTE_DECISION_SCHEMA_VERSION,
    ROUTE_POLICY_SCHEMA_VERSION, ROUTE_REPORT_SCHEMA_VERSION, ROUTE_RESOLUTION_SCHEMA_VERSION,
    ROUTE_SNAPSHOT_SCHEMA_VERSION, SEARCH_RESULTS_SCHEMA_VERSION,
};
pub use nl_route_adjustments::nl_route_signal_registry_names_json;
pub use policy::{build_route_diff_report, build_route_policy, build_route_resolution};
pub use records::load_records_cached_for_stdio_with_default_runtime_path;
// Public re-exports for browser-mcp crate
pub use routing::{build_search_results_payload, filter_record_indices_for_host, search_skills_subset};
pub use records::load_records_cached_for_stdio;
// Crate-internal re-exports
pub use records::{
    invalidate_records_cache, load_inline_records,
    load_records, load_records_from_manifest,
};
pub use routing::{
    build_route_snapshot, filter_records_for_host, literal_framework_alias_decision, route_task,
    search_skills, should_accept_manifest_fallback, should_retry_with_manifest,
};
pub use signals::{
    has_github_pr_context, has_parallel_review_candidate_context, has_paper_context,
    has_paper_prose_edit_context, has_paper_writing_context, looks_like_pasted_manuscript_prose,
};
pub use text::{read_json, tokenize_query, tokenize_route_text, value_to_string};
pub use types::{
    MatchRow, RouteContextPayload, RouteDecision, RouteDecisionSnapshotPayload,
    RouteDiffReportPayload, RouteExecutionPolicyPayload, RouteResolutionPayload,
    RouteSnapshotEnvelopePayload, RouteSnapshotRequestPayload, SearchMatchPayload,
    SearchMatchRecordPayload, SearchResultsPayload, SkillRecord,
};
pub use types::{
    EvaluatedRoutingCase, RoutingEvalCasePayload, RoutingEvalCasesPayload,
    RoutingEvalMetricsPayload, RoutingEvalReportPayload, RoutingEvalResultPayload,
};

pub use eval::{evaluate_routing_cases, load_routing_eval_cases};
