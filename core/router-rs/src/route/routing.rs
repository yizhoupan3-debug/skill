//! Primary routing entrypoints (search + `route_task`) and manifest fallback helpers.
use super::aliases::has_literal_framework_alias_call;
use super::constants::{
    NO_SKILL_SELECTED, PARALLEL_RECORD_SCAN_MIN, PROFILE_COMPILE_AUTHORITY, ROUTE_AUTHORITY,
    ROUTE_DECISION_SCHEMA_VERSION, SEARCH_RESULTS_SCHEMA_VERSION,
};
use super::fuzzy::{fuzzy_fallback_score, FUZZY_MIN_SIMILARITY};
use super::scoring::{
    compact_route_reasons, pick_overlay, pick_owner, reasons_class, round2,
    score_bucket, score_route_candidate,
};
use super::scoring_config::scoring_weights;
use super::signals::{build_route_context, is_overlay_record};
use super::text::{common_route_stop_tokens, normalize_text, tokenize_route_text};
use super::types::{
    MatchRow, RouteContextPayload, RouteDecision, RouteDecisionSnapshotPayload, SearchMatchPayload,
    SearchMatchRecordPayload, SearchResultsPayload, SkillRecord,
};
use rayon::prelude::*;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};

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

#[derive(Eq, PartialEq)]
struct SearchRankKey {
    score_bits: u64,
    matched_terms: usize,
    slug: String,
}

impl Ord for SearchRankKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score_bits
            .cmp(&other.score_bits)
            .then_with(|| self.matched_terms.cmp(&other.matched_terms))
            .then_with(|| self.slug.cmp(&other.slug))
    }
}

impl PartialOrd for SearchRankKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct SearchHeapEntry {
    key: SearchRankKey,
    idx: usize,
}

impl Eq for SearchHeapEntry {}

impl PartialEq for SearchHeapEntry {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Ord for SearchHeapEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key.cmp(&other.key)
    }
}

impl PartialOrd for SearchHeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn search_rank_key(row: &MatchRow) -> SearchRankKey {
    SearchRankKey {
        score_bits: row.score.to_bits(),
        matched_terms: row.matched_terms,
        slug: row.slug.clone(),
    }
}

fn cmp_match_rows(left: &MatchRow, right: &MatchRow) -> Ordering {
    right
        .score
        .partial_cmp(&left.score)
        .unwrap_or(Ordering::Equal)
        .then_with(|| right.matched_terms.cmp(&left.matched_terms))
        .then_with(|| left.slug.cmp(&right.slug))
}

fn finalize_search_rows(mut rows: Vec<MatchRow>, limit: usize) -> Vec<MatchRow> {
    if rows.len() <= limit {
        rows.sort_unstable_by(cmp_match_rows);
        return rows;
    }
    // Top-k heap when many positives: avoid sorting the full match set.
    let mut heap: BinaryHeap<SearchHeapEntry> = BinaryHeap::with_capacity(limit);
    for (idx, row) in rows.iter().enumerate() {
        let key = search_rank_key(row);
        if heap.len() < limit {
            heap.push(SearchHeapEntry { key, idx });
            continue;
        }
        if let Some(worst) = heap.peek() {
            if key > worst.key {
                heap.pop();
                heap.push(SearchHeapEntry { key, idx });
            }
        }
    }
    let mut out = heap
        .into_iter()
        .map(|entry| rows[entry.idx].clone())
        .collect::<Vec<_>>();
    out.sort_unstable_by(cmp_match_rows);
    out
}

/// Score and rank skills for a query. When `indices` is `Some`, only those
/// record positions are considered (avoids cloning records for host filtering).
pub(crate) fn search_skills(
    records: &[SkillRecord],
    query: &str,
    limit: usize,
) -> Vec<MatchRow> {
    search_skills_subset(records, None, query, limit)
}

pub(crate) fn search_skills_subset(
    records: &[SkillRecord],
    indices: Option<&[usize]>,
    query: &str,
    limit: usize,
) -> Vec<MatchRow> {
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

    let scan_len = indices.map(|idxs| idxs.len()).unwrap_or(records.len());
    let w = scoring_weights();
    let score_record = |record: &SkillRecord| {
        let candidate = score_route_candidate(
            record,
            &normalized_query,
            &query_token_list,
            &query_tokens,
            true,
            w,
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
            matched_terms: candidate.matched_token_count,
            total_terms: query_tokens.len().max(query_token_list.len()),
        })
    };

    let rows = if scan_len < PARALLEL_RECORD_SCAN_MIN {
        match indices {
            Some(idxs) => idxs
                .iter()
                .filter_map(|&idx| score_record(&records[idx]))
                .collect::<Vec<_>>(),
            None => records
                .iter()
                .filter_map(score_record)
                .collect::<Vec<_>>(),
        }
    } else {
        match indices {
            Some(idxs) => idxs
                .par_iter()
                .filter_map(|&idx| score_record(&records[idx]))
                .collect::<Vec<_>>(),
            None => records
                .par_iter()
                .filter_map(score_record)
                .collect::<Vec<_>>(),
        }
    };

    finalize_search_rows(rows, limit)
}


pub(crate) fn filter_record_indices_for_host(
    records: &[SkillRecord],
    host_id: Option<&str>,
) -> Result<Vec<usize>, String> {
    let Some(host_id) = host_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok((0..records.len()).collect());
    };
    let host_id = host_id.to_ascii_lowercase();
    let aliases = crate::hosts::host_provider_routing_aliases(&host_id);
    let original_len = records.len();
    let mut saw_host = false;
    let mut indices = Vec::new();
    for (idx, record) in records.iter().enumerate() {
        if record.record_kind == "framework_command" {
            saw_host = true;
            indices.push(idx);
            continue;
        }
        let allowed = record.host_platforms.iter().any(|platform| {
            aliases
                .iter()
                .any(|alias| platform.eq_ignore_ascii_case(alias))
        });
        saw_host |= allowed;
        if allowed {
            indices.push(idx);
        }
    }

    if indices.is_empty() && original_len > 0 {
        eprintln!(
            "[router-rs warning] host_id={} filtered all {} records (saw_host={})",
            host_id, original_len, saw_host
        );
    }

    if !saw_host {
        return Err(format!(
            "host-aware routing has no skill records for host_id `{host_id}`; host_platforms metadata is missing or the host id is unsupported"
        ));
    }
    Ok(indices)
}

pub(crate) fn filter_records_for_host(
    records: impl AsRef<[SkillRecord]>,
    host_id: Option<&str>,
) -> Result<Vec<SkillRecord>, String> {
    let records = records.as_ref();
    let indices = filter_record_indices_for_host(records, host_id)?;
    Ok(indices
        .into_iter()
        .map(|idx| records[idx].clone())
        .collect())
}

pub(crate) fn route_task(
    records: &[SkillRecord],
    query: &str,
    session_id: &str,
    allow_overlay: bool,
    first_turn: bool,
) -> Result<RouteDecision, String> {
    crate::touch_test_kernel_bootstrap();
    crate::kernel_bootstrap::ensure_kernel_bootstrap();
    if records.is_empty() {
        return Err("No skill records available for route decision.".to_string());
    }
    if super::aliases::query_invokes_retired_framework_slash_command(query) {
        let route_context = build_route_context(
            &normalize_text(query),
            &tokenize_route_text(query),
        );
        let fallback_reasons = compact_route_reasons(&[
            "Retired framework slash command; native runtime should proceed without loading a skill."
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
            matched_token_count: 0,
            fuzzy_match: false,
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
    let primary_query = primary_owner_query_text(query, records, allow_overlay);
    let normalized_query = normalize_text(&primary_query);
    let query_token_list = tokenize_route_text(&primary_query);
    let query_tokens = query_token_list
        .iter()
        .filter(|token| !common_route_stop_tokens().contains(&token.as_str()))
        .cloned()
        .collect::<HashSet<String>>();
    let route_context = build_route_context(&normalized_query, &query_token_list);
    let overlay_normalized_query = normalize_text(query);
    let overlay_query_tokens = tokenize_route_text(query);

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
            fuzzy_match: false,
            route_snapshot: build_route_snapshot(
                "rust",
                &record.slug,
                None,
                &record.layer,
                100.0,
                &reasons,
            ),
            reasons,
            matched_token_count: 0,
        });
    }

    let w = scoring_weights();
    let score = |record| {
        score_route_candidate(
            record,
            &normalized_query,
            &query_token_list,
            &query_tokens,
            first_turn,
            w,
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
        // --- Fuzzy fallback: try trigram similarity against all records ---
        if let Some((record, sim)) = fuzzy_rescue_primary_record(records, &primary_query) {
            return Ok(build_fuzzy_rescue_decision(
                records,
                record,
                sim,
                query,
                &overlay_normalized_query,
                &overlay_query_tokens,
                session_id,
                route_context,
                allow_overlay,
                &format!("Fuzzy trigram fallback rescued match (similarity={sim:.3})."),
                None,
            ));
        }
        eprintln!(
            "[router-rs route] NO SKILL HIT: query=\"{}\" session_id=\"{}\"",
            query, session_id
        );
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
            matched_token_count: 0,
            fuzzy_match: false,
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
    // 所有候选（含仅 1 个）全部是 overlay 且 caller 未允许 overlay 时返回 no-hit
    if !viable.is_empty()
        && viable
            .iter()
            .all(|candidate| is_overlay_record(candidate.record))
        && !allow_overlay
    {
        let fallback_reasons = compact_route_reasons(&[
            "Only overlay signals matched; native runtime should proceed without loading a primary skill."
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
            matched_token_count: 0,
            fuzzy_match: false,
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

    // Log: all-overlay candidates allowed through by caller
    if viable.iter().all(|candidate| is_overlay_record(candidate.record)) {
        eprintln!("[router-rs route] ALL-OVERLAY ALLOWED: query=\"{}\" session_id=\"{}\"",
            query, session_id
        );
    }
    let selected = pick_owner(viable, &normalized_query, &query_token_list, scoring_weights());
    if selected.score < scoring_weights().layer_threshold(&selected.record.layer) {
        // --- Fuzzy fallback: try trigram similarity before giving up ---
        if let Some((record, sim)) = fuzzy_rescue_primary_record(records, &primary_query) {
            return Ok(build_fuzzy_rescue_decision(
                records,
                record,
                sim,
                query,
                &overlay_normalized_query,
                &overlay_query_tokens,
                session_id,
                route_context,
                allow_overlay,
                &format!(
                    "Fuzzy trigram fallback rescued below-threshold match (similarity={sim:.3}, exact_score={:.2}).",
                    selected.score
                ),
                Some(selected.score),
            ));
        }
        eprintln!(
            "[router-rs route] BELOW THRESHOLD: query=\"{}\" selected={} score={:.2} threshold={:.2}",
            query,
            selected.record.slug,
            selected.score,
            scoring_weights().layer_threshold(&selected.record.layer)
        );
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
            matched_token_count: 0,
            fuzzy_match: false,
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
            &overlay_normalized_query,
            &overlay_query_tokens,
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
        matched_token_count: selected.matched_token_count,
        fuzzy_match: false,
    })
}

fn primary_owner_query_text(query: &str, records: &[SkillRecord], allow_overlay: bool) -> String {
    if !allow_overlay {
        return query.to_string();
    }
    let mut text = query.to_string();
    for record in records.iter().filter(|record| is_overlay_record(record)) {
        for hint in &record.trigger_hints {
            if hint.chars().count() > 3 {
                text = text.replace(hint, " ");
            }
        }
        let slug_spaced = record.slug.replace('-', " ");
        for token in [record.slug.as_str(), slug_spaced.as_str()] {
            if token.chars().count() > 3 {
                text = text.replace(token, " ");
            }
        }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn fuzzy_rescue_best_match<'a>(
    records: impl Iterator<Item = &'a SkillRecord>,
    query: &str,
) -> Option<(&'a SkillRecord, f64)> {
    records
        .map(|record| (record, fuzzy_fallback_score(query, record)))
        .filter(|(_, sim)| *sim >= FUZZY_MIN_SIMILARITY)
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(Ordering::Equal))
        .map(|(record, sim)| (record, sim))
}

fn fuzzy_rescue_primary_record<'a>(
    records: &'a [SkillRecord],
    query: &str,
) -> Option<(&'a SkillRecord, f64)> {
    fuzzy_rescue_best_match(
        records.iter().filter(|record| !is_overlay_record(record)),
        query,
    )
}

fn build_fuzzy_rescue_decision(
    records: &[SkillRecord],
    record: &SkillRecord,
    sim: f64,
    query: &str,
    normalized_query: &str,
    query_token_list: &[String],
    session_id: &str,
    route_context: RouteContextPayload,
    allow_overlay: bool,
    reason_line: &str,
    exact_score: Option<f64>,
) -> RouteDecision {
    if let Some(exact) = exact_score {
        eprintln!(
            "[router-rs route] FUZZY RESCUE (below-threshold): query=\"{}\" skill=\"{}\" sim={:.3} exact_score={:.2}",
            query, record.slug, sim, exact
        );
    } else {
        eprintln!(
            "[router-rs route] FUZZY RESCUE: query=\"{}\" skill=\"{}\" sim={:.3}",
            query, record.slug, sim
        );
    }
    let overlay = if allow_overlay {
        pick_overlay(records, normalized_query, query_token_list, record)
    } else {
        None
    };
    let filtered_overlay = overlay
        .as_ref()
        .filter(|item| *item != &record.slug)
        .cloned();
    let fuzzy_reasons = compact_route_reasons(&[reason_line.to_string()]);
    RouteDecision {
        decision_schema_version: ROUTE_DECISION_SCHEMA_VERSION.to_string(),
        authority: ROUTE_AUTHORITY.to_string(),
        compile_authority: PROFILE_COMPILE_AUTHORITY.to_string(),
        task: query.to_string(),
        session_id: session_id.to_string(),
        selected_skill: record.slug.clone(),
        selected_skill_path: record.skill_path.clone(),
        overlay_skill: filtered_overlay,
        route_context,
        layer: record.layer.clone(),
        score: round2(sim * 100.0),
        reasons: fuzzy_reasons.clone(),
        matched_token_count: 0,
        fuzzy_match: true,
        route_snapshot: build_route_snapshot(
            "rust",
            &record.slug,
            overlay
                .as_deref()
                .filter(|item| *item != record.slug.as_str()),
            &record.layer,
            round2(sim * 100.0),
            &fuzzy_reasons,
        ),
    }
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
        matched_token_count: 0,
        fuzzy_match: false,
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
        matched_token_count: 0,
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

    if hot_decision.selected_skill == "visual-review"
        && full_decision.selected_skill == "systematic-debugging"
        && should_retry_with_manifest(hot_decision)
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
            matched_token_count: 0,
            fuzzy_match: false,
            route_snapshot: RouteDecisionSnapshotPayload {
                engine: "rust".to_string(),
                selected_skill: skill.to_string(),
                overlay_skill: None,
                layer: layer.to_string(),
                score,
                score_bucket: String::new(),
                reasons: Vec::new(),
                matched_token_count: 0,
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

    #[test]
    fn visual_review_hot_does_not_block_manifest_systematic_debugging() {
        let mut hot = make_decision("visual-review", 14.0, "L3", "four_step");
        hot.reasons.push(
            "Visual-review weak match: no explicit visual evidence, reduced score.".to_string(),
        );
        let mut full = make_decision("systematic-debugging", 72.0, "L2", "four_step");
        full.reasons.push("Routing gate matched: bug.".to_string());
        full.reasons.push(
            "Systematic-debugging boost applied: explicit bug, root-cause, failure, or regression-test diagnostic wording detected.".to_string(),
        );
        let runtime_records = vec![SkillRecord {
            slug: "visual-review".to_string(),
            skill_path: Some("skills/visual-review/SKILL.md".to_string()),
            layer: "L3".to_string(),
            owner: "gate".to_string(),
            gate: "evidence".to_string(),
            priority: "P1".to_string(),
            session_start: "required".to_string(),
            summary: String::new(),
            slug_lower: "visual-review".to_string(),
            owner_lower: "gate".to_string(),
            gate_lower: "evidence".to_string(),
            session_start_lower: "required".to_string(),
            gate_phrases: vec!["bug".to_string()],
            trigger_hints: vec![],
            name_tokens: HashSet::new(),
            keyword_tokens: HashSet::new(),
            alias_tokens: HashSet::new(),
            do_not_use_tokens: HashSet::new(),
            framework_alias_entrypoints: vec![],
            metadata_positive_triggers: vec![],
            host_platforms: vec![],
            record_kind: "skill".to_string(),
            primary_allowed: true,
            fallback_policy_mode: "eligible-in-runtime".to_string(),
        }];
        assert!(should_retry_with_manifest(&hot));
        assert!(should_accept_manifest_fallback(
            &hot,
            &full,
            &runtime_records,
            true,
            false,
        ));
    }
}
