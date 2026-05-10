//! Primary routing entrypoints (search + `route_task`) and manifest fallback helpers.
use super::aliases::has_literal_framework_alias_call;
use super::constants::{
    NO_SKILL_SELECTED, PARALLEL_RECORD_SCAN_MIN, PROFILE_COMPILE_AUTHORITY, ROUTE_AUTHORITY,
    ROUTE_DECISION_SCHEMA_VERSION, SEARCH_RESULTS_SCHEMA_VERSION,
};
use super::scoring::{
    compact_route_reasons, layer_threshold, pick_overlay, pick_owner, reasons_class, round2,
    score_bucket, score_route_candidate,
};
use super::signals::{build_route_context, is_overlay_record};
use super::text::{common_route_stop_tokens, normalize_text, tokenize_route_text};
use super::types::{
    MatchRow, RouteContextPayload, RouteDecision, RouteDecisionSnapshotPayload, SearchMatchPayload,
    SearchMatchRecordPayload, SearchResultsPayload, SkillRecord,
};
use rayon::prelude::*;
use std::cmp::Ordering;
use std::collections::HashSet;

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
        .find(|record| has_literal_framework_alias_call(&normalized_query, record))
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
            selected_skill_path: record.skill_path.clone(),
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
            selected_skill_path: None,
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
            selected_skill_path: None,
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
    if selected.score < layer_threshold(&selected.record.layer) {
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
            selected_skill_path: None,
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
        selected_skill_path: selected.record.skill_path.clone(),
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
    let record = records
        .iter()
        .find(|record| has_literal_framework_alias_call(&normalized_query, record))?;
    let reasons =
        compact_route_reasons(&["Framework alias entrypoint matched explicitly.".to_string()]);
    Some(RouteDecision {
        decision_schema_version: ROUTE_DECISION_SCHEMA_VERSION.to_string(),
        authority: ROUTE_AUTHORITY.to_string(),
        compile_authority: PROFILE_COMPILE_AUTHORITY.to_string(),
        task: query.to_string(),
        session_id: session_id.to_string(),
        selected_skill: record.slug.clone(),
        selected_skill_path: record.skill_path.clone(),
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

pub(crate) fn should_retry_with_manifest(decision: &RouteDecision) -> bool {
    if route_decision_is_no_hit(decision) {
        return true;
    }
    if decision.score < 35.0 {
        return true;
    }
    if decision.selected_skill == "visual-review" {
        return decision.route_context.execution_protocol != "audit"
            || !visual_review_has_concrete_visual_signal(decision);
    }
    false
}

fn route_decision_is_no_hit(decision: &RouteDecision) -> bool {
    decision.score <= 0.0
        || decision.selected_skill == NO_SKILL_SELECTED
        || decision.layer == "runtime"
}

fn visual_review_has_concrete_visual_signal(decision: &RouteDecision) -> bool {
    decision.reasons.iter().any(|reason| {
        let lowered = reason.to_ascii_lowercase();
        lowered.contains("visual-review boost")
            || lowered.contains("screenshot")
            || lowered.contains("rendered")
            || lowered.contains("chart")
            || lowered.contains("ui")
            || lowered.contains("截图")
            || lowered.contains("视觉")
    })
}

#[cfg(test)]
mod should_retry_with_manifest_tests {
    use super::*;

    fn make_decision(skill: &str, score: f64, layer: &str, protocol: &str) -> RouteDecision {
        RouteDecision {
            decision_schema_version: ROUTE_DECISION_SCHEMA_VERSION.to_string(),
            authority: ROUTE_AUTHORITY.to_string(),
            compile_authority: PROFILE_COMPILE_AUTHORITY.to_string(),
            task: "test".to_string(),
            session_id: "test-session".to_string(),
            selected_skill: skill.to_string(),
            selected_skill_path: None,
            overlay_skill: None,
            route_context: RouteContextPayload {
                execution_protocol: protocol.to_string(),
                verification_required: true,
                evidence_required: true,
                supervisor_required: false,
                delegation_candidate: false,
                continue_safe_local_steps: false,
                route_reason: "test".to_string(),
            },
            layer: layer.to_string(),
            score,
            reasons: Vec::new(),
            route_snapshot: RouteDecisionSnapshotPayload {
                engine: "rust".to_string(),
                selected_skill: skill.to_string(),
                overlay_skill: None,
                layer: layer.to_string(),
                score,
                score_bucket: String::new(),
                reasons: Vec::new(),
                reasons_class: String::new(),
            },
        }
    }

    #[test]
    fn low_score_triggers_retry() {
        let decision = make_decision("doc", 20.0, "L1", "four_step");
        assert!(should_retry_with_manifest(&decision));
    }

    #[test]
    fn high_score_owner_does_not_retry() {
        let decision = make_decision("doc", 60.0, "L1", "four_step");
        assert!(!should_retry_with_manifest(&decision));
    }

    #[test]
    fn boundary_score_at_threshold_does_not_retry() {
        let decision = make_decision("doc", 35.0, "L1", "four_step");
        assert!(!should_retry_with_manifest(&decision));
    }

    #[test]
    fn no_hit_skill_triggers_retry_even_with_high_score() {
        let mut decision = make_decision(NO_SKILL_SELECTED, 100.0, "L1", "four_step");
        decision.route_snapshot.selected_skill = NO_SKILL_SELECTED.to_string();
        assert!(should_retry_with_manifest(&decision));
    }

    #[test]
    fn runtime_layer_triggers_retry_even_with_high_score() {
        let decision = make_decision("doc", 80.0, "runtime", "four_step");
        assert!(should_retry_with_manifest(&decision));
    }

    #[test]
    fn zero_score_triggers_retry() {
        let decision = make_decision("doc", 0.0, "L1", "four_step");
        assert!(should_retry_with_manifest(&decision));
    }

    #[test]
    fn visual_review_non_audit_triggers_retry_even_with_high_score() {
        let decision = make_decision("visual-review", 90.0, "L1", "four_step");
        assert!(should_retry_with_manifest(&decision));
    }

    #[test]
    fn visual_review_audit_does_not_retry_when_score_is_high() {
        let mut decision = make_decision("visual-review", 90.0, "L1", "audit");
        decision
            .reasons
            .push("Visual-review boost applied: visible UI evidence and concrete visual findings requested.".to_string());
        assert!(!should_retry_with_manifest(&decision));
    }

    #[test]
    fn visual_review_audit_still_retries_when_score_is_low() {
        let decision = make_decision("visual-review", 20.0, "L1", "audit");
        assert!(should_retry_with_manifest(&decision));
    }

    #[test]
    fn visual_review_audit_retries_without_concrete_visual_signal() {
        let mut decision = make_decision("visual-review", 90.0, "L1", "audit");
        decision
            .reasons
            .push("Trigger hint matched: review.".to_string());
        assert!(should_retry_with_manifest(&decision));
    }

    #[test]
    fn systematic_debugging_high_score_does_not_retry() {
        let decision = make_decision("systematic-debugging", 60.0, "L1", "four_step");
        assert!(!should_retry_with_manifest(&decision));
    }

    #[test]
    fn systematic_debugging_low_score_retries_via_threshold() {
        let decision = make_decision("systematic-debugging", 30.0, "L1", "four_step");
        assert!(should_retry_with_manifest(&decision));
    }
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
    runtime_records: &[SkillRecord],
    should_retry: bool,
    explicit_manifest: bool,
) -> bool {
    if runtime_gate_blocks_manifest_owner(hot_decision, full_decision, runtime_records) {
        return false;
    }

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

fn runtime_gate_blocks_manifest_owner(
    hot_decision: &RouteDecision,
    full_decision: &RouteDecision,
    runtime_records: &[SkillRecord],
) -> bool {
    if route_decision_is_no_hit(hot_decision)
        || hot_decision.selected_skill == full_decision.selected_skill
        || full_decision
            .reasons
            .iter()
            .any(|reason| reason.contains("Framework alias entrypoint matched explicitly"))
    {
        return false;
    }

    if hot_decision.selected_skill == "visual-review"
        && full_decision.selected_skill == "screenshot"
        && hot_decision.route_context.execution_protocol != "audit"
    {
        return false;
    }

    if full_decision.selected_skill == "skill-framework-developer"
        && full_decision.score > hot_decision.score
        && has_non_generic_manifest_signal(full_decision)
    {
        return false;
    }

    is_runtime_required_gate(&hot_decision.selected_skill, runtime_records)
}

fn is_runtime_required_gate(slug: &str, runtime_records: &[SkillRecord]) -> bool {
    runtime_records
        .iter()
        .find(|record| record.slug == slug)
        .is_some_and(|record| {
            record.session_start_lower == "required"
                && (record.owner_lower == "gate" || record.gate_lower != "none")
        })
}
