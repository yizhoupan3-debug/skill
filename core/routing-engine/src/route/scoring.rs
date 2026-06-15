//! Candidate scoring and owner/overlay selection.
use super::aliases::{framework_alias_requires_explicit_call, has_explicit_framework_alias_call};
use super::scoring_config::ScoringWeights;
use super::signal_cache::cached_signal;
use super::signals::*;
use tracing::debug;
use super::text::{
    common_route_stop_tokens, normalize_text, text_matches_phrase, tokenize_route_text,
};
use super::types::{RouteCandidate, SkillRecord};
use crate::hooks::is_review_prompt;
use std::cmp::Ordering;
use std::collections::HashSet;

/// Score agent-swarm related signals. Returns `(delta, reasons)`.
#[inline]
fn score_agent_swarm_signals(
    record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    w: &ScoringWeights,
    bounded_subagent_context: bool,
    workflow_orchestration_context: bool,
    parallel_execution_context: bool,
    token_budget_pressure: bool,
) -> (f64, Vec<String>) {
    if record.slug != "agent-swarm-orchestration" {
        return (0.0, Vec::new());
    }
    if !(bounded_subagent_context
        || workflow_orchestration_context
        || has_parallel_review_candidate_context(query_text, query_token_list)
        || parallel_execution_context)
    {
        return (0.0, Vec::new());
    }
    let mut delta = w.agent_swarm_boost;
    let mut reasons = Vec::with_capacity(4);
    reasons.push(
        "Agent-swarm boost applied: multi-agent delegation or worker orchestration wording detected."
            .to_string(),
    );
    if parallel_execution_context {
        delta += w.parallel_execution_boost;
        reasons.push(
            "Parallel-execution boost applied: independent lanes can run as bounded sidecars."
                .to_string(),
        );
    }
    if has_parallel_review_candidate_context(query_text, query_token_list) {
        delta += w.parallel_review_boost;
        reasons.push(
            "Parallel-review boost applied: broad review scope should run subagent admission before a single-lane review."
                .to_string(),
        );
    }
    if bounded_subagent_context && token_budget_pressure {
        delta += w.token_budget_boost;
        reasons.push(
            "Token-budget boost applied: bounded sidecars fit prompt-budget pressure better than wider orchestration."
                .to_string(),
        );
    }
    (delta, reasons)
}

/// Check framework-alias suppression. Returns `Some(candidate)` for early return, `None` otherwise.
#[inline]
fn check_framework_alias_suppression<'a>(
    record: &'a SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    explicit_framework_alias: bool,
) -> Option<RouteCandidate<'a>> {
    if framework_alias_requires_explicit_call(record) && !explicit_framework_alias {
        let ci_gate_nl_routing =
            record.slug == "gh-fix-ci" && should_route_to_gh_fix_ci(query_text, query_token_list);
        if !ci_gate_nl_routing {
            return Some(RouteCandidate {
                record,
                score: 0.0,
                reasons: vec![
                    "Suppressed: framework alias skills only route from explicit /alias or $alias entrypoints."
                        .to_string(),
                ],
                matched_token_count: 0,
            });
        }
    }
    None
}

/// Score design-md signals. Returns `(delta, reasons)`.
#[inline]
fn score_design_md_signals(
    record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    current_score: f64,
    w: &ScoringWeights,
) -> (f64, Vec<String>) {
    if record.slug != "design-md" {
        return (0.0, Vec::new());
    }
    if !has_design_contract_context(query_text, query_token_list) {
        return (0.0, Vec::new());
    }
    if has_design_output_audit_context(query_text, query_token_list)
        || has_design_workflow_protocol_context(query_text, query_token_list)
    {
        return (0.0, Vec::new());
    }
    let mut reasons = Vec::with_capacity(1);
    if has_quick_artifact_context(query_text, query_token_list) {
        let new_score = current_score * w.design_md_quick_suppression_factor;
        reasons.push(
            "Design-md quick-task suppression applied: one-off artifact wording should not force a design contract."
                .to_string(),
        );
        (new_score - current_score, reasons)
    } else {
        reasons.push(
            "Design-md boost applied: reusable visual contract or design-token wording detected."
                .to_string(),
        );
        (w.design_md_boost, reasons)
    }
}

/// Score gate phrases, exact skill name, name tokens, and trigger hints.
/// Returns `(delta, reasons, matched_query_tokens)`.
#[inline]
fn score_gate_name_token_signals(
    record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    query_tokens: &HashSet<String>,
    w: &ScoringWeights,
) -> (f64, Vec<String>, HashSet<String>) {
    let mut delta = 0.0f64;
    let mut reasons = Vec::with_capacity(4);
    let mut matched_query_tokens: HashSet<String> = HashSet::new();

    // Exact skill name
    if !record.slug_lower.is_empty()
        && (text_matches_phrase(query_token_list, &record.slug_lower)
            || query_text.contains(&format!("${}", record.slug_lower)))
    {
        delta += w.exact_skill_name_boost;
        reasons.push(format!("Exact skill name matched: {}.", record.slug));
        for slug_tok in tokenize_route_text(&record.slug_lower) {
            if query_tokens.contains(slug_tok.as_str()) {
                matched_query_tokens.insert(slug_tok.clone());
            }
        }
    }

    // Gate phrases
    let matched_gates: Vec<&str> = record
        .gate_phrases
        .iter()
        .filter(|phrase| text_matches_phrase(query_token_list, phrase))
        .map(|s| s.as_str())
        .collect();
    if !matched_gates.is_empty() {
        delta += w.gate_match_base
            + i32::min(
                w.gate_match_max_extra,
                ((matched_gates.len() - 1) as i32) * w.gate_match_per_additional,
            ) as f64;
        reasons.push(format!(
            "Routing gate matched: {}.",
            matched_gates.join(", ")
        ));
        for phrase in &matched_gates {
            let ptokens = tokenize_route_text(phrase);
            if ptokens.len() == 1 {
                for t in query_token_list {
                    if text_matches_phrase(std::slice::from_ref(t), phrase) {
                        matched_query_tokens.insert(t.clone());
                    }
                }
            }
        }
    }

    // Name tokens
    let mut shared_name_tokens = record
        .name_tokens
        .iter()
        .filter(|token| query_tokens.contains(*token))
        .cloned()
        .collect::<Vec<_>>();
    shared_name_tokens.sort();
    if !shared_name_tokens.is_empty() {
        delta += w.name_tokens_base + (shared_name_tokens.len() as f64) * w.name_tokens_per_token;
        reasons.push(format!(
            "Name tokens matched: {}.",
            shared_name_tokens.join(", ")
        ));
        for tok in &shared_name_tokens {
            matched_query_tokens.insert(tok.clone());
        }
    }

    // Trigger hints
    let matched_trigger_hints: Vec<&str> = record
        .trigger_hints
        .iter()
        .filter(|phrase| {
            phrase.chars().count() >= 2
                && !common_route_stop_tokens().contains(&phrase.as_str())
                && text_matches_phrase(query_token_list, phrase)
        })
        .map(|s| s.as_str())
        .collect();
    if !matched_trigger_hints.is_empty() {
        delta += (matched_trigger_hints.len() as f64) * w.trigger_hint_per_match;
        reasons.push(format!(
            "Trigger hint matched: {}.",
            matched_trigger_hints.join(", ")
        ));
        for phrase in &matched_trigger_hints {
            let ptokens = tokenize_route_text(phrase);
            if ptokens.len() == 1 {
                for t in query_token_list {
                    if text_matches_phrase(std::slice::from_ref(t), phrase) {
                        matched_query_tokens.insert(t.clone());
                    }
                }
            }
        }
    }

    (delta, reasons, matched_query_tokens)
}

/// Score metadata positive triggers, keyword tokens, and alias tokens.
/// Returns `(delta, reasons, matched_query_tokens)`.
#[inline]
fn score_metadata_trigger_signals(
    record: &SkillRecord,
    query_tokens: &HashSet<String>,
    query_token_list: &[String],
    w: &ScoringWeights,
) -> (f64, Vec<String>, HashSet<String>) {
    let mut delta = 0.0f64;
    let mut reasons = Vec::with_capacity(3);
    let mut matched_query_tokens: HashSet<String> = HashSet::new();

    // Metadata positive triggers
    let matched_metadata_triggers: Vec<&str> = record
        .metadata_positive_triggers
        .iter()
        .filter(|phrase| {
            phrase.chars().count() >= 2
                && !common_route_stop_tokens().contains(&phrase.as_str())
                && text_matches_phrase(query_token_list, phrase)
        })
        .map(|s| s.as_str())
        .collect();
    if !matched_metadata_triggers.is_empty() {
        delta += (matched_metadata_triggers.len() as f64) * w.metadata_trigger_per_match;
        reasons.push(format!(
            "Routing metadata positive trigger matched: {}.",
            matched_metadata_triggers.join(", ")
        ));
        for phrase in &matched_metadata_triggers {
            let ptokens = tokenize_route_text(phrase);
            if ptokens.len() == 1 {
                for t in query_token_list {
                    if text_matches_phrase(std::slice::from_ref(t), phrase) {
                        matched_query_tokens.insert(t.clone());
                    }
                }
            }
        }
    }

    // Keyword tokens
    let mut shared_keywords = record
        .keyword_tokens
        .iter()
        .filter(|token| query_tokens.contains(*token))
        .cloned()
        .collect::<Vec<_>>();
    shared_keywords.sort();
    if !shared_keywords.is_empty() {
        delta += f64::min(
            w.keywords_max,
            (shared_keywords.len() as f64) * w.keywords_per_keyword,
        );
        reasons.push(format!(
            "Description keywords matched: {}.",
            shared_keywords
                .iter()
                .take(8)
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        for tok in &shared_keywords {
            matched_query_tokens.insert(tok.clone());
        }
    }

    // Alias tokens
    let mut alias_hits = record
        .alias_tokens
        .iter()
        .filter(|token| query_tokens.contains(*token))
        .cloned()
        .collect::<Vec<_>>();
    alias_hits.sort();
    if !alias_hits.is_empty() {
        delta += w.alias_hits_base + (alias_hits.len() as f64) * w.alias_hits_per_hit;
        reasons.push(format!(
            "Skill alias hints matched: {}.",
            alias_hits
                .iter()
                .take(8)
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        for tok in &alias_hits {
            matched_query_tokens.insert(tok.clone());
        }
    }

    (delta, reasons, matched_query_tokens)
}

/// Score session-start and code-review-deep signals. Returns `(delta, reasons)`.
#[inline]
fn score_session_start_signals(
    record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    first_turn: bool,
    current_score: f64,
    w: &ScoringWeights,
) -> (f64, Vec<String>) {
    let mut delta = 0.0f64;
    let mut reasons = Vec::with_capacity(2);

    if first_turn && current_score > 0.0 {
        if record.session_start_lower == "required" {
            delta += w.session_start_required_boost;
            reasons.push(format!(
                "Session-start required boost applied (+{:.0}).",
                w.session_start_required_boost
            ));
        } else if record.session_start_lower == "preferred" {
            delta += w.session_start_preferred_boost;
            reasons.push(format!(
                "Session-start preferred boost applied (+{:.0}).",
                w.session_start_preferred_boost
            ));
        }
    }

    if record.slug == "code-review-deep"
        && first_turn
        && is_review_prompt(query_text)
        && !has_paper_context(query_text, query_token_list)
    {
        delta += w.code_review_deep_boost;
        reasons.push(
            "Code-review-deep boost applied: review-class prompt without paper-only context."
                .to_string(),
        );
    }

    (delta, reasons)
}

pub fn score_route_candidate<'a>(
    record: &'a SkillRecord,
    query_text: &'a str,
    query_token_list: &'a [String],
    query_tokens: &'a HashSet<String>,
    first_turn: bool,
    w: &ScoringWeights,
) -> RouteCandidate<'a> {
    let mut score = 0.0f64;
    let mut reasons = Vec::new();
    let mut matched_query_tokens: HashSet<String> = HashSet::new();

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
    let workflow_orchestration_context = cached_signal(
        "has_workflow_orchestration_context",
        query_text,
        query_token_list,
        || {
            has_workflow_orchestration_context(query_text, query_token_list)
                && !has_workflow_negation_context(query_text, query_token_list)
        },
    );
    let explicit_framework_alias = framework_alias_requires_explicit_call(record)
        && has_explicit_framework_alias_call(query_text, query_token_list, record);
    let parallel_execution_context = has_parallel_execution_context(query_text, query_token_list);

    // --- agent-swarm signals ---
    let (swarm_delta, swarm_reasons) = score_agent_swarm_signals(
        record,
        query_text,
        query_token_list,
        w,
        bounded_subagent_context,
        workflow_orchestration_context,
        parallel_execution_context,
        token_budget_pressure,
    );
    score += swarm_delta;
    reasons.extend(swarm_reasons);

    // --- framework-alias suppression (early return) ---
    if let Some(done) = check_framework_alias_suppression(
        record,
        query_text,
        query_token_list,
        explicit_framework_alias,
    ) {
        return done;
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

    // --- design-md signals ---
    let (design_delta, design_reasons) =
        score_design_md_signals(record, query_text, query_token_list, score, w);
    score += design_delta;
    reasons.extend(design_reasons);

    if explicit_framework_alias {
        score += w.framework_alias_explicit_boost;
        reasons.push("Framework alias entrypoint matched explicitly.".to_string());
    }

    // --- gate / name-token / trigger-hint signals ---
    let (gate_delta, gate_reasons, gate_tokens) =
        score_gate_name_token_signals(record, query_text, query_token_list, query_tokens, w);
    score += gate_delta;
    reasons.extend(gate_reasons);
    matched_query_tokens.extend(gate_tokens);

    // --- metadata-trigger / keyword / alias signals ---
    let (meta_delta, meta_reasons, meta_tokens) =
        score_metadata_trigger_signals(record, query_tokens, query_token_list, w);
    score += meta_delta;
    reasons.extend(meta_reasons);
    matched_query_tokens.extend(meta_tokens);

    // --- session-start / code-review-deep signals ---
    let (start_delta, start_reasons) =
        score_session_start_signals(record, query_text, query_token_list, first_turn, score, w);
    score += start_delta;
    reasons.extend(start_reasons);

    if record.owner_lower == "gate" && score > 0.0 {
        score += w.gate_owner_boost;
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
        score += w.visual_review_boost;
        reasons.push(
            "Visual-review boost applied: visible UI evidence and concrete visual findings requested."
                .to_string(),
        );
    }

    if record.slug == "visual-review" && score > 0.0 {
        let markers = super::nl_route_adjustments::visual_evidence_markers();
        if !markers
            .iter()
            .any(|marker| query_text.contains(marker.as_str()))
        {
            score *= w.visual_review_weak_factor;
            reasons.push(
                "Visual-review weak match: no explicit visual evidence, reduced score.".to_string(),
            );
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
            let penalty = f64::min(
                score * w.do_not_use_penalty_max_ratio,
                (negative_hits.len() as f64) * w.do_not_use_penalty_per_hit,
            );
            score = f64::max(0.0, score - penalty);
            reasons.push(format!(
                "Do-not-use penalty applied: {}.",
                negative_hits
                    .iter()
                    .take(5)
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    if record.slug == "paper-workbench"
        && has_paper_review_revision_intent(query_text, query_token_list)
    {
        score += w.paper_workbench_boost;
        reasons.push(
            "Paper workbench boost applied: review-driven manuscript revision intent detected."
                .to_string(),
        );
    }

    if is_overlay_record(record) && score > 0.0 {
        score *= w.overlay_suppression_factor;
        reasons.push(format!(
            "Owner suppression applied: {} is overlay-only.",
            record.slug
        ));
    }
    let candidate = RouteCandidate {
        record,
        score,
        reasons,
        matched_token_count: matched_query_tokens.len(),
    };
    if score > 0.0 {
        debug!(
            slug = %candidate.record.slug,
            score = candidate.score,
            matched = candidate.matched_token_count,
            "route candidate scored"
        );
    }
    candidate
}

pub fn pick_owner<'a>(
    mut candidates: Vec<RouteCandidate<'a>>,
    query_text: &str,
    query_token_list: &[String],
    w: &ScoringWeights,
) -> RouteCandidate<'a> {
    let n = candidates.len();
    if n == 0 {
        panic!("pick_owner called with empty candidates");
    }

    // Owner candidate indices
    let mut owner_idx: Vec<usize> = (0..n)
        .filter(|&i| can_be_primary_owner(candidates[i].record))
        .collect();
    owner_idx.sort_unstable_by(|&a, &b| route_candidate_cmp(&candidates[a], &candidates[b]));

    let top_owner_score = owner_idx
        .first()
        .map(|&i| candidates[i].score)
        .unwrap_or(f64::NEG_INFINITY);

    // Gate index
    let gate_idx: Option<usize> = (0..n)
        .filter(|&i| {
            candidates[i].record.owner_lower == "gate"
                || candidates[i].record.gate_lower != "none"
        })
        .min_by(|&a, &b| route_candidate_cmp(&candidates[a], &candidates[b]));

    // Agent-swarm special case
    if let Some(idx) = gate_idx {
        if candidates[idx].record.slug == "agent-swarm-orchestration"
            && candidates[idx].score >= w.agent_swarm_candidate_threshold
            && !has_plan_mode_owner_context(query_text, query_token_list)
            && !has_systematic_debug_context(query_text, query_token_list)
        {
            let mut gate = candidates.swap_remove(idx);
            gate.reasons.push(
                "Prioritized delegation gate before strong owner for broad parallel-review admission."
                    .to_string(),
            );
            return gate;
        }
    }

    // Top owner above threshold
    if let Some(&top_idx) = owner_idx.first() {
        if candidates[top_idx].score >= w.top_owner_score_threshold {
            return candidates.swap_remove(top_idx);
        }
    }

    // Gate before owner
    if let Some(idx) = gate_idx {
        if candidates[idx].score >= w.gate_before_owner_threshold
            && candidates[idx].score >= top_owner_score
        {
            let mut gate = candidates.swap_remove(idx);
            gate.reasons
                .push("Prioritized via gate-before-owner precedence.".to_string());
            return gate;
        }
    }

    // Build owner-pool indices (no RouteCandidate clones)
    let mut pool_indices: Vec<usize> = if owner_idx.is_empty() {
        (0..candidates.len())
            .filter(|&i| !is_overlay_record(candidates[i].record))
            .collect()
    } else {
        owner_idx
    };

    if pool_indices.is_empty() {
        pool_indices = (0..candidates.len())
            .filter(|&i| can_be_fallback_owner(candidates[i].record))
            .collect();
    }
    if pool_indices.is_empty() {
        pool_indices = (0..candidates.len()).collect();
    }

    // Layer ranking (use &str instead of cloned String)
    let mut layers: Vec<&str> = pool_indices
        .iter()
        .map(|&i| candidates[i].record.layer.as_str())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    layers.sort_unstable_by_key(|layer| layer_rank(layer));

    for layer in layers {
        let mut layer_candidates: Vec<usize> = pool_indices
            .iter()
            .filter(|&&i| candidates[i].record.layer == layer)
            .copied()
            .collect();
        layer_candidates
            .sort_unstable_by(|&a, &b| route_candidate_cmp(&candidates[a], &candidates[b]));
        if let Some(&top) = layer_candidates.first() {
            if candidates[top].score >= w.layer_threshold(layer) {
                return candidates.swap_remove(top);
            }
        }
    }

    // Fallback: sort pool by layer, score, priority, slug
    let mut fallback_pool = pool_indices;
    fallback_pool.sort_unstable_by(|&a, &b| {
        layer_rank(&candidates[a].record.layer)
            .cmp(&layer_rank(&candidates[b].record.layer))
            .then_with(|| {
                finite_route_score(candidates[b].score)
                    .partial_cmp(&finite_route_score(candidates[a].score))
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| {
                priority_rank(&candidates[a].record.priority)
                    .cmp(&priority_rank(&candidates[b].record.priority))
            })
            .then_with(|| candidates[a].record.slug.cmp(&candidates[b].record.slug))
    });
    candidates.swap_remove(fallback_pool[0])
}

pub fn route_candidate_cmp(left: &RouteCandidate<'_>, right: &RouteCandidate<'_>) -> Ordering {
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

pub fn pick_overlay(
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

pub fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

pub fn score_bucket(score: f64) -> String {
    let clamped = score.max(0.0).min(100.0);
    let floor = ((clamped / 10.0).floor() as i32) * 10;
    format!("{:02}-{:02}", floor, (floor + 9).min(100))
}

pub fn compact_route_reasons(reasons: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut compact = Vec::with_capacity(reasons.len().min(6));
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

pub fn reasons_class(reasons: &[String]) -> String {
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

pub fn layer_rank(layer: &str) -> i32 {
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

pub fn priority_rank(priority: &str) -> i32 {
    match priority {
        "P0" => 0,
        "P1" => 1,
        "P2" => 2,
        "P3" => 3,
        _ => 99,
    }
}

#[cfg(test)]
mod paper_prose_routing_score_tests {
    use super::*;
    use crate::route::load_records;
    use crate::route::scoring_config::scoring_weights;
    use std::collections::HashSet;
    use std::path::PathBuf;

    #[test]
    fn sci_polish_scores_paper_workbench_above_doc_eng() {
        let runtime_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../skills/SKILL_ROUTING_RUNTIME.json");
        let records = load_records(Some(&runtime_path), None).expect("load runtime");
        let workbench = records
            .iter()
            .find(|r| r.slug == "paper-workbench")
            .expect("paper-workbench in runtime");
        let doc_eng = records
            .iter()
            .find(|r| r.slug == "documentation-engineering");
        let q = "SCI润色 abstract";
        let tokens = tokenize_route_text(q);
        let set: HashSet<String> = tokens.iter().cloned().collect();
        let w = scoring_weights();
        let wb = score_route_candidate(workbench, q, &tokens, &set, true, w);
        assert!(
            wb.score >= 82.0,
            "paper-workbench score {} reasons {:?}",
            wb.score,
            wb.reasons
        );
        if let Some(doc) = doc_eng {
            let de = score_route_candidate(doc, q, &tokens, &set, true, w);
            assert!(
                wb.score > de.score,
                "workbench {} must beat doc-eng {}",
                wb.score,
                de.score
            );
        }
    }

    #[test]
    fn snapshot_scoring_output_for_common_queries() {
        let runtime_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../skills/SKILL_ROUTING_RUNTIME.json");
        let records = load_records(Some(&runtime_path), None).expect("load runtime");
        let w = scoring_weights();

        let queries = vec![
            "SCI润色 abstract",
            "code review 找bug",
            "help me write tests",
            "重构这个模块",
            "security audit",
        ];

        let mut results = Vec::new();
        for q in queries {
            let tokens = tokenize_route_text(q);
            let set: HashSet<String> = tokens.iter().cloned().collect();
            let mut scores: Vec<_> = records
                .iter()
                .map(|r| {
                    let s = score_route_candidate(r, q, &tokens, &set, true, w);
                    (r.slug.clone(), s.score, s.reasons)
                })
                .filter(|(_, score, _)| *score > 0.0)
                .collect();
            scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            results.push(serde_json::json!({
                "query": q,
                "top3": scores.into_iter().take(3).map(|(slug, score, reasons)| {
                    serde_json::json!({"slug": slug, "score": score, "reasons": reasons})
                }).collect::<Vec<_>>()
            }));
        }
        insta::assert_json_snapshot!("scoring_common_queries", results);
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

#[cfg(test)]
mod snapshot_scoring_edge_cases {
    use super::*;
    use crate::route::load_records;
    use crate::route::scoring_config::scoring_weights;
    use std::path::PathBuf;

    #[test]
    fn snapshot_scoring_empty_query() {
        let runtime_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../skills/SKILL_ROUTING_RUNTIME.json");
        let records = load_records(Some(&runtime_path), None).expect("load runtime");
        let w = scoring_weights();
        let tokens = tokenize_route_text("");
        let set: HashSet<String> = tokens.iter().cloned().collect();
        let results: Vec<_> = records
            .iter()
            .map(|r| {
                let s = score_route_candidate(r, "", &tokens, &set, true, w);
                (r.slug.clone(), s.score)
            })
            .filter(|(_, score)| *score > 0.0)
            .collect();
        insta::assert_json_snapshot!("scoring_empty_query", results);
    }

    #[test]
    fn snapshot_scoring_chinese_query() {
        let runtime_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../skills/SKILL_ROUTING_RUNTIME.json");
        let records = load_records(Some(&runtime_path), None).expect("load runtime");
        let w = scoring_weights();
        let q = "帮我写一个单元测试覆盖边界情况";
        let tokens = tokenize_route_text(q);
        let set: HashSet<String> = tokens.iter().cloned().collect();
        let mut scores: Vec<_> = records
            .iter()
            .map(|r| {
                let s = score_route_candidate(r, q, &tokens, &set, true, w);
                (r.slug.clone(), s.score, s.reasons)
            })
            .filter(|(_, score, _)| *score > 0.0)
            .collect();
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        insta::assert_json_snapshot!("scoring_chinese_query", scores.into_iter().take(3).collect::<Vec<_>>());
    }
}
