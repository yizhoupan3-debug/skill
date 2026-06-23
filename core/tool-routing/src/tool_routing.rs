//! Tool routing: simple scoring pipeline for tool selection.

use crate::tool_types::{ToolDecision, ToolRecord};

/// Score a tool record against a query.
fn score_tool(record: &ToolRecord, query_lower: &str) -> (f64, Vec<String>) {
    let mut score = 0.0;
    let mut reasons = Vec::new();

    // Exact name match
    if query_lower.contains(&record.slug) {
        score += 100.0;
        reasons.push(format!("Exact tool name matched: {}.", record.slug));
    }

    // Name tokens match
    for token in &record.name_tokens {
        if query_lower.contains(token.as_str()) {
            score += 20.0;
            reasons.push(format!("Name token matched: {}.", token));
        }
    }

    // Trigger hints match
    for hint in &record.trigger_hints {
        if query_lower.contains(&hint.to_lowercase()) {
            score += 15.0;
            reasons.push(format!("Trigger hint matched: {}.", hint));
        }
    }

    // Gate match
    if record.gate == "artifact" && has_artifact_context(query_lower) {
        score += 10.0;
        reasons.push("Artifact gate context matched.".to_string());
    } else if record.gate == "source" && has_source_context(query_lower) {
        score += 10.0;
        reasons.push("Source gate context matched.".to_string());
    }

    (score, reasons)
}

fn has_artifact_context(query: &str) -> bool {
    query.contains("ppt")
        || query.contains("pdf")
        || query.contains("docx")
        || query.contains("xlsx")
        || query.contains("幻灯片")
        || query.contains("演示")
        || query.contains("文档")
}

fn has_source_context(query: &str) -> bool {
    query.contains("github")
        || query.contains("源码")
        || query.contains("citation")
        || query.contains("引用")
        || query.contains("financial")
        || query.contains("金融")
}

/// Route a query to the best matching tool.
pub fn route_tool(
    records: &[ToolRecord],
    query: &str,
    host_id: Option<&str>,
) -> Option<ToolDecision> {
    let query_lower = query.to_lowercase();

    let candidates: Vec<(&ToolRecord, f64, Vec<String>)> = records
        .iter()
        .filter(|r| {
            host_id.is_none() || r.host_platforms.iter().any(|h| h == host_id.unwrap())
        })
        .map(|r| {
            let (score, reasons) = score_tool(r, &query_lower);
            (r, score, reasons)
        })
        .filter(|(_, score, _)| *score > 0.0)
        .collect();

    if candidates.is_empty() {
        return None;
    }

    let (best, score, reasons) = candidates
        .into_iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap();

    Some(ToolDecision {
        selected_tool: best.slug.clone(),
        score,
        reasons,
        mcp_endpoint: best.mcp_endpoint.clone(),
        binary: best.binary.clone(),
    })
}
