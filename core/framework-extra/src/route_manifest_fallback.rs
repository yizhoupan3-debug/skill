//! Simplified routing entrypoint (formerly route-with-SKILL_MANIFEST-fallback).
//!
//! `route_task_with_manifest_fallback` is a routing-engine-based entrypoint
//! that filters records for host, checks for a literal framework alias decision,
//! and runs `route_task` on the filtered records. All manifest fallback logic
//! has been removed — `SKILL_ROUTING_RUNTIME.json` is the single source of truth.

use routing_engine::route::{
    RouteDecision, SkillRecord, filter_records_for_host, literal_framework_alias_decision,
    normalize_text, route_task, tokenize_route_text,
};
use runtime_infra::telemetry_emit;

pub fn route_task_with_manifest_fallback(
    runtime_records: &[SkillRecord],
    host_id: Option<&str>,
    query: &str,
    session_id: &str,
    allow_overlay: bool,
    first_turn: bool,
) -> Result<RouteDecision, String> {
    let scoped_runtime = filter_records_for_host(runtime_records, host_id)?;
    let normalized = normalize_text(query);
    let tokens = tokenize_route_text(query);
    if let Some(decision) = literal_framework_alias_decision(&scoped_runtime, query, &normalized, &tokens, session_id) {
        telemetry_emit::emit_route_decision(query, &decision, false, 0, "");
        return Ok(decision);
    }
    let decision = route_task(&scoped_runtime, query, session_id, allow_overlay, first_turn)?;
    telemetry_emit::emit_route_decision(query, &decision, false, 0, "");
    Ok(decision)
}
