//! Candidate scoring and owner/overlay selection.
use super::aliases::{framework_alias_requires_explicit_call, has_explicit_framework_alias_call};
use super::signals::*;
use super::text::{
    common_route_stop_tokens, normalize_text, text_matches_phrase, tokenize_route_text,
};
use super::types::{RouteCandidate, SkillRecord};
use crate::hook_common::is_review_prompt;
use std::cmp::Ordering;
use std::collections::HashSet;

pub(crate) fn score_route_candidate<'a>(
    record: &'a SkillRecord,
    query_text: &'a str,
    query_token_list: &'a [String],
    query_tokens: &'a HashSet<String>,
    first_turn: bool,
) -> RouteCandidate<'a> {
    let mut score = 0.0f64;
    let mut reasons = Vec::new();

    if let Some(done) = super::nl_route_adjustments::apply_nl_pre_framework_alias_rules(
        record,
        query_text,
        query_token_list,
        query_tokens,
        first_turn,
        &mut score,
        &mut reasons,
    ) {
        return done;
    }
    let _checklist_execution_context = has_checklist_execution_context(query_text);
    let bounded_subagent_context = has_bounded_subagent_context(query_text, query_token_list);
    let token_budget_pressure = has_token_budget_pressure(query_text, query_token_list);
    let team_orchestration_context = has_team_orchestration_context(query_text, query_token_list)
        && !has_team_negation_context(query_text, query_token_list);
    let explicit_framework_alias = framework_alias_requires_explicit_call(record)
        && has_explicit_framework_alias_call(query_text, query_token_list, record);
    let parallel_execution_context = has_parallel_execution_context(query_text, query_token_list);
    if record.slug == "agent-swarm-orchestration"
        && (bounded_subagent_context
            || team_orchestration_context
            || has_parallel_review_candidate_context(query_text, query_token_list)
            || parallel_execution_context)
    {
        score += 60.0;
        reasons.push(
            "Agent-swarm boost applied: multi-agent delegation or worker orchestration wording detected."
                .to_string(),
        );
        if parallel_execution_context {
            score += 12.0;
            reasons.push(
                "Parallel-execution boost applied: independent lanes can run as bounded sidecars."
                    .to_string(),
            );
        }
        if has_parallel_review_candidate_context(query_text, query_token_list) {
            score += 10.0;
            reasons.push(
                "Parallel-review boost applied: broad review scope should run subagent admission before a single-lane review."
                    .to_string(),
            );
        }
        if bounded_subagent_context && token_budget_pressure {
            score += 8.0;
            reasons.push(
                "Token-budget boost applied: bounded sidecars fit prompt-budget pressure better than wider orchestration."
                    .to_string(),
            );
        }
    }
    if framework_alias_requires_explicit_call(record) && !explicit_framework_alias {
        return RouteCandidate {
            record,
            score: 0.0,
            reasons: vec![
                "Suppressed: framework alias skills only route from explicit /alias or $alias entrypoints."
                    .to_string(),
            ],
        };
    }
    if let Some(done) = super::nl_route_adjustments::apply_nl_post_framework_alias_rules(
        record,
        query_text,
        query_token_list,
        query_tokens,
        first_turn,
        &mut score,
        &mut reasons,
    ) {
        return done;
    }
    let design_output_audit_context = has_design_output_audit_context(query_text, query_token_list);
    let design_workflow_protocol_context =
        has_design_workflow_protocol_context(query_text, query_token_list);
    if record.slug == "design-md"
        && has_design_contract_context(query_text, query_token_list)
        && !design_output_audit_context
        && !design_workflow_protocol_context
    {
        if has_quick_artifact_context(query_text, query_token_list) {
            score *= 0.65;
            reasons.push(
                "Design-md quick-task suppression applied: one-off artifact wording should not force a design contract."
                    .to_string(),
            );
        } else {
            score += 42.0;
            reasons.push(
                "Design-md boost applied: reusable visual contract or design-token wording detected."
                    .to_string(),
            );
        }
    }
    if explicit_framework_alias {
        score += 1000.0;
        reasons.push("Framework alias entrypoint matched explicitly.".to_string());
    }

    if !record.slug_lower.is_empty()
        && (text_matches_phrase(query_token_list, &record.slug_lower)
            || query_text.contains(&format!("${}", record.slug_lower)))
    {
        score += 100.0;
        reasons.push(format!("Exact skill name matched: {}.", record.slug));
    }

    let matched_gates = record
        .gate_phrases
        .iter()
        .filter(|phrase| text_matches_phrase(query_token_list, phrase))
        .cloned()
        .collect::<Vec<_>>();
    if !matched_gates.is_empty() {
        score += 18.0 + i32::min(12, ((matched_gates.len() - 1) as i32) * 6) as f64;
        reasons.push(format!(
            "Routing gate matched: {}.",
            matched_gates.join(", ")
        ));
    }

    let mut shared_name_tokens = record
        .name_tokens
        .iter()
        .filter(|token| query_tokens.contains(*token))
        .cloned()
        .collect::<Vec<_>>();
    shared_name_tokens.sort();
    if !shared_name_tokens.is_empty() {
        score += 14.0 + (shared_name_tokens.len() as f64) * 4.0;
        reasons.push(format!(
            "Name tokens matched: {}.",
            shared_name_tokens.join(", ")
        ));
    }

    let matched_trigger_hints = record
        .trigger_hints
        .iter()
        .filter(|phrase| {
            phrase.chars().count() >= 2
                && !common_route_stop_tokens().contains(&phrase.as_str())
                && text_matches_phrase(query_token_list, phrase)
        })
        .cloned()
        .collect::<Vec<_>>();
    if !matched_trigger_hints.is_empty() {
        score += (matched_trigger_hints.len() as f64) * 20.0;
        reasons.push(format!(
            "Trigger hint matched: {}.",
            matched_trigger_hints.join(", ")
        ));
    }

    let matched_metadata_triggers = record
        .metadata_positive_triggers
        .iter()
        .filter(|phrase| {
            phrase.chars().count() >= 2
                && !common_route_stop_tokens().contains(&phrase.as_str())
                && text_matches_phrase(query_token_list, phrase)
        })
        .cloned()
        .collect::<Vec<_>>();
    if !matched_metadata_triggers.is_empty() {
        score += (matched_metadata_triggers.len() as f64) * 8.0;
        reasons.push(format!(
            "Routing metadata positive trigger matched: {}.",
            matched_metadata_triggers.join(", ")
        ));
    }

    let mut shared_keywords = record
        .keyword_tokens
        .iter()
        .filter(|token| query_tokens.contains(*token))
        .cloned()
        .collect::<Vec<_>>();
    shared_keywords.sort();
    if !shared_keywords.is_empty() {
        score += f64::min(24.0, (shared_keywords.len() as f64) * 3.0);
        reasons.push(format!(
            "Description keywords matched: {}.",
            shared_keywords
                .iter()
                .take(8)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    let mut alias_hits = record
        .alias_tokens
        .iter()
        .filter(|token| query_tokens.contains(*token))
        .cloned()
        .collect::<Vec<_>>();
    alias_hits.sort();
    if !alias_hits.is_empty() {
        score += 12.0 + (alias_hits.len() as f64) * 4.0;
        reasons.push(format!(
            "Skill alias hints matched: {}.",
            alias_hits
                .iter()
                .take(8)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    if first_turn && score > 0.0 {
        if record.session_start_lower == "required" {
            score += 8.0;
            reasons.push("Session-start required boost applied (+8).".to_string());
        } else if record.session_start_lower == "preferred" {
            score += 3.0;
            reasons.push("Session-start preferred boost applied (+3).".to_string());
        }
    }

    if record.slug == "code-review-deep"
        && first_turn
        && is_review_prompt(query_text)
        && !has_paper_context(query_text, query_token_list)
    {
        score += 22.0;
        reasons.push(
            "Code-review-deep boost applied: review-class prompt without paper-only context."
                .to_string(),
        );
    }

    if record.owner_lower == "gate" && score > 0.0 {
        score += 2.0;
    }

    let visual_evidence_review_context =
        has_visual_evidence_review_context(query_text, query_token_list);
    let redesign_context = text_matches_phrase(query_token_list, "重新梳理")
        || text_matches_phrase(query_token_list, "改版")
        || text_matches_phrase(query_token_list, "redesign");

    if record.slug == "visual-review"
        && first_turn
        && visual_evidence_review_context
        && !redesign_context
    {
        score += 36.0;
        reasons.push(
            "Visual-review boost applied: visible UI evidence and concrete visual findings requested."
                .to_string(),
        );
    }

    if record.slug == "visual-review" && score > 0.0 {
        let visual_evidence_markers = [
            "看图",
            "截图",
            "渲染",
            "render",
            "screenshot",
            "ui",
            "layout",
            "chart",
            "视觉",
        ];
        if !visual_evidence_markers
            .iter()
            .any(|marker| query_text.contains(marker))
        {
            return RouteCandidate {
                record,
                score: 0.0,
                reasons: vec![
                    "Suppressed: visual-review requires visible evidence, not a generic review token."
                        .to_string(),
                ],
            };
        }
    }

    if !record.do_not_use_tokens.is_empty() && score > 0.0 {
        let negative_hits = record
            .do_not_use_tokens
            .iter()
            .filter(|token| query_tokens.contains(*token))
            .cloned()
            .collect::<Vec<_>>();
        if !negative_hits.is_empty() {
            let penalty = f64::min(score * 0.3, (negative_hits.len() as f64) * 5.0);
            score = f64::max(0.0, score - penalty);
            reasons.push(format!(
                "Do-not-use penalty applied: {}.",
                negative_hits
                    .into_iter()
                    .take(5)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    if record.slug == "paper-workbench"
        && has_paper_review_revision_intent(query_text, query_token_list)
    {
        score += 28.0;
        reasons.push(
            "Paper workbench boost applied: review-driven manuscript revision intent detected."
                .to_string(),
        );
    }

    if is_overlay_record(record) && score > 0.0 {
        score *= 0.15;
        reasons.push(format!(
            "Owner suppression applied: {} is overlay-only.",
            record.slug
        ));
    }
    RouteCandidate {
        record,
        score,
        reasons,
    }
}

pub(crate) fn pick_owner<'a>(
    candidates: Vec<RouteCandidate<'a>>,
    query_text: &str,
    query_token_list: &[String],
) -> RouteCandidate<'a> {
    let mut owner_candidates = candidates
        .iter()
        .filter(|candidate| can_be_primary_owner(candidate.record))
        .cloned()
        .collect::<Vec<_>>();
    owner_candidates.sort_unstable_by(route_candidate_cmp);
    let top_owner_score = owner_candidates
        .first()
        .map(|candidate| candidate.score)
        .unwrap_or(f64::NEG_INFINITY);
    let top_gate = candidates
        .iter()
        .filter(|candidate| {
            candidate.record.owner_lower == "gate" || candidate.record.gate_lower != "none"
        })
        .min_by(|left, right| route_candidate_cmp(left, right))
        .cloned();
    if let Some(mut top_gate) = top_gate
        .as_ref()
        .filter(|candidate| {
            candidate.record.slug == "agent-swarm-orchestration"
                && candidate.score >= 60.0
                && !has_plan_mode_owner_context(query_text, query_token_list)
        })
        .cloned()
    {
        top_gate.reasons.push(
            "Prioritized delegation gate before strong owner for broad parallel-review admission."
                .to_string(),
        );
        return top_gate;
    }
    if let Some(top_owner) = owner_candidates.first() {
        if top_owner.score >= 60.0 {
            return top_owner.clone();
        }
    }
    if let Some(mut top_gate) =
        top_gate.filter(|candidate| candidate.score >= 30.0 && candidate.score >= top_owner_score)
    {
        top_gate
            .reasons
            .push("Prioritized via gate-before-owner precedence.".to_string());
        return top_gate;
    }
    let owner_pool = if owner_candidates.is_empty() {
        candidates
            .iter()
            .filter(|candidate| !is_overlay_record(candidate.record))
            .cloned()
            .collect::<Vec<_>>()
    } else {
        owner_candidates.clone()
    };
    let owner_pool = if owner_pool.is_empty() {
        candidates
            .iter()
            .filter(|candidate| can_be_fallback_owner(candidate.record))
            .cloned()
            .collect::<Vec<_>>()
    } else {
        owner_pool
    };
    let owner_pool = if owner_pool.is_empty() {
        candidates.clone()
    } else {
        owner_pool
    };

    let mut layers = owner_pool
        .iter()
        .map(|candidate| candidate.record.layer.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    layers.sort_unstable_by_key(|layer| layer_rank(layer));

    for layer in layers {
        let mut layer_candidates = owner_pool
            .iter()
            .filter(|candidate| candidate.record.layer == layer)
            .cloned()
            .collect::<Vec<_>>();
        layer_candidates.sort_unstable_by(route_candidate_cmp);
        if let Some(top) = layer_candidates.first().cloned() {
            if top.score >= layer_threshold(&layer) {
                return top;
            }
        }
    }

    let mut fallback_pool = owner_pool;
    fallback_pool.sort_unstable_by(|left, right| {
        layer_rank(&left.record.layer)
            .cmp(&layer_rank(&right.record.layer))
            .then_with(|| {
                finite_route_score(right.score)
                    .partial_cmp(&finite_route_score(left.score))
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| {
                priority_rank(&left.record.priority).cmp(&priority_rank(&right.record.priority))
            })
            .then_with(|| left.record.slug.cmp(&right.record.slug))
    });
    fallback_pool.remove(0)
}

pub(crate) fn route_candidate_cmp(
    left: &RouteCandidate<'_>,
    right: &RouteCandidate<'_>,
) -> Ordering {
    let left_s = finite_route_score(left.score);
    let right_s = finite_route_score(right.score);
    right_s
        .partial_cmp(&left_s)
        .unwrap_or(Ordering::Equal)
        .then_with(|| {
            priority_rank(&left.record.priority).cmp(&priority_rank(&right.record.priority))
        })
        .then_with(|| left.record.slug.cmp(&right.record.slug))
}

fn finite_route_score(score: f64) -> f64 {
    if score.is_nan() {
        return f64::NEG_INFINITY;
    }
    if score.is_infinite() {
        return if score.is_sign_positive() {
            f64::MAX
        } else {
            f64::NEG_INFINITY
        };
    }
    score
}

pub(crate) fn pick_overlay(
    records: &[SkillRecord],
    query_text: &str,
    query_tokens: &[String],
    selected_skill: &SkillRecord,
) -> Option<String> {
    if selected_skill.slug == "skill-framework-developer"
        && has_framework_review_overlay_context(query_text, query_tokens)
        && records
            .iter()
            .any(|record| record.slug == "code-review-deep")
    {
        return Some("code-review-deep".to_string());
    }

    let mut ordered = records.iter().collect::<Vec<_>>();
    ordered.sort_unstable_by(|left, right| {
        layer_rank(&left.layer)
            .cmp(&layer_rank(&right.layer))
            .then_with(|| priority_rank(&left.priority).cmp(&priority_rank(&right.priority)))
            .then_with(|| left.slug.cmp(&right.slug))
    });

    for record in ordered {
        if record.slug == selected_skill.slug {
            continue;
        }
        if !is_overlay_record(record) {
            continue;
        }
        let explicit_name_match = text_matches_phrase(query_tokens, &record.slug_lower);
        let explicit_trigger_match = record
            .trigger_hints
            .iter()
            .any(|phrase| phrase.chars().count() > 3 && text_matches_phrase(query_tokens, phrase));
        if explicit_name_match || explicit_trigger_match {
            return Some(record.slug.clone());
        }
    }

    None
}

fn has_framework_review_overlay_context(query_text: &str, query_tokens: &[String]) -> bool {
    let framework_surface = [
        "harness",
        "路由",
        "router",
        "routing",
        "hook",
        "hooks",
        "framework",
        "runtime",
    ]
    .iter()
    .any(|marker| query_text.contains(marker) || text_matches_phrase(query_tokens, marker));
    let review_surface = [
        "深度 review",
        "深度review",
        "深底 review",
        "深底review",
        "deep review",
        "code review",
        "审计",
    ]
    .iter()
    .any(|marker| query_text.contains(marker) || text_matches_phrase(query_tokens, marker));
    framework_surface && review_surface
}

pub(crate) fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

pub(crate) fn score_bucket(score: f64) -> String {
    let floor = ((score.max(0.0) / 10.0).floor() as i32) * 10;
    format!("{floor:02}-{ceiling:02}", ceiling = floor + 9)
}

pub(crate) fn compact_route_reasons(reasons: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut compact = Vec::new();
    for reason in reasons {
        let normalized = normalize_text(reason);
        if normalized.is_empty() || !seen.insert(normalized) {
            continue;
        }
        compact.push(reason.clone());
        if compact.len() >= 6 {
            break;
        }
    }
    compact
}

pub(crate) fn reasons_class(reasons: &[String]) -> String {
    let mut normalized = reasons
        .iter()
        .map(|reason| normalize_text(reason))
        .filter(|reason| !reason.is_empty())
        .collect::<Vec<_>>();
    if normalized.is_empty() {
        return "none".to_string();
    }
    normalized.sort();
    normalized.dedup();
    normalized.join("|")
}

pub(crate) fn layer_rank(layer: &str) -> i32 {
    match layer {
        "L-1" => -1,
        "L0" => 0,
        "L1" => 1,
        "L2" => 2,
        "L3" => 3,
        "L4" => 4,
        _ => 99,
    }
}

pub(crate) fn priority_rank(priority: &str) -> i32 {
    match priority {
        "P0" => 0,
        "P1" => 1,
        "P2" => 2,
        "P3" => 3,
        _ => 99,
    }
}

pub(crate) fn layer_threshold(layer: &str) -> f64 {
    match layer {
        "L0" => 18.0,
        "L1" => 16.0,
        "L2" | "L3" => 14.0,
        _ => 15.0,
    }
}

#[cfg(test)]
mod framework_review_overlay_typo_tests {
    use super::has_framework_review_overlay_context;
    use super::tokenize_route_text;

    #[test]
    fn shendi_typo_with_routing_matches_overlay_context() {
        let q = "深底review 路由系统";
        let tokens = tokenize_route_text(q);
        assert!(has_framework_review_overlay_context(q, &tokens));
    }

    #[test]
    fn shendi_typo_spaced_review_with_hook_matches_overlay_context() {
        let q = "深底 review hooks 是否合理";
        let tokens = tokenize_route_text(q);
        assert!(has_framework_review_overlay_context(q, &tokens));
    }

    #[test]
    fn skill_only_with_code_review_does_not_match_overlay_context() {
        let q = "skill packaging code review only";
        let tokens = tokenize_route_text(q);
        assert!(
            !has_framework_review_overlay_context(q, &tokens),
            "`skill` keyword alone must not imply framework-overlay surface without routing/harness/hook cues"
        );
    }
}
