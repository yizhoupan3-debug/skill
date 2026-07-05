//! Primary routing entrypoints (`route_task`, search).
use super::aliases::{has_literal_framework_alias_call, qg_checker_id_for_slug};
use super::constants::{
    FRAMEWORK_COMMAND_KIND, NO_SKILL_SELECTED, PARALLEL_RECORD_SCAN_MIN, PROFILE_COMPILE_AUTHORITY,
    ROUTE_AUTHORITY, ROUTE_DECISION_SCHEMA_VERSION, SEARCH_RESULTS_SCHEMA_VERSION,
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
use core_errors::FrameworkError;
use routing_core::audit_log::AuditLog;
use tracing;

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
        checker_id: None,
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
        0,
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
            && key > worst.key
        {
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
) -> Result<Vec<usize>, FrameworkError> {
    let Some(host_id) = host_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok((0..records.len()).collect());
    };
    let host_id = host_id.to_ascii_lowercase();
    let aliases = crate::hooks::host_provider_routing_aliases(&host_id);
    let original_len = records.len();
    let mut saw_host = false;
    let mut indices = Vec::with_capacity(records.len());
    for (idx, record) in records.iter().enumerate() {
        if record.record_kind == FRAMEWORK_COMMAND_KIND {
            saw_host = true;
            indices.push(idx);
            continue;
        }
        let allowed = record.host_platforms.is_empty()
            || record.host_platforms.iter().any(|platform| {
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
            host_id,
            original_len,
            saw_host,
            "host_id filtered all records"
        );
    }

    if !saw_host {
        tracing::warn!(
            host_id,
            original_len,
            "host-aware routing has no skill records for host_id; returning empty"
        );
        return Ok(vec![]);
    }
    Ok(indices)
}

pub fn filter_records_for_host(
    records: impl AsRef<[SkillRecord]>,
    host_id: Option<&str>,
) -> Result<Vec<SkillRecord>, FrameworkError> {
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
) -> Result<RouteDecision, FrameworkError> {
    if records.is_empty() {
        return Err(FrameworkError::NotFound {
            what: "No skill records available for route decision.".to_string(),
        });
    }
    if super::aliases::query_invokes_retired_framework_slash_command(query) {
        let route_context =
            build_route_context(&normalize_text(query), &tokenize_route_text(query));
        let fallback_reasons = compact_route_reasons(&[
            "Retired framework slash command; native runtime should proceed without loading a skill.",
        ]);
        return Ok(log_decision(
            make_no_hit_decision(query, session_id, route_context, fallback_reasons),
            query,
            session_id,
        ));
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

    if let Some(decision) = literal_framework_alias_decision(
        records,
        query,
        &normalized_query,
        &query_token_list,
        session_id,
    ) {
        return Ok(log_decision(decision, query, session_id));
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
            return Ok(log_decision(
                build_fuzzy_rescue_decision(
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
                ),
                query,
                session_id,
            ));
        }
        tracing::debug!(query, session_id, "route: no skill hit");
        let fallback_reasons = compact_route_reasons(&[
            "No explicit skill hit; native runtime should proceed without loading a skill.",
        ]);
        return Ok(log_decision(
            make_no_hit_decision(query, session_id, route_context, fallback_reasons),
            query,
            session_id,
        ));
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
        return Ok(log_decision(
            make_no_hit_decision(query, session_id, route_context, fallback_reasons),
            query,
            session_id,
        ));
    }

    // Log: all-overlay candidates allowed through by caller
    if viable
        .iter()
        .all(|candidate| is_overlay_record(candidate.record))
    {
        tracing::debug!(query, session_id, "route: all-overlay candidates allowed");
    }
    let selected = pick_owner(viable, &normalized_query, &query_token_list, w)?;
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
        &selected
            .reasons
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>(),
    );

    let route_snapshot = build_route_snapshot(
        "rust",
        &selected.record.slug,
        snapshot_overlay,
        &selected.record.layer,
        round2(selected.score),
        &compact_reasons,
        selected.matched_token_count,
    );
    let skeleton =
        route_decision_skeleton(query, session_id, route_context, compact_reasons.clone());
    Ok(log_decision(
        RouteDecision {
            selected_skill: selected.record.slug.clone(),
            selected_skill_path: selected.record.skill_path.clone(),
            overlay_skill: filtered_overlay.clone(),
            layer: selected.record.layer.clone(),
            score: round2(selected.score),
            matched_token_count: selected.matched_token_count,
            route_snapshot,
            reasons: compact_reasons,
            ..skeleton
        },
        query,
        session_id,
    ))
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

fn fuzzy_rescue_best_match<'a>(
    records: impl Iterator<Item = &'a SkillRecord>,
    query: &str,
) -> Option<(&'a SkillRecord, f64)> {
    records
        .map(|record| {
            
            let effective = fuzzy_fallback_score(query, record);
            (record, effective)
        })
        .filter(|(_, sim)| *sim >= FUZZY_MIN_SIMILARITY)
        // partial_cmp returns None for NaN; unwrap_or(Ordering::Equal) handles
        // the hypothetical case gracefully — fuzzy_fallback_score never produces NaN
        // for normal inputs (no division by zero, no sqrt of negative numbers).
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
            0,
        ),
        reasons: fuzzy_reasons,
        ..skeleton
    }
}

pub fn literal_framework_alias_decision(
    records: &[SkillRecord],
    query: &str,
    normalized_query: &str,
    query_token_list: &[String],
    session_id: &str,
) -> Option<RouteDecision> {
    let route_context = build_route_context(normalized_query, query_token_list);
    let record = records
        .iter()
        .find(|record| has_literal_framework_alias_call(normalized_query, record))?;
    let reasons = compact_route_reasons(&["Framework alias entrypoint matched explicitly."]);
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
            0,
        ),
        reasons,
        matched_token_count: 0,
        fuzzy_match: false,
        checker_id: qg_checker_id_for_slug(&record.slug).map(|s| s.to_string()),
    })
}

pub fn build_route_snapshot(
    engine: &str,
    selected_skill: &str,
    overlay_skill: Option<&str>,
    layer: &str,
    score: f64,
    reasons: &[String],
    matched_token_count: usize,
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
        matched_token_count,
    }
}

/// Shared helper: check if a SkillRecord has a specific flag.
fn has_skill_flag(record: &SkillRecord, flag: &str) -> bool {
    record.skill_flags.iter().any(|f| f == flag)
}

/// Wrapper that logs every routing decision to `logs/skill-routing/routing_audit.ndjson`.
/// Logger auto-initializes on first call (creates directory + file).
fn log_decision(decision: RouteDecision, query: &str, _session_id: &str) -> RouteDecision {
    static LOG: AuditLog = AuditLog::new();

    let entry = serde_json::json!({
        "ts": routing_core::audit_log::iso_timestamp_now(),
        "query": query,
        "selected_skill": decision.selected_skill,
        "score": decision.score,
        "layer": decision.layer,
        "fuzzy_match": decision.fuzzy_match,
        "overlay_skill": decision.overlay_skill,
        "matched_token_count": decision.matched_token_count,
        "top_3_reasons": &decision.reasons.iter().take(3).cloned().collect::<Vec<_>>(),
    });

    LOG.write_entry("logs/skill-routing/routing_audit.ndjson", &entry);

    decision
}



// End of routing.rs
