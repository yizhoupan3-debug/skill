//! Skill routing: record loading, scoring, and route decisions.
#![allow(unused_imports)] // `pub(crate) use` re-exports are used outside this module.

mod aliases;
mod constants;
#[cfg(test)]
mod eval;
mod fuzzy;
mod gate_hints;
#[cfg(test)]
mod metadata_tests;
mod nl_route_adjustments;
mod policy;
mod records;
mod routing;
mod scoring;
mod scoring_config;
pub(crate) use scoring_config::scoring_weights;
mod signals;
mod skill_record;
mod text;
pub(crate) use fuzzy::{fuzzy_fallback_score, trigram_similarity, FUZZY_FALLBACK_THRESHOLD, FUZZY_MIN_SIMILARITY};
mod types;

pub(crate) use constants::{
    PROFILE_COMPILE_AUTHORITY, ROUTE_AUTHORITY, ROUTE_DECISION_SCHEMA_VERSION,
    ROUTE_POLICY_SCHEMA_VERSION, ROUTE_REPORT_SCHEMA_VERSION, ROUTE_RESOLUTION_SCHEMA_VERSION,
    ROUTE_SNAPSHOT_SCHEMA_VERSION, SEARCH_RESULTS_SCHEMA_VERSION,
};
pub(crate) use nl_route_adjustments::nl_route_signal_registry_names_json;
pub(crate) use policy::{build_route_diff_report, build_route_policy, build_route_resolution};
#[cfg(test)]
pub(crate) use records::load_records_cached_for_stdio_with_default_runtime_path;
pub(crate) use records::{
    invalidate_records_cache, load_inline_records,
    load_records, load_records_cached_for_stdio, load_records_from_manifest,
};
pub(crate) use routing::{
    build_route_snapshot, build_search_results_payload, filter_records_for_host,
    literal_framework_alias_decision, route_task, search_skills, should_accept_manifest_fallback,
    should_retry_with_manifest,
};
#[cfg(test)]
pub(crate) use signals::has_parallel_review_candidate_context;
pub(crate) use signals::{
    has_github_pr_context, has_paper_context, has_paper_prose_edit_context, has_paper_writing_context,
    looks_like_pasted_manuscript_prose,
};
pub(crate) use text::{read_json, tokenize_query, tokenize_route_text, value_to_string};
pub(crate) use types::{
    MatchRow, RouteContextPayload, RouteDecision, RouteDecisionSnapshotPayload,
    RouteDiffReportPayload, RouteExecutionPolicyPayload, RouteResolutionPayload,
    RouteSnapshotEnvelopePayload, RouteSnapshotRequestPayload, SearchMatchPayload,
    SearchMatchRecordPayload, SearchResultsPayload, SkillRecord,
};
#[cfg(test)]
pub(crate) use types::{
    RoutingEvalCasesPayload, RoutingEvalMetricsPayload, RoutingEvalReportPayload,
    RoutingEvalResultPayload,
};

#[cfg(test)]
pub(crate) use eval::{evaluate_routing_cases, load_routing_eval_cases};
