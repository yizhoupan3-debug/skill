//! Search query building, claim priority scoring, and novelty briefs.
//!
//! 从 autoresearch-rs/search.rs 迁入策略层逻辑。

use serde_json::{json, Value};

use crate::util::{novelty_gate, novelty_gate_mut, str_field, str_field_default};

// ── 自包含辅助函数 ──

/// 提取内容词（去停用词、去短词、去重）。
fn compact_words(text: &str, limit: usize) -> Vec<String> {
    let stopwords: &[&str] = &[
        "the", "a", "an", "is", "are", "was", "were", "be", "been", "have", "has",
        "do", "does", "did", "will", "would", "could", "should", "may", "might",
        "of", "in", "for", "on", "with", "at", "by", "from", "as", "and", "but",
        "or", "not", "this", "that", "these", "those", "it", "its", "we", "our",
        "they", "their", "into", "through", "during", "before", "after", "than",
        "more", "most", "other", "some", "such", "no", "only", "own", "same",
        "to", "is", "are", "was", "be", "have", "had", "do", "did", "will",
        "can", "may", "shall", "could", "would", "should", "might", "must",
    ];
    let mut seen = std::collections::HashSet::new();
    let mut words = Vec::new();
    for word in text.split(|c: char| !c.is_alphanumeric() && c != '_') {
        let lower = word.to_ascii_lowercase();
        if lower.len() >= 3 && !stopwords.contains(&lower.as_str()) && seen.insert(lower.clone()) {
            words.push(lower);
            if words.len() >= limit { break; }
        }
    }
    words
}

// ── 公开 API ──

/// 为给定 axis 生成默认的 required evidence 列表。
pub fn default_required_evidence(axis: &str) -> Vec<String> {
    let axis_lower = axis.to_lowercase();
    if axis_lower.contains("method") || axis_lower.contains("workflow") {
        return vec![
            "Direct overlap papers using the same mechanism".into(),
            "Nearest baseline implementations".into(),
            "Claims about what is structurally different".into(),
        ];
    }
    if axis_lower.contains("setting") || axis_lower.contains("domain") || axis_lower.contains("task") {
        return vec![
            "Prior work in the same domain or task".into(),
            "Recent competitors in the last 3 years".into(),
            "Evidence that the setting is materially different".into(),
        ];
    }
    vec![
        "Closest prior work for the same core claim".into(),
        "Recent competitors from the last 3 years".into(),
        "Evidence for the exact differentiation sentence".into(),
    ]
}

/// 为 claim 生成 4 种搜索查询变体（broad/focused/recent/combination）。
pub fn build_search_queries(claim: &str, axis: &str) -> Vec<Value> {
    let keywords = compact_words(claim, 6);
    let broad_terms = if keywords.is_empty() { claim.to_string() } else { keywords.iter().take(3).cloned().collect::<Vec<_>>().join(" ") };
    let focused_terms = if keywords.is_empty() { claim.to_string() } else { keywords.iter().take(5).cloned().collect::<Vec<_>>().join(" ") };
    let combination_terms = if keywords.len() >= 4 {
        format!("{} {} {} {}", keywords[0], keywords[1], keywords[keywords.len() - 2], keywords[keywords.len() - 1])
    } else { focused_terms.clone() };
    let axis_hint = if axis.trim().is_empty() { "claim" } else { axis.trim() }.to_lowercase();
    vec![
        json!({"label": "broad", "query": broad_terms}),
        json!({"label": "focused", "query": format!("{focused_terms} {axis_hint}").trim()}),
        json!({"label": "recent", "query": format!("{focused_terms} last 3 years").trim()}),
        json!({"label": "combination", "query": combination_terms}),
    ]
}

/// Axis 权重：(novelty, cost, reviewer)。
pub fn axis_weights(axis: &str) -> (i64, i64, i64) {
    match axis {
        "method" | "workflow" => (5, 2, 3),
        "task" => (4, 3, 4),
        "comparison" => (4, 1, 5),
        "setting" => (3, 4, 2),
        "framing" => (2, 1, 4),
        _ => (3, 2, 3),
    }
}

/// 计算 claim 优先级评分。
pub fn score_claim_priority(record: &Value) -> Value {
    let axis = str_field_default(record, "axis", "claim").to_lowercase();
    let (mut novelty, mut cost, mut reviewer) = axis_weights(&axis);
    match record.get("overlap").and_then(Value::as_str) {
        Some("low") => novelty += 2,
        Some("medium") => novelty += 1,
        Some("high") => { novelty -= 1; reviewer += 1; }
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
        Some("risky") => { reviewer += 1; cost += 1; }
        Some("not-novel") => { novelty -= 2; cost += 1; }
        _ => {}
    }
    let score = novelty * 3 + reviewer * 2 - cost * 2;
    let label = if score >= 18 { "first" } else if score >= 13 { "next" } else { "later" };
    let reason = if novelty >= reviewer && cost <= 2 { "high novelty upside with cheap verification" }
        else if reviewer >= novelty && cost <= 3 { "reviewer pressure is high" }
        else if cost >= 4 { "verification is expensive" }
        else { "worth checking, but not the best first target" };
    let mut out = record.clone();
    let map = out.as_object_mut().expect("claim record must be object");
    map.insert("priority_score".into(), json!(score));
    map.insert("priority_label".into(), json!(label));
    map.insert("priority_reason".into(), json!(reason));
    out
}

/// 按优先级排序 claims。
pub fn prioritize_claims(records: &[Value]) -> Vec<Value> {
    let mut scored: Vec<Value> = records.iter().map(score_claim_priority).collect();
    scored.sort_by(|a, b| {
        let sa = a.get("priority_score").and_then(Value::as_i64).unwrap_or(0);
        let sb = b.get("priority_score").and_then(Value::as_i64).unwrap_or(0);
        sb.cmp(&sa)
    });
    for (i, record) in scored.iter_mut().enumerate() {
        if let Some(obj) = record.as_object_mut() {
            obj.insert("recommended_order".into(), json!(i + 1));
        }
    }
    scored
}

/// Get the highest-priority claim from a slice of claim records.
pub fn top_priority_claim_from_records(records: &[Value]) -> Option<Value> {
    prioritize_claims(records).into_iter().next()
}

/// 为 axis 生成预期 baselines 列表。
pub fn expected_baselines_for_axis(axis: &str) -> Vec<String> {
    match axis {
        "method" => vec!["Random baseline".into(), "Previous SOTA".into(), "Ablation without key component".into()],
        "task" => vec!["Same task, different method".into(), "Same method, different task".into(), "Transfer baseline".into()],
        "setting" => vec!["In-domain baseline".into(), "Out-of-domain baseline".into(), "Zero-shot baseline".into()],
        _ => vec!["Naive baseline".into(), "Prior work baseline".into(), "Oracle baseline".into()],
    }
}

/// 验证标准描述。
pub fn verification_standard_for_priority(priority: &str) -> &'static str {
    match priority {
        "first" => "Must have at least one direct overlap paper and one structural difference claim.",
        "next" => "Needs either a closest-prior-work comparison or a recent competitor.",
        "later" => "Can proceed with weaker evidence if the claim is narrow and low-risk.",
        _ => "Standard verification.",
    }
}

// ── State-aware novelty gate helpers ──

fn novelty_arr<'a>(state: &'a Value, key: &str) -> &'a [Value] {
    novelty_gate(state)
        .get(key)
        .and_then(Value::as_array)
        .map(|a| a.as_slice())
        .unwrap_or(&[])
}

// ── State-aware search plan & brief ──

/// Get the highest-priority claim from the state's novelty gate.
pub fn top_priority_claim(state: &Value) -> Option<Value> {
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
                .then_with(|| str_field(a, "claim_id").cmp(str_field(b, "claim_id")))
        });
        return ranked.into_iter().next();
    }
    None
}

/// Build a search plan entry for a single claim record.
pub fn build_search_plan_entry(record: &Value) -> Value {
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
        "keywords": compact_words(claim, 6),
        "queries": build_search_queries(claim, axis),
        "sources": ["Semantic Scholar", "arXiv", "Google Scholar"],
        "required_evidence": default_required_evidence(axis),
    })
}

/// Generate the full search plan from the state's novelty gate.
pub fn current_search_plan(state: &Value) -> Vec<Value> {
    let source_records = if !novelty_arr(state, "claim_records").is_empty() {
        novelty_arr(state, "claim_records").to_vec()
    } else {
        novelty_arr(state, "draft_claims").to_vec()
    };
    let mut plan: Vec<Value> = source_records
        .iter()
        .map(build_search_plan_entry)
        .collect();
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
            .then_with(|| str_field(a, "claim_id").cmp(str_field(b, "claim_id")))
    });
    plan
}

/// Get the recommended focus for the current state.
pub fn current_recommended_focus(state: &Value) -> Option<String> {
    let claim = top_priority_claim(state)?;
    Some(format!(
        "{}: {}",
        str_field_default(&claim, "claim_id", "C?"),
        str_field_default(&claim, "claim", "_No claim recorded._")
    ))
}

/// Generate the novelty brief for the top-priority claim.
pub fn current_brief(state: &Value) -> Option<Value> {
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
        "verification_standard": verification_standard_for_priority(str_field_default(&top, "priority_label", "later")),
        "sources": matching.and_then(|item| item.get("sources")).cloned().unwrap_or(json!(["Semantic Scholar", "arXiv", "Google Scholar"])),
        "queries": matching.and_then(|item| item.get("queries")).cloned().unwrap_or_else(|| json!(build_search_queries(str_field(&top, "claim"), axis))),
        "required_evidence": matching.and_then(|item| item.get("required_evidence")).cloned().unwrap_or_else(|| json!(default_required_evidence(axis))),
        "expected_baselines": expected_baselines_for_axis(axis),
    }))
}

/// Refresh novelty-related views (search plan, recommended focus, brief) in the state.
pub fn refresh_novelty_views(state: &mut Value) {
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

// ── 测试 ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_search_queries_returns_four() {
        let queries = build_search_queries("neural architecture search efficiency", "method");
        assert_eq!(queries.len(), 4);
        let labels: Vec<&str> = queries.iter().filter_map(|q| q.get("label").and_then(Value::as_str)).collect();
        assert!(labels.contains(&"broad"));
        assert!(labels.contains(&"focused"));
        assert!(labels.contains(&"recent"));
        assert!(labels.contains(&"combination"));
    }

    #[test]
    fn axis_weights_known() {
        assert_eq!(axis_weights("method"), (5, 2, 3));
        assert_eq!(axis_weights("task"), (4, 3, 4));
    }

    #[test]
    fn score_claim_priority_novel_low_overlap() {
        let record = json!({"axis": "method", "overlap": "low", "confidence": "high", "verdict": "novel"});
        let scored = score_claim_priority(&record);
        let score = scored.get("priority_score").and_then(Value::as_i64).unwrap();
        assert!(score > 15, "expected high score, got {score}");
        assert_eq!(scored.get("priority_label").and_then(Value::as_str), Some("first"));
    }

    #[test]
    fn score_claim_priority_not_novel_high_overlap() {
        let record = json!({"axis": "method", "overlap": "high", "confidence": "low", "verdict": "not-novel"});
        let scored = score_claim_priority(&record);
        let score = scored.get("priority_score").and_then(Value::as_i64).unwrap();
        assert!(score < 13, "expected low score, got {score}");
    }

    #[test]
    fn prioritize_claims_sorts_by_score() {
        let claims = vec![
            json!({"claim_id": "C2", "axis": "method", "overlap": "high", "verdict": "not-novel", "confidence": "low"}),
            json!({"claim_id": "C1", "axis": "method", "overlap": "low", "verdict": "novel", "confidence": "high"}),
        ];
        let prioritized = prioritize_claims(&claims);
        assert_eq!(prioritized[0].get("claim_id").and_then(Value::as_str), Some("C1"));
        assert_eq!(prioritized[0].get("recommended_order").and_then(Value::as_i64), Some(1));
    }

    #[test]
    fn default_required_evidence_method() {
        let evidence = default_required_evidence("method");
        assert_eq!(evidence.len(), 3);
    }

    #[test]
    fn expected_baselines_for_axis_all_variants() {
        for axis in ["method", "task", "setting", "unknown"] {
            let baselines = expected_baselines_for_axis(axis);
            assert_eq!(baselines.len(), 3, "axis={axis} should have 3 baselines");
        }
    }

    #[test]
    fn top_priority_claim_from_state() {
        let state = json!({
            "novelty_gate": {
                "draft_claims": [
                    {"claim_id": "C1", "axis": "method", "claim": "test claim"}
                ]
            }
        });
        let claim = top_priority_claim(&state);
        assert!(claim.is_some());
        assert_eq!(claim.unwrap().get("claim_id").and_then(Value::as_str), Some("C1"));
    }

    #[test]
    fn top_priority_claim_empty_state() {
        let state = json!({});
        assert!(top_priority_claim(&state).is_none());
    }

    #[test]
    fn current_search_plan_generates_entries() {
        let state = json!({
            "novelty_gate": {
                "draft_claims": [
                    {"claim_id": "C1", "axis": "method", "claim": "test claim"}
                ]
            }
        });
        let plan = current_search_plan(&state);
        assert_eq!(plan.len(), 1);
        assert!(plan[0].get("queries").unwrap().is_array());
    }

    #[test]
    fn current_recommended_focus_returns_top_claim() {
        let state = json!({
            "novelty_gate": {
                "draft_claims": [
                    {"claim_id": "C1", "axis": "method", "claim": "my claim"}
                ]
            }
        });
        let focus = current_recommended_focus(&state);
        assert!(focus.is_some());
        assert!(focus.unwrap().contains("C1"));
    }

    #[test]
    fn current_brief_returns_brief() {
        let state = json!({
            "novelty_gate": {
                "draft_claims": [
                    {"claim_id": "C1", "axis": "method", "claim": "test claim", "priority_label": "first"}
                ]
            }
        });
        let brief = current_brief(&state);
        assert!(brief.is_some());
        let brief = brief.unwrap();
        assert_eq!(brief.get("claim_id").and_then(Value::as_str), Some("C1"));
        assert!(brief.get("decision_goal").is_some());
    }

    #[test]
    fn refresh_novelty_views_populates_gate() {
        let mut state = json!({
            "novelty_gate": {
                "draft_claims": [
                    {"claim_id": "C1", "axis": "method", "claim": "test claim"}
                ]
            }
        });
        refresh_novelty_views(&mut state);
        let gate = state.get("novelty_gate").unwrap();
        assert!(gate.get("search_plan").unwrap().is_array());
        assert!(gate.get("recommended_focus").is_some());
    }

    #[test]
    fn build_search_plan_entry_has_required_fields() {
        let record = json!({
            "claim_id": "C1",
            "claim": "neural architecture search",
            "axis": "method",
            "priority_score": 15,
            "priority_label": "first"
        });
        let entry = build_search_plan_entry(&record);
        assert_eq!(entry.get("claim_id").and_then(Value::as_str), Some("C1"));
        assert!(entry.get("queries").unwrap().is_array());
        assert!(entry.get("sources").unwrap().is_array());
        assert!(entry.get("required_evidence").unwrap().is_array());
    }
}
