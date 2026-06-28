//! Skill routing: record loading, scoring, and route decisions.
//!
//! This module provides the core routing engine functionality, including signal
//! heuristics, scoring, NL adjustment rules, record loading with caching, and
//! the primary `route_task` / `search_skills` entry points.

mod aliases;
mod constants;
mod eval;
pub mod gate_hints;
pub(crate) mod ngram;
pub mod nl_route_adjustments;
mod policy;
pub mod records;
pub mod routing;
mod scoring;
mod signal_cache;
pub(crate) mod signals;
pub mod skill_record;

// Re-export parent-level modules for `super::` compatibility within this module.
// These are the leaf modules that were migrated to routing-engine earlier.
pub use crate::fuzzy;
pub use crate::scoring_config;
pub use crate::text;
pub use crate::types;

// ── public re-exports (API surface preserved for downstream crates) ──

pub use fuzzy::{FUZZY_MIN_SIMILARITY, fuzzy_fallback_score};
pub use scoring_config::scoring_weights;

pub use constants::{
    PROFILE_COMPILE_AUTHORITY, ROUTE_AUTHORITY, ROUTE_DECISION_SCHEMA_VERSION,
    ROUTE_POLICY_SCHEMA_VERSION, ROUTE_REPORT_SCHEMA_VERSION, ROUTE_RESOLUTION_SCHEMA_VERSION,
    ROUTE_SNAPSHOT_SCHEMA_VERSION, SEARCH_RESULTS_SCHEMA_VERSION,
};
pub use nl_route_adjustments::nl_route_signal_registry_names_json;
pub use policy::{build_route_diff_report, build_route_policy, build_route_resolution};
pub use records::load_records_cached_for_stdio_with_default_runtime_path;
// Public re-exports for browser-mcp crate
pub use records::load_records_cached_for_stdio;
pub use routing::{
    build_search_results_payload, filter_record_indices_for_host, search_skills_subset,
};
// Crate-internal re-exports
pub use records::{invalidate_records_cache, load_inline_records, load_records};
pub use routing::{
    build_route_snapshot, filter_records_for_host, literal_framework_alias_decision, route_task,
    search_skills,
};
pub use signals::{
    has_github_pr_context, has_paper_context, has_paper_prose_edit_context,
    has_paper_writing_context, has_parallel_review_candidate_context,
    looks_like_pasted_manuscript_prose,
};
pub use text::{normalize_text, read_json, tokenize_query, tokenize_route_text, value_to_string};
pub use types::{
    EvaluatedRoutingCase, RoutingEvalCasePayload, RoutingEvalCasesPayload,
    RoutingEvalMetricsPayload, RoutingEvalReportPayload, RoutingEvalResultPayload,
};
pub use types::{
    MatchRow, RouteContextPayload, RouteDecision, RouteDecisionSnapshotPayload,
    RouteDiffReportPayload, RouteExecutionPolicyPayload, RouteResolutionPayload,
    RouteSnapshotEnvelopePayload, RouteSnapshotRequestPayload, SearchMatchPayload,
    SearchMatchRecordPayload, SearchResultsPayload, SkillRecord,
};
pub use types::{RawSkillRecord, RecordRowIndexes};

pub use eval::{evaluate_routing_cases, load_routing_eval_cases};

// ── test modules (behind #[cfg(test)]) ──

// Re-exports for test modules that need access to private submodules.
// In Rust, child modules cannot path-resolve through private sibling modules,
// so test modules need these items re-exported at the parent level.
pub use aliases::{
    framework_alias_entrypoints_from_hints, has_explicit_framework_alias_call,
    has_literal_framework_alias_call, qg_checker_id_for_slug,
};
pub use records::{load_records_cached_for_stdio_resolved, load_records_from_runtime};
pub use signals::has_paper_review_judgment_context;
pub use skill_record::skill_record_from_raw;

#[cfg(test)]
mod metadata_tests;
#[cfg(test)]
mod search_regression_tests;
