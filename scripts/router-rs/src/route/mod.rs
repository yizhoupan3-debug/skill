//! Skill routing: record loading, scoring, and route decisions.
#![allow(unused_imports)] // `pub(crate) use` re-exports are used outside this module.

mod aliases;
mod constants;
#[cfg(test)]
mod eval;
mod gate_hints;
#[cfg(test)]
mod metadata_tests;
mod policy;
mod records;
mod routing;
mod scoring;
mod signals;
mod skill_record;
mod text;
mod types;

pub(crate) use constants::{
    PROFILE_COMPILE_AUTHORITY, ROUTE_AUTHORITY, ROUTE_DECISION_SCHEMA_VERSION,
    ROUTE_POLICY_SCHEMA_VERSION, ROUTE_REPORT_SCHEMA_VERSION, ROUTE_RESOLUTION_SCHEMA_VERSION,
    ROUTE_SNAPSHOT_SCHEMA_VERSION, SEARCH_RESULTS_SCHEMA_VERSION,
};
pub(crate) use policy::{build_route_diff_report, build_route_policy, build_route_resolution};
#[cfg(test)]
pub(crate) use records::load_records_cached_for_stdio_with_default_runtime_path;
pub(crate) use records::{
    load_inline_records, load_records, load_records_cached_for_stdio, load_records_from_manifest,
};
pub(crate) use routing::{
    build_route_snapshot, build_search_results_payload, literal_framework_alias_decision,
    route_task, search_skills, should_accept_manifest_fallback, should_retry_with_manifest,
};
#[cfg(test)]
pub(crate) use signals::has_parallel_review_candidate_context;
pub(crate) use signals::{has_github_pr_context, has_paper_context};
pub(crate) use text::{read_json, tokenize_query, value_to_string};
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
