//! Search query building, claim priority scoring, and novelty briefs.

use serde_json::{Value, json};

use crate::*;

pub(super) fn default_required_evidence(axis: &str) -> Vec<String> {
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

pub(super) fn build_search_queries(claim: &str, axis: &str) -> Vec<Value> {
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

pub(super) fn axis_weights(axis: &str) -> (i64, i64, i64) {
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

pub(super) fn score_claim_priority(record: &Value) -> Value {
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

pub(super) fn prioritize_claims(claims: &[Value]) -> Vec<Value> {
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

pub(super) fn top_priority_claim(state: &Value) -> Option<Value> {
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

pub(super) fn current_recommended_focus(state: &Value) -> Option<String> {
    let claim = top_priority_claim(state)?;
    Some(format!(
        "{}: {}",
        str_field_default(&claim, "claim_id", "C?"),
        str_field_default(&claim, "claim", "_No claim recorded._")
    ))
}

pub(super) fn build_search_plan_entry(record: &Value) -> Value {
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

pub(super) fn current_search_plan(state: &Value) -> Vec<Value> {
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

pub(super) fn refresh_novelty_views(state: &mut Value) {
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

pub(super) fn expected_baselines_for_axis(axis: &str) -> Vec<String> {
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

pub(super) fn verification_standard_for_priority(label: &str) -> &'static str {
    match label {
        "first" => "You should be able to decide proceed vs reframe after one focused search pass.",
        "next" => "This should be checked after the first claim is clarified, not before.",
        _ => "Useful later, but not strong enough to spend the first search budget on.",
    }
}

pub(super) fn current_brief(state: &Value) -> Option<Value> {
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
