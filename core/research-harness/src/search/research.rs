//! Claim-driven research orchestration: external search, novelty gate
//! recommendation, claim proposal, and research evaluation.
//!
//! Migrated from `tools/autoresearch-rs/src/research.rs`.
//!
//! This module provides the **upper-layer orchestration** for the research
//! loop: "what to search", "when to search", and "what to decide after search".

use anyhow::{bail, Result};
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};

use crate::search::helpers::*;
use crate::search::strategy::*;

// ── Local helpers ──

fn str_key<'a>(state: &'a Value, key: &str) -> &'a str {
    state.get(key).and_then(Value::as_str).unwrap_or("")
}

fn novelty_gate(state: &Value) -> &Value {
    state.get("novelty_gate").unwrap_or(&Value::Null)
}

fn novelty_gate_mut(state: &mut Value) -> &mut serde_json::Map<String, Value> {
    state
        .as_object_mut()
        .expect("state must be object")
        .entry("novelty_gate".to_string())
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .expect("novelty_gate must be object")
}

fn novelty_arr<'a>(state: &'a Value, key: &str) -> &'a [Value] {
    novelty_gate(state)
        .get(key)
        .and_then(Value::as_array)
        .map(|a| a.as_slice())
        .unwrap_or(&[])
}

fn ensure_state_defaults(state: &Value) -> Value {
    let mut s = state.clone();
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let obj = s.as_object_mut().expect("state must be object");
    for (key, default) in [
        ("hypotheses", json!([])),
        ("hypothesis_backlog", json!([])),
        ("run_history", json!([])),
        ("decisions", json!([])),
        ("evidence_index", json!([])),
        ("external_research", json!([])),
        ("blockers", json!([])),
        ("next_actions", json!([])),
    ] {
        obj.entry(key).or_insert(default);
    }
    obj.entry("novelty_gate").or_insert(json!({
        "status": "pending", "claims": [], "claim_records": [], "draft_claims": []
    }));
    obj.entry("updated_at").or_insert(json!(now));
    obj.entry("created_at").or_insert(json!(now));
    s
}

#[allow(dead_code)]
fn arr<'a>(state: &'a Value, key: &str) -> &'a [Value] {
    state
        .get(key)
        .and_then(Value::as_array)
        .map(|a| a.as_slice())
        .unwrap_or(&[])
}

fn arr_mut<'a>(state: &'a mut Value, key: &str) -> &'a mut Vec<Value> {
    state
        .as_object_mut()
        .expect("state must be object")
        .entry(key.to_string())
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .expect("expected array")
}

#[allow(dead_code)]
fn set_key(state: &mut Value, key: &str, value: Value) {
    state
        .as_object_mut()
        .expect("state must be object")
        .insert(key.to_string(), value);
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

// ── Query building ──

/// Find the claim record for research by ID, or fall back to top-priority claim.
pub fn claim_record_for_research(state: &Value, claim_id: Option<&str>) -> Option<Value> {
    if let Some(desired) = claim_id {
        for key in ["claim_records", "draft_claims"] {
            if let Some(record) = novelty_gate(state)
                .get(key)
                .and_then(Value::as_array)
                .and_then(|items| {
                    items
                        .iter()
                        .find(|item| item.get("claim_id").and_then(Value::as_str) == Some(desired))
                })
            {
                return Some(record.clone());
            }
        }
    }
    top_priority_claim(state)
}

/// Build a default research query from a claim record or explicit query.
pub fn default_research_query(
    record: Option<&Value>,
    explicit_query: Option<&str>,
) -> Result<String> {
    if let Some(query) = explicit_query
        .map(str::trim)
        .filter(|query| !query.is_empty())
    {
        return Ok(query.to_string());
    }
    if let Some(record) = record
        && let Some(query) = build_search_queries(
            &str_field_default(record, "claim", ""),
            &str_field_default(record, "axis", "claim"),
        )
        .into_iter()
        .find(|item| item.get("label").and_then(Value::as_str) == Some("focused"))
        .and_then(|item| {
            item.get("query")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        })
    {
        return Ok(query);
    }
    bail!("No query available. Run draft-claims first or pass --query.");
}

// ── Single claim research ──

/// Research a single claim (builds its own HTTP client).
pub fn research_claim(
    state: &Value,
    claim_id: Option<&str>,
    explicit_query: Option<&str>,
    source: &ExternalSourceArg,
    limit: usize,
    timeout_secs: u64,
) -> Result<Value> {
    let client = http_client(timeout_secs)?;
    research_claim_with_client(state, claim_id, explicit_query, source, limit, &client)
}

/// Research a single claim using an existing HTTP client.
pub fn research_claim_with_client(
    state: &Value,
    claim_id: Option<&str>,
    explicit_query: Option<&str>,
    source: &ExternalSourceArg,
    limit: usize,
    client: &Client,
) -> Result<Value> {
    let source_record = claim_record_for_research(state, claim_id);
    if let (Some(claim_id), None) = (claim_id, source_record.as_ref()) {
        bail!("Unknown claim id: {claim_id}");
    }
    let query = default_research_query(source_record.as_ref(), explicit_query)?;
    let mut results = Vec::new();
    let mut errors = Vec::new();
    if matches!(
        source,
        ExternalSourceArg::All | ExternalSourceArg::SemanticScholar
    ) {
        match crate::search::semantic_scholar::search(client, &query, limit) {
            Ok(items) => results.extend(items),
            Err(err) => errors.push(format!("semantic-scholar: {err}")),
        }
    }
    if matches!(source, ExternalSourceArg::All | ExternalSourceArg::Arxiv) {
        match crate::search::arxiv::search(client, &query, limit) {
            Ok(items) => results.extend(items),
            Err(err) => errors.push(format!("arxiv: {err}")),
        }
    }
    if results.is_empty() && !errors.is_empty() {
        bail!("External research failed: {}", errors.join("; "));
    }
    // Use nanosecond timestamp for research IDs (64 bits entropy from lower 16 hex chars)
    let research_id = {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        format!("ext-{nanos:016x}")
    };
    Ok(json!({
        "research_id": research_id,
        "claim_id": source_record
            .as_ref()
            .and_then(|item| item.get("claim_id"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        "claim": source_record
            .as_ref()
            .map(|item| str_field_default(item, "claim", "-"))
            .unwrap_or_else(|| explicit_query.unwrap_or("-").to_string()),
        "query": query,
        "source": source.as_str(),
        "results": dedupe_research_results(results),
        "errors": errors,
        "created_at": now_iso(),
    }))
}

// ── Batch research ──

/// Get claim records for batch research.
pub fn claim_records_for_batch(state: &Value, max_claims: usize) -> Vec<Value> {
    let mut records = current_search_plan(state);
    if records.is_empty() {
        if let Some(top) = top_priority_claim(state) {
            records.push(build_search_plan_entry(&top));
        }
    }
    records.into_iter().take(max_claims.clamp(1, 10)).collect()
}

/// Research all claims in parallel using thread workers.
pub fn research_all_claims(
    state: &Value,
    source: &ExternalSourceArg,
    limit: usize,
    max_claims: usize,
    timeout_secs: u64,
) -> Result<Value> {
    let mut next_state = ensure_state_defaults(state);
    let records = claim_records_for_batch(&next_state, max_claims);
    if records.is_empty() {
        bail!("No claims available. Run draft-claims first.");
    }
    // Pre-filter: skip claims that already have matching external research
    let to_process: Vec<Value> = records
        .into_iter()
        .filter(|record| {
            let claim_id = record.get("claim_id").and_then(Value::as_str);
            match default_research_query(Some(record), None) {
                Ok(q) => !has_matching_external_research(&next_state, claim_id, &q, source),
                Err(_) => true,
            }
        })
        .collect();
    if to_process.is_empty() {
        return Ok(next_state);
    }
    let client = Arc::new(http_client(timeout_secs)?);
    let state_ref = Arc::new(next_state.clone());
    let source = source.clone();
    let worker_count = to_process.len().clamp(1, 4);
    let (result_tx, result_rx) = std::sync::mpsc::channel();
    let tasks = Arc::new(to_process);
    let next_idx = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut handles = Vec::with_capacity(worker_count);
    for _ in 0..worker_count {
        let tasks = Arc::clone(&tasks);
        let next_idx = Arc::clone(&next_idx);
        let result_tx = result_tx.clone();
        let client = Arc::clone(&client);
        let state_ref = Arc::clone(&state_ref);
        let source = source.clone();
        handles.push(std::thread::spawn(move || {
            loop {
                let idx = next_idx.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if idx >= tasks.len() {
                    break;
                }
                let record = &tasks[idx];
                let claim_id = record.get("claim_id").and_then(Value::as_str);
                let query = match default_research_query(Some(record), None) {
                    Ok(q) => q,
                    Err(e) => {
                        let _ = result_tx.send(Err(format!("query extraction: {e}")));
                        continue;
                    }
                };
                let result = research_claim_with_client(
                    &state_ref,
                    claim_id,
                    Some(&query),
                    &source,
                    limit,
                    &client,
                )
                .map_err(|e| e.to_string());
                let _ = result_tx.send(result);
            }
        }));
    }
    drop(result_tx);
    let mut errors: Vec<String> = Vec::new();
    for result in result_rx {
        match result {
            Ok(research) => arr_mut(&mut next_state, "external_research").push(research),
            Err(e) => errors.push(e),
        }
    }
    for handle in handles {
        handle.join().ok();
    }
    if !errors.is_empty() && arr(&next_state, "external_research").is_empty() {
        bail!("External research failed: {}", errors.join("; "));
    }
    Ok(next_state)
}

// ── External research management ──

/// Add a completed external research entry to the state.
pub fn add_external_research(state: &Value, research: Value) -> Value {
    let mut next_state = ensure_state_defaults(state);
    arr_mut(&mut next_state, "external_research").push(research);
    next_state
}

/// Get the latest external research entry.
pub fn latest_external_research(state: &Value) -> Option<&Value> {
    arr(state, "external_research").last()
}

/// Count results in an external research entry.
pub fn external_research_result_count(entry: &Value) -> usize {
    entry
        .get("results")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

/// Filter external research entries by claim ID.
pub fn external_research_entries_for_claim<'a>(
    state: &'a Value,
    claim_id: &str,
) -> Vec<&'a Value> {
    arr(state, "external_research")
        .iter()
        .filter(|entry| entry.get("claim_id").and_then(Value::as_str) == Some(claim_id))
        .collect()
}

/// Check if a source covers a requested source.
pub fn source_covers(existing_source: &str, requested_source: &ExternalSourceArg) -> bool {
    match requested_source {
        ExternalSourceArg::All => existing_source == ExternalSourceArg::All.as_str(),
        ExternalSourceArg::SemanticScholar => {
            existing_source == ExternalSourceArg::SemanticScholar.as_str()
                || existing_source == ExternalSourceArg::All.as_str()
        }
        ExternalSourceArg::Arxiv => {
            existing_source == ExternalSourceArg::Arxiv.as_str()
                || existing_source == ExternalSourceArg::All.as_str()
        }
    }
}

/// Check if matching external research already exists for a claim.
pub fn has_matching_external_research(
    state: &Value,
    claim_id: Option<&str>,
    query: &str,
    source: &ExternalSourceArg,
) -> bool {
    let Some(claim_id) = claim_id else {
        return false;
    };
    arr(state, "external_research").iter().any(|entry| {
        entry.get("claim_id").and_then(Value::as_str) == Some(claim_id)
            && entry.get("query").and_then(Value::as_str) == Some(query)
            && entry
                .get("source")
                .and_then(Value::as_str)
                .map(|existing| source_covers(existing, source))
                .unwrap_or(false)
            && external_research_result_count(entry) > 0
    })
}

// ── Claim ID tracking ──

/// Collect all claim IDs from the novelty gate.
pub fn claim_ids_for_gate(state: &Value) -> Vec<String> {
    let mut ids = Vec::new();
    for key in ["claim_records", "draft_claims"] {
        for claim in novelty_gate(state)
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(id) = claim.get("claim_id").and_then(Value::as_str) {
                if !ids.iter().any(|item| item == id) {
                    ids.push(id.to_string());
                }
            }
        }
    }
    ids
}

/// Get IDs of claims that have been compared (have claim_records).
pub fn compared_claim_ids(state: &Value) -> HashSet<String> {
    novelty_arr(state, "claim_records")
        .iter()
        .filter_map(|record| {
            record
                .get("claim_id")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        })
        .collect()
}

// ── Novelty gate recommendation ──

/// Generate a novelty gate recommendation based on external research status.
pub fn novelty_gate_recommendation_from_research(
    state: &Value,
    min_results: usize,
) -> Value {
    let min_results = min_results.max(1);
    let claim_ids = claim_ids_for_gate(state);
    let compared_ids = compared_claim_ids(state);
    let mut reviewed = Vec::new();
    let mut missing = Vec::new();
    let mut uncompared = Vec::new();
    for claim_id in &claim_ids {
        let entries = external_research_entries_for_claim(state, claim_id);
        let result_count: usize = entries
            .iter()
            .map(|entry| external_research_result_count(entry))
            .sum();
        if result_count >= min_results {
            reviewed.push(json!({
                "claim_id": claim_id,
                "searches": entries.len(),
                "results": result_count,
            }));
        } else {
            missing.push(json!({
                "claim_id": claim_id,
                "results": result_count,
                "needed": min_results,
            }));
        }
        if !compared_ids.contains(claim_id) {
            uncompared.push(json!({ "claim_id": claim_id }));
        }
    }
    let existing_records = novelty_arr(state, "claim_records").len();
    let recommended_status =
        if claim_ids.is_empty() || !missing.is_empty() || !uncompared.is_empty() {
            "pending"
        } else {
            "passed"
        };
    let decision = if recommended_status == "passed" {
        "External searches exist for every drafted claim; proceed if manual comparisons are defensible."
    } else if claim_ids.is_empty() {
        "Draft claims before deciding the novelty gate."
    } else if !uncompared.is_empty() {
        "External searches exist, but claim comparisons are still missing."
    } else {
        "Some claims still need external research before the gate can pass."
    };
    json!({
        "recommended_status": recommended_status,
        "decision": decision,
        "reviewed_claims": reviewed,
        "missing_claims": missing,
        "uncompared_claims": uncompared,
        "claim_comparisons": existing_records,
        "generated_at": now_iso(),
    })
}

/// Apply a novelty gate recommendation to the state.
pub fn apply_novelty_gate_recommendation(state: &Value, recommendation: &Value) -> Value {
    let mut next_state = ensure_state_defaults(state);
    let status = str_field_default(recommendation, "recommended_status", "pending");
    let decision = str_field_default(recommendation, "decision", "-");
    let reviewed = recommendation
        .get("reviewed_claims")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    format!(
                        "{}={} results",
                        str_field_default(item, "claim_id", "?"),
                        item.get("results")
                            .map(value_to_string)
                            .unwrap_or_else(|| "0".into())
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let gate = novelty_gate_mut(&mut next_state);
    gate.insert("status".into(), json!(status));
    gate.insert("decision".into(), json!(decision));
    if !reviewed.is_empty() {
        gate.insert("overlap_summary".into(), json!(reviewed));
    }
    next_state
}

/// Format a novelty gate recommendation as human-readable text.
pub fn format_gate_recommendation(recommendation: &Value) -> String {
    let mut lines = vec![
        format!(
            "recommended_status: {}",
            str_field_default(recommendation, "recommended_status", "pending")
        ),
        format!(
            "decision: {}",
            str_field_default(recommendation, "decision", "-")
        ),
        format!(
            "claim_comparisons: {}",
            recommendation
                .get("claim_comparisons")
                .map(value_to_string)
                .unwrap_or_else(|| "0".into())
        ),
        "reviewed_claims:".into(),
    ];
    for item in recommendation
        .get("reviewed_claims")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        lines.push(format!(
            "- {}: {} results",
            str_field_default(item, "claim_id", "?"),
            item.get("results")
                .map(value_to_string)
                .unwrap_or_else(|| "0".into())
        ));
    }
    lines.push("missing_claims:".into());
    for item in recommendation
        .get("missing_claims")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        lines.push(format!(
            "- {}: {} / {} results",
            str_field_default(item, "claim_id", "?"),
            item.get("results")
                .map(value_to_string)
                .unwrap_or_else(|| "0".into()),
            item.get("needed")
                .map(value_to_string)
                .unwrap_or_else(|| "1".into())
        ));
    }
    lines.push("uncompared_claims:".into());
    for item in recommendation
        .get("uncompared_claims")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        lines.push(format!("- {}", str_field_default(item, "claim_id", "?")));
    }
    lines.join("\n")
}

// ── Claim proposal ──

/// Clean question text: remove question prefixes and trailing punctuation.
pub fn cleanup_question_text(question: &str) -> String {
    let trimmed = question.trim().trim_end_matches(['?', '.', '!']);
    static CLEANUP_RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = CLEANUP_RE.get_or_init(|| {
        regex::Regex::new(
            r"(?i)^(can|could|does|do|did|is|are|should|would|will|how|why|what|whether)\s+",
        )
        .expect("invalid CLEANUP_RE regex")
    });
    let cleaned = re.replace(trimmed, "").trim().to_string();
    if cleaned.is_empty() {
        question.trim().to_string()
    } else {
        cleaned
    }
}

/// Extract focus, target, and effect from a research question.
pub fn extract_question_parts(question: &str) -> (String, String, String) {
    let cleaned = cleanup_question_text(question);
    let lowered = cleaned.to_lowercase();
    let mut focus = cleaned.clone();
    let mut target = "the stated task or setting".to_string();
    let mut effect = "a meaningful measurable improvement".to_string();
    static MAIN_RE: OnceLock<regex::Regex> = OnceLock::new();
    let main_re = MAIN_RE.get_or_init(|| {
        regex::Regex::new(
            r"(.+?)\s+(improve|improves|reduce|reduces|increase|increases|enable|enables)\s+(.+)",
        )
        .expect("invalid MAIN_RE regex")
    });
    if let Some(caps) = main_re.captures(&lowered) {
        focus = caps[1].trim().to_string();
        target = caps[3].trim().to_string();
        effect = format!("{} {}", caps[2].trim(), caps[3].trim());
    } else {
        static USING_RE: OnceLock<regex::Regex> = OnceLock::new();
        let using_re =
            USING_RE.get_or_init(|| regex::Regex::new(r"using\s+(.+?)\s+for\s+(.+)").expect("invalid USING_RE regex"));
        if let Some(caps) = using_re.captures(&lowered) {
            focus = caps[1].trim().to_string();
            target = caps[2].trim().to_string();
            effect = format!("improve {target}");
        } else {
            let keywords = compact_words(&cleaned, 8);
            if !keywords.is_empty() {
                focus = keywords.iter().take(4).cloned().collect::<Vec<_>>().join(" ");
                if keywords.len() >= 6 {
                    target = keywords
                        .iter()
                        .skip(4)
                        .take(4)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(" ");
                    effect = format!("improve {target}");
                }
            }
        }
    }
    (focus, target, effect)
}

/// Generate default required evidence for a draft claim.
pub fn default_draft_claim_evidence(axis: &str, focus: &str, target: &str) -> Vec<String> {
    match axis.to_lowercase().as_str() {
        "method" => vec![
            format!("Closest papers using {focus}"),
            "Nearest mechanism-level baselines".into(),
            "Evidence that the mechanism meaningfully differs".into(),
        ],
        "task" => vec![
            format!("Prior work on {focus} for {target}"),
            "Recent task/domain competitors from the last 3 years".into(),
            "Evidence that the task framing is not already saturated".into(),
        ],
        "setting" => vec![
            format!("Papers in the same constrained setting as {target}"),
            "Evidence that the constraint changes the problem materially".into(),
            "Comparable results in adjacent settings".into(),
        ],
        _ => vec![
            "Papers making a similar contribution claim".into(),
            "Reviewer-expected baseline papers".into(),
            "Evidence for the exact differentiation sentence".into(),
        ],
    }
}

/// Propose draft claims from a research question.
pub fn propose_claims_from_question(question: &str, count: usize) -> Vec<Value> {
    let (focus, target, effect) = extract_question_parts(question);
    let templates = vec![
        (
            "method",
            "testable hypothesis",
            format!("Using {focus} is itself a defensible mechanism-level contribution."),
        ),
        (
            "task",
            "direction with concrete benchmark target",
            format!(
                "Applying {focus} to {target} is novel enough to justify a focused study."
            ),
        ),
        (
            "setting",
            "testable hypothesis",
            format!(
                "The value of {focus} depends on the specific setting or constraint around {target}."
            ),
        ),
        (
            "framing",
            "paper-facing positioning claim",
            format!(
                "The strongest paper-facing claim is that {focus} can {effect}, not that it is universally better."
            ),
        ),
        (
            "comparison",
            "reviewer-facing claim",
            format!(
                "The core reviewer question is whether {focus} beats the closest simple baseline for {target}."
            ),
        ),
    ];
    templates
        .into_iter()
        .take(count.clamp(1, 5))
        .enumerate()
        .map(|(index, (axis, specificity, claim))| {
            json!({
                "claim_id": format!("C{}", index + 1),
                "axis": axis,
                "specificity": specificity,
                "claim": claim,
                "required_evidence": default_draft_claim_evidence(axis, &focus, &target),
            })
        })
        .collect()
}

/// Draft claims from state, optionally overriding the question.
pub fn draft_claims_from_state(
    state: &Value,
    question_override: Option<&str>,
    count: usize,
) -> Value {
    let mut next_state = ensure_state_defaults(state);
    let question = question_override
        .map(ToString::to_string)
        .unwrap_or_else(|| str_key(&next_state, "question").to_string());
    let drafts = prioritize_claims(&propose_claims_from_question(&question, count));
    let gate = novelty_gate_mut(&mut next_state);
    gate.insert("draft_claims".into(), json!(drafts));
    let claims = gate
        .get("draft_claims")
        .and_then(Value::as_array)
        .expect("draft_claims just inserted must be an array")
        .iter()
        .map(|draft| str_field(draft, "claim"))
        .collect::<Vec<_>>();
    gate.insert("claims".into(), json!(claims));
    next_state
}

// ── Assessment ──

/// Compute the overall novelty assessment from claim records.
pub fn overall_novelty_assessment(state: &Value) -> &'static str {
    let records = novelty_arr(state, "claim_records");
    if records.is_empty() {
        return "insufficient";
    }
    if records
        .iter()
        .all(|record| record.get("verdict").and_then(Value::as_str) == Some("novel"))
    {
        return "strong";
    }
    let not_novel_count = records
        .iter()
        .filter(|record| record.get("verdict").and_then(Value::as_str) == Some("not-novel"))
        .count();
    if not_novel_count > 0 {
        return if not_novel_count >= 2 {
            "weak"
        } else {
            "moderate"
        };
    }
    if records
        .iter()
        .any(|record| record.get("verdict").and_then(Value::as_str) == Some("risky"))
    {
        return "moderate";
    }
    if records
        .iter()
        .any(|record| record.get("verdict").and_then(Value::as_str) == Some("novel"))
    {
        "strong"
    } else {
        "moderate"
    }
}

/// Find the strongest current claim from claim records.
pub fn strongest_current_claim(state: &Value) -> String {
    let records = novelty_arr(state, "claim_records");
    if !records.is_empty() {
        let verdict_order = HashMap::from([
            ("novel", 0),
            ("defensible", 1),
            ("risky", 2),
            ("not-novel", 3),
        ]);
        let confidence_order = HashMap::from([("high", 0), ("medium", 1), ("low", 2)]);
        let mut ranked = records.to_vec();
        ranked.sort_by(|a, b| {
            let va = verdict_order
                .get(a.get("verdict").and_then(Value::as_str).unwrap_or("risky"))
                .unwrap_or(&9);
            let vb = verdict_order
                .get(b.get("verdict").and_then(Value::as_str).unwrap_or("risky"))
                .unwrap_or(&9);
            let ca = confidence_order
                .get(
                    a.get("confidence")
                        .and_then(Value::as_str)
                        .unwrap_or("medium"),
                )
                .unwrap_or(&1);
            let cb = confidence_order
                .get(
                    b.get("confidence")
                        .and_then(Value::as_str)
                        .unwrap_or("medium"),
                )
                .unwrap_or(&1);
            va.cmp(vb)
                .then_with(|| ca.cmp(cb))
                .then_with(|| {
                    str_field(a, "claim_id").cmp(&str_field(b, "claim_id"))
                })
        });
        return str_field_default(&ranked[0], "claim", "_No strong claim recorded yet._");
    }
    // Fall back to active hypothesis
    if let Some(active_id) = state.get("active_hypothesis").and_then(Value::as_str) {
        if let Some(active) = state
            .get("hypotheses")
            .and_then(Value::as_array)
            .and_then(|arr| {
                arr.iter()
                    .find(|h| h.get("id").and_then(Value::as_str) == Some(active_id))
            })
        {
            return str_field_default(active, "claim", "_No strong claim recorded yet._");
        }
    }
    "_No strong claim recorded yet._".to_string()
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_question_text_removes_prefix() {
        assert_eq!(
            cleanup_question_text("Can you improve performance?"),
            "you improve performance"
        );
    }

    #[test]
    fn cleanup_question_text_removes_punctuation() {
        assert_eq!(
            cleanup_question_text("improve performance."),
            "improve performance"
        );
    }

    #[test]
    fn extract_question_parts_with_using_for() {
        let (focus, target, effect) =
            extract_question_parts("Using attention for image classification");
        assert_eq!(focus, "attention");
        assert_eq!(target, "image classification");
        assert!(effect.contains("improve"));
    }

    #[test]
    fn default_draft_claim_evidence_method() {
        let evidence = default_draft_claim_evidence("method", "transformer", "NLP");
        assert_eq!(evidence.len(), 3);
        assert!(evidence[0].contains("transformer"));
    }

    #[test]
    fn propose_claims_from_question_returns_count() {
        let claims = propose_claims_from_question("How to improve model accuracy?", 3);
        assert_eq!(claims.len(), 3);
        assert_eq!(claims[0]["claim_id"], "C1");
    }

    #[test]
    fn propose_claims_clamps_to_max() {
        assert_eq!(propose_claims_from_question("test", 10).len(), 5);
    }

    #[test]
    fn overall_novelty_assessment_empty() {
        let state = ensure_state_defaults(&json!({}));
        assert_eq!(overall_novelty_assessment(&state), "insufficient");
    }

    #[test]
    fn overall_novelty_assessment_all_novel() {
        let mut state = ensure_state_defaults(&json!({}));
        novelty_gate_mut(&mut state).insert(
            "claim_records".into(),
            json!([{"verdict": "novel"}, {"verdict": "novel"}]),
        );
        assert_eq!(overall_novelty_assessment(&state), "strong");
    }

    #[test]
    fn overall_novelty_assessment_weak() {
        let mut state = ensure_state_defaults(&json!({}));
        novelty_gate_mut(&mut state).insert(
            "claim_records".into(),
            json!([{"verdict": "not-novel"}, {"verdict": "not-novel"}]),
        );
        assert_eq!(overall_novelty_assessment(&state), "weak");
    }

    #[test]
    fn strongest_current_claim_returns_novel() {
        let mut state = ensure_state_defaults(&json!({}));
        novelty_gate_mut(&mut state).insert(
            "claim_records".into(),
            json!([
                {"claim_id": "C1", "verdict": "not-novel", "confidence": "high", "claim": "weak"},
                {"claim_id": "C2", "verdict": "novel", "confidence": "high", "claim": "strong"}
            ]),
        );
        assert_eq!(strongest_current_claim(&state), "strong");
    }

    #[test]
    fn strongest_current_claim_falls_back_to_hypothesis() {
        let state = ensure_state_defaults(&json!({
            "active_hypothesis": "H1",
            "hypotheses": [{"id": "H1", "claim": "hypothesis claim"}]
        }));
        assert_eq!(strongest_current_claim(&state), "hypothesis claim");
    }

    #[test]
    fn source_covers_all() {
        assert!(source_covers("all", &ExternalSourceArg::All));
        assert!(source_covers("all", &ExternalSourceArg::SemanticScholar));
    }

    #[test]
    fn source_covers_specific() {
        assert!(source_covers(
            "semantic-scholar",
            &ExternalSourceArg::SemanticScholar
        ));
        assert!(!source_covers(
            "semantic-scholar",
            &ExternalSourceArg::Arxiv
        ));
    }

    #[test]
    fn has_matching_external_research_finds_match() {
        let state = json!({
            "external_research": [{
                "claim_id": "C1",
                "query": "test",
                "source": "semantic-scholar",
                "results": [{"title": "A"}]
            }]
        });
        assert!(has_matching_external_research(
            &state,
            Some("C1"),
            "test",
            &ExternalSourceArg::SemanticScholar
        ));
    }

    #[test]
    fn has_matching_external_research_no_match() {
        let state = json!({});
        assert!(!has_matching_external_research(
            &state,
            None,
            "q",
            &ExternalSourceArg::All
        ));
    }

    #[test]
    fn claim_ids_for_gate_collects() {
        let mut state = ensure_state_defaults(&json!({}));
        novelty_gate_mut(&mut state).insert(
            "claim_records".into(),
            json!([{"claim_id": "C1"}, {"claim_id": "C2"}]),
        );
        let ids = claim_ids_for_gate(&state);
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn external_research_result_count_basic() {
        let entry = json!({"results": [{"title": "A"}, {"title": "B"}]});
        assert_eq!(external_research_result_count(&entry), 2);
    }

    #[test]
    fn external_research_entries_for_claim_filters() {
        let state = json!({
            "external_research": [
                {"claim_id": "C1", "query": "q1"},
                {"claim_id": "C2", "query": "q2"}
            ]
        });
        assert_eq!(external_research_entries_for_claim(&state, "C1").len(), 1);
    }

    #[test]
    fn novelty_gate_recommendation_pending_when_empty() {
        let state = ensure_state_defaults(&json!({}));
        let rec = novelty_gate_recommendation_from_research(&state, 1);
        assert_eq!(
            rec.get("recommended_status").and_then(Value::as_str),
            Some("pending")
        );
    }

    #[test]
    fn format_gate_recommendation_basic() {
        let rec = json!({
            "recommended_status": "passed",
            "decision": "All reviewed",
            "claim_comparisons": 3,
            "reviewed_claims": [{"claim_id": "C1", "results": 10}],
            "missing_claims": [],
            "uncompared_claims": []
        });
        let formatted = format_gate_recommendation(&rec);
        assert!(formatted.contains("recommended_status: passed"));
        assert!(formatted.contains("C1: 10 results"));
    }

    #[test]
    fn default_research_query_uses_explicit() {
        let q = default_research_query(None, Some("  my query  ")).unwrap();
        assert_eq!(q, "my query");
    }

    #[test]
    fn default_research_query_fails_without_input() {
        assert!(default_research_query(None, None).is_err());
    }

    #[test]
    fn draft_claims_from_state_creates_drafts() {
        let state = json!({"question": "How to improve X?"});
        let result = draft_claims_from_state(&state, None, 3);
        let drafts = result
            .get("novelty_gate")
            .and_then(|g| g.get("draft_claims"))
            .and_then(Value::as_array)
            .unwrap();
        assert_eq!(drafts.len(), 3);
    }
}
