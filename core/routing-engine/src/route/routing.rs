//! Primary routing entrypoints (search + `route_task`) and manifest fallback helpers.
use tracing;
use super::aliases::has_literal_framework_alias_call;
use super::constants::{
    NO_SKILL_SELECTED, PARALLEL_RECORD_SCAN_MIN, PROFILE_COMPILE_AUTHORITY, ROUTE_AUTHORITY,
    ROUTE_DECISION_SCHEMA_VERSION, SEARCH_RESULTS_SCHEMA_VERSION,
};
use super::fuzzy::{FUZZY_MIN_SIMILARITY, fuzzy_fallback_score};
use super::scoring::{
    compact_route_reasons, pick_overlay, pick_owner, reasons_class, round2, score_bucket,
    score_route_candidate,
};
use super::scoring_config::scoring_weights;
use super::signals::{build_route_context, is_overlay_record, should_route_to_gh_fix_ci};
use super::text::{common_route_stop_tokens, normalize_text, tokenize_route_text};
use super::types::{
    MatchRow, RouteContextPayload, RouteDecision, RouteDecisionSnapshotPayload, SearchMatchPayload,
    SearchMatchRecordPayload, SearchResultsPayload, SkillRecord,
};

/// Shared skeleton for every RouteDecision — avoids repeated `.to_string()` on static constants
/// per construction site (saves ~21 String allocs across 7 call sites).  Override
/// skill-specific fields with struct update syntax.
///
/// NOTE: `route_snapshot` is intentionally NOT built here — callers that need it
/// (i.e. `make_no_hit_decision`) build it manually. The hit-path callers
/// (`route_task`, `build_fuzzy_rescue_decision`) always override `route_snapshot`
/// via struct update syntax, so building it in the skeleton would be wasted work.
fn route_decision_skeleton(
    query: &str,
    session_id: &str,
    route_context: RouteContextPayload,
    reasons: Vec<String>,
) -> RouteDecision {
    RouteDecision {
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
        reasons: reasons.clone(),
        matched_token_count: 0,
        fuzzy_match: false,
        // route_snapshot is set by each caller — see NOTE above.
        route_snapshot: RouteDecisionSnapshotPayload::default(),
    }
}

/// No-hit decision (zero-score / runtime-layer fallback).  Thin wrapper around
/// [`route_decision_skeleton`].
fn make_no_hit_decision(
    query: &str,
    session_id: &str,
    route_context: RouteContextPayload,
    reasons: Vec<String>,
) -> RouteDecision {
    let mut decision = route_decision_skeleton(query, session_id, route_context, reasons);
    decision.route_snapshot = build_route_snapshot(
        "rust",
        &decision.selected_skill,
        None,
        "runtime",
        0.0,
        &decision.reasons,
    );
    decision
}
use rayon::prelude::*;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};

pub fn build_search_results_payload(query: &str, matches: Vec<MatchRow>) -> SearchResultsPayload {
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
        // Reversed: BinaryHeap is a max-heap; reversed Ord makes it a min-heap,
        // so peek() returns the lowest-ranked element — the correct behavior for
        // a top-k filter where we pop the lowest-rank to keep the best.
        other.key.cmp(&self.key)
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
        if let Some(worst) = heap.peek()
            && key > worst.key {
                heap.pop();
                heap.push(SearchHeapEntry { key, idx });
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
pub fn search_skills(records: &[SkillRecord], query: &str, limit: usize) -> Vec<MatchRow> {
    search_skills_subset(records, None, query, limit)
}

pub fn search_skills_subset(
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
    let query_tokens: HashSet<&str> = query_token_list
        .iter()
        .filter(|token| !common_route_stop_tokens().contains(&token.as_str()))
        .map(|s| s.as_str())
        .collect();
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
            None => records.iter().filter_map(score_record).collect::<Vec<_>>(),
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

pub fn filter_record_indices_for_host(
    records: &[SkillRecord],
    host_id: Option<&str>,
) -> Result<Vec<usize>, String> {
    let Some(host_id) = host_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok((0..records.len()).collect());
    };
    let host_id = host_id.to_ascii_lowercase();
    let aliases = crate::hooks::host_provider_routing_aliases(&host_id);
    let original_len = records.len();
    let mut saw_host = false;
    let mut indices = Vec::with_capacity(records.len());
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
        tracing::warn!(
            host_id, original_len, saw_host,
            "host_id filtered all records"
        );
    }

    if !saw_host {
        return Err(format!(
            "host-aware routing has no skill records for host_id `{host_id}`; host_platforms metadata is missing or the host id is unsupported"
        ));
    }
    Ok(indices)
}

pub fn filter_records_for_host(
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

/// Filter the overlay slug to prevent a skill from overlaying itself.
fn filter_overlay_self(overlay: Option<String>, slug: &str) -> Option<String> {
    overlay.filter(|item| item != slug)
}

pub fn route_task(
    records: &[SkillRecord],
    query: &str,
    session_id: &str,
    allow_overlay: bool,
    first_turn: bool,
) -> Result<RouteDecision, String> {
    crate::hooks::touch_kernel_bootstrap();
    crate::hooks::ensure_kernel_bootstrap();
    if records.is_empty() {
        return Err("No skill records available for route decision.".to_string());
    }
    if super::aliases::query_invokes_retired_framework_slash_command(query) {
        let route_context =
            build_route_context(&normalize_text(query), &tokenize_route_text(query));
        let fallback_reasons = compact_route_reasons(&[
            "Retired framework slash command; native runtime should proceed without loading a skill.",
        ]);
        return Ok(make_no_hit_decision(query, session_id, route_context, fallback_reasons));
    }
    let primary_query = primary_owner_query_text(query, records, allow_overlay);
    let normalized_query = normalize_text(&primary_query);
    let query_token_list = tokenize_route_text(&primary_query);
    let query_tokens: HashSet<&str> = query_token_list
        .iter()
        .filter(|token| !common_route_stop_tokens().contains(&token.as_str()))
        .map(|s| s.as_str())
        .collect();
    let route_context = build_route_context(&normalized_query, &query_token_list);
    let overlay_normalized_query = normalize_text(query);
    let overlay_query_tokens = tokenize_route_text(query);

    if let Some(decision) = literal_framework_alias_decision(records, query, session_id) {
        return Ok(decision);
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
        tracing::debug!(query, session_id, "route: no skill hit");
        let fallback_reasons = compact_route_reasons(&[
            "No explicit skill hit; native runtime should proceed without loading a skill.",
        ]);
        return Ok(make_no_hit_decision(query, session_id, route_context, fallback_reasons));
    }
    // 所有候选（含仅 1 个）全部是 overlay 且 caller 未允许 overlay 时返回 no-hit
    if !viable.is_empty()
        && viable
            .iter()
            .all(|candidate| is_overlay_record(candidate.record))
        && !allow_overlay
    {
        let fallback_reasons = compact_route_reasons(&[
            "Only overlay signals matched; native runtime should proceed without loading a primary skill.",
        ]);
        return Ok(make_no_hit_decision(query, session_id, route_context, fallback_reasons));
    }

    // Log: all-overlay candidates allowed through by caller
    if viable
        .iter()
        .all(|candidate| is_overlay_record(candidate.record))
    {
        tracing::debug!(query, session_id, "route: all-overlay candidates allowed");
    }
    let selected = pick_owner(
        viable,
        &normalized_query,
        &query_token_list,
        w,
    ).map_err(|e| format!("Routing failure: {e}"))?;
    if selected.score < w.layer_threshold(&selected.record.layer) {
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
        tracing::debug!(
            query,
            skill = %selected.record.slug,
            score = selected.score,
            threshold = w.layer_threshold(&selected.record.layer),
            "route: below threshold"
        );
        let fallback_reasons = compact_route_reasons(&[
            "No explicit skill hit; native runtime should proceed without loading a skill.",
        ]);
        return Ok(make_no_hit_decision(query, session_id, route_context, fallback_reasons));
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

    let filtered_overlay = filter_overlay_self(overlay, &selected.record.slug);
    let snapshot_overlay: Option<&str> = filtered_overlay.as_deref();
    let compact_reasons = compact_route_reasons(
        &selected.reasons.iter().map(|s| s.as_str()).collect::<Vec<_>>()
    );

    let route_snapshot = build_route_snapshot(
        "rust",
        &selected.record.slug,
        snapshot_overlay,
        &selected.record.layer,
        round2(selected.score),
        &compact_reasons,
    );
    let skeleton = route_decision_skeleton(query, session_id, route_context, compact_reasons.clone());
    Ok(RouteDecision {
        selected_skill: selected.record.slug.clone(),
        selected_skill_path: selected.record.skill_path.clone(),
                overlay_skill: filtered_overlay.clone(),
        layer: selected.record.layer.clone(),
        score: round2(selected.score),
        matched_token_count: selected.matched_token_count,
        route_snapshot,
        reasons: compact_reasons,
        ..skeleton
    })
}

/// Strip overlay-related trigger terms from the query text so the
/// remaining text can be matched against primary (non-overlay) skill
/// records.  Only performs work when `allow_overlay` is true; otherwise
/// returns the query unchanged.
///
/// Overlay records (`owner_lower == "overlay"`) carry trigger hints and
/// slugs that users may include in prompts, but that should not affect
/// primary-owner scoring.  This function collects all such terms and
/// removes them via token-level matching (rather than raw substring
/// replacement) to avoid mangling legitimate query words.
fn primary_owner_query_text(query: &str, records: &[SkillRecord], allow_overlay: bool) -> String {
    if !allow_overlay {
        return query.to_string();
    }
    // Collect overlay trigger hints and slug forms (minimum length > 3
    // to avoid stripping very short tokens that could be legitimate
    // query words).  Store normalized forms for token-level matching.
    let mut overlay_terms: Vec<String> = Vec::new();
    for record in records.iter().filter(|record| is_overlay_record(record)) {
        for hint in &record.trigger_hints {
            if hint.len() > 3 {
                overlay_terms.push(normalize_text(hint));
            }
        }
        if record.slug.len() > 3 {
            overlay_terms.push(normalize_text(&record.slug));
        }
        let slug_spaced = record.slug.replace('-', " ");
        if slug_spaced.len() > 3 {
            overlay_terms.push(normalize_text(&slug_spaced));
        }
    }
    if overlay_terms.is_empty() {
        return query.to_string();
    }

    // Token-level removal: normalize + tokenize the query, then skip
    // tokens that form overlay phrases (contiguous token sequences).
    // Uses the same tokenization as the routing scoring pipeline so
    // the removal is semantically consistent with later scoring steps.
    let query_tokens = tokenize_route_text(query);
    let mut keep = Vec::with_capacity(query_tokens.len());
    let mut pos = 0usize;
    while pos < query_tokens.len() {
        let mut skip = 0usize;
        for term in &overlay_terms {
            let phrase_tokens: Vec<String> = tokenize_route_text(term);
            if phrase_tokens.is_empty() || phrase_tokens.len() > query_tokens.len() - pos {
                continue;
            }
            let is_match = phrase_tokens
                .iter()
                .enumerate()
                .all(|(offset, pt)| query_tokens[pos + offset] == *pt);
            if is_match {
                skip = phrase_tokens.len();
                break;
            }
        }
        if skip > 0 {
            pos += skip;
        } else {
            keep.push(query_tokens[pos].clone());
            pos += 1;
        }
    }
    keep.join(" ")
}

/// Layer-based penalty applied during fuzzy rescue to discourage
/// low-priority layers from winning fuzzy matches over higher-priority ones.
fn fuzzy_layer_penalty(layer: &str) -> f64 {
    match layer {
        "L0" => 0.0,
        "L1" => -0.02,
        "L2" => -0.05,
        "L3" => -0.08,
        "L4" => -0.12,
        _ => -0.05,
    }
}

fn fuzzy_rescue_best_match<'a>(
    records: impl Iterator<Item = &'a SkillRecord>,
    query: &str,
) -> Option<(&'a SkillRecord, f64)> {
    records
        .map(|record| {
            let raw_sim = fuzzy_fallback_score(query, record);
            let effective = (raw_sim + fuzzy_layer_penalty(&record.layer)).max(0.0);
            (record, effective)
        })
        .filter(|(_, sim)| *sim >= FUZZY_MIN_SIMILARITY)
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(Ordering::Equal))
        
}

/// Gate skills excluded from fuzzy rescue when a matching gate context (e.g. CI)
/// is detected.  These skills have dedicated gate handlers that should take
/// precedence over fuzzy fallback.  Controlled by the
/// `behavior:ci_gate_fuzzy_rescue_excluded` skill flag.
fn fuzzy_rescue_primary_record<'a>(
    records: &'a [SkillRecord],
    query: &str,
) -> Option<(&'a SkillRecord, f64)> {
    let normalized = normalize_text(query);
    let tokens = tokenize_route_text(query);
    let ci_gate = should_route_to_gh_fix_ci(&normalized, &tokens);
    fuzzy_rescue_best_match(
        records.iter().filter(|record| {
            if is_overlay_record(record) {
                return false;
            }
            if ci_gate && has_skill_flag(record, "behavior:ci_gate_fuzzy_rescue_excluded") {
                return false;
            }
            true
        }),
        query,
    )
}

// 11 params — above threshold=8, OK to keep.
// Params span three distinct concerns (records, query, route context);
// no single natural group for struct extraction without introducing
// intermediate types that would also be > threshold.
#[allow(clippy::too_many_arguments)]
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
        tracing::debug!(
            query, skill = %record.slug, sim, exact_score = exact,
            "fuzzy rescue (below-threshold)"
        );
    } else {
        tracing::debug!(
            query, skill = %record.slug, sim,
            "fuzzy rescue"
        );
    }
    let overlay = if allow_overlay {
        pick_overlay(records, normalized_query, query_token_list, record)
    } else {
        None
    };
    let filtered_overlay = filter_overlay_self(overlay, &record.slug);
    let fuzzy_reasons = compact_route_reasons(&[reason_line]);
    let skeleton = route_decision_skeleton(query, session_id, route_context, fuzzy_reasons.clone());
    RouteDecision {
        selected_skill: record.slug.clone(),
        selected_skill_path: record.skill_path.clone(),
                overlay_skill: filtered_overlay.clone(),
        layer: record.layer.clone(),
        score: round2(sim * 100.0),
        fuzzy_match: true,
        route_snapshot: build_route_snapshot(
            "rust",
            &record.slug,
            filtered_overlay.as_deref(),
            &record.layer,
            round2(sim * 100.0),
            &fuzzy_reasons,
        ),
        reasons: fuzzy_reasons,
        ..skeleton
    }
}

pub fn literal_framework_alias_decision(
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
        compact_route_reasons(&["Framework alias entrypoint matched explicitly."]);
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

pub fn build_route_snapshot(
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

/// Check if a skill record in `records` matching the decision's selected_skill has a specific flag.
fn decision_has_skill_flag(decision: &RouteDecision, records: &[SkillRecord], flag: &str) -> bool {
    records
        .iter()
        .any(|r| r.slug == decision.selected_skill && has_skill_flag(r, flag))
}

/// Shared helper: check if a SkillRecord has a specific flag.
fn has_skill_flag(record: &SkillRecord, flag: &str) -> bool {
    record.skill_flags.iter().any(|f| f == flag)
}

pub fn should_retry_with_manifest(decision: &RouteDecision) -> bool {
    should_retry_with_manifest_records(decision, &[])
}

pub fn should_retry_with_manifest_records(decision: &RouteDecision, records: &[SkillRecord]) -> bool {
    if route_decision_is_no_hit(decision) {
        return true;
    }
    if decision.score < 35.0 {
        return true;
    }
    if decision_has_skill_flag(decision, records, "behavior:gate_exception_visual_screenshot") {
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

fn is_explicit_manifest_upgrade(hot: &RouteDecision, full: &RouteDecision) -> bool {
    full.score > hot.score
        || (full.score == hot.score && full.selected_skill != hot.selected_skill)
        || (full.selected_skill == hot.selected_skill
            && full.overlay_skill.is_some()
            && hot.overlay_skill.is_none())
}

/// Skills that use a low-score override in manifest fallback decisions.
/// Driven by `behavior:low_score_override:*` skill flag prefix.
fn find_low_score_override_min_score(records: &[SkillRecord]) -> Option<f64> {
    records
        .iter()
        .find_map(|r| {
            r.skill_flags.iter().find_map(|f| {
                f.strip_prefix("behavior:low_score_override:")
                    .and_then(|s| s.parse::<f64>().ok())
            })
        })
}

/// Gate skills whose own owners gate_retrigger_on_hot (session_start "required" + gate).
/// These block manifest fallback decisions from other owners.
fn is_runtime_required_gate(slug: &str, runtime_records: &[SkillRecord]) -> bool {
    runtime_records
        .iter()
        .find(|record| record.slug == slug)
        .is_some_and(|record| {
            record.session_start_lower == "required"
                && (record.owner_lower == "gate" || record.gate_lower != "none")
        })
}

fn hot_qualifies_for_retry(hot: &RouteDecision, records: &[SkillRecord]) -> bool {
    route_decision_is_no_hit(hot)
        || hot.score < 25.0
        || (hot.score < 35.0
            && records
                .iter()
                .any(|r| r.slug == hot.selected_skill && has_skill_flag(r, "behavior:hot_retry_eligible")))
        || (decision_has_skill_flag(hot, records, "behavior:systematic_debugging_high_threshold") && hot.score < 50.0)
    // ^^ systematic-debugging gets a higher retry threshold (50.0 vs 35.0)
    // because its gate-purpose (root-cause analysis) benefits from a wider
    // fallback safety margin — a low hot-score with a different skill shown
    // by the full manifest is likely a better route.
}

fn is_significant_score_gap_with_signal(full: &RouteDecision, hot: &RouteDecision) -> bool {
    full.score >= hot.score + 8.0 && has_non_generic_manifest_signal(full)
}

fn is_low_score_override(full: &RouteDecision, records: &[SkillRecord]) -> bool {
    records
        .iter()
        .any(|r| {
            r.slug == full.selected_skill && find_low_score_override_min_score(std::slice::from_ref(r))
                .is_some_and(|min_score| full.score >= min_score)
        })
}

fn is_minimal_score_without_signal(full: &RouteDecision, records: &[SkillRecord]) -> bool {
    full.score <= 10.0 && !is_low_score_override(full, records)
}

/// Consolidation of visual-review / systematic-debugging / screenshot gate-block exceptions.
/// Returns `true` when the gate block should NOT apply (i.e. the manifest decision is allowed).
fn runtime_gate_block_exception(
    hot_decision: &RouteDecision,
    full_decision: &RouteDecision,
    runtime_records: &[SkillRecord],
) -> bool {
    // visual-review → screenshot: allow fallback unless in audit protocol
    if decision_has_skill_flag(hot_decision, runtime_records, "behavior:gate_exception_visual_screenshot")
        && full_decision.selected_skill == "screenshot"
        && hot_decision.route_context.execution_protocol != "audit"
    {
        return true;
    }

    // visual-review → systematic-debugging: allow when hot qualifies for retry
    if decision_has_skill_flag(hot_decision, runtime_records, "behavior:gate_exception_visual_debugging")
        && full_decision.selected_skill == "systematic-debugging"
        && should_retry_with_manifest_records(hot_decision, runtime_records)
    {
        return true;
    }

    // skill-framework-developer override: always allow when higher-scored with signal
    if decision_has_skill_flag(full_decision, runtime_records, "behavior:gate_exception_framework_dev")
        && full_decision.score > hot_decision.score
        && has_non_generic_manifest_signal(full_decision)
    {
        return true;
    }

    false
}

fn is_same_skill_with_extra_overlay(hot: &RouteDecision, full: &RouteDecision) -> bool {
    full.selected_skill == hot.selected_skill
        && full.overlay_skill.is_some()
        && hot.overlay_skill.is_none()
}

/// Check whether the runtime gate blocks a manifest fallback owner from being selected.
/// First checks for exception cases (visual-review↔screenshot, etc.), then falls back
/// to the standard required-gate check.
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

    if runtime_gate_block_exception(hot_decision, full_decision, runtime_records) {
        return false;
    }

    is_runtime_required_gate(&hot_decision.selected_skill, runtime_records)
}

pub fn should_accept_manifest_fallback(
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
        return is_explicit_manifest_upgrade(hot_decision, full_decision);
    }

    if is_same_skill_with_extra_overlay(hot_decision, full_decision) {
        return true;
    }

    if !should_retry || !hot_qualifies_for_retry(hot_decision, runtime_records) {
        return is_significant_score_gap_with_signal(full_decision, hot_decision);
    }

    if is_minimal_score_without_signal(full_decision, runtime_records) {
        return false;
    }

    // should_retry = true with hot qualifying for retry
    if should_retry && full_decision.score > hot_decision.score {
        return true;
    }

    if is_low_score_override(full_decision, runtime_records) {
        return true;
    }

    if !has_non_generic_manifest_signal(full_decision) {
        return false;
    }

    full_decision.score > hot_decision.score
        || (full_decision.score == hot_decision.score
            && full_decision.selected_skill != hot_decision.selected_skill)
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

    fn make_visual_review_record() -> SkillRecord {
        SkillRecord {
            slug: "visual-review".to_string(),
            skill_path: None,
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
            gate_phrases: vec![],
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
            skill_flags: vec![
                "scoring:visual_review".to_string(),
                "behavior:gate_exception_visual_screenshot".to_string(),
                "behavior:gate_exception_visual_debugging".to_string(),
            ],
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
        assert!(should_retry_with_manifest_records(&decision, &[make_visual_review_record()]));
    }

    #[test]
    fn visual_review_audit_does_not_retry_when_score_is_high() {
        let mut decision = make_decision("visual-review", 90.0, "L1", "audit");
        decision
            .reasons
            .push("Visual-review boost applied: visible UI evidence and concrete visual findings requested.".to_string());
        assert!(!should_retry_with_manifest_records(&decision, &[make_visual_review_record()]));
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
        assert!(should_retry_with_manifest_records(&decision, &[make_visual_review_record()]));
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
            skill_flags: vec![
                "scoring:visual_review".to_string(),
                "behavior:gate_exception_visual_screenshot".to_string(),
                "behavior:gate_exception_visual_debugging".to_string(),
            ],
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
