use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::constants::*;
use crate::helpers::*;

// ── search queries & claim priority ───────────────────────────────────

pub(crate) fn default_required_evidence(axis: &str) -> Vec<String> {
    let axis_lower = axis.to_lowercase();
    if axis_lower.contains("method") || axis_lower.contains("workflow") {
        return vec![
            "Direct overlap papers using the same mechanism".into(),
            "Nearest baseline implementations or orchestration frameworks".into(),
            "Claims about what is structurally different".into(),
        ];
    }
    if axis_lower.contains("setting")
        || axis_lower.contains("domain")
        || axis_lower.contains("task")
    {
        return vec![
            "Prior work in the same domain or task".into(),
            "Recent competitors in the last 3 years".into(),
            "Evidence that the constraint or setting is materially different".into(),
        ];
    }
    if axis_lower.contains("combination") {
        return vec![
            "Papers combining the same building blocks".into(),
            "Closest papers combining two of the three components".into(),
            "Evidence that the composition order or objective is different".into(),
        ];
    }
    vec![
        "Closest prior work for the same core claim".into(),
        "Recent competitors from the last 3 years".into(),
        "Evidence for the exact differentiation sentence".into(),
    ]
}

pub(crate) fn build_search_queries(claim: &str, axis: &str) -> Vec<Value> {
    let keywords = compact_words(claim, 6);
    let broad_terms = if keywords.is_empty() {
        claim.to_string()
    } else {
        keywords
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let focused_terms = if keywords.is_empty() {
        claim.to_string()
    } else {
        keywords
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let combination_terms = if keywords.len() >= 4 {
        [
            keywords[0].clone(),
            keywords[1].clone(),
            keywords[keywords.len() - 2].clone(),
            keywords[keywords.len() - 1].clone(),
        ]
        .join(" ")
    } else {
        focused_terms.clone()
    };
    let axis_hint = if axis.trim().is_empty() {
        "claim".to_string()
    } else {
        axis.trim().to_lowercase()
    };
    vec![
        json!({"label": "broad", "query": broad_terms}),
        json!({"label": "focused", "query": format!("{focused_terms} {axis_hint}").trim().to_string()}),
        json!({"label": "recent", "query": format!("{focused_terms} last 3 years").trim().to_string()}),
        json!({"label": "combination", "query": combination_terms}),
    ]
}

pub(crate) fn axis_weights(axis: &str) -> (i64, i64, i64) {
    match axis {
        "method" => (5, 2, 3),
        "workflow" => (5, 2, 3),
        "task" => (4, 3, 4),
        "comparison" => (4, 1, 5),
        "setting" => (3, 4, 2),
        "framing" => (2, 1, 4),
        _ => (3, 2, 3),
    }
}

pub(crate) fn score_claim_priority(record: &Value) -> Value {
    let axis = str_field_default(record, "axis", "claim").to_lowercase();
    let (mut novelty, mut cost, mut reviewer) = axis_weights(&axis);
    match record.get("overlap").and_then(Value::as_str) {
        Some("low") => novelty += 2,
        Some("medium") => novelty += 1,
        Some("high") => {
            novelty -= 1;
            reviewer += 1;
        }
        _ => {}
    }
    match record.get("confidence").and_then(Value::as_str) {
        Some("high") => cost -= 1,
        Some("low") => cost += 1,
        _ => {}
    }
    match record.get("verdict").and_then(Value::as_str) {
        Some("novel") => novelty += 2,
        Some("defensible") => novelty += 1,
        Some("risky") => {
            reviewer += 1;
            cost += 1;
        }
        Some("not-novel") => {
            novelty -= 2;
            cost += 1;
        }
        _ => {}
    }
    let specificity = str_field(record, "specificity").to_lowercase();
    if specificity.contains("testable") {
        cost -= 1;
    }
    if specificity.contains("paper-facing") {
        reviewer += 1;
    }
    let score = novelty * 3 + reviewer * 2 - cost * 2;
    let label = if score >= 18 {
        "first"
    } else if score >= 13 {
        "next"
    } else {
        "later"
    };
    let reason = if novelty >= reviewer && cost <= 2 {
        "high novelty upside with relatively cheap verification"
    } else if reviewer >= novelty && cost <= 3 {
        "reviewer pressure is high, so checking this early reduces risk"
    } else if cost >= 4 {
        "potentially useful, but verification is expensive"
    } else {
        "worth checking, but not the best first search target"
    };
    let mut out = record.clone();
    let map = out.as_object_mut().expect("claim record must be object");
    map.insert("priority_score".into(), json!(score));
    map.insert("priority_label".into(), json!(label));
    map.insert("priority_reason".into(), json!(reason));
    out
}

pub(crate) fn prioritize_claims(claims: &[Value]) -> Vec<Value> {
    let mut scored: Vec<Value> = claims.iter().map(score_claim_priority).collect();
    scored.sort_by(|a, b| {
        let score_a = a.get("priority_score").and_then(Value::as_i64).unwrap_or(0);
        let score_b = b.get("priority_score").and_then(Value::as_i64).unwrap_or(0);
        score_b
            .cmp(&score_a)
            .then_with(|| str_field(a, "claim_id").cmp(&str_field(b, "claim_id")))
    });
    for (index, item) in scored.iter_mut().enumerate() {
        item.as_object_mut()
            .unwrap()
            .insert("recommended_order".into(), json!(index + 1));
    }
    scored
}

// ── claim selection ───────────────────────────────────────────────────

pub(crate) fn top_priority_claim(state: &Value) -> Option<Value> {
    for key in ["claim_records", "draft_claims"] {
        let entries = novelty_gate(state)
            .get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if entries.is_empty() {
            continue;
        }
        let mut ranked = entries;
        ranked.sort_by(|a, b| {
            let order_a = a
                .get("recommended_order")
                .and_then(Value::as_i64)
                .unwrap_or(999);
            let order_b = b
                .get("recommended_order")
                .and_then(Value::as_i64)
                .unwrap_or(999);
            let score_a = a.get("priority_score").and_then(Value::as_i64).unwrap_or(0);
            let score_b = b.get("priority_score").and_then(Value::as_i64).unwrap_or(0);
            order_a
                .cmp(&order_b)
                .then_with(|| score_b.cmp(&score_a))
                .then_with(|| str_field(a, "claim_id").cmp(&str_field(b, "claim_id")))
        });
        return ranked.into_iter().next();
    }
    None
}

pub(crate) fn current_recommended_focus(state: &Value) -> Option<String> {
    let claim = top_priority_claim(state)?;
    Some(format!(
        "{}: {}",
        str_field_default(&claim, "claim_id", "C?"),
        str_field_default(&claim, "claim", "_No claim recorded._")
    ))
}

pub(crate) fn claim_ids_for_gate(state: &Value) -> Vec<String> {
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

pub(crate) fn compared_claim_ids(state: &Value) -> std::collections::HashSet<String> {
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

// ── search plan ───────────────────────────────────────────────────────

fn expected_baselines_for_axis(axis: &str) -> Vec<String> {
    match axis.to_lowercase().as_str() {
        "method" | "workflow" => vec![
            "Closest simple baseline implementation".into(),
            "Nearest orchestration or workflow framework baseline".into(),
            "A stripped-down version without the claimed mechanism".into(),
        ],
        "task" => vec![
            "Closest task-specific prior method".into(),
            "Simple transfer baseline without the claimed novelty".into(),
            "Recent strongest competitor from the last 3 years".into(),
        ],
        "setting" => vec![
            "Same method in an adjacent setting".into(),
            "Simple baseline in the same constraint".into(),
            "Closest unconstrained baseline to show what the setting changes".into(),
        ],
        "comparison" => vec![
            "Closest simple baseline the reviewer will ask about first".into(),
            "A stronger but obvious comparator".into(),
            "An ablated version removing the claimed differentiator".into(),
        ],
        "framing" => vec![
            "Closest paper making a similar framing claim".into(),
            "A simpler framing that could explain the same result".into(),
            "The baseline narrative a reviewer would default to".into(),
        ],
        _ => vec![
            "Closest prior work the reviewer will expect".into(),
            "A simple baseline explanation".into(),
            "The strongest recent competitor in the same area".into(),
        ],
    }
}

fn verification_standard_for_priority(label: &str) -> &'static str {
    match label {
        "first" => "You should be able to decide proceed vs reframe after one focused search pass.",
        "next" => "This should be checked after the first claim is clarified, not before.",
        _ => "Useful later, but not strong enough to spend the first search budget on.",
    }
}

pub(crate) fn build_search_plan_entry(record: &Value) -> Value {
    let claim = str_field(record, "claim");
    let axis = str_field_default(record, "axis", "claim");
    json!({
        "claim_id": str_field_default(record, "claim_id", "C?"),
        "claim": claim,
        "axis": axis,
        "priority_score": record.get("priority_score").cloned().unwrap_or(Value::Null),
        "priority_label": record.get("priority_label").cloned().unwrap_or(Value::Null),
        "priority_reason": record.get("priority_reason").cloned().unwrap_or(Value::Null),
        "recommended_order": record.get("recommended_order").cloned().unwrap_or(Value::Null),
        "keywords": compact_words(&claim, 6),
        "queries": build_search_queries(&claim, &axis),
        "sources": ["Semantic Scholar", "arXiv", "Google Scholar"],
        "required_evidence": default_required_evidence(&axis),
    })
}

pub(crate) fn current_search_plan(state: &Value) -> Vec<Value> {
    let source_records = if !novelty_arr(state, "claim_records").is_empty() {
        novelty_arr(state, "claim_records").clone()
    } else {
        novelty_arr(state, "draft_claims").clone()
    };
    let mut plan: Vec<Value> = source_records.iter().map(build_search_plan_entry).collect();
    plan.sort_by(|a, b| {
        let order_a = a
            .get("recommended_order")
            .and_then(Value::as_i64)
            .unwrap_or(999);
        let order_b = b
            .get("recommended_order")
            .and_then(Value::as_i64)
            .unwrap_or(999);
        let score_a = a.get("priority_score").and_then(Value::as_i64).unwrap_or(0);
        let score_b = b.get("priority_score").and_then(Value::as_i64).unwrap_or(0);
        order_a
            .cmp(&order_b)
            .then_with(|| score_b.cmp(&score_a))
            .then_with(|| str_field(a, "claim_id").cmp(&str_field(b, "claim_id")))
    });
    plan
}

pub(crate) fn refresh_novelty_views(state: &mut Value) {
    let search_plan = current_search_plan(state);
    let recommended_focus = current_recommended_focus(state);
    let brief = current_brief(state);
    let gate = novelty_gate_mut(state);
    gate.insert("search_plan".into(), json!(search_plan));
    gate.insert(
        "recommended_focus".into(),
        recommended_focus.map_or(Value::Null, Value::String),
    );
    gate.insert("brief".into(), brief.unwrap_or(Value::Null));
}

pub(crate) fn current_brief(state: &Value) -> Option<Value> {
    let top = top_priority_claim(state)?;
    let plan = current_search_plan(state);
    let matching = plan
        .iter()
        .find(|entry| entry.get("claim_id") == top.get("claim_id"));
    let axis = str_field_default(&top, "axis", "claim");
    Some(json!({
        "claim_id": str_field_default(&top, "claim_id", "C?"),
        "claim": str_field_default(&top, "claim", "_No claim recorded._"),
        "axis": axis,
        "priority_label": str_field_default(&top, "priority_label", "later"),
        "priority_score": top.get("priority_score").cloned().unwrap_or(json!(0)),
        "priority_reason": str_field_default(&top, "priority_reason", "_No reason recorded._"),
        "decision_goal": "Decide whether this claim is safe to keep, should be reframed, or should be dropped.",
        "verification_standard": verification_standard_for_priority(&str_field_default(&top, "priority_label", "later")),
        "sources": matching.and_then(|item| item.get("sources")).cloned().unwrap_or(json!(["Semantic Scholar", "arXiv", "Google Scholar"])),
        "queries": matching.and_then(|item| item.get("queries")).cloned().unwrap_or_else(|| json!(build_search_queries(&str_field(&top, "claim"), &axis))),
        "required_evidence": matching.and_then(|item| item.get("required_evidence")).cloned().unwrap_or_else(|| json!(default_required_evidence(&axis))),
        "expected_baselines": expected_baselines_for_axis(&axis),
    }))
}

// ── claim generation ──────────────────────────────────────────────────

fn cleanup_question_text(question: &str) -> String {
    let trimmed = question.trim().trim_end_matches(['?', '.', '!']);
    let re = regex::Regex::new(
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

fn extract_question_parts(question: &str) -> (String, String, String) {
    let cleaned = cleanup_question_text(question);
    let lowered = cleaned.to_lowercase();
    let mut focus = cleaned.clone();
    let mut target = "the stated task or setting".to_string();
    let mut effect = "a meaningful measurable improvement".to_string();
    let main_re = regex::Regex::new(
        r"(.+?)\s+(improve|improves|reduce|reduces|increase|increases|enable|enables)\s+(.+)",
    )
    .unwrap();
    if let Some(caps) = main_re.captures(&lowered) {
        focus = caps[1].trim().to_string();
        target = caps[3].trim().to_string();
        effect = format!("{} {}", caps[2].trim(), caps[3].trim());
    } else {
        let using_re = regex::Regex::new(r"using\s+(.+?)\s+for\s+(.+)").unwrap();
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

fn default_draft_claim_evidence(axis: &str, focus: &str, target: &str) -> Vec<String> {
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

fn propose_claims_from_question(question: &str, count: usize) -> Vec<Value> {
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

pub(crate) fn draft_claims_from_state(
    state: &Value,
    question_override: Option<&str>,
    count: usize,
) -> Value {
    let mut next_state = crate::state::ensure_state_defaults(state);
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

// ── novelty assessment ────────────────────────────────────────────────

pub(crate) fn overall_novelty_assessment(state: &Value) -> &'static str {
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

pub(crate) fn strongest_current_claim(state: &Value) -> String {
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

// ── context / freshness ───────────────────────────────────────────────

fn sort_entries_by_recency(entries: &[Value], timestamp_field: &str) -> Vec<Value> {
    let mut sorted = entries.to_vec();
    sorted.sort_by(|a, b| {
        let ta =
            parse_iso_timestamp(&str_field(a, timestamp_field)).unwrap_or(DateTime::<Utc>::MIN_UTC);
        let tb =
            parse_iso_timestamp(&str_field(b, timestamp_field)).unwrap_or(DateTime::<Utc>::MIN_UTC);
        tb.cmp(&ta)
    });
    sorted
}

fn recent_entries(
    entries: &[Value],
    timestamp_field: &str,
    max_age_days: i64,
    limit: usize,
    hypothesis_id: Option<&str>,
) -> Vec<Value> {
    let mut filtered = Vec::new();
    for entry in sort_entries_by_recency(entries, timestamp_field) {
        if let Some(hypothesis_id) = hypothesis_id {
            if entry.get("hypothesis_id").and_then(Value::as_str) != Some(hypothesis_id) {
                continue;
            }
        }
        let age = days_since(&str_field(&entry, timestamp_field));
        if age.is_none() || age.unwrap() > max_age_days {
            continue;
        }
        filtered.push(entry);
        if filtered.len() >= limit {
            break;
        }
    }
    filtered
}

pub(crate) fn current_context_runs(state: &Value) -> Vec<Value> {
    let runs = arr(state, "run_history");
    let active_id = state.get("active_hypothesis").and_then(Value::as_str);
    if let Some(active_id) = active_id {
        let active_recent = recent_entries(
            runs,
            "recorded_at",
            RECENT_ACTIVITY_DAYS,
            FALLBACK_ACTIVITY_LIMIT,
            Some(active_id),
        );
        if !active_recent.is_empty() {
            return active_recent;
        }
    }
    let global_recent = recent_entries(
        runs,
        "recorded_at",
        RECENT_ACTIVITY_DAYS,
        FALLBACK_ACTIVITY_LIMIT,
        None,
    );
    if !global_recent.is_empty() {
        return global_recent;
    }
    sort_entries_by_recency(runs, "recorded_at")
        .into_iter()
        .take(FALLBACK_ACTIVITY_LIMIT)
        .collect()
}

fn current_context_decisions(state: &Value) -> Vec<Value> {
    let decisions = arr(state, "decisions");
    let active_id = state.get("active_hypothesis").and_then(Value::as_str);
    if let Some(active_id) = active_id {
        let active_recent = recent_entries(
            decisions,
            "recorded_at",
            RECENT_ACTIVITY_DAYS,
            FALLBACK_ACTIVITY_LIMIT,
            Some(active_id),
        );
        if !active_recent.is_empty() {
            return active_recent;
        }
    }
    let global_recent = recent_entries(
        decisions,
        "recorded_at",
        RECENT_ACTIVITY_DAYS,
        FALLBACK_ACTIVITY_LIMIT,
        None,
    );
    if !global_recent.is_empty() {
        return global_recent;
    }
    sort_entries_by_recency(decisions, "recorded_at")
        .into_iter()
        .take(FALLBACK_ACTIVITY_LIMIT)
        .collect()
}

pub(crate) fn reusable_runs(state: &Value) -> Vec<Value> {
    sort_entries_by_recency(arr(state, "run_history"), "recorded_at")
        .into_iter()
        .filter(|record| {
            !str_field_default(record, "finding", "").is_empty()
                || !str_field_default(record, "decision_delta", "").is_empty()
                || !str_field_default(record, "reuse_note", "").is_empty()
                || !value_as_string_list(record, "applies_to").is_empty()
                || !value_as_string_list(record, "does_not_apply_to").is_empty()
        })
        .collect()
}

pub(crate) fn missing_reuse_annotation_runs(state: &Value) -> Vec<Value> {
    sort_entries_by_recency(arr(state, "run_history"), "recorded_at")
        .into_iter()
        .filter(|record| {
            str_field_default(record, "finding", "").is_empty()
                || str_field_default(record, "decision_delta", "").is_empty()
                || str_field_default(record, "reuse_note", "").is_empty()
        })
        .collect()
}

pub(crate) fn reuse_audit(state: &Value) -> Value {
    let reusable = reusable_runs(state);
    let missing = missing_reuse_annotation_runs(state);
    json!({
        "runs": arr(state, "run_history").len(),
        "reusable_runs": reusable.len(),
        "missing_annotations": missing.len(),
        "missing_runs": missing
            .iter()
            .take(20)
            .map(|record| {
                let missing_fields = [
                    ("finding", str_field_default(record, "finding", "").is_empty()),
                    (
                        "decision_delta",
                        str_field_default(record, "decision_delta", "").is_empty(),
                    ),
                    (
                        "reuse_note",
                        str_field_default(record, "reuse_note", "").is_empty(),
                    ),
                ]
                .into_iter()
                .filter_map(|(field, missing)| missing.then_some(field))
                .collect::<Vec<_>>();
                json!({
                    "run_id": str_field(record, "run_id"),
                    "summary": str_field_default(record, "summary", "-"),
                    "missing": missing_fields,
                })
            })
            .collect::<Vec<_>>(),
        "generated_at": now_iso(),
    })
}

struct Freshness {
    stale: bool,
    history_bias_risk: bool,
    recent_runs: Vec<Value>,
    recent_decisions: Vec<Value>,
}

fn state_freshness(state: &Value) -> Freshness {
    let updated_days = days_since(&str_key(state, "updated_at"));
    let recent_runs = current_context_runs(state);
    let recent_decisions = current_context_decisions(state);
    let stale = updated_days.is_some_and(|days| days > STALE_STATE_DAYS);
    let history_bias_risk = stale
        || ((!arr(state, "run_history").is_empty() || !arr(state, "decisions").is_empty())
            && recent_runs.is_empty()
            && recent_decisions.is_empty());
    Freshness {
        stale,
        history_bias_risk,
        recent_runs,
        recent_decisions,
    }
}

// ── recommend next actions ────────────────────────────────────────────

pub(crate) fn recommend_next_actions(state: &Value) -> Vec<String> {
    let freshness = state_freshness(state);
    if freshness.history_bias_risk {
        return vec![
            "\u{5148}\u{5237}\u{65b0}\u{5f53}\u{524d}\u{4e0a}\u{4e0b}\u{6587}\u{ff1a}\u{786e}\u{8ba4} active hypothesis \u{548c}\u{5f53}\u{524d}\u{76ee}\u{6807}\u{ff0c}\u{65e7}\u{65e5}\u{5fd7}\u{53ea}\u{5f53}\u{80cc}\u{666f}\u{3002}".into(),
            "\u{5148}\u{770b} CURRENT_CONTEXT.md \u{548c} research-state.yaml\u{ff0c}\u{4e0d}\u{8981}\u{76f4}\u{63a5}\u{6cbf}\u{7528}\u{66f4}\u{65e9}\u{7684} findings \u{6216} research-log \u{7ed3}\u{8bba}\u{3002}".into(),
            "\u{91cd}\u{67e5}\u{4e00}\u{904d}\u{5f53}\u{524d}\u{4ee3}\u{7801}\u{3001}\u{6570}\u{636e}\u{6216}\u{6700}\u{65b0}\u{5b9e}\u{9a8c}\u{8f93}\u{51fa}\u{ff0c}\u{518d}\u{51b3}\u{5b9a}\u{8981}\u{4e0d}\u{8981}\u{7ee7}\u{7eed}\u{65e7}\u{65b9}\u{5411}\u{3002}".into(),
        ];
    }
    if state.get("status").and_then(Value::as_str) == Some("concluded") {
        return vec![
            "Freeze the final narrative in findings.md and to_human/.".into(),
            "Archive the winning evidence path and keep experiment folders append-only.".into(),
        ];
    }
    let gate_status = novelty_str(state, "status", "pending");
    let claims = novelty_arr(state, "claims");
    if gate_status != "passed" {
        let mut actions = Vec::new();
        if claims.is_empty() {
            actions
                .push("\u{63d0}\u{70bc} 3 \u{5230} 5 \u{6761} novelty claims\u{ff0c}\u{5148}\u{5199}\u{8fdb} literature/NOVELTY_GATE.md\u{3002}".into());
        }
        actions.push(
            "\u{7528} research-claim \u{505a}\u{4e00}\u{8f6e}\u{5916}\u{90e8}\u{68c0}\u{7d22}\u{ff0c}\u{628a}\u{6700}\u{8fd1}\u{8bba}\u{6587}\u{8bc1}\u{636e}\u{5199}\u{8fdb} EXTERNAL_RESEARCH.md\u{3002}".into(),
        );
        actions.push("\u{591a}\u{4e2a} claim \u{65f6}\u{76f4}\u{63a5}\u{8dd1} research-all\u{ff0c}\u{518d}\u{7528} gate-from-research \u{770b}\u{7f3a}\u{53e3}\u{3002}".into());
        actions.push("\u{5148}\u{5b8c}\u{6210} novelty gate\u{ff0c}\u{518d}\u{542f}\u{52a8}\u{9ad8}\u{6210}\u{672c}\u{5b9e}\u{9a8c}\u{3002}".into());
        if gate_status == "pending" {
            actions.push("\u{7ed9}\u{6bcf}\u{6761} claim \u{6807}\u{6ce8} overlap level\u{ff0c}\u{5e76}\u{5199} differentiation strategy\u{3002}".into());
        }
        return actions;
    }
    let hypotheses = arr(state, "hypotheses");
    if hypotheses.is_empty() {
        return vec![
            "\u{8865} 3 \u{6761}\u{53ef}\u{6bd4}\u{8f83}\u{7684} hypothesis\u{ff0c}\u{5e76}\u{4e3a}\u{6bcf}\u{6761}\u{5199} prediction \u{548c} success threshold\u{3002}".into(),
            "\u{4ece}\u{6700}\u{9ad8}\u{4f18}\u{5148}\u{7ea7} hypothesis \u{5f00}\u{59cb}\u{ff0c}\u{4e0d}\u{8981}\u{5e76}\u{53d1}\u{6539}\u{540c}\u{4e00}\u{4efd}\u{7814}\u{7a76}\u{72b6}\u{6001}\u{3002}".into(),
        ];
    }
    let active = state
        .get("active_hypothesis")
        .and_then(Value::as_str)
        .and_then(|id| find_hypothesis(state, id));
    if active.is_none() {
        if let Some(candidate) = choose_backlog_hypothesis(state) {
            let candidate_id = str_field(candidate, "id");
            return vec![
                format!("\u{628a} {candidate_id} \u{8bbe}\u{4e3a} active hypothesis\u{ff0c}\u{5e76}\u{5148}\u{5199}\u{534f}\u{8bae}\u{3002}"),
                format!("\u{5728} experiments/{candidate_id}/ \u{4e0b}\u{843d} protocol \u{548c} run record\u{3002}"),
            ];
        }
        return vec!["\u{6e05}\u{7406} hypothesis \u{5217}\u{8868}\u{ff0c}\u{91cd}\u{65b0}\u{6307}\u{5b9a}\u{4e00}\u{4e2a} active hypothesis\u{3002}".into()];
    }
    let active = active.unwrap();
    let active_id = str_field(active, "id");
    let latest_run = latest_run_for_hypothesis(state, &active_id);
    if latest_run.is_none() {
        return vec![
            format!("\u{5148}\u{4e3a} {active_id} \u{5199} protocol\u{ff0c}\u{518d}\u{505a}\u{7b2c}\u{4e00}\u{8f6e} bounded run\u{3002}"),
            "\u{8dd1}\u{5b8c}\u{7acb}\u{523b}\u{8bb0}\u{5f55} metric\u{3001}sanity check \u{548c} rules in / rules out\u{3002}".into(),
        ];
    }
    let latest_run = latest_run.unwrap();
    if str_field_default(latest_run, "finding", "").is_empty()
        || str_field_default(latest_run, "decision_delta", "").is_empty()
        || str_field_default(latest_run, "reuse_note", "").is_empty()
    {
        return vec![
            format!(
                "\u{5148}\u{7ed9} {} \u{8865} reusable finding\u{3001}decision delta \u{548c} reuse note\u{ff0c}\u{907f}\u{514d}\u{53ea}\u{7559}\u{4e0b}\u{6d41}\u{6c34}\u{8d26}\u{3002}",
                str_field(latest_run, "run_id")
            ),
            format!(
                "\u{7528} annotate-run --run-id {} \u{8865}\u{9f50} applies-to / does-not-apply-to\u{3002}",
                str_field(latest_run, "run_id")
            ),
        ];
    }
    let latest_decision = latest_decision_for_hypothesis(state, &active_id);
    if latest_decision.is_none()
        || latest_decision.unwrap().get("run_id") != latest_run.get("run_id")
    {
        return vec![
            format!(
                "\u{5bf9} {} \u{505a} reflection\u{ff0c}\u{5e76}\u{660e}\u{786e}\u{9009} DEEPEN/BROADEN/PIVOT/CONCLUDE\u{3002}",
                str_field(latest_run, "run_id")
            ),
            "\u{628a}\u{7ed3}\u{679c}\u{5199}\u{56de} findings.md\u{ff0c}\u{800c}\u{4e0d}\u{662f}\u{53ea}\u{7559}\u{5728}\u{804a}\u{5929}\u{91cc}\u{3002}".into(),
        ];
    }
    match latest_decision
        .unwrap()
        .get("direction")
        .and_then(Value::as_str)
    {
        Some("DEEPEN") => vec![
            format!("\u{56f4}\u{7ed5} {active_id} \u{6536}\u{7d27}\u{53d8}\u{91cf}\u{ff0c}\u{518d}\u{505a}\u{4e00}\u{4e2a}\u{66f4}\u{5c0f}\u{66f4}\u{5e72}\u{51c0}\u{7684}\u{9a8c}\u{8bc1}\u{5b9e}\u{9a8c}\u{3002}"),
            "\u{53ea}\u{6539}\u{4e00}\u{4e2a}\u{5173}\u{952e}\u{56e0}\u{7d20}\u{ff0c}\u{907f}\u{514d}\u{628a}\u{56e0}\u{679c}\u{89e3}\u{91ca}\u{6405}\u{6df7}\u{3002}".into(),
        ],
        Some("BROADEN") => vec![
            format!("\u{628a} {active_id} \u{7684}\u{7ed3}\u{8bba}\u{6269}\u{5230}\u{7b2c}\u{4e8c}\u{4e2a} setting \u{6216} baseline\u{3002}"),
            "\u{4fdd}\u{6301}\u{534f}\u{8bae}\u{4e0d}\u{53d8}\u{ff0c}\u{53ea}\u{6269}\u{6570}\u{636e}\u{9762}\u{6216}\u{6bd4}\u{8f83}\u{9762}\u{3002}".into(),
        ],
        Some("PIVOT") => {
            if let Some(candidate) = choose_backlog_hypothesis(state) {
                let candidate_id = str_field(candidate, "id");
                if candidate_id != active_id {
                    return vec![
                        format!("\u{505c}\u{6b62}\u{7ee7}\u{7eed}\u{5806} {active_id}\u{ff0c}\u{5207}\u{5230} {candidate_id} \u{5f00}\u{65b0} protocol\u{3002}"),
                        "\u{628a}\u{65e7}\u{65b9}\u{5411}\u{5931}\u{8d25}\u{539f}\u{56e0}\u{5199}\u{6e05}\u{695a}\u{ff0c}\u{907f}\u{514d}\u{91cd}\u{590d}\u{8bd5}\u{9519}\u{3002}".into(),
                    ];
                }
            }
            vec![
                "\u{5f53}\u{524d}\u{65b9}\u{5411}\u{8be5} pivot\u{ff0c}\u{4f46}\u{8fd8}\u{7f3a}\u{65b0}\u{7684}\u{5019}\u{9009} hypothesis\u{3002}".into(),
                "\u{5148}\u{8865} hypothesis backlog\u{ff0c}\u{518d}\u{9009}\u{65b0}\u{7684} active hypothesis\u{3002}".into(),
            ]
        }
        _ => vec!["\u{8fdb}\u{5165} finalize\u{ff0c}\u{628a} strongest claim\u{3001}\u{8bc1}\u{636e}\u{94fe}\u{548c}\u{672a}\u{89e3}\u{51b3}\u{98ce}\u{9669}\u{6536}\u{675f}\u{6210} handoff\u{3002}".into()],
    }
}

// ── hypothesis management ─────────────────────────────────────────────

pub(crate) fn find_hypothesis<'a>(state: &'a Value, hypothesis_id: &str) -> Option<&'a Value> {
    arr(state, "hypotheses")
        .iter()
        .find(|item| item.get("id").and_then(Value::as_str) == Some(hypothesis_id))
}

pub(crate) fn find_hypothesis_index(state: &Value, hypothesis_id: &str) -> Option<usize> {
    arr(state, "hypotheses")
        .iter()
        .position(|item| item.get("id").and_then(Value::as_str) == Some(hypothesis_id))
}

fn choose_backlog_hypothesis(state: &Value) -> Option<&Value> {
    if arr(state, "hypotheses").is_empty() {
        return None;
    }
    for id in arr(state, "hypothesis_backlog")
        .iter()
        .filter_map(Value::as_str)
    {
        if let Some(candidate) = find_hypothesis(state, id) {
            if candidate.get("status").and_then(Value::as_str) != Some("concluded") {
                return Some(candidate);
            }
        }
    }
    let priority_order = HashMap::from([("high", 0), ("medium", 1), ("low", 2)]);
    let mut ranked = arr(state, "hypotheses").iter().collect::<Vec<_>>();
    ranked.sort_by(|a, b| {
        let pa = priority_order
            .get(
                a.get("priority")
                    .and_then(Value::as_str)
                    .unwrap_or("medium"),
            )
            .unwrap_or(&1);
        let pb = priority_order
            .get(
                b.get("priority")
                    .and_then(Value::as_str)
                    .unwrap_or("medium"),
            )
            .unwrap_or(&1);
        pa.cmp(pb)
            .then_with(|| str_field(a, "id").cmp(&str_field(b, "id")))
    });
    ranked.into_iter().next()
}

pub(crate) fn latest_run_for_hypothesis<'a>(
    state: &'a Value,
    hypothesis_id: &str,
) -> Option<&'a Value> {
    arr(state, "run_history")
        .iter()
        .rev()
        .find(|item| {
            item.get("hypothesis_id").and_then(Value::as_str) == Some(hypothesis_id)
        })
}

pub(crate) fn latest_run_by_id<'a>(state: &'a Value, run_id: &str) -> Option<&'a Value> {
    arr(state, "run_history")
        .iter()
        .rev()
        .find(|item| item.get("run_id").and_then(Value::as_str) == Some(run_id))
}

fn latest_decision_for_hypothesis<'a>(
    state: &'a Value,
    hypothesis_id: &str,
) -> Option<&'a Value> {
    arr(state, "decisions")
        .iter()
        .rev()
        .find(|item| {
            item.get("hypothesis_id").and_then(Value::as_str) == Some(hypothesis_id)
        })
}

fn next_run_id(state: &Value) -> String {
    format!("run-{:03}", arr(state, "run_history").len() + 1)
}

fn default_run_record_path(hypothesis_id: &str, run_id: &str) -> String {
    format!("experiments/{hypothesis_id}/{run_id}.md")
}

fn default_reflection_path(hypothesis_id: &str, run_id: Option<&str>) -> String {
    format!(
        "experiments/{hypothesis_id}/{}-reflection.md",
        run_id.unwrap_or("reflection")
    )
}

fn transition_hypothesis(
    state: &mut Value,
    hypothesis_index: usize,
    new_status: &str,
    reason: Option<&str>,
) -> Result<()> {
    let hypotheses = arr_mut(state, "hypotheses");
    let hypothesis = hypotheses
        .get_mut(hypothesis_index)
        .ok_or_else(|| anyhow!("Missing hypothesis index"))?;
    let previous = hypothesis.get("status").and_then(Value::as_str);
    let allowed = match previous {
        None => vec![
            "queued",
            "active",
            "needs_reflection",
            "parked",
            "concluded",
        ],
        Some("queued") => vec!["queued", "active", "parked", "concluded"],
        Some("active") => vec!["active", "needs_reflection", "parked", "concluded"],
        Some("needs_reflection") => vec!["needs_reflection", "active", "parked", "concluded"],
        Some("parked") => vec!["parked", "queued", "active", "concluded"],
        Some("concluded") => vec!["concluded"],
        _ => vec![],
    };
    if !allowed.contains(&new_status) {
        bail!(
            "Invalid hypothesis transition for {}: {} -> {}",
            str_field_default(hypothesis, "id", "?"),
            previous.unwrap_or("none"),
            new_status
        );
    }
    let hypothesis_id = str_field(hypothesis, "id");
    let item = hypothesis.as_object_mut().unwrap();
    item.insert("status".into(), json!(new_status));
    item.insert(
        "status_reason".into(),
        reason.map(Value::from).unwrap_or(Value::Null),
    );
    item.insert("status_updated_at".into(), json!(now_iso()));
    let backlog = arr_mut(state, "hypothesis_backlog");
    if new_status == "queued" {
        if !backlog
            .iter()
            .any(|item| item.as_str() == Some(&hypothesis_id))
        {
            backlog.push(json!(hypothesis_id));
        }
    } else if let Some(index) = backlog
        .iter()
        .position(|item| item.as_str() == Some(&hypothesis_id))
    {
        backlog.remove(index);
    }
    Ok(())
}

pub(crate) struct HypothesisInput<'a> {
    pub claim: &'a str,
    pub prediction: Option<&'a str>,
    pub mechanism: Option<&'a str>,
    pub falsifiable_prediction: Option<&'a str>,
    pub success_threshold: Option<&'a str>,
    pub stop_condition: Option<&'a str>,
    pub baselines: &'a [String],
    pub confounders: &'a [String],
    pub negative_signals: &'a [String],
    pub minimal_test: Option<&'a str>,
    pub priority: &'a str,
    pub hypothesis_id: Option<&'a str>,
}

pub(crate) fn add_hypothesis(state: &Value, input: HypothesisInput<'_>) -> Result<Value> {
    let mut next_state = crate::state::ensure_state_defaults(state);
    let resolved_id = input
        .hypothesis_id
        .map(ToString::to_string)
        .unwrap_or_else(|| slugify(input.claim).chars().take(40).collect());
    if find_hypothesis(&next_state, &resolved_id).is_some() {
        bail!("Hypothesis already exists: {resolved_id}");
    }
    let entry = json!({
        "id": resolved_id,
        "claim": input.claim,
        "prediction": input.prediction,
        "mechanism": optional_string(input.mechanism),
        "falsifiable_prediction": optional_string(input.falsifiable_prediction.or(input.prediction)),
        "success_threshold": optional_string(input.success_threshold),
        "stop_condition": optional_string(input.stop_condition),
        "baselines": string_vec(input.baselines),
        "confounders": string_vec(input.confounders),
        "negative_signals": string_vec(input.negative_signals),
        "minimal_test": optional_string(input.minimal_test),
        "priority": input.priority,
        "status": "queued",
        "status_reason": Value::Null,
        "status_updated_at": now_iso(),
        "created_at": now_iso(),
    });
    arr_mut(&mut next_state, "hypotheses").push(entry);
    let backlog = arr_mut(&mut next_state, "hypothesis_backlog");
    if !backlog
        .iter()
        .any(|item| item.as_str() == Some(&resolved_id))
    {
        backlog.push(json!(resolved_id.clone()));
    }
    if next_state
        .get("active_hypothesis")
        .and_then(Value::as_str)
        .is_none()
        && novelty_str(&next_state, "status", "pending") == "passed"
    {
        set_key(
            &mut next_state,
            "active_hypothesis",
            json!(resolved_id.clone()),
        );
        let index = find_hypothesis_index(&next_state, &resolved_id).unwrap();
        transition_hypothesis(
            &mut next_state,
            index,
            "active",
            Some("first active hypothesis after novelty gate passed"),
        )?;
    }
    Ok(next_state)
}

pub(crate) struct RunAnnotationInput<'a> {
    pub finding: Option<&'a str>,
    pub decision_delta: Option<&'a str>,
    pub reuse_note: Option<&'a str>,
    pub applies_to: &'a [String],
    pub does_not_apply_to: &'a [String],
}

pub(crate) fn annotate_run(
    state: &Value,
    run_id: &str,
    input: RunAnnotationInput<'_>,
) -> Result<Value> {
    let mut next_state = crate::state::ensure_state_defaults(state);
    let Some(record) = arr_mut(&mut next_state, "run_history")
        .iter_mut()
        .find(|item| item.get("run_id").and_then(Value::as_str) == Some(run_id))
    else {
        bail!("Unknown run id: {run_id}");
    };
    let item = record.as_object_mut().expect("run record must be object");
    if let Some(value) = input.finding {
        let value = optional_string(Some(value));
        if !value.is_null() {
            item.insert("finding".into(), value);
        }
    }
    if let Some(value) = input.decision_delta {
        let value = optional_string(Some(value));
        if !value.is_null() {
            item.insert("decision_delta".into(), value);
        }
    }
    if let Some(value) = input.reuse_note {
        let value = optional_string(Some(value));
        if !value.is_null() {
            item.insert("reuse_note".into(), value);
        }
    }
    let applies_to = merge_string_array(
        item.get("applies_to").unwrap_or(&Value::Null),
        input.applies_to,
    );
    item.insert("applies_to".into(), applies_to);
    let does_not_apply_to = merge_string_array(
        item.get("does_not_apply_to").unwrap_or(&Value::Null),
        input.does_not_apply_to,
    );
    item.insert("does_not_apply_to".into(), does_not_apply_to);
    item.insert("reuse_annotated_at".into(), json!(now_iso()));
    Ok(next_state)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn record_run(
    state: &Value,
    hypothesis_id: &str,
    outcome: &str,
    summary: &str,
    metric_name: Option<&str>,
    metric_value: Option<&str>,
    command: Option<&str>,
    evidence_path: Option<&str>,
    sanity_checks: &[String],
    baseline_result: Option<&str>,
    rules_in: &[String],
    rules_out: &[String],
    alternative_explanations: &[String],
    threats: &[String],
    interpretation: Option<&str>,
    finding: Option<&str>,
    decision_delta: Option<&str>,
    reuse_note: Option<&str>,
    applies_to: &[String],
    does_not_apply_to: &[String],
    override_novelty_gate: bool,
    override_reason: Option<&str>,
    workspace: &std::path::Path,
) -> Result<Value> {
    let mut next_state = crate::state::ensure_state_defaults(state);
    let Some(index) = find_hypothesis_index(&next_state, hypothesis_id) else {
        bail!("Unknown hypothesis: {hypothesis_id}");
    };
    let gate_status = novelty_str(&next_state, "status", "pending");
    if gate_status != "passed" {
        if !override_novelty_gate {
            bail!("Novelty gate must pass before recording runs (current: {gate_status})");
        }
        if override_reason.unwrap_or("").trim().is_empty() {
            bail!("Novelty gate override requires --override-reason");
        }
    }
    let current_status = find_hypothesis(&next_state, hypothesis_id)
        .and_then(|item| item.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("queued")
        .to_string();
    if !["active", "queued"].contains(&current_status.as_str()) {
        bail!("Hypothesis {hypothesis_id} must be active or queued before a run, current status: {current_status}");
    }
    if current_status == "queued" {
        transition_hypothesis(
            &mut next_state,
            index,
            "active",
            Some("activated by first recorded run"),
        )?;
    }
    let run_id = next_run_id(&next_state);
    set_key(&mut next_state, "stage", json!(STAGE_OUTER_LOOP));
    set_key(&mut next_state, "active_hypothesis", json!(hypothesis_id));
    let environment = if workspace.join("research-state.yaml").exists() {
        next_state
            .get("environment")
            .filter(|value| !value.is_null())
            .cloned()
            .unwrap_or_else(|| crate::state::capture_environment_fingerprint(workspace))
    } else {
        crate::state::capture_environment_fingerprint(workspace)
    };
    let provenance = if workspace.join("research-state.yaml").exists() {
        next_state
            .get("git")
            .filter(|value| !value.is_null())
            .cloned()
            .unwrap_or_else(|| crate::state::capture_git_provenance(workspace))
    } else {
        crate::state::capture_git_provenance(workspace)
    };
    set_key(&mut next_state, "environment", environment.clone());
    set_key(&mut next_state, "git", provenance.clone());
    let record = json!({
        "run_id": run_id,
        "hypothesis_id": hypothesis_id,
        "outcome": outcome,
        "summary": summary,
        "metric_name": metric_name,
        "metric_value": metric_value,
        "command": command,
        "evidence_path": evidence_path.map(ToString::to_string).unwrap_or_else(|| default_run_record_path(hypothesis_id, &run_id)),
        "sanity_checks": string_vec(sanity_checks),
        "baseline_result": optional_string(baseline_result),
        "rules_in": string_vec(rules_in),
        "rules_out": string_vec(rules_out),
        "alternative_explanations": string_vec(alternative_explanations),
        "threats": string_vec(threats),
        "interpretation": optional_string(interpretation),
        "finding": optional_string(finding),
        "decision_delta": optional_string(decision_delta),
        "reuse_note": optional_string(reuse_note),
        "applies_to": string_vec(applies_to),
        "does_not_apply_to": string_vec(does_not_apply_to),
        "novelty_gate_status_at_run": gate_status,
        "novelty_gate_override": override_novelty_gate,
        "override_reason": override_reason,
        "environment_fingerprint": environment,
        "git_provenance": provenance,
        "recorded_at": now_iso(),
    });
    arr_mut(&mut next_state, "run_history").push(record.clone());
    transition_hypothesis(
        &mut next_state,
        index,
        "needs_reflection",
        Some(&format!("{run_id} recorded")),
    )?;
    arr_mut(&mut next_state, "evidence_index").push(json!({
        "run_id": run_id,
        "path": record.get("evidence_path").cloned().unwrap_or(Value::Null),
        "added_at": now_iso(),
    }));
    Ok(next_state)
}

pub(crate) fn reflect(
    state: &Value,
    hypothesis_id: &str,
    direction: &str,
    reason: &str,
    next_step: Option<&str>,
    activate_hypothesis: Option<&str>,
) -> Result<Value> {
    let mut next_state = crate::state::ensure_state_defaults(state);
    let Some(index) = find_hypothesis_index(&next_state, hypothesis_id) else {
        bail!("Unknown hypothesis: {hypothesis_id}");
    };
    let status = find_hypothesis(&next_state, hypothesis_id)
        .and_then(|item| item.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("-");
    if status != "needs_reflection" {
        bail!("Hypothesis {hypothesis_id} must be in needs_reflection before reflect, current status: {status}");
    }
    let latest_run = latest_run_for_hypothesis(&next_state, hypothesis_id)
        .ok_or_else(|| anyhow!("Cannot reflect without a recorded run for {hypothesis_id}"))?;
    if let Some(latest_decision) = latest_decision_for_hypothesis(&next_state, hypothesis_id) {
        if latest_decision.get("run_id") == latest_run.get("run_id") {
            bail!(
                "Run {} already has a reflection",
                str_field(latest_run, "run_id")
            );
        }
    }
    let run_id = str_field(latest_run, "run_id");
    let decision = json!({
        "hypothesis_id": hypothesis_id,
        "run_id": run_id,
        "direction": direction,
        "reason": reason,
        "next_step": next_step,
        "note_path": default_reflection_path(hypothesis_id, Some(&run_id)),
        "recorded_at": now_iso(),
    });
    arr_mut(&mut next_state, "decisions").push(decision);
    set_key(&mut next_state, "current_direction", json!(direction));
    match direction {
        "CONCLUDE" => {
            set_key(&mut next_state, "status", json!("concluded"));
            set_key(&mut next_state, "stage", json!(STAGE_FINALIZE));
            transition_hypothesis(&mut next_state, index, "concluded", Some(reason))?;
        }
        "PIVOT" => {
            set_key(&mut next_state, "stage", json!(STAGE_INNER_LOOP));
            transition_hypothesis(&mut next_state, index, "parked", Some(reason))?;
        }
        _ => {
            set_key(&mut next_state, "stage", json!(STAGE_INNER_LOOP));
            transition_hypothesis(&mut next_state, index, "active", Some(reason))?;
        }
    }
    if let Some(target_id) = activate_hypothesis {
        let Some(target_index) = find_hypothesis_index(&next_state, target_id) else {
            bail!("Unknown activate_hypothesis: {target_id}");
        };
        set_key(&mut next_state, "active_hypothesis", json!(target_id));
        let target_status = find_hypothesis(&next_state, target_id)
            .and_then(|item| item.get("status"))
            .and_then(Value::as_str)
            .unwrap_or("-");
        if target_status == "queued" {
            transition_hypothesis(
                &mut next_state,
                target_index,
                "active",
                Some("activated after pivot"),
            )?;
        } else if target_status == "parked" {
            transition_hypothesis(
                &mut next_state,
                target_index,
                "active",
                Some("reactivated after pivot"),
            )?;
        }
    } else if direction != "CONCLUDE" {
        set_key(&mut next_state, "active_hypothesis", json!(hypothesis_id));
    }
    Ok(next_state)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn add_claim_comparison(
    state: &Value,
    claim: &str,
    axis: &str,
    closest_prior_work: &str,
    overlap: &str,
    difference: &str,
    confidence: &str,
    verdict: &str,
    claim_id: Option<&str>,
) -> Value {
    let mut next_state = crate::state::ensure_state_defaults(state);
    let resolved_id = claim_id
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("C{}", novelty_arr(&next_state, "claim_records").len() + 1));
    let record = json!({
        "claim_id": resolved_id,
        "claim": claim,
        "axis": axis,
        "closest_prior_work": closest_prior_work,
        "overlap": overlap,
        "difference": difference,
        "confidence": confidence,
        "verdict": verdict,
        "recorded_at": now_iso(),
    });
    let gate = novelty_gate_mut(&mut next_state);
    let records = gate
        .entry("claim_records")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .unwrap();
    if let Some(index) = records
        .iter()
        .position(|item| item.get("claim_id").and_then(Value::as_str) == Some(&resolved_id))
    {
        records[index] = record;
    } else {
        records.push(record);
    }
    let prioritized = prioritize_claims(records);
    gate.insert("claim_records".into(), json!(prioritized.clone()));
    gate.insert(
        "claims".into(),
        json!(prioritized
            .iter()
            .map(|item| str_field(item, "claim"))
            .collect::<Vec<_>>()),
    );
    gate.insert(
        "overlap_summary".into(),
        json!(prioritized
            .iter()
            .map(|item| format!(
                "{}={}",
                str_field(item, "claim_id"),
                str_field(item, "overlap")
            ))
            .collect::<Vec<_>>()
            .join(", ")),
    );
    next_state
}
