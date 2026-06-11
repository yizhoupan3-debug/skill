//! External research: HTTP clients, Semantic Scholar/arXiv adapters, claim research,
//! novelty gate recommendation from research.

use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, USER_AGENT};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::time::Duration;
use uuid::Uuid;

use crate::*;

pub(super) fn http_client(timeout_secs: u64) -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(timeout_secs.clamp(3, 120)))
        .build()
        .context("failed to build HTTP client")
}

pub(super) fn fetch_semantic_scholar(client: &Client, query: &str, limit: usize) -> Result<Vec<Value>> {
    let response: Value = client
        .get(SEMANTIC_SCHOLAR_BASE_URL)
        .header(USER_AGENT, "autoresearch-rs/0.1")
        .header(ACCEPT, "application/json")
        .query(&[
            ("query", query),
            (
                "fields",
                "title,authors,year,venue,url,abstract,citationCount,externalIds",
            ),
            ("limit", &normalize_limit(limit).to_string()),
        ])
        .send()
        .context("Semantic Scholar request failed")?
        .error_for_status()
        .context("Semantic Scholar returned an error")?
        .json()
        .context("Semantic Scholar returned invalid JSON")?;
    let mut results = Vec::new();
    for item in response
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    {
        let authors = item
            .get("authors")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(|author| author.get("name").and_then(Value::as_str))
                    .take(4)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        results.push(json!({
            "source": "Semantic Scholar",
            "title": str_field_default(&item, "title", "_untitled_"),
            "authors": authors,
            "year": item.get("year").cloned().unwrap_or(Value::Null),
            "venue": item.get("venue").cloned().unwrap_or(Value::Null),
            "url": item.get("url").cloned().unwrap_or(Value::Null),
            "abstract": item.get("abstract").cloned().unwrap_or(Value::Null),
            "citation_count": item.get("citationCount").cloned().unwrap_or(Value::Null),
            "external_ids": item.get("externalIds").cloned().unwrap_or(Value::Null),
        }));
    }
    Ok(results)
}

pub(super) fn fetch_arxiv(client: &Client, query: &str, limit: usize) -> Result<Vec<Value>> {
    let raw = client
        .get(ARXIV_BASE_URL)
        .header(USER_AGENT, "autoresearch-rs/0.1")
        .query(&[
            ("search_query", format!("all:{query}")),
            ("start", "0".to_string()),
            ("max_results", normalize_limit(limit).to_string()),
            ("sortBy", "relevance".to_string()),
            ("sortOrder", "descending".to_string()),
        ])
        .send()
        .context("arXiv request failed")?
        .error_for_status()
        .context("arXiv returned an error")?
        .text()
        .context("arXiv returned invalid text")?;
    let entry_re = &*ARXIV_ENTRY_RE;
    let author_re = &*ARXIV_AUTHOR_RE;
    let mut results = Vec::new();
    for entry in entry_re.captures_iter(&raw) {
        let entry_raw = entry.get(1).map(|item| item.as_str()).unwrap_or("");
        let authors = author_re
            .captures_iter(entry_raw)
            .filter_map(|cap| cap.get(1).map(|item| decode_xml_entities(item.as_str())))
            .take(4)
            .collect::<Vec<_>>()
            .join(", ");
        results.push(json!({
            "source": "arXiv",
            "title": xml_text_between(entry_raw, "title").unwrap_or_else(|| "_untitled_".into()),
            "authors": authors,
            "year": xml_text_between(entry_raw, "published").map(|date| date.chars().take(4).collect::<String>()).unwrap_or_default(),
            "venue": "arXiv",
            "url": xml_text_between(entry_raw, "id").unwrap_or_default(),
            "abstract": xml_text_between(entry_raw, "summary").unwrap_or_default(),
            "citation_count": Value::Null,
            "external_ids": Value::Null,
        }));
    }
    Ok(results)
}

pub(super) fn dedupe_research_results(results: Vec<Value>) -> Vec<Value> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for result in results {
        let key = format!(
            "{}::{}",
            str_field_default(&result, "source", "-").to_lowercase(),
            str_field_default(&result, "title", "-").to_lowercase()
        );
        if seen.insert(key) {
            deduped.push(result);
        }
    }
    deduped
}

pub(super) fn claim_record_for_research(state: &Value, claim_id: Option<&str>) -> Option<Value> {
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

pub(super) fn default_research_query(record: Option<&Value>, explicit_query: Option<&str>) -> Result<String> {
    if let Some(query) = explicit_query
        .map(str::trim)
        .filter(|query| !query.is_empty())
    {
        return Ok(query.to_string());
    }
    if let Some(record) = record {
        if let Some(query) = build_search_queries(
            &str_field_default(record, "claim", ""),
            &str_field_default(record, "axis", "claim"),
        )
        .into_iter()
        .find(|item| item.get("label").and_then(Value::as_str) == Some("focused"))
        .and_then(|item| {
            item.get("query")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        }) {
            return Ok(query);
        }
    }
    bail!("No query available. Run draft-claims first or pass --query.");
}

pub(super) fn research_claim(
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

pub(super) fn research_claim_with_client(
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
        match fetch_semantic_scholar(client, &query, limit) {
            Ok(items) => results.extend(items),
            Err(err) => errors.push(format!("semantic-scholar: {err}")),
        }
    }
    if matches!(source, ExternalSourceArg::All | ExternalSourceArg::Arxiv) {
        match fetch_arxiv(client, &query, limit) {
            Ok(items) => results.extend(items),
            Err(err) => errors.push(format!("arxiv: {err}")),
        }
    }
    if results.is_empty() && !errors.is_empty() {
        bail!("External research failed: {}", errors.join("; "));
    }
    Ok(json!({
        "research_id": format!("ext-{}", Uuid::new_v4().simple().to_string().chars().take(8).collect::<String>()),
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

pub(super) fn claim_records_for_batch(state: &Value, max_claims: usize) -> Vec<Value> {
    let mut records = current_search_plan(state);
    if records.is_empty() {
        if let Some(top) = top_priority_claim(state) {
            records.push(build_search_plan_entry(&top));
        }
    }
    records.into_iter().take(max_claims.clamp(1, 10)).collect()
}

pub(super) fn research_all_claims(
    state: &Value,
    source: &ExternalSourceArg,
    limit: usize,
    max_claims: usize,
    timeout_secs: u64,
) -> Result<Value> {
    use std::sync::{mpsc, Arc};
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
    let worker_count = to_process.len().min(4).max(1);
    let (result_tx, result_rx) = mpsc::channel();
    let tasks = Arc::new(to_process);
    let next_idx = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    // Spawn worker threads — each claims work via atomic index (no Mutex contention)
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
                let query = match default_research_query(Some(&record), None) {
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
    drop(result_tx); // drop original sender; workers hold clones
    // Collect results: blocks until all worker clones are dropped
    let mut errors: Vec<String> = Vec::new();
    for result in result_rx {
        match result {
            Ok(research) => arr_mut(&mut next_state, "external_research").push(research),
            Err(e) => errors.push(e.to_string()),
        }
    }
    // All result_tx clones dropped (workers finished), now join
    for handle in handles {
        handle.join().ok();
    }
    if !errors.is_empty() && arr(&next_state, "external_research").is_empty() {
        bail!("External research failed: {}", errors.join("; "));
    }
    Ok(next_state)
}

pub(super) fn add_external_research(state: &Value, research: Value) -> Value {
    let mut next_state = ensure_state_defaults(state);
    arr_mut(&mut next_state, "external_research").push(research);
    next_state
}

pub(super) fn latest_external_research(state: &Value) -> Option<&Value> {
    arr(state, "external_research").last()
}

pub(super) fn external_research_result_count(entry: &Value) -> usize {
    entry
        .get("results")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

pub(super) fn external_research_entries_for_claim<'a>(state: &'a Value, claim_id: &str) -> Vec<&'a Value> {
    arr(state, "external_research")
        .iter()
        .filter(|entry| entry.get("claim_id").and_then(Value::as_str) == Some(claim_id))
        .collect()
}

pub(super) fn source_covers(existing_source: &str, requested_source: &ExternalSourceArg) -> bool {
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

pub(super) fn has_matching_external_research(
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

pub(super) fn claim_ids_for_gate(state: &Value) -> Vec<String> {
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

pub(super) fn compared_claim_ids(state: &Value) -> HashSet<String> {
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

pub(super) fn novelty_gate_recommendation_from_research(state: &Value, min_results: usize) -> Value {
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

pub(super) fn apply_novelty_gate_recommendation(state: &Value, recommendation: &Value) -> Value {
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

pub(super) fn format_gate_recommendation(recommendation: &Value) -> String {
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

pub(super) fn cleanup_question_text(question: &str) -> String {
    let trimmed = question.trim().trim_end_matches(['?', '.', '!']);
    let re = Regex::new(
        r"(?i)^(can|could|does|do|did|is|are|should|would|will|how|why|what|whether)\s+",
    )
    .unwrap();
    let cleaned = re.replace(trimmed, "").trim().to_string();
    if cleaned.is_empty() {
        question.trim().to_string()
    } else {
        cleaned
    }
}

pub(super) fn extract_question_parts(question: &str) -> (String, String, String) {
    let cleaned = cleanup_question_text(question);
    let lowered = cleaned.to_lowercase();
    let mut focus = cleaned.clone();
    let mut target = "the stated task or setting".to_string();
    let mut effect = "a meaningful measurable improvement".to_string();
    let main_re = Regex::new(
        r"(.+?)\s+(improve|improves|reduce|reduces|increase|increases|enable|enables)\s+(.+)",
    )
    .unwrap();
    if let Some(caps) = main_re.captures(&lowered) {
        focus = caps[1].trim().to_string();
        target = caps[3].trim().to_string();
        effect = format!("{} {}", caps[2].trim(), caps[3].trim());
    } else {
        let using_re = Regex::new(r"using\s+(.+?)\s+for\s+(.+)").unwrap();
        if let Some(caps) = using_re.captures(&lowered) {
            focus = caps[1].trim().to_string();
            target = caps[2].trim().to_string();
            effect = format!("improve {target}");
        } else {
            let keywords = compact_words(&cleaned, 8);
            if !keywords.is_empty() {
                focus = keywords
                    .iter()
                    .take(4)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" ");
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

pub(super) fn default_draft_claim_evidence(axis: &str, focus: &str, target: &str) -> Vec<String> {
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

pub(super) fn propose_claims_from_question(question: &str, count: usize) -> Vec<Value> {
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
            format!("Applying {focus} to {target} is novel enough to justify a focused study."),
        ),
        (
            "setting",
            "testable hypothesis",
            format!("The value of {focus} depends on the specific setting or constraint around {target}."),
        ),
        (
            "framing",
            "paper-facing positioning claim",
            format!("The strongest paper-facing claim is that {focus} can {effect}, not that it is universally better."),
        ),
        (
            "comparison",
            "reviewer-facing claim",
            format!("The core reviewer question is whether {focus} beats the closest simple baseline for {target}."),
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

pub(super) fn draft_claims_from_state(state: &Value, question_override: Option<&str>, count: usize) -> Value {
    let mut next_state = ensure_state_defaults(state);
    let question = question_override
        .map(ToString::to_string)
        .unwrap_or_else(|| str_key(&next_state, "question"));
    let drafts = prioritize_claims(&propose_claims_from_question(&question, count));
    let gate = novelty_gate_mut(&mut next_state);
    gate.insert("draft_claims".into(), json!(drafts));
    let claims = gate
        .get("draft_claims")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .map(|draft| str_field(draft, "claim"))
        .collect::<Vec<_>>();
    gate.insert("claims".into(), json!(claims));
    next_state
}

pub(super) fn overall_novelty_assessment(state: &Value) -> &'static str {
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

pub(super) fn strongest_current_claim(state: &Value) -> String {
    let records = novelty_arr(state, "claim_records");
    if !records.is_empty() {
        let verdict_order = HashMap::from([
            ("novel", 0),
            ("defensible", 1),
            ("risky", 2),
            ("not-novel", 3),
        ]);
        let confidence_order = HashMap::from([("high", 0), ("medium", 1), ("low", 2)]);
        let mut ranked = records.clone();
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
                .then_with(|| str_field(a, "claim_id").cmp(&str_field(b, "claim_id")))
        });
        return str_field_default(&ranked[0], "claim", "_No strong claim recorded yet._");
    }
    if let Some(active_id) = state.get("active_hypothesis").and_then(Value::as_str) {
        if let Some(active) = find_hypothesis(state, active_id) {
            return str_field_default(active, "claim", "_No strong claim recorded yet._");
        }
    }
    "_No strong claim recorded yet._".to_string()
}
